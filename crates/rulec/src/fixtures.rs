//! Records of past data (§10.2) and their validation.
//!
//! Extracting production logs into this shape (ETL) is deliberately the user's job; what
//! rulec takes on is only the validation of types and ranges. The format is fixed to JSONL,
//! one record per line.
//!
//! ```text
//! {"ts":"2025-08-14T09:12:33+09:00","tag":"order:1234567",
//!  "in":{"届け先":"鹿児島県","重量":800,"注文金額":4200,"会員":"一般"},
//!  "observed":{"送料":800}}
//! ```
//!
//! **Fixtures do not go into the repository.** They contain order amounts, so hand them to CI
//! as an artifact or through protected storage (§10.2).

use crate::ast::RuleFile;
use crate::eval::Val;
use crate::json::Json;
use crate::num::Rat;
use crate::types::{Checked, Ty};
use std::collections::BTreeMap;

/// One record. `in` has been resolved into the form the evaluator accepts.
pub struct Record {
    pub line: usize,
    pub tag: String,
    pub ts: String,
    pub input: BTreeMap<String, Val>,
    /// The observed outputs, looked up by name rather than by declaration order (the order
    /// on the record's side is not relied on).
    pub observed: BTreeMap<String, Val>,
    /// The fields filled in with default values. If non-empty, this record is a "filled
    /// record" (§10.3).
    pub filled: Vec<String>,
    /// The rows that matched when the record was made, as `(table, 1-based row)`, when the
    /// record carries them. The generated code's record function writes them (§15.35); a
    /// record made by hand may leave them out, and then this is empty.
    pub trace: Vec<(String, usize)>,
    /// The label each entry of `trace` carried, when it did. A row keeps its label when a row
    /// is inserted above it, so `replay` matches labelled rows by label rather than by number.
    pub trace_labels: Vec<Option<String>>,
}

pub struct Problem {
    pub line: usize,
    pub tag: String,
    /// A stable identifier for the kind of problem, for `--format json`. It never changes
    /// with `--lang`, which `what` and `hint` do (docs/formats.md).
    pub kind: &'static str,
    /// The field of the record at fault, empty when the problem is about the whole record.
    pub field: String,
    pub what: String,
    pub hint: String,
}

pub struct Load {
    pub records: Vec<Record>,
    pub problems: Vec<Problem>,
    /// How many records were excluded outright because a field was missing (mode 1 of
    /// §10.3).
    pub dropped: usize,
    /// The observed values of the records read that lie outside what the rule can produce
    /// (§15.199). Not a problem of the form — the records are read and `replay` counts them as
    /// mismatches — but what a record written before a step or a unit changed looks like.
    pub beyond: Vec<Beyond>,
}

/// One observed value outside what the rule can produce (§15.199).
pub struct Beyond {
    pub line: usize,
    pub tag: String,
    /// The output.
    pub field: String,
    /// What the rule can produce, as the rule writes values (`60万円..2000万円`).
    pub range: String,
}

impl Load {
    /// The number of observed records (records with nothing filled in). The headline
    /// agreement rate is computed from these alone.
    pub fn measured(&self) -> usize {
        self.records.iter().filter(|r| r.filled.is_empty()).count()
    }
    pub fn filled(&self) -> usize {
        self.records.len() - self.measured()
    }
}

/// Convert a JSON value into the `Val` of the declared type. On the wire, numbers are
/// integers in the canonical unit (§10.1).
pub fn to_val(j: &Json, ty: &Ty, c: &Checked, name: &str) -> Result<Val, String> {
    to_val_as(j, ty, c, name, None)
}

/// [`to_val`], with the integer read the way `then` — another version of the rule — wrote
/// it: at that version's step and in its unit, and brought to this one's (§15.145). A record
/// carries no step and no unit of its own, so one written before either changed can only be
/// read by naming the version that wrote it.
pub fn to_val_as(j: &Json, ty: &Ty, c: &Checked, name: &str, then: Option<&Checked>) -> Result<Val, String> {
    let inner = match ty {
        Ty::Opt(t) => {
            if *j == Json::Null {
                return Ok(Val::Enum(crate::kw::NONE.into()));
            }
            (**t).clone()
        }
        other => other.clone(),
    };
    match (&inner, j) {
        (Ty::Enum(en), Json::Str(s)) => {
            let vs = c.enums.get(en).cloned().unwrap_or_default();
            if vs.iter().any(|v| v == s) {
                Ok(Val::Enum(s.clone()))
            } else {
                Err(tr!("`{s}` は列挙 {en} の値ではありません", "`{s}` is not a value of enum {en}"))
            }
        }
        (Ty::Bool, Json::Bool(b)) => Ok(Val::Bool(*b)),
        (Ty::Str, Json::Str(s)) => Ok(Val::Str(s.clone())),
        (Ty::Date, Json::Str(s)) => {
            let p: Vec<&str> = s.split('-').collect();
            let ok = p.len() == 3 && p[0].len() == 4 && p[1].len() == 2 && p[2].len() == 2;
            let n: Option<Vec<i64>> = p.iter().map(|x| x.parse::<i64>().ok()).collect();
            match (ok, n) {
                (true, Some(v)) if (1..=12).contains(&v[1]) && (1..=31).contains(&v[2]) => {
                    Ok(Val::Date(v[0] as i32, v[1] as u32, v[2] as u32))
                }
                _ => Err(tr!("`{s}` は YYYY-MM-DD の日付ではありません", "`{s}` is not a YYYY-MM-DD date")),
            }
        }
        (Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number, Json::Int(n)) => {
            // The wire carries an integer in the canonical unit; a rate carries a count of
            // steps (§10.2). The range was declared in true values, so convert first.
            let v = match then {
                Some(t) => read_as(Rat::int(*n), name, &inner, t)?,
                None => crate::types::from_wire(*n, c.wire_scale(name)),
            };
            if let Some((lo, hi)) = c.ranges.get(name) {
                if lo.is_some_and(|l| v.cmp_to(l) == std::cmp::Ordering::Less)
                    || hi.is_some_and(|h| v.cmp_to(h) == std::cmp::Ordering::Greater)
                {
                    // The range is written the way the rule writes it, and so is what the
                    // integer stands for: a rate's range used to come out as `0.01..0.03`
                    // beside a count of steps, two scales in one sentence.
                    let show = |b: &Option<Rat>| b.map(|x| crate::types::fmt_val(x, &inner)).unwrap_or_else(|| "…".into());
                    let meant = crate::types::fmt_val(v, &inner);
                    let n = if meant == n.to_string() { tr!("{n} ", "{n}") } else { tr!("{n}（{meant}）", "{n} ({meant})") };
                    return Err(tr!(
                        "{n}は宣言した範囲 {}..{} の外です",
                        "{n} is outside the declared range {}..{}",
                        show(lo),
                        show(hi)
                    ));
                }
            }
            Ok(Val::Num(v))
        }
        (_, got) => Err(tr!("{} を期待しましたが {} でした", "expected {}, found {}", ty_word(&inner), crate::json::kind(got))),
    }
}

