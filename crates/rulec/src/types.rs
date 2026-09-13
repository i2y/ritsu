//! Types, names, and rounding (§2, §7). Produces diagnostics E011, E103, E104, E106, W111.

use crate::ast::*;
use crate::diag::{Diag, Span};
use crate::num::{Rat, RoundMode};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    /// A declared or imported enumeration.
    Enum(String),
    /// §2.1: currency and tax are a double brand. `tax: None` is a bare literal,
    /// which unifies with either brand.
    Money { cur: String, tax: Option<String> },
    /// `mass[g]` / `length[cm]` — a dimension plus the declared unit.
    Qty { dim: String, unit: String },
    Rate,
    /// §2.1: a whole number that carries no unit — a count, a number of days, a score. It is
    /// dimensionless like a rate, but its values are whole and it has no step to declare.
    Number,
    Bool,
    Str,
    Date,
    /// `T?`. Consumed only by `none` in a cell; it never appears in an expression (§2.1).
    Opt(Box<Ty>),
    Unknown,
}

impl std::fmt::Display for Ty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ty::Enum(n) => write!(f, "{n}"),
            Ty::Money { cur, tax: Some(t) } => write!(f, "{}[{cur}, {t}]", crate::kw::MONEY),
            Ty::Money { cur, tax: None } => write!(f, "{}[{cur}]", crate::kw::MONEY),
            Ty::Qty { dim, unit } => write!(f, "{dim}[{unit}]"),
            Ty::Rate => write!(f, "{}", crate::kw::RATE),
            Ty::Number => write!(f, "{}", crate::kw::NUMBER),
            Ty::Bool => write!(f, "{}", crate::kw::BOOL),
            Ty::Str => write!(f, "{}", crate::kw::STRING),
            Ty::Date => write!(f, "{}", crate::kw::DATE),
            Ty::Opt(t) => write!(f, "{t}?"),
            Ty::Unknown => write!(f, "?"),
        }
    }
}

impl Ty {
    pub fn is_numeric(&self) -> bool {
        matches!(self, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number)
    }
    /// Do two types describe the same quantity? Money literals carry no tax brand,
    /// so they unify with either.
    fn unifies(&self, o: &Ty) -> bool {
        match (self, o) {
            (Ty::Money { cur: a, tax: ta }, Ty::Money { cur: b, tax: tb }) => {
                a == b && (ta.is_none() || tb.is_none() || ta == tb)
            }
            (Ty::Unknown, _) | (_, Ty::Unknown) => true,
            _ => self == o,
        }
    }
}

/// (dimension, factor to the dimension's base unit)
fn unit_info(u: &str) -> Option<(&'static str, Rat)> {
    Some(match u {
        "g" => (crate::kw::MASS, Rat::int(1)),
        "kg" => (crate::kw::MASS, Rat::int(1000)),
        "cm" => (crate::kw::LENGTH, Rat::int(1)),
        "m" => (crate::kw::LENGTH, Rat::int(100)),
        "円" => (crate::kw::MONEY, Rat::int(1)),
        "銭" => (crate::kw::MONEY, Rat::new(1, 100)),
        "%" => (crate::kw::RATE, Rat::new(1, 100)),
        _ => return None,
    })
}

/// A date is held as the **day count since the epoch** (proleptic Gregorian calendar,
/// 1970-01-01 = 0).
///
/// `y*10000 + m*100 + d` would preserve the ordering too, but it leaves gaps of integers
/// that are not real dates at every month end (20260332..20260400). A table tiled with
/// `<=2026-03-31` and `>=2026-04-01` is actually complete, yet it would get a **false
/// E101** because the phantom integers are not covered. Tiling with inclusive bounds is
/// the natural way to write business rules (§3.1), so this would be hit every time.
///
/// With day counts, adjacency is exactly +1, so the ±1 at boundaries, the emptiness test,
/// the merging of neighbors, and the write-back of witnesses all stay correct with the
/// existing integer machinery, unchanged.
pub fn date_ord(y: i32, m: u32, d: u32) -> Rat {
    Rat::int(days_from_civil(y, m as i64, d as i64))
}

pub fn ord_to_date(v: Rat) -> (i32, u32, u32) {
    let (y, m, d) = civil_from_days(v.num / v.den);
    (y as i32, m as u32, d as u32)
}

/// Howard Hinnant's days_from_civil. Proleptic Gregorian calendar.
fn days_from_civil(y: i32, m: i64, d: i64) -> i128 {
    let y = y as i64 - if m <= 2 { 1 } else { 0 };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era * 146097 + doe - 719468) as i128
}

fn civil_from_days(z: i128) -> (i64, i64, i64) {
    let z = z as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    (y + if m <= 2 { 1 } else { 0 }, m, d)
}

#[cfg(test)]
mod date_tests {
    use super::*;

    #[test]
    fn 通算日は往復して隣接が一になる() {
        for (y, m, d) in [(2026, 4, 1), (2026, 3, 31), (2024, 2, 29), (1970, 1, 1), (1999, 12, 31)] {
            let o = date_ord(y, m, d);
            assert_eq!(ord_to_date(o), (y, m, d), "{y}-{m}-{d} round trip");
        }
        // The last day of a month and the first day of the next are adjacent. This is why
        // no phantom integer gap appears.
        let a = date_ord(2026, 3, 31);
        let b = date_ord(2026, 4, 1);
        assert_eq!(b.sub(a), Rat::int(1), "3/31 and 4/1 are adjacent");
        // Leap day
        let c = date_ord(2024, 2, 28);
        assert_eq!(date_ord(2024, 2, 29).sub(c), Rat::int(1));
        assert_eq!(date_ord(2024, 3, 1).sub(date_ord(2024, 2, 29)), Rat::int(1));
        // In a common year the day after 2/28 is 3/1
        assert_eq!(date_ord(2025, 3, 1).sub(date_ord(2025, 2, 28)), Rat::int(1));
    }
}

/// The number a literal writes, before its unit: `100万` is 1000000 and `1.5` is 3/2. Exact,
/// because §2.1 asks for `0.5%` to be the rational 1/200 rather than a rounded decimal.
fn magnitude(n: &crate::lex::Num) -> Option<Rat> {
    let whole: i128 = n.digits.parse().ok()?;
    let den = 10i128.checked_pow(u32::try_from(n.frac.len()).ok()?)?;
    let frac: i128 = if n.frac.is_empty() { 0 } else { n.frac.parse().ok()? };
    let v = Rat::new(whole.checked_mul(den)?.checked_add(frac)?, den).mul(Rat::int(n.mult as i128));
    Some(if n.neg { Rat::zero().sub(v) } else { v })
}

/// Value of a numeric literal, expressed in `want`'s declared unit.
/// `None` means the literal's unit does not belong to `want`'s dimension, or that a decimal
/// was written where the declared unit only has whole values.
fn lit_value_in(n: &crate::lex::Num, want: &Ty) -> Option<Rat> {
    let v = magnitude(n)?;
    // A number carries no unit, and that is the whole point of it: `3` is three of whatever
    // the column counts. Nothing else accepts a bare literal (§3).
    if matches!(want, Ty::Number) {
        return if n.unit.is_none() && v.is_int() { Some(v) } else { None };
    }
    let unit = n.unit.as_deref()?;
    let (dim, f) = unit_info(unit)?;
    // §2.1: money and quantities are integers in their declared unit, and only a rate is an
    // exact rational. `1.5kg` is fine in a `mass[g]` column because it lands on 1500g; `0.5円`
    // is not a value of `money[円]` at all.
    let whole = |v: Rat| if v.is_int() { Some(v) } else { None };
    match want {
        Ty::Money { .. } if dim == crate::kw::MONEY => whole(v.mul(f)),
        Ty::Qty { dim: d, unit: du } if dim == *d => {
            let (_, fd) = unit_info(du)?;
            whole(v.mul(f).div(fd))
        }
        Ty::Rate if dim == crate::kw::RATE => Some(v.mul(f)),
        _ => None,
    }
}

