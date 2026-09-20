//! The reference evaluator (§0).
//!
//! The semantics have exactly one source of truth, and it is this file. Checking the
//! `examples` section, computing the expected values of test vectors, and replaying past
//! records all go through this same evaluator. Only the generated code runs in production;
//! this evaluator appears only on the verification side.

use crate::ast::*;
use crate::diag::Diag;
use crate::num::{Rat, RoundMode};
use crate::types::{Checked, Ty, lit_value_in_pub};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Val {
    Enum(String),
    /// A value in the declared unit: yen for money, the declared unit for a mass.
    Num(Rat),
    Bool(bool),
    Str(String),
    Date(i32, u32, u32),
    /// The sequence a `fold` walks: one map of field name to value per element (§15.56).
    /// It never appears in a cell, an expression or a witness — only as the value of the
    /// name `elements` declared, on the wire and in the evaluator's environment.
    Seq(Vec<std::collections::BTreeMap<String, Val>>),
}

impl Val {
    fn show(&self, ty: &Ty) -> String {
        match self {
            Val::Enum(s) | Val::Str(s) => s.clone(),
            Val::Bool(b) => if *b { crate::kw::TRUE } else { crate::kw::FALSE }.into(),
            Val::Date(y, m, d) => format!("{y:04}-{m:02}-{d:02}"),
            Val::Num(r) => match ty {
                Ty::Money { cur, .. } => format!("{r}{cur}"),
                Ty::Qty { unit, .. } => format!("{r}{unit}"),
                Ty::Rate => format!("{}%", r.mul(Rat::int(100))),
                _ => format!("{r}"),
            },
            // A sequence is never spelled into a cell or a message; how many there are is
            // the only thing about it a reader of prose ever needs (§15.56).
            Val::Seq(xs) => tr!("{} 件", "{} elements", xs.len()),
        }
    }
}

pub struct Env<'a> {
    pub vals: HashMap<String, Val>,
    pub c: &'a Checked,
    /// The rows that fired. E107 of §11 is reported "with the fired rows".
    pub fired: Vec<String>,
    /// The same rows as `(table, row)`, for the structured half of a diagnostic. Kept beside
    /// the tags rather than parsed back out of them: `row_tag` is prose, and prose is allowed
    /// to change (§11 principle 5).
    pub fired_rows: Vec<(String, usize)>,
}

impl<'a> Env<'a> {
    pub fn new(c: &'a Checked, vals: HashMap<String, Val>) -> Env<'a> {
        Env { vals, c, fired: Vec::new(), fired_rows: Vec::new() }
    }
}

/// A value as it goes into a diagnostic's witness: an integer in the canonical unit, an enum
/// value by name, a boolean, a date as `YYYY-MM-DD` (§10.2). A rate travels as a count of
/// steps, so the conversion needs the name it is declared under.
pub fn wval(c: &crate::types::Checked, name: &str, v: &Val) -> crate::diag::WVal {
    use crate::diag::WVal;
    match v {
        Val::Enum(s) | Val::Str(s) => WVal::Str(s.clone()),
        Val::Bool(b) => WVal::Bool(*b),
        Val::Date(y, m, d) => WVal::Str(format!("{y:04}-{m:02}-{d:02}")),
        Val::Num(r) => WVal::Int(crate::types::wire_int(*r, c.wire_scale(name))),
        // A witness names one input at a time, and an element's fields are named on their
        // own; the sequence as a whole never stands in one.
        Val::Seq(xs) => WVal::Str(tr!("{} 件", "{} elements", xs.len())),
    }
}

/// The tag of a fired row, `表 名前 行N`. E107 prints it, and `coverage`, `vectors`, and
/// `replay` match against it as a key, so it is built in exactly one place.
pub fn row_tag(table: &str, row: usize) -> String {
    tr!("表 {table} 行{row}", "table {table} row {row}")
}

pub fn lit_to_val(l: &Lit, ty: &Ty) -> Option<Val> {
    Some(match l {
        Lit::Num(n) => Val::Num(lit_value_in_pub(n, ty)?),
        Lit::Word(w) => {
            if w == crate::kw::TRUE {
                Val::Bool(true)
            } else if w == crate::kw::FALSE {
                Val::Bool(false)
            } else {
                Val::Enum(w.clone())
            }
        }
        Lit::Str(s) => Val::Str(s.clone()),
        Lit::Date(y, m, d) => Val::Date(*y, *m, *d),
    })
}

impl<'a> Env<'a> {
    /// Whether a cell matches a value. Corresponds one-to-one to the seven unary tests of §3.
    pub fn matches_pub(&self, cell: &Cell, v: &Val, ty: &Ty) -> bool {
        self.matches(cell, v, ty)
    }

