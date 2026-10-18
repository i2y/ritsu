//! A rule as one step of a state machine (§15.148).
//!
//! `machine … over …` names one output that comes back as one input on the next call. The
//! generated function does not change — it is passed the state and answers the next one, and
//! the host keeps it — so everything here is about **sequences of calls**: which states a case
//! can reach, whether it can always get to an end, whether an end is really an end, whether a
//! state is reached after another, whether a value is answered twice.
//!
//! Those questions are decidable for the reason a table's are. The state is a finite enum, and
//! every other input is cut by the rule's own boundaries into finitely many classes: the cells
//! `diff` walks (§15.122). Each cell is settled by an input that realizes it, and what the rule
//! answers there is a transition, from the state the cell has to the state the rule answers.
//! The claims are then searches over a finite graph, and a broken one comes back as the
//! shortest sequence of calls that breaks it, every call an input somebody could send.
//!
//! Three-valued, as `diff` is (§15.99): a cell no input was built for and none was shown
//! impossible is unsettled, and a claim that could turn on one is reported as undecided
//! (W127), never as holding.
//!
//! A machine may carry more than its state (§15.217): a number with a range, a boolean or
//! another enum, each on a `carry` line of its own. What a case is in is then the state
//! together with those values, and a node of the searches is that combination. While the
//! combinations are few enough for the budget, every value of a carried number is a coordinate
//! of its own, a node is one place a case can really be, and the claims come out exact, as
//! before. Past that, a number is cut into intervals at the boundaries that matter to it, and a
//! call from an interval goes to every interval its next value can fall in: more transitions
//! than happen, so a claim that nothing bad is reached still holds where it holds there. A
//! claim broken over the intervals is made again from calls that really happen, from where a
//! case starts, and is reported broken only when that succeeds; otherwise it is W127.

use crate::ast::*;
use crate::diag::{Diag, RowRef, Span, Step, WVal, Witness};
use crate::eval::Val;
use crate::num::{Rat, RoundMode, RoundTo};
use crate::types::{Checked, Ty};
use crate::vdiff::{Axis, Coord, Settled, Walker};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

/// How many region nodes of `--budget` one cell of the machine's walk counts as. A cell is
/// settled by running the rule, which costs about what fifty nodes of the region check do, so
/// the default budget of the check lets the walk visit a million cells — the point where
/// `diff` stops feeling like a gate (§15.122).
pub const NODES_PER_CELL: usize = 50;

/// How far the search for calls that really happen goes when the carried numbers are walked in
/// intervals (§15.217): the states a case is found in, and the calls the rule is asked. Past
/// either, what was not found stays unsettled (W127).
pub const REAL_STATES: usize = 20_000;
pub const REAL_RUNS: usize = 200_000;

/// The coordinate of a carried number a call answers outside its input's range (§15.217). A
/// case that gets there holds a value no next call can be passed — the door refuses it — so
/// nothing leads out; a final state reached that way still ends the case.
pub const OUT: usize = usize::MAX;

/// Whether one call's answer counts for one `once` line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Flag {
    No,
    Yes,
    /// Some call of this class counts and some may not, or it was not shown either way.
    Maybe,
}

/// One transition: a class of calls from one state, with a call that shows it.
#[derive(Debug, Clone)]
pub struct Edge {
    pub from: String,
    pub to: String,
    /// One per `once` line, in order.
    pub flags: Vec<Flag>,
    /// The row that decided the carried output, as `(table, row)`.
    pub row: Option<(String, usize)>,
    /// Every row that fired.
    pub rows: Vec<(String, usize)>,
    /// A call that makes it: every input, the carried one included.
    pub inputs: BTreeMap<String, Val>,
    /// What that call answers, in the order the outputs are declared.
    pub outputs: Vec<(String, Option<Val>)>,
    /// The coordinates of the `held` inputs this call is made with, one per held input
    /// (§15.149). A case holds them from its first call to its last, so it only ever takes
    /// edges of one world.
    pub world: Vec<usize>,
    /// The coordinates of the further carried values the call is made at, and of the ones it
    /// answers, one per further `carry` line (§15.217). Empty for a machine that carries only
    /// its state.
    pub from_carry: Vec<usize>,
    pub to_carry: Vec<usize>,
    /// Whether no call was seen to make this edge: it is one of the coordinates a carried
    /// number's next value can fall in over its cell, past the ones the cell's points answered
    /// (§15.217). Followed where nothing bad is to be reached; never where a call has to really
    /// happen.
    pub approx: bool,
    /// The cell of the walk the call was found in.
    pub cell: Vec<usize>,
}

/// Where the searches walk: a state, the world a case is in (§15.149), and the coordinates of
/// the further carried values (§15.217; empty when there are none).
pub type Node = (String, Vec<usize>, Vec<usize>);

/// How a claim came out.
#[derive(Debug, Clone)]
pub enum Verdict {
    Holds,
    /// Broken, by this sequence of calls from the initial state.
    Broken(Vec<Edge>),
    /// Not settled either way, and why.
    Undecided(String),
}

impl Verdict {
    pub fn holds(&self) -> bool {
        matches!(self, Verdict::Holds)
    }
}

/// One value a machine carries besides its state (§15.217).
#[derive(Debug, Clone)]
pub struct Carried {
    pub input: String,
    pub output: String,
    pub ty: Ty,
    /// Where a case starts.
    pub initial: Val,
    /// The axis its input is cut on: one coordinate per value when the machine is walked whole,
    /// intervals otherwise.
    pub axis: Axis,
    /// The index of that axis among the walk's.
    pub at: usize,
    /// The coordinate of the initial value.
    pub start: usize,
    /// Both ends of the input's range, for a number.
    pub range: Option<(Rat, Rat)>,
}

impl Carried {
    pub fn number(&self) -> bool {
        self.range.is_some()
    }
}

/// The calls that really happen, from where a case starts (§15.217). When the carried numbers
/// are walked whole these are the walk's own calls; otherwise each was made from a state a case
/// was found in, with that state's values, and answered by the rule.
#[derive(Debug, Clone, Default)]
pub struct Real {
    /// What a case is in: the state, the further carried values in `carry` order, the world.
    pub nodes: Vec<(String, Vec<Val>, Vec<usize>)>,
    /// (from, the call, to). A call that answers a carried number outside its input's range goes
    /// to a place nothing leads out of: the next call cannot be passed the value.
    pub calls: Vec<(usize, Edge, Option<usize>)>,
    /// Where a case starts, one node per world.
    pub starts: Vec<usize>,
    /// Whether everything met was followed, rather than the search stopping at its cap.
    pub complete: bool,
}

impl Real {
    /// Breadth-first from where a case starts: the nodes in the order met, and how each was
    /// first reached (the node before and the call).
    pub fn reach(&self) -> (Vec<usize>, HashMap<usize, (usize, usize)>) {
        let mut out: Vec<Vec<usize>> = vec![Vec::new(); self.nodes.len()];
        for (i, (from, _, to)) in self.calls.iter().enumerate() {
            if to.is_some() {
                out[*from].push(i);
            }
        }
        bfs_from(self.starts.clone(), |n: &usize| out[*n].iter().filter_map(|&i| self.calls[i].2.map(|t| (i, t))).collect())
    }

    /// The calls from a start to `n`, each one really made.
    pub fn path(&self, parent: &HashMap<usize, (usize, usize)>, n: usize) -> Vec<Edge> {
        path_to(parent, &n).into_iter().map(|i| self.calls[i].1.clone()).collect()
    }
}

/// Everything the walk found.
#[derive(Debug, Clone)]
pub struct Analysis {
    pub name: String,
    pub alias: Option<String>,
    pub over: String,
    /// The carried input and output.
    pub carry: (String, String),
    pub state_enum: String,
    /// The values of the state enum, in declaration order.
    pub states: Vec<String>,
    pub initial: String,
    pub finals: Vec<String>,
    pub edges: Vec<Edge>,
    /// (state, the further carried values' coordinates) → how many cells of calls from there
    /// could not be settled.
    pub unknown: BTreeMap<(String, Vec<usize>), usize>,
    /// How many cells the space has, and whether that was more than the budget allows.
    pub cells: u128,
    pub over_budget: bool,
    /// Set when the space does not cut into cells at all.
    pub blocked: Option<String>,
    /// The states a case can reach, in the order a breadth-first search meets them.
    pub reachable: Vec<String>,
    /// State → the edge that first reached it.
    pub parent: HashMap<String, usize>,
    /// Whether the reachable set is exact: no state in it has a cell left unsettled.
    pub exact: bool,
    /// Per final state: whether nothing leads out of it.
    pub final_claims: Vec<(String, Verdict)>,
    /// Per reachable state that is not final: whether a final state can still be reached.
    pub finish_claims: Vec<(String, Verdict)>,
    pub never_claims: Vec<Verdict>,
    pub once_claims: Vec<Verdict>,
    /// Rows of the tables that decide the carried output that no call from a reachable state
    /// takes. Only when the reachable set is exact.
    pub unused: Vec<(String, usize)>,
    /// For each `never` line: the pairs (state, has been in the `after` states) a case can be
    /// in. Kept for the certificate, which re-checks that the set is closed.
    pub never_sets: Vec<Vec<(String, bool)>>,
    /// For each `once` line: the pairs (state, how many calls counted, at most 2).
    pub once_sets: Vec<Vec<(String, u8)>>,
    /// The `held` inputs (§15.149), and their axes: what tells the worlds apart.
    pub held: Vec<String>,
    pub held_axes: Vec<Axis>,
    /// The held inputs that a computed column of the walk or a `constraint` also reads.
    /// There one value of the input may allow calls another value of the same class does not,
    /// so whether a case can still finish is not followed.
    pub coupled: Vec<String>,
    /// Edges whose call could not be made with the value its world's other calls are made
    /// with. A sequence through one is not shown to be one case.
    pub mixed: BTreeSet<usize>,
    /// The (state, world, carried coordinates) a case can reach, in the order a breadth-first
    /// search meets them, and how each was first reached.
    pub nodes: Vec<Node>,
    pub node_parent: HashMap<Node, (Node, usize)>,
    /// The values carried besides the state (§15.217), in the order of their `carry` lines.
    pub more: Vec<Carried>,
    /// Whether every carried number was walked one value at a time, so that a node is one
    /// place a case can really be. Otherwise the numbers were cut into intervals, and the
    /// searches over them say more than happens.
    pub whole: bool,
    /// Per `always` line.
    pub always_claims: Vec<Verdict>,
    /// Per carried number: whether its next value stays within its input's range (E134).
    pub range_claims: Vec<(String, Verdict)>,
    /// Why whether a case can still finish was not followed, when it was not: the numbers were
    /// walked in intervals, where a path of calls is not shown to be one a case can make.
    pub finish_open: Option<String>,
    /// The calls that really happen, from where a case starts (§15.217). Empty for a machine
    /// that carries only its state.
    pub real: Real,
}

impl Analysis {
    /// The shortest sequence of calls from the initial state to `s`, in whichever world meets
    /// it first, when there is one.
    pub fn trace_to(&self, s: &str) -> Option<Vec<usize>> {
        if s == self.initial {
            return Some(Vec::new());
        }
        let n = self.nodes.iter().find(|n| n.0 == s)?;
        self.trace_to_node(n)
    }

    /// The shortest sequence of calls from the initial state to the state of `n`, staying in
    /// its world: every call is one the same case can make.
    pub fn trace_to_node(&self, n: &Node) -> Option<Vec<usize>> {
        if !self.nodes.contains(n) {
            return None;
        }
        let mut out = Vec::new();
        let mut at = n.clone();
        let mut guard = 0;
        while let Some((prev, e)) = self.node_parent.get(&at) {
            out.push(*e);
            at = prev.clone();
            guard += 1;
            if guard > self.nodes.len() + 1 {
                return None;
            }
        }
        out.reverse();
        Some(out)
    }

    /// Whether a case can be in state `s` in world `w`.
    pub fn reaches(&self, s: &str, w: &[usize]) -> bool {
        self.nodes.iter().any(|(x, y, _)| x == s && y.as_slice() == w)
    }