/// Type of a numeric literal read on its own, before any expectation.
fn lit_ty(n: &crate::lex::Num) -> Ty {
    match n.unit.as_deref().and_then(unit_info) {
        Some((crate::kw::MONEY, _)) => Ty::Money { cur: n.unit.clone().unwrap(), tax: None },
        Some((crate::kw::RATE, _)) => Ty::Rate,
        Some((d, _)) => Ty::Qty { dim: d.to_string(), unit: n.unit.clone().unwrap() },
        // A literal with no unit is a plain number. It used to be "unknown", which unified
        // with anything and let `3` stand where a yen amount was meant.
        None if n.unit.is_none() => Ty::Number,
        None => Ty::Unknown,
    }
}

#[derive(Debug, Clone)]
pub struct Sym {
    pub ty: Ty,
    pub span: Span,
    pub kind: SymKind,
    pub contract_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SymKind {
    Input,
    Derived,
    Define,
    TableOut,
    Output,
}

pub struct Checked {
    pub syms: HashMap<String, Sym>,
    /// Output name → rounding (mode and grid). E106 checks that literals sit on this grid.
    pub roundings: HashMap<String, (RoundMode, Rat)>,
    /// Input name → declared range. E112 uses it to check that the reachable interval is
    /// contained.
    pub ranges: HashMap<String, (Option<Rat>, Option<Rat>)>,
    /// Table output name → the values that table can actually produce. A downstream row that
    /// names a value the upstream table never produces is unreachable (a variant of §11 E102).
    pub out_values: HashMap<String, Vec<String>>,
    /// Boolean definition name → the names its expression mentions. Used to decide whether
    /// a witness can be constructed.
    pub define_deps: HashMap<String, Vec<String>>,
    /// Derived name → the inputs it depends on. Used to see whether two derived values share
    /// an input: when they do, a sieve of independent intervals per derived value cannot see
    /// the dependency (§6.2).
    pub derived_deps: HashMap<String, Vec<String>>,
    /// Name → reciprocal of the step: the value is a multiple of 1/k. Under §7.1's
    /// "a single int64 plus a static rational scale", the stored integer is value×k.
    pub scales: HashMap<String, i128>,
    /// Enum name -> values, in declaration order (§6.3 picks the first as a witness).
    pub enums: HashMap<String, Vec<String>>,
    /// Group name -> the enum it belongs to and its members.
    pub groups: HashMap<String, (String, Vec<String>)>,
    pub used: HashSet<String>,
    /// Enum values named in a cell or expression. W111 uses it to report "values that appear
    /// in no row".
    pub used_values: HashSet<String>,
    pub diags: Vec<Diag>,
}

pub fn check(f: &RuleFile, path: &str) -> Checked {
    let mut c = Checked {
        syms: HashMap::new(),
        roundings: HashMap::new(),
        ranges: HashMap::new(),
        derived_deps: HashMap::new(),
        define_deps: HashMap::new(),
        out_values: HashMap::new(),
        scales: HashMap::new(),
        enums: HashMap::new(),
        groups: HashMap::new(),
        used: HashSet::new(),
        used_values: HashSet::new(),
        diags: Vec::new(),
    };
    let at = |line: usize| format!("{path}:{line}");

    // std/都道府県 is the only import the corpus needs; its 47 values come from the
    // prelude (§2.2). Until the prelude is a real file, accept any value for it.
    for (p, sp) in &f.imports {
        match crate::prelude::lookup(p) {
            Some((name, values)) => {
                c.enums.insert(name, values);
            }
            None => c.diags.push(
                Diag::error("E013", tr!("`{p}` という取込先はありません", "There is no import named `{p}`"))
                    .at(at(sp.line))
                    .mark(sp.clone(), "")
                    .note(tr!("組み込みは 標準/都道府県 だけです（§2.2）。", "The only built-in module is std/都道府県 (§2.2).")),
            ),
        }
    }

    for e in &f.enums {
        c.enums.insert(
            e.name.text.clone(),
            e.values.iter().map(|v| v.text.clone()).collect(),
        );
    }
    for g in &f.groups {
        // The group's enum is whichever enumeration holds its first member.
        let owner = g
            .members
            .first()
            .and_then(|m| {
                c.enums
                    .iter()
                    .find(|(_, vs)| vs.iter().any(|v| v == &m.text))
                    .map(|(n, _)| n.clone())
            })
            .unwrap_or_else(|| "都道府県".to_string());
        c.groups.insert(
            g.name.text.clone(),
            (owner, g.members.iter().map(|m| m.text.clone()).collect()),
        );
    }

    for i in &f.inputs {
        let ty = c.resolve(&i.ty);
        if let Some(r) = &i.range {
            c.ranges.insert(i.name.text.clone(), bounds_of(r, &ty));
        }
        c.scales.insert(i.name.text.clone(), scale_of_type(&i.ty, &ty));
        c.syms.insert(
            i.name.text.clone(),
            Sym { ty, span: i.name.span.clone(), kind: SymKind::Input, contract_only: i.contract_only },
        );
    }
    if f.outputs.is_empty() {
        c.diags.push(
            Diag::error("E009", tr!("出力がありません", "No outputs are declared"))
                .at(at(f.name.span.line))
                .mark(f.name.span.clone(), "")
                .note(tr!("規則は値をひとつ以上返します。`{}` の節を書いてください。", "A rule returns at least one value. Write an `{}` section.", crate::kw::OUTPUTS)),
        );
    }
    for o in &f.outputs {
        let ty = c.resolve(&o.ty);
        // §7.2: a numeric output must declare its rounding.
        if ty.is_numeric() && o.rounding.is_none() {
            c.diags.push(
                Diag::error("E104", tr!("出力に丸めの宣言がありません", "The output declares no rounding"))
                    .at(at(o.span.line))
                    .mark(o.ty.span.clone(), tr!("丸め の宣言がありません", "no rounding is declared"))
                    .fix(
                        crate::diag::FixKind::AddRounding,
                        format!("{} {}({})", crate::kw::ROUND, crate::kw::DOWN, unit_one(&ty)),
                    )
                    .note(tr!("出力 {} は {} です。", "Output {} has type {}.", o.name.text, ty)),
            );
        }
        if let Some(rd) = &o.rounding {
            if let (Some(m), Some(g)) = (RoundMode::parse(&rd.mode), lit_value_in(&rd.grid, &ty)) {
                c.roundings.insert(o.name.text.clone(), (m, g));
            }
        }
        c.syms.insert(
            o.name.text.clone(),
            Sym { ty, span: o.name.span.clone(), kind: SymKind::Output, contract_only: false },
        );
    }

    // §1.1 fixes each keyword to a single spelling. When a name collides with a keyword,
    // the line-oriented parser reads the declaration as the start of a section and silently
    // drops it. Silent dropping is the worst outcome, so it is the name that gets rejected.
    const KEYWORDS: &[&str] = crate::kw::RESERVED;
    let mut named: Vec<(&str, &Span)> = Vec::new();
    named.push((f.name.text.as_str(), &f.name.span));
    for i in &f.inputs {
        named.push((i.name.text.as_str(), &i.name.span));
    }
    for o in &f.outputs {
        named.push((o.name.text.as_str(), &o.name.span));
    }
    for e in &f.enums {
        named.push((e.name.text.as_str(), &e.name.span));
        for v in &e.values {
            named.push((v.text.as_str(), &v.span));
        }
    }
    for g in &f.groups {
        named.push((g.name.text.as_str(), &g.name.span));
    }
    for it in &f.items {
        match it {
            Item::Derived(d) => named.push((d.name.text.as_str(), &d.name.span)),
            Item::Define(d) => named.push((d.name.text.as_str(), &d.name.span)),
            Item::Table(t) => {
                if let Some(n) = &t.name {
                    named.push((n.text.as_str(), &n.span));
                }
                for oc in &t.outputs {
                    named.push((oc.name.text.as_str(), &oc.name.span));
                }
            }
        }
    }
    for (n, sp) in named {
        if KEYWORDS.contains(&n) {
            c.diags.push(
                Diag::error("E009", tr!("`{n}` はキーワードなので、名前にできません", "`{n}` is a keyword and cannot be used as a name"))
                    .at(at(sp.line))
                    .mark(sp.clone(), "")
                    .note(tr!("行指向の構文なので、キーワードと同じ名前は宣言を黙って捨ててしまいます。", "The syntax is line-oriented, so a declaration named like a keyword is silently dropped."))
                    .note(tr!("別の名前を付けてください。", "Choose a different name.")),
            );
        }
    }

    // §1.3: the public face needs ASCII aliases; the inside does not. A name that is
    // already ASCII needs none either — the reason for the alias is that a kanji has no
    // uppercase and so cannot begin an exported Go identifier, which is not a problem
    // `shipping_fee` has. Demanding `rule shipping_fee(shipping_fee)` was pure ceremony.
    let mut missing: Vec<String> = Vec::new();
    if needs_alias(&f.name) {
        missing.push(tr!("規則 {}", "rule {}", f.name.text));
    }
    for i in &f.inputs {
        if needs_alias(&i.name) {
            missing.push(tr!("入力 {}", "input {}", i.name.text));
        }
    }
    for o in &f.outputs {
        if needs_alias(&o.name) {
            missing.push(tr!("出力 {}", "output {}", o.name.text));
        }
    }
    if !missing.is_empty() {
        c.diags.push(
            Diag::error("E011", tr!("公開面の名前に ASCII 別名がありません", "A public name has no ASCII alias"))
                .at(at(f.name.span.line))
                .fix_kind(crate::diag::FixKind::AddAlias)
                .mark(f.name.span.clone(), "")
                .note(tr!("不足: {}", "Missing: {}", missing.join(" / ")))
                .note(tr!("Go の公開識別子は先頭が大文字である必要があり、漢字とかなは大文字を持ちません（§1.3）。名前がもとから ASCII なら別名は要りません。", "An exported Go identifier must start with an uppercase letter, and kanji and kana have no uppercase (§1.3). A name that is already ASCII needs no alias."))
                .note(tr!("宣言の位置に丸括弧で書いてください。例: 届け先(dest)", "Write it in parentheses at the declaration, e.g. 届け先(dest)")),
        );
    }

    // §5.1: items are a define-before-use pipeline, so a single forward pass is enough.
    for it in &f.items {
        match it {
            Item::Derived(d) => {
                let ty = c.resolve(&d.ty);
                let got = c.expr_ty(&d.expr, path);
                c.check_same(&ty, &got, &d.span, path, &tr!("導出", "derived value"));
                c.derived_range(d, &ty, path);
                c.overflow(&d.expr, &ty, &d.span, path, &d.name.text);
                if let (Some(iv), Some(sc)) = (c.interval(&d.expr, &ty), c.scale(&d.expr)) {
                    c.ranges.insert(d.name.text.clone(), (Some(iv.0), Some(iv.1)));
                    c.scales.insert(d.name.text.clone(), sc);
                }
                let mut deps = Vec::new();
                collect_names(&d.expr, &mut deps);
                c.derived_deps.insert(d.name.text.clone(), deps);
                c.syms.insert(
                    d.name.text.clone(),
                    Sym { ty, span: d.name.span.clone(), kind: SymKind::Derived, contract_only: false },
                );
            }
            Item::Define(d) => {
                let ty = c.resolve(&d.ty);
                let got = c.expr_ty(&d.expr, path);
                c.check_same(&ty, &got, &d.span, path, &tr!("定義", "definition"));
                c.overflow(&d.expr, &ty, &d.span, path, &d.name.text);
                if let (Some(iv), Some(sc)) = (c.interval(&d.expr, &ty), c.scale(&d.expr)) {
                    c.ranges.insert(d.name.text.clone(), (Some(iv.0), Some(iv.1)));
                    c.scales.insert(d.name.text.clone(), sc);
                }
                c.syms.insert(
                    d.name.text.clone(),
                    Sym { ty, span: d.name.span.clone(), kind: SymKind::Define, contract_only: false },
                );
            }
            Item::Table(t) => c.table(t, path),
        }
    }

    // §5.3 E113: the atoms of a boolean definition are limited to unary tests on an input or
    // a derived value. Relaxing this lets half-spaces like `x − y >= c` into the columns, and
    // the box algebra of §6.2 falls apart.
    for it in &f.items {
        if let Item::Define(d) = it {
            if c.resolve(&d.ty) == Ty::Bool {
                c.atoms(&d.expr, &d.name.text, path);
                let mut deps = Vec::new();
                collect_names(&d.expr, &mut deps);
                c.define_deps.insert(d.name.text.clone(), deps);
            }
        }
    }

    if let Some(r) = &f.result {
        c.used.insert(r.name.clone());
        let got = c.expr_ty(&r.expr, path);
        if let Some(s) = c.syms.get(&r.name).cloned() {
            c.check_same(&s.ty, &got, &r.span, path, &tr!("結果", "result"));
        }
    }

    // §11 W111: declarations nothing names. `contract_only` silences a range-guard-only input.
    for i in &f.inputs {
        if !c.used.contains(&i.name.text) && !i.contract_only {
            c.diags.push(
                Diag::warning("W111", tr!("入力 {} はどの表でも使われていません", "Input {} is not used by any table", i.name.text))
                    .at(at(i.span.line))
                    .fix(crate::diag::FixKind::MarkContractOnly, crate::kw::CONTRACT_ONLY)
                    .mark(i.name.span.clone(), tr!("どの列にも現れません", "appears in no column"))
                    .note(tr!("本来使うべき列の書き忘れかもしれません。", "A column that should use it may have been left out."))
                    .note(tr!("範囲の入口検査としてだけ効かせるつもりなら、宣言に `{}` を付けてください（§11 W111）。", "If it is meant only as an entry check on its range, add `{}` to the declaration (§11 W111).", crate::kw::CONTRACT_ONLY)),
            );
        }
    }
    for it in &f.items {
        if let Item::Derived(d) = it {
            if !c.used.contains(&d.name.text) {
                c.diags.push(
                    Diag::warning("W111", tr!("導出 {} はどこでも使われていません", "Derived value {} is never used", d.name.text))
                        .at(at(d.span.line))
                        .mark(d.name.span.clone(), tr!("どの列にも式にも現れません", "appears in no column and no expression"))
                        .note(tr!("使わない導出は、検査の軸を一本増やすだけです。消すか、使ってください。", "An unused derived value only adds one more axis to the checks. Remove it, or use it.")),
                );
            }
        }
    }
    // Only types declared in this file are checked. For an imported type (the 47 prefectures)
    // it is normal that a cell like `not: 沖縄` leaves 45 values unnamed; demanding a mark on
    // each of them would wreck the whole warning channel.
    for e in &f.enums {
        let unused: Vec<&Name> = e
            .values
            .iter()
            .enumerate()
            .filter(|(i, v)| !c.used_values.contains(&v.text) && !e.default_marks.get(*i).copied().unwrap_or(false))
            .map(|(_, v)| v)
            .collect();
        if !unused.is_empty() {
            let names: Vec<String> = unused.iter().map(|v| v.text.clone()).collect();
            c.diags.push(
                Diag::warning("W111", tr!("型 {} の値がどの行にも現れません", "Values of type {} appear in no row", e.name.text))
                    .at(at(e.span.line))
                    .fix(crate::diag::FixKind::MarkDefault, crate::kw::DEFAULT)
                    .mark(e.name.span.clone(), "")
                    .note(tr!("現れない値: {}", "Values that never appear: {}", names.join(" / ")))
                    .note(tr!("完全性検査は通っていても、その値に当たる行が `-` に吸われているだけかもしれません。", "Even though the completeness check passes, the rows for those values may simply be absorbed by a `-`.")),
            );
        }
    }
    // A table's output column may introduce a name of its own. Nothing downstream reading it
    // means the column computes a value that never leaves the table — and Go will not compile
    // a local that is never read, so this has to be said here rather than discovered there.
    for it in &f.items {
        let Item::Table(t) = it else { continue };
        for oc in &t.outputs {
            let n = &oc.name.text;
            if c.used.contains(n) || f.outputs.iter().any(|o| o.name.text == *n) {
                continue;
            }
            c.diags.push(
                Diag::warning("W111", tr!("表の列 {n} はどこでも使われていません", "Table column {n} is never used"))
                    .at(at(oc.name.span.line))
                    .mark(oc.name.span.clone(), tr!("この列の値は表から出ていきません", "this column's value never leaves the table"))
                    .note(tr!("規則の出力にするか、後ろの表か `result` で使うか、消してください。", "Make it an output of the rule, use it in a later table or in `result`, or remove it.")),
            );
        }
    }
    for g in &f.groups {
        if !c.used.contains(&g.name.text) {
            c.diags.push(
                Diag::warning("W111", tr!("グループ {} はどのセルでも使われていません", "Group {} is not used in any cell", g.name.text))
                    .at(at(g.span.line))
                    .mark(g.name.span.clone(), ""),
            );
        }
    }
    c
}

impl Checked {
    fn resolve(&mut self, t: &TypeRef) -> Ty {
        let base = match t.base.as_str() {
            crate::kw::MONEY => {
                let mut it = t.args.iter().filter_map(|a| match a {
                    TypeArg::Word(w) => Some(w.clone()),
                    _ => None,
                });
                Ty::Money { cur: it.next().unwrap_or_else(|| "円".into()), tax: it.next() }
            }
            crate::kw::MASS | crate::kw::LENGTH => {
                let unit = t
                    .args
                    .iter()
                    .find_map(|a| match a {
                        TypeArg::Word(w) => Some(w.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                Ty::Qty { dim: t.base.clone(), unit }
            }
            crate::kw::RATE => Ty::Rate,
            crate::kw::NUMBER => Ty::Number,
            crate::kw::BOOL => Ty::Bool,
            crate::kw::STRING => Ty::Str,
            crate::kw::DATE => Ty::Date,
            other => Ty::Enum(other.to_string()),
        };
        if t.optional { Ty::Opt(Box::new(base)) } else { base }
    }

    fn check_same(&mut self, want: &Ty, got: &Ty, span: &Span, path: &str, what: &str) {
        // §2.1: a rate and a number are both dimensionless and share one runtime form — an
        // integer with a scale. Which of the two a ratio is called is for the declaration to
        // say: `達成率 : rate = 合計点 ÷ 満点` writes its thresholds as percentages, while
        // `基本点 : number = 税込金額 ÷ 100円` writes them as counts. Only a declaration gets
        // this latitude; `unifies` stays strict, so a rate still cannot be added to a count.
        let both_dimensionless =
            matches!((want, got), (Ty::Rate, Ty::Number) | (Ty::Number, Ty::Rate));
        if !want.unifies(got) && !both_dimensionless && *got != Ty::Unknown {
            self.diags.push(
                Diag::error("E103", tr!("型が合いません: {want} に {got} を入れています", "Type mismatch: {want} is given {got}"))
                    .at(format!("{path}:{} {what}", span.line))
                    .mark(span.clone(), ""),
            );
        }
    }

    fn expr_ty(&mut self, e: &Expr, path: &str) -> Ty {
        match e {
            Expr::Name(n, sp) => {
                self.used.insert(n.clone());
                match self.syms.get(n) {
                    Some(s) => s.ty.clone(),
                    None => {
                        self.diags.push(
                            Diag::error("E012", tr!("`{n}` という名前は宣言されていません", "The name `{n}` is not declared"))
                                .at(format!("{path}:{}", sp.line))
                                .mark(sp.clone(), "")
                                .note(tr!("上の行までに 入力 / 導出 / 定義 / 表の出力 として宣言されている必要があります（§5.1）。", "It must be declared on an earlier line as an input, a derived value, a definition, or a table output (§5.1).")),
                        );
                        Ty::Unknown
                    }
                }
            }
            Expr::Lit(l, _) => match l {
                Lit::Num(n) => lit_ty(n),
                Lit::Str(_) => Ty::Str,
                Lit::Date(..) => Ty::Date,
                Lit::Word(w) => {
                    if w == crate::kw::TRUE || w == crate::kw::FALSE {
                        Ty::Bool
                    } else {
                        self.enum_of_value(w).map(Ty::Enum).unwrap_or(Ty::Unknown)
                    }
                }
            },
            Expr::Call(name, args, sp) => {
                let ats: Vec<Ty> = args.iter().map(|a| self.expr_ty(a, path)).collect();
                match name.as_str() {
                    crate::kw::DOWN | crate::kw::UP | crate::kw::HALF_UP | crate::kw::HALF_EVEN => {
                        ats.first().cloned().unwrap_or(Ty::Unknown)
                    }
                    crate::kw::MIN | crate::kw::MAX => {
                        if ats.len() == 2 && !ats[0].unifies(&ats[1]) {
                            self.diags.push(
                                Diag::error(
                                    "E103",
                                    tr!("{name} の二つの引数の型が違います: {} と {}", "The two arguments of {name} have different types: {} and {}", ats[0], ats[1]),
                                )
                                .at(format!("{path}:{}", sp.line))
                                .mark(sp.clone(), ""),
                            );
                        }
                        ats.first().cloned().unwrap_or(Ty::Unknown)
                    }
                    _ => Ty::Unknown,
                }
            }
            Expr::Bin(l, op, r, sp) => {
                let lt = self.expr_ty(l, path);
                let rt = self.expr_ty(r, path);
                use BinOp::*;
                match op {
                    Le | Ge | Lt | Gt | Eq => {
                        if !lt.unifies(&rt) {
                            self.mix(&lt, &rt, sp, path);
                        }
                        Ty::Bool
                    }
                    Add | Sub => {
                        if !lt.unifies(&rt) {
                            self.mix(&lt, &rt, sp, path);
                            return Ty::Unknown;
                        }
                        if matches!(lt, Ty::Money { tax: None, .. }) { rt } else { lt }
                    }
                    Mul => match (&lt, &rt) {
                        // §2.3: money × rate is unrounded money. money × money is forbidden.
                        // A number is dimensionless too, so it multiplies the same way — that
                        // is what makes `送料 × 個数` say what it means.
                        (Ty::Money { .. }, Ty::Rate | Ty::Number) => lt.clone(),
                        (Ty::Rate | Ty::Number, Ty::Money { .. }) => rt.clone(),
                        (Ty::Qty { .. }, Ty::Rate | Ty::Number)
                        | (Ty::Rate | Ty::Number, Ty::Qty { .. }) => {
                            if matches!(lt, Ty::Rate | Ty::Number) { rt } else { lt }
                        }
                        // A rate of a count is a rate, and a count of counts is a count.
                        (Ty::Number, Ty::Rate) | (Ty::Rate, Ty::Number) => Ty::Rate,
                        (Ty::Money { .. }, Ty::Money { .. }) => {
                            self.diags.push(
                                Diag::error("E103", tr!("金額どうしを掛けています", "Multiplying money by money"))
                                    .at(format!("{path}:{}", sp.line))
                                    .mark(sp.clone(), "")
                                    .note(tr!("合成次元は業務ルールに現れないので、モデリングの誤りとして止めます（§2.1）。", "Compound dimensions do not occur in business rules, so this is stopped as a modeling error (§2.1).")),
                            );
                            Ty::Unknown
                        }
                        (Ty::Rate, Ty::Rate) => Ty::Rate,
                        _ => lt.clone(),
                    },
                    Div => {
                        // §2.3 and §7.4: the divisor is a constant, always. A variable
                        // divisor would leave the scale undecidable, and the generator would
                        // have to fall back on a language's own division — Python rounds
                        // toward -inf and Go toward zero, so the two would disagree.
                        if const_value(r).is_none() {
                            self.diags.push(
                                Diag::error("E115", tr!("変数では割れません", "Cannot divide by a variable"))
                                    .at(format!("{path}:{}", sp.line))
                                    .mark(sp.clone(), tr!("割る数が定数ではありません", "the divisor is not a constant"))
                                    .note(tr!(
                                        "割る数は正の整数の定数か、同じ単位の金額・数量の定数だけです（§2.3）。",
                                        "A divisor is a positive whole constant, or a constant amount or quantity in the same unit (§2.3)."
                                    ))
                                    .note(tr!(
                                        "割る数が業務のデータなら、それは率か、定数を引く表として書けるはずです。割合そのものを渡したいなら `rate` の入力にしてください。",
                                        "If the divisor is business data, it can be written as a rate, or as a table that looks the constant up. To pass a ratio in, make it a `rate` input."
                                    ))
                                    .note(tr!(
                                        "刻みが静的に決まらないと、生成コードは言語の除算に頼ることになり、Python は −∞ 方向、Go は 0 方向に丸めて答えが食い違います（§7.1）。",
                                        "Without a statically known step the generated code would fall back on the language's own division, and Python rounding toward -inf and Go toward zero would give different answers (§7.1)."
                                    )),
                            );
                            return Ty::Unknown;
                        }
                        // Dividing by a constant of the same dimension cancels it and
                        // leaves a plain number — this is the "one point per 100 yen" case.
                        // Anything else keeps the left side's type.
                        let same_dim = matches!(
                            (&lt, &rt),
                            (Ty::Money { .. }, Ty::Money { .. }) | (Ty::Qty { .. }, Ty::Qty { .. })
                        ) && lt.unifies(&rt);
                        if same_dim { Ty::Number } else { lt.clone() }
                    }
                }
            }
        }
    }

    fn mix(&mut self, a: &Ty, b: &Ty, sp: &Span, path: &str) {
        let note = match (a, b) {
            (Ty::Money { tax: Some(x), .. }, Ty::Money { tax: Some(y), .. }) if x != y => {
                tr!("税の変換は変換式ではなく表として書いてください（§2.1）。", "Write a tax conversion as a table, not as a conversion formula (§2.1).")
            }
            _ => tr!("次元の違う値は足せません。数量に応じた加算料金なら、それは表で書きます。", "Values of different dimensions cannot be added. A surcharge that depends on a quantity is written as a table."),
        };
        self.diags.push(
            Diag::error("E103", tr!("単位の混同: {a} に {b} を足しています", "Mixed units: adding {b} to {a}"))
                .at(format!("{path}:{}", sp.line))
                .mark(sp.clone(), "")
                .note(tr!("ヒント: {note}", "Hint: {note}")),
        );
    }

    fn enum_of_value(&self, v: &str) -> Option<String> {
        self.enums
            .iter()
            .find(|(_, vs)| vs.iter().any(|x| x == v))
            .map(|(n, _)| n.clone())
    }

    fn table(&mut self, t: &Table, path: &str) {
        let at = |line: usize| match &t.name {
            Some(n) => tr!("{path}:{line} 表 {}", "{path}:{line} table {}", n.text),
            None => tr!("{path}:{line} 例", "{path}:{line} examples"),
        };

        // Column headers must name something already in scope (§5.1).
        let mut col_ty: Vec<Ty> = Vec::new();
        for (name, sp) in &t.inputs {
            self.used.insert(name.clone());
            match self.syms.get(name) {
                Some(s) => col_ty.push(s.ty.clone()),
                None => {
                    if self.groups.contains_key(name) || self.enums.contains_key(name) {
                        col_ty.push(Ty::Unknown);
                    } else {
                        self.diags.push(
                            Diag::error("E012", tr!("列 `{name}` という名前は宣言されていません", "Column `{name}` is not a declared name"))
                                .at(at(sp.line))
                                .mark(sp.clone(), "")
                                .note(tr!("列に書けるのは 入力・導出・真偽/列挙の中間値です（§5.3）。", "A column can only be an input, a derived value, or a boolean/enum intermediate value (§5.3).")),
                        );
                        col_ty.push(Ty::Unknown);
                    }
                }
            }
        }

        // Output columns enter scope for later tables and for `result`.
        let mut out_ty: Vec<Ty> = Vec::new();
        for oc in &t.outputs {
            let ty = match &oc.ty {
                Some(tr) => self.resolve(tr),
                None => self.syms.get(&oc.name.text).map(|s| s.ty.clone()).unwrap_or(Ty::Unknown),
            };
            out_ty.push(ty.clone());
            if t.name.is_some() {
                self.syms.insert(
                    oc.name.text.clone(),
                    Sym { ty, span: oc.name.span.clone(), kind: SymKind::TableOut, contract_only: false },
                );
            }
        }

        // Collect the values the upstream table can produce. Only enums and booleans reach a
        // downstream column (§5.3).
        if t.name.is_some() {
            for (oi, oc) in t.outputs.iter().enumerate() {
                if !matches!(out_ty.get(oi), Some(Ty::Enum(_)) | Some(Ty::Bool)) {
                    continue;
                }
                let mut vs: Vec<String> = Vec::new();
                for row in &t.rows {
                    if let Some(OutCell::Name(w)) = row.outs.get(oi) {
                        if !vs.contains(w) {
                            vs.push(w.clone());
                        }
                    }
                }
                if !vs.is_empty() {
                    self.out_values.insert(oc.name.text.clone(), vs);
                }
            }
        }

        for row in &t.rows {
            for (ci, cell) in row.cells.iter().enumerate() {
                let Some(want) = col_ty.get(ci) else { continue };
                let sp = row.cell_spans.get(ci).unwrap_or(&row.span).clone();
                let sc = t
                    .inputs
                    .get(ci)
                    .map(|(n, _)| *self.scales.get(n).unwrap_or(&1))
                    .unwrap_or(1);
                self.cell(cell, want, sc, &sp, &at(row.span.line));
            }
            for (oi, oc) in row.outs.iter().enumerate() {
                let Some(want) = out_ty.get(oi) else { continue };
                let osp = row.out_spans.get(oi).unwrap_or(&row.span).clone();
                let ocol = t.outputs.get(oi).map(|o| o.name.text.clone()).unwrap_or_default();
                match oc {
                    OutCell::Lit(Lit::Num(n)) => match lit_value_in(n, want) {
                        None => self.diags.push(
                            Diag::error("E103", tr!("この列は {want} ですが `{}` が書かれています", "This column is {want}, but `{}` is written here", n.raw))
                                .at(at(row.span.line))
                                .mark(osp.clone(), ""),
                        ),
                        Some(v) => {
                            // §2.4: a tool that silently snaps to the declared rounding defeats
                            // itself as a tool for reviewing diffs. If the value is off the grid,
                            // let the author decide.
                            if let Some((m, g)) = self.roundings.get(&ocol).copied() {
                                if !v.on_grid(g) {
                                    let near = v.round_to(m, g);
                                    self.diags.push(
                                        Diag::error("E106", tr!("`{}` は丸めの刻みに載っていません", "`{}` is not on the rounding grid", n.raw))
                                            .at(at(row.span.line))
                                            .mark(osp.clone(), "")
                                            .note(tr!("出力 {ocol} の丸めは {}({}) です。", "The rounding of output {ocol} is {}({}).", m.name(), fmt_val(g, want)))
                                            .note(tr!("ヒント: {} と書くか、丸めの宣言のほうを直してください。", "Hint: write {} instead, or fix the rounding declaration.", fmt_val(near, want)))
                                            .note(tr!("黙って寄せることはしません。どちらが正しいかは業務の判断です（§2.4）。", "Nothing is snapped silently. Which one is right is a business decision (§2.4).")),
                                    );
                                }
                            }
                        }
                    },
                    OutCell::Name(w) => {
                        // An enum value, or (§3.2) the name of an input / derived / define.
                        self.used_values.insert(w.clone());
                        if let Some(s) = self.syms.get(w).cloned() {
                            self.used.insert(w.clone());
                            if !s.ty.unifies(want) {
                                self.diags.push(
                                    Diag::error("E103", tr!("この列は {want} ですが `{w}` は {} です", "This column is {want}, but `{w}` is {}", s.ty))
                                        .at(at(row.span.line))
                                        .mark(osp.clone(), ""),
                                );
                            }
                        } else if self.enum_of_value(w).is_none() && w != crate::kw::TRUE && w != crate::kw::FALSE {
                            self.diags.push(
                                Diag::error("E012", tr!("`{w}` は値の名前としても、宣言された名前としても見つかりません", "`{w}` is found neither as a value nor as a declared name"))
                                    .at(at(row.span.line))
                                    .mark(osp.clone(), "")
                                    .note(tr!("出力セルに書けるのは リテラルか名前（入力・導出・定義）だけです（§3.2）。式は書けません。", "An output cell holds only a literal or a name (an input, derived value, or definition) (§3.2). Expressions are not allowed.")),
                            );
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn cell(&mut self, cell: &Cell, want: &Ty, scale: i128, span: &Span, at: &str) {
        // Writing a present-side value in an optional column is correct. `none` arrives as
        // Cell::Nothing.
        let want = match want {
            Ty::Opt(inner) => inner.as_ref(),
            other => other,
        };
        let check_lit = |s: &mut Self, l: &Lit| match l {
            Lit::Num(n) => {
                let Some(v) = lit_value_in(n, want) else {
                    s.diags.push(
                        Diag::error("E103", tr!("この列は {want} ですが `{}` が書かれています", "This column is {want}, but `{}` is written here", n.raw))
                            .at(at.to_string())
                            .mark(span.clone(), "")
                            .note(if n.unit.is_none() {
                                tr!("単位が要ります。裸の数は書けません（§3）。", "A unit is required. A bare number cannot be written (§3).")
                            } else {
                                tr!("`{}` は {want} の単位ではありません。", "`{}` is not a unit of {want}.", n.raw)
                            }),
                    );
                    return;
                };
                // The runtime value is an integer count of the declared step (§2.1), so a
                // value between two steps has no representation. Rounding it silently would
                // move a boundary the reader can read on the page.
                if scale > 1 && !v.mul(Rat::int(scale)).is_int() {
                    let step = fmt_val(Rat::new(1, scale), want);
                    // The nearest value that is on the step, rounded half away from zero.
                    let k = v.mul(Rat::int(scale));
                    let half = if k.num < 0 { -k.den } else { k.den };
                    let near = fmt_val(Rat::new((k.num * 2 + half) / (k.den * 2), scale), want);
                    s.diags.push(
                        Diag::error("E114", tr!("`{}` はこの列の刻みに載っていません", "`{}` does not sit on this column's step", n.raw))
                            .at(at.to_string())
                            .mark(span.clone(), tr!("刻みは {step} です", "the step is {step}"))
                            .note(tr!(
                                "この列の型は刻みを {step} と宣言しているので、実行時の値はその整数倍だけです（§2.1）。",
                                "The column's type declares a step of {step}, so at runtime the value is a whole number of them (§2.1)."
                            ))
                            .note(tr!(
                                "`{near}` のように刻みに載る値に直すか、型の刻みを細かくしてください。黙って寄せると、読める境界と動く境界が食い違います。",
                                "Write a value on the step, such as `{near}`, or declare a finer step. Rounding it quietly would make the boundary on the page differ from the boundary in the code."
                            )),
                    );
                }
            }
            Lit::Date(..) => {
                if *want != Ty::Date && *want != Ty::Unknown {
                    s.diags.push(
                        Diag::error("E103", tr!("この列は {want} ですが 日付 が書かれています", "This column is {want}, but a date is written here"))
                            .at(at.to_string())
                            .mark(span.clone(), ""),
                    );
                }
            }
            Lit::Word(w) => {
                if w == crate::kw::TRUE || w == crate::kw::FALSE {
                    if *want != Ty::Bool && *want != Ty::Unknown {
                        s.diags.push(
                            Diag::error("E103", tr!("この列は {want} ですが 真偽 が書かれています", "This column is {want}, but a boolean is written here"))
                                .at(at.to_string())
                                .mark(span.clone(), ""),
                        );
                    }
                    return;
                }
                if let Some((_, members)) = s.groups.get(w).cloned() {
                    s.used.insert(w.clone());
                    for m in members {
                        s.used_values.insert(m);
                    }
                    return;
                }
                s.used_values.insert(w.clone());
                match (&s.enum_of_value(w), want) {
                    (Some(e), Ty::Enum(w2)) if e == w2 => {}
                    (_, Ty::Unknown) => {}
                    (None, _) => s.diags.push(
                        Diag::error("E012", tr!("`{w}` は値としてもグループとしても見つかりません", "`{w}` is found neither as a value nor as a group"))
                            .at(at.to_string())
                            .mark(span.clone(), ""),
                    ),
                    (Some(e), _) => s.diags.push(
                        Diag::error("E103", tr!("この列は {want} ですが `{w}`（{e} の値）が書かれています", "This column is {want}, but `{w}` (a value of {e}) is written here"))
                            .at(at.to_string())
                            .mark(span.clone(), ""),
                    ),
                }
            }
            _ => {}
        };
        match cell {
            Cell::DontCare | Cell::Nothing => {}
            Cell::Lit(l) => check_lit(self, l),
            Cell::Set(ls) | Cell::Not(ls) => {
                for l in ls {
                    check_lit(self, l);
                }
            }
            Cell::Cmp(cs) => {
                for (_, l) in cs {
                    check_lit(self, l);
                }
            }
        }
    }
}

impl Checked {
    pub fn ty_of(&self, name: &str) -> Option<Ty> {
        self.syms.get(name).map(|s| s.ty.clone())
    }

    /// The factor a named value is multiplied by to become the integer it travels as.
    ///
    /// One for everything with a unit of its own — yen, grams, centimetres are already
    /// integers in the unit they declare. A **rate has no unit**, so it travels as the
    /// number of steps its declaration names: `10%` under `rate[step 1%]` is 10 (§2.1,
    /// §10.2). A rate with no step declared falls back to hundredths, which is what the
    /// generated code assumes for a column whose literals fixed no finer grid.
    ///
    /// Everything that turns a value into an integer — the vectors, the fixtures, a
    /// report, a witness, the entry guard of the generated code — asks here. Two answers
    /// for one value is how a rate came to mean 1 on one side of the wire and 100 on the
    /// other.
    pub fn wire_scale(&self, name: &str) -> i128 {
        match self.ty_of(name) {
            Some(Ty::Rate) => *self.scales.get(name).unwrap_or(&100),
            _ => 1,
        }
    }
}

/// A value as the integer it travels as, at the scale `Checked::wire_scale` gives.
pub fn wire_int(v: Rat, scale: i128) -> i128 {
    let s = v.mul(Rat::int(scale));
    s.num / s.den
}

/// The same journey back: an integer off the wire into the value it stands for.
pub fn from_wire(n: i128, scale: i128) -> Rat {
    Rat::new(n, scale)
}

pub fn lit_ty_pub(n: &crate::lex::Num) -> Ty {
    lit_ty(n)
}

/// Entry point so that `region` uses the same conversion.
pub fn lit_value_in_pub(n: &crate::lex::Num, want: &Ty) -> Option<Rat> {
    lit_value_in(n, want)
}

/// The §1.6 rendering uses the same write-back. Showing an approver `1000000円` forces a
/// mental conversion before it can be matched against the `100万円` in the rule source.
pub fn fmt_big_pub(v: Rat) -> String {
    fmt_big(v)
}

/// Large amounts are written back with 万 and 億. Answering `1000000円` to an author who
/// wrote `100万円` forces a mental conversion before they can fix anything.
fn fmt_big(v: Rat) -> String {
    if !v.is_int() {
        return format!("{v}");
    }
    let n = v.num;
    if n != 0 && n % 1_000_000_000_000 == 0 {
        return format!("{}兆", n / 1_000_000_000_000);
    }
    if n != 0 && n % 100_000_000 == 0 {
        return format!("{}億", n / 100_000_000);
    }
    if n != 0 && n % 10_000 == 0 {
        return format!("{}万", n / 10_000);
    }
    format!("{n}")
}

/// One unit of the type, as it is written: `1円`, `1g`, `1%`. It is the finest grid a
/// rounding declaration can name, so `round down(1円)` is the edit that removes E104 while
/// changing the answer the least. **Which direction and which grid are right is a business
/// decision**, which the notes say and `fix.text` cannot.
fn unit_one(ty: &Ty) -> String {
    match ty {
        Ty::Money { cur, .. } => format!("1{cur}"),
        Ty::Qty { unit, .. } => format!("1{unit}"),
        Ty::Rate => "1%".into(),
        Ty::Number => "1".into(),
        _ => "1".into(),
    }
}

fn fmt_val(v: Rat, ty: &Ty) -> String {
    match ty {
        Ty::Money { cur, .. } => format!("{}{cur}", fmt_big(v)),
        Ty::Qty { unit, .. } => format!("{v}{unit}"),
        Ty::Rate => format!("{}%", v.mul(Rat::int(100))),
        _ => format!("{v}"),
    }
}

/// Lower `range >=a <=b` to an interval. Open bounds are treated as closed (overestimating
/// is the safe side).
fn bounds_of(r: &Range, ty: &Ty) -> (Option<Rat>, Option<Rat>) {
    let (mut lo, mut hi) = (None, None);
    for (op, l) in &r.bounds {
        let v = match l {
            Lit::Num(n) => match lit_value_in(n, ty) {
                Some(v) => v,
                None => continue,
            },
            Lit::Date(y, m, d) if *ty == Ty::Date => date_ord(*y, *m, *d),
            _ => continue,
        };
        match op {
            CmpOp::Ge | CmpOp::Gt => lo = Some(v),
            CmpOp::Le | CmpOp::Lt => hi = Some(v),
        }
    }
    (lo, hi)
}

impl Checked {
    /// E112: the declared range must contain the reachable interval obtained by interval
    /// arithmetic over the input ranges. Otherwise the completeness check answers "complete"
    /// without ever seeing the values that actually occur.
    fn derived_range(&mut self, d: &DerivedDecl, ty: &Ty, path: &str) {
        let Some((rl, rh)) = self.interval(&d.expr, ty) else { return };
        let Some(rg) = &d.range else { return };
        let (dl, dh) = bounds_of(rg, ty);
        let too_low = matches!((dl, Some(rl)), (Some(a), Some(b)) if b.cmp_to(a) == std::cmp::Ordering::Less);
        let too_high = matches!((dh, Some(rh)), (Some(a), Some(b)) if b.cmp_to(a) == std::cmp::Ordering::Greater);
        if too_low || too_high {
            self.diags.push(
                Diag::error("E112", tr!("導出の範囲が、実際に到達しうる値を含んでいません", "The range of the derived value does not contain the values it can actually reach"))
                    .at(tr!("{path}:{} 導出 {}", "{path}:{} derived value {}", rg.span.line, d.name.text))
                    .fix(
                        crate::diag::FixKind::WidenRange,
                        format!("{} >={} <={}", crate::kw::RANGE, fmt_val(rl, ty), fmt_val(rh, ty)),
                    )
                    .mark(rg.span.clone(), tr!("到達区間は >={} <={} です", "the reachable interval is >={} <={}", fmt_val(rl, ty), fmt_val(rh, ty)))
                    .note(tr!("範囲が狭いと、網羅性検査が実際に起きる値を見ないまま「完全」と答えます。", "With a range that is too narrow, the completeness check answers \"complete\" without ever seeing the values that actually occur."))
                    .note(tr!(
                        "ヒント: 範囲 >={} <={} に広げてください。到達しない分まで広げても、実現不能な領域として検査が篩うので害はありません。",
                        "Hint: widen the range to >={} <={}. Widening it past what is reachable does no harm; the checks sieve that part out as an infeasible region.",
                        fmt_val(rl, ty),
                        fmt_val(rh, ty)
                    )),
            );
        }
    }

    /// Interval arithmetic from the declared input ranges. The right-hand side of a derived
    /// value is a linear combination of inputs only, so addition, subtraction, and constant
    /// multiples are all that is needed (§5.2).
    fn interval(&self, e: &Expr, ty: &Ty) -> Option<(Rat, Rat)> {
        match e {
            Expr::Name(n, _) => {
                if let Some((lo, hi)) = self.ranges.get(n) {
                    return Some(((*lo)?, (*hi)?));
                }
                // A rate is taken to lie within 0..100%. No range declaration is required for
                // it, so this is the conservative view.
                match self.syms.get(n).map(|s| s.ty.clone()) {
                    Some(Ty::Rate) => Some((Rat::zero(), Rat::int(1))),
                    _ => None,
                }
            }
            Expr::Lit(Lit::Num(n), _) => {
                // The expression's own type is only a hint: `商品合計 × 10%` and
                // `税込金額 ÷ 100円` both mix dimensions on purpose (§2.3), so a literal that
                // does not belong to `ty` is read in its own unit instead of giving up.
                let v = lit_value_in(n, ty).or_else(|| lit_value_in(n, &lit_ty(n)))?;
                Some((v, v))
            }
            Expr::Bin(l, op, r, _) => {
                let (al, ah) = self.interval(l, ty)?;
                let (bl, bh) = self.interval(r, ty)?;
                match op {
                    BinOp::Add => Some((al.add(bl), ah.add(bh))),
                    BinOp::Sub => Some((al.sub(bh), ah.sub(bl))),
                    BinOp::Mul | BinOp::Div => {
                        // Minimum and maximum over the endpoint combinations. A rate is not
                        // necessarily non-negative, so all four are examined.
                        let f = |x: Rat, y: Rat| if *op == BinOp::Mul { x.mul(y) } else { x.div(y) };
                        let mut vs = [f(al, bl), f(al, bh), f(ah, bl), f(ah, bh)];
                        vs.sort_by(|a, b| a.cmp_to(*b));
                        Some((vs[0], vs[3]))
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

/// Whether a public name still has to be given an ASCII alias. One written in ASCII already
/// serves as its own identifier in every target language (§1.3).
fn needs_alias(n: &Name) -> bool {
    if n.ascii.is_some() {
        return false;
    }
    let mut cs = n.text.chars();
    let first_ok = cs.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    !(first_ok && cs.all(|c| c.is_ascii_alphanumeric() || c == '_'))
}

/// The value of a constant divisor, which §2.3 requires to be a positive whole number of its
/// own unit: `100円` is 100 and `3` is 3.
fn const_value(e: &Expr) -> Option<i128> {
    let Expr::Lit(Lit::Num(n), _) = e else { return None };
    let v = lit_value_in(n, &lit_ty(n))?;
    if v.den == 1 && v.num > 0 { Some(v.num) } else { None }
}

/// The reciprocal of the step from the declaration: 1000 for `rate[step 0.1%]`.
fn scale_of_type(tr: &TypeRef, ty: &Ty) -> i128 {
    for a in &tr.args {
        if let TypeArg::Scaled(w, n) = a {
            if w == crate::kw::STEP {
                if let Some(v) = lit_value_in(n, ty) {
                    if v.num != 0 {
                        return v.den * (1 / v.num.max(1)).max(1);
                    }
                }
            }
        }
    }
    1
}

impl Checked {
    /// The scale of an expression: the lcm for addition and subtraction, the product for
    /// multiplication, and the divisor's numerator times for division by a constant.
    fn scale(&self, e: &Expr) -> Option<i128> {
        fn lcm(a: i128, b: i128) -> i128 {
            fn g(mut a: i128, mut b: i128) -> i128 {
                while b != 0 {
                    let t = a % b;
                    a = b;
                    b = t;
                }
                a.max(1)
            }
            a / g(a, b) * b
        }
        match e {
            Expr::Name(n, _) => Some(*self.scales.get(n).unwrap_or(&1)),
            Expr::Lit(Lit::Num(n), _) => {
                // The scale has to be one at which the literal is a whole number. A decimal
                // moves it by a power of ten: `0.5%` is 1/200, so 100 would not clear it.
                let d = 10i128.checked_pow(u32::try_from(n.frac.len()).ok()?)?;
                Some(d * match n.unit.as_deref() {
                    Some("%") => 100,
                    Some("銭") => 100,
                    _ => 1,
                })
            }
            Expr::Bin(l, op, r, _) => {
                let a = self.scale(l)?;
                let b = self.scale(r)?;
                Some(match op {
                    BinOp::Add | BinOp::Sub => lcm(a, b),
                    BinOp::Mul => a * b,
                    // Dividing by the constant k does not shrink the stored integer: it
                    // multiplies the scale by k, so the division is exact and no language's
                    // rounding convention gets a say (§7.1). The divisor is a constant —
                    // §2.3 forbids dividing by a variable, and E103 says so.
                    BinOp::Div => a.checked_mul(const_value(r)?)?,
                    _ => 1,
                })
            }
            Expr::Call(name, args, _) => {
                if crate::num::RoundMode::parse(name).is_some() {
                    // After rounding, the scale is back to the grid's step.
                    return args.get(1).and_then(|g| self.scale(g));
                }
                args.first().and_then(|a| self.scale(a))
            }
            _ => Some(1),
        }
    }

    /// E108: prove from the declared ranges and scales that an intermediate value fits in
    /// int64. If it cannot be proved, stop: rather than overflowing silently, ask the business
    /// where to round.
    fn overflow(&mut self, e: &Expr, ty: &Ty, span: &Span, path: &str, name: &str) {
        let (Some((lo, hi)), Some(sc)) = (self.interval(e, ty), self.scale(e)) else { return };
        // The stored integer is value × reciprocal of the step. Use whichever endpoint of the
        // interval has the larger absolute value.
        let abs = |r: Rat| if r.num < 0 { Rat::zero().sub(r) } else { r };
        let mag = if abs(lo).cmp_to(abs(hi)) == std::cmp::Ordering::Greater { abs(lo) } else { abs(hi) };
        let stored_r = mag.mul(Rat::int(sc));
        let stored = stored_r.num / stored_r.den;
        if stored > i64::MAX as i128 {
            self.diags.push(
                Diag::error("E108", tr!("{name} が int64 に収まることを証明できません", "Cannot prove that {name} fits in int64"))
                    .at(format!("{path}:{}", span.line))
                    .mark(span.clone(), "")
                    .note(tr!(
                        "到達区間は {} から {} で、刻みが 1/{sc} なので、格納される整数は最大 {stored} になります。",
                        "The reachable interval is {} to {} and the step is 1/{sc}, so the stored integer reaches {stored}.",
                        fmt_val(lo, ty),
                        fmt_val(hi, ty)
                    ))
                    .note(tr!("ヒント: 入力の範囲を狭めるか、途中で丸めを一つ入れてください。", "Hint: narrow the input ranges, or insert one rounding step along the way."))
                    .note(tr!("どこで丸めるかは円が動く業務の判断なので、道具が勝手に決めません（§7.1）。", "Where to round is a business decision that moves yen, so the tool does not decide it on its own (§7.1).")),
            );
        }
    }
}

impl Checked {
    /// Walk the atoms of a boolean definition. Conjunction, disjunction, and negation are
    /// allowed; a comparison must have the name of an input or derived value on one side and
    /// a constant on the other.
    fn atoms(&mut self, e: &Expr, owner: &str, path: &str) {
        match e {
            Expr::Bin(l, op, r, sp) => {
                use BinOp::*;
                if matches!(op, Le | Ge | Lt | Gt | Eq) {
                    let axis_of = |x: &Expr, s: &Checked| -> Option<Ty> {
                        let Expr::Name(n, _) = x else { return None };
                        match s.syms.get(n) {
                            Some(v) if matches!(v.kind, SymKind::Input | SymKind::Derived) => {
                                Some(v.ty.clone())
                            }
                            _ => None,
                        }
                    };
                    // First form: a unary test on an axis (a name and a literal).
                    let ok = |x: &Expr, y: &Expr, s: &Checked| -> bool {
                        axis_of(x, s).is_some() && matches!(y, Expr::Lit(..))
                    };
                    // Second form: a comparison between two axes of the same type. Numeric pairs
                    // stay E113, though: what could be analyzed exactly as a derived value must
                    // not be left opaque out of laziness (a discipline of precision, not a matter
                    // of soundness; §5.3).
                    let pair = match (axis_of(l, self), axis_of(r, self)) {
                        (Some(a), Some(b)) if a.unifies(&b) => Some(a),
                        _ => None,
                    };
                    if let Some(t) = &pair {
                        if !t.is_numeric() {
                            return;
                        }
                        self.diags.push(
                            Diag::error("E113", tr!("定義 {owner} が {t} どうしを直接比べています", "Definition {owner} compares two {t} values directly"))
                                .at(format!("{path}:{}", sp.line))
                                .mark(sp.clone(), "")
                                .note(tr!("数値どうしの比較は、差を `{}` として宣言してから定数と比べてください。そのほうが厳密に解析できます。", "To compare two numbers, declare their difference with `{}` and compare that with a constant. It can then be analyzed exactly.", crate::kw::DERIVE))
                                .note(tr!("導出にできない型（日付、列挙）どうしなら、そのまま比べられます（§5.3）。", "Two values of a type that cannot be derived (a date, an enum) may be compared directly (§5.3).")),
                        );
                        return;
                    }
                    if !ok(l, r, self) && !ok(r, l, self) {
                        let hint = if matches!(**l, Expr::Name(..)) && matches!(**r, Expr::Name(..)) {
                            tr!("入力の線形結合なら `{}` として宣言してから比べてください。表の出力と比べたいなら、その表に真偽の出力列を足すのが正しい書き方です。", "A linear combination of inputs must be declared with `{}` before it is compared. To compare with a table output, the right way is to add a boolean output column to that table.", crate::kw::DERIVE)
                        } else {
                            tr!("比較の片側は入力か導出の名前、もう片側は定数である必要があります。", "One side of the comparison must be the name of an input or derived value, and the other side a constant.")
                        };
                        self.diags.push(
                            Diag::error("E113", tr!("定義 {owner} の条件が、値ひとつと定数の比較になっていません", "The condition of definition {owner} is not one value compared with a constant"))
                                .at(format!("{path}:{}", sp.line))
                                .mark(sp.clone(), "")
                                .note(hint.to_string())
                                .note(tr!("この制限が、完全性と重複の検査が有限で終わることの土台です（§5.3、§6.2）。", "This restriction is what makes the completeness and overlap checks finite (§5.3, §6.2).")),
                        );
                    }
                    return;
                }
                self.atoms(l, owner, path);
                self.atoms(r, owner, path);
            }
            Expr::Call(_, args, _) => {
                for a in args {
                    self.atoms(a, owner, path);
                }
            }
            _ => {}
        }
    }
}

fn collect_names(e: &Expr, out: &mut Vec<String>) {
    match e {
        Expr::Name(n, _) => out.push(n.clone()),
        Expr::Bin(l, _, r, _) => {
            collect_names(l, out);
            collect_names(r, out);
        }
        Expr::Call(_, args, _) => {
            for a in args {
                collect_names(a, out);
            }
        }
        _ => {}
    }
}
