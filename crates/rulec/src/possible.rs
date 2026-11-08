//! `possible` (§15.287): what is to happen, and the shortest calls that make it happen.
//!
//! `never`, `once` and `always` say that something bad does not happen, and a table that does nothing
//! says all of them. A `possible` line is the far side: it says what does happen, and the check shows
//! it. It holds only where calls that really happen were found, each one the door takes; it is E146
//! only where everything that can happen was looked at and it was not there; and anything between is
//! not settled, W127 under a `machine` and W135 outside one. It says nothing to the generated code.
//!
//! Under a `machine` the search is `machine::settle_possible`'s. Here are the words of the lines, the
//! lines outside a `machine`, which are about one call, and what the views read.

use crate::ast::*;
use crate::diag::{Diag, Witness};
use crate::eval::Val;
use crate::machine::{Edge, NODES_PER_CELL};
use crate::num::Rat;
use crate::types::{Checked, Ty};
use crate::vdiff::{Axis, Coord, Settled, Walker};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// How one claim came out.
#[derive(Debug, Clone)]
pub enum Possibly {
    /// It happens: the shortest calls found. No call for a case that starts there; one for a line
    /// outside a `machine`.
    Shown(Vec<Edge>),
    /// It does not, whatever the calls, and what is there instead.
    Absent(String),
    /// Not settled either way, and why.
    Undecided(String),
}

/// One claim: a line outside a `machine`, or one pair of a line under it.
#[derive(Debug, Clone)]
pub struct Claim {
    /// Which `possible` line of the `machine` it is; `None` outside one.
    pub decl: Option<usize>,
    /// The line of the file.
    pub line: usize,
    /// The claim as it reads: one state and one `after`, however long the lists are.
    pub said: String,
    pub verdict: Possibly,
}

/// The pairs a `possible` line asks for, each on its own, the way a `never` line's lists divide: the
/// claim as it reads, the state it is to reach, the state it comes after, and the state it speaks of.
pub(crate) fn pairs(p: &PossibleDecl) -> Vec<(String, Option<&str>, Option<&str>, Option<&str>)> {
    let kw_p = crate::kw::POSSIBLE;
    let (a, w) = (crate::kw::AFTER, crate::kw::WHEN);
    let mut out = Vec::new();
    if p.tests.is_empty() {
        for s in &p.states {
            if p.after.is_empty() {
                out.push((format!("{kw_p} {}", s.text), Some(s.text.as_str()), None, None));
            }
            for b in &p.after {
                out.push((format!("{kw_p} {} {a} {}", s.text, b.text), Some(s.text.as_str()), Some(b.text.as_str()), None));
            }
        }
        return out;
    }
    let tests = p.tests_said();
    if !p.when.is_empty() {
        for s in &p.when {
            out.push((format!("{kw_p} {tests} {w} {}", s.text), None, None, Some(s.text.as_str())));
        }
    } else if !p.after.is_empty() {
        for b in &p.after {
            out.push((format!("{kw_p} {tests} {a} {}", b.text), None, Some(b.text.as_str()), None));
        }
    } else {
        out.push((format!("{kw_p} {tests}"), None, None, None));
    }
    out
}

/// The claims of a `machine`, before anything is walked: not settled. What cannot be walked stays so.
pub(crate) fn machine_claims(m: &MachineDecl) -> Vec<Claim> {
    let why = tr!("ステートマシンの入力の区画を調べきれていません", "the machine's inputs' cells were not all walked");
    let mut out = Vec::new();
    for (li, p) in m.possibles.iter().enumerate() {
        for (said, ..) in pairs(p) {
            out.push(Claim { decl: Some(li), line: p.span.line, said, verdict: Possibly::Undecided(why.clone()) });
        }
    }
    out
}

