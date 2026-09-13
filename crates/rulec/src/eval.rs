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
            Cell::Nothing => false,
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
            Expr::Lit(l, _) => {
                // A literal inside an expression is lowered from the unit it is written in
                // to the base unit.
                lit_to_val(l, &Ty::Money { cur: "円".into(), tax: None })
                    .or_else(|| lit_to_val(l, &Ty::Rate))
                    // A literal with no unit at all is a plain number, which is the only
                    // thing `× 2` can mean.
                    .or_else(|| lit_to_val(l, &Ty::Number))
            }
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
    fn table(&mut self, t: &Table) -> Option<usize> {
        let name = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
        let hit = t.rows.iter().position(|row| {
            t.inputs.iter().enumerate().all(|(ci, (col, _))| {
                let (Some(v), Some(ty)) = (self.vals.get(col), self.c.ty_of(col)) else {
                    return false;
                };
                row.cells.get(ci).is_none_or(|cell| self.matches(cell, v, &ty))
            })
        })?;
        self.fired.push(row_tag(&name, hit + 1));
        self.fired_rows.push((name.clone(), hit + 1));
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
pub fn run_all_traced(
    f: &RuleFile,
    c: &Checked,
    inputs: HashMap<String, Val>,
) -> (Vec<(String, Option<Val>)>, Vec<String>, Vec<(String, usize)>, HashMap<String, Val>) {
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
        }
    }
    let mut outs: Vec<(String, Option<Val>)> = Vec::new();
    for (oi, od) in f.outputs.iter().enumerate() {
        let name = od.name.text.clone();
        let mut v = match (&f.result, oi) {
            (Some(r), 0) => env.expr(&r.expr),
            _ => env.vals.get(&name).cloned(),
        };
        // The rounding declared on the output applies last — §7.2, "an unrounded value
        // never reaches the output".
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
    (outs, env.fired, env.fired_rows, env.vals)
}

/// The match test for a single cell, used from outside the table (the coverage check of §9.2).
pub fn cell_matches(c: &Checked, cell: &Cell, v: &Val, ty: &Ty) -> bool {
    Env::new(c, HashMap::new()).matches(cell, v, ty)
}

/// The `examples` section is executable specification (§1.2). A miss is E107, reported with
/// the fired rows.
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
                    .note(tr!("例は実装どうしの照合では捕まらない誤りを捕まえる唯一の楔なので、出力は全部書きます。", "The examples are the only wedge that catches errors the implementations can share, so every output is written."))
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
        let mut env: HashMap<String, Val> = HashMap::new();
        for (ci, (col, _)) in ex.inputs.iter().enumerate() {
            let Some(ty) = c.ty_of(col) else { continue };
            if let Some(Cell::Lit(l)) = row.cells.get(ci) {
                if let Some(v) = lit_to_val(l, &ty) {
                    env.insert(col.clone(), v);
                }
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
                    .note(tr!("発火した行: {}", "Fired rows: {}", fired.join(" / ")))
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
                        tr!("どの表も発火しませんでした。", "No table fired.")
                    } else {
                        tr!("発火した行: {}", "Fired rows: {}", fired.join(" / "))
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
        }
    }
    let out_name = f.outputs.first().map(|o| o.name.text.clone()).unwrap_or_default();
    let v = match &f.result {
        Some(r) => env.expr(&r.expr),
        None => env.vals.get(&out_name).cloned(),
    };
    (v, env.fired)
}
