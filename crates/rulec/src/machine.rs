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

use crate::ast::*;
use crate::diag::{Diag, RowRef, Span, Step, WVal, Witness};
use crate::eval::Val;
use crate::num::Rat;
use crate::types::{Checked, Ty};
use crate::vdiff::{Axis, Coord, Settled, Walker};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

/// How many region nodes of `--budget` one cell of the machine's walk counts as. A cell is
/// settled by running the rule, which costs about what fifty nodes of the region check do, so
/// the default budget of the check lets the walk visit a million cells — the point where
/// `diff` stops feeling like a gate (§15.122).
pub const NODES_PER_CELL: usize = 50;

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
}

/// A state together with the world a case is in: where the searches walk.
pub type Node = (String, Vec<usize>);

/// How a claim came out.
#[derive(Debug, Clone)]
pub enum Verdict {
    Holds,
    /// Broken, by this sequence of calls (edge indices, from the initial state).
    Broken(Vec<usize>),
    /// Not settled either way, and why.
    Undecided(String),
}

impl Verdict {
    pub fn holds(&self) -> bool {
        matches!(self, Verdict::Holds)
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
    /// State → how many cells of calls from it could not be settled.
    pub unknown: BTreeMap<String, usize>,
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
    /// The (state, world) pairs a case can reach, in the order a breadth-first search meets
    /// them, and how each was first reached.
    pub nodes: Vec<Node>,
    pub node_parent: HashMap<Node, (Node, usize)>,
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
        self.nodes.iter().any(|(x, y)| x == s && y.as_slice() == w)
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

    /// The (state, world) pairs from which a final state can still be reached.
    pub fn can_finish(&self) -> std::collections::HashSet<Node> {
        let mut can: std::collections::HashSet<Node> = std::collections::HashSet::new();
        let worlds: BTreeSet<&Vec<usize>> = self.edges.iter().map(|e| &e.world).chain(self.nodes.iter().map(|n| &n.1)).collect();
        for fs in &self.finals {
            for w in &worlds {
                can.insert((fs.clone(), (*w).clone()));
            }
        }
        loop {
            let before = can.len();
            for e in &self.edges {
                if can.contains(&(e.to.clone(), e.world.clone())) {
                    can.insert((e.from.clone(), e.world.clone()));
                }
            }
            if can.len() == before {
                break;
            }
        }
        can
    }

    /// Edges out of one state.
    pub fn out_of<'b>(&'b self, s: &'b str) -> impl Iterator<Item = (usize, &'b Edge)> + 'b {
        self.edges.iter().enumerate().filter(move |(_, e)| e.from == s)
    }

    /// The transitions a reader sees: one per (from, row, to), the flags merged.
    pub fn transitions(&self) -> Vec<(String, Option<(String, usize)>, String)> {
        let mut out: Vec<(String, Option<(String, usize)>, String)> = Vec::new();
        for e in &self.edges {
            let k = (e.from.clone(), e.row.clone(), e.to.clone());
            if !out.contains(&k) {
                out.push(k);
            }
        }
        out
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
            Item::Agg(_) => {}
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
            if (&k.left == x || &k.right == x) && !out.contains(x) {
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
    };
    let (extra, more) = once_cuts(f, c, m);
    let mut walker = match Walker::new(f, c, &extra, &more) {
        Ok(w) => w,
        Err(why) => {
            a.blocked = Some(why);
            return Some(a);
        }
    };
    a.cells = walker.size();
    if a.cells > (budget / NODES_PER_CELL).max(1) as u128 {
        a.over_budget = true;
        return Some(a);
    }
    let axes = walker.axes.clone();
    let Some(carry_ax) = axes.iter().position(|ax| ax.col == cin) else {
        a.blocked = Some(tr!("持ち越す入力 {} に軸がありません", "the carried input {} has no axis", cin));
        return Some(a);
    };
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
    let decided_by = |rows: &[(String, usize)]| -> Option<(String, usize)> { rows.iter().find(|(t, _)| deciders.contains(t)).cloned() };
    // The value each world's calls are made with: the first call's. A later call of the world
    // is made again with it, and kept that way when it answers the same — which, when no
    // computed column reads the held input, it always does. Then every sequence of one
    // world's calls is a sequence one case can make, held inputs and all.
    let mut rep: HashMap<Vec<usize>, Vec<(String, Val)>> = HashMap::new();
    let fixed_names = a.held.clone();
    let mut pin = |e: Edge| -> (Edge, bool) {
        if fixed_names.is_empty() {
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
            && e.flags.iter().zip(fl.iter()).all(|(x, y)| *x == Flag::Maybe || x == y);
        if !same {
            return (e, true);
        }
        (Edge { inputs, outputs: an.outs, rows: an.rows, ..e }, false)
    };
    let mut index: HashMap<(String, Option<(String, usize)>, String, Vec<Flag>, Vec<usize>), usize> = HashMap::new();
    let mut add = |a: &mut Analysis, e: Edge| {
        let (e, mixed) = pin(e);
        let k = (e.from.clone(), e.row.clone(), e.to.clone(), e.flags.clone(), e.world.clone());
        match index.get(&k) {
            None => {
                index.insert(k, a.edges.len());
                if mixed {
                    a.mixed.insert(a.edges.len());
                }
                a.edges.push(e);
            }
            // A call that stays in its world's value is the better witness of the same edge.
            Some(&i) if a.mixed.contains(&i) && !mixed => {
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
        match walker.settle(&cell) {
            Settled::Realized(inputs, ans) => {
                let next = next_of(&ans.outs);
                let next_ok = next.is_some()
                    && crate::vdiff::names_constant(f, c, &axes, &cell, &ans.rows, &[cout.to_string()]);
                let settled_flags: Vec<bool> =
                    m.onces.iter().map(|o| flag_settled(f, c, &axes, &cell, &ans.rows, o)).collect();
                let edge = |inp: &HashMap<String, Val>, an: &crate::vdiff::Answer, flags: Vec<Flag>, to: String| Edge {
                    from: s.clone(),
                    to,
                    flags,
                    row: decided_by(&an.rows),
                    rows: an.rows.clone(),
                    inputs: inp.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
                    outputs: an.outs.clone(),
                    world: world.clone(),
                };
                if next_ok && settled_flags.iter().all(|b| *b) {
                    let fl = flags_of(&ans.outs);
                    let e = edge(&inputs, &ans, fl, next.unwrap_or_default());
                    add(&mut a, e);
                } else {
                    // Not one answer over the cell, or not shown to be. Ask the cell's other
                    // ends; whatever they answer is a transition that really happens.
                    let mut seen: Vec<(HashMap<String, Val>, crate::vdiff::Answer)> = vec![(inputs.clone(), ans)];
                    seen.extend(walker.others(&cell, &inputs));
                    let mut observed_flags: Vec<BTreeSet<Flag>> = vec![BTreeSet::new(); m.onces.len()];
                    for (inp, an) in &seen {
                        let Some(to) = next_of(&an.outs) else { continue };
                        let fl = flags_of(&an.outs);
                        for (k, x) in fl.iter().enumerate() {
                            observed_flags[k].insert(*x);
                        }
                        let e = edge(inp, an, fl, to);
                        add(&mut a, e);
                    }
                    if !next_ok {
                        // Another next state may hide in the cell.
                        *a.unknown.entry(s.clone()).or_default() += 1;
                    } else {
                        let open: Vec<usize> = (0..m.onces.len()).filter(|&k| !settled_flags[k]).collect();
                        let covered = open.len() == 1 && observed_flags[open[0]].len() == 2;
                        if !covered {
                            let (inp, an) = &seen[0];
                            let mut fl = flags_of(&an.outs);
                            for &k in &open {
                                fl[k] = Flag::Maybe;
                            }
                            let e = edge(inp, an, fl, next.clone().unwrap_or_default());
                            add(&mut a, e);
                        }
                    }
                }
            }
            Settled::Empty => {}
            Settled::Unknown => *a.unknown.entry(s).or_default() += 1,
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
    // A case's calls stay in one world, so the searches walk (state, world).
    let succ = |n: &Node| -> Vec<(usize, Node)> {
        edges.iter().enumerate().filter(|(_, e)| e.from == n.0 && e.world == n.1).map(|(i, e)| (i, (e.to.clone(), n.1.clone()))).collect()
    };
    // The worlds a case can start in: those with a call from the initial state.
    let mut starts: Vec<Node> = Vec::new();
    for e in &edges {
        let n = (a.initial.clone(), e.world.clone());
        if e.from == a.initial && !starts.contains(&n) {
            starts.push(n);
        }
    }
    if starts.is_empty() {
        starts.push((a.initial.clone(), vec![0; a.held.len()]));
    }
    let (order, parent) = bfs_from(starts.clone(), &succ);
    a.nodes = order.clone();
    a.node_parent = parent;
    let mut states: Vec<String> = Vec::new();
    for (st, _) in &order {
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
    a.exact = !a.reachable.iter().any(|s| a.unknown.get(s).copied().unwrap_or(0) > 0);
    let unsettled = |a: &Analysis, set: &mut dyn Iterator<Item = String>| -> Vec<String> {
        let mut v: Vec<String> = set.filter(|s| a.unknown.get(s).copied().unwrap_or(0) > 0).collect();
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
                "見つかった手順は、途中で {} を変えないと通れません。一つの値のまま通れるかは、たどっていません",
                "the sequence found needs {} to change partway, and whether one value carries it is not followed",
                fixed_text
            ))
        } else {
            Verdict::Broken(t)
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
                _ if a.unknown.get(fs).copied().unwrap_or(0) > 0 => Verdict::Undecided(undecided(std::slice::from_ref(fs))),
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
                let open = unsettled(a, &mut closure.into_iter().map(|n| n.0));
                let here = if open.is_empty() { broken(a, a.trace_to_node(n).unwrap_or_default()) } else { Verdict::Undecided(undecided(&open)) };
                match (&v, &here) {
                    (Verdict::Broken(_), _) => {}
                    (_, Verdict::Broken(_)) | (Verdict::Holds, _) => v = here,
                    _ => {}
                }
            }
            if v.holds() && !a.coupled.is_empty() {
                v = Verdict::Undecided(tr!(
                    "{} は、ほかの入力と一緒に計算される値にも使われています。一つの値のまま終わりに着けるかは、たどっていません",
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
                let open = unsettled(a, &mut order.iter().map(|(n, _)| n.0.clone()));
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
                let open = unsettled(a, &mut loose.iter().map(|(n, _)| n.0.clone()));
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
        let taken: BTreeSet<(String, usize)> =
            a.edges.iter().filter(|e| a.reaches(&e.from, &e.world)).filter_map(|e| e.row.clone()).collect();
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
    if let Some(el) = &f.elements {
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

/// The lines of prose that walk a reader through a trace.
fn trace_lines(a: &Analysis, c: &Checked, f: &RuleFile, trace: &[usize], show: &[String]) -> Vec<String> {
    let mut out = vec![tr!("手順（{} から）:", "Calls (from {}):", a.initial)];
    for (k, &ei) in trace.iter().enumerate() {
        let e = &a.edges[ei];
        let args: Vec<String> = f
            .inputs
            .iter()
            .filter(|i| i.name.text != a.carry.0)
            .filter_map(|i| e.inputs.get(&i.name.text).map(|v| format!("{} = {}", i.name.text, human(c, &i.name.text, v))))
            .collect();
        let mut after: Vec<String> = show
            .iter()
            .filter_map(|o| out_val(&e.outputs, o).map(|v| format!("{o} = {}", human(c, o, v))))
            .collect();
        if let Some((t, r)) = &e.row {
            after.push(crate::eval::row_tag(t, *r));
        }
        let head = match (args.is_empty(), crate::i18n::ja()) {
            (true, _) => e.from.clone(),
            (false, true) => format!("{} のとき {}", e.from, args.join(", ")),
            (false, false) => format!("{}, {}", e.from, args.join(", ")),
        };
        out.push(tr!("  {}. {} → {}（{}）", "  {}. at {} → {} ({})", k + 1, head, e.to, after.join(&tr!("、", ", "))));
    }
    out
}

fn with_trace(mut d: Diag, a: &Analysis, c: &Checked, f: &RuleFile, trace: &[usize], show: &[String]) -> Diag {
    let steps: Vec<Step> = trace.iter().map(|&e| step_of(c, f, &a.edges[e])).collect();
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
    };
    d = d.wit(w);
    for l in trace_lines(a, c, f, trace, show) {
        d = d.note(l);
    }
    d
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
                    "証明できなかった主張を緑にはしません。`--budget` を上げるか、状態の遷移を決める表の列を減らしてください。",
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
                let last = &a.edges[*t.last().unwrap_or(&0)];
                let d = Diag::error("E124", tr!("終わりの状態 {fs} から出る遷移があります", "A final state, {fs}, has a way out"))
                    .at(at(final_line))
                    .key(format!("machine:final:{fs}"))
                    .mark(final_mark.clone(), tr!("{fs} は終わりの状態です", "{fs} is final"));
                let d = with_trace(d, &a, c, f, t, &[]);
                out.push(
                    d.note(tr!(
                        "`final` は案件が終わる状態です。そこから {} へ移る行があると、終わったはずの案件がまた動きます。",
                        "`final` is where a case ends. A row that moves it on to {} sets an ended case going again.",
                        last.to
                    ))
                    .note(tr!(
                        "その行で状態を {fs} に留めるか、{fs} を `final` から外してください。どちらが正しいかは業務の判断です。",
                        "Keep the state at {fs} on that row, or take {fs} off the `final` line. Which is right is the business's to say."
                    )),
                );
            }
            Verdict::Undecided(why) => out.push(undecided_diag(at(final_line), final_mark.clone(), tr!("終わりの状態 {fs} から出る遷移は無い", "nothing leads out of the final state {fs}"), why, format!("machine:undecided:final:{fs}"))),
            Verdict::Holds => {}
        }
    }
    for (s, v) in &a.finish_claims {
        match v {
            Verdict::Broken(t) => {
                let (closure, _) = bfs(s.clone(), |x: &String| {
                    a.edges.iter().enumerate().filter(|(_, e)| &e.from == x).map(|(i, e)| (i, e.to.clone())).collect()
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
            Verdict::Undecided(why) => out.push(undecided_diag(at(final_line), final_mark.clone(), tr!("状態 {s} から終わりの状態に着ける", "a final state can be reached from {s}"), why, format!("machine:undecided:stuck:{s}"))),
            Verdict::Holds => {}
        }
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
            Verdict::Undecided(why) => out.push(undecided_diag(at(nv.span.line), nv.span.clone(), tr!("{bw} のあとに {aw} に着かない", "{aw} is never reached after {bw}"), why, format!("machine:undecided:never:{aw}:{bw}"))),
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
            Verdict::Undecided(why) => out.push(undecided_diag(at(on.span.line), on.span.clone(), tr!("{} が {cell} になる呼び出しは一回まで", "{} is answered {cell} at most once", on.output.text), why, format!("machine:undecided:once:{}:{cell}", on.output.text))),
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
            out.push(
                Diag::warning("W126", tr!("表 {t} 行{r} の遷移は、案件が着ける状態からは使われません", "The transition in table {t} row {r} is never taken from a state a case can reach"))
                    .at(tr!("{path}:{} 表 {t}", "{path}:{} table {t}", sp.line))
                    .key(format!("machine:unused:{t}:{r}"))
                    .rowref(t.clone(), *r)
                    .mark(sp, "")
                    .note(
                        // The state is reached, only never with the values of the held inputs
                        // this row needs (§15.149).
                        if a.edges.iter().any(|e| e.row.as_ref() == Some(&(t.clone(), *r)) && a.reachable.contains(&e.from)) {
                            tr!(
                                "この行が当てはまる状態には着けますが、この行が要る {} の値のままでは着きません。`held` の入力は、一つの案件のあいだ変わりません。",
                                "A case does reach the state this row applies in, but never with the value of {} this row needs: a `held` input does not change within a case.",
                                a.held.join(", ")
                            )
                        } else {
                            tr!(
                                "この行が当てはまるのは、{} から始まる手順では着かない状態のときだけです（着けるのは {}）。",
                                "This row applies only in states that no sequence of calls from {} reaches (it reaches {}).",
                                a.initial,
                                a.reachable.join(" / ")
                            )
                        },
                    ),
            );
        }
    } else {
        let open: Vec<String> = a.reachable.iter().filter(|s| a.unknown.get(*s).copied().unwrap_or(0) > 0).cloned().collect();
        out.push(undecided_diag(
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

fn undecided_diag(at: String, mark: Span, claim: String, why: &str, key: String) -> Diag {
    Diag::warning("W127", tr!("ステートマシンの主張を決めきれませんでした: {claim}", "The machine's claim could not be settled: {claim}"))
        .at(at)
        .key(key)
        .mark(mark, "")
        .note(why.to_string())
        .note(tr!(
            "決めきれなかったことを、成り立つとは言いません。導出どうしが入力を共有していると、ここが残ります（`diff` の決めきれない区画と同じ場所です）。",
            "What was not settled is not said to hold. Derived values that share an input leave this behind — the same blind spot as `diff`'s unsettled cells."
        ))
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
    let mut calls = Vec::new();
    for row in &sc.table.rows {
        let mut env = crate::eval::example_env(f, c, &sc.table, row);
        env.insert(cin.to_string(), state.clone());
        let (outs, _, rows, _) = crate::eval::run_all_traced(f, c, env.clone());
        let next = out_val(&outs, cout).cloned();
        calls.push((env, outs, rows));
        match next {
            Some(v) => state = v,
            None => break,
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
                            "手順の例も、実装どうしの照合では捕まらない誤りを捕まえるためのものなので、出力は全部書きます。持ち越す出力 {} の列は、次の呼び出しが始まる状態です。",
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
        let mut first_fixed: Option<Vec<(String, Option<Val>)>> = None;
        let mut done: Vec<Step> = Vec::new();
        let mut lines: Vec<String> = Vec::new();
        for (k, row) in t.rows.iter().enumerate() {
            let mut env = crate::eval::example_env(f, c, t, row);
            env.insert(cin.to_string(), state.clone());
            let args: Vec<String> = f
                .inputs
                .iter()
                .filter(|i| i.name.text != cin)
                .filter_map(|i| env.get(&i.name.text).map(|v| format!("{} = {}", i.name.text, human(c, &i.name.text, v))))
                .collect();
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
                            "制約は「この組み合わせは起きない」という宣言で、生成コードは入口で断ります。手順の例の値を直すか、その組み合わせが本当に起きるなら制約のほうを消してください。",
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
            };
            done.push(step_of(c, f, &e));
            let head = match (args.is_empty(), crate::i18n::ja()) {
                (true, _) => vectors_show(&state),
                (false, true) => format!("{} のとき {}", vectors_show(&state), args.join(", ")),
                (false, false) => format!("{}, {}", vectors_show(&state), args.join(", ")),
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
            match out_val(&got, cout) {
                Some(v) => state = v.clone(),
                None => break,
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
        Cell::Cmp(_) | Cell::Prefix(_) | Cell::Not(_) | Cell::DontCare | Cell::Nothing => crate::ast::cell_text(c),
        Cell::Lit(_) | Cell::Set(_) => crate::ast::cell_text(c).trim_start_matches("= ").to_string(),
    }
}