/// One call, as an edge: what the views read of a trace, with no state to go from.
fn call_edge(inputs: BTreeMap<String, Val>, outputs: Vec<(String, Option<Val>)>, rows: Vec<(String, usize)>) -> Edge {
    Edge {
        from: String::new(),
        to: String::new(),
        flags: Vec::new(),
        row: rows.first().cloned(),
        rows,
        inputs,
        outputs,
        world: Vec::new(),
        from_carry: Vec::new(),
        to_carry: Vec::new(),
        approx: false,
        cell: Vec::new(),
    }
}

/// The value a call has for a name: what it passes, what it answers, or what is worked out on the way.
fn value_of<'v>(name: &str, inputs: &'v HashMap<String, Val>, outs: &'v [(String, Option<Val>)], binds: &'v HashMap<String, Val>) -> Option<&'v Val> {
    inputs.get(name).or_else(|| crate::machine::out_val(outs, name)).or_else(|| binds.get(name))
}

/// Whether one call has every condition of a line.
fn call_holds(c: &Checked, n: &PossibleLine, inputs: &HashMap<String, Val>, outs: &[(String, Option<Val>)], binds: &HashMap<String, Val>) -> bool {
    n.conds.iter().all(|k| {
        let ty = c.ty_of(&k.name.text).unwrap_or(Ty::Unknown);
        value_of(&k.name.text, inputs, outs, binds).is_some_and(|v| crate::eval::cell_matches(c, &k.cell, v, &ty))
    })
}

/// How far a line got over the walk.
#[derive(Default)]
struct Seen {
    shown: Option<Edge>,
    /// Some cell could not be settled either way.
    open: bool,
    /// The calls that had the line's conditions on inputs, and what they answered for its outputs.
    met: bool,
    answers: BTreeMap<String, BTreeSet<String>>,
}

/// What a name can be over a cell, read off the coordinates with no call to run: one value, or
/// every value between two ends on its grid (an end it runs on from is `None`).
enum Reach {
    Is(Val),
    Between(Option<Rat>, Option<Rat>),
}

/// Whether a row can fire anywhere in the cell. A coordinate rules a row out only where one value
/// of it answers for all: a point, a word or a truth value, or an interval tested by a comparison,
/// which the axes are cut at. A string class is a prefix, and one string does not stand for it.
fn may_fire(c: &Checked, axes: &[Axis], cell: &[usize], t: &Table, r: &Row) -> bool {
    t.inputs.iter().enumerate().all(|(ci, (col, _))| {
        let Some(test) = r.cells.get(ci) else { return true };
        let Some(ai) = axes.iter().position(|a| a.col == *col) else { return true };
        let ax = &axes[ai];
        let decides = match &ax.coords[cell[ai]] {
            Coord::Point(_) => true,
            Coord::Word(_) => *ax.ty.present() != Ty::Str,
            Coord::Open(..) => matches!(test, Cell::Cmp(_) | Cell::Lit(_) | Cell::DontCare),
            Coord::OtherStr => false,
        };
        !decides || crate::vdiff::coord_val(ax, cell[ai]).is_none_or(|v| crate::eval::cell_matches(c, test, &v, &ax.ty))
    })
}