/// A number another version of the rule wrote, read at that version's step and in its unit,
/// then brought to the unit `now` counts in. `円` and `銭` are one dimension and convert; two
/// currencies, or a rate and an amount, do not. The number is what the record holds — an
/// integer, or for an output a decimal (§15.199).
fn read_as(w: Rat, name: &str, now: &Ty, then: &Checked) -> Result<Rat, String> {
    let was = match then.ty_of(name) {
        Some(Ty::Opt(t)) => *t,
        Some(t) => t,
        None => return Err(tr!("読む版の規則に {name} がありません", "the version the records are read as has no {name}")),
    };
    let v = w.div(Rat::int(then.wire_scale(name)));
    convert(v, &was, now).ok_or_else(|| {
        tr!(
            "読む版の {name} は {was} で、いまは {now} なので、値を移せません",
            "{name} is {was} in the version the records are read as and {now} now, and a value does not carry across"
        )
    })
}

/// `v`, counted in `from`'s unit, counted in `to`'s, exactly (§15.199): money in another
/// spelling or subunit of its currency (yen and sen, `JPY` and `円`), a quantity in another unit
/// of its dimension (kilograms and grams, ℃ and ℉ — the offset as well as the factor). The tax
/// brand of money does not change the amount. A rate and a number are what they are. None
/// between two kinds of value, two currencies or two dimensions.
pub fn convert(v: Rat, from: &Ty, to: &Ty) -> Option<Rat> {
    let bare = |t: &Ty| match t {
        Ty::Opt(t) => (**t).clone(),
        other => other.clone(),
    };
    let unit = |t: &Ty| match t {
        Ty::Money { cur, .. } => ritsu_units::Unit::money(cur, None),
        Ty::Qty { dim, unit } => ritsu_units::Unit::quantity(dim, unit),
        _ => None,
    };
    match (bare(from), bare(to)) {
        (Ty::Rate, Ty::Rate) | (Ty::Number, Ty::Number) => Some(v),
        (a, b) => unit(&a)?.convert(v, &unit(&b)?),
    }
}

/// A decimal as the JSON reader keeps it (`123.4`, `-0.5`), as the exact rational it is.
fn decimal(text: &str) -> Option<Rat> {
    let (int, frac) = text.split_once('.')?;
    let den = 10i128.checked_pow(frac.len() as u32)?;
    let num = format!("{int}{frac}").parse::<i128>().ok()?;
    Rat::checked_new(num, den)
}

/// An output's value as a record holds it, or as an implementation answered it (§15.199). The
/// type, the enum's value and the unit are held to the declaration, as an input's are; what
/// the rule can produce is not. It is not a declaration of the record's: an implementation
/// that answered outside it, or finer than the output's step, disagrees with the rule, and
/// `replay` and `verify` count that as a mismatch rather than leave the record out. A number
/// may carry decimals for the same reason — 12.34% where the rule's step is 0.1% is `123.4` —
/// and is read exactly, never at the step.
pub fn observed_as(j: &Json, ty: &Ty, c: &Checked, name: &str, then: Option<&Checked>) -> Result<Val, String> {
    let inner = match ty {
        Ty::Opt(_) if *j == Json::Null => return Ok(Val::Enum(crate::kw::NONE.into())),
        Ty::Opt(t) => (**t).clone(),
        other => other.clone(),
    };
    let wire = match j {
        Json::Int(n) => Some(Rat::int(*n)),
        Json::Frac(s) => decimal(s),
        _ => None,
    };
    match (&inner, wire) {
        (Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number, Some(w)) => Ok(Val::Num(match then {
            Some(t) => read_as(w, name, &inner, t)?,
            None => w.div(Rat::int(c.wire_scale(name))),
        })),
        (Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number, None) => Err(tr!(
            "決まった単位の数（整数か小数）を期待しましたが {} でした",
            "expected a number in the canonical unit (whole or decimal), found {}",
            crate::json::kind(j)
        )),
        _ => to_val_as(j, ty, c, name, then),
    }
}

