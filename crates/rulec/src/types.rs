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
    /// 質量[g] / 長さ[cm] — a dimension plus the declared unit.
    Qty { dim: String, unit: String },
    Rate,
    Bool,
    Str,
    Date,
    Unknown,
}

impl std::fmt::Display for Ty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ty::Enum(n) => write!(f, "{n}"),
            Ty::Money { cur, tax: Some(t) } => write!(f, "金額[{cur}, {t}]"),
            Ty::Money { cur, tax: None } => write!(f, "金額[{cur}]"),
            Ty::Qty { dim, unit } => write!(f, "{dim}[{unit}]"),
            Ty::Rate => write!(f, "率"),
            Ty::Bool => write!(f, "真偽"),
            Ty::Str => write!(f, "文字列"),
            Ty::Date => write!(f, "日付"),
            Ty::Unknown => write!(f, "?"),
        }
    }
}

impl Ty {
    fn is_numeric(&self) -> bool {
        matches!(self, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate)
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
        "g" => ("質量", Rat::int(1)),
        "kg" => ("質量", Rat::int(1000)),
        "cm" => ("長さ", Rat::int(1)),
        "m" => ("長さ", Rat::int(100)),
        "円" => ("金額", Rat::int(1)),
        "銭" => ("金額", Rat::new(1, 100)),
        "%" => ("率", Rat::new(1, 100)),
        _ => return None,
    })
}

/// 日付は**元期からの通算日**で持つ（先発グレゴリオ暦、1970-01-01 = 0）。
///
/// `y*10000 + m*100 + d` でも順序は保たれるが、月末に実在しない整数の隙間
/// （20260332〜20260400）ができる。`<=2026-03-31` と `>=2026-04-01` で敷き詰めた
/// 表は実際には完全なのに、幻の整数が覆われていないとして**偽の E101** が出る。
/// 両端含みの敷き詰めは業務の自然な書き方（§3.1）なので、必ず踏まれる。
///
/// 通算日なら隣接が +1 で一致するので、境界の ±1、空判定、隣接の合流、
/// 証人の書き戻しが、全部そのまま既存の整数の機構で正しくなる。
pub fn date_ord(y: i32, m: u32, d: u32) -> Rat {
    Rat::int(days_from_civil(y, m as i64, d as i64))
}

pub fn ord_to_date(v: Rat) -> (i32, u32, u32) {
    let (y, m, d) = civil_from_days(v.num / v.den);
    (y as i32, m as u32, d as u32)
}

/// Howard Hinnant の days_from_civil。先発グレゴリオ暦。
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
            assert_eq!(ord_to_date(o), (y, m, d), "{y}-{m}-{d} の往復");
        }
        // 月末と翌月初が隣接する。ここが幻の整数の隙間を作らない根拠。
        let a = date_ord(2026, 3, 31);
        let b = date_ord(2026, 4, 1);
        assert_eq!(b.sub(a), Rat::int(1), "3/31 と 4/1 は隣り合う");
        // 閏日
        let c = date_ord(2024, 2, 28);
        assert_eq!(date_ord(2024, 2, 29).sub(c), Rat::int(1));
        assert_eq!(date_ord(2024, 3, 1).sub(date_ord(2024, 2, 29)), Rat::int(1));
        // 平年の 2/28 の翌日は 3/1
        assert_eq!(date_ord(2025, 3, 1).sub(date_ord(2025, 2, 28)), Rat::int(1));
    }
}

/// Value of a numeric literal, expressed in `want`'s declared unit.
/// `None` means the literal's unit does not belong to `want`'s dimension.
fn lit_value_in(n: &crate::lex::Num, want: &Ty) -> Option<Rat> {
    let digits: i128 = n.digits.parse().ok()?;
    let mut v = Rat::int(digits * n.mult as i128);
    if n.neg {
        v = Rat::zero().sub(v);
    }
    let unit = n.unit.as_deref()?;
    let (dim, f) = unit_info(unit)?;
    match want {
        Ty::Money { .. } if dim == "金額" => Some(v.mul(f)),
        Ty::Qty { dim: d, unit: du } if dim == *d => {
            let (_, fd) = unit_info(du)?;
            Some(v.mul(f).div(fd))
        }
        Ty::Rate if dim == "率" => Some(v.mul(f)),
        _ => None,
    }
}