/// Every value `name` can take over the cell: off its coordinate when it has an axis, otherwise off
/// each row that can fire there, followed through the tables in between. `false` when that cannot
/// be read without a call: a name an expression works out and no axis follows, a string class, a
/// date between two ends.
fn reaches(f: &RuleFile, c: &Checked, axes: &[Axis], cell: &[usize], name: &str, depth: usize, out: &mut Vec<Reach>) -> bool {
    if depth > 16 {
        return false;
    }
    if let Some(ai) = axes.iter().position(|a| a.col == name) {
        let ax = &axes[ai];
        return match &ax.coords[cell[ai]] {
            Coord::Word(_) if *ax.ty.present() == Ty::Str => false,
            Coord::Point(_) | Coord::Word(_) => match crate::vdiff::coord_val(ax, cell[ai]) {
                Some(v) => {
                    out.push(Reach::Is(v));
                    true
                }
                None => false,
            },
            co @ Coord::Open(..) if !ax.date => {
                let (lo, hi) = crate::vdiff::ival(co, ax.step);
                out.push(Reach::Between(lo, hi));
                true
            }
            _ => false,
        };
    }
    // The first output of a `result` line is its expression.
    if let (Some(r), Some(first)) = (&f.result, f.outputs.first()) {
        if first.name.text == name {
            return match &r.expr {
                Expr::Name(m, _) => reaches(f, c, axes, cell, m, depth + 1, out),
                _ => false,
            };
        }
    }
    let computed = f.items.iter().any(|it| match it {
        Item::Derived(d) => d.name.text == name,
        Item::Define(d) => d.name.text == name,
        _ => false,
    });
    if computed || f.inputs.iter().any(|i| i.name.text == name) {
        return false;
    }
    let Some(set) = c.sets.iter().find(|s| s.table.outputs.iter().any(|o| o.name.text == name)) else { return false };
    let Some(oi) = set.table.outputs.iter().position(|o| o.name.text == name) else { return false };
    let ty = c.ty_of(name).unwrap_or(Ty::Unknown);
    let word = |w: &Lit, out: &mut Vec<Reach>| crate::eval::lit_to_val(w, &ty).map(|v| out.push(Reach::Is(v))).is_some();
    for r in set.table.rows.iter().filter(|r| may_fire(c, axes, cell, &set.table, r)) {
        let read = match r.outs.get(oi) {
            Some(OutCell::Lit(l)) => word(l, out),
            Some(OutCell::Nothing) => {
                out.push(Reach::Is(Val::Enum(crate::kw::NONE.into())));
                true
            }
            // A name the rule knows is read; any other word is the value it spells (§3.2).
            Some(OutCell::Name(m)) if c.ty_of(m).is_some() => reaches(f, c, axes, cell, m, depth + 1, out),
            Some(OutCell::Name(m)) => word(&Lit::Word(m.clone()), out),
            None => false,
        };
        if !read {
            return false;
        }
    }
    true
}

/// What the output's own rounding makes of a value that reaches it, as the evaluator applies it
/// last (§7.2).
fn rounded(f: &RuleFile, c: &Checked, output: &str, x: Rat) -> Rat {
    use crate::num::RoundTo;
    let Some(rd) = f.outputs.iter().find(|o| o.name.text == output).and_then(|o| o.rounding.as_ref()) else { return x };
    let ty = c.ty_of(output).unwrap_or(Ty::Unknown);
    match (crate::types::lit_value_in_pub(&rd.grid, &ty), crate::num::RoundMode::parse(&rd.mode)) {
        (Some(g), Some(m)) => x.round_to(m, g),
        _ => x,
    }
}

/// Whether no call of the cell has `output` meet the test, read off the rows that can fire there
/// (§15.291). A cell no input was built for, and that was not shown empty either, leaves a line
/// open only when its answer could meet the line there.
fn output_outside(f: &RuleFile, c: &Checked, axes: &[Axis], cell: &[usize], output: &str, tested: &Cell) -> bool {
    let Some(ty) = c.ty_of(output) else { return false };
    let mut reach = Vec::new();
    if !reaches(f, c, axes, cell, output, 0, &mut reach) {
        return false;
    }
    let bounds = crate::machine::cell_bounds(tested, &ty);
    let written = match tested {
        Cell::Cmp(ops) => ops.len(),
        Cell::Lit(_) => 1,
        Cell::Set(ls) | Cell::Not(ls) => ls.len(),
        _ => usize::MAX,
    };
    reach.iter().all(|r| match r {
        Reach::Is(Val::Num(x)) => !crate::eval::cell_matches(c, tested, &Val::Num(rounded(f, c, output, *x)), &ty),
        Reach::Is(v) => !crate::eval::cell_matches(c, tested, v, &ty),
        // The answers between two ends are the rounding of each value, and rounding keeps their
        // order. With no boundary of the test among them, the test answers alike for all.
        Reach::Between(lo, hi) => {
            if bounds.len() != written {
                return false;
            }
            let (lo, hi) = (lo.map(|x| rounded(f, c, output, x)), hi.map(|x| rounded(f, c, output, x)));
            let among = |b: &Rat| lo.is_none_or(|l| b.cmp_to(l) != Ordering::Less) && hi.is_none_or(|h| b.cmp_to(h) != Ordering::Greater);
            match lo.or(hi) {
                Some(at) if !bounds.iter().any(among) => !crate::eval::cell_matches(c, tested, &Val::Num(at), &ty),
                _ => false,
            }
        }
    })
}