    fn matches(&self, cell: &Cell, v: &Val, ty: &Ty) -> bool {
        let lit_hit = |l: &Lit| -> bool {
            match (l, v) {
                (Lit::Word(w), Val::Enum(e)) => {
                    if w == e {
                        return true;
                    }
                    // A group is a declared subset, so a member matches.
                    self.c.groups.get(w).is_some_and(|(_, ms)| ms.contains(e))
                }
                (Lit::Word(w), Val::Bool(b)) => (w == crate::kw::TRUE) == *b,
                (l, Val::Num(x)) => lit_to_val(l, ty).is_some_and(|o| o == Val::Num(*x)),
                (Lit::Str(s), Val::Str(t)) => s == t,
                (Lit::Date(a, b, c), Val::Date(d, e, f)) => (a, b, c) == (d, e, f),
                _ => false,
            }
        };
        match cell {
            Cell::DontCare => true,
            // The absent value of an optional column. The region machinery has always
            // modelled it as one more value of the enum (`region.rs`), but the evaluator
            // answered `false` to every value there is — so a `| none |` row could not fire,
            // no vector was built for it, and the generated code's handling of the absent
            // value was never run in any language (§15.87).
            Cell::Nothing => matches!(v, Val::Enum(w) if w == crate::kw::NONE),
            Cell::Prefix(ps) => matches!(v, Val::Str(t) if ps.iter().any(|p| t.starts_with(p.as_str()))),
            Cell::Lit(l) => lit_hit(l),
            Cell::Set(ls) => ls.iter().any(lit_hit),
            Cell::Not(ls) => !ls.iter().any(lit_hit),
            Cell::Cmp(cs) => cs.iter().all(|(op, l)| {
                // Dates are lowered to ordinals and compared with the same machinery as
                // numbers (§2.1 allows only comparisons and ranges on them).
                let ord = |x: &Val| match x {
                    Val::Num(r) => Some(*r),
                    Val::Date(y, m, d) => Some(crate::types::date_ord(*y, *m, *d)),
                    _ => None,
                };
                let (Some(x), Some(b)) = (ord(v), lit_to_val(l, ty).as_ref().and_then(ord)) else {
                    return false;
                };
                use std::cmp::Ordering::*;
                match (op, x.cmp_to(b)) {
                    (CmpOp::Le, Less | Equal) => true,
                    (CmpOp::Lt, Less) => true,
                    (CmpOp::Ge, Greater | Equal) => true,
                    (CmpOp::Gt, Greater) => true,
                    _ => false,
                }
            }),
        }
    }

    fn expr(&self, e: &Expr) -> Option<Val> {
        match e {
            Expr::Name(n, _) => self.vals.get(n).cloned(),
            // A literal inside an expression is read **in its own unit**, which is the
            // only reading that can be right: E103 refuses a literal whose unit differs
            // from what it meets (`重量(mass[g]) - 2kg`, `額(money[銭]) >= 5円`), so by
            // the time this runs the two agree. `codegen` reads it the same way, which is
            // what keeps the generated code and this evaluator in step.
            //
            // It used to guess instead — 円, then a rate, then a plain number — and
            // returned *nothing* for every other unit. A `define` over `重量 >= 500g` or
            // over `額 >= 250EUR` then had no value, no table fired, and the vectors came
            // out empty or `null` while the generated code was right all along.
            Expr::Lit(l, _) => match l {
                Lit::Num(n) => lit_to_val(l, &crate::types::lit_ty_pub(n)),
                _ => lit_to_val(l, &Ty::Number),
            },
            Expr::Call(name, args, _) => {
                let a: Vec<Val> = args.iter().filter_map(|x| self.expr(x)).collect();
                match (name.as_str(), a.as_slice()) {
                    (crate::kw::MIN, [Val::Num(x), Val::Num(y)]) => {
                        Some(Val::Num(if x.cmp_to(*y) == std::cmp::Ordering::Less { *x } else { *y }))
                    }
                    (crate::kw::MAX, [Val::Num(x), Val::Num(y)]) => {
                        Some(Val::Num(if x.cmp_to(*y) == std::cmp::Ordering::Greater { *x } else { *y }))
                    }
                    (m, [Val::Num(x), Val::Num(g)]) if RoundMode::parse(m).is_some() => {
                        Some(Val::Num(x.round_to(RoundMode::parse(m).unwrap(), *g)))
                    }
                    _ => None,
                }
            }
            Expr::Bin(l, op, r, _) => {
                let (Some(lv), Some(rv)) = (self.expr(l), self.expr(r)) else { return None };
                use BinOp::*;
                // Dates are lowered to ordinals and compared. They have no addition or
                // subtraction, so only comparisons apply (§2.1).
                let lv = match (&lv, op) {
                    (Val::Date(y, m, d), Le | Lt | Ge | Gt) => Val::Num(crate::types::date_ord(*y, *m, *d)),
                    _ => lv,
                };
                let rv = match (&rv, op) {
                    (Val::Date(y, m, d), Le | Lt | Ge | Gt) => Val::Num(crate::types::date_ord(*y, *m, *d)),
                    _ => rv,
                };
                match (lv, rv) {
                    (Val::Num(a), Val::Num(b)) => Some(match op {
                        Add => Val::Num(a.add(b)),
                        Sub => Val::Num(a.sub(b)),
                        Mul => Val::Num(a.mul(b)),
                        Div => Val::Num(a.div(b)),
                        Le => Val::Bool(a.cmp_to(b) != std::cmp::Ordering::Greater),
                        Lt => Val::Bool(a.cmp_to(b) == std::cmp::Ordering::Less),
                        Ge => Val::Bool(a.cmp_to(b) != std::cmp::Ordering::Less),
                        Gt => Val::Bool(a.cmp_to(b) == std::cmp::Ordering::Greater),
                        Eq => Val::Bool(a.cmp_to(b) == std::cmp::Ordering::Equal),
                    }),
                    (a, b) if *op == Eq => Some(Val::Bool(a == b)),
                    _ => None,
                }
            }
        }
    }