/// What the rule can produce for an output, when its value lies outside it: the range as the
/// rule writes values (§15.199). It is computed, not declared, so a value outside it is no
/// fault of the record's form.
pub fn beyond_reach(c: &Checked, name: &str, v: &Val) -> Option<String> {
    let Val::Num(x) = v else { return None };
    let (lo, hi) = c.ranges.get(name)?;
    let below = lo.is_some_and(|l| x.cmp_to(l) == std::cmp::Ordering::Less);
    let above = hi.is_some_and(|h| x.cmp_to(h) == std::cmp::Ordering::Greater);
    let ty = match c.ty_of(name)? {
        Ty::Opt(t) => *t,
        t => t,
    };
    (below || above).then(|| format!("{}..{}", end(*lo, &ty), end(*hi, &ty)))
}

/// Why the generated code's entry turns an input away (§15.197). `fixtures lint` reports it
/// against the version that reads the records; `diff` reports it as the new version's refusal
/// of an input the old version took. The shape is data, and the two word it each their own way.
#[derive(Debug, Clone)]
pub enum Shut {
    /// A number or a date outside the declared range. The ends are written the way the rule
    /// writes them.
    Range { name: String, value: String, lo: String, hi: String },
    /// A value the enum does not have.
    EnumValue { name: String, value: String, en: String },
    /// `null` for an input that is not optional.
    NotOptional { name: String },
    /// A value that does not carry into the other version's type: another kind of value,
    /// another currency, another dimension.
    Type { name: String, was: String, now: String },
    /// A number that is not a whole count of the other version's step or unit.
    Step { name: String, ty: String },
    /// A date that is not one of the days a koyomi date comes to.
    Day { name: String, value: String, from: String },
    /// A `constraint` that does not hold.
    Constraint { said: String },
}

impl Shut {
    /// A stable identifier for `--format json`: the same words `diff` without records uses for
    /// what a version accepts, where there is one.
    pub fn kind(&self) -> &'static str {
        match self {
            Shut::Range { .. } => "input_range",
            Shut::EnumValue { .. } => "enum_value",
            Shut::NotOptional { .. } | Shut::Type { .. } => "input_type",
            Shut::Step { .. } => "input_step",
            Shut::Day { .. } => "input_day",
            Shut::Constraint { .. } => "constraint",
        }
    }

    /// The input at fault; empty for a constraint, which is about two of them.
    pub fn field(&self) -> &str {
        match self {
            Shut::Range { name, .. }
            | Shut::EnumValue { name, .. }
            | Shut::NotOptional { name }
            | Shut::Type { name, .. }
            | Shut::Step { name, .. }
            | Shut::Day { name, .. } => name,
            Shut::Constraint { .. } => "",
        }
    }

    /// The reason as `diff` gives it, about the new version. The record's own value is left out
    /// where it varies from record to record, so the records refused for one reason read as
    /// one line; an enum's value stays, because the values are few and each is its own reason.
    pub fn refusal(&self) -> String {
        match self {
            Shut::Range { name, lo, hi, .. } => tr!("{name} が新しい版の範囲 {lo}..{hi} の外です", "{name} is outside the new version's range {lo}..{hi}"),
            Shut::EnumValue { name, value, en } => tr!("{name} = {value} は、新しい版の列挙 {en} の値ではありません", "{name} = {value} is not a value of the new version's enum {en}"),
            Shut::NotOptional { name } => tr!("{name} が none（null）ですが、新しい版の {name} は optional ではありません", "{name} is none (null), and the new version's {name} is not optional"),
            Shut::Type { name, was, now } => tr!("{name} は旧版では {was}、新しい版では {now} で、値を移せません", "{name} is {was} in the old version and {now} in the new one, and a value does not carry across"),
            Shut::Step { name, ty } => tr!("{name} の値が、新しい版の {ty} の刻みに載りません", "{name} does not sit on the step of the new version's {ty}"),
            Shut::Day { name, from, .. } => tr!("{name} が、新しい版の {from} がとる日ではありません", "{name} is not a day the new version's {from} comes to"),
            Shut::Constraint { said } => tr!("新しい版の制約 `{said}` が成り立ちません", "the new version's constraint `{said}` does not hold"),
        }
    }
}

/// One end of a range, the way the rule writes it: an amount in its unit, a rate in percent, a
/// date as a date. `…` for an end that is open.
fn end(x: Option<Rat>, ty: &Ty) -> String {
    match x {
        None => "…".into(),
        Some(v) if *ty == Ty::Date => date_text(v),
        Some(v) => crate::types::fmt_val(v, ty),
    }
}