/// The lines outside a `machine` (§15.287): a walk over the rule's own cells, each settled by an input
/// the door takes. `Err` is why the walk could not be made: the rule folds a sequence, or its cells
/// are past the budget.
fn walk_lines(f: &RuleFile, c: &Checked, budget: usize) -> Result<Vec<Seen>, String> {
    let mut extra: Vec<(String, Rat)> = Vec::new();
    let mut more: BTreeSet<String> = BTreeSet::new();
    for n in &f.possibles {
        for k in &n.conds {
            if f.outputs.iter().any(|o| o.name.text == k.name.text) {
                crate::machine::output_cuts(f, c, &k.name.text, &k.cell, &mut extra, &mut more);
            } else if f.is_summary(&k.name.text) {
                more.insert(k.name.text.clone());
            }
        }
    }
    let mut walker = Walker::new(f, c, &extra, &more)?;
    let cap = (budget / NODES_PER_CELL).max(1) as u128;
    if walker.size() > cap {
        return Err(tr!(
            "入力の組み合わせは {} 区画で、予算で見られるのは {} 区画までです",
            "the inputs cut into {} cells, and the budget covers {}",
            walker.size(),
            cap
        ));
    }
    let axes = walker.axes.clone();
    let mut seen: Vec<Seen> = f.possibles.iter().map(|_| Seen::default()).collect();
    let lens: Vec<usize> = axes.iter().map(|x| x.len()).collect();
    let mut cell = vec![0usize; axes.len()];
    let is_output = |name: &str| f.outputs.iter().any(|o| o.name.text == name);
    'cells: loop {
        let settled = walker.settle(&cell);
        for (li, n) in f.possibles.iter().enumerate() {
            if seen[li].shown.is_some() {
                continue;
            }
            match &settled {
                Settled::Empty => {}
                Settled::Unknown => {
                    // A coordinate wholly outside a condition says the cell is not this line's, and
                    // so does an output no row that can fire here brings inside it (§15.291).
                    let out = n.conds.iter().any(|k| match axes.iter().position(|a| a.col == k.name.text) {
                        Some(ai) => crate::vdiff::coord_val(&axes[ai], cell[ai]).is_some_and(|v| !crate::eval::cell_matches(c, &k.cell, &v, &axes[ai].ty)),
                        None => is_output(&k.name.text) && output_outside(f, c, &axes, &cell, &k.name.text, &k.cell),
                    });
                    seen[li].open |= !out;
                }
                Settled::Realized(inputs, ans) => {
                    let takes = |k: &NeverCond| -> Option<bool> {
                        let ty = c.ty_of(&k.name.text).unwrap_or(Ty::Unknown);
                        value_of(&k.name.text, inputs, &ans.outs, &ans.binds).map(|v| crate::eval::cell_matches(c, &k.cell, v, &ty))
                    };
                    let on_output = |k: &NeverCond| is_output(&k.name.text);
                    // The conditions on what the call passes are settled by the coordinates, which
                    // are cut where they test.
                    if n.conds.iter().filter(|k| !on_output(k)).any(|k| takes(k) != Some(true)) {
                        let plain_string = n.conds.iter().filter(|k| !on_output(k)).any(|k| {
                            takes(k) != Some(true) && c.ty_of(&k.name.text).is_some_and(|t| *t.present() == Ty::Str) && !matches!(k.cell, Cell::Prefix(_))
                        });
                        seen[li].open |= plain_string;
                        continue;
                    }
                    seen[li].met = true;
                    for k in n.conds.iter().filter(|k| on_output(k)) {
                        if let Some(v) = crate::machine::out_val(&ans.outs, &k.name.text) {
                            seen[li].answers.entry(k.name.text.clone()).or_default().insert(crate::machine::human(c, &k.name.text, v));
                        }
                    }
                    let as_map = |m: &HashMap<String, Val>| -> BTreeMap<String, Val> { m.iter().map(|(a, b)| (a.clone(), b.clone())).collect() };
                    if call_holds(c, n, inputs, &ans.outs, &ans.binds) && crate::vectors::allowed(f, c, &as_map(inputs)) {
                        seen[li].shown = Some(call_edge(as_map(inputs), ans.outs.clone(), ans.rows.clone()));
                        continue;
                    }
                    // Not this call. An output that is one value over the cell, or that lies on one
                    // side of the cell's boundaries, says it of every call here.
                    let closed = n.conds.iter().filter(|k| on_output(k)).any(|k| {
                        takes(k) != Some(true)
                            && crate::machine::flag_settled(f, c, &axes, &cell, &ans.rows, &k.name.text, &k.cell)
                            && !crate::machine::reads_a_string(f, c, &k.name.text)
                    });
                    if closed {
                        continue;
                    }
                    let others = walker.others(&cell, inputs);
                    let hit = others.iter().find(|(p, a)| call_holds(c, n, p, &a.outs, &a.binds) && crate::vectors::allowed(f, c, &as_map(p)));
                    match hit {
                        Some((p, a)) => seen[li].shown = Some(call_edge(as_map(p), a.outs.clone(), a.rows.clone())),
                        None => seen[li].open = true,
                    }
                }
            }
        }
        if seen.iter().all(|s| s.shown.is_some()) {
            break;
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
    Ok(seen)
}

/// What the walk does see of a line it did not find (E146).
fn absent_text(n: &PossibleLine, s: &Seen) -> String {
    if !s.met {
        return tr!(
            "この入力の組み合わせで呼び出すことはできません。入力の範囲か `constraint` か `never` の行が外しています。",
            "No call passes these inputs: the ranges of the inputs, a `constraint` or a `never` line leave them out."
        );
    }
    let parts: Vec<String> = n
        .conds
        .iter()
        .filter_map(|k| s.answers.get(&k.name.text).map(|vs| (k, vs)))
        .map(|(k, vs)| {
            let list = if vs.len() <= 6 { vs.iter().cloned().collect::<Vec<_>>().join(" / ") } else { tr!("{} 通りの値", "{} different values", vs.len()) };
            tr!("{} が答えるのは {list} です", "{} answers {list}", k.name.text)
        })
        .collect();
    tr!("入力の条件に当てはまる呼び出しで、{}。", "On the calls that meet the line's inputs, {}.", parts.join(&tr!("、", "; ")))
}

/// The lines outside a `machine`, each settled.
pub fn one_call_claims(f: &RuleFile, c: &Checked, budget: usize) -> Vec<Claim> {
    if f.possibles.is_empty() {
        return Vec::new();
    }
    let claim = |n: &PossibleLine, verdict: Possibly| Claim { decl: None, line: n.span.line, said: n.said(), verdict };
    match walk_lines(f, c, budget) {
        Ok(seen) => f
            .possibles
            .iter()
            .zip(seen)
            .map(|(n, s)| {
                let verdict = match (&s.shown, s.open) {
                    (Some(e), _) => Possibly::Shown(vec![e.clone()]),
                    (None, true) => Possibly::Undecided(tr!(
                        "答えを出す入力を作れず、起こらないとも示せなかった区画があります",
                        "some cells had no input built for them and were not shown impossible"
                    )),
                    (None, false) => Possibly::Absent(absent_text(n, &s)),
                };
                claim(n, verdict)
            })
            .collect(),
        // The space does not cut into cells, or not within the budget: a call found among the inputs
        // the vectors draw from still shows the line. Not finding one shows nothing.
        Err(why) => {
            let inputs = crate::vectors::plain_inputs(f, c);
            f.possibles
                .iter()
                .map(|n| {
                    let hit = inputs.iter().find_map(|a| {
                        let env: HashMap<String, Val> = a.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                        let an = crate::vdiff::run(f, c, env.clone());
                        call_holds(c, n, &env, &an.outs, &an.binds).then(|| call_edge(a.clone(), an.outs, an.rows))
                    });
                    claim(n, match hit {
                        Some(e) => Possibly::Shown(vec![e]),
                        None => Possibly::Undecided(why.clone()),
                    })
                })
                .collect()
        }
    }
}

/// The lines outside a `machine`, as findings: E146 where nothing has them, W135 where that is not
/// settled.
pub fn check(f: &RuleFile, c: &Checked, path: &str, budget: usize) -> Vec<Diag> {
    let mut out = Vec::new();
    for (n, cl) in f.possibles.iter().zip(one_call_claims(f, c, budget)) {
        let said = &cl.said;
        let at = tr!("{path}:{} 起きること", "{path}:{} possible", n.span.line);
        match &cl.verdict {
            Possibly::Shown(_) => {}
            Possibly::Absent(what) => out.push(
                Diag::error("E146", tr!("`{said}` になる呼び出しはありません", "No call makes `{said}` happen"))
                    .at(at)
                    .key(format!("possible:{said}"))
                    .mark(n.span.clone(), "")
                    .note(what.clone())
                    .note(tr!(
                        "入力の区画を全部調べました。答えを決める表の行を直すか、起きなくてよいことなら `possible` の行を消してください。",
                        "Every cell of the inputs was looked at. Correct the rows that decide the answer, or drop the `possible` line if this need not happen."
                    )),
            ),
            Possibly::Undecided(why) => out.push(
                Diag::warning("W135", tr!("`{said}` を決めきれませんでした", "Whether `{said}` happens could not be settled"))
                    .at(at)
                    .key(format!("possible:undecided:{said}"))
                    .mark(n.span.clone(), "")
                    .note(why.clone())
                    .note(tr!(
                        "rulec は、決めきれなかったことを成り立つとは言いません。`--budget` を上げるか、その呼び出しを `examples` に書いてください。",
                        "What was not settled is not said to hold. Raise `--budget`, or write that call down in `examples`."
                    )),
            ),
        }
    }
    out
}

/// Everything a rule's `possible` lines come to, in the order of the file, and the walk of its
/// `machine` when it has one: what the views read.
pub struct Results {
    pub analysis: Option<crate::machine::Analysis>,
    pub claims: Vec<Claim>,
}

/// The `possible` lines of a rule that has any. `None` for one that has none.
pub fn results(f: &RuleFile, c: &Checked, budget: usize) -> Option<Results> {
    let under = f.machine.as_ref().is_some_and(|m| !m.possibles.is_empty());
    if !under && f.possibles.is_empty() {
        return None;
    }
    let analysis = if under { crate::machine::analyze(f, c, budget) } else { None };
    let mut claims: Vec<Claim> = match (&analysis, &f.machine) {
        (Some(a), _) => a.possible_claims.clone(),
        (None, Some(m)) => machine_claims(m),
        (None, None) => Vec::new(),
    };
    claims.extend(one_call_claims(f, c, budget));
    claims.sort_by_key(|cl| cl.line);
    Some(Results { analysis, claims })
}

/// [`results`], from a file's source: the rule and what its lines come to. `None` for a file that does
/// not get as far, and for a file with no `possible` line.
pub fn of_source(src: &str, path: &str, budget: i64) -> Option<(RuleFile, Checked, Results)> {
    let (f, c) = crate::prepare(src, path).ok()?;
    let res = results(&f, &c, budget.max(0) as usize)?;
    Some((f, c, res))
}

/// `rulec check --show-possible`: each claim and the calls that show it.
pub fn report_text(f: &RuleFile, c: &Checked, r: &Results, path: &str) -> String {
    let mut out = String::new();
    for cl in &r.claims {
        out.push_str(&format!("{path}:{} {}\n", cl.line, cl.said));
        match &cl.verdict {
            Possibly::Shown(t) => {
                for l in shown_lines(f, c, r, cl, t) {
                    out.push_str(&format!("  {l}\n"));
                }
            }
            Possibly::Absent(_) => out.push_str(&format!("  {}\n", tr!("起きません（E146）", "does not happen (E146)"))),
            Possibly::Undecided(_) => {
                let code = undecided_code(cl);
                out.push_str(&format!("  {}\n", tr!("決めきれません（{code}）", "not settled ({code})")));
            }
        }
    }
    out
}

/// The warning a claim that is not settled comes as.
pub fn undecided_code(cl: &Claim) -> &'static str {
    if cl.decl.is_some() {
        "W127"
    } else {
        "W135"
    }
}