    /// Evaluate one table and bind its outputs. Returns the index of the row that matched.
    ///
    /// What is evaluated is the table's definition set (§15.66): the merged table
    /// of every table defining the same output, in evaluation order, at the position of the
    /// last of them. At any other member's position nothing happens.
    fn table(&mut self, t: &Table) -> Option<usize> {
        let t = self.c.table_at(t)?;
        let name = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
        let hit = t.rows.iter().position(|row| {
            t.inputs.iter().enumerate().all(|(ci, (col, _))| {
                let (Some(v), Some(ty)) = (self.vals.get(col), self.c.ty_of(col)) else {
                    return false;
                };
                row.cells.get(ci).is_none_or(|cell| self.matches(cell, v, &ty))
            })
        })?;
        // The trace names the row as written: the table it came from and its position there.
        let tn = t.rows[hit].origin.clone().unwrap_or_else(|| name.clone());
        let rn = t.rows[hit].index;
        self.fired.push(row_tag(&tn, rn));
        self.fired_rows.push((tn, rn));
        for (oi, oc) in t.outputs.iter().enumerate() {
            let ty = self.c.ty_of(&oc.name.text).unwrap_or(Ty::Unknown);
            let v = match t.rows[hit].outs.get(oi) {
                Some(OutCell::Lit(l)) => lit_to_val(l, &ty),
                Some(OutCell::Name(w)) => self
                    .vals
                    .get(w)
                    .cloned()
                    .or_else(|| lit_to_val(&Lit::Word(w.clone()), &ty)),
                None => None,
            };
            if let Some(v) = v {
                self.vals.insert(oc.name.text.clone(), v);
            }
        }
        Some(hit)
    }
}

/// Run the rule to the end from the input bindings and return the value of the output.
pub fn run(f: &RuleFile, c: &Checked, inputs: HashMap<String, Val>) -> (Option<Val>, Vec<String>) {
    let (v, fired, _) = run_bindings(f, c, inputs);
    (v, fired)
}

/// The coverage check of §9.2 has to see whether "the cells of a row hold for this input",
/// including the derived and defined columns. This entry point returns the bindings as they
/// are once the run has finished.
pub fn run_bindings(
    f: &RuleFile,
    c: &Checked,
    inputs: HashMap<String, Val>,
) -> (Option<Val>, Vec<String>, HashMap<String, Val>) {
    // A sequence of exactly one element **is** one element's case, and the coverage audit
    // reads it that way: the element's fields and everything the tables bound from them
    // (§15.56). A longer sequence has no single set of bindings, and nothing asks it for one.
    if let Some(fold) = &f.fold {
        if let Some(Val::Seq(xs)) = inputs.get(&fold.over) {
            if xs.len() == 1 {
                let mut m: HashMap<String, Val> =
                    inputs.iter().filter(|(k, _)| *k != &fold.over).map(|(k, v)| (k.clone(), v.clone())).collect();
                for (k, v) in &xs[0] {
                    m.insert(k.clone(), v.clone());
                }
                let (fired, _, _, binds) = run_tables(f, c, m);
                let (outs, _, _, _) = run_fold(f, c, inputs);
                return (outs.into_iter().next().and_then(|(_, v)| v), fired, binds);
            }
        }
    }
    let (outs, fired, binds) = run_all(f, c, inputs);
    (outs.into_iter().next().and_then(|(_, v)| v), fired, binds)
}

/// Return every output (the multiple outputs of §8.5), in declaration order; rounding is
/// applied once per output. `result` is sugar that affects only the first output (§1.2), so
/// the second and later ones are taken from the binding of the same name.
pub fn run_all(
    f: &RuleFile,
    c: &Checked,
    inputs: HashMap<String, Val>,
) -> (Vec<(String, Option<Val>)>, Vec<String>, HashMap<String, Val>) {
    let (outs, fired, _, binds) = run_all_traced(f, c, inputs);
    (outs, fired, binds)
}

/// `run_all`, plus the fired rows as `(table, row)` for a diagnostic's structured half.
#[allow(clippy::type_complexity)]
/// A rule that walks a sequence (§15.56).
///
/// The items run once per element with that element's fields in scope, and the fold reduces
/// the column of verdicts. `held` is what the walk has picked up — by `take_unique`,
/// `take_first` or `keep_max`. A walk that picked nothing up holds the `empty` answer, which
/// is what makes `exhausted -> held` total: it is either what was taken, or the answer for a
/// sequence with nothing in it.
///
/// Two elements taking under `take_unique` is a contradiction, and the answer is **no
/// answer** — the same shape the generated code raises on. No vector carries one: the
/// generator drops a case its own evaluator cannot answer.
fn run_fold(
    f: &RuleFile,
    c: &Checked,
    inputs: HashMap<String, Val>,
) -> (Vec<(String, Option<Val>)>, Vec<String>, Vec<(String, usize)>, HashMap<String, Val>) {
    let fold = f.fold.as_ref().expect("run_fold is only called for a rule that has one");
    let seq = match inputs.get(&fold.over) {
        Some(Val::Seq(xs)) => xs.clone(),
        _ => Vec::new(),
    };
    let scalars: HashMap<String, Val> =
        inputs.iter().filter(|(k, _)| *k != &fold.over).map(|(k, v)| (k.clone(), v.clone())).collect();

    let answer_of = |expr: &Option<Expr>, held: Option<Val>| -> Option<Val> {
        let e = expr.as_ref()?;
        let mut m = scalars.clone();
        if let Some(h) = held {
            m.insert(crate::kw::HELD.to_string(), h);
        }
        Env::new(c, m).expr(e)
    };
    let empty_answer = answer_of(&fold.empty, None);

    let (mut fired, mut fired_rows) = (Vec::new(), Vec::new());
    let mut held: Option<Val> = None;
    let mut taken: Option<Val> = None;
    let mut best: Option<Val> = None;
    let mut broken = false;
    let mut stopped: Option<Option<Val>> = None;
    for e in &seq {
        let mut m = scalars.clone();
        for (k, v) in e {
            m.insert(k.clone(), v.clone());
        }
        let (ef, er, _, vals) = run_tables(f, c, m.clone());
        fired.extend(ef);
        fired_rows.extend(er);
        let mut env = Env::new(c, m);
        env.vals = vals.clone();
        let Some(Val::Enum(v)) = vals.get(&fold.verdict).cloned() else { continue };
        let Some((_, arm, _)) = fold.arms.iter().find(|(n, _, _)| n.text == v) else { continue };
        match arm {
            Arm::Next => {}
            Arm::Stop(None) => {
                stopped = Some(None);
                break;
            }
            Arm::Stop(Some(x)) => {
                stopped = Some(env.expr(x));
                break;
            }
            // A take is definite and a keep is provisional, so they do not share a slot: a
            // provisional value already held is not what `take_unique` is unique about. Two
            // **takes** are the contradiction; a take after any number of keeps is the answer.
            Arm::Take { expr, unique } => {
                if taken.is_some() {
                    if *unique {
                        broken = true;
                        break;
                    }
                    // take_first: the first one stands and the rest are passed over.
                } else {
                    taken = env.expr(expr);
                }
            }
            Arm::KeepMax { expr, key } => {
                let k = env.expr(key);
                let better = match (&best, &k) {
                    (None, Some(_)) => true,
                    (Some(Val::Num(a)), Some(Val::Num(b))) => b.cmp_to(*a) == std::cmp::Ordering::Greater,
                    _ => false,
                };
                if better {
                    best = k;
                    held = env.expr(expr);
                }
            }
        }
    }

    let answer = if broken {
        None
    } else if seq.is_empty() {
        empty_answer
    } else {
        match stopped {
            Some(Some(v)) => Some(v),
            _ => answer_of(&fold.exhausted, taken.or(held).or(empty_answer)),
        }
    };

    let mut outs: Vec<(String, Option<Val>)> = Vec::new();
    for (oi, od) in f.outputs.iter().enumerate() {
        let name = od.name.text.clone();
        let mut v = if oi == 0 { answer.clone() } else { None };
        if let Some(Val::Num(x)) = &v {
            if let Some(rd) = &od.rounding {
                let ty = c.ty_of(&name).unwrap_or(Ty::Unknown);
                if let (Some(g), Some(m)) = (lit_value_in_pub(&rd.grid, &ty), RoundMode::parse(&rd.mode)) {
                    v = Some(Val::Num(x.round_to(m, g)));
                }
            }
        }
        outs.push((name, v));
    }
    (outs, fired, fired_rows, HashMap::new())
}

/// Run the rule's items over one environment, without the fold: the tables, the derived
/// values and the definitions, and everything they bound. A rule that walks a sequence uses
/// it once per element, and so does the vector generator when it asks what verdict an
/// element lands on (§15.56).
pub fn run_tables(
    f: &RuleFile,
    c: &Checked,
    inputs: HashMap<String, Val>,
) -> (Vec<String>, Vec<(String, usize)>, Vec<String>, HashMap<String, Val>) {
    let mut env = Env::new(c, inputs);
    for it in &f.items {
        match it {
            Item::Derived(d) => {
                if let Some(v) = env.expr(&d.expr) {
                    env.vals.insert(d.name.text.clone(), v);
                }
            }
            Item::Define(d) => {
                if let Some(v) = env.expr(&d.expr) {
                    env.vals.insert(d.name.text.clone(), v);
                }
            }
            Item::Table(t) => {
                env.table(t);
            }
            // A count is what the walk leaves behind, not something one element does.
            Item::Agg(_) => {}
        }
    }
    (env.fired.clone(), env.fired_rows.clone(), Vec::new(), env.vals)
}

/// One element of a counting walk: the items that only have a value inside it (§15.58).
///
/// A rule that counts has items on both sides of the walk, so — unlike a fold, where every
/// item belongs to one element — the element phase runs a subset. Running the rest here as
/// well would put rows in the trace that fired once per element and answer with a count
/// nothing had counted yet.
fn run_walk(
    f: &RuleFile,
    c: &Checked,
    scoped: &std::collections::HashSet<String>,
    inputs: HashMap<String, Val>,
) -> (Vec<String>, Vec<(String, usize)>, HashMap<String, Val>) {
    let mut env = Env::new(c, inputs);
    for it in &f.items {
        match it {
            Item::Derived(d) if scoped.contains(&d.name.text) => {
                if let Some(v) = env.expr(&d.expr) {
                    env.vals.insert(d.name.text.clone(), v);
                }
            }
            Item::Define(d) if scoped.contains(&d.name.text) => {
                if let Some(v) = env.expr(&d.expr) {
                    env.vals.insert(d.name.text.clone(), v);
                }
            }
            Item::Table(t) if t.outputs.iter().any(|o| scoped.contains(&o.name.text)) => {
                env.table(t);
            }
            _ => {}
        }
    }
    (env.fired.clone(), env.fired_rows.clone(), env.vals)
}

/// Does this element's column hold the value the count is looking for?
fn counts_here(v: Option<&Val>, want: &Option<crate::ast::Name>) -> bool {
    match (v, want) {
        (Some(Val::Bool(b)), None) => *b,
        (Some(Val::Bool(b)), Some(w)) => *b == (w.text == crate::kw::TRUE),
        (Some(Val::Enum(x)), Some(w)) => *x == w.text,
        _ => false,
    }
}

/// A rule that counts (§15.58) runs in two phases: the walk, which is the element-scoped
/// items once per element with the counters beside them, and then the rule proper, with
/// each count bound like any other number.
///
/// The second phase is an ordinary rule — tables, `result`, rounding — which is the whole
/// point of counting rather than folding: what turns the count into a class is a table, and
/// a table is checked.
fn run_counted(
    f: &RuleFile,
    c: &Checked,
    inputs: HashMap<String, Val>,
) -> (Vec<(String, Option<Val>)>, Vec<String>, Vec<(String, usize)>, HashMap<String, Val>) {
    let scoped = crate::types::element_scoped(f);
    let counts: Vec<&crate::ast::AggDecl> = f
        .items
        .iter()
        .filter_map(|it| if let Item::Agg(d) = it { Some(d) } else { None })
        .collect();
    let over = counts.first().map(|d| d.over.clone()).unwrap_or_default();
    let seq = match inputs.get(&over) {
        Some(Val::Seq(xs)) => xs.clone(),
        _ => Vec::new(),
    };
    let scalars: HashMap<String, Val> =
        inputs.iter().filter(|(k, _)| *k != &over).map(|(k, v)| (k.clone(), v.clone())).collect();

    // The sequence is capped by the smallest bound any count declares, and a longer one has
    // no answer here for the same reason the generated code refuses it: the completeness
    // proof was made over the declared universe (§15.58).
    let cap = counts
        .iter()
        .filter_map(|d| c.ranges.get(&d.name.text).and_then(|(_, hi)| *hi))
        .map(|hi| hi.num / hi.den)
        .min();
    if cap.is_some_and(|cap| seq.len() as i128 > cap) {
        let outs = f.outputs.iter().map(|od| (od.name.text.clone(), None)).collect();
        return (outs, Vec::new(), Vec::new(), HashMap::new());
    }

    let (mut fired, mut fired_rows) = (Vec::new(), Vec::new());
    let mut tally: HashMap<String, Rat> = counts.iter().map(|d| (d.name.text.clone(), Rat::zero())).collect();
    for e in &seq {
        let mut m = scalars.clone();
        for (k, v) in e {
            m.insert(k.clone(), v.clone());
        }
        let (ef, er, vals) = run_walk(f, c, &scoped, m);
        fired.extend(ef);
        fired_rows.extend(er);
        for d in &counts {
            // A count adds one per element that passes its test; a sum adds the column
            // itself (§15.100).
            let add = match d.kind {
                AggKind::Count => {
                    if counts_here(vals.get(&d.column.text), &d.value) { Rat::int(1) } else { Rat::zero() }
                }
                AggKind::Sum => match vals.get(&d.column.text) {
                    Some(Val::Num(v)) => *v,
                    _ => Rat::zero(),
                },
            };
            let e = tally.entry(d.name.text.clone()).or_insert(Rat::zero());
            *e = e.add(add);
        }
    }

    let mut m = scalars;
    for (name, n) in &tally {
        m.insert(name.clone(), Val::Num(*n));
    }
    let mut env = Env::new(c, m);
    env.fired = fired;
    env.fired_rows = fired_rows;
    for it in &f.items {
        match it {
            Item::Derived(d) if !scoped.contains(&d.name.text) => {
                if let Some(v) = env.expr(&d.expr) {
                    env.vals.insert(d.name.text.clone(), v);
                }
            }
            Item::Define(d) if !scoped.contains(&d.name.text) => {
                if let Some(v) = env.expr(&d.expr) {
                    env.vals.insert(d.name.text.clone(), v);
                }
            }
            Item::Table(t) if !t.outputs.iter().any(|o| scoped.contains(&o.name.text)) => {
                env.table(t);
            }
            _ => {}
        }
    }
    let outs = outputs_of(f, c, &mut env);
    (outs, env.fired.clone(), env.fired_rows.clone(), env.vals)
}

/// The rule's outputs, read out of an environment that has finished running: the `result`
/// expression for the first one, the binding of its own name for the rest, and the declared
/// rounding applied last (§7.2).
fn outputs_of(f: &RuleFile, c: &Checked, env: &mut Env) -> Vec<(String, Option<Val>)> {
    let mut outs: Vec<(String, Option<Val>)> = Vec::new();
    for (oi, od) in f.outputs.iter().enumerate() {
        let name = od.name.text.clone();
        let mut v = match (&f.result, oi) {
            (Some(r), 0) => env.expr(&r.expr),
            _ => env.vals.get(&name).cloned(),
        };
        if let Some(Val::Num(x)) = &v {
            if let Some(rd) = &od.rounding {
                let ty = c.ty_of(&name).unwrap_or(Ty::Unknown);
                if let Some(g) = lit_value_in_pub(&rd.grid, &ty) {
                    if let Some(m) = RoundMode::parse(&rd.mode) {
                        v = Some(Val::Num(x.round_to(m, g)));
                    }
                }
            }
        }
        outs.push((name, v));
    }
    outs
}

pub fn run_all_traced(
    f: &RuleFile,
    c: &Checked,
    inputs: HashMap<String, Val>,
) -> (Vec<(String, Option<Val>)>, Vec<String>, Vec<(String, usize)>, HashMap<String, Val>) {
    if f.fold.is_some() {
        return run_fold(f, c, inputs);
    }
    if f.items.iter().any(|it| matches!(it, Item::Agg(_))) {
        return run_counted(f, c, inputs);
    }
    let mut env = Env::new(c, inputs);
    for it in &f.items {
        match it {
            Item::Derived(d) => {
                if let Some(v) = env.expr(&d.expr) {
                    env.vals.insert(d.name.text.clone(), v);
                }
            }
            Item::Define(d) => {
                if let Some(v) = env.expr(&d.expr) {
                    env.vals.insert(d.name.text.clone(), v);
                }
            }
            Item::Table(t) => {
                env.table(t);
            }
            Item::Agg(_) => {}
        }
    }
    // The rounding declared on the output applies last — §7.2, "an unrounded value never
    // reaches the output".
    let outs = outputs_of(f, c, &mut env);
    (outs, env.fired, env.fired_rows, env.vals)
}

/// The match test for a single cell, used from outside the table (the coverage check of §9.2).
pub fn cell_matches(c: &Checked, cell: &Cell, v: &Val, ty: &Ty) -> bool {
    Env::new(c, HashMap::new()).matches(cell, v, ty)
}

/// The `examples` section is executable specification (§1.2). A miss is E107, reported with
/// the fired rows.
/// The value of a named `sequence`: one map per row, keyed by the element's fields (§15.56).
/// A block with no rows is the empty sequence, which is a case of its own.
pub fn seq_value(f: &RuleFile, c: &Checked, name: &str) -> Option<Val> {
    let sq = f.sequences.iter().find(|s| s.name.text == name)?;
    let mut rows = Vec::new();
    for row in &sq.rows {
        let mut one: std::collections::BTreeMap<String, Val> = Default::default();
        for (ci, (col, _)) in sq.cols.iter().enumerate() {
            let ty = c.ty_of(col)?;
            let Some(Cell::Lit(l)) = row.cells.get(ci) else { continue };
            one.insert(col.clone(), lit_to_val(l, &ty)?);
        }
        rows.push(one);
    }
    Some(Val::Seq(rows))
}

/// The inputs one example row stands for, the sequence included. The cell of the sequence
/// column names a `sequence` block; everything else is a literal of its column's type.
pub fn example_env(f: &RuleFile, c: &Checked, ex: &crate::ast::Table, row: &Row) -> HashMap<String, Val> {
    let mut env: HashMap<String, Val> = HashMap::new();
    let seq_col = f.elements.as_ref().map(|e| e.name.text.clone());
    for (ci, (col, _)) in ex.inputs.iter().enumerate() {
        if Some(col) == seq_col.as_ref() {
            if let Some(Cell::Lit(Lit::Word(w))) = row.cells.get(ci) {
                if let Some(v) = seq_value(f, c, w) {
                    env.insert(col.clone(), v);
                }
            }
            continue;
        }
        let Some(ty) = c.ty_of(col) else { continue };
        match row.cells.get(ci) {
            Some(Cell::Lit(l)) => {
                if let Some(v) = lit_to_val(l, &ty) {
                    env.insert(col.clone(), v);
                }
            }
            // `none` in an example is a case: the caller passed nothing for an optional
            // input. It bound nothing at all, so no table fired and E107 said the output had
            // no value — naming the output rather than the cell (§15.87).
            Some(Cell::Nothing) if matches!(ty, Ty::Opt(_)) => {
                env.insert(col.clone(), Val::Enum(crate::kw::NONE.into()));
            }
            _ => {}
        }
    }
    env
}

pub fn check_examples(f: &RuleFile, c: &Checked, path: &str) -> Vec<Diag> {
    let mut out = Vec::new();
    let Some(ex) = &f.examples else { return out };
    if f.outputs.is_empty() {
        return out;
    }

    // §1.2: the examples are the wedge that breaks the blind spot the implementations share.
    // When all three share the same mistake the agreement stays green, and the only thing
    // that can break it is an expected value written by a person. That is why every output
    // must be written; a missing column is an error (E111).
    let head = ex.rows.first().map(|r| r.span.clone());
    for od in &f.outputs {
        if !ex.outputs.iter().any(|o| o.name.text == od.name.text) {
            let Some(sp) = head.clone() else { continue };
            out.push(
                Diag::error("E111", tr!("例に出力 {} の列がありません", "The examples have no column for output {}", od.name.text))
                    .at(tr!("{path}:{} 例", "{path}:{} examples", sp.line))
                    .fix(crate::diag::FixKind::AddExpected, &od.name.text)
                    .mark(sp, tr!("{} の期待値がありません", "no expected value for {}", od.name.text))
                    .note(tr!("実装どうしの照合では捕まらない誤りを捕まえられるのは例だけなので、出力は全部書きます。", "The examples are the only wedge that catches errors the implementations can share, so every output is written."))
                    .note(tr!("ヒント: 見出しに `{}` の列を足してください。", "Hint: add a `{}` column to the header.", od.name.text)),
            );
        }
    }
    for row in &ex.rows {
        if row.outs.len() < ex.outputs.len() {
            out.push(
                Diag::error(
                    "E111",
                    tr!(
                        "例の期待値が {} 列足りません",
                        "The example is missing {} expected-value column(s)",
                        ex.outputs.len() - row.outs.len()
                    ),
                )
                .at(tr!("{path}:{} 例", "{path}:{} examples", row.span.line))
                .mark(row.span.clone(), "")
                .note(tr!("宣言した出力の数だけ期待値を書いてください。", "Write one expected value for each declared output.")),
            );
        }
    }
    if out.iter().any(|d| d.code == "E111") {
        return out;
    }

    for row in &ex.rows {
        let env = example_env(f, c, ex, row);
        // An example is a case the rule is claimed to answer, so it has to be a case the rule
        // can receive. A `constraint` says which combinations exist (§15.55); an example
        // outside them would be asserting an answer for an input the generated code refuses
        // at the door, and the checker never demanded a row for it either.
        {
            let a: std::collections::BTreeMap<String, Val> = env.clone().into_iter().collect();
            if !crate::vectors::allowed(f, &a) {
                for k in &f.constraints {
                    let one: std::collections::BTreeMap<String, Val> =
                        a.iter().map(|(x, y)| (x.clone(), y.clone())).collect();
                    let mut only = RuleFile { constraints: vec![k.clone()], ..f.clone() };
                    only.constraints = vec![k.clone()];
                    if crate::vectors::allowed(&only, &one) {
                        continue;
                    }
                    let said = format!("{} {} {}", k.left, k.op.word(), k.right);
                    let mut d = Diag::error("E019", tr!("例が制約を破っています", "An example breaks a constraint"))
                        .at(tr!("{path}:{} 例", "{path}:{} examples", row.span.line))
                        .mark(row.span.clone(), tr!("この入力では `{said}` が成り立ちません", "`{said}` does not hold for this input"));
                    for (col, _) in &ex.inputs {
                        if let Some(v) = env.get(col) {
                            d = d.win(col.clone(), wval(c, col, v));
                        }
                    }
                    out.push(
                        d
                            .note(tr!(
                                "制約は「この組み合わせは起きない」という宣言で、検査はそれを信じて行を要求していません。生成コードも入口で断ります。",
                                "A constraint declares that a combination does not happen; the checks believed it and demanded no row, and the generated code refuses it at the door."
                            ))
                            .note(tr!(
                                "例の値を直すか、その組み合わせが本当に起きるなら制約のほうを消してください。",
                                "Correct the example's values, or drop the constraint if that combination really does happen."
                            )),
                    );
                }
                continue;
            }
        }
        let (got, fired, fired_rows, _) = run_all_traced(f, c, env.clone());
        // The witness of E107 is the example's own row: the inputs as written, and the value
        // it expected. A caller can hand these straight back as a vector (§10.2).
        let wit_in: Vec<(String, crate::diag::WVal)> = ex
            .inputs
            .iter()
            .filter_map(|(col, _)| env.get(col).map(|v| (col.clone(), wval(c, col, v))))
            .collect();
        let with_rows = |mut d: Diag| -> Diag {
            for (t, r) in &fired_rows {
                d = d.rowref(t.clone(), *r);
            }
            d
        };
        // Every output is checked. Looking only at the first, an example would pass silently
        // no matter how wrong the second expected value is (the same shape as E110, which
        // silently skipped columns whose type could not be resolved).
        for (oi, od) in f.outputs.iter().enumerate() {
            let oty = c.ty_of(&od.name.text).unwrap_or(Ty::Unknown);
            let want = row.outs.get(oi).and_then(|o| match o {
                OutCell::Lit(l) => lit_to_val(l, &oty),
                OutCell::Name(w) => lit_to_val(&Lit::Word(w.clone()), &oty),
            });
            let g = got.get(oi).and_then(|(_, v)| v.clone());
            match (&want, &g) {
                (Some(w), Some(g)) if w == g => {}
                (Some(w), Some(g)) => out.push(
                    with_rows(Diag::error(
                        "E107",
                        tr!(
                            "例が合いません: {} は {} のはずが {} になりました",
                            "Example does not hold: {} should be {} but came out as {}",
                            od.name.text,
                            w.show(&oty),
                            g.show(&oty)
                        ),
                    ))
                    .at(tr!("{path}:{} 例", "{path}:{} examples", row.span.line))
                    .wit(crate::diag::Witness {
                        inputs: wit_in.clone(),
                        outputs: vec![(od.name.text.clone(), wval(c, &od.name.text, g))],
                        expected: vec![(od.name.text.clone(), wval(c, &od.name.text, w))],
                    })
                    .mark(row.span.clone(), "")
                    .note(tr!("当てはまった行: {}", "Fired rows: {}", fired.join(" / ")))
                    .note(tr!("例は実行される仕様です。表を直すか、例のほうが間違っているなら例を直してください。", "Examples are executable specification. Fix the table, or fix the example if the example is what is wrong.")),
                ),
                (Some(w), None) => out.push(
                    with_rows(Diag::error(
                        "E107",
                        tr!("例が合いません: {} は {} のはずが、値が出ませんでした", "Example does not hold: {} should be {} but no value came out", od.name.text, w.show(&oty)),
                    ))
                    .at(tr!("{path}:{} 例", "{path}:{} examples", row.span.line))
                    .wit(crate::diag::Witness {
                        inputs: wit_in.clone(),
                        outputs: Vec::new(),
                        expected: vec![(od.name.text.clone(), wval(c, &od.name.text, w))],
                    })
                    .mark(row.span.clone(), "")
                    .note(if fired.is_empty() {
                        tr!("どの表にも当てはまりませんでした。", "No table fired.")
                    } else {
                        tr!("当てはまった行: {}", "Fired rows: {}", fired.join(" / "))
                    }),
                ),
                _ => {}
            }
        }
    }
    out
}

/// The witness used to word E104: one value chosen per input. An enum takes the first value
/// in declaration order; a number takes a value inside the declared range that is likely to
/// produce a fraction (§11 principle 2).
/// Where a numeric input is taken from. A fraction comes from "how the value meshes with the
/// other side's step", so looking at the middle alone can miss it (the value hits the upper
/// bound and gets rounded, and so on).
#[derive(Clone, Copy)]
pub enum Pick {
    Mid(i128),
    Lo(i128),
    Hi(i128),
}

pub fn witness_inputs(f: &RuleFile, c: &Checked, pick: Pick) -> HashMap<String, Val> {
    let mut env = HashMap::new();
    for i in &f.inputs {
        let Some(ty) = c.ty_of(&i.name.text) else { continue };
        let v = match &ty {
            Ty::Enum(en) => c.enums.get(en).and_then(|vs| vs.first()).map(|v| Val::Enum(v.clone())),
            Ty::Bool => Some(Val::Bool(true)),
            Ty::Rate => {
                // The value must lie on the declared step. A witness off the step is an input
                // that cannot occur in the first place, so it is no witness at all.
                let k = *c.scales.get(&i.name.text).unwrap_or(&100);
                Some(Val::Num(Rat::new(12, k.max(1))))
            }
            Ty::Money { .. } | Ty::Qty { .. } => {
                let (lo, hi) = c.ranges.get(&i.name.text).copied().unwrap_or((None, None));
                let base = match (pick, lo, hi) {
                    (Pick::Lo(_), Some(a), _) => a,
                    (Pick::Hi(_), _, Some(b)) => b,
                    (_, Some(a), Some(b)) => {
                        let mid = a.add(b).div(Rat::int(2));
                        if mid.is_int() { mid } else { Rat::int(mid.num / mid.den) }
                    }
                    (_, Some(a), None) => a,
                    (_, None, Some(b)) => b,
                    (_, None, None) => Rat::int(800),
                };
                let off = match pick {
                    Pick::Mid(k) | Pick::Lo(k) => k,
                    Pick::Hi(k) => -k,
                };
                let v = base.add(Rat::int(off));
                let v = match (lo, hi) {
                    (_, Some(b)) if v.cmp_to(b) == std::cmp::Ordering::Greater => b,
                    (Some(a), _) if v.cmp_to(a) == std::cmp::Ordering::Less => a,
                    _ => v,
                };
                Some(Val::Num(v))
            }
            _ => None,
        };
        if let Some(v) = v {
            env.insert(i.name.text.clone(), v);
        }
    }
    env
}

/// The raw value that reaches the output, ignoring the rounding declaration. E104 uses it to
/// say how much the result can move.
pub fn unrounded_output(f: &RuleFile, c: &Checked) -> Option<Rat> {
    // Look for a witness that produces a fraction first. If none does, return the value of
    // the first witness (the rule then really produces no fraction, and the wording of E104
    // branches accordingly).
    let mut first = None;
    let picks = [
        Pick::Mid(0),
        Pick::Mid(1),
        Pick::Lo(1),
        Pick::Lo(3),
        Pick::Hi(1),
        Pick::Hi(3),
        Pick::Mid(7),
    ];
    for pick in picks {
        let (v, _) = run_raw(f, c, witness_inputs(f, c, pick));
        if let Some(Val::Num(r)) = v {
            if !r.is_int() {
                return Some(r);
            }
            first.get_or_insert(r);
        }
    }
    first
}

fn run_raw(f: &RuleFile, c: &Checked, inputs: HashMap<String, Val>) -> (Option<Val>, Vec<String>) {
    let mut env = Env::new(c, inputs);
    for it in &f.items {
        match it {
            Item::Derived(d) => {
                if let Some(v) = env.expr(&d.expr) {
                    env.vals.insert(d.name.text.clone(), v);
                }
            }
            Item::Define(d) => {
                if let Some(v) = env.expr(&d.expr) {
                    env.vals.insert(d.name.text.clone(), v);
                }
            }
            Item::Table(t) => {
                env.table(t);
            }
            // E104 asks what the output looks like unrounded; a walk has no witness here.
            Item::Agg(_) => {}
        }
    }
    let out_name = f.outputs.first().map(|o| o.name.text.clone()).unwrap_or_default();
    let v = match &f.result {
        Some(r) => env.expr(&r.expr),
        None => env.vals.get(&out_name).cloned(),
    };
    (v, env.fired)
}