fn date_text(ord: Rat) -> String {
    let (y, m, d) = crate::types::ord_to_date(ord);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Whether one `constraint` holds for these inputs. Numbers and dates are what it compares
/// (E018); an input that is not there is not held to it, the way the generated code compares
/// only what it was passed.
fn holds(k: &crate::ast::Constraint, a: &BTreeMap<String, Val>) -> bool {
    let num = |v: Option<&Val>| match v {
        Some(Val::Num(x)) => Some(*x),
        Some(Val::Date(y, m, d)) => Some(crate::types::date_ord(*y, *m, *d)),
        _ => None,
    };
    let (Some(x), Some(y)) = (num(a.get(&k.left)), num(a.get(&k.right))) else { return true };
    let o = x.cmp_to(y);
    match k.op {
        crate::ast::CmpOp::Le => o != std::cmp::Ordering::Greater,
        crate::ast::CmpOp::Lt => o == std::cmp::Ordering::Less,
        crate::ast::CmpOp::Ge => o != std::cmp::Ordering::Less,
        crate::ast::CmpOp::Gt => o == std::cmp::Ordering::Greater,
    }
}

/// What the generated code's entry turns away besides one number's type and range, which
/// [`to_val`] holds a value to as it reads it: a date outside its declared range, a date that
/// is not one of the days a koyomi date comes to, and a combination a `constraint` rules out
/// (§15.197). Those need the whole input, or a date's place among the days. `None` when the
/// entry lets the input through.
pub fn door(f: &RuleFile, c: &Checked, input: &BTreeMap<String, Val>) -> Option<Shut> {
    for i in &f.inputs {
        let name = &i.name.text;
        let Some(Val::Date(y, m, d)) = input.get(name) else { continue };
        let ord = crate::types::date_ord(*y, *m, *d);
        let value = format!("{y:04}-{m:02}-{d:02}");
        if let Some((lo, hi)) = c.ranges.get(name) {
            let below = lo.is_some_and(|l| ord.cmp_to(l) == std::cmp::Ordering::Less);
            let above = hi.is_some_and(|h| ord.cmp_to(h) == std::cmp::Ordering::Greater);
            if below || above {
                return Some(Shut::Range { name: name.clone(), value, lo: end(*lo, &Ty::Date), hi: end(*hi, &Ty::Date) });
            }
        }
        if let Some(days) = c.day_sets.get(name) {
            if days.days.binary_search(&(ord.num as i64)).is_err() {
                let from = days.from.as_ref().map(|fr| format!("koyomi \"{}\" date {}", fr.file, fr.date)).unwrap_or_default();
                return Some(Shut::Day { name: name.clone(), value, from });
            }
        }
    }
    f.constraints
        .iter()
        .find(|k| !holds(k, input))
        .map(|k| Shut::Constraint { said: format!("{} {} {}", k.left, k.op.word(), k.right) })
}

/// A value of one version's input, as another version of the rule takes it (§15.197). `diff`
/// reads a record the way the old version reads it and hands its inputs to the new one. Money
/// and a quantity are brought to the new unit when the dimension is the same. A value that
/// does not land on a whole count of the new step or unit, that is outside the new range, or
/// that the new enum does not have is turned away, with why; so is one of another kind.
pub fn carry(v: &Val, name: &str, from: &Checked, to: &Checked) -> Result<Val, Shut> {
    let was = from.ty_of(name).unwrap_or(Ty::Unknown);
    let now = to.ty_of(name).unwrap_or(Ty::Unknown);
    let bare = |t: &Ty| match t {
        Ty::Opt(t) => (**t).clone(),
        other => other.clone(),
    };
    let (was_in, now_in) = (bare(&was), bare(&now));
    // The `none` of an optional travels as `null` and reads back as this word (`to_val`).
    if matches!(was, Ty::Opt(_)) && *v == Val::Enum(crate::kw::NONE.into()) {
        return if matches!(now, Ty::Opt(_)) { Ok(v.clone()) } else { Err(Shut::NotOptional { name: name.into() }) };
    }
    let other_kind = || Shut::Type { name: name.into(), was: was_in.to_string(), now: now_in.to_string() };
    match (v, &now_in) {
        (Val::Enum(s), Ty::Enum(en)) if matches!(was_in, Ty::Enum(_)) => {
            if to.enums.get(en).is_some_and(|vs| vs.iter().any(|x| x == s)) {
                Ok(v.clone())
            } else {
                Err(Shut::EnumValue { name: name.into(), value: s.clone(), en: en.clone() })
            }
        }
        (Val::Bool(_), Ty::Bool) | (Val::Str(_), Ty::Str) | (Val::Date(..), Ty::Date) => Ok(v.clone()),
        (Val::Num(x), Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number) => {
            // The same amount in the new unit (as `read_as`), the offset of ℉ included.
            let Some(y) = convert(*x, &was_in, &now_in) else { return Err(other_kind()) };
            // The wire carries a whole number of the new step: 1.5円 has no integer in yen.
            let scale = to.wire_scale(name);
            if !y.mul(Rat::int(scale)).is_int() {
                let ty = match now_in {
                    Ty::Rate => format!("{}[step {}]", crate::kw::RATE, crate::types::fmt_val(Rat::new(1, scale), &Ty::Rate)),
                    ref t => t.to_string(),
                };
                return Err(Shut::Step { name: name.into(), ty });
            }
            if let Some((lo, hi)) = to.ranges.get(name) {
                let below = lo.is_some_and(|l| y.cmp_to(l) == std::cmp::Ordering::Less);
                let above = hi.is_some_and(|h| y.cmp_to(h) == std::cmp::Ordering::Greater);
                if below || above {
                    return Err(Shut::Range { name: name.into(), value: crate::types::fmt_val(y, &now_in), lo: end(*lo, &now_in), hi: end(*hi, &now_in) });
                }
            }
            Ok(Val::Num(y))
        }
        _ => Err(other_kind()),
    }
}

fn ty_word(ty: &Ty) -> String {
    match ty {
        Ty::Enum(e) => tr!("列挙 {e} の値（文字列）", "value of enum {e} (string)"),
        Ty::Bool => crate::kw::BOOL.into(),
        Ty::Date => tr!("日付（YYYY-MM-DD の文字列）", "date (YYYY-MM-DD string)"),
        Ty::Str => tr!("文字列", "string"),
        Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number => tr!("決まった単位の整数", "integer in the canonical unit"),
        _ => format!("{ty}"),
    }
}

/// The replay manifest (§10.3): a small JSON file placed next to the fixtures. **It holds
/// nothing but default values and the fields they apply to, so it contains nothing sensitive
/// and goes into the repository.**
///
/// ```json
/// {"rulec":"replay/1","rule":"送料","fills":{"会員":"一般"}}
/// ```
///
/// Default values are not baked into the rule itself because the rule is a pure function and
/// filling in is **a decision of one particular replay experiment**. Running "fill 会員 with
/// 一般" and "fill it with ゴールド to see the upper bound of the impact" separately against
/// the same rule is a legitimate use, and baking one value into the rule makes that
/// impossible. There is also the harm that an annotation with no bearing on the semantics
/// changes the rule's hash and makes `gen --check` demand a pointless regeneration.
///
/// Honesty is guaranteed not by "where the justification is written" but by "**whether the
/// justification appears where the numbers appear**": the report always stamps the default
/// values used and the number of records filled in per field.
#[derive(Default)]
pub struct Manifest {
    /// Field → default value. A record missing a field that is not listed here is excluded
    /// outright rather than filled in.
    pub fills: BTreeMap<String, Val>,
    /// `diff` only: the default values of the inputs only the new version takes, at the new
    /// version's types (§15.197). The records were written without them, so a record the new
    /// version answers with one is a filled record.
    pub fills_new: BTreeMap<String, Val>,
    /// The spelling exactly as written, for the stamp in the report.
    pub shown: BTreeMap<String, String>,
}

/// The version a default value belongs to: the one the records are read as, or — for `diff`
/// — the new version, when only it has the input.
fn owner<'a>(name: &str, read: (&'a RuleFile, &'a Checked), new: Option<(&'a RuleFile, &'a Checked)>) -> Option<(&'a Checked, bool)> {
    let has = |f: &RuleFile| f.inputs.iter().any(|i| i.name.text == name);
    if has(read.0) {
        Some((read.1, false))
    } else {
        new.filter(|(f, _)| has(f)).map(|(_, c)| (c, true))
    }
}