/// `rulec check --format json`: one more line, for a file that has `possible` lines.
pub fn report_json(f: &RuleFile, c: &Checked, r: &Results, path: &str) -> String {
    let claims: Vec<String> = r.claims.iter().map(|cl| claim_json(f, c, cl)).collect();
    crate::json::Obj::new().int("v", 2).str("kind", "possible").str("file", path).raw("possible", crate::json::arr(&claims)).finish()
}

/// Why a claim that is shown or not comes in these words: `holds`, `absent` or `undecided`.
pub fn result_word(v: &Possibly) -> &'static str {
    match v {
        Possibly::Shown(_) => "holds",
        Possibly::Absent(_) => "absent",
        Possibly::Undecided(_) => "undecided",
    }
}

/// One call as a person reads it: `tier = gold, tools = 1 → fee = 3USD, approved = true (table t row 2)`.
pub fn call_line(f: &RuleFile, c: &Checked, e: &Edge) -> String {
    let args: Vec<String> = f.inputs.iter().filter_map(|i| e.inputs.get(&i.name.text).map(|v| format!("{} = {}", i.name.text, crate::machine::human(c, &i.name.text, v)))).collect();
    let answers: Vec<String> = e.outputs.iter().filter_map(|(k, v)| v.as_ref().map(|v| format!("{k} = {}", crate::machine::human(c, k, v)))).collect();
    format!("{} → {}", args.join(", "), answers.join(", "))
}