    /// The world a call with these inputs is in: the coordinate of each held input. `None`
    /// when a value lies on no coordinate (outside the declared range).
    pub fn world_of<'v>(&self, get: impl Fn(&str) -> Option<&'v Val>) -> Option<Vec<usize>> {
        self.held_axes
            .iter()
            .map(|ax| {
                let v = get(&ax.col)?;
                (0..ax.coords.len()).find(|&ci| crate::vdiff::holds(ax, ci, v))
            })
            .collect()
    }

    /// The coordinates the further carried values of a call lie on (§15.217). `None` when one
    /// is missing or lies on none (outside its range).
    pub fn carry_of<'v>(&self, get: impl Fn(&str) -> Option<&'v Val>) -> Option<Vec<usize>> {
        self.more.iter().map(|it| coord_of(&it.axis, get(&it.input)?)).collect()
    }

    /// The (state, world, carried coordinates) from which a final state can still be reached,
    /// by calls that were seen to happen.
    pub fn can_finish(&self) -> HashSet<Node> {
        let mut can: HashSet<Node> = HashSet::new();
        if self.more.is_empty() {
            let worlds: BTreeSet<&Vec<usize>> = self.edges.iter().map(|e| &e.world).chain(self.nodes.iter().map(|n| &n.1)).collect();
            for fs in &self.finals {
                for w in &worlds {
                    can.insert((fs.clone(), (*w).clone(), Vec::new()));
                }
            }
        } else {
            for e in self.edges.iter().filter(|e| !e.approx) {
                for (s, cc) in [(&e.from, &e.from_carry), (&e.to, &e.to_carry)] {
                    if self.finals.contains(s) {
                        can.insert((s.clone(), e.world.clone(), cc.clone()));
                    }
                }
            }
            for n in &self.nodes {
                if self.finals.contains(&n.0) {
                    can.insert(n.clone());
                }
            }
        }
        loop {
            let before = can.len();
            for e in self.edges.iter().filter(|e| !e.approx) {
                if can.contains(&(e.to.clone(), e.world.clone(), e.to_carry.clone())) {
                    can.insert((e.from.clone(), e.world.clone(), e.from_carry.clone()));
                }
            }
            if can.len() == before {
                break;
            }
        }
        can
    }

    /// Whether a case in state `s`, in world `w`, holding these further carried values, can
    /// still reach a final state (§15.217): the values read onto their coordinates.
    pub fn finishes_from(&self, can: &HashSet<Node>, s: &str, w: &[usize], vals: &[Val]) -> Option<bool> {
        let cc: Vec<usize> = self.more.iter().zip(vals).map(|(it, v)| coord_of(&it.axis, v)).collect::<Option<Vec<usize>>>()?;
        Some(can.contains(&(s.to_string(), w.to_vec(), cc)))
    }

    /// Whether a case in state `s`, in world `w`, holding these further carried values, can no
    /// longer get to a final state — said only where it is shown. For a machine that carries only
    /// its state that is: not among `can`. With further carried values (§15.217), only a machine
    /// walked whole can say it, and then only when nothing the walk found from there, nor a cell
    /// it left unsettled, leads to a final state.
    pub fn stuck_at(&self, can: &HashSet<Node>, s: &str, w: &[usize], vals: &[Val]) -> bool {
        if self.more.is_empty() {
            return !can.contains(&(s.to_string(), w.to_vec(), Vec::new()));
        }
        if !self.whole || self.finals.is_empty() {
            return false;
        }
        let Some(cc) = self.more.iter().zip(vals).map(|(it, v)| coord_of(&it.axis, v)).collect::<Option<Vec<usize>>>() else { return false };
        let start: Node = (s.to_string(), w.to_vec(), cc);
        let (closure, _) = bfs_from(vec![start], |n: &Node| {
            self.edges
                .iter()
                .enumerate()
                .filter(|(_, e)| e.from == n.0 && e.world == n.1 && e.from_carry == n.2)
                .map(|(i, e)| (i, (e.to.clone(), n.1.clone(), e.to_carry.clone())))
                .collect()
        });
        !closure.iter().any(|n| self.finals.contains(&n.0) || self.unknown.get(&(n.0.clone(), n.2.clone())).copied().unwrap_or(0) > 0)
    }

    /// Whether a call from state `s` was left unsettled somewhere.
    pub fn unsettled_at(&self, s: &str) -> bool {
        self.unknown.iter().any(|((x, _), n)| x == s && *n > 0)
    }

    /// Edges out of one state.
    pub fn out_of<'b>(&'b self, s: &'b str) -> impl Iterator<Item = (usize, &'b Edge)> + 'b {
        self.edges.iter().enumerate().filter(move |(_, e)| e.from == s && !e.approx)
    }

    /// The transitions a reader sees: one per (from, row, to), the flags merged.
    pub fn transitions(&self) -> Vec<(String, Option<(String, usize)>, String)> {
        let mut out: Vec<(String, Option<(String, usize)>, String)> = Vec::new();
        for e in self.edges.iter().filter(|e| !e.approx) {
            let k = (e.from.clone(), e.row.clone(), e.to.clone());
            if !out.contains(&k) {
                out.push(k);
            }
        }
        out
    }

    /// The values a node's further carried coordinates stand for, shown: `16` for a point,
    /// `2..15` for an interval (§15.217).
    pub fn carry_text(&self, cc: &[usize]) -> Vec<String> {
        self.more.iter().zip(cc).map(|(it, &ci)| coord_text(&it.axis, ci)).collect()
    }

    /// The values the `k`th further carried value takes over some coordinates of its axis, as a
    /// person reads them: neighbouring coordinates run together, `1..16`, and a boolean's or an
    /// enum's values one by one.
    pub fn values_text(&self, k: usize, cs: &BTreeSet<usize>) -> String {
        let ax = &self.more[k].axis;
        if !self.more[k].number() {
            return cs.iter().map(|&ci| coord_text(ax, ci)).collect::<Vec<_>>().join(" / ");
        }
        let mut runs: Vec<(usize, usize)> = Vec::new();
        for &ci in cs {
            match runs.last_mut() {
                Some((_, hi)) if *hi + 1 == ci => *hi = ci,
                _ => runs.push((ci, ci)),
            }
        }
        runs.iter()
            .map(|&(lo, hi)| {
                if lo == hi {
                    coord_text(ax, lo)
                } else {
                    let from = coord_text(ax, lo);
                    let to = coord_text(ax, hi);
                    let from = from.split("..").next().unwrap_or_default().to_string();
                    let to = to.rsplit("..").next().unwrap_or_default().to_string();
                    format!("{from}..{to}")
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// The coordinate a value lies on.
pub fn coord_of(ax: &Axis, v: &Val) -> Option<usize> {
    (0..ax.coords.len()).find(|&ci| crate::vdiff::holds(ax, ci, v))
}

/// The coordinate a further carried value lies on, `OUT` for a number outside its input's range
/// (§15.217).
pub fn place_of(it: &Carried, v: &Val) -> Option<usize> {
    match coord_of(&it.axis, v) {
        Some(ci) => Some(ci),
        None if it.number() && matches!(v, Val::Num(_)) => Some(OUT),
        None => None,
    }
}

/// One coordinate of a carried value, as a person reads it: the value, or the interval's two
/// ends joined by `..`.
pub fn coord_text(ax: &Axis, ci: usize) -> String {
    if ci == OUT {
        return tr!("範囲の外", "outside the range");
    }
    let num = |r: Rat| {
        let v = r.mul(Rat::int(ax.shown));
        let body = if v.den == 1 { v.num.to_string() } else { format!("{}", v.num as f64 / v.den as f64) };
        format!("{body}{}", ax.unit)
    };
    match &ax.coords[ci] {
        Coord::Word(w) => w.clone(),
        Coord::OtherStr => "…".into(),
        Coord::Point(v) => num(*v),
        co => {
            let (lo, hi) = crate::vdiff::ival(co, ax.step);
            format!("{}..{}", lo.map(num).unwrap_or_default(), hi.map(num).unwrap_or_default())
        }
    }
}

/// The numeric boundaries a cell names, in the unit of `ty`.
fn cell_bounds(cell: &Cell, ty: &Ty) -> Vec<Rat> {
    let ty = match ty {
        Ty::Opt(t) => t.as_ref(),
        t => t,
    };
    let lit = |l: &Lit| -> Option<Rat> {
        match l {
            Lit::Num(n) => crate::types::lit_value_in_pub(n, ty),
            Lit::Date(y, m, d) if *ty == Ty::Date => Some(crate::types::date_ord(*y, *m, *d)),
            _ => None,
        }
    };
    match cell {
        Cell::Cmp(ops) => ops.iter().filter_map(|(_, l)| lit(l)).collect(),
        Cell::Lit(l) => lit(l).into_iter().collect(),
        Cell::Set(ls) | Cell::Not(ls) => ls.iter().filter_map(lit).collect(),
        _ => Vec::new(),
    }
}

/// The names an output can take its value from: the inputs, derived values and definitions a
/// row writes into it, followed through the tables in between. A `once` line tests the
/// output, so each of these is cut at the line's boundaries — which is what makes one input
/// stand for its whole cell, as a table's own cell makes it for the column it tests.
fn sources(f: &RuleFile, name: &str) -> Vec<String> {
    let inputs: BTreeSet<&str> = f.inputs.iter().map(|i| i.name.text.as_str()).collect();
    let computed = |n: &str| {
        f.items.iter().any(|it| match it {
            Item::Derived(d) => d.name.text == n,
            Item::Define(d) => d.name.text == n,
            _ => false,
        })
    };
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    let mut stack = vec![name.to_string()];
    while let Some(n) = stack.pop() {
        if !seen.insert(n.clone()) {
            continue;
        }
        if inputs.contains(n.as_str()) || computed(&n) {
            out.push(n);
            continue;
        }
        for it in &f.items {
            let Item::Table(t) = it else { continue };
            let Some(ci) = t.outputs.iter().position(|o| o.name.text == n) else { continue };
            for r in &t.rows {
                if let Some(OutCell::Name(m)) = r.outs.get(ci) {
                    stack.push(m.clone());
                }
            }
        }
    }
    out
}

/// The boundaries every `once` line adds, and the computed columns it makes tested.
fn once_cuts(f: &RuleFile, c: &Checked, m: &MachineDecl) -> (Vec<(String, Rat)>, BTreeSet<String>) {
    let mut extra = Vec::new();
    let mut more = BTreeSet::new();
    let inputs: BTreeSet<&str> = f.inputs.iter().map(|i| i.name.text.as_str()).collect();
    for on in &m.onces {
        let Some(oty) = c.ty_of(&on.output.text) else { continue };
        let bounds = cell_bounds(&on.cell, &oty);
        if bounds.is_empty() {
            continue;
        }
        let mut srcs = sources(f, &on.output.text);
        // An output that is a `define` of its own name is its own source.
        if f.items.iter().any(|it| matches!(it, Item::Define(d) if d.name.text == on.output.text)) && !srcs.contains(&on.output.text) {
            srcs.push(on.output.text.clone());
        }
        for s in srcs {
            if !inputs.contains(s.as_str()) {
                more.insert(s.clone());
            }
            for b in &bounds {
                extra.push((s.clone(), *b));
            }
        }
    }
    (extra, more)
}

/// Whether the answer to `on` is one flag over the whole cell: the output is one value there,
/// or it is read off a column whose coordinate lies wholly on one side of every boundary the
/// cell names.
fn flag_settled(f: &RuleFile, c: &Checked, axes: &[Axis], cell: &[usize], rows: &[(String, usize)], on: &OnceDecl) -> bool {
    if crate::vdiff::names_constant(f, c, axes, cell, rows, std::slice::from_ref(&on.output.text)) {
        return true;
    }
    let fired: HashMap<&str, usize> = rows.iter().map(|(t, r)| (t.as_str(), *r)).collect();
    // Follow the output to the column it is read off in this cell.
    let mut n = on.output.text.clone();
    for _ in 0..16 {
        if let Some(ai) = axes.iter().position(|a| a.col == n) {
            let Some(ty) = c.ty_of(&n) else { return false };
            let bounds = cell_bounds(&on.cell, &ty);
            return match &axes[ai].coords[cell[ai]] {
                Coord::Open(lo, hi) => !bounds.iter().any(|b| {
                    lo.is_none_or(|l| b.cmp_to(l) == std::cmp::Ordering::Greater) && hi.is_none_or(|h| b.cmp_to(h) == std::cmp::Ordering::Less)
                }),
                _ => true,
            };
        }
        let mut next = None;
        for set in &c.sets {
            let Some(ci) = set.table.outputs.iter().position(|o| o.name.text == n) else { continue };
            for (i, r) in set.table.rows.iter().enumerate() {
                if fired.get(set.row_table(i)) == Some(&r.index) {
                    next = match r.outs.get(ci) {
                        Some(OutCell::Name(m)) => Some(m.clone()),
                        _ => None,
                    };
                }
            }
        }
        match next {
            Some(m) => n = m,
            None => return false,
        }
    }
    false
}

/// The held inputs a computed column of the walk reads, followed through definitions and
/// tables, and the ones a `constraint` names (§15.149). A column computed from a held input
/// and from inputs that change cuts the space so that one value of the held input allows
/// calls another value of its class does not; the walk tells the worlds apart by the held
/// input's own coordinates, so there it does not follow which calls one value allows.
fn coupled_inputs(f: &RuleFile, fixed: &[String], axes: &[Axis]) -> Vec<String> {
    fn names(e: &Expr, out: &mut BTreeSet<String>) {
        match e {
            Expr::Name(n, _) => {
                out.insert(n.clone());
            }
            Expr::Lit(..) => {}
            Expr::Bin(l, _, r, _) => {
                names(l, out);
                names(r, out);
            }
            Expr::Call(_, args, _) => args.iter().for_each(|a| names(a, out)),
        }
    }
    // What each computed name reads directly.
    let mut reads: HashMap<String, BTreeSet<String>> = HashMap::new();
    for it in &f.items {
        match it {
            Item::Derived(d) => names(&d.expr, reads.entry(d.name.text.clone()).or_default()),
            Item::Define(d) => names(&d.expr, reads.entry(d.name.text.clone()).or_default()),
            Item::Table(t) => {
                for (ci, oc) in t.outputs.iter().enumerate() {
                    let r = reads.entry(oc.name.text.clone()).or_default();
                    r.extend(t.inputs.iter().map(|(n, _)| n.clone()));
                    for row in &t.rows {
                        if let Some(OutCell::Name(n)) = row.outs.get(ci) {
                            r.insert(n.clone());
                        }
                    }
                }
            }
            Item::Agg(_) | Item::Fold(_) => {}
        }
    }
    let depends = |start: &str| -> BTreeSet<String> {
        let mut seen = BTreeSet::new();
        let mut stack = vec![start.to_string()];
        while let Some(n) = stack.pop() {
            if !seen.insert(n.clone()) {
                continue;
            }
            if let Some(r) = reads.get(&n) {
                stack.extend(r.iter().cloned());
            }
        }
        seen
    };
    let mut out: Vec<String> = Vec::new();
    for ax in axes.iter().filter(|ax| ax.kind == crate::vdiff::Kind::Derived) {
        let d = depends(&ax.col);
        for x in fixed {
            if d.contains(x) && !out.contains(x) {
                out.push(x.clone());
            }
        }
    }
    for k in &f.constraints {
        for x in fixed {
            if k.names().any(|n| n == x) && !out.contains(x) {
                out.push(x.clone());
            }
        }
    }
    out
}

/// The value of a named output in an answer.
fn out_val<'b>(outs: &'b [(String, Option<Val>)], name: &str) -> Option<&'b Val> {
    outs.iter().find(|(k, _)| k == name).and_then(|(_, v)| v.as_ref())
}

/// The further carried values (§15.217), with where each starts, in the order of their `carry`
/// lines: (input, output, the value a case starts at). A line whose pair or start did not check
/// is left out; the type check has said why (E051, E072).
pub fn more_starts(f: &RuleFile, c: &Checked) -> Vec<(String, String, Val)> {
    let Some(m) = &f.machine else { return Vec::new() };
    let mut out = Vec::new();
    for (i, o) in m.more() {
        let Some(ty) = c.ty_of(i) else { continue };
        let Some((_, lit, _)) = m.initials.iter().find(|(n, _, _)| n.text == i) else { continue };
        let Some(v) = crate::eval::lit_to_val(lit, &ty) else { continue };
        out.push((i.to_string(), o.to_string(), v));
    }
    out
}

/// Whether every further `carry` line checked, with a start that fits: only then is there a
/// sequence of calls to speak of.
fn more_complete(f: &RuleFile, c: &Checked, m: &MachineDecl) -> Option<Vec<(String, String, Val, Ty, Option<(Rat, Rat)>)>> {
    let starts = more_starts(f, c);
    if starts.len() != m.carries.len() {
        return None;
    }
    let mut out = Vec::new();
    for (i, o, v) in starts {
        let (ti, to) = (c.ty_of(&i)?, c.ty_of(&o)?);
        if ti != to {
            return None;
        }
        let range = match &ti {
            Ty::Enum(en) => {
                let Val::Enum(w) = &v else { return None };
                if !c.enums.get(en)?.contains(w) {
                    return None;
                }
                None
            }
            Ty::Bool => {
                if !matches!(v, Val::Bool(_)) {
                    return None;
                }
                None
            }
            Ty::Number | Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate => {
                let (Some(lo), Some(hi)) = c.ranges.get(&i).cloned()? else { return None };
                let Val::Num(r) = &v else { return None };
                if r.cmp_to(lo) == std::cmp::Ordering::Less || r.cmp_to(hi) == std::cmp::Ordering::Greater {
                    return None;
                }
                Some((lo, hi))
            }
            _ => return None,
        };
        out.push((i, o, v, ti, range));
    }
    Some(out)
}

/// Where a carried number is cut when it is walked in intervals (§15.217), on top of where the
/// rule's own cells cut it: at every value a row writes into its output, at every boundary an
/// `always` line names for it, and at the value a case starts with — so the start is a point,
/// and an interval is either wholly inside an `always` cell or wholly outside it.
fn interval_cuts(c: &Checked, m: &MachineDecl, items: &[(String, String, Val, Ty, Option<(Rat, Rat)>)]) -> Vec<(String, Rat)> {
    let mut out = Vec::new();
    for (i, o, v, ty, range) in items {
        if range.is_none() {
            continue;
        }
        for set in &c.sets {
            let Some(ci) = set.table.outputs.iter().position(|oc| oc.name.text == *o) else { continue };
            for r in &set.table.rows {
                if let Some(OutCell::Lit(Lit::Num(n))) = r.outs.get(ci) {
                    if let Some(x) = crate::types::lit_value_in_pub(n, ty) {
                        out.push((i.clone(), x));
                    }
                }
            }
        }
        for al in &m.alwayses {
            for (n, cell, _) in &al.tests {
                if n.text == *i {
                    for b in cell_bounds(cell, ty) {
                        out.push((i.clone(), b));
                    }
                }
            }
        }
        if let Val::Num(x) = v {
            out.push((i.clone(), *x));
        }
    }
    out
}

/// The values of a carried number on the grid of its axis, from one end of its range to the
/// other, when there are no more than `cap` of them.
fn grid_points(range: (Rat, Rat), step: Rat, cap: u128) -> Option<Vec<Rat>> {
    let (lo, hi) = range;
    let first = lo.checked_div(step)?;
    let k0 = first.num.div_euclid(first.den) + i128::from(first.num.rem_euclid(first.den) != 0);
    let last = hi.checked_div(step)?;
    let k1 = last.num.div_euclid(last.den);
    if k1 < k0 {
        return None;
    }
    let n = u128::try_from(k1 - k0 + 1).ok()?;
    if n > cap {
        return None;
    }
    Some((k0..=k1).map(|k| step.mul(Rat::int(k))).collect())
}

/// A literal number read in `hint`'s unit, or in its own when it has another.
fn lit_num(n: &crate::lex::Num, hint: &Ty) -> Option<Rat> {
    crate::types::lit_value_in_pub(n, hint).or_else(|| crate::types::lit_value_in_pub(n, &crate::types::lit_ty_pub(n)))
}

/// The interval a named value lies in over one cell, given the rows that fired there
/// (§15.217): an input's or a tested column's coordinate, a computed value's expression in
/// interval arithmetic, a table's column through the row that wrote it. The cell is cut at
/// every boundary any table names, so the rows that fired are the same over all of it, as
/// `names_constant` reads them. None when it cannot be bounded.
#[allow(clippy::too_many_arguments)]
fn image_name(f: &RuleFile, c: &Checked, axes: &[Axis], cell: &[usize], fired: &HashMap<&str, usize>, n: &str, hint: &Ty, depth: usize) -> Option<(Rat, Rat)> {
    if depth > 64 {
        return None;
    }
    let from_axis = axes.iter().position(|a| a.col == n).and_then(|ai| {
        let (lo, hi) = crate::vdiff::ival(&axes[ai].coords[cell[ai]], axes[ai].step);
        Some((lo?, hi?))
    });
    if f.inputs.iter().any(|i| i.name.text == n) {
        return from_axis;
    }
    let computed = f
        .items
        .iter()
        .find_map(|it| match it {
            Item::Derived(d) if d.name.text == n => Some(&d.expr),
            Item::Define(d) if d.name.text == n => Some(&d.expr),
            _ => None,
        })
        .or_else(|| match &f.result {
            Some(r) if r.name == n => Some(&r.expr),
            _ => None,
        });
    let own = c.ty_of(n).unwrap_or_else(|| hint.clone());
    let got = match computed {
        Some(e) => image_expr(f, c, axes, cell, fired, e, &own, depth + 1),
        None => {
            let mut out = None;
            'sets: for set in &c.sets {
                let Some(ci) = set.table.outputs.iter().position(|o| o.name.text == n) else { continue };
                for (i, r) in set.table.rows.iter().enumerate() {
                    if fired.get(set.row_table(i)) == Some(&r.index) {
                        out = match r.outs.get(ci) {
                            Some(OutCell::Lit(Lit::Num(num))) => lit_num(num, &own).map(|v| (v, v)),
                            Some(OutCell::Name(m)) => image_name(f, c, axes, cell, fired, m, &own, depth + 1),
                            _ => None,
                        };
                        break 'sets;
                    }
                }
            }
            out
        }
    };
    // A computed column a table tests has a coordinate of its own, and the value lies in both.
    match (got, from_axis) {
        (Some((a, b)), Some((x, y))) => {
            let lo = if a.cmp_to(x) == std::cmp::Ordering::Less { x } else { a };
            let hi = if b.cmp_to(y) == std::cmp::Ordering::Greater { y } else { b };
            if lo.cmp_to(hi) == std::cmp::Ordering::Greater { Some((x, y)) } else { Some((lo, hi)) }
        }
        (Some(g), None) => Some(g),
        (None, ax) => ax,
    }
}

#[allow(clippy::too_many_arguments)]
fn image_expr(f: &RuleFile, c: &Checked, axes: &[Axis], cell: &[usize], fired: &HashMap<&str, usize>, e: &Expr, hint: &Ty, depth: usize) -> Option<(Rat, Rat)> {
    if depth > 64 {
        return None;
    }
    let sub = |x: &Expr| image_expr(f, c, axes, cell, fired, x, hint, depth + 1);
    match e {
        Expr::Lit(Lit::Num(n), _) => {
            let v = lit_num(n, hint)?;
            Some((v, v))
        }
        Expr::Lit(..) => None,
        Expr::Name(n, _) => image_name(f, c, axes, cell, fired, n, hint, depth + 1),
        Expr::Bin(l, op, r, _) => {
            let (al, ah) = sub(l)?;
            let (bl, bh) = sub(r)?;
            match op {
                BinOp::Add => Some((al.checked_add(bl)?, ah.checked_add(bh)?)),
                BinOp::Sub => Some((al.checked_sub(bh)?, ah.checked_sub(bl)?)),
                BinOp::Mul | BinOp::Div => {
                    if *op == BinOp::Div && bl.cmp_to(Rat::zero()) != std::cmp::Ordering::Greater && bh.cmp_to(Rat::zero()) != std::cmp::Ordering::Less {
                        return None;
                    }
                    let g = |x: Rat, y: Rat| if *op == BinOp::Mul { x.checked_mul(y) } else { x.checked_div(y) };
                    let mut vs = [g(al, bl)?, g(al, bh)?, g(ah, bl)?, g(ah, bh)?];
                    vs.sort_by(|a, b| a.cmp_to(*b));
                    Some((vs[0], vs[3]))
                }
                _ => None,
            }
        }
        Expr::Call(name, args, _) => match (name.as_str(), args.as_slice()) {
            // Every rounding mode keeps the order of what it rounds.
            (m, [x, g]) if RoundMode::parse(m).is_some() => {
                let (lo, hi) = sub(x)?;
                let Expr::Lit(Lit::Num(n), _) = g else { return None };
                let grid = lit_num(n, hint)?;
                let mode = RoundMode::parse(m)?;
                Some((lo.round_to(mode, grid), hi.round_to(mode, grid)))
            }
            (crate::kw::MIN, [x, y]) | (crate::kw::MAX, [x, y]) => {
                let (al, ah) = sub(x)?;
                let (bl, bh) = sub(y)?;
                let pick = |a: Rat, b: Rat| {
                    let less = a.cmp_to(b) == std::cmp::Ordering::Less;
                    if (name == crate::kw::MIN) == less { a } else { b }
                };
                Some((pick(al, bl), pick(ah, bh)))
            }
            _ => None,
        },
    }
}

/// What a carried number's output can come to over a cell: its image, rounded as the output
/// declares, since the rounded value is what the next call is passed.
fn image_out(f: &RuleFile, c: &Checked, axes: &[Axis], cell: &[usize], rows: &[(String, usize)], out: &str, ty: &Ty) -> Option<(Rat, Rat)> {
    let fired: HashMap<&str, usize> = rows.iter().map(|(t, r)| (t.as_str(), *r)).collect();
    let (lo, hi) = image_name(f, c, axes, cell, &fired, out, ty, 0)?;
    let od = f.outputs.iter().find(|o| o.name.text == out)?;
    match &od.rounding {
        Some(rd) => {
            let grid = crate::types::lit_value_in_pub(&rd.grid, ty)?;
            let mode = RoundMode::parse(&rd.mode)?;
            Some((lo.round_to(mode, grid), hi.round_to(mode, grid)))
        }
        None => Some((lo, hi)),
    }
}

/// The coordinates of an axis an interval meets.
fn coords_meeting(ax: &Axis, lo: Rat, hi: Rat) -> Vec<usize> {
    (0..ax.coords.len())
        .filter(|&ci| {
            let (a, b) = crate::vdiff::ival(&ax.coords[ci], ax.step);
            a.is_none_or(|a| a.cmp_to(hi) != std::cmp::Ordering::Greater) && b.is_none_or(|b| b.cmp_to(lo) != std::cmp::Ordering::Less)
        })
        .collect()
}

/// Every way of taking one coordinate from each list.
fn combos(lists: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut out: Vec<Vec<usize>> = vec![Vec::new()];
    for l in lists {
        let mut next = Vec::new();
        for base in &out {
            for &x in l {
                let mut b = base.clone();
                b.push(x);
                next.push(b);
            }
        }
        out = next;
    }
    out
}

/// Walk the rule's space and settle the machine's claims. `None` when the rule has no
/// machine, or its declarations did not check (the type check has said why).
pub fn analyze(f: &RuleFile, c: &Checked, budget: usize) -> Option<Analysis> {
    let m = f.machine.as_ref()?;
    let (cin, cout) = m.carried()?;
    let state_enum = match (c.ty_of(cin), c.ty_of(cout)) {
        (Some(Ty::Enum(a)), Some(Ty::Enum(b))) if a == b => a,
        _ => return None,
    };
    let states = c.enums.get(&state_enum)?.clone();
    let initial = m.initial.as_ref()?.0.text.clone();
    if !states.contains(&initial) {
        return None;
    }
    let items = more_complete(f, c, m)?;
    let finals: Vec<String> = m.finals.iter().map(|n| n.text.clone()).collect();
    let deciders: BTreeSet<String> = f
        .items
        .iter()
        .filter_map(|it| match it {
            Item::Table(t) if t.outputs.iter().any(|o| o.name.text == cout) => t.name.as_ref().map(|n| n.text.clone()),
            _ => None,
        })
        .collect();
    let mut a = Analysis {
        name: m.name.text.clone(),
        alias: m.name.ascii.clone(),
        over: m.over.as_ref().map(|n| n.text.clone()).unwrap_or_default(),
        carry: (cin.to_string(), cout.to_string()),
        state_enum,
        states: states.clone(),
        initial: initial.clone(),
        finals: finals.clone(),
        edges: Vec::new(),
        unknown: BTreeMap::new(),
        cells: 0,
        over_budget: false,
        blocked: None,
        reachable: Vec::new(),
        parent: HashMap::new(),
        exact: true,
        final_claims: Vec::new(),
        finish_claims: Vec::new(),
        never_claims: Vec::new(),
        once_claims: Vec::new(),
        unused: Vec::new(),
        never_sets: Vec::new(),
        once_sets: Vec::new(),
        held: Vec::new(),
        held_axes: Vec::new(),
        coupled: Vec::new(),
        mixed: BTreeSet::new(),
        nodes: Vec::new(),
        node_parent: HashMap::new(),
        more: Vec::new(),
        whole: true,
        always_claims: Vec::new(),
        range_claims: Vec::new(),
        finish_open: None,
        real: Real::default(),
    };
    let (mut extra, more) = once_cuts(f, c, m);
    let cap = (budget / NODES_PER_CELL).max(1) as u128;
    let mut walker = if items.is_empty() {
        match Walker::new(f, c, &extra, &more) {
            Ok(w) => w,
            Err(why) => {
                a.blocked = Some(why);
                return Some(a);
            }
        }
    } else {
        // The further carried values (§15.217). A number is cut where it matters first; when
        // the space with every one of its values a coordinate of its own still fits the budget,
        // it is walked that way instead, and every node is one place a case can really be.
        extra.extend(interval_cuts(c, m, &items));
        let cut = match Walker::new(f, c, &extra, &more) {
            Ok(w) => w,
            Err(why) => {
                a.blocked = Some(why);
                return Some(a);
            }
        };
        let mut size: Option<u128> = Some(cut.size());
        let mut every: Vec<(String, Rat)> = Vec::new();
        for (i, _, _, _, range) in &items {
            let Some(range) = range else { continue };
            let Some(ax) = cut.axes.iter().find(|ax| ax.col == *i) else {
                size = None;
                break;
            };
            match grid_points(*range, ax.step, cap) {
                Some(pts) => {
                    size = size.and_then(|s| (s / ax.len().max(1) as u128).checked_mul(pts.len() as u128));
                    every.extend(pts.into_iter().map(|p| (i.clone(), p)));
                }
                None => size = None,
            }
        }
        match size {
            Some(s) if s <= cap => {
                let mut all = extra.clone();
                all.extend(every);
                match Walker::new(f, c, &all, &more) {
                    Ok(w) => w,
                    Err(why) => {
                        a.blocked = Some(why);
                        return Some(a);
                    }
                }
            }
            _ => {
                a.whole = false;
                cut
            }
        }
    };
    a.cells = walker.size();
    if a.cells > cap {
        a.over_budget = true;
        return Some(a);
    }
    let axes = walker.axes.clone();
    let Some(carry_ax) = axes.iter().position(|ax| ax.col == cin) else {
        a.blocked = Some(tr!("持ち越す入力 {} に軸がありません", "the carried input {} has no axis", cin));
        return Some(a);
    };
    for (i, o, v, ty, range) in &items {
        let Some(at) = axes.iter().position(|ax| ax.col == *i) else {
            a.blocked = Some(tr!("持ち越す入力 {} に軸がありません", "the carried input {} has no axis", i));
            return Some(a);
        };
        let Some(start) = coord_of(&axes[at], v) else {
            a.blocked = Some(tr!("持ち越す入力 {} の始まりの値が軸にありません", "the starting value of the carried input {} lies on no coordinate", i));
            return Some(a);
        };
        a.more.push(Carried { input: i.clone(), output: o.clone(), ty: ty.clone(), initial: v.clone(), axis: axes[at].clone(), at, start, range: *range });
    }
    // The worlds (§15.149). A case holds its `held` inputs from its first call to its last,
    // so only calls with the same coordinates on their axes follow one another in one case.
    let fixed_idx: Vec<usize> = m.held.iter().filter_map(|n| axes.iter().position(|ax| ax.col == n.text)).collect();
    a.held = fixed_idx.iter().map(|&i| axes[i].col.clone()).collect();
    a.held_axes = fixed_idx.iter().map(|&i| axes[i].clone()).collect();
    a.coupled = coupled_inputs(f, &a.held, &axes);
    let tys: Vec<Option<Ty>> = m.onces.iter().map(|o| c.ty_of(&o.output.text)).collect();
    let flags_of = |outs: &[(String, Option<Val>)]| -> Vec<Flag> {
        m.onces
            .iter()
            .zip(tys.iter())
            .map(|(o, ty)| match (out_val(outs, &o.output.text), ty) {
                (Some(v), Some(ty)) if crate::eval::cell_matches(c, &o.cell, v, ty) => Flag::Yes,
                _ => Flag::No,
            })
            .collect()
    };
    let next_of = |outs: &[(String, Option<Val>)]| -> Option<String> {
        match out_val(outs, cout) {
            Some(Val::Enum(s)) => Some(s.clone()),
            _ => None,
        }
    };
    let more_items = a.more.clone();
    // The coordinates the further carried values of an answer lie on, `OUT` for a number
    // outside its input's range: None when one is missing.
    let to_of = |outs: &[(String, Option<Val>)]| -> Option<Vec<usize>> {
        more_items.iter().map(|it| place_of(it, out_val(outs, &it.output)?)).collect()
    };
    let decided_by = |rows: &[(String, usize)]| -> Option<(String, usize)> { rows.iter().find(|(t, _)| deciders.contains(t)).cloned() };
    // The value each world's calls are made with: the first call's. A later call of the world
    // is made again with it, and kept that way when it answers the same — which, when no
    // computed column reads the held input, it always does. Then every sequence of one
    // world's calls is a sequence one case can make, held inputs and all.
    let mut rep: HashMap<Vec<usize>, Vec<(String, Val)>> = HashMap::new();
    let fixed_names = a.held.clone();
    let mut pin = |e: Edge| -> (Edge, bool) {
        if fixed_names.is_empty() || e.approx {
            return (e, false);
        }
        let mine: Vec<(String, Val)> = fixed_names.iter().filter_map(|n| e.inputs.get(n).map(|v| (n.clone(), v.clone()))).collect();
        let Some(vals) = rep.get(&e.world).cloned() else {
            rep.insert(e.world.clone(), mine);
            return (e, false);
        };
        if vals == mine {
            return (e, false);
        }
        let mut inputs = e.inputs.clone();
        for (n, v) in &vals {
            inputs.insert(n.clone(), v.clone());
        }
        if !crate::vectors::allowed(f, &inputs) {
            return (e, true);
        }
        let an = crate::vdiff::run(f, c, inputs.iter().map(|(k, v)| (k.clone(), v.clone())).collect());
        let fl = flags_of(&an.outs);
        let same = next_of(&an.outs).as_deref() == Some(e.to.as_str())
            && decided_by(&an.rows) == e.row
            && e.flags.iter().zip(fl.iter()).all(|(x, y)| *x == Flag::Maybe || x == y)
            && (more_items.is_empty() || to_of(&an.outs).as_ref() == Some(&e.to_carry));
        if !same {
            return (e, true);
        }
        (Edge { inputs, outputs: an.outs, rows: an.rows, ..e }, false)
    };
    type Key = (String, Option<(String, usize)>, String, Vec<Flag>, Vec<usize>, Vec<usize>, Vec<usize>);
    let mut index: HashMap<Key, usize> = HashMap::new();
    let mut add = |a: &mut Analysis, e: Edge| {
        let (e, mixed) = pin(e);
        let k = (e.from.clone(), e.row.clone(), e.to.clone(), e.flags.clone(), e.world.clone(), e.from_carry.clone(), e.to_carry.clone());
        match index.get(&k) {
            None => {
                index.insert(k, a.edges.len());
                if mixed {
                    a.mixed.insert(a.edges.len());
                }
                a.edges.push(e);
            }
            // A call that was seen is the better witness of an edge only the interval gave; a
            // call that stays in its world's value is the better witness of the same edge.
            Some(&i) if a.edges[i].approx && !e.approx => {
                a.edges[i] = e;
                if mixed {
                    a.mixed.insert(i);
                } else {
                    a.mixed.remove(&i);
                }
            }
            Some(&i) if a.mixed.contains(&i) && !mixed && !e.approx => {
                a.edges[i] = e;
                a.mixed.remove(&i);
            }
            Some(_) => {}
        }
    };

    let lens: Vec<usize> = axes.iter().map(|x| x.len()).collect();
    let mut cell = vec![0usize; axes.len()];
    'cells: loop {
        let s = match &axes[carry_ax].coords[cell[carry_ax]] {
            Coord::Word(w) => w.clone(),
            _ => String::new(),
        };
        let world: Vec<usize> = fixed_idx.iter().map(|&i| cell[i]).collect();
        let fc: Vec<usize> = a.more.iter().map(|it| cell[it.at]).collect();
        match walker.settle(&cell) {
            Settled::Realized(inputs, ans) => {
                let next = next_of(&ans.outs);
                let next_ok = next.is_some()
                    && crate::vdiff::names_constant(f, c, &axes, &cell, &ans.rows, &[cout.to_string()]);
                let settled_flags: Vec<bool> =
                    m.onces.iter().map(|o| flag_settled(f, c, &axes, &cell, &ans.rows, o)).collect();
                let edge = |inp: &HashMap<String, Val>, an: &crate::vdiff::Answer, flags: Vec<Flag>, to: String, tc: Vec<usize>, approx: bool| Edge {
                    from: s.clone(),
                    to,
                    flags,
                    row: decided_by(&an.rows),
                    rows: an.rows.clone(),
                    inputs: inp.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
                    outputs: an.outs.clone(),
                    world: world.clone(),
                    from_carry: fc.clone(),
                    to_carry: tc,
                    approx,
                    cell: cell.clone(),
                };
                // What each further carried value can come to over the cell (§15.217): the
                // coordinates, or None when that is not one value or cannot be bounded.
                let mut targets: Vec<Option<Vec<usize>>> = Vec::new();
                let mut one_answer = next_ok;
                for it in &more_items {
                    if it.number() {
                        match image_out(f, c, &axes, &cell, &ans.rows, &it.output, &it.ty) {
                            Some((lo, hi)) => {
                                let (rlo, rhi) = it.range.unwrap_or((lo, hi));
                                if lo != hi {
                                    one_answer = false;
                                }
                                let past = lo.cmp_to(rlo) == std::cmp::Ordering::Less || hi.cmp_to(rhi) == std::cmp::Ordering::Greater;
                                let lo = if lo.cmp_to(rlo) == std::cmp::Ordering::Less { rlo } else { lo };
                                let hi = if hi.cmp_to(rhi) == std::cmp::Ordering::Greater { rhi } else { hi };
                                let mut cs = if lo.cmp_to(hi) == std::cmp::Ordering::Greater { Vec::new() } else { coords_meeting(&it.axis, lo, hi) };
                                if past {
                                    cs.push(OUT);
                                }
                                targets.push(Some(cs));
                            }
                            None => {
                                one_answer = false;
                                targets.push(None);
                            }
                        }
                    } else {
                        let constant = crate::vdiff::names_constant(f, c, &axes, &cell, &ans.rows, std::slice::from_ref(&it.output));
                        match (constant, out_val(&ans.outs, &it.output).and_then(|v| coord_of(&it.axis, v))) {
                            (true, Some(ci)) => targets.push(Some(vec![ci])),
                            _ => {
                                one_answer = false;
                                targets.push(None);
                            }
                        }
                    }
                }
                if one_answer && settled_flags.iter().all(|b| *b) {
                    let fl = flags_of(&ans.outs);
                    match to_of(&ans.outs) {
                        Some(tc) => {
                            let e = edge(&inputs, &ans, fl, next.unwrap_or_default(), tc, false);
                            add(&mut a, e);
                        }
                        None => *a.unknown.entry((s.clone(), fc.clone())).or_default() += 1,
                    }
                } else {
                    // Not one answer over the cell, or not shown to be. Ask the cell's other
                    // ends; whatever they answer is a transition that really happens.
                    let mut seen: Vec<(HashMap<String, Val>, crate::vdiff::Answer)> = vec![(inputs.clone(), ans)];
                    seen.extend(walker.others(&cell, &inputs));
                    let mut observed_flags: Vec<BTreeSet<Flag>> = vec![BTreeSet::new(); m.onces.len()];
                    let mut observed_to: BTreeSet<(String, Vec<usize>)> = BTreeSet::new();
                    for (inp, an) in &seen {
                        let Some(to) = next_of(&an.outs) else { continue };
                        let fl = flags_of(&an.outs);
                        for (k, x) in fl.iter().enumerate() {
                            observed_flags[k].insert(*x);
                        }
                        if let Some(tc) = to_of(&an.outs) {
                            observed_to.insert((to.clone(), tc.clone()));
                            let e = edge(inp, an, fl, to, tc, false);
                            add(&mut a, e);
                        }
                    }
                    let open: Vec<usize> = (0..m.onces.len()).filter(|&k| !settled_flags[k]).collect();
                    if !next_ok || targets.iter().any(|t| t.is_none()) {
                        // Another next state, or another value of a carried boolean or enum, may
                        // hide in the cell.
                        *a.unknown.entry((s.clone(), fc.clone())).or_default() += 1;
                    } else {
                        let (inp, an) = &seen[0];
                        let mut fl = flags_of(&an.outs);
                        for &k in &open {
                            fl[k] = Flag::Maybe;
                        }
                        let to = next.clone().unwrap_or_default();
                        if !more_items.is_empty() {
                            // The coordinates a carried number's next value can fall in, past
                            // the ones the points answered: edges no call was seen to make.
                            let lists: Vec<Vec<usize>> = targets.iter().map(|t| t.clone().unwrap_or_default()).collect();
                            for tc in combos(&lists) {
                                if !observed_to.contains(&(to.clone(), tc.clone())) {
                                    let e = edge(inp, an, fl.clone(), to.clone(), tc, true);
                                    add(&mut a, e);
                                }
                            }
                        }
                        let covered = open.len() == 1 && observed_flags[open[0]].len() == 2;
                        if !covered && !open.is_empty() {
                            if let Some(tc) = to_of(&an.outs) {
                                let e = edge(inp, an, fl, to, tc, false);
                                add(&mut a, e);
                            }
                        } else if !covered && more_items.is_empty() {
                            let e = edge(inp, an, fl, to, Vec::new(), false);
                            add(&mut a, e);
                        }
                    }
                }
            }
            Settled::Empty => {}
            Settled::Unknown => *a.unknown.entry((s, fc)).or_default() += 1,
        }
        let mut i = axes.len();
        loop {
            if i == 0 {
                break 'cells;
            }
            i -= 1;
            cell[i] += 1;
            if cell[i] < lens[i] {
                break;
            }
            cell[i] = 0;
        }
    }
    settle_claims(&mut a, f, m);
    if !a.more.is_empty() {
        settle_more(&mut a, f, c, m, &mut walker);
    }
    Some(a)
}