impl Manifest {
    /// Read the `会員=一般` form (a one-off override for sensitivity analysis, §10.3).
    pub fn add(&mut self, spec: &str, f: &RuleFile, c: &Checked) -> Result<(), String> {
        self.add_to(spec, (f, c), None)
    }

    /// [`Manifest::add`] for `diff`, whose records are read as the old version reads them: a
    /// field the old version has is read at its types, one only the new version has at the new
    /// version's (§15.197).
    pub fn add_two(&mut self, spec: &str, old: (&RuleFile, &Checked), new: (&RuleFile, &Checked)) -> Result<(), String> {
        self.add_to(spec, old, Some(new))
    }

    fn add_to(&mut self, spec: &str, read: (&RuleFile, &Checked), new: Option<(&RuleFile, &Checked)>) -> Result<(), String> {
        let (name, text) = spec.split_once('=').ok_or_else(|| {
            tr!("`{spec}` は `フィールド=値` の形ではありません", "`{spec}` is not of the form `field=value`")
        })?;
        let (name, text) = (name.trim(), text.trim());
        let Some((c, only_new)) = owner(name, read, new) else {
            return Err(tr!("`{name}` は規則の入力ではありません", "`{name}` is not an input of the rule"));
        };
        let ty = c.ty_of(name).unwrap_or(Ty::Unknown);
        // Quantities, money, and rates are read as integers; everything else as a string.
        let j = match &ty {
            Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number => text
                .parse::<i128>()
                .map(Json::Int)
                .map_err(|_| tr!("`{name}` は決まった単位の整数で書いてください", "`{name}` must be written as an integer in the canonical unit"))?,
            Ty::Bool => match text {
                crate::kw::TRUE => Json::Bool(true),
                crate::kw::FALSE => Json::Bool(false),
                _ => return Err(tr!("`{name}` は {} か {} です", "`{name}` is either {} or {}", crate::kw::TRUE, crate::kw::FALSE)),
            },
            _ => Json::Str(text.into()),
        };
        let v = to_val(&j, &ty, c, name).map_err(|e| format!("`{name}`: {e}"))?;
        self.shown.insert(name.into(), text.into());
        self.put(name, v, only_new);
        Ok(())
    }

    fn put(&mut self, name: &str, v: Val, only_new: bool) {
        if only_new {
            self.fills_new.insert(name.into(), v);
        } else {
            self.fills.insert(name.into(), v);
        }
    }

    /// Read the manifest JSON.
    pub fn load(src: &str, f: &RuleFile, c: &Checked) -> Result<Manifest, String> {
        Manifest::load_to(src, (f, c), None)
    }

    /// [`Manifest::load`] for `diff`, read the way [`Manifest::add_two`] reads one field.
    pub fn load_two(src: &str, old: (&RuleFile, &Checked), new: (&RuleFile, &Checked)) -> Result<Manifest, String> {
        Manifest::load_to(src, old, Some(new))
    }

    fn load_to(src: &str, read: (&RuleFile, &Checked), new: Option<(&RuleFile, &Checked)>) -> Result<Manifest, String> {
        let f = read.0;
        let j = crate::json::parse(src.trim()).map_err(|e| tr!("マニフェストが読めません: {e}", "Cannot read the manifest: {e}"))?;
        if let Some(r) = j.get("rule").and_then(|x| x.as_str()) {
            if r != f.name.text {
                return Err(tr!(
                    "マニフェストは規則 `{r}` のものです（いま見ているのは `{}`）",
                    "The manifest belongs to rule `{r}` (the current rule is `{}`)",
                    f.name.text
                ));
            }
        }
        let mut m = Manifest::default();
        let Some(fills) = crate::json::members_of(&j, "fills") else {
            return Ok(m);
        };
        for (name, v) in fills {
            let Some((c, only_new)) = owner(name, read, new) else {
                return Err(tr!("`{name}` は規則の入力ではありません", "`{name}` is not an input of the rule"));
            };
            let ty = c.ty_of(name).unwrap_or(Ty::Unknown);
            let val = to_val(v, &ty, c, name).map_err(|e| tr!("既定値 `{name}`: {e}", "default value `{name}`: {e}"))?;
            m.shown.insert(name.to_string(), crate::json::show(v));
            m.put(name, val, only_new);
        }
        Ok(m)
    }
}