/// The lines that show one claim: the calls of a trace, as E126 has them, or the one call of a line
/// outside a `machine`.
pub fn shown_lines(f: &RuleFile, c: &Checked, r: &Results, cl: &Claim, t: &[Edge]) -> Vec<String> {
    match (cl.decl, &r.analysis) {
        (Some(_), Some(a)) if t.is_empty() => vec![tr!("案件はここから始まります（{}）。呼び出しは要りません。", "A case starts here ({}), with no call needed.", a.initial)],
        (Some(k), Some(a)) => {
            // The output the line is about is shown beside each call.
            let show: Vec<String> = f.machine.as_ref().and_then(|m| m.possibles.get(k)).map(|p| p.tests.iter().filter(|(n, _, _)| f.outputs.iter().any(|o| o.name.text == n.text)).map(|(n, _, _)| n.text.clone()).collect()).unwrap_or_default();
            crate::machine::trace_lines(a, c, f, t, &show)
        }
        _ => t.iter().map(|e| call_line(f, c, e)).collect(),
    }
}

/// The JSON of one claim (`rulec check --format json`): `witness` in the shape a finding's has.
pub fn claim_json(f: &RuleFile, c: &Checked, cl: &Claim) -> String {
    let mut o = crate::json::Obj::new().int("line", cl.line as i128).str("claim", &cl.said).str("result", result_word(&cl.verdict));
    match &cl.verdict {
        Possibly::Shown(t) => {
            let steps: Vec<crate::diag::Step> = t.iter().map(|e| crate::machine::step_of(c, f, e)).collect();
            let w = if cl.decl.is_some() {
                Witness {
                    inputs: steps.last().map(|s| s.inputs.clone()).unwrap_or_default(),
                    outputs: steps.last().map(|s| s.outputs.clone()).unwrap_or_default(),
                    trace: steps,
                    ..Witness::default()
                }
            } else {
                Witness {
                    inputs: steps.first().map(|s| s.inputs.clone()).unwrap_or_default(),
                    outputs: steps.first().map(|s| s.outputs.clone()).unwrap_or_default(),
                    ..Witness::default()
                }
            };
            o = o.raw("witness", crate::diag::witness_json(&w));
        }
        Possibly::Absent(why) | Possibly::Undecided(why) => o = o.str("why", why),
    }
    o.finish()
}