/// Breadth-first from `starts`; `succ` gives (edge, node) pairs. Returns the nodes in the
/// order met and each node's (previous node, edge). A start has no previous node.
fn bfs_from<N: Clone + Eq + std::hash::Hash>(starts: Vec<N>, succ: impl Fn(&N) -> Vec<(usize, N)>) -> (Vec<N>, HashMap<N, (N, usize)>) {
    let mut order = Vec::new();
    let mut parent: HashMap<N, (N, usize)> = HashMap::new();
    let mut seen: std::collections::HashSet<N> = std::collections::HashSet::new();
    let mut q = VecDeque::new();
    for s in starts {
        if seen.insert(s.clone()) {
            order.push(s.clone());
            q.push_back(s);
        }
    }
    while let Some(n) = q.pop_front() {
        for (e, m) in succ(&n) {
            if seen.insert(m.clone()) {
                parent.insert(m.clone(), (n.clone(), e));
                order.push(m.clone());
                q.push_back(m);
            }
        }
    }
    (order, parent)
}

/// Breadth-first from one node.
fn bfs<N: Clone + Eq + std::hash::Hash>(start: N, succ: impl Fn(&N) -> Vec<(usize, N)>) -> (Vec<N>, HashMap<N, (N, usize)>) {
    bfs_from(vec![start], succ)
}