/// Read and validate the JSONL. **Broken records are reported, not discarded.** Dropping
/// them silently shrinks the denominator, which makes the agreement rate look higher (§10.3).
pub fn load(src: &str, f: &RuleFile, c: &Checked, m: &Manifest) -> Load {
    load_as(src, f, c, m, None)
}

/// [`load`], with every number read the way `then` — the version that wrote the records —
/// wrote it (§15.145).
pub fn load_as(src: &str, f: &RuleFile, c: &Checked, m: &Manifest, then: Option<&Checked>) -> Load {
    read(src, f, c, m, then, true)
}

/// [`load_as`], reading what `diff` uses and nothing else: the inputs, and `tag` and `ts`
/// (§15.197). Two versions' answers to the same input are compared, so what came out at the
/// time plays no part; `observed` and `trace` are neither required nor read here. Holding
/// them to a version is `fixtures lint`'s work, against the version that wrote the records.
pub fn load_inputs(src: &str, f: &RuleFile, c: &Checked, m: &Manifest, then: Option<&Checked>) -> Load {
    read(src, f, c, m, then, false)
}

/// `whole` reads `observed` and `trace` as well.
fn read(src: &str, f: &RuleFile, c: &Checked, m: &Manifest, then: Option<&Checked>, whole: bool) -> Load {
    let mut out = Load { records: Vec::new(), problems: Vec::new(), dropped: 0, beyond: Vec::new() };
    for (li, raw) in src.lines().enumerate() {
        let line = li + 1;
        if raw.trim().is_empty() {
            continue;
        }
        let j = match crate::json::parse(raw) {
            Ok(j) => j,
            Err(e) => {
                out.problems.push(Problem {
                    line,
                    tag: String::new(),
                    kind: "not_json",
                    field: String::new(),
                    what: tr!("JSON として読めません: {e}", "Not readable as JSON: {e}"),
                    hint: tr!("1 件 1 行の JSON Lines です。", "The format is JSON Lines, one record per line."),
                });
                continue;
            }
        };
        let tag = j.get("tag").and_then(|x| x.as_str()).unwrap_or_default().to_string();
        let ts = j.get("ts").and_then(|x| x.as_str()).unwrap_or_default().to_string();
        let mut bad = |kind: &'static str, field: &str, what: String, hint: &str| {
            out.problems.push(Problem {
                line,
                tag: tag.clone(),
                kind,
                field: field.to_string(),
                what,
                hint: hint.into(),
            });
        };

        let Some(ins) = crate::json::members_of(&j, "in") else {
            bad("no_in", "", tr!("`in` がありません", "`in` is missing"), &tr!("入力は、`in` の下に規則の名前（別名ではないほう）で置いてください。", "Inputs go under `in`, keyed by the names used in the rule."));
            continue;
        };
        let obs = crate::json::members_of(&j, "observed");
        if whole && obs.is_none() {
            bad("no_observed", "", tr!("`observed` がありません", "`observed` is missing"), &tr!("そのとき実際に出た値を `observed` に置いてください。", "Put the values that actually came out at the time under `observed`."));
            continue;
        }

        // A field the rule does not know is reported as an error. Discarding it silently turns
        // a spelling mistake into "filled in with the default value", and only the agreement
        // rate moves.
        let known: Vec<&str> = f.inputs.iter().map(|i| i.name.text.as_str()).collect();
        let mut extra: Vec<&str> = ins.keys().copied().filter(|k| !known.contains(k)).collect();
        extra.sort();
        if !extra.is_empty() {
            bad(
                "unknown_field",
                extra[0],
                tr!("`in` に規則が知らないフィールドがあります: {}", "`in` has fields the rule does not know: {}", extra.join(", ")),
                &tr!("規則の入力の名前（別名ではないほう）と綴りを合わせてください。", "Match the spelling of the rule's input names."),
            );
            continue;
        }

        let mut input: BTreeMap<String, Val> = BTreeMap::new();
        let mut filled: Vec<String> = Vec::new();
        let mut broken = false;
        for i in &f.inputs {
            let name = &i.name.text;
            let ty = c.ty_of(name).unwrap_or(Ty::Unknown);
            match ins.get(name.as_str()) {
                Some(v) => match to_val_as(v, &ty, c, name, then) {
                    Ok(v) => {
                        input.insert(name.clone(), v);
                    }
                    Err(e) => {
                        bad("bad_input", name, format!("`in.{name}`: {e}"), &tr!("型か範囲が宣言と食い違っています。", "The type or range disagrees with the declaration."));
                        broken = true;
                    }
                },
                // §10.3: there are only two modes per field. If a default value is declared,
                // fill it in and label the record "filled"; if not, **exclude the whole
                // record**. No inference backwards. A missing field is an absent key, while
                // the `none` of an optional is `null` ("known to be absent" is not something
                // to fill in).
                None => match m.fills.get(name) {
                    Some(v) => {
                        input.insert(name.clone(), v.clone());
                        filled.push(name.clone());
                    }
                    None => {
                        broken = true;
                        out.dropped += 1;
                        break;
                    }
                },
            }
        }
        if broken {
            continue;
        }
        // The rest of what the generated code's entry turns away (§15.197). A number outside
        // its range has always been a problem of the record; a date outside its range, a day
        // the koyomi date does not come to and a combination a `constraint` rules out are the
        // same entry's declarations, and the rule answers none of them.
        if let Some(s) = door(f, c, &input) {
            let range = tr!("型か範囲が宣言と食い違っています。", "The type or range disagrees with the declaration.");
            let (what, hint) = match &s {
                Shut::Range { name, value, lo, hi } => (
                    tr!("`in.{name}`: {value} は宣言した範囲 {lo}..{hi} の外です", "`in.{name}`: {value} is outside the declared range {lo}..{hi}"),
                    range,
                ),
                Shut::Day { name, value, from } => (
                    tr!("`in.{name}`: {value} は {from} がとる日ではありません", "`in.{name}`: {value} is not a day {from} comes to"),
                    range,
                ),
                Shut::Constraint { said } => (
                    tr!("`in`: 制約 `{said}` が成り立ちません", "`in`: the constraint `{said}` does not hold"),
                    tr!(
                        "制約は、その組み合わせが起きないという宣言です。記録の値か、規則の制約を確かめてください。",
                        "A constraint declares that the combination does not happen; check the record's values, or the rule's constraint."
                    ),
                ),
                // `door` says nothing else; the rest is what `carry` says about another version.
                other => (format!("`in.{}`: {}", other.field(), other.refusal()), range),
            };
            bad("bad_input", s.field(), what, &hint);
            continue;
        }

        let mut observed: BTreeMap<String, Val> = BTreeMap::new();
        let obs = match obs {
            Some(obs) if whole => obs,
            _ => {
                out.records.push(Record { line, tag, ts, input, observed, filled, trace: Vec::new(), trace_labels: Vec::new() });
                continue;
            }
        };
        // The outputs whose value is outside what the rule can produce: kept, and counted
        // once the record is read (§15.199).
        let mut beyond: Vec<(String, String)> = Vec::new();
        for o in &f.outputs {
            let name = &o.name.text;
            let ty = c.ty_of(name).unwrap_or(Ty::Unknown);
            match obs.get(name.as_str()) {
                Some(v) => match observed_as(v, &ty, c, name, then) {
                    Ok(v) => {
                        if let Some(range) = beyond_reach(c, name, &v) {
                            beyond.push((name.clone(), range));
                        }
                        observed.insert(name.clone(), v);
                    }
                    Err(e) => {
                        bad("bad_observed", name, format!("`observed.{name}`: {e}"), &tr!("そのとき出た値を、決まった単位で書いてください。", "Write the value that came out at the time, in the canonical unit."));
                        broken = true;
                    }
                },
                None => {
                    bad(
                        "missing_observed",
                        name,
                        tr!("`observed.{name}` がありません", "`observed.{name}` is missing"),
                        &tr!("出力は全部要ります。片方だけ比べると、比べなかった側の食い違いを見逃します。", "Every output is required. Comparing only one side turns a mismatch on the other side green."),
                    );
                    broken = true;
                }
            }
        }
        if broken {
            continue;
        }

        // `trace` is optional. When it is there, every table must be one of the rule's and
        // every row one the table has — a trace that names nothing real is a record from
        // some other version of the rule, and it is reported rather than read.
        let mut trace: Vec<(String, usize)> = Vec::new();
        let mut trace_labels: Vec<Option<String>> = Vec::new();
        if let Some(t) = j.get("trace") {
            let hint = tr!(
                "`trace` は `{{\"table\":表名,\"row\":行番号}}` の並びです。生成コードの record 関数が書きます。",
                "`trace` is a list of `{{\"table\":name,\"row\":number}}`; the generated code's record function writes it."
            );
            let mut ok = true;
            match t {
                Json::Arr(items) => {
                    for it in items {
                        let table = it.get("table").and_then(|x| x.as_str());
                        let row = it.get("row").and_then(|x| x.as_int());
                        let rows = table.and_then(|name| {
                            f.items.iter().find_map(|i| match i {
                                crate::ast::Item::Table(tb) if tb.name.as_ref().is_some_and(|n| n.text == name) => Some(tb.rows.len()),
                                _ => None,
                            })
                        });
                        match (table, row, rows) {
                            (Some(name), Some(r), Some(n)) if r >= 1 && (r as usize) <= n => {
                                trace.push((name.to_string(), r as usize));
                                trace_labels.push(it.get("label").and_then(|x| x.as_str()).map(|l| l.to_string()));
                            }
                            (Some(name), _, None) => {
                                bad("bad_trace", "trace", tr!("`trace`: 表 {name} はこの規則にありません", "`trace`: table {name} is not in this rule"), &hint);
                                ok = false;
                            }
                            (Some(name), Some(r), Some(n)) => {
                                bad("bad_trace", "trace", tr!("`trace`: 表 {name} に 行{r} はありません（{n} 行）", "`trace`: table {name} has no row {r} (it has {n})"), &hint);
                                ok = false;
                            }
                            _ => {
                                bad("bad_trace", "trace", tr!("`trace` の要素が `table` と `row` を持っていません", "an entry of `trace` lacks `table` or `row`"), &hint);
                                ok = false;
                            }
                        }
                        if !ok {
                            break;
                        }
                    }
                }
                _ => {
                    bad("bad_trace", "trace", tr!("`trace` が配列ではありません", "`trace` is not an array"), &hint);
                    ok = false;
                }
            }
            if !ok {
                continue;
            }
        }
        out.beyond.extend(beyond.into_iter().map(|(field, range)| Beyond { line, tag: tag.clone(), field, range }));
        out.records.push(Record { line, tag, ts, input, observed, filled, trace, trace_labels });
    }
    out
}