/// `rulec api`: the lines under a `machine`, each in the form it is written, one key of `states`,
/// `output` or `tests` telling which.
pub(crate) fn machine_api(f: &RuleFile, m: &MachineDecl) -> String {
    let names = |v: &[Name]| crate::json::strs(&v.iter().map(|n| n.text.clone()).collect::<Vec<_>>());
    let lines: Vec<String> = m
        .possibles
        .iter()
        .map(|p| {
            let mut o = crate::json::Obj::new().str("said", &p.said());
            match p.tests.as_slice() {
                [] => o = o.raw("states", names(&p.states)).raw("after", names(&p.after)),
                [(n, cell, _)] if f.outputs.iter().any(|x| x.name.text == n.text) && !m.more().iter().any(|(i, _)| *i == n.text) => {
                    o = o.str("output", &n.text).str("cell", &crate::machine::cell_text(cell)).raw("after", names(&p.after));
                }
                ts => {
                    let tests: Vec<String> = ts.iter().map(|(n, cell, _)| crate::json::Obj::new().str("input", &n.text).str("cell", &crate::machine::cell_text(cell)).finish()).collect();
                    o = o.raw("tests", crate::json::arr(&tests)).raw("when", names(&p.when));
                }
            }
            o.finish()
        })
        .collect();
    crate::json::arr(&lines)
}