/// The edges from a start of the search to `end`.
fn path_to<N: Clone + Eq + std::hash::Hash>(parent: &HashMap<N, (N, usize)>, end: &N) -> Vec<usize> {
    let mut out = Vec::new();
    let mut at = end.clone();
    while let Some((p, e)) = parent.get(&at) {
        out.push(*e);
        at = p.clone();
    }
    out.reverse();
    out
}

fn settle_claims(a: &mut Analysis, f: &RuleFile, m: &MachineDecl) {
    let edges = a.edges.clone();
    // A case's calls stay in one world, so the searches walk (state, world) — and the further
    // carried values' coordinates, when there are any (§15.217).
    let succ = |n: &Node| -> Vec<(usize, Node)> {
        edges
            .iter()
            .enumerate()
            .filter(|(_, e)| e.from == n.0 && e.world == n.1 && e.from_carry == n.2)
            .map(|(i, e)| (i, (e.to.clone(), n.1.clone(), e.to_carry.clone())))
            .collect()
    };
    // The worlds a case can start in: those with a call from the initial state.
    let start_carry: Vec<usize> = a.more.iter().map(|it| it.start).collect();
    let mut starts: Vec<Node> = Vec::new();
    for e in &edges {
        let n = (a.initial.clone(), e.world.clone(), start_carry.clone());
        if e.from == a.initial && e.from_carry == start_carry && !starts.contains(&n) {
            starts.push(n);
        }
    }
    if starts.is_empty() {
        starts.push((a.initial.clone(), vec![0; a.held.len()], start_carry.clone()));
    }
    let (order, parent) = bfs_from(starts.clone(), &succ);
    a.nodes = order.clone();
    a.node_parent = parent;
    let mut states: Vec<String> = Vec::new();
    for (st, _, _) in &order {
        if !states.contains(st) {
            states.push(st.clone());
        }
    }
    a.reachable = states;
    a.parent = a
        .reachable
        .iter()
        .filter_map(|st| {
            let n = order.iter().find(|n| &n.0 == st)?;
            a.node_parent.get(n).map(|(_, e)| (st.clone(), *e))
        })
        .collect();
    let open_at = |a: &Analysis, n: &Node| a.unknown.get(&(n.0.clone(), n.2.clone())).copied().unwrap_or(0) > 0;
    a.exact = !a.nodes.iter().any(|n| open_at(a, n));
    let unsettled = |a: &Analysis, set: &mut dyn Iterator<Item = Node>| -> Vec<String> {
        let mut v: Vec<String> = set.filter(|n| open_at(a, n)).map(|n| n.0).collect();
        v.sort();
        v.dedup();
        v
    };
    let undecided = |states: &[String]| {
        tr!(
            "状態 {} からの呼び出しに、答えを出す入力を作れず、起こらないとも示せなかった区画があります",
            "calls from state {} include cells no input was built for and none shown impossible",
            states.join(" / ")
        )
    };
    // A sequence through a call that could not be made with its world's value is not shown to
    // be one case's (§15.149).
    let fixed_text = a.held.join(", ");
    let broken = |a: &Analysis, t: Vec<usize>| -> Verdict {
        if t.iter().any(|e| a.mixed.contains(e)) {
            Verdict::Undecided(tr!(
                "見つかった手順は、途中で {} を変えないと通れません。一つの値のまま通れるかまでは、調べていません",
                "the sequence found needs {} to change partway, and whether one value carries it is not followed",
                fixed_text
            ))
        } else {
            Verdict::Broken(t.iter().map(|&i| a.edges[i].clone()).collect())
        }
    };

    // Final states: nothing leads out.
    for fs in &a.finals.clone() {
        let exit = a
            .nodes
            .iter()
            .filter(|n| &n.0 == fs)
            .find_map(|n| succ(n).into_iter().find(|(_, to)| &to.0 != fs).map(|(e, _)| (n.clone(), e)));
        let v = match exit {
            Some((n, e)) => {
                let mut t = a.trace_to_node(&n).unwrap_or_default();
                t.push(e);
                broken(a, t)
            }
            // A final state no case reaches in any world still should not lead out: the row
            // is a claim the table makes, whether or not a case ever tests it. One a case
            // reaches is judged by the worlds it reaches it in (§15.149).
            None => match a.out_of(fs).find(|(_, e)| &e.to != fs).map(|(i, _)| i) {
                Some(e) if !a.reachable.contains(fs) => broken(a, vec![e]),
                _ if a.unsettled_at(fs) => Verdict::Undecided(undecided(std::slice::from_ref(fs))),
                _ => Verdict::Holds,
            },
        };
        a.final_claims.push((fs.clone(), v));
    }

    // Every reachable state can still get to a final one, in the world the case is in.
    if !a.finals.is_empty() {
        let can = a.can_finish();
        for st in a.reachable.clone() {
            if a.finals.contains(&st) {
                continue;
            }
            let mut v = Verdict::Holds;
            for n in a.nodes.clone().iter().filter(|n| n.0 == st) {
                if can.contains(n) {
                    continue;
                }
                let (closure, _) = bfs_from(vec![n.clone()], &succ);
                let open = unsettled(a, &mut closure.into_iter());
                let here = if open.is_empty() { broken(a, a.trace_to_node(n).unwrap_or_default()) } else { Verdict::Undecided(undecided(&open)) };
                match (&v, &here) {
                    (Verdict::Broken(_), _) => {}
                    (_, Verdict::Broken(_)) | (Verdict::Holds, _) => v = here,
                    _ => {}
                }
            }
            if v.holds() && !a.coupled.is_empty() {
                v = Verdict::Undecided(tr!(
                    "{} は、ほかの入力と一緒に計算される値にも使われています。一つの値のまま終わりに着けるかまでは、調べていません",
                    "{} also feeds a value computed with other inputs, and whether one value of it can always still reach an end is not followed",
                    a.coupled.join(", ")
                ));
            }
            a.finish_claims.push((st, v));
        }
    }

    // `never A after B`.
    for nv in &m.nevers {
        let aset: BTreeSet<String> = nv.states.iter().map(|n| n.text.clone()).collect();
        let bset: BTreeSet<String> = nv.after.iter().map(|n| n.text.clone()).collect();
        let first: Vec<(Node, bool)> = starts.iter().map(|n| (n.clone(), bset.contains(&n.0))).collect();
        let (order, parent) = bfs_from(first, |(n, seen): &(Node, bool)| {
            succ(n).into_iter().map(|(i, to)| (i, (to.clone(), *seen || bset.contains(&to.0)))).collect()
        });
        let bad = order.iter().find(|(n, seen)| *seen && aset.contains(&n.0));
        let v = match bad {
            Some(b) => broken(a, path_to(&parent, b)),
            None => {
                let open = unsettled(a, &mut order.iter().map(|(n, _)| n.clone()));
                if open.is_empty() { Verdict::Holds } else { Verdict::Undecided(undecided(&open)) }
            }
        };
        let mut set: Vec<(String, bool)> = Vec::new();
        for (n, seen) in &order {
            if !set.contains(&(n.0.clone(), *seen)) {
                set.push((n.0.clone(), *seen));
            }
        }
        a.never_sets.push(set);
        a.never_claims.push(v);
    }

    // `once`: at most one call of a case counts.
    for (k, _) in m.onces.iter().enumerate() {
        let count = |fl: Flag, loose: bool| -> u8 { u8::from(fl == Flag::Yes || (loose && fl == Flag::Maybe)) };
        let search = |loose: bool| {
            let first: Vec<(Node, u8)> = starts.iter().map(|n| (n.clone(), 0u8)).collect();
            bfs_from(first, |(n, c): &(Node, u8)| {
                succ(n).into_iter().map(|(i, to)| (i, (to, (*c + count(edges[i].flags[k], loose)).min(2)))).collect()
            })
        };
        let (order, parent) = search(false);
        let v = match order.iter().find(|(_, c)| *c >= 2) {
            Some(b) => broken(a, path_to(&parent, b)),
            None => {
                let (loose, _) = search(true);
                let open = unsettled(a, &mut loose.iter().map(|(n, _)| n.clone()));
                if loose.iter().any(|(_, c)| *c >= 2) {
                    Verdict::Undecided(tr!(
                        "数えるかどうかを区画全体では決めきれない呼び出しがあります",
                        "whether some calls count could not be settled over their whole cell"
                    ))
                } else if !open.is_empty() {
                    Verdict::Undecided(undecided(&open))
                } else {
                    Verdict::Holds
                }
            }
        };
        let mut set: Vec<(String, u8)> = Vec::new();
        for (n, c) in &order {
            if !set.contains(&(n.0.clone(), *c)) {
                set.push((n.0.clone(), *c));
            }
        }
        a.once_sets.push(set);
        a.once_claims.push(v);
    }

    // Transitions nobody takes.
    if a.exact {
        let reached: HashSet<&Node> = a.nodes.iter().collect();
        let taken: BTreeSet<(String, usize)> = a
            .edges
            .iter()
            .filter(|e| reached.contains(&(e.from.clone(), e.world.clone(), e.from_carry.clone())))
            .filter_map(|e| e.row.clone())
            .collect();
        for it in &f.items {
            let Item::Table(t) = it else { continue };
            if !t.outputs.iter().any(|o| o.name.text == a.carry.1) {
                continue;
            }
            let Some(tn) = &t.name else { continue };
            for r in &t.rows {
                let k = (tn.text.clone(), r.index);
                if !taken.contains(&k) {
                    a.unused.push(k);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// Further carried values (§15.217)
// ---------------------------------------------------------------------------------------

/// The values of the `held` inputs one world's calls are made with: the first call's.
fn world_inputs(a: &Analysis) -> HashMap<Vec<usize>, Vec<(String, Val)>> {
    let mut out: HashMap<Vec<usize>, Vec<(String, Val)>> = HashMap::new();
    for (i, e) in a.edges.iter().enumerate() {
        if e.approx || a.mixed.contains(&i) || out.contains_key(&e.world) {
            continue;
        }
        out.insert(e.world.clone(), a.held.iter().filter_map(|n| e.inputs.get(n).map(|v| (n.clone(), v.clone()))).collect());
    }
    out
}

/// The calls that really happen, from where a case starts (§15.217).
///
/// When the numbers are walked whole, a node of the walk is one place a case can be, and the
/// calls the walk saw are calls from there: they are taken as they are. Otherwise a node is an
/// interval, and its call was made at one value of it; the search then starts from the initial
/// values and, from each place a case is found in, asks the rule again with that place's values
/// and each of the node's calls — the one the walk saw, and the far ends of its cell — keeping
/// what comes back. It stops at `REAL_STATES` places or `REAL_RUNS` calls.
fn real_calls(a: &Analysis, f: &RuleFile, c: &Checked, m: &MachineDecl, walker: &mut Walker) -> Real {
    let mut r = Real { complete: true, ..Default::default() };
    let mut ids: HashMap<(String, Vec<String>, Vec<usize>), usize> = HashMap::new();
    let mut intern = |r: &mut Real, s: &str, vals: &[Val], w: &[usize]| -> (usize, bool) {
        let k = (s.to_string(), vals.iter().map(crate::vectors::show).collect::<Vec<_>>(), w.to_vec());
        if let Some(&i) = ids.get(&k) {
            return (i, false);
        }
        ids.insert(k, r.nodes.len());
        r.nodes.push((s.to_string(), vals.to_vec(), w.to_vec()));
        (r.nodes.len() - 1, true)
    };
    let starts: Vec<Vec<usize>> = {
        let mut ws: Vec<Vec<usize>> = Vec::new();
        for n in a.nodes.iter().filter(|n| n.0 == a.initial && a.node_parent.get(*n).is_none()) {
            if !ws.contains(&n.1) {
                ws.push(n.1.clone());
            }
        }
        ws
    };
    let initial_vals: Vec<Val> = a.more.iter().map(|it| it.initial.clone()).collect();
    if a.whole {
        for w in &starts {
            let (i, _) = intern(&mut r, &a.initial, &initial_vals, w);
            r.starts.push(i);
        }
        // Each call the walk saw is one a case makes from the values it was made with, to the
        // values it answers — outside the range, too.
        for (i, e) in a.edges.iter().enumerate() {
            if e.approx || a.mixed.contains(&i) {
                continue;
            }
            let fv: Option<Vec<Val>> = a.more.iter().map(|it| e.inputs.get(&it.input).cloned()).collect();
            let tv: Option<Vec<Val>> = a.more.iter().map(|it| out_val(&e.outputs, &it.output).cloned()).collect();
            let (Some(fv), Some(tv)) = (fv, tv) else { continue };
            let (from, _) = intern(&mut r, &e.from, &fv, &e.world);
            let (to, _) = intern(&mut r, &e.to, &tv, &e.world);
            r.calls.push((from, concrete(m, c, e.clone()), Some(to)));
        }
        return r;
    }
    let (cin, cout) = (a.carry.0.clone(), a.carry.1.clone());
    let worlds = world_inputs(a);
    // The calls of each node of the walk, and of the cells whose next value can fall outside the
    // range: the inputs a call is made with, the ones the walk saw first.
    let mut seeds: HashMap<Node, Vec<(Vec<usize>, BTreeMap<String, Val>)>> = HashMap::new();
    for (i, e) in a.edges.iter().enumerate() {
        if e.approx || a.mixed.contains(&i) {
            continue;
        }
        let list = seeds.entry((e.from.clone(), e.world.clone(), e.from_carry.clone())).or_default();
        if !list.iter().any(|(cell, _)| *cell == e.cell) {
            list.push((e.cell.clone(), e.inputs.clone()));
        }
    }
    let mut others: HashMap<Vec<usize>, Vec<BTreeMap<String, Val>>> = HashMap::new();
    let mut q: VecDeque<usize> = VecDeque::new();
    for w in &starts {
        let (i, _) = intern(&mut r, &a.initial, &initial_vals, w);
        r.starts.push(i);
        q.push_back(i);
    }
    let tys: Vec<Option<Ty>> = m.onces.iter().map(|o| c.ty_of(&o.output.text)).collect();
    let deciders: BTreeSet<String> = f
        .items
        .iter()
        .filter_map(|it| match it {
            Item::Table(t) if t.outputs.iter().any(|o| o.name.text == cout) => t.name.as_ref().map(|n| n.text.clone()),
            _ => None,
        })
        .collect();
    let mut made: HashSet<(usize, Option<usize>, Option<(String, usize)>)> = HashSet::new();
    let mut runs = 0usize;
    while let Some(id) = q.pop_front() {
        if r.nodes.len() > REAL_STATES || runs > REAL_RUNS {
            r.complete = false;
            break;
        }
        let (s, vals, w) = r.nodes[id].clone();
        let Some(cc) = a.more.iter().zip(&vals).map(|(it, v)| coord_of(&it.axis, v)).collect::<Option<Vec<usize>>>() else { continue };
        let Some(list) = seeds.get(&(s.clone(), w.clone(), cc.clone())).cloned() else { continue };
        for (cell, first) in list {
            let more_points = others
                .entry(cell.clone())
                .or_insert_with(|| {
                    let firsts: HashMap<String, Val> = first.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                    walker.others(&cell, &firsts).into_iter().map(|(p, _)| p.into_iter().collect()).collect()
                })
                .clone();
            for cand in std::iter::once(first.clone()).chain(more_points) {
                let mut inp = cand;
                inp.insert(cin.clone(), Val::Enum(s.clone()));
                for (it, v) in a.more.iter().zip(&vals) {
                    inp.insert(it.input.clone(), v.clone());
                }
                if let Some(hv) = worlds.get(&w) {
                    for (n, v) in hv {
                        inp.insert(n.clone(), v.clone());
                    }
                }
                if !crate::vectors::allowed(f, &inp) {
                    continue;
                }
                runs += 1;
                let an = crate::vdiff::run(f, c, inp.iter().map(|(k, v)| (k.clone(), v.clone())).collect());
                let Some(Val::Enum(next)) = out_val(&an.outs, &cout).cloned() else { continue };
                let row = an.rows.iter().find(|(t, _)| deciders.contains(t)).cloned();
                let nv: Option<Vec<Val>> = a.more.iter().map(|it| out_val(&an.outs, &it.output).cloned()).collect();
                let Some(nv) = nv else { continue };
                // A value outside its range is a place nothing leads out of.
                let inside = a.more.iter().zip(&nv).all(|(it, v)| coord_of(&it.axis, v).is_some());
                let (t, fresh) = intern(&mut r, &next, &nv, &w);
                if fresh && inside {
                    q.push_back(t);
                }
                let to = Some(t);
                if !made.insert((id, to, row.clone())) {
                    continue;
                }
                let flags = m
                    .onces
                    .iter()
                    .zip(tys.iter())
                    .map(|(o, ty)| match (out_val(&an.outs, &o.output.text), ty) {
                        (Some(v), Some(ty)) if crate::eval::cell_matches(c, &o.cell, v, ty) => Flag::Yes,
                        _ => Flag::No,
                    })
                    .collect();
                let e = Edge {
                    from: s.clone(),
                    to: next,
                    flags,
                    row,
                    rows: an.rows,
                    inputs: inp,
                    outputs: an.outs,
                    world: w.clone(),
                    from_carry: cc.clone(),
                    to_carry: Vec::new(),
                    approx: false,
                    cell: cell.clone(),
                };
                r.calls.push((id, e, to));
            }
        }
    }
    r
}

/// A call the walk saw, with its `once` flags read off what it answers: a call that really
/// happens counts or does not.
fn concrete(m: &MachineDecl, c: &Checked, mut e: Edge) -> Edge {
    e.flags = m
        .onces
        .iter()
        .map(|o| match (out_val(&e.outputs, &o.output.text), c.ty_of(&o.output.text)) {
            (Some(v), Some(ty)) if crate::eval::cell_matches(c, &o.cell, v, &ty) => Flag::Yes,
            _ => Flag::No,
        })
        .collect();
    e
}

/// The claims of a machine that carries more than its state (§15.217), settled again over the
/// calls that really happen, and the two that only such a machine makes: `always`, and that a
/// carried number stays within its input's range.
///
/// The walk has already settled every claim over its nodes. Where a claim holds there, it
/// holds: the nodes take in every place a case can be. Where it is broken there, the sequence
/// is looked for among the calls that really happen; found, it is the counterexample, every
/// call one the rule takes from the values the call before answered; not found, the claim is
/// not settled (W127).
fn settle_more(a: &mut Analysis, f: &RuleFile, c: &Checked, m: &MachineDecl, walker: &mut Walker) {
    a.real = real_calls(a, f, c, m, walker);
    let real = a.real.clone();
    let (order, parent) = real.reach();
    let mut out: Vec<Vec<usize>> = vec![Vec::new(); real.nodes.len()];
    for (i, (from, _, _)) in real.calls.iter().enumerate() {
        out[*from].push(i);
    }
    let not_made = |was: &Verdict| -> Verdict {
        let mut why = tr!(
            "持ち越す値を区間にまとめた上では破れる手順が見つかりましたが、本当に起きる呼び出しでは作り直せませんでした",
            "a sequence that breaks it was found over the intervals of the carried values, and it could not be made again from calls that really happen"
        );
        if let Verdict::Undecided(w) = was {
            why = w.clone();
        }
        if !real.complete {
            why.push_str(&tr!(
                "（本当に起きる呼び出しは、{} 通りの状態まで調べました）",
                " (the calls that really happen were followed to {} places a case can be in)",
                real.nodes.len()
            ));
        }
        Verdict::Undecided(why)
    };
    let more = a.more.clone();
    let nodes = a.nodes.clone();
    let unknown = a.unknown.clone();
    // A node's values against its coordinates, as the walk's nodes have them.
    let walk_node = |i: usize| -> Option<Node> {
        let (s, vals, w) = &real.nodes[i];
        let cc: Option<Vec<usize>> = more.iter().zip(vals).map(|(it, v)| place_of(it, v)).collect();
        Some((s.clone(), w.clone(), cc?))
    };

    // `final`: a call that leaves a final state, from a place a case really reaches.
    for k in 0..a.final_claims.len() {
        let (fs, v) = a.final_claims[k].clone();
        if v.holds() {
            continue;
        }
        let found = order.iter().find_map(|&n| {
            if real.nodes[n].0 != fs {
                return None;
            }
            out[n].iter().find(|&&i| real.calls[i].1.to != fs).map(|&i| (n, i))
        });
        a.final_claims[k].1 = match found {
            Some((n, i)) => {
                let mut t = real.path(&parent, n);
                t.push(real.calls[i].1.clone());
                Verdict::Broken(t)
            }
            // A final state no case reaches: its row leads out whatever the values.
            None if !a.reachable.contains(&fs) && matches!(v, Verdict::Broken(_)) => v,
            None => not_made(&v),
        };
    }

    // `never A after B`.
    for (k, nv) in m.nevers.iter().enumerate() {
        if a.never_claims[k].holds() {
            continue;
        }
        let aset: BTreeSet<&str> = nv.states.iter().map(|n| n.text.as_str()).collect();
        let bset: BTreeSet<&str> = nv.after.iter().map(|n| n.text.as_str()).collect();
        let first: Vec<(usize, bool)> = real.starts.iter().map(|&n| (n, bset.contains(real.nodes[n].0.as_str()))).collect();
        let (seen_order, seen_parent) = bfs_from(first, |(n, seen): &(usize, bool)| {
            out[*n].iter().filter_map(|&i| real.calls[i].2.map(|t| (i, (t, *seen || bset.contains(real.nodes[t].0.as_str()))))).collect()
        });
        let bad = seen_order.iter().find(|(n, seen)| *seen && aset.contains(real.nodes[*n].0.as_str()));
        a.never_claims[k] = match bad {
            Some(b) => Verdict::Broken(path_to(&seen_parent, b).into_iter().map(|i| real.calls[i].1.clone()).collect()),
            None => not_made(&a.never_claims[k]),
        };
    }

    // `once`.
    for k in 0..m.onces.len() {
        if a.once_claims[k].holds() {
            continue;
        }
        let first: Vec<(usize, u8)> = real.starts.iter().map(|&n| (n, 0u8)).collect();
        let (cnt_order, cnt_parent) = bfs_from(first, |(n, cnt): &(usize, u8)| {
            out[*n]
                .iter()
                .filter_map(|&i| real.calls[i].2.map(|t| (i, (t, (*cnt + u8::from(real.calls[i].1.flags[k] == Flag::Yes)).min(2)))))
                .collect()
        });
        let found: Option<Vec<Edge>> = cnt_order.iter().find(|(_, cnt)| *cnt >= 2).map(|b| path_to(&cnt_parent, b).into_iter().map(|i| real.calls[i].1.clone()).collect());
        a.once_claims[k] = match found {
            Some(t) => Verdict::Broken(t),
            None => not_made(&a.once_claims[k]),
        };
    }

    // `always`: over the walk's nodes first, then over the places a case really reaches.
    let open_at = |n: &Node| unknown.get(&(n.0.clone(), n.2.clone())).copied().unwrap_or(0) > 0;
    let open_states = |set: &mut dyn Iterator<Item = &Node>| -> Vec<String> {
        let mut v: Vec<String> = set.filter(|n| open_at(n)).map(|n| n.0.clone()).collect();
        v.sort();
        v.dedup();
        v
    };
    let fits = |al: &AlwaysDecl, vals: &[Val]| -> bool {
        al.tests.iter().all(|(n, cell, _)| match more.iter().position(|it| it.input == n.text) {
            Some(k) => crate::eval::cell_matches(c, cell, &vals[k], &more[k].ty),
            None => true,
        })
    };
    // A place where a carried number has left its range takes no further call, and E134 says
    // so: `always` speaks of the places within the ranges.
    let within = |vals: &[Val]| more.iter().zip(vals).all(|(it, v)| place_of(it, v) != Some(OUT));
    for al in &m.alwayses {
        let speaks = |s: &str| al.when.is_empty() || al.when.iter().any(|w| w.text == s);
        let bad_node = nodes.iter().any(|n| {
            speaks(&n.0) && !n.2.contains(&OUT) && {
                let vals: Option<Vec<Val>> = more.iter().zip(&n.2).map(|(it, &ci)| crate::vdiff::coord_val(&it.axis, ci)).collect();
                vals.is_none_or(|vals| !fits(al, &vals))
            }
        });
        let open = open_states(&mut nodes.iter());
        let v = if !bad_node && open.is_empty() {
            Verdict::Holds
        } else {
            let found = order.iter().find(|&&n| speaks(&real.nodes[n].0) && within(&real.nodes[n].1) && !fits(al, &real.nodes[n].1));
            match found {
                Some(&n) => Verdict::Broken(real.path(&parent, n)),
                None if !bad_node => Verdict::Undecided(tr!(
                    "状態 {} からの呼び出しに、答えを出す入力を作れず、起こらないとも示せなかった区画があります",
                    "calls from state {} include cells no input was built for and none shown impossible",
                    open.join(" / ")
                )),
                None => not_made(&Verdict::Holds),
            }
        };
        a.always_claims.push(v);
    }

    // A carried number's next value stays within its input's range (E134).
    let reached: HashSet<Node> = nodes.iter().cloned().collect();
    for (k, it) in more.iter().enumerate() {
        if !it.number() {
            continue;
        }
        let beyond = reached.iter().any(|n| n.2.get(k) == Some(&OUT));
        let open = open_states(&mut nodes.iter());
        let v = if !beyond && open.is_empty() {
            Verdict::Holds
        } else {
            let found = order.iter().find(|&&n| coord_of(&it.axis, &real.nodes[n].1[k]).is_none());
            match found {
                Some(&n) => Verdict::Broken(real.path(&parent, n)),
                None if !beyond => Verdict::Undecided(tr!(
                    "状態 {} からの呼び出しに、答えを出す入力を作れず、起こらないとも示せなかった区画があります",
                    "calls from state {} include cells no input was built for and none shown impossible",
                    open.join(" / ")
                )),
                None => not_made(&Verdict::Holds),
            }
        };
        a.range_claims.push((it.input.clone(), v));
    }

    // Whether every place a case reaches can still get to a final one. Walked whole, a node is
    // one place, and a way to the end is made of calls that were seen; in intervals, a path of
    // the walk is not one a case is shown to take, and the claim is not followed.
    if !a.finals.is_empty() {
        if !a.whole {
            a.finish_claims.clear();
            a.finish_open = Some(tr!(
                "持ち越す数を区間にまとめて調べたので、本当に起きる呼び出しで終わりの状態まで着けるかは、確かめていません",
                "the carried numbers were walked in intervals, and that calls which really happen take a case to a final state is not followed there"
            ));
        } else {
            let mut can: HashSet<usize> = real.nodes.iter().enumerate().filter(|(_, n)| a.finals.contains(&n.0)).map(|(i, _)| i).collect();
            loop {
                let before = can.len();
                for (from, _, to) in &real.calls {
                    if to.is_some_and(|t| can.contains(&t)) {
                        can.insert(*from);
                    }
                }
                if can.len() == before {
                    break;
                }
            }
            let real_id: HashMap<Node, usize> = (0..real.nodes.len()).filter_map(|i| walk_node(i).map(|n| (n, i))).collect();
            let edges = a.edges.clone();
            let succ = |n: &Node| -> Vec<(usize, Node)> {
                edges
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| e.from == n.0 && e.world == n.1 && e.from_carry == n.2)
                    .map(|(i, e)| (i, (e.to.clone(), n.1.clone(), e.to_carry.clone())))
                    .collect()
            };
            let reached_real: HashSet<usize> = order.iter().copied().collect();
            for k in 0..a.finish_claims.len() {
                let st = a.finish_claims[k].0.clone();
                let mut v = Verdict::Holds;
                // A place outside a range takes no further call; E134 says so, and it is not
                // said again here.
                for n in nodes.iter().filter(|n| n.0 == st && !n.2.contains(&OUT)) {
                    if real_id.get(n).is_some_and(|i| can.contains(i)) {
                        continue;
                    }
                    let (closure, _) = bfs_from(vec![n.clone()], &succ);
                    let finishes = closure.iter().any(|x| a.finals.contains(&x.0));
                    let open = open_states(&mut closure.iter());
                    let here = match real_id.get(n) {
                        Some(&i) if !finishes && open.is_empty() && reached_real.contains(&i) => Verdict::Broken(real.path(&parent, i)),
                        _ if !open.is_empty() => Verdict::Undecided(tr!(
                            "状態 {} からの呼び出しに、答えを出す入力を作れず、起こらないとも示せなかった区画があります",
                            "calls from state {} include cells no input was built for and none shown impossible",
                            open.join(" / ")
                        )),
                        _ => Verdict::Undecided(tr!(
                            "{} から終わりの状態へ行く手順は、入力を作れなかった呼び出しを通ります",
                            "the way from {} to a final state goes through calls no input was seen to make",
                            st
                        )),
                    };
                    match (&v, &here) {
                        (Verdict::Broken(_), _) => {}
                        (_, Verdict::Broken(_)) | (Verdict::Holds, _) => v = here,
                        _ => {}
                    }
                }
                if v.holds() && !a.coupled.is_empty() {
                    v = Verdict::Undecided(tr!(
                        "{} は、ほかの入力と一緒に計算される値にも使われています。一つの値のまま終わりに着けるかまでは、調べていません",
                        "{} also feeds a value computed with other inputs, and whether one value of it can always still reach an end is not followed",
                        a.coupled.join(", ")
                    ));
                }
                a.finish_claims[k].1 = v;
            }
        }
    }
}

// ---------------------------------------------------------------------------------------
// Findings
// ---------------------------------------------------------------------------------------

/// A value as a person reads it: `3000円`, `8%`, `受付`.
fn human(c: &Checked, name: &str, v: &Val) -> String {
    v.show(&c.ty_of(name).unwrap_or(Ty::Unknown))
}

/// One call, in the wire's words.
fn step_of(c: &Checked, f: &RuleFile, e: &Edge) -> Step {
    let mut inputs: Vec<(String, WVal)> = Vec::new();
    for i in &f.inputs {
        if let Some(v) = e.inputs.get(&i.name.text) {
            inputs.push((i.name.text.clone(), crate::eval::wval(c, &i.name.text, v)));
        }
    }
    for el in &f.elements {
        if let Some(v) = e.inputs.get(&el.name.text) {
            inputs.push((el.name.text.clone(), crate::eval::wval(c, &el.name.text, v)));
        }
    }
    let outputs = e
        .outputs
        .iter()
        .filter_map(|(k, v)| v.as_ref().map(|v| (k.clone(), crate::eval::wval(c, k, v))))
        .collect();
    let rows = e.rows.iter().map(|(t, r)| RowRef { table: t.clone(), row: *r }).collect();
    Step { inputs, outputs, rows }
}

/// The lines of prose that walk a reader through a trace. A machine that carries more than its
/// state shows those values beside the state each call is made in, and what the call answers
/// for them (§15.217).
fn trace_lines(a: &Analysis, c: &Checked, f: &RuleFile, trace: &[Edge], show: &[String]) -> Vec<String> {
    let mut out = vec![tr!("手順（{} から）:", "Calls (from {}):", a.initial)];
    let carried: Vec<&str> = std::iter::once(a.carry.0.as_str()).chain(a.more.iter().map(|it| it.input.as_str())).collect();
    for (k, e) in trace.iter().enumerate() {
        let args: Vec<String> = f
            .inputs
            .iter()
            .filter(|i| !carried.contains(&i.name.text.as_str()))
            .filter_map(|i| e.inputs.get(&i.name.text).map(|v| format!("{} = {}", i.name.text, human(c, &i.name.text, v))))
            .collect();
        let mut after: Vec<String> = a
            .more
            .iter()
            .filter_map(|it| out_val(&e.outputs, &it.output).map(|v| format!("{} = {}", it.output, human(c, &it.output, v))))
            .collect();
        after.extend(show.iter().filter_map(|o| out_val(&e.outputs, o).map(|v| format!("{o} = {}", human(c, o, v)))));
        if let Some((t, r)) = &e.row {
            after.push(crate::eval::row_tag(t, *r));
        }
        let held: Vec<String> = a
            .more
            .iter()
            .filter_map(|it| e.inputs.get(&it.input).map(|v| format!("{} = {}", it.input, human(c, &it.input, v))))
            .collect();
        let from = if held.is_empty() { e.from.clone() } else { tr!("{}（{}）", "{} ({})", e.from, held.join(", ")) };
        let head = match (args.is_empty(), crate::i18n::ja()) {
            (true, _) => from,
            (false, true) => format!("{from} のとき {}", args.join(", ")),
            (false, false) => format!("{from}, {}", args.join(", ")),
        };
        out.push(tr!("  {}. {} → {}（{}）", "  {}. at {} → {} ({})", k + 1, head, e.to, after.join(&tr!("、", ", "))));
    }
    out
}

fn with_trace(mut d: Diag, a: &Analysis, c: &Checked, f: &RuleFile, trace: &[Edge], show: &[String]) -> Diag {
    let steps: Vec<Step> = trace.iter().map(|e| step_of(c, f, e)).collect();
    if let Some(last) = steps.last() {
        for r in &last.rows {
            d = d.rowref(r.table.clone(), r.row);
        }
    }
    let w = Witness {
        inputs: steps.last().map(|s| s.inputs.clone()).unwrap_or_default(),
        outputs: steps.last().map(|s| s.outputs.clone()).unwrap_or_default(),
        expected: Vec::new(),
        trace: steps,
        carry: Some(a.carry.clone()),
        more: a.more.iter().map(|it| (it.input.clone(), it.output.clone())).collect(),
    };
    d = d.wit(w);
    for l in trace_lines(a, c, f, trace, show) {
        d = d.note(l);
    }
    d
}

/// An `always` line as it is written, after the word: `残り日数 >=1 <=7 when 猶予中, 督促中`.
pub fn always_text(al: &AlwaysDecl) -> String {
    let tests: Vec<String> = al.tests.iter().map(|(n, cell, _)| format!("{} {}", n.text, cell_text(cell))).collect();
    let mut s = tests.join(" and ");
    if !al.when.is_empty() {
        s.push_str(&format!(" {} {}", crate::kw::WHEN, al.when.iter().map(|n| n.text.clone()).collect::<Vec<_>>().join(", ")));
    }
    s
}

/// The machine's claims as findings.
pub fn check(f: &RuleFile, c: &Checked, path: &str, budget: usize) -> Vec<Diag> {
    let mut out = Vec::new();
    let Some(m) = &f.machine else { return out };
    let at = |line: usize| tr!("{path}:{line} ステートマシン {}", "{path}:{line} machine {}", m.name.text);
    let Some(a) = analyze(f, c, budget) else { return out };
    if let Some(why) = &a.blocked {
        out.push(
            Diag::error("E128", tr!("ステートマシンの検査ができませんでした", "The machine's claims could not be checked"))
                .at(at(m.span.line))
                .mark(m.span.clone(), "")
                .note(why.clone()),
        );
        return out;
    }
    if a.over_budget {
        out.push(
            Diag::error("E128", tr!("ステートマシンの検査の予算を超えました", "The machine's check exceeded its budget"))
                .at(at(m.span.line))
                .key("machine:budget")
                .mark(m.span.clone(), "")
                .note(tr!(
                    "入力の組み合わせは {} 区画で、予算で見られるのは {} 区画までです（`--budget` の {} ノードを一区画 {} ノードで数えます）。",
                    "The inputs cut into {} cells, and the budget covers {} (`--budget` counts {} nodes, {} per cell).",
                    a.cells,
                    budget / NODES_PER_CELL,
                    budget,
                    NODES_PER_CELL
                ))
                .note(tr!(
                    "rulec は、証明できなかった主張を通ったことにはしません。`--budget` を上げるか、状態の遷移を決める表の列を減らしてください。",
                    "A claim that was not proven is never green. Raise `--budget`, or give the table that decides the transitions fewer columns."
                )),
        );
        return out;
    }
    let final_mark = m.final_span.clone().unwrap_or_else(|| m.span.clone());
    let final_line = final_mark.line;
    let finals_text = a.finals.join(", ");

    for (fs, v) in &a.final_claims {
        match v {
            Verdict::Broken(t) => {
                let last_to = t.last().map(|e| e.to.clone()).unwrap_or_default();
                let d = Diag::error("E124", tr!("終わりの状態 {fs} から出る遷移があります", "A final state, {fs}, has a way out"))
                    .at(at(final_line))
                    .key(format!("machine:final:{fs}"))
                    .mark(final_mark.clone(), tr!("{fs} は終わりの状態です", "{fs} is final"));
                let d = with_trace(d, &a, c, f, t, &[]);
                out.push(
                    d.note(tr!(
                        "`final` は案件が終わる状態です。そこから {} へ移る行があると、終わったはずの案件がまた動きます。",
                        "`final` is where a case ends. A row that moves it on to {} sets an ended case going again.",
                        last_to
                    ))
                    .note(tr!(
                        "その行で状態を {fs} に留めるか、{fs} を `final` から外してください。どちらが正しいかは業務の判断です。",
                        "Keep the state at {fs} on that row, or take {fs} off the `final` line. Which is right is the business's to say."
                    )),
                );
            }
            Verdict::Undecided(why) => out.push(undecided_diag(&a, at(final_line), final_mark.clone(), tr!("終わりの状態 {fs} から出る遷移は無い", "nothing leads out of the final state {fs}"), why, format!("machine:undecided:final:{fs}"))),
            Verdict::Holds => {}
        }
    }
    for (s, v) in &a.finish_claims {
        match v {
            Verdict::Broken(t) => {
                let (closure, _) = bfs(s.clone(), |x: &String| {
                    a.edges.iter().enumerate().filter(|(_, e)| &e.from == x && !e.approx).map(|(i, e)| (i, e.to.clone())).collect()
                });
                let d = Diag::error("E125", tr!("状態 {s} に着いた案件は、終わりの状態に着けません", "A case that reaches {s} can never get to a final state"))
                    .at(at(final_line))
                    .key(format!("machine:stuck:{s}"))
                    .mark(
                        final_mark.clone(),
                        if a.finals.len() == 1 {
                            tr!("{finals_text} に着けません", "{finals_text} cannot be reached")
                        } else {
                            tr!("{finals_text} のどれにも着けません", "none of {finals_text} can be reached")
                        },
                    );
                let d = with_trace(d, &a, c, f, t, &[]);
                out.push(
                    d.note(tr!(
                        "{s} から着けるのは {} だけで、どれも終わりの状態ではありません。そこに入った案件は終わることがありません。",
                        "From {s} a case can only get to {}, none of them final. A case that goes there never ends.",
                        closure.join(" / ")
                    ))
                    .note(tr!(
                        "終わりの状態へ移る行を足すか、ここで終わるのが正しいなら `final` に加えてください。",
                        "Add a row that moves on to a final state, or, if a case rightly ends here, add it to `final`."
                    )),
                );
            }
            Verdict::Undecided(why) => out.push(undecided_diag(&a, at(final_line), final_mark.clone(), tr!("状態 {s} から終わりの状態に着ける", "a final state can be reached from {s}"), why, format!("machine:undecided:stuck:{s}"))),
            Verdict::Holds => {}
        }
    }
    if let Some(why) = &a.finish_open {
        out.push(undecided_diag(&a, 
            at(final_line),
            final_mark.clone(),
            tr!("案件が着けるどの状態からも、終わりの状態に着ける", "a final state can be reached from every state a case reaches"),
            why,
            "machine:undecided:finish".into(),
        ));
    }
    for (nv, v) in m.nevers.iter().zip(&a.never_claims) {
        let aw = nv.states.iter().map(|n| n.text.clone()).collect::<Vec<_>>().join(", ");
        let bw = nv.after.iter().map(|n| n.text.clone()).collect::<Vec<_>>().join(", ");
        match v {
            Verdict::Broken(t) => {
                let d = Diag::error("E126", tr!("{bw} のあとに {aw} に着く手順があります", "A sequence of calls reaches {aw} after {bw}"))
                    .at(at(nv.span.line))
                    .key(format!("machine:never:{aw}:{bw}"))
                    .mark(nv.span.clone(), "");
                let d = with_trace(d, &a, c, f, t, &[]);
                out.push(d.note(tr!(
                    "この並びの呼び出しは、どれも規則が受け付けて答えを返します。行き先を決める表の行を直すか、この主張が業務として誤りなら `never` の行を消してください。",
                    "Every call in it is one the rule takes and answers. Correct the rows that decide where it goes, or, if the claim is wrong as business, drop the `never` line."
                )));
            }
            Verdict::Undecided(why) => out.push(undecided_diag(&a, at(nv.span.line), nv.span.clone(), tr!("{bw} のあとに {aw} に着かない", "{aw} is never reached after {bw}"), why, format!("machine:undecided:never:{aw}:{bw}"))),
            Verdict::Holds => {}
        }
    }
    for (on, v) in m.onces.iter().zip(&a.once_claims) {
        let cell = cell_text(&on.cell);
        match v {
            Verdict::Broken(t) => {
                let d = Diag::error("E127", tr!("{} が {cell} になる呼び出しが二回ある手順があります", "A sequence of calls answers {} {cell} twice", on.output.text))
                    .at(at(on.span.line))
                    .key(format!("machine:once:{}:{cell}", on.output.text))
                    .mark(on.span.clone(), "");
                let d = with_trace(d, &a, c, f, t, std::slice::from_ref(&on.output.text));
                out.push(d.note(tr!(
                    "一件の案件で、{} が {cell} になるのは一回までと宣言しています。二回目より前で手順が止まるように表の行を直すか、宣言が誤りなら `once` の行を消してください。",
                    "The line says one case answers {} {cell} at most once. Correct the rows so that the sequence stops short of the second, or drop the `once` line if it is wrong.",
                    on.output.text
                )));
            }
            Verdict::Undecided(why) => out.push(undecided_diag(&a, at(on.span.line), on.span.clone(), tr!("{} が {cell} になる呼び出しは一回まで", "{} is answered {cell} at most once", on.output.text), why, format!("machine:undecided:once:{}:{cell}", on.output.text))),
            Verdict::Holds => {}
        }
    }
    // `always` (§15.217): what the carried values are in every state a case can reach.
    for (al, v) in m.alwayses.iter().zip(&a.always_claims) {
        let text = always_text(al);
        match v {
            Verdict::Broken(t) => {
                // Where the case ends up: the last call's answer, or where it starts.
                let (state, vals): (String, Vec<String>) = match t.last() {
                    Some(e) => (
                        e.to.clone(),
                        al.tests
                            .iter()
                            .filter_map(|(n, _, _)| {
                                let it = a.more.iter().find(|it| it.input == n.text)?;
                                out_val(&e.outputs, &it.output).map(|v| format!("{} = {}", n.text, human(c, &it.output, v)))
                            })
                            .collect(),
                    ),
                    None => (
                        a.initial.clone(),
                        al.tests
                            .iter()
                            .filter_map(|(n, _, _)| a.more.iter().find(|it| it.input == n.text).map(|it| format!("{} = {}", n.text, human(c, &it.input, &it.initial))))
                            .collect(),
                    ),
                };
                let vals = vals.join(&tr!("、", ", "));
                let fits = if al.tests.len() == 1 { "the value fits its cell" } else { "the values fit their cells" };
                let d = Diag::error("E135", tr!("`always {text}` が破れる手順があります", "A sequence of calls breaks `always {text}`"))
                    .at(at(al.span.line))
                    .key(format!("machine:always:{text}"))
                    .mark(al.span.clone(), "");
                let d = with_trace(d, &a, c, f, t, &[]);
                let d = if t.is_empty() {
                    d.note(tr!(
                        "案件は {state} で {vals} から始まり、始まりからこの行に当てはまりません。",
                        "A case starts in {state} with {vals}, which the line already does not accept."
                    ))
                } else if al.when.is_empty() {
                    d.note(tr!(
                        "この並びの最後で、案件は {state} にいて {vals} です。この行は、案件が着けるどの状態でもセルに当てはまると言っています。",
                        "At the end of this sequence the case is in {state} with {vals}. The line says {fits} in every state a case can reach."
                    ))
                } else {
                    let named = al.when.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(&tr!("・", ", "));
                    d.note(tr!(
                        "この並びの最後で、案件は {state} にいて {vals} です。この行は、案件が {named} にいるときはいつもセルに当てはまると言っています。",
                        "At the end of this sequence the case is in {state} with {vals}. The line says {fits} whenever a case is in {named}."
                    ))
                };
                out.push(d.note(tr!(
                    "行き先や次の値を決める表の行を直すか、主張が業務として誤りなら `always` の行を直してください。",
                    "Correct the rows that decide where a case goes and what it carries on, or the `always` line if the claim is wrong as business."
                )));
            }
            Verdict::Undecided(why) => out.push(undecided_diag(&a, at(al.span.line), al.span.clone(), format!("{} {text}", crate::kw::ALWAYS), why, format!("machine:undecided:always:{text}"))),
            Verdict::Holds => {}
        }
    }
    // A carried number's next value outside its input's range (§15.217): there is no next call
    // to pass it to.
    for (input, v) in &a.range_claims {
        let Some(it) = a.more.iter().find(|it| it.input == *input) else { continue };
        let line = m.carries.iter().find(|(i, _, _)| i.text == *input).map(|(_, _, sp)| sp.clone()).unwrap_or_else(|| m.span.clone());
        let range = match it.range {
            Some((lo, hi)) => format!(">={} <={}", crate::types::fmt_val(lo, &it.ty), crate::types::fmt_val(hi, &it.ty)),
            None => String::new(),
        };
        match v {
            Verdict::Broken(t) => {
                let value = t.last().and_then(|e| out_val(&e.outputs, &it.output)).map(|v| human(c, &it.output, v)).unwrap_or_default();
                let d = Diag::error("E134", tr!("{} が {} の範囲を出る呼び出しがあります", "A call answers {} outside the range of {}", it.output, it.input))
                    .at(at(line.line))
                    .key(format!("machine:range:{}", it.input))
                    .mark(line.clone(), tr!("{} の範囲は {range} です", "the range of {} is {range}", it.input));
                let d = with_trace(d, &a, c, f, t, &[]);
                out.push(
                    d.note(tr!(
                        "{} は {value} になりえます。{} の範囲は {range} なので、次の呼び出しにこの値を渡せません。",
                        "{} can come to {value}. The range of {} is {range}, so the next call cannot be passed it.",
                        it.output,
                        it.input
                    ))
                    .note(tr!(
                        "次の値を決める表の行を直すか、その値が業務として起きるなら {} の範囲を広げてください。",
                        "Correct the row that decides the next value, or, if the value does occur in the business, widen the range of {}.",
                        it.input
                    )),
                );
            }
            Verdict::Undecided(why) => out.push(undecided_diag(&a, 
                at(line.line),
                line.clone(),
                tr!("{} は {} の範囲を出ない", "{} stays within the range of {}", it.output, it.input),
                why,
                format!("machine:undecided:range:{}", it.input),
            )),
            Verdict::Holds => {}
        }
    }
    if a.exact {
        for s in &a.states {
            if !a.reachable.contains(s) {
                out.push(
                    Diag::warning("W125", tr!("状態 {s} には、どの手順でも着きません", "No sequence of calls reaches state {s}"))
                        .at(at(m.span.line))
                        .key(format!("machine:unreachable:{s}"))
                        .mark(m.span.clone(), tr!("{} から始まる手順", "the calls that start at {}", a.initial))
                        .note(tr!(
                            "{} から着ける状態は {} です。{s} の行は、ほかの版から移ってきた案件のために残しているのでなければ、書き忘れた遷移があるはずです。",
                            "From {} a case reaches {}. Unless the rows for {s} are kept for cases moved over from another version, a transition into it is missing.",
                            a.initial,
                            a.reachable.join(" / ")
                        )),
                );
            }
        }
        for (t, r) in &a.unused {
            let row = f.items.iter().find_map(|it| match it {
                Item::Table(x) if x.name.as_ref().is_some_and(|n| &n.text == t) => x.rows.iter().find(|y| y.index == *r),
                _ => None,
            });
            let sp = row.map(|y| y.span.clone()).unwrap_or_else(|| m.span.clone());
            // The state the row applies in, where a case does get to it.
            let in_reached = a
                .edges
                .iter()
                .find(|e| e.row.as_ref() == Some(&(t.clone(), *r)) && a.reachable.contains(&e.from))
                .map(|e| e.from.clone());
            out.push(
                Diag::warning("W126", tr!("表 {t} 行{r} の遷移は、案件が着ける状態からは使われません", "The transition in table {t} row {r} is never taken from a state a case can reach"))
                    .at(tr!("{path}:{} 表 {t}", "{path}:{} table {t}", sp.line))
                    .key(format!("machine:unused:{t}:{r}"))
                    .rowref(t.clone(), *r)
                    .mark(sp, "")
                    .note(match in_reached {
                        // A case reaches the state, only never with the further carried values
                        // the row needs (§15.217).
                        Some(st) if !a.more.is_empty() => {
                            let names: Vec<String> = a.more.iter().map(|it| it.input.clone()).collect();
                            let had: Vec<String> = (0..a.more.len())
                                .map(|k| {
                                    let cs: BTreeSet<usize> = a.nodes.iter().filter(|n| n.0 == st).map(|n| n.2[k]).collect();
                                    format!("{} {}", a.more[k].input, a.values_text(k, &cs))
                                })
                                .collect();
                            tr!(
                                "この行が当てはまる状態 {st} には着きますが、この行が要る {} の値では着きません。{st} に着くときは {} です。",
                                "A case does reach {st}, the state this row applies in, but never with the {} this row needs. It reaches {st} with {}.",
                                names.join(", "),
                                had.join(tr!("、", "; ").as_str())
                            )
                        }
                        // The state is reached, only never with the values of the held inputs
                        // this row needs (§15.149).
                        Some(_) => tr!(
                            "この行が当てはまる状態には着けますが、この行が要る {} の値のままでは着きません。`held` の入力は、一つの案件のあいだ変わりません。",
                            "A case does reach the state this row applies in, but never with the value of {} this row needs: a `held` input does not change within a case.",
                            a.held.join(", ")
                        ),
                        None => tr!(
                            "この行が当てはまるのは、{} から始まる手順では着かない状態のときだけです（着けるのは {}）。",
                            "This row applies only in states that no sequence of calls from {} reaches (it reaches {}).",
                            a.initial,
                            a.reachable.join(" / ")
                        ),
                    }),
            );
        }
    } else {
        let open: Vec<String> = a.reachable.iter().filter(|s| a.unsettled_at(s)).cloned().collect();
        out.push(undecided_diag(&a, 
            at(m.span.line),
            m.span.clone(),
            tr!("どの状態に着けるか", "which states a case can reach"),
            &tr!(
                "状態 {} からの呼び出しに、答えを出す入力を作れず、起こらないとも示せなかった区画があります",
                "calls from state {} include cells no input was built for and none shown impossible",
                open.join(" / ")
            ),
            "machine:undecided:reach".into(),
        ));
    }
    out
}

fn undecided_diag(a: &Analysis, at: String, mark: Span, claim: String, why: &str, key: String) -> Diag {
    // What leaves a claim unsettled, and so what to do about it: a cell no input was built for;
    // carried numbers walked in intervals, where a path is not shown to be a case's (§15.217);
    // or a carried number whose next value moves with another input inside a cell, where the
    // values between the ones the calls answered were not each tried.
    let cells = why.contains(&tr!("答えを出す入力を作れず", "include cells no input was built for"));
    let tail = if a.more.is_empty() || cells {
        tr!(
            "rulec は、決めきれなかったことを成り立つとは言いません。導出どうしが入力を共有していると、ここが残ります（`diff` の決めきれない区画と同じ場所です）。",
            "What was not settled is not said to hold. Derived values that share an input leave this behind — the same blind spot as `diff`'s unsettled cells."
        )
    } else if !a.whole {
        tr!(
            "rulec は、決めきれなかったことを成り立つとは言いません。持ち越す数の値を一つずつ調べても予算に収まるなら、そう調べて決めます。`--budget` を上げるか、範囲を狭めてください。",
            "What was not settled is not said to hold. Where every value of the carried numbers fits the budget one by one, they are walked that way and the claim is settled: raise `--budget`, or narrow their ranges."
        )
    } else {
        tr!(
            "rulec は、決めきれなかったことを成り立つとは言いません。持ち越す数の次の値が、一つの区画の中でほかの入力とともに動くと、ここが残ります。見た呼び出しの答えのあいだの値は、一つずつは試していません。その手順が本当にあるなら、手順の例に書いてください。",
            "What was not settled is not said to hold. A carried number whose next value moves with another input inside one cell leaves this behind: the values between the ones the calls seen answered were not each tried. If the sequence does happen, write it down as a scenario."
        )
    };
    Diag::warning("W127", tr!("ステートマシンの主張を決めきれませんでした: {claim}", "The machine's claim could not be settled: {claim}"))
        .at(at)
        .key(key)
        .mark(mark, "")
        .note(why.to_string())
        .note(tail)
}

// ---------------------------------------------------------------------------------------
// Scenarios
// ---------------------------------------------------------------------------------------

/// One scenario run through the reference evaluator: each row one call, the carried input the
/// state the call before it answered (`initial` for the first).
pub struct Run {
    /// Per call: the inputs, the answer, the rows that fired.
    pub calls: Vec<(HashMap<String, Val>, Vec<(String, Option<Val>)>, Vec<(String, usize)>)>,
}

/// Run a scenario's calls, as far as they go. Stops at a call the rule cannot answer.
pub fn run_scenario(f: &RuleFile, c: &Checked, sc: &ScenarioDecl) -> Option<Run> {
    let m = f.machine.as_ref()?;
    let (cin, cout) = m.carried()?;
    let mut state = Val::Enum(m.initial.as_ref()?.0.text.clone());
    // The further carried values (§15.217), each from where it starts.
    let more = more_starts(f, c);
    let mut held: Vec<Val> = more.iter().map(|(_, _, v)| v.clone()).collect();
    let mut calls = Vec::new();
    for row in &sc.table.rows {
        let mut env = crate::eval::example_env(f, c, &sc.table, row);
        env.insert(cin.to_string(), state.clone());
        for ((i, _, _), v) in more.iter().zip(&held) {
            env.insert(i.clone(), v.clone());
        }
        let (outs, _, rows, _) = crate::eval::run_all_traced(f, c, env.clone());
        let next = out_val(&outs, cout).cloned();
        let next_more: Option<Vec<Val>> = more.iter().map(|(_, o, _)| out_val(&outs, o).cloned()).collect();
        calls.push((env, outs, rows));
        match (next, next_more) {
            (Some(v), Some(nm)) => {
                state = v;
                held = nm;
            }
            _ => break,
        }
    }
    Some(Run { calls })
}

/// E111, E019 and E107 for the scenarios: every output has a column, every call is one the
/// rule can be sent, and every call answers what the row says.
pub fn check_scenarios(f: &RuleFile, c: &Checked, path: &str) -> Vec<Diag> {
    let mut out = Vec::new();
    let Some(m) = &f.machine else { return out };
    let Some((cin, cout)) = m.carried() else { return out };
    for sc in &f.scenarios {
        let at = |line: usize| tr!("{path}:{line} 手順の例 {}", "{path}:{line} scenario {}", sc.name.text);
        let t = &sc.table;
        let mut shaped = true;
        for od in &f.outputs {
            if !t.outputs.iter().any(|o| o.name.text == od.name.text) {
                shaped = false;
                out.push(
                    Diag::error("E111", tr!("手順の例 {} に出力 {} の列がありません", "Scenario {} has no column for output {}", sc.name.text, od.name.text))
                        .at(at(sc.span.line))
                        .fix(crate::diag::FixKind::AddExpected, &od.name.text)
                        .mark(sc.span.clone(), tr!("{} の期待値がありません", "no expected value for {}", od.name.text))
                        .note(tr!(
                            "手順の例も、実装どうしを照らし合わせても見つからない誤りを見つけるためのものなので、出力は全部書いてください。持ち越す出力 {} の列は、次の呼び出しが始まる状態です。",
                            "A scenario, like an example, is there to catch what the implementations can share, so every output is written. The column for the carried output {} is the state the next call starts in.",
                            cout
                        )),
                );
            }
        }
        for row in &t.rows {
            if row.outs.len() < t.outputs.len() {
                shaped = false;
                out.push(
                    Diag::error("E111", tr!("手順の例 {} の期待値が {} 列足りません", "Scenario {} is missing {} expected-value column(s)", sc.name.text, t.outputs.len() - row.outs.len()))
                        .at(at(row.span.line))
                        .mark(row.span.clone(), ""),
                );
            }
        }
        if !shaped {
            continue;
        }
        let mut state = Val::Enum(m.initial.as_ref().map(|(v, _)| v.text.clone()).unwrap_or_default());
        // The further carried values (§15.217), each from where it starts.
        let more = more_starts(f, c);
        let mut carried: Vec<Val> = more.iter().map(|(_, _, v)| v.clone()).collect();
        let more_names: Vec<String> = more.iter().map(|(i, _, _)| i.clone()).collect();
        let mut first_fixed: Option<Vec<(String, Option<Val>)>> = None;
        let mut done: Vec<Step> = Vec::new();
        let mut lines: Vec<String> = Vec::new();
        for (k, row) in t.rows.iter().enumerate() {
            let mut env = crate::eval::example_env(f, c, t, row);
            env.insert(cin.to_string(), state.clone());
            for ((i, _, _), v) in more.iter().zip(&carried) {
                env.insert(i.clone(), v.clone());
            }
            let args: Vec<String> = f
                .inputs
                .iter()
                .filter(|i| i.name.text != cin && !more_names.contains(&i.name.text))
                .filter_map(|i| env.get(&i.name.text).map(|v| format!("{} = {}", i.name.text, human(c, &i.name.text, v))))
                .collect();
            let now: Vec<String> = more.iter().zip(&carried).map(|((i, _, _), v)| format!("{i} = {}", human(c, i, v))).collect();
            let shown_state = if now.is_empty() { vectors_show(&state) } else { tr!("{}（{}）", "{} ({})", vectors_show(&state), now.join(", ")) };
            let a: BTreeMap<String, Val> = env.clone().into_iter().collect();
            // A case holds its `held` inputs (§15.149): a scenario is one case.
            let fixed_now: Vec<(String, Option<Val>)> = m.held.iter().map(|n| (n.text.clone(), a.get(&n.text).cloned())).collect();
            match &first_fixed {
                None => first_fixed = Some(fixed_now),
                Some(was) if *was != fixed_now => {
                    let changed: Vec<String> =
                        was.iter().zip(fixed_now.iter()).filter(|(x, y)| x != y).map(|(x, _)| x.0.clone()).collect();
                    out.push(
                        Diag::error(
                            "E055",
                            tr!(
                                "手順の例 {} の {} 回目の呼び出しで、`held` の {} が変わっています",
                                "Call {1} of scenario {0} changes {2}, which the machine holds (`held`)",
                                sc.name.text,
                                k + 1,
                                changed.join(", ")
                            ),
                        )
                        .at(at(row.span.line))
                        .mark(row.span.clone(), "")
                        .note(tr!(
                            "手順の例は一つの案件です。`held` の入力は、最初の呼び出しから最後の呼び出しまで同じ値で渡されます。",
                            "A scenario is one case, and a `held` input is passed with the same value from its first call to its last."
                        )),
                    );
                    break;
                }
                Some(_) => {}
            }
            if !crate::vectors::allowed(f, &a) {
                out.push(
                    Diag::error("E019", tr!("手順の例 {} の {} 回目の呼び出しが制約を破っています", "Call {1} of scenario {0} breaks a constraint", sc.name.text, k + 1))
                        .at(at(row.span.line))
                        .mark(row.span.clone(), "")
                        .note(tr!(
                            "制約は「この組み合わせは起きない」という宣言で、生成コードはその組み合わせを入口で受け付けません。手順の例の値を直すか、その組み合わせが本当に起きるなら制約のほうを消してください。",
                            "A constraint says the combination does not happen, and the generated code refuses it at the door. Correct the scenario's values, or drop the constraint if the combination really does happen."
                        )),
                );
                break;
            }
            let (got, _, rows, _) = crate::eval::run_all_traced(f, c, env.clone());
            let e = Edge {
                from: vectors_show(&state),
                to: out_val(&got, cout).map(vectors_show).unwrap_or_default(),
                flags: Vec::new(),
                row: None,
                rows: rows.clone(),
                inputs: a.clone(),
                outputs: got.clone(),
                world: Vec::new(),
                from_carry: Vec::new(),
                to_carry: Vec::new(),
                approx: false,
                cell: Vec::new(),
            };
            done.push(step_of(c, f, &e));
            let head = match (args.is_empty(), crate::i18n::ja()) {
                (true, _) => shown_state.clone(),
                (false, true) => format!("{} のとき {}", shown_state, args.join(", ")),
                (false, false) => format!("{}, {}", shown_state, args.join(", ")),
            };
            // The row of the transition table that answered, as the claims' traces name it.
            let over = m.over.as_ref().map(|o| o.text.as_str()).unwrap_or_default();
            let to = out_val(&got, cout).map(vectors_show).unwrap_or_else(|| "-".into());
            lines.push(match rows.iter().find(|(t2, _)| t2 == over) {
                Some((t2, r2)) => tr!("  {}. {} → {}（{}）", "  {}. at {} → {} ({})", k + 1, head, to, crate::eval::row_tag(t2, *r2)),
                None => tr!("  {}. {} → {}", "  {}. at {} → {}", k + 1, head, to),
            });
            let mut broke = false;
            for od in &f.outputs {
                let Some(hi) = t.outputs.iter().position(|o| o.name.text == od.name.text) else { continue };
                let oty = c.ty_of(&od.name.text).unwrap_or(Ty::Unknown);
                let want = row.outs.get(hi).and_then(|o| match o {
                    OutCell::Lit(l) => crate::eval::lit_to_val(l, &oty),
                    OutCell::Name(w) => crate::eval::lit_to_val(&Lit::Word(w.clone()), &oty),
                    // An expected `none`: the output has no value after this call (§15.249).
                    OutCell::Nothing => Some(crate::eval::Val::Enum(crate::kw::NONE.into())),
                });
                let g = out_val(&got, &od.name.text).cloned();
                let title = match (&want, &g) {
                    (Some(w), Some(g)) if w == g => continue,
                    (Some(w), Some(g)) => tr!(
                        "手順の例 {} の {} 回目の呼び出しが合いません: {} は {} のはずが {} になりました",
                        "Call {1} of scenario {0} does not hold: {2} should be {3} but came out as {4}",
                        sc.name.text,
                        k + 1,
                        od.name.text,
                        w.show(&oty),
                        g.show(&oty)
                    ),
                    (Some(w), None) => tr!(
                        "手順の例 {} の {} 回目の呼び出しが合いません: {} は {} のはずが、値が出ませんでした",
                        "Call {1} of scenario {0} does not hold: {2} should be {3} but no value came out",
                        sc.name.text,
                        k + 1,
                        od.name.text,
                        w.show(&oty)
                    ),
                    _ => continue,
                };
                let mut d = Diag::error("E107", title).at(at(row.span.line)).mark(row.span.clone(), "");
                for (t2, r2) in &rows {
                    d = d.rowref(t2.clone(), *r2);
                }
                let mut w = Witness {
                    inputs: done.last().map(|s| s.inputs.clone()).unwrap_or_default(),
                    outputs: g.as_ref().map(|g| vec![(od.name.text.clone(), crate::eval::wval(c, &od.name.text, g))]).unwrap_or_default(),
                    expected: want.as_ref().map(|w| vec![(od.name.text.clone(), crate::eval::wval(c, &od.name.text, w))]).unwrap_or_default(),
                    trace: done.clone(),
                    carry: Some((cin.to_string(), cout.to_string())),
                    more: more.iter().map(|(i, o, _)| (i.clone(), o.clone())).collect(),
                };
                w.trace = done.clone();
                d = d.wit(w);
                d = d.note(tr!("手順（{} から）:", "Calls (from {}):", m.initial.as_ref().map(|(v, _)| v.text.clone()).unwrap_or_default()));
                for l in &lines {
                    d = d.note(l.clone());
                }
                out.push(d.note(tr!(
                    "手順の例は実行される仕様です。表を直すか、手順の例のほうが間違っているなら手順の例を直してください。",
                    "A scenario is executable specification. Fix the table, or fix the scenario if the scenario is what is wrong."
                )));
                broke = true;
            }
            if broke {
                break;
            }
            let next_more: Option<Vec<Val>> = more.iter().map(|(_, o, _)| out_val(&got, o).cloned()).collect();
            match (out_val(&got, cout), next_more) {
                (Some(v), Some(nm)) => {
                    state = v.clone();
                    carried = nm;
                }
                _ => break,
            }
        }
    }
    out
}

fn vectors_show(v: &Val) -> String {
    crate::vectors::show(v)
}

/// The cell as it is written, for a message.
pub fn cell_text(c: &Cell) -> String {
    match c {
        Cell::Cmp(_) | Cell::Ranked { .. } | Cell::Prefix(_) | Cell::Not(_) | Cell::DontCare | Cell::Nothing => crate::ast::cell_text(c),
        Cell::Lit(_) | Cell::Set(_) => crate::ast::cell_text(c).trim_start_matches("= ").to_string(),
    }
}