/// The report of `rulec fixtures lint`.
pub fn render_lint(l: &Load, path: &str) -> String {
    let mut o = tr!(
        "{path}: 記録 {} 件（そのまま {}、補った分 {}）\n",
        "{path}: {} records ({} observed, {} filled)\n",
        l.records.len(),
        l.measured(),
        l.filled()
    );
    if l.dropped > 0 {
        o.push_str(&tr!("フィールドが欠けていたので外した記録: {} 件\n", "Records excluded because a field was missing: {}\n", l.dropped));
    }
    if l.problems.is_empty() {
        o.push_str(&tr!("形式の問題はありません。\n", "No format problems.\n"));
        o.push_str(&beyond_text(l));
        return o;
    }
    o.push_str(&tr!("\n問題 {} 件:\n", "\n{} problems:\n", l.problems.len()));
    // Problems of the same shape are grouped. In a 10,000-line extract, the same misspelling
    // listed 10,000 times is unreadable.
    let mut by: BTreeMap<&str, Vec<&Problem>> = BTreeMap::new();
    for p in &l.problems {
        by.entry(p.what.as_str()).or_default().push(p);
    }
    for (what, ps) in &by {
        let ex = ps[0];
        let where_ = if ex.tag.is_empty() {
            tr!("{} 行目", "line {}", ex.line)
        } else {
            tr!("{} 行目 ({})", "line {} ({})", ex.line, ex.tag)
        };
        o.push_str(&tr!("  {what}\n    {} 件。例: {where_}\n    {}\n", "  {what}\n    {} record(s). Example: {where_}\n    {}\n", ps.len(), ex.hint));
    }
    o.push_str(&beyond_text(l));
    o
}