/// Type of a numeric literal read on its own, before any expectation.
fn lit_ty(n: &crate::lex::Num) -> Ty {
    match n.unit.as_deref().and_then(unit_info) {
        Some(("金額", _)) => Ty::Money { cur: n.unit.clone().unwrap(), tax: None },
        Some(("率", _)) => Ty::Rate,
        Some((d, _)) => Ty::Qty { dim: d.to_string(), unit: n.unit.clone().unwrap() },
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
    /// 出力名 → 丸め（モードと格子）。E106 はリテラルがこの格子に載るかを見る。
    pub roundings: HashMap<String, (RoundMode, Rat)>,
    /// 入力名 → 宣言範囲。E112 が到達区間の包含を見るのに使う。
    pub ranges: HashMap<String, (Option<Rat>, Option<Rat>)>,
    /// 表の出力名 → その表が実際に出しうる値。上流が決して出さない値を名指しする
    /// 行は、下流では到達不能になる（§11 E102 の変種）。
    pub out_values: HashMap<String, Vec<String>>,
    /// 真偽の定義名 → その式が名指しする名前。証人を構成できるかの判定に使う。
    pub define_deps: HashMap<String, Vec<String>>,
    /// 導出名 → 依存する入力名。二つの導出が入力を共有するかを見るのに使う。
    /// 共有していると、導出ごとに独立な区間の篩は従属を見られない（§6.2）。
    pub derived_deps: HashMap<String, Vec<String>>,
    /// 名前 → 刻みの逆数。値が 1/k の倍数であることを表す。§7.1 の
    /// 「int64 一本＋静的有理スケール」で、格納される整数は 値×k になる。
    pub scales: HashMap<String, i128>,
    /// Enum name -> values, in declaration order (§6.3 picks the first as a witness).
    pub enums: HashMap<String, Vec<String>>,
    /// Group name -> the enum it belongs to and its members.
    pub groups: HashMap<String, (String, Vec<String>)>,
    pub used: HashSet<String>,
    /// セルや式で名指しされた列挙値。W111 が「どの行にも現れない値」を出すのに使う。
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

    // 標準/都道府県 is the only import the corpus needs; its 47 values come from the
    // prelude (§2.2). Until the prelude is a real file, accept any value for it.
    for (p, sp) in &f.imports {
        match crate::prelude::lookup(p) {
            Some((name, values)) => {
                c.enums.insert(name, values);
            }
            None => c.diags.push(
                Diag::error("E013", format!("`{p}` という取込先はありません"))
                    .at(at(sp.line))
                    .mark(sp.clone(), "")
                    .note("組み込みは 標準/都道府県 だけです（§2.2）。"),
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
    for o in &f.outputs {
        let ty = c.resolve(&o.ty);
        // §7.2: a numeric output must declare its rounding.
        if ty.is_numeric() && o.rounding.is_none() {
            c.diags.push(
                Diag::error("E104", "出力に丸めの宣言がありません")
                    .at(at(o.span.line))
                    .mark(o.ty.span.clone(), "丸め の宣言がありません")
                    .note(format!("出力 {} は {} です。", o.name.text, ty)),
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

    // §1.3: the public face needs ASCII aliases; the inside does not.
    let mut missing: Vec<String> = Vec::new();
    if f.name.ascii.is_none() {
        missing.push(format!("規則 {}", f.name.text));
    }
    for i in &f.inputs {
        if i.name.ascii.is_none() {
            missing.push(format!("入力 {}", i.name.text));
        }
    }
    for o in &f.outputs {
        if o.name.ascii.is_none() {
            missing.push(format!("出力 {}", o.name.text));
        }
    }
    if !missing.is_empty() {
        c.diags.push(
            Diag::error("E011", "公開面の名前に ASCII 別名がありません")
                .at(at(f.name.span.line))
                .mark(f.name.span.clone(), "")
                .note(format!("不足: {}", missing.join(" / ")))
                .note("Go の公開識別子は先頭が大文字である必要があり、漢字とかなは大文字を持ちません（§1.3）。")
                .note("宣言の位置に丸括弧で書いてください。例: 届け先(dest)"),
        );
    }

    // §5.1: items are a define-before-use pipeline, so a single forward pass is enough.
    for it in &f.items {
        match it {
            Item::Derived(d) => {
                let ty = c.resolve(&d.ty);
                let got = c.expr_ty(&d.expr, path);
                c.check_same(&ty, &got, &d.span, path, "導出");
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
                c.check_same(&ty, &got, &d.span, path, "定義");
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

    // §5.3 E113: 真偽定義の原子は、入力か導出への単項テストに限る。
    // ここを緩めると `x − y >= c` の半空間が列に入り、§6.2 の箱代数が崩れる。
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
            c.check_same(&s.ty, &got, &r.span, path, "結果");
        }
    }

    // §11 W111: declarations nothing names. `契約のみ` silences a range-guard-only input.
    for i in &f.inputs {
        if !c.used.contains(&i.name.text) && !i.contract_only {
            c.diags.push(
                Diag::warning("W111", format!("入力 {} はどの表でも使われていません", i.name.text))
                    .at(at(i.span.line))
                    .mark(i.name.span.clone(), "どの列にも現れません")
                    .note("本来使うべき列の書き忘れかもしれません。")
                    .note("範囲の入口検査としてだけ効かせるつもりなら、宣言に `契約のみ` を付けてください（§11 W111）。"),
            );
        }
    }
    for it in &f.items {
        if let Item::Derived(d) = it {
            if !c.used.contains(&d.name.text) {
                c.diags.push(
                    Diag::warning("W111", format!("導出 {} はどこでも使われていません", d.name.text))
                        .at(at(d.span.line))
                        .mark(d.name.span.clone(), "どの列にも式にも現れません")
                        .note("使わない導出は、検査の軸を一本増やすだけです。消すか、使ってください。"),
                );
            }
        }
    }
    // 対象は「このファイルで宣言された型」に限る。取込した型（都道府県 47 値）は
    // `以外: 沖縄` のような書き方で 45 値が名指しされないのが正常なので、
    // 値ごとの印を書かせると警告チャンネルごと壊れる。
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
                Diag::warning("W111", format!("型 {} の値がどの行にも現れません", e.name.text))
                    .at(at(e.span.line))
                    .mark(e.name.span.clone(), "")
                    .note(format!("現れない値: {}", names.join(" / ")))
                    .note("完全性検査は通っていても、その値に当たる行が `-` に吸われているだけかもしれません。"),
            );
        }
    }
    for g in &f.groups {
        if !c.used.contains(&g.name.text) {
            c.diags.push(
                Diag::warning("W111", format!("群 {} はどのセルでも使われていません", g.name.text))
                    .at(at(g.span.line))
                    .mark(g.name.span.clone(), ""),
            );
        }
    }
    c
}

impl Checked {
    fn resolve(&mut self, t: &TypeRef) -> Ty {
        match t.base.as_str() {
            "金額" => {
                let mut it = t.args.iter().filter_map(|a| match a {
                    TypeArg::Word(w) => Some(w.clone()),
                    _ => None,
                });
                Ty::Money { cur: it.next().unwrap_or_else(|| "円".into()), tax: it.next() }
            }
            "質量" | "長さ" => {
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
            "率" => Ty::Rate,
            "真偽" => Ty::Bool,
            "文字列" => Ty::Str,
            "日付" => Ty::Date,
            other => Ty::Enum(other.to_string()),
        }
    }

    fn check_same(&mut self, want: &Ty, got: &Ty, span: &Span, path: &str, what: &str) {
        if !want.unifies(got) && *got != Ty::Unknown {
            self.diags.push(
                Diag::error("E103", format!("型が合いません: {want} に {got} を入れています"))
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
                            Diag::error("E012", format!("`{n}` という名前は宣言されていません"))
                                .at(format!("{path}:{}", sp.line))
                                .mark(sp.clone(), "")
                                .note("上の行までに 入力 / 導出 / 定義 / 表の出力 として宣言されている必要があります（§5.1）。"),
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
                    if w == "真" || w == "偽" {
                        Ty::Bool
                    } else {
                        self.enum_of_value(w).map(Ty::Enum).unwrap_or(Ty::Unknown)
                    }
                }
            },
            Expr::Call(name, args, sp) => {
                let ats: Vec<Ty> = args.iter().map(|a| self.expr_ty(a, path)).collect();
                match name.as_str() {
                    "切り捨て" | "切り上げ" | "四捨五入" | "銀行家丸め" => {
                        ats.first().cloned().unwrap_or(Ty::Unknown)
                    }
                    "最小" | "最大" => {
                        if ats.len() == 2 && !ats[0].unifies(&ats[1]) {
                            self.diags.push(
                                Diag::error(
                                    "E103",
                                    format!("{name} の二つの引数の型が違います: {} と {}", ats[0], ats[1]),
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
                        // §2.3: 金額 × 率 は未丸め金額。金額 × 金額 は禁止。
                        (Ty::Money { .. }, Ty::Rate) => lt.clone(),
                        (Ty::Rate, Ty::Money { .. }) => rt.clone(),
                        (Ty::Qty { .. }, Ty::Rate) | (Ty::Rate, Ty::Qty { .. }) => {
                            if matches!(lt, Ty::Rate) { rt } else { lt }
                        }
                        (Ty::Money { .. }, Ty::Money { .. }) => {
                            self.diags.push(
                                Diag::error("E103", "金額どうしを掛けています")
                                    .at(format!("{path}:{}", sp.line))
                                    .mark(sp.clone(), "")
                                    .note("合成次元は業務ルールに現れないので、モデリングの誤りとして止めます（§2.1）。"),
                            );
                            Ty::Unknown
                        }
                        (Ty::Rate, Ty::Rate) => Ty::Rate,
                        _ => lt.clone(),
                    },
                    Div => lt.clone(),
                }
            }
        }
    }

    fn mix(&mut self, a: &Ty, b: &Ty, sp: &Span, path: &str) {
        let note = match (a, b) {
            (Ty::Money { tax: Some(x), .. }, Ty::Money { tax: Some(y), .. }) if x != y => {
                "税の変換は変換式ではなく表として書いてください（§2.1）。".to_string()
            }
            _ => "次元の違う値は足せません。数量に応じた加算料金なら、それは表で書きます。".to_string(),
        };
        self.diags.push(
            Diag::error("E103", format!("単位の混同: {a} に {b} を足しています"))
                .at(format!("{path}:{}", sp.line))
                .mark(sp.clone(), "")
                .note(format!("ヒント: {note}")),
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
            Some(n) => format!("{path}:{line} 表 {}", n.text),
            None => format!("{path}:{line} 例"),
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
                            Diag::error("E012", format!("列 `{name}` という名前は宣言されていません"))
                                .at(at(sp.line))
                                .mark(sp.clone(), "")
                                .note("列に書けるのは 入力・導出・真偽/列挙の中間値です（§5.3）。"),
                        );
                        col_ty.push(Ty::Unknown);
                    }
                }
            }
        }

        // Output columns enter scope for later tables and for 結果.
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

        // 上流が出しうる値を集めておく。列挙と真偽だけが下流の列に来る（§5.3）。
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
                self.cell(cell, want, &sp, &at(row.span.line));
            }
            for (oi, oc) in row.outs.iter().enumerate() {
                let Some(want) = out_ty.get(oi) else { continue };
                let osp = row.out_spans.get(oi).unwrap_or(&row.span).clone();
                let ocol = t.outputs.get(oi).map(|o| o.name.text.clone()).unwrap_or_default();
                match oc {
                    OutCell::Lit(Lit::Num(n)) => match lit_value_in(n, want) {
                        None => self.diags.push(
                            Diag::error("E103", format!("この列は {want} ですが `{}` が書かれています", n.raw))
                                .at(at(row.span.line))
                                .mark(osp.clone(), ""),
                        ),
                        Some(v) => {
                            // §2.4: 宣言された丸めに黙って寄せる道具は、差分を見る道具として自殺である。
                            // 格子に載っていなければ、書いた人に決めさせる。
                            if let Some((m, g)) = self.roundings.get(&ocol).copied() {
                                if !v.on_grid(g) {
                                    let near = v.round_to(m, g);
                                    self.diags.push(
                                        Diag::error("E106", format!("`{}` は丸めの格子に載っていません", n.raw))
                                            .at(at(row.span.line))
                                            .mark(osp.clone(), "")
                                            .note(format!("出力 {ocol} の丸めは {}({}) です。", m.name(), fmt_val(g, want)))
                                            .note(format!("ヒント: {} と書くか、丸めの宣言のほうを直してください。", fmt_val(near, want)))
                                            .note("黙って寄せることはしません。どちらが正しいかは業務の判断です（§2.4）。"),
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
                                    Diag::error("E103", format!("この列は {want} ですが `{w}` は {} です", s.ty))
                                        .at(at(row.span.line))
                                        .mark(osp.clone(), ""),
                                );
                            }
                        } else if self.enum_of_value(w).is_none() && w != "真" && w != "偽" {
                            self.diags.push(
                                Diag::error("E012", format!("`{w}` は値の名前としても、宣言された名前としても見つかりません"))
                                    .at(at(row.span.line))
                                    .mark(osp.clone(), "")
                                    .note("出力セルに書けるのは リテラルか名前（入力・導出・定義）だけです（§3.2）。式は書けません。"),
                            );
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn cell(&mut self, cell: &Cell, want: &Ty, span: &Span, at: &str) {
        let mut check_lit = |s: &mut Self, l: &Lit| match l {
            Lit::Num(n) => {
                if lit_value_in(n, want).is_none() {
                    s.diags.push(
                        Diag::error("E103", format!("この列は {want} ですが `{}` が書かれています", n.raw))
                            .at(at.to_string())
                            .mark(span.clone(), "")
                            .note(if n.unit.is_none() {
                                "単位が要ります。裸の数は書けません（§3）。".to_string()
                            } else {
                                format!("`{}` は {want} の単位ではありません。", n.raw)
                            }),
                    );
                }
            }
            Lit::Date(..) => {
                if *want != Ty::Date && *want != Ty::Unknown {
                    s.diags.push(
                        Diag::error("E103", format!("この列は {want} ですが 日付 が書かれています"))
                            .at(at.to_string())
                            .mark(span.clone(), ""),
                    );
                }
            }
            Lit::Word(w) => {
                if w == "真" || w == "偽" {
                    if *want != Ty::Bool && *want != Ty::Unknown {
                        s.diags.push(
                            Diag::error("E103", format!("この列は {want} ですが 真偽 が書かれています"))
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
                        Diag::error("E012", format!("`{w}` は値としても群としても見つかりません"))
                            .at(at.to_string())
                            .mark(span.clone(), ""),
                    ),
                    (Some(e), _) => s.diags.push(
                        Diag::error("E103", format!("この列は {want} ですが `{w}`（{e} の値）が書かれています"))
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
}

/// region が同じ換算を使うための入口。
pub fn lit_value_in_pub(n: &crate::lex::Num, want: &Ty) -> Option<Rat> {
    lit_value_in(n, want)
}

/// 大きい金額は 万・億 で書き戻す。書き手が `100万円` と書いたものに
/// `1000000円` と返すと、直す前に読み替えが要る。
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

fn fmt_val(v: Rat, ty: &Ty) -> String {
    match ty {
        Ty::Money { cur, .. } => format!("{}{cur}", fmt_big(v)),
        Ty::Qty { unit, .. } => format!("{v}{unit}"),
        Ty::Rate => format!("{}%", v.mul(Rat::int(100))),
        _ => format!("{v}"),
    }
}

/// `範囲 >=a <=b` を区間に落とす。開閉は閉じ側に寄せる（広く見積もるほうが安全側）。
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
    /// E112: 宣言範囲は、入力範囲から区間演算で得た到達区間を含まねばならない。
    /// 含まないと、網羅性検査が実際に起きる値を見ないまま「完全」と答える。
    fn derived_range(&mut self, d: &DerivedDecl, ty: &Ty, path: &str) {
        let Some((rl, rh)) = self.interval(&d.expr, ty) else { return };
        let Some(rg) = &d.range else { return };
        let (dl, dh) = bounds_of(rg, ty);
        let too_low = matches!((dl, Some(rl)), (Some(a), Some(b)) if b.cmp_to(a) == std::cmp::Ordering::Less);
        let too_high = matches!((dh, Some(rh)), (Some(a), Some(b)) if b.cmp_to(a) == std::cmp::Ordering::Greater);
        if too_low || too_high {
            self.diags.push(
                Diag::error("E112", "導出の範囲が、実際に到達しうる値を含んでいません")
                    .at(format!("{path}:{} 導出 {}", rg.span.line, d.name.text))
                    .mark(rg.span.clone(), format!("到達区間は >={} <={} です", fmt_val(rl, ty), fmt_val(rh, ty)))
                    .note("範囲が狭いと、網羅性検査が実際に起きる値を見ないまま「完全」と答えます。")
                    .note(format!(
                        "ヒント: 範囲 >={} <={} に広げてください。到達しない分まで広げても、実現不能な領域として検査が篩うので害はありません。",
                        fmt_val(rl, ty),
                        fmt_val(rh, ty)
                    )),
            );
        }
    }

    /// 入力の宣言範囲からの区間演算。導出の右辺は入力だけの線形結合なので、
    /// 加減と定数倍で閉じる（§5.2）。
    fn interval(&self, e: &Expr, ty: &Ty) -> Option<(Rat, Rat)> {
        match e {
            Expr::Name(n, _) => {
                if let Some((lo, hi)) = self.ranges.get(n) {
                    return Some(((*lo)?, (*hi)?));
                }
                // 率は 0〜100% を上限とみなす。範囲宣言を要求しないぶん保守的に見る。
                match self.syms.get(n).map(|s| s.ty.clone()) {
                    Some(Ty::Rate) => Some((Rat::zero(), Rat::int(1))),
                    _ => None,
                }
            }
            Expr::Lit(Lit::Num(n), _) => {
                let v = lit_value_in(n, ty)?;
                Some((v, v))
            }
            Expr::Bin(l, op, r, _) => {
                let (al, ah) = self.interval(l, ty)?;
                let (bl, bh) = self.interval(r, ty)?;
                match op {
                    BinOp::Add => Some((al.add(bl), ah.add(bh))),
                    BinOp::Sub => Some((al.sub(bh), ah.sub(bl))),
                    BinOp::Mul | BinOp::Div => {
                        // 端の組合せの最小と最大。率は非負とは限らないので四通り見る。
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

/// 宣言から刻みの逆数を取る。`率[刻み 0.1%]` なら 1000。
fn scale_of_type(tr: &TypeRef, ty: &Ty) -> i128 {
    for a in &tr.args {
        if let TypeArg::Scaled(w, n) = a {
            if w == "刻み" {
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
    /// 式のスケール。加減は最小公倍数、乗算は積、定数除算は割る数の分子倍。
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
                let digits: i128 = n.digits.parse().ok()?;
                let _ = digits;
                Some(match n.unit.as_deref() {
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
                    BinOp::Div => a * b,
                    _ => 1,
                })
            }
            Expr::Call(name, args, _) => {
                if crate::num::RoundMode::parse(name).is_some() {
                    // 丸めたあとは格子の刻みに戻る。
                    return args.get(1).and_then(|g| self.scale(g));
                }
                args.first().and_then(|a| self.scale(a))
            }
            _ => Some(1),
        }
    }

    /// E108: 中間値が int64 に収まることを、宣言範囲とスケールから証明する。
    /// 証明できなければ止める。黙って溢れるより、業務に「どこで丸めるか」を訊く。
    fn overflow(&mut self, e: &Expr, ty: &Ty, span: &Span, path: &str, name: &str) {
        let (Some((lo, hi)), Some(sc)) = (self.interval(e, ty), self.scale(e)) else { return };
        // 格納される整数は 値×刻みの逆数。区間の両端の絶対値の大きいほうで見る。
        let abs = |r: Rat| if r.num < 0 { Rat::zero().sub(r) } else { r };
        let mag = if abs(lo).cmp_to(abs(hi)) == std::cmp::Ordering::Greater { abs(lo) } else { abs(hi) };
        let stored_r = mag.mul(Rat::int(sc));
        let stored = stored_r.num / stored_r.den;
        if stored > i64::MAX as i128 {
            self.diags.push(
                Diag::error("E108", format!("{name} が int64 に収まることを証明できません"))
                    .at(format!("{path}:{}", span.line))
                    .mark(span.clone(), "")
                    .note(format!(
                        "到達区間は {} から {} で、刻みが 1/{sc} なので、格納される整数は最大 {stored} になります。",
                        fmt_val(lo, ty),
                        fmt_val(hi, ty)
                    ))
                    .note("ヒント: 入力の範囲を狭めるか、途中で丸めを一つ入れてください。")
                    .note("どこで丸めるかは円が動く業務の判断なので、道具が勝手に決めません（§7.1）。"),
            );
        }
    }
}

impl Checked {
    /// 真偽定義の原子を歩く。連言・選言・否定は許し、比較の片側が
    /// 入力か導出の名前、もう片側が定数であることを要求する。
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
                    // 第一形: 軸への単項テスト（名前 と リテラル）。
                    let ok = |x: &Expr, y: &Expr, s: &Checked| -> bool {
                        axis_of(x, s).is_some() && matches!(y, Expr::Lit(..))
                    };
                    // 第二形: 同型の軸どうしの比較。ただし数値どうしは E113 のまま。
                    // 導出にすれば厳密に解析できるものを、怠けて不透明にさせないため
                    // （健全性の話ではなく精度の規律。§5.3）。
                    let pair = match (axis_of(l, self), axis_of(r, self)) {
                        (Some(a), Some(b)) if a.unifies(&b) => Some(a),
                        _ => None,
                    };
                    if let Some(t) = &pair {
                        if !t.is_numeric() {
                            return;
                        }
                        self.diags.push(
                            Diag::error("E113", format!("定義 {owner} が {t} どうしを直接比べています"))
                                .at(format!("{path}:{}", sp.line))
                                .mark(sp.clone(), "")
                                .note("数値どうしの比較は、差を `導出` として宣言してから定数と比べてください。そのほうが厳密に解析できます。")
                                .note("導出にできない型（日付、列挙）どうしなら、そのまま比べられます（§5.3）。"),
                        );
                        return;
                    }
                    if !ok(l, r, self) && !ok(r, l, self) {
                        let hint = if matches!(**l, Expr::Name(..)) && matches!(**r, Expr::Name(..)) {
                            "入力の線形結合なら `導出` として宣言してから比べてください。表の出力と比べたいなら、その表に真偽の出力列を足すのが正しい書き方です。"
                        } else {
                            "比較の片側は入力か導出の名前、もう片側は定数である必要があります。"
                        };
                        self.diags.push(
                            Diag::error("E113", format!("定義 {owner} の条件が、入力か導出への単項テストになっていません"))
                                .at(format!("{path}:{}", sp.line))
                                .mark(sp.clone(), "")
                                .note(hint.to_string())
                                .note("この制限が、完全性と重複の検査が有限で終わることの土台です（§5.3、§6.2）。"),
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
