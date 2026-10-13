//! The reference evaluator (§0).
//!
//! The semantics have exactly one source of truth, and it is this file. Checking the
//! `examples` section, computing the expected values of test vectors, and replaying past
//! records all go through this same evaluator. Only the generated code runs in production;
//! this evaluator appears only on the verification side.

use crate::ast::*;
use crate::diag::Diag;
use crate::num::{Rat, RoundMode, RoundTo};
use crate::types::{Checked, Ty, lit_value_in_pub};
use std::collections::{BTreeMap, HashMap};

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
    pub fn show(&self, ty: &Ty) -> String {
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
        // A literal in an optional column is read in the type the column wraps: a number was
        // read in `number?` itself, where no number is a value, so `<5` matched nothing at all.
        // The absent value is `none`, which no literal and no comparison takes (§15.201).
        let ty = ty.present();
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
            // The values the comparison covers in the enum's order (§15.210).
            Cell::Ranked { values, .. } => matches!(v, Val::Enum(e) if values.contains(e)),
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

    /// The value of an expression in this environment. Public so that the version diff can
    /// weigh a `derive` without running the whole rule for every probe (§15.122).
    pub fn expr(&self, e: &Expr) -> Option<Val> {
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
                // The later of two values of an ordered enum, or the earlier (§15.210).
                if let (crate::kw::MIN | crate::kw::MAX, [Val::Enum(x), Val::Enum(y)]) = (name.as_str(), a.as_slice()) {
                    let en = args.first().and_then(|e| self.enum_of(e))?;
                    let (rx, ry) = (self.c.rank(&en, x)?, self.c.rank(&en, y)?);
                    let later = if rx >= ry { x } else { y };
                    let earlier = if rx <= ry { x } else { y };
                    return Some(Val::Enum(if name == crate::kw::MAX { later.clone() } else { earlier.clone() }));
                }
                match (name.as_str(), a.as_slice()) {
                    (crate::kw::MIN, [Val::Num(x), Val::Num(y)]) => {
                        Some(Val::Num(if x.cmp_to(*y) == std::cmp::Ordering::Less { *x } else { *y }))
                    }
                    (crate::kw::MAX, [Val::Num(x), Val::Num(y)]) => {
                        Some(Val::Num(if x.cmp_to(*y) == std::cmp::Ordering::Greater { *x } else { *y }))
                    }
                    // `floor(T × C ÷ S)` — one step, so the scale is decided once and the
                    // answer is the same integer in every target (§15.102).
                    (crate::kw::ALLOCATE, [Val::Num(t), Val::Num(c), Val::Num(s)]) => {
                        if s.cmp_to(Rat::zero()) != std::cmp::Ordering::Greater {
                            return None;
                        }
                        Some(Val::Num(t.mul(*c).div(*s).round_to(RoundMode::Down, Rat::int(1))))
                    }
                    (m, [Val::Num(x), Val::Num(g)]) if RoundMode::parse(m).is_some() => {
                        Some(Val::Num(x.round_to(RoundMode::parse(m).unwrap(), *g)))
                    }
                    // The remainder of the whole part by a positive whole constant, from 0 to the
                    // constant less one whatever the sign (§15.214). The argument is whole (E103),
                    // so the whole part is the argument itself.
                    (crate::kw::MOD, [Val::Num(x), Val::Num(n)]) => {
                        if !(n.is_int() && n.num > 0) {
                            return None;
                        }
                        let whole = Rat::int(x.num.div_euclid(x.den));
                        Some(Val::Num(crate::types::mod_rat(whole, n.num)))
                    }
                    // The largest whole number whose square does not pass the argument (§15.215);
                    // below zero there is none (E139).
                    (crate::kw::ISQRT, [Val::Num(x)]) => crate::types::isqrt_rat(*x).map(Val::Num),
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
                // Two values of an ordered enum compare by their places in its order (§15.210).
                if let (Val::Enum(a), Val::Enum(b), Le | Lt | Ge | Gt) = (&lv, &rv, op) {
                    let en = self.enum_of(l).or_else(|| self.enum_of(r))?;
                    let (x, y) = (self.c.rank(&en, a)?, self.c.rank(&en, b)?);
                    return Some(Val::Bool(match op {
                        Le => x <= y,
                        Lt => x < y,
                        Ge => x >= y,
                        _ => x > y,
                    }));
                }
                match (lv, rv) {
                    // A divisor of zero is E115's to report; the typing refuses every divisor
                    // that is not a positive constant. The evaluator still runs on a rule that
                    // has errors (to describe an unrounded output, say), and `÷ 0` there
                    // panicked the whole of `rulec check` before E115 was printed (§15.156).
                    (Val::Num(_), Val::Num(b)) if *op == Div && b.cmp_to(Rat::zero()) == std::cmp::Ordering::Equal => None,
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

    /// The enum an expression's value belongs to, when it is a name of one or `min` or `max`
    /// of such names (§15.210).
    fn enum_of(&self, e: &Expr) -> Option<String> {
        match e {
            Expr::Name(n, _) => match self.c.ty_of(n)? {
                Ty::Enum(en) => Some(en),
                Ty::Opt(t) => match *t {
                    Ty::Enum(en) => Some(en),
                    _ => None,
                },
                _ => None,
            },
            Expr::Call(_, args, _) => args.iter().find_map(|a| self.enum_of(a)),
            _ => None,
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
    // (§15.56), beside what was bound before and after the walk. A longer sequence has no single
    // set of bindings for an element, and nothing asks it for one (`run_walking`).
    let (outs, fired, mut binds) = run_all(f, c, inputs);
    // A `result` line binds nothing of its own: the first output is its expression, read at
    // the end. The audit asks for the value before rounding by the output's name — whether it
    // sits on a tie — so it is added here. Without it, an output written with `result` never
    // had a tie to look for (§15.151).
    if let (Some(r), Some(od)) = (&f.result, f.outputs.first()) {
        if !binds.contains_key(&od.name.text) {
            if let Some(v) = Env::new(c, binds.clone()).expr(&r.expr) {
                binds.insert(od.name.text.clone(), v);
            }
        }
    }
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

/// One item, run in an environment: a derived value or a definition bound to its name, a table
/// evaluated (§5.1).
fn run_item(it: &Item, env: &mut Env) {
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
        // A summary is a walk itself, not something a stage runs.
        Item::Agg(_) | Item::Fold(_) => {}
    }
}

/// Run the rule's items over one environment, without walking: what one element sees on its
/// own (§15.208). Every item that is not a summary runs, in the order written, except one of a
/// call that reads what a walk answers when that answer is not in the environment — the walk
/// is not run here, so it has none, unless the caller put it there (§15.219). The vector
/// generator and the coverage audit use it to ask what verdict an element lands on (§15.56).
pub fn run_tables(
    f: &RuleFile,
    c: &Checked,
    inputs: HashMap<String, Val>,
) -> (Vec<String>, Vec<(String, usize)>, Vec<String>, HashMap<String, Val>) {
    let st = crate::types::stages(f);
    let mut env = Env::new(c, inputs);
    for (i, it) in f.items.iter().enumerate() {
        if let crate::types::Stage::Once(k) = st.at(i) {
            if k > 0 && crate::types::item_reads(it).iter().any(|r| !env.vals.contains_key(r) && st.level_of(r) > 0) {
                continue;
            }
        }
        run_item(it, &mut env);
    }
    (env.fired.clone(), env.fired_rows.clone(), Vec::new(), env.vals)
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

/// The order of two values of a column `max` and `min` read (§15.219): numbers and dates by their
/// value, an ordered enum's values by their place. `None` when they are not two of one kind.
fn order_of(c: &Checked, col: &str, a: &Val, b: &Val) -> Option<std::cmp::Ordering> {
    match (a, b) {
        (Val::Num(x), Val::Num(y)) => Some(x.cmp_to(*y)),
        (Val::Date(..), Val::Date(..)) => {
            let ord = |v: &Val| match v {
                Val::Date(y, m, d) => crate::types::date_ord(*y, *m, *d),
                _ => Rat::zero(),
            };
            Some(ord(a).cmp_to(ord(b)))
        }
        (Val::Enum(x), Val::Enum(y)) => {
            let en = c.ty_of(col).and_then(|t| c.ordered_enum(&t).map(str::to_string))?;
            Some(c.rank(&en, x)?.cmp(&c.rank(&en, y)?))
        }
        _ => None,
    }
}

/// Where one summary stands while its walk is on (§15.56, §15.58, §15.219).
enum Acc {
    /// A count or a sum so far; `None` once an element left its column without a value.
    Tally(Option<Rat>),
    /// The largest (smallest) value so far of the elements the `where` let in, and whether an
    /// element it let in had no value in the column.
    Extreme { best: Option<Val>, missing: bool },
    /// A fold: what it has taken and kept, by what key, and whether it stopped (with the value
    /// `stop with` gave, if any).
    Fold { taken: Option<Val>, held: Option<Val>, best: Option<Val>, stopped: Option<Option<Val>> },
}

/// Everything one walk left behind: the answers bound to their names, and — for the coverage
/// audit — the verdict each element landed on, per fold.
struct Walked {
    /// Two elements took under `take_unique`, in the fold at this place among the items: the
    /// rule has no answer.
    broken: Option<usize>,
    /// A `sum` whose total passed the top of the range it declares, by its name: the generated
    /// code refuses the call there (§15.100), and the rule has no answer either.
    past: Option<String>,
}

/// A rule that walks sequences (§15.56, §15.58, §15.219), in the stages its items fall into
/// (§15.208).
///
/// The items **before** every walk run once, on the inputs. Then the walks, in order: walk 1 over
/// each sequence that has one, then the items of one call that read what walk 1 answered, then
/// walk 2, and so on (`types::stages`). In a walk, the items of one element run once per element,
/// with that element's fields and everything worked out so far in scope, and the walk's summaries
/// reduce them: a fold reads its column of verdicts, a `count` and a `sum` add up their column,
/// `max` and `min` keep the largest and the smallest value of theirs. Once a walk is over, what each
/// summary answered is bound to its name — a fold's to the name after its `->`, or the first output
/// — and the outputs are read out of the end, the way every rule's are (`outputs_of`).
///
/// A fold's `held` is what the walk has picked up — by `take_unique`, `take_first` or `keep_max`. A
/// walk that picked nothing up holds the `empty` answer, which is what makes `exhausted -> held`
/// total. `empty`, `exhausted` and `stop with` are read with what was worked out before the walk,
/// and `held`. Two elements taking under `take_unique` is a contradiction, and every output is **no
/// answer** — the shape the generated code raises on. A fold that stops stops reading: the other
/// summaries of the same walk see every element. A walk with one fold and nothing else ends there,
/// as it always did.
///
/// A summary over a column one element leaves without a value has no value either. It used to
/// count as zero, and a total came out 0 where the walk had read nothing at all.
///
/// The bindings returned are what the items of one call bound, and — for a sequence of exactly one
/// element, which **is** one element's case (§15.56) — the element's own: the coverage audit reads
/// a row of an element's table off them.
#[allow(clippy::type_complexity)]
fn run_walking(
    f: &RuleFile,
    c: &Checked,
    inputs: HashMap<String, Val>,
) -> (Vec<(String, Option<Val>)>, Vec<String>, Vec<(String, usize)>, HashMap<String, Val>) {
    walk_all(f, c, inputs, &mut Observe::default())
}

/// What a walk is watched for besides its answer (§15.219): set what to collect before the walk,
/// read what was found after it.
#[derive(Default)]
struct Observe {
    /// Go on past a `stop`, a contradiction and a sum past its range, so that every element is
    /// seen: what the coverage of a fold's transitions is judged on.
    all: bool,
    /// Collect the case of every element of a walk after the first (`ElementCase`).
    cases: bool,
    /// Per fold, by its place among the items, the verdict of every element of its sequence.
    verdicts: HashMap<usize, Vec<Option<String>>>,
    /// The fold two elements both took under `take_unique` in, by its place among the items.
    broke: Option<usize>,
    /// The `sum` whose total passed the top of the range it declares, by its name.
    past: Option<String>,
    /// The cases of the elements of the walks after the first.
    found: Vec<ElementCase>,
}

/// One element of a walk after the first, as a case of its own (§15.219).
///
/// An element of the first walk is a case on its own: a sequence of one element **is** that
/// element's case (§15.56), and the coverage audit reads it off the bindings of such a sequence.
/// An element of a second walk is not: what the first walk found is part of what it is worked out
/// from, and that comes from every element of the sequence. Its case is the values of one call,
/// what the walks before it answered, and the element's own fields — two elements of one sequence
/// are two cases that differ in their fields alone.
#[derive(Debug, Clone)]
pub struct ElementCase {
    /// The sequence, by its place among the rule's `elements`, and the walk over it.
    pub seq: usize,
    pub walk: usize,
    /// What the case is made of: the values of one call, the answers of the walks before it, and
    /// the element's fields.
    pub key: BTreeMap<String, Val>,
    /// Everything bound while the element was worked out in that walk.
    pub binds: BTreeMap<String, Val>,
    /// The rows that fired for it in that walk.
    pub fired: Vec<String>,
}

/// The `sum` whose total passed the top of the range it declares on these inputs, by its name:
/// the generated code refuses the call the moment the walk is over (§15.100), so the reference
/// evaluator has no answer for it either. `None` when every total stays inside.
pub fn sum_past_range(f: &RuleFile, c: &Checked, inputs: HashMap<String, Val>) -> Option<String> {
    if !f.aggs().any(|d| d.kind == AggKind::Sum) {
        return None;
    }
    let mut o = Observe::default();
    let _ = walk_all(f, c, inputs, &mut o);
    o.past
}

/// The fold two elements both took under `take_unique` in, by its place among the rule's items:
/// what the generated code raises on, and the reason the reference evaluator has no answer
/// (§15.56). `None` when no fold did.
pub fn contradiction(f: &RuleFile, c: &Checked, inputs: HashMap<String, Val>) -> Option<usize> {
    if !f.walks() {
        return None;
    }
    let mut o = Observe::default();
    let _ = walk_all(f, c, inputs, &mut o);
    o.broke
}

/// For each fold, by its place among the rule's items, the verdict every element of its sequence
/// lands on, in order: read in the walk the fold runs in, with what was worked out before it, and
/// past a `stop` — the coverage of a fold's transitions is judged on which verdict follows which,
/// and an element after a `stop` is one of them (§15.56, §15.219). A fold whose walk is never
/// reached, because a walk before it had no answer, has none.
pub fn fold_verdicts(f: &RuleFile, c: &Checked, inputs: HashMap<String, Val>) -> HashMap<usize, Vec<Option<String>>> {
    let mut o = Observe { all: true, ..Observe::default() };
    let _ = walk_all(f, c, inputs, &mut o);
    o.verdicts
}

/// The case of every element of the walks after the first (`ElementCase`), in the order they
/// were worked out.
pub fn element_cases(f: &RuleFile, c: &Checked, inputs: HashMap<String, Val>) -> Vec<ElementCase> {
    if !f.walks() {
        return Vec::new();
    }
    let mut o = Observe { cases: true, ..Observe::default() };
    let _ = walk_all(f, c, inputs, &mut o);
    o.found
}

#[allow(clippy::type_complexity)]
fn walk_all(
    f: &RuleFile,
    c: &Checked,
    inputs: HashMap<String, Val>,
    obs: &mut Observe,
) -> (Vec<(String, Option<Val>)>, Vec<String>, Vec<(String, usize)>, HashMap<String, Val>) {
    let st = crate::types::stages(f);
    let seqs: Vec<Vec<std::collections::BTreeMap<String, Val>>> = f
        .elements
        .iter()
        .map(|el| match inputs.get(&el.name.text) {
            Some(Val::Seq(xs)) => xs.clone(),
            _ => Vec::new(),
        })
        .collect();
    let names: Vec<&String> = f.elements.iter().map(|el| &el.name.text).collect();
    let scalars: HashMap<String, Val> = inputs.iter().filter(|(k, _)| !names.contains(k)).map(|(k, v)| (k.clone(), v.clone())).collect();
    let nothing = || -> Vec<(String, Option<Val>)> { f.outputs.iter().map(|od| (od.name.text.clone(), None)).collect() };

    // A sequence is capped by the smallest bound any count over it declares, and a longer one has
    // no answer here for the same reason the generated code refuses it: the completeness proof was
    // made over the declared universe (§15.58). A sum's range bounds its total, not the length.
    for (k, el) in f.elements.iter().enumerate() {
        let cap = f
            .aggs()
            .filter(|d| d.kind == AggKind::Count && d.over == el.name.text)
            .filter_map(|d| c.ranges.get(&d.name.text).and_then(|(_, hi)| *hi))
            .map(|hi| hi.num / hi.den)
            .min();
        if cap.is_some_and(|cap| seqs[k].len() as i128 > cap) {
            return (nothing(), Vec::new(), Vec::new(), HashMap::new());
        }
    }

    // Before every walk.
    let mut env = Env::new(c, scalars);
    for i in st.once(0) {
        run_item(&f.items[i], &mut env);
    }
    let mut one: HashMap<String, Val> = HashMap::new();
    let mut broken = false;
    for level in 1..=st.depth {
        for (w, s) in st.walks() {
            if w != level {
                continue;
            }
            let walked = walk_one(f, c, &st, s, level, &seqs[s], &mut env, &mut one, obs);
            if let Some(n) = walked.past {
                obs.past.get_or_insert(n);
                if !obs.all {
                    return (nothing(), Vec::new(), Vec::new(), HashMap::new());
                }
            }
            if let Some(i) = walked.broken {
                broken = true;
                obs.broke.get_or_insert(i);
                if !obs.all {
                    let fired = std::mem::take(&mut env.fired);
                    let fired_rows = std::mem::take(&mut env.fired_rows);
                    return (nothing(), fired, fired_rows, HashMap::new());
                }
            }
        }
        if broken {
            break;
        }
        // What reads the answers of the walks up to this one.
        for i in st.once(level) {
            run_item(&f.items[i], &mut env);
        }
    }
    let outs = outputs_of(f, c, &mut env);
    let mut binds = env.vals.clone();
    for (k, v) in one {
        binds.entry(k).or_insert(v);
    }
    (outs, env.fired, env.fired_rows, binds)
}

/// One walk over one sequence (§15.219): its items of one element on every element, its summaries
/// reduced, and their answers bound in `env` once it is over.
#[allow(clippy::too_many_arguments)]
fn walk_one(
    f: &RuleFile,
    c: &Checked,
    st: &crate::types::Stages,
    s: usize,
    level: usize,
    seq: &[std::collections::BTreeMap<String, Val>],
    env: &mut Env,
    one: &mut HashMap<String, Val>,
    obs: &mut Observe,
) -> Walked {
    let observe = obs.all;
    let body = st.body(f, s, level);
    let sums = st.summaries_in(f, s, level);
    let before = env.vals.clone();
    // What a walk answers, by name: part of an element's case in a later walk.
    let answers: std::collections::HashSet<String> = crate::types::walk_answers(f).into_iter().collect();
    // A walk of one fold and nothing else ends when the fold does (§15.56).
    let alone = sums.len() == 1 && matches!(f.items[sums[0]], Item::Fold(_));
    let answer_of = |expr: &Option<Expr>, held: Option<Val>| -> Option<Val> {
        let e = expr.as_ref()?;
        let mut m = before.clone();
        if let Some(h) = held {
            m.insert(crate::kw::HELD.to_string(), h);
        }
        Env::new(c, m).expr(e)
    };
    let mut accs: Vec<Acc> = sums
        .iter()
        .map(|&i| match &f.items[i] {
            Item::Agg(d) if d.kind.extreme() => Acc::Extreme { best: None, missing: false },
            Item::Agg(_) => Acc::Tally(Some(Rat::zero())),
            _ => Acc::Fold { taken: None, held: None, best: None, stopped: None },
        })
        .collect();
    let mut broken: Option<usize> = None;
    'elements: for e in seq {
        let mut m = before.clone();
        for (k, v) in e {
            m.insert(k.clone(), v.clone());
        }
        let mut eenv = Env::new(c, m);
        for &i in &body {
            run_item(&f.items[i], &mut eenv);
        }
        if obs.cases && level > 1 {
            obs.found.push(ElementCase {
                seq: s,
                walk: level,
                key: before
                    .iter()
                    .filter(|(k, _)| f.inputs.iter().any(|i| i.name.text == **k) || answers.contains(*k))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .chain(e.iter().map(|(k, v)| (k.clone(), v.clone())))
                    .collect(),
                binds: eenv.vals.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
                fired: eenv.fired.clone(),
            });
        }
        env.fired.append(&mut eenv.fired);
        env.fired_rows.append(&mut eenv.fired_rows);
        if seq.len() == 1 {
            for (k, v) in &eenv.vals {
                one.insert(k.clone(), v.clone());
            }
        }
        for (si, &i) in sums.iter().enumerate() {
            match (&f.items[i], &mut accs[si]) {
                (Item::Agg(d), Acc::Tally(t)) => {
                    // A count adds one per element that passes its test; a sum adds the column
                    // itself (§15.100). A column with no value leaves the summary with none.
                    let add = match (d.kind, eenv.vals.get(&d.column.text)) {
                        (_, None) => None,
                        (AggKind::Count, v) => Some(if counts_here(v, &d.value) { Rat::int(1) } else { Rat::zero() }),
                        (AggKind::Sum, Some(Val::Num(v))) => Some(*v),
                        _ => None,
                    };
                    *t = match (*t, add) {
                        (Some(t), Some(a)) => Some(t.add(a)),
                        _ => None,
                    };
                }
                (Item::Agg(d), Acc::Extreme { best, missing }) => {
                    // The elements the `where` lets in; a test with no value lets nothing in.
                    let looked = match &d.filter {
                        None => true,
                        Some((w, want)) => counts_here(eenv.vals.get(&w.text), want),
                    };
                    if !looked {
                        continue;
                    }
                    match eenv.vals.get(&d.column.text) {
                        None => *missing = true,
                        Some(v) => {
                            let better = match best.as_ref() {
                                None => true,
                                Some(b) => match order_of(c, &d.column.text, v, b) {
                                    Some(o) => (d.kind == AggKind::Max && o == std::cmp::Ordering::Greater) || (d.kind == AggKind::Min && o == std::cmp::Ordering::Less),
                                    None => false,
                                },
                            };
                            if better {
                                *best = Some(v.clone());
                            }
                        }
                    }
                }
                (Item::Fold(fold), Acc::Fold { taken, held, best, stopped }) => {
                    let v = match eenv.vals.get(&fold.verdict) {
                        Some(Val::Enum(v)) => Some(v.clone()),
                        _ => None,
                    };
                    if observe {
                        obs.verdicts.entry(i).or_default().push(v.clone());
                    }
                    if stopped.is_some() {
                        continue;
                    }
                    let Some(v) = v else { continue };
                    let Some((_, arm, _)) = fold.arms.iter().find(|(n, _, _)| n.text == v) else { continue };
                    match arm {
                        Arm::Next => {}
                        Arm::Stop(None) => {
                            *stopped = Some(None);
                        }
                        Arm::Stop(Some(x)) => {
                            *stopped = Some(eenv.expr(x));
                        }
                        // A take is definite and a keep is provisional, so they do not share a
                        // slot: a provisional value already held is not what `take_unique` is
                        // unique about. Two **takes** are the contradiction; a take after any
                        // number of keeps is the answer.
                        Arm::Take { expr, unique } => {
                            if taken.is_some() {
                                if *unique {
                                    broken.get_or_insert(i);
                                    if !observe {
                                        break 'elements;
                                    }
                                }
                                // take_first: the first one stands and the rest are passed over.
                            } else {
                                *taken = eenv.expr(expr);
                            }
                        }
                        Arm::KeepMax { expr, key } => {
                            let k = eenv.expr(key);
                            let better = match (&*best, &k) {
                                (None, Some(_)) => true,
                                (Some(Val::Num(a)), Some(Val::Num(b))) => b.cmp_to(*a) == std::cmp::Ordering::Greater,
                                _ => false,
                            };
                            if better {
                                *best = k;
                                *held = eenv.expr(expr);
                            }
                        }
                    }
                    if alone && stopped.is_some() && !observe {
                        break 'elements;
                    }
                }
                _ => {}
            }
        }
    }
    if broken.is_some() {
        return Walked { broken, past: None };
    }
    let mut past: Option<String> = None;

    // The walk is over: what each summary answered, under its name.
    for (si, &i) in sums.iter().enumerate() {
        let acc = std::mem::replace(&mut accs[si], Acc::Tally(None));
        match (&f.items[i], acc) {
            (Item::Agg(d), Acc::Tally(t)) => {
                // A total past the top of its range is refused where the generated code refuses
                // it (§15.100).
                let hi = c.ranges.get(&d.name.text).and_then(|(_, hi)| *hi);
                if let (AggKind::Sum, Some(t), Some(hi)) = (d.kind, t, hi) {
                    if t.cmp_to(hi) == std::cmp::Ordering::Greater {
                        past.get_or_insert(d.name.text.clone());
                    }
                }
                if let Some(t) = t {
                    env.vals.insert(d.name.text.clone(), Val::Num(t));
                }
            }
            (Item::Agg(d), Acc::Extreme { best, missing }) => {
                if missing {
                    continue;
                }
                let empty = d.empty.as_ref().and_then(|(l, _)| lit_to_val(l, c.ty_of(&d.column.text).unwrap_or(Ty::Unknown).present()));
                if let Some(v) = best.or(empty) {
                    env.vals.insert(d.name.text.clone(), v);
                }
            }
            (Item::Fold(fold), Acc::Fold { taken, held, stopped, .. }) => {
                let empty_answer = answer_of(&fold.empty, None);
                let answer = if seq.is_empty() {
                    empty_answer
                } else {
                    match stopped {
                        Some(Some(v)) => Some(v),
                        _ => answer_of(&fold.exhausted, taken.or(held).or(empty_answer)),
                    }
                };
                if let (Some(v), Some(name)) = (answer, fold.answer_name(f)) {
                    env.vals.insert(name.to_string(), v);
                }
            }
            _ => {}
        }
    }
    Walked { broken: None, past }
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

/// `run_all`, plus the fired rows as `(table, row)` for a diagnostic's structured half.
#[allow(clippy::type_complexity)]
pub fn run_all_traced(
    f: &RuleFile,
    c: &Checked,
    inputs: HashMap<String, Val>,
) -> (Vec<(String, Option<Val>)>, Vec<String>, Vec<(String, usize)>, HashMap<String, Val>) {
    if f.walks() {
        return run_walking(f, c, inputs);
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
            Item::Agg(_) | Item::Fold(_) => {}
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
            one.insert(col.clone(), lit_to_val(l, ty.present())?);
        }
        rows.push(one);
    }
    Some(Val::Seq(rows))
}

/// The inputs one example row stands for, the sequence included. The cell of the sequence
/// column names a `sequence` block; everything else is a literal of its column's type.
pub fn example_env(f: &RuleFile, c: &Checked, ex: &crate::ast::Table, row: &Row) -> HashMap<String, Val> {
    let mut env: HashMap<String, Val> = HashMap::new();
    let seq_cols: Vec<&String> = f.elements.iter().map(|e| &e.name.text).collect();
    for (ci, (col, _)) in ex.inputs.iter().enumerate() {
        if seq_cols.contains(&col) {
            if let Some(Cell::Lit(Lit::Word(w))) = row.cells.get(ci) {
                if let Some(v) = seq_value(f, c, w) {
                    env.insert(col.clone(), v);
                }
            }
            continue;
        }
        let Some(ty) = c.ty_of(col) else { continue };
        match row.cells.get(ci) {
            // A value of an optional input is read in the type it wraps (§15.201).
            Some(Cell::Lit(l)) => {
                if let Some(v) = lit_to_val(l, ty.present()) {
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

/// The first value of an example that lies outside what the generated code takes at its
/// door: an input, or a field of an element of the sequence, beyond its range — the declared
/// one, or the one a rate has without a declaration. Returned with the value and its type.
fn outside_range(f: &RuleFile, c: &Checked, env: &HashMap<String, Val>) -> Option<(String, Val, Ty)> {
    let doors: Vec<&str> = f
        .inputs
        .iter()
        .chain(f.elements.iter().flat_map(|e| e.fields.iter()))
        .map(|d| d.name.text.as_str())
        .collect();
    let out = |name: &str, v: &Val| -> Option<(String, Val, Ty)> {
        if !doors.contains(&name) {
            return None;
        }
        // An input of an ordered enum held to a range takes the values it lets in (§15.210).
        if let (Val::Enum(e), Some(allowed)) = (v, c.enum_ranges.get(name)) {
            return (!allowed.contains(e)).then(|| (name.to_string(), v.clone(), c.ty_of(name).unwrap_or(Ty::Unknown)));
        }
        let (lo, hi) = c.ranges.get(name)?;
        let x = match v {
            Val::Num(x) => *x,
            Val::Date(y, m, d) => crate::types::date_ord(*y, *m, *d),
            _ => return None,
        };
        let below = lo.is_some_and(|l| x.cmp_to(l) == std::cmp::Ordering::Less);
        let above = hi.is_some_and(|h| x.cmp_to(h) == std::cmp::Ordering::Greater);
        (below || above).then(|| (name.to_string(), v.clone(), c.ty_of(name).unwrap_or(Ty::Unknown)))
    };
    let mut names: Vec<&String> = env.keys().collect();
    names.sort();
    for name in names {
        match &env[name] {
            Val::Seq(elements) => {
                for el in elements {
                    for (k, v) in el {
                        if let Some(hit) = out(k, v) {
                            return Some(hit);
                        }
                    }
                }
            }
            v => {
                if let Some(hit) = out(name, v) {
                    return Some(hit);
                }
            }
        }
    }
    None
}

pub fn check_examples(f: &RuleFile, c: &Checked, path: &str) -> Vec<Diag> {
    // Every section runs, each held to its own header (§15.149).
    f.examples.iter().flat_map(|ex| check_example_table(f, c, path, ex)).collect()
}

fn check_example_table(f: &RuleFile, c: &Checked, path: &str, ex: &Table) -> Vec<Diag> {
    let mut out = Vec::new();
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
                    .note(tr!("実装どうしを照らし合わせても見つからない誤りを見つけられるのは例だけなので、出力は全部書いてください。", "The examples are the only wedge that catches errors the implementations can share, so every output is written."))
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
    // With every column in the header, a row with a cell more than it has columns is a slip
    // of its own; the extra value used to be ignored (§15.156). A header short of a column
    // is E111 above, or E025 for the sequence, and never reaches here.
    for row in &ex.rows {
        let cells = row.cells.len() + row.outs.len();
        let cols = ex.inputs.len() + ex.outputs.len();
        if cells > cols {
            out.push(
                Diag::error("E064", tr!("行が見出しと合いません", "The row does not fit the header"))
                    .at(tr!("{path}:{} 例", "{path}:{} examples", row.span.line))
                    .mark(row.span.clone(), tr!("この行のセルは {cells} 個で、見出しの列は {cols} 個です", "this row has {cells} cells, and the header {cols} columns"))
                    .note(tr!(
                        "一つの列に一つずつセルを書いてください。どの値でもよい列には `-` を書いてください。",
                        "Write one cell per column; a column that takes any value gets `-`."
                    )),
            );
        }
    }
    if out.iter().any(|d| d.code == "E064") {
        return out;
    }

    for row in &ex.rows {
        let env = example_env(f, c, ex, row);
        // The declared range is the door as well: the generated code refuses a value outside
        // it, the elements of a sequence field by field. An example there passed the check,
        // and its vector then failed against the generated code (§15.156).
        if let Some((name, v, ty)) = outside_range(f, c, &env) {
            let (lo, hi) = c.ranges.get(&name).copied().unwrap_or((None, None));
            let end = |x: Option<Rat>| x.map(|x| Val::Num(x).show(&ty)).unwrap_or_else(|| "…".into());
            let shown = v.show(&ty);
            let range = if let Some(allowed) = c.enum_ranges.get(&name) {
                tr!("宣言した範囲（{}）", "the declared range ({})", allowed.join(", "))
            } else if matches!(ty, Ty::Date) {
                tr!("宣言した範囲", "the declared range")
            } else {
                format!(">={} <={}", end(lo), end(hi))
            };
            out.push(
                Diag::error("E019", tr!("例の値が、宣言した範囲の外です", "An example's value is outside the declared range"))
                    .at(tr!("{path}:{} 例", "{path}:{} examples", row.span.line))
                    .mark(row.span.clone(), tr!("{name} = {shown} は {range} の外です", "{name} = {shown} is outside {range}"))
                    .note(tr!(
                        "範囲の外の入力は、生成コードが入口で受け付けません。例はその入力に答えがあると言っていることになり、検査もその入力には行を求めていません。",
                        "The generated code refuses an input outside the range at its door. The example claims an answer for it, and the checks never asked a row to cover it."
                    ))
                    .note(tr!(
                        "例の値を直すか、その値が本当に来るなら範囲のほうを広げてください。",
                        "Correct the example's value, or widen the range if that value really arrives."
                    )),
            );
            continue;
        }
        // A date input of koyomi's days takes those days only (§15.174), and the generated code
        // refuses any other at its door: an example on another day claims an answer for it.
        if let Some((name, shown)) = c.day_sets.iter().find_map(|(name, d)| match env.get(name) {
            Some(v @ Val::Date(y, m, dd)) if d.days.binary_search(&(crate::types::date_ord(*y, *m, *dd).num as i64)).is_err() => Some((name.clone(), v.show(&Ty::Date))),
            _ => None,
        }) {
            let from = c.day_sets[&name].from.as_ref().map(|fr| format!("koyomi \"{}\" date {}", fr.file, fr.date)).unwrap_or_default();
            out.push(
                Diag::error("E019", tr!("例の値が、宣言した範囲の外です", "An example's value is outside the declared range"))
                    .at(tr!("{path}:{} 例", "{path}:{} examples", row.span.line))
                    .mark(row.span.clone(), tr!("{name} = {shown} は {from} がとる日ではありません", "{name} = {shown} is not a day {from} comes to"))
                    .note(tr!(
                        "{name} がとるのは koyomi が数えた日だけで、ほかの日は生成コードが入口で受け付けません。例はその日に答えがあると言っていることになり、検査もその日には行を求めていません。",
                        "{name} takes only the days koyomi counted, and the generated code refuses any other at its door. The example claims an answer for that day, and the checks never asked a row to cover it."
                    ))
                    .note(tr!("例の日付を、koyomi のファイルがとる日に直してください。", "Correct the example's date to a day the koyomi file comes to.")),
            );
            continue;
        }
        // An example is a case the rule is claimed to answer, so it has to be a case the rule
        // can receive. A `constraint` says which combinations exist (§15.55); an example
        // outside them would be asserting an answer for an input the generated code refuses
        // at the door, and the checker never demanded a row for it either.
        {
            let a: std::collections::BTreeMap<String, Val> = env.clone().into_iter().collect();
            if !crate::vectors::allowed(f, &a) {
                for k in &f.constraints {
                    if crate::vectors::constraint_holds(k, &|n| a.get(n)) {
                        continue;
                    }
                    let said = k.said();
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
                                "制約は「この組み合わせは起きない」という宣言で、検査はそれを信じて行を要求していません。生成コードも入口で受け付けません。",
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
            // The expected value sits under the output's own heading, wherever the header put
            // it. Reading it by the output's place in the declaration held `-> y | x` to the
            // wrong column and reported two examples that were right as wrong (§15.148).
            let Some(hi) = ex.outputs.iter().position(|o| o.name.text == od.name.text) else { continue };
            let want = row.outs.get(hi).and_then(|o| match o {
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
                        ..Default::default()
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
                        ..Default::default()
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
    // branches accordingly). A fraction is one of the grid the output is stored on: a whole
    // yen, a whole gram, one step of a rate — 0.5% is no fraction of `rate[step 0.1%]`.
    let scale = f.outputs.first().map(|o| c.wire_scale(&o.name.text)).unwrap_or(1);
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
            if !r.mul(Rat::int(scale)).is_int() {
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
            Item::Agg(_) | Item::Fold(_) => {}
        }
    }
    let out_name = f.outputs.first().map(|o| o.name.text.clone()).unwrap_or_default();
    let v = match &f.result {
        Some(r) => env.expr(&r.expr),
        None => env.vals.get(&out_name).cloned(),
    };
    (v, env.fired)
}