/// `rulec api`: the lines outside a `machine`, as a `never` line's are said.
pub(crate) fn lines_api(f: &RuleFile) -> String {
    let lines: Vec<String> = f
        .possibles
        .iter()
        .map(|n| {
            let conds: Vec<String> = n.conds.iter().map(|k| crate::json::Obj::new().str("name", &k.name.text).str("cell", &crate::ast::cell_text(&k.cell)).finish()).collect();
            crate::json::Obj::new().str("said", &n.said()).raw("conds", crate::json::arr(&conds)).finish()
        })
        .collect();
    crate::json::arr(&lines)
}

/// `rulec certificate`: the lines outside a `machine`, each with the call that shows it. The inputs and
/// answers are in the shape the vectors use.
pub(crate) fn certificate_json(f: &RuleFile, c: &Checked) -> String {
    let lines: Vec<String> = one_call_claims(f, c, crate::region::DEFAULT_BUDGET as usize)
        .iter()
        .map(|cl| {
            let mut o = crate::json::Obj::new().int("line", cl.line as i128).str("said", &cl.said).str("result", result_word(&cl.verdict));
            if let Possibly::Shown(t) = &cl.verdict {
                if let Some(e) = t.first() {
                    let st = crate::machine::step_of(c, f, e);
                    let w = Witness { inputs: st.inputs, outputs: st.outputs, ..Witness::default() };
                    o = o.raw("witness", crate::diag::witness_json(&w));
                }
            }
            o.finish()
        })
        .collect();
    crate::json::arr(&lines)
}

/// The inputs that show the lines outside a `machine`, for the vectors: each with the line it shows.
pub(crate) fn one_call_inputs(f: &RuleFile, c: &Checked) -> Vec<(String, BTreeMap<String, Val>)> {
    one_call_claims(f, c, crate::region::DEFAULT_BUDGET as usize)
        .into_iter()
        .filter_map(|cl| match cl.verdict {
            Possibly::Shown(t) => t.first().map(|e| (cl.said.clone(), e.inputs.clone())),
            _ => None,
        })
        .collect()
}