/// The observed values outside what the rule can produce, grouped by output (§15.199). Not a
/// problem: `replay` reads the records and counts each such value as a mismatch. They are
/// listed because they are also what a record written before a step or a unit changed looks
/// like, and that has a reading of its own.
fn beyond_groups(l: &Load) -> Vec<(String, Vec<&Beyond>)> {
    let mut by: BTreeMap<String, Vec<&Beyond>> = BTreeMap::new();
    for b in &l.beyond {
        by.entry(beyond_what(b)).or_default().push(b);
    }
    by.into_iter().collect()
}

fn beyond_what(b: &Beyond) -> String {
    tr!("`observed.{}`: 規則が取りうる範囲 {} の外です", "`observed.{}`: outside what the rule can produce, {}", b.field, b.range)
}

fn beyond_hint() -> String {
    tr!(
        "replay は不一致として数えます。刻みや単位を変える前に書いた記録なら、`--read-as <規則>@<版>` でその版の読み方で読めます。",
        "replay counts them as mismatches. If the records were written before a step or a unit changed, `--read-as <rule>@<rev>` reads them the way that version wrote them."
    )
}

fn beyond_text(l: &Load) -> String {
    if l.beyond.is_empty() {
        return String::new();
    }
    let mut o = tr!(
        "\nobserved の値が規則の取りうる範囲の外にある記録（形式の問題ではありません）:\n",
        "\nObserved values outside what the rule can produce (not problems of the format):\n"
    );
    for (what, bs) in beyond_groups(l) {
        let ex = bs[0];
        let where_ = if ex.tag.is_empty() {
            tr!("{} 行目", "line {}", ex.line)
        } else {
            tr!("{} 行目 ({})", "line {} ({})", ex.line, ex.tag)
        };
        o.push_str(&tr!("  {what}\n    {} 件。例: {where_}\n    {}\n", "  {what}\n    {} record(s). Example: {where_}\n    {}\n", bs.len(), beyond_hint()));
    }
    o
}

/// `--format json` for `fixtures lint` (docs/formats.md). Problems of the same shape are
/// grouped exactly as in the text rendering: in a 10,000-line extract, the same misspelling
/// listed 10,000 times is unreadable in either form.
pub fn render_lint_json(l: &Load, path: &str) -> String {
    let mut by: BTreeMap<&str, Vec<&Problem>> = BTreeMap::new();
    for p in &l.problems {
        by.entry(p.what.as_str()).or_default().push(p);
    }
    let problems: Vec<String> = by
        .values()
        .map(|ps| {
            let ex = ps[0];
            let example = crate::json::Obj::new()
                .int("line", ex.line as i128)
                .str("tag", &ex.tag)
                .finish();
            let mut o = crate::json::Obj::new().str("kind", ex.kind);
            if !ex.field.is_empty() {
                o = o.str("field", &ex.field);
            }
            o.int("count", ps.len() as i128)
                .raw("example", example)
                .str("what", &ex.what)
                .str("hint", &ex.hint)
                .finish()
        })
        .collect();
    // §15.199: what the rule cannot produce, grouped the same way; not problems.
    let beyond: Vec<String> = beyond_groups(l)
        .iter()
        .map(|(what, bs)| {
            let ex = bs[0];
            crate::json::Obj::new()
                .str("field", &ex.field)
                .int("count", bs.len() as i128)
                .raw("example", crate::json::Obj::new().int("line", ex.line as i128).str("tag", &ex.tag).finish())
                .str("what", what)
                .str("hint", &beyond_hint())
                .finish()
        })
        .collect();
    crate::json::Obj::new()
        .str("file", path)
        .int("records", l.records.len() as i128)
        .int("observed", l.measured() as i128)
        .int("filled", l.filled() as i128)
        .int("dropped", l.dropped as i128)
        .raw("problems", crate::json::arr(&problems))
        .raw("out_of_reach", crate::json::arr(&beyond))
        .finish()
}
