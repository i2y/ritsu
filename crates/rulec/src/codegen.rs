//! Code generation (§8).
//!
//! Readability of the generated code is defined by the five criteria of §8.1. In particular,
//! "one table row is one branch" and "conditions already known to be true from earlier branches
//! are not dropped" are the promises that let a reader match `.rule` rows to generated lines by
//! eye, so no clever optimization is attempted.

use crate::ast::*;
use crate::num::{Rat, RoundMode};
use crate::types::{Checked, Ty};
use std::collections::BTreeMap;

/// Build the ASCII brand name from a unit. Units are a closed set, so a fixed table suffices.
fn brand_of(ty: &Ty) -> String {
    match ty {
        Ty::Money { cur, tax } => {
            let c = match cur.as_str() {
                "円" => "Yen",
                "銭" => "Sen",
                other => other,
            };
            let t = match tax.as_deref() {
                Some(crate::kw::INCL_TAX) => "InclTax",
                Some(crate::kw::EXCL_TAX) => "ExclTax",
                _ => "",
            };
            format!("{c}{t}")
        }
        Ty::Qty { unit, .. } => match unit.as_str() {
            "g" => "Gram".into(),
            "kg" => "Kilogram".into(),
            "cm" => "Cm".into(),
            "m" => "Meter".into(),
            other => other.into(),
        },
        Ty::Rate => "Rate".into(),
        Ty::Date => "Date".into(),
        Ty::Bool => "bool".into(),
        Ty::Str => "str".into(),
        Ty::Enum(n) => n.clone(),
        Ty::Opt(t) => format!("Optional{}", brand_of(t)),
        Ty::Unknown => "int".into(),
    }
}

/// `member_kind` → `MemberKind`.
fn pascal(s: &str) -> String {
    s.split('_')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut ch = p.chars();
            match ch.next() {
                Some(c) => c.to_uppercase().collect::<String>() + ch.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

/// The public surface uses the ASCII alias; internals keep the Japanese name (§8.1).
fn pub_name(n: &Name) -> String {
    n.ascii.clone().unwrap_or_else(|| n.text.clone())
}

pub struct Gen<'a> {
    f: &'a RuleFile,
    c: &'a Checked,
    /// Table name → the row pairs reported by W114. Guards are emitted only for these.
    w114: BTreeMap<String, Vec<(usize, usize)>>,
    /// Table output column → storage scale. Rate columns declare no step, so the scale is the
    /// least common multiple of the denominators of that column's literals. Unless it is fixed
    /// once per column, 50% and 100% in the same column would come out at different scales and
    /// the values would be corrupted.
    col_scales: BTreeMap<String, i128>,
    /// Type name → the ASCII alias of that enum (PascalCase).
    enum_names: BTreeMap<String, String>,
    /// Enum value → (type name, ASCII alias).
    value_names: BTreeMap<String, (String, String)>,
    src_hash: String,
}

impl<'a> Gen<'a> {
    pub fn new(f: &'a RuleFile, c: &'a Checked, src: &str) -> Self {
        let mut enum_names = BTreeMap::new();
        let mut value_names = BTreeMap::new();
        for e in &f.enums {
            let ty = pascal(&pub_name(&e.name));
            enum_names.insert(e.name.text.clone(), ty.clone());
            for v in &e.values {
                value_names.insert(v.text.clone(), (ty.clone(), pascal(&pub_name(v))));
            }
        }
        if f.imports.iter().any(|(p, _)| p.ends_with("都道府県")) {
            enum_names.insert("都道府県".into(), "Prefecture".into());
            for (j, r) in crate::prelude::PREFECTURES {
                value_names.insert((*j).into(), ("Prefecture".into(), (*r).into()));
            }
        }
        let mut col_scales: BTreeMap<String, i128> = BTreeMap::new();
        for it in &f.items {
            let Item::Table(t) = it else { continue };
            for (oi, oc) in t.outputs.iter().enumerate() {
                let ty = c.ty_of(&oc.name.text).unwrap_or(Ty::Unknown);
                if !matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) {
                    continue;
                }
                let mut sc: i128 = 1;
                for row in &t.rows {
                    match row.outs.get(oi) {
                        Some(OutCell::Lit(Lit::Num(n))) => {
                            if let Some(v) = crate::types::lit_value_in_pub(n, &ty) {
                                sc = lcm(sc, v.den);
                            }
                        }
                        // A cell that names a value writes that value's scale into the
                        // column. Sizing the column only by its literals is how a rate-valued
                        // definition ended up assigned into a column a hundred times coarser
                        // than itself, and silently multiplied.
                        Some(OutCell::Name(w)) => sc = lcm(sc, *c.scales.get(w).unwrap_or(&1)),
                        _ => {}
                    }
                }
                col_scales.insert(oc.name.text.clone(), sc);
            }
        }
        let mut w114: BTreeMap<String, Vec<(usize, usize)>> = BTreeMap::new();
        for it in &f.items {
            if let Item::Table(t) = it {
                let r = crate::region::check_table(t, c, f, "", crate::region::DEFAULT_BUDGET);
                if !r.w114.is_empty() {
                    w114.insert(t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default(), r.w114);
                }
            }
        }
        Gen { f, c, w114, col_scales, enum_names, value_names, src_hash: hash(src) }
    }

    fn ty_of(&self, n: &str) -> Ty {
        self.c.ty_of(n).unwrap_or(Ty::Unknown)
    }

    /// Storage scale of a value (values are held as multiples of 1/k; this is that k). §7.1.
    fn scale(&self, n: &str) -> i128 {
        if let Some(s) = self.col_scales.get(n) {
            return *s;
        }
        *self.c.scales.get(n).unwrap_or(&1)
    }

    /// A value written into an output column, brought to that column's scale.
    ///
    /// A name carries the scale of whatever it names — a definition over a rate input is
    /// held in hundredths, say — while the column has a scale of its own, fixed by the
    /// literals that appear in it. Assigning one into the other without this is how a rate
    /// landed in a column a hundred times its size.
    fn rescaled(&self, from: &str, to: &str, text: String) -> String {
        let (a, b) = (self.scale(from), self.scale(to));
        // The column is sized to hold whatever is written into it (see `Gen::new`), so this
        // only ever widens. Anything else would silently drop precision the evaluator keeps.
        if a == b || a == 0 || b % a != 0 {
            text
        } else {
            // No parentheses: this lands on the right of an assignment, and both formatters
            // strip a redundant pair there.
            format!("{text} * {}", b / a)
        }
    }

    /// An integer literal adjusted to the type and the column's scale.
    fn int_lit(&self, n: &crate::lex::Num, ty: &Ty, scale: i128) -> String {
        let v = crate::types::lit_value_in_pub(n, ty).unwrap_or(Rat::zero());
        format!("{}", v.mul(Rat::int(scale)).num)
    }

    fn header(&self, comment: &str) -> String {
        tr!(
            "{comment} Code generated by rulec {}. DO NOT EDIT.\n\
             {comment} 原本: {} (規則 {} v{}, sha256:{})\n",
            "{comment} Code generated by rulec {}. DO NOT EDIT.\n\
             {comment} Source: {} (rule {} v{}, sha256:{})\n",
            env!("CARGO_PKG_VERSION"),
            self.f.name.text,
            self.f.name.text,
            self.f.version,
            &self.src_hash[..12]
        )
    }
}

/// A deterministic short hash of our own (FNV-1a 128), so as not to add a dependency.
pub fn hash(s: &str) -> String {
    let mut h: u128 = 0x6c62272e07bb014262b821756295c58d;
    for b in s.as_bytes() {
        h ^= *b as u128;
        h = h.wrapping_mul(0x0000000001000000000000000000013b);
    }
    format!("{h:032x}")
}

// ---------------------------------------------------------------------------
// Expressions
// ---------------------------------------------------------------------------

/// An expression rendered as text together with its scale. Both languages get the same integer
/// arithmetic.
struct Expr2 {
    text: String,
    scale: i128,
}

impl<'a> Gen<'a> {
    fn expr(&self, e: &Expr, local: &dyn Fn(&str) -> String) -> Expr2 {
        match e {
            Expr::Name(n, _) => Expr2 { text: local(n), scale: self.scale(n) },
            Expr::Lit(Lit::Num(n), _) => {
                let ty = crate::types::lit_ty_pub(n);
                let v = crate::types::lit_value_in_pub(n, &ty).unwrap_or(Rat::zero());
                let s = if v.den == 1 { 1 } else { v.den };
                Expr2 { text: format!("{}", v.num * (s / v.den)), scale: s }
            }
            Expr::Lit(Lit::Word(w), _) if w == crate::kw::TRUE || w == crate::kw::FALSE => {
                Expr2 { text: (if w == crate::kw::TRUE { "True" } else { "False" }).into(), scale: 1 }
            }
            Expr::Lit(..) => Expr2 { text: "0".into(), scale: 1 },
            Expr::Call(name, args, _) => {
                let a: Vec<Expr2> = args.iter().map(|x| self.expr(x, local)).collect();
                match (name.as_str(), a.as_slice()) {
                    (crate::kw::MIN, [x, y]) | (crate::kw::MAX, [x, y]) => {
                        let s = lcm(x.scale, y.scale);
                        let f = if name == crate::kw::MIN { "_min" } else { "_max" };
                        Expr2 {
                            text: format!("{f}({}, {})", rescale(x, s), rescale(y, s)),
                            scale: s,
                        }
                    }
                    (m, [x, g]) if RoundMode::parse(m).is_some() => {
                        // Rounding snaps to a multiple of the grid, which is brought to x's scale.
                        let mode = RoundMode::parse(m).unwrap();
                        let gs = rescale(g, x.scale);
                        Expr2 {
                            text: format!("_round_{}({}, {})", mode_fn(mode), x.text, gs),
                            scale: x.scale,
                        }
                    }
                    _ => Expr2 { text: "0".into(), scale: 1 },
                }
            }
            Expr::Bin(l, op, r, _) => {
                let a = self.expr(l, local);
                let b = self.expr(r, local);
                use BinOp::*;
                match op {
                    Add | Sub => {
                        let s = lcm(a.scale, b.scale);
                        let o = if *op == Add { "+" } else { "-" };
                        Expr2 { text: format!("({} {o} {})", rescale(&a, s), rescale(&b, s)), scale: s }
                    }
                    Mul => Expr2 {
                        text: format!("({} * {})", a.text, b.text),
                        scale: a.scale * b.scale,
                    },
                    Div => Expr2 { text: format!("({} // {})", a.text, b.text), scale: a.scale },
                    Le | Lt | Ge | Gt | Eq => {
                        let s = lcm(a.scale, b.scale);
                        let o = match op {
                            Le => "<=",
                            Lt => "<",
                            Ge => ">=",
                            Gt => ">",
                            _ => "==",
                        };
                        Expr2 {
                            text: format!("({} {o} {})", rescale(&a, s), rescale(&b, s)),
                            scale: 1,
                        }
                    }
                }
            }
        }
    }
}

fn mode_fn(m: RoundMode) -> &'static str {
    match m {
        RoundMode::Up => "up",
        RoundMode::Down => "down",
        RoundMode::Half => "half",
        RoundMode::Bankers => "bankers",
    }
}

fn rescale(e: &Expr2, to: i128) -> String {
    if e.scale == to {
        e.text.clone()
    } else {
        format!("({} * {})", e.text, to / e.scale)
    }
}

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


// ---------------------------------------------------------------------------
// Python
// ---------------------------------------------------------------------------

impl<'a> Gen<'a> {
    fn py_value(&self, v: &str) -> String {
        match self.value_names.get(v) {
            Some((ty, alias)) => format!("{ty}.{}", alias.to_uppercase()),
            None => format!("{v:?}"),
        }
    }

    /// Render a cell as a Python condition. A don't-care yields None (no condition).
    fn py_cell(&self, cell: &Cell, var: &str, ty: &Ty, col_scale: i128) -> Option<String> {
        let inner = match ty {
            Ty::Opt(t) => t.as_ref(),
            other => other,
        };
        let lit = |l: &Lit| -> String {
            match l {
                Lit::Word(w) if w == crate::kw::TRUE => "True".into(),
                Lit::Word(w) if w == crate::kw::FALSE => "False".into(),
                Lit::Word(w) => self.py_value(w),
                Lit::Num(n) => self.int_lit(n, inner, col_scale),
                Lit::Date(y, m, d) => format!("{}", crate::types::date_ord(*y, *m, *d).num),
                Lit::Str(s) => format!("{s:?}"),
            }
        };
        let members = |ls: &Vec<Lit>| -> String {
            let mut out: Vec<String> = Vec::new();
            for l in ls {
                if let Lit::Word(w) = l {
                    if let Some((_, ms)) = self.c.groups.get(w) {
                        out.extend(ms.iter().map(|m| self.py_value(m)));
                        continue;
                    }
                }
                out.push(lit(l));
            }
            format!("{{{}}}", out.join(", "))
        };
        Some(match cell {
            Cell::DontCare => return None,
            Cell::Nothing => format!("{var} is None"),
            Cell::Lit(Lit::Word(w)) if self.c.groups.contains_key(w) => {
                format!("{var} in {}", members(&vec![Lit::Word(w.clone())]))
            }
            Cell::Lit(Lit::Word(w)) if w == crate::kw::TRUE => var.to_string(),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::FALSE => format!("not {var}"),
            Cell::Lit(l) => format!("{var} == {}", lit(l)),
            Cell::Set(ls) => format!("{var} in {}", members(ls)),
            Cell::Not(ls) => format!("{var} not in {}", members(ls)),
            Cell::Cmp(cs) => cs
                .iter()
                .map(|(o, l)| {
                    let op = match o {
                        CmpOp::Le => "<=",
                        CmpOp::Ge => ">=",
                        CmpOp::Lt => "<",
                        CmpOp::Gt => ">",
                    };
                    format!("{var} {op} {}", lit(l))
                })
                .collect::<Vec<_>>()
                .join(" and "),
        })
    }

    pub fn python(&self) -> String {
        let mut o = self.header("#");
        // Import from typing only what is used. NamedTuple is needed only with multiple outputs.
        let typing = if self.f.outputs.len() > 1 { "NamedTuple, NewType" } else { "NewType" };
        o.push_str(&format!("from __future__ import annotations\n\nimport enum\nfrom typing import {typing}\n\n"));

        // Brands. They work with mypy and pyright and cost nothing at runtime.
        let mut brands: BTreeMap<String, String> = BTreeMap::new();
        for v in self.f.inputs.iter().map(|i| &i.name.text).chain(self.f.outputs.iter().map(|o| &o.name.text)) {
            let ty = self.ty_of(v);
            if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) {
                brands.insert(brand_of(&ty), format!("{ty}"));
            }
        }
        for (b, doc) in &brands {
            o.push_str(&format!("{b} = NewType(\"{b}\", int)  # {doc}\n"));
        }
        if !brands.is_empty() {
            o.push('\n');
        }

        // Enums. Each value carries its Japanese name, used in logs and in the wire format (§10).
        let mut emitted: Vec<String> = Vec::new();
        for (jp, ascii) in &self.enum_names {
            if emitted.contains(ascii) {
                continue;
            }
            emitted.push(ascii.clone());
            let Some(vals) = self.c.enums.get(jp) else { continue };
            o.push_str(&format!("class {ascii}(enum.Enum):\n"));
            for v in vals {
                let name = self.value_names.get(v).map(|(_, a)| a.to_uppercase()).unwrap_or_else(|| v.clone());
                o.push_str(&format!("    {name} = \"{v}\"\n"));
            }
            o.push('\n');
        }

        o.push_str(&format!(
            "class RuleInputError(ValueError):\n    \"\"\"{}\"\"\"\n\n",
            tr!("宣言された入力域の外。呼び出し側の契約違反。", "Outside the declared input domain: a contract violation by the caller.")
        ));
        o.push_str(&format!(
            "class RuleContradictionError(AssertionError):\n    \"\"\"{}\"\"\"\n\n",
            tr!("規則そのものの矛盾。呼び出し側の誤りではない。", "A contradiction in the rule itself, not a mistake by the caller.")
        ));

        // Groups
        for g in &self.f.groups {
            let ms: Vec<String> = g.members.iter().map(|m| self.py_value(&m.text)).collect();
            o.push_str(&format!("_{} = frozenset({{{}}})\n", g.name.text, ms.join(", ")));
        }
        if !self.f.groups.is_empty() {
            o.push('\n');
        }

        o.push_str(&round_py());
        o.push_str(&self.py_fn());
        pep8_blanks(&o)
    }
}

/// The four modes of §7.3. Negative values and exact halves are pinned to the spec as well;
/// nothing is left to the language's native division (Python's `//` goes toward −∞, Go's
/// toward 0).
fn round_py() -> String {
    format!(
        r#"
{note}
_isinstance = isinstance


def _min(a: int, b: int) -> int:
    return a if a < b else b


def _max(a: int, b: int) -> int:
    return a if a > b else b


def _round_down(x: int, g: int) -> int:
    """{down}"""
    q, r = abs(x) // g, abs(x) % g
    v = q * g
    return -v if x < 0 else v


def _round_up(x: int, g: int) -> int:
    """{up}"""
    q, r = abs(x) // g, abs(x) % g
    v = (q + 1) * g if r else q * g
    return -v if x < 0 else v


def _round_half(x: int, g: int) -> int:
    """{half}"""
    q, r = abs(x) // g, abs(x) % g
    v = (q + 1) * g if 2 * r >= g else q * g
    return -v if x < 0 else v


def _round_bankers(x: int, g: int) -> int:
    """{bankers}"""
    q, r = abs(x) // g, abs(x) % g
    if 2 * r > g or (2 * r == g and q % 2 == 1):
        q += 1
    v = q * g
    return -v if x < 0 else v

"#,
        note = tr!(
            "# 生成コードは組み込みを裸で呼ばない。入力の ASCII 別名が `min` や `list` の\n\
             # ような名前でも壊れないようにするため（衝突の族ごと消す）。",
            "# Generated code never calls a builtin bare, so that an input whose ASCII alias is a\n\
             # name like `min` or `list` does not break it (the whole family of collisions is gone)."
        ),
        down = tr!("0 へ寄せる。-4.8円 → -4円。", "Toward zero: -4.8 yen → -4 yen."),
        up = tr!("0 から遠ざける。-4.2円 → -5円。", "Away from zero: -4.2 yen → -5 yen."),
        half = tr!("半分ちょうどは 0 から遠ざける。", "An exact half goes away from zero."),
        bankers = tr!("半分ちょうどは偶数へ。", "An exact half goes to the even neighbor."),
    )
}

impl<'a> Gen<'a> {
    /// The Python annotation for a type. The public surface uses brands; enums are classes.
    fn py_ty(&self, ty: &Ty) -> String {
        match ty {
            Ty::Enum(n) => self.enum_names.get(n).cloned().unwrap_or_else(|| "str".into()),
            Ty::Bool => "bool".into(),
            Ty::Date => "int".into(),
            Ty::Str => "str".into(),
            Ty::Opt(t) => format!("{} | None", self.py_ty(t)),
            _ => brand_of(ty),
        }
    }

    /// Storage scale of an output: a single integer in the declared unit (§7.1).
    fn out_scale(&self, name: &str) -> i128 {
        match self.ty_of(name) {
            Ty::Rate => *self.c.scales.get(name).unwrap_or(&100),
            _ => 1,
        }
    }

    fn py_fn(&self) -> String {
        let fname = pub_name(&self.f.name);
        let params: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("{}: {}", pub_name(&i.name), self.py_ty(&self.ty_of(&i.name.text))))
            .collect();
        let outs = &self.f.outputs;
        let ret = if outs.len() == 1 {
            self.py_ty(&self.ty_of(&outs[0].name.text))
        } else {
            "Output".into()
        };
        let mut o = String::new();

        // Multiple outputs are a NamedTuple (§8.5).
        if outs.len() > 1 {
            o.push_str("class Output(NamedTuple):\n");
            for od in outs {
                o.push_str(&format!("    {}: {}\n", pub_name(&od.name), self.py_ty(&self.ty_of(&od.name.text))));
            }
            o.push('\n');
        }

        o.push_str(&format!("def {fname}({}) -> {ret}:\n", params.join(", ")));
        o.push_str(&tr!(
            "    \"\"\"規則 {} v{}。分岐は原本の行と 1:1 に対応する。\"\"\"\n",
            "    \"\"\"Rule {} v{}. Each branch corresponds 1:1 to a row of the rule source.\"\"\"\n",
            self.f.name.text, self.f.version
        ));

        // Entry guards (§8.5). They enforce at runtime what the proof assumes: inputs lie within
        // their declared domains.
        let local = |n: &str| -> String {
            match self.f.inputs.iter().find(|i| i.name.text == n) {
                Some(i) => pub_name(&i.name),
                None => n.to_string(),
            }
        };
        for i in &self.f.inputs {
            let v = pub_name(&i.name);
            let ty = self.ty_of(&i.name.text);
            match &ty {
                Ty::Enum(_) => o.push_str(&format!(
                    "    if not _isinstance({v}, {}):\n        raise RuleInputError(f\"{}\")\n",
                    self.py_ty(&ty),
                    tr!(
                        "{} が列挙 {} の値ではありません: {{{v}!r}}",
                        "{} is not a value of enum {}: {{{v}!r}}",
                        i.name.text,
                        self.py_ty(&ty)
                    )
                )),
                // A date is an ordinal, and its declared range is the universe the
                // completeness proof used, so it is guarded like any other number. Without
                // this, a date outside the declared range falls into whichever branch happens
                // to catch it and the caller gets a silently wrong answer.
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Date => {
                    if let Some((lo, hi)) = self.c.ranges.get(&i.name.text) {
                        if let (Some(lo), Some(hi)) = (lo, hi) {
                            // The bound has to be in the units the argument arrives in, which
                            // for a rate is the number of steps. Taking the numerator alone
                            // turned `range >=0% <=100%` into `0 <= r <= 1` and refused every
                            // value above one step.
                            let sc = self.c.wire_scale(&i.name.text);
                            o.push_str(&format!(
                                "    if not {} <= {v} <= {}:\n        raise RuleInputError(f\"{}\")\n",
                                crate::types::wire_int(*lo, sc),
                                crate::types::wire_int(*hi, sc),
                                tr!("{} が範囲の外です: {{{v}}}", "{} is out of range: {{{v}}}", i.name.text)
                            ));
                        }
                    }
                }
                _ => {}
            }
        }

        // Derived values and definitions, in declaration order (define-before-use, §5.1).
        for it in &self.f.items {
            match it {
                Item::Derived(d) => {
                    let e = self.expr(&d.expr, &local);
                    o.push_str(&format!("    {} = {}  # {}\n", d.name.text, unparen(&e.text), tr!("導出", "derived value")));
                }
                Item::Define(d) => {
                    let e = self.expr(&d.expr, &local);
                    o.push_str(&format!("    {} = {}  # {}\n", d.name.text, unparen(&e.text), tr!("定義", "definition")));
                }
                Item::Table(t) => o.push_str(&self.py_table(t, &local)),
            }
        }

        // The result and the final rounding.
        let out_name = &outs[0].name.text;
        let res = match &self.f.result {
            Some(r) => self.expr(&r.expr, &local),
            None => Expr2 { text: local(out_name), scale: self.scale(out_name) },
        };
        let os = self.out_scale(out_name);
        let text = match &outs[0].rounding {
            Some(rd) => {
                let ty = self.ty_of(out_name);
                let g = crate::types::lit_value_in_pub(&rd.grid, &ty).unwrap_or(Rat::int(1));
                let m = RoundMode::parse(&rd.mode).unwrap_or(RoundMode::Down);
                let grid_i = g.num * res.scale / g.den;
                // Bind the expression to an intermediate instead of nesting it (the same shape as
                // the example in §8.2). Criterion 1 demands readability, and deep nesting breaks
                // the visual correspondence with the generated code.
                if res.scale != os {
                    o.push_str(&format!(
                        "    raw = {}  # {}\n",
                        unparen(&res.text),
                        tr!("単位: 1/{} {}", "unit: 1/{} {}", res.scale, ty)
                    ));
                    format!("_round_{}(raw, {}) // {}", mode_fn(m), grid_i, res.scale / os)
                } else {
                    format!("_round_{}({}, {})", mode_fn(m), res.text, grid_i)
                }
            }
            None => res.text.clone(),
        };
        if outs.len() == 1 {
            o.push_str(&format!("    return {text}\n"));
        } else {
            // Each output gets its own rounding, applied exactly once (§7.2).
            let fields: Vec<String> = outs
                .iter()
                .map(|od| {
                    let n = &od.name.text;
                    let Some(rd) = &od.rounding else { return local(n) };
                    let ty = self.ty_of(n);
                    let g = crate::types::lit_value_in_pub(&rd.grid, &ty).unwrap_or(Rat::int(1));
                    let m = RoundMode::parse(&rd.mode).unwrap_or(RoundMode::Down);
                    format!("_round_{}({}, {})", mode_fn(m), local(n), g.num * self.scale(n) / g.den)
                })
                .collect();
            o.push_str(&format!("    return Output({})\n", fields.join(", ")));
        }
        o
    }

    fn py_table(&self, t: &Table, local: &dyn Fn(&str) -> String) -> String {
        let name = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
        let policy = if t.policy == Policy::Unique { crate::kw::UNIQUE } else { crate::kw::FIRST };
        let mut o = format!("    # {}\n", tr!("表 {name}（{} {policy}）", "table {name} ({} {policy})", crate::kw::POLICY));
        for (ri, row) in t.rows.iter().enumerate() {
            // §8.1 criterion 1: write out every cell. Conditions already known to be true from
            // earlier branches are not dropped either.
            let conds: Vec<String> = t
                .inputs
                .iter()
                .enumerate()
                .filter_map(|(ci, (col, _))| {
                    let ty = self.ty_of(col);
                    self.py_cell(row.cells.get(ci)?, &local(col), &ty, self.scale(col))
                })
                .collect();
            let cond = if conds.is_empty() { "True".into() } else { conds.join(" and ") };
            let kw = if ri == 0 { "if" } else { "elif" };
            let cells: Vec<String> = row
                .cells
                .iter()
                .map(cell_src)
                .chain(row.outs.iter().map(out_src))
                .collect();
            o.push_str(&format!("    {kw} {cond}:  # {}\n", tr!("行{}: {}", "row {}: {}", ri + 1, cells.join(" | "))));
            for (oi, oc) in t.outputs.iter().enumerate() {
                let v = match row.outs.get(oi) {
                    Some(OutCell::Lit(Lit::Num(n))) => {
                        let ty = self.ty_of(&oc.name.text);
                        self.int_lit(n, &ty, self.scale(&oc.name.text))
                    }
                    Some(OutCell::Lit(l)) => match l {
                        Lit::Word(w) if w == crate::kw::TRUE => "True".into(),
                        Lit::Word(w) if w == crate::kw::FALSE => "False".into(),
                        Lit::Word(w) => self.py_value(w),
                        Lit::Date(y, m, d) => format!("{}", crate::types::date_ord(*y, *m, *d).num),
                        _ => "0".into(),
                    },
                    Some(OutCell::Name(w)) => {
                        if w == crate::kw::TRUE {
                            "True".into()
                        } else if w == crate::kw::FALSE {
                            "False".into()
                        } else if self.value_names.contains_key(w) {
                            self.py_value(w)
                        } else {
                            self.rescaled(w, &oc.name.text, local(w))
                        }
                    }
                    None => "0".into(),
                };
                o.push_str(&format!("        {} = {v}\n", oc.name.text));
            }
        }
        o.push_str(&format!(
            "    else:\n        raise AssertionError(\"{}\")\n",
            tr!("到達不能: 完全性は rulec が静的に検査済み", "unreachable: completeness was statically checked by rulec")
        ));
        o.push_str(&self.guards(t, local, "    ", |name, i, j| {
            format!(
                "        raise RuleContradictionError(\"{}\")\n",
                tr!("表 {name}: 行{i} と 行{j} が同時に当たりました", "table {name}: row {i} and row {j} matched at the same time")
            )
        }));
        o
    }

    /// Guards (§8.1). Emitted only for row pairs whose exclusivity could not be proven statically.
    /// No vector exercises this branch: had such an input been constructible, it would have been
    /// an E105.
    fn guards(
        &self,
        t: &Table,
        local: &dyn Fn(&str) -> String,
        indent: &str,
        raise: impl Fn(&str, usize, usize) -> String,
    ) -> String {
        let Some(pairs) = self.w114.get(&t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default()) else {
            return String::new();
        };
        let name = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
        let guard = tr!("番人", "guard");
        let mut o = String::new();
        for (i, j) in pairs {
            let go = indent.starts_with('\t');
            let mut conds: Vec<String> = Vec::new();
            for (ci, (col, _)) in t.inputs.iter().enumerate() {
                let ty = self.ty_of(col);
                let sc = self.scale(col);
                for r in [*i, *j] {
                    let Some(cell) = t.rows[r].cells.get(ci) else { continue };
                    let c = if go {
                        self.go_cell(cell, &local(col), &ty, sc)
                    } else {
                        self.py_cell(cell, &local(col), &ty, sc)
                    };
                    if let Some(c) = c {
                        if !conds.contains(&c) {
                            conds.push(c);
                        }
                    }
                }
            }
            if conds.is_empty() {
                continue;
            }
            let joined = conds.join(if go { " && " } else { " and " });
            o.push_str(&format!(
                "{indent}// {guard}: {}\n",
                tr!(
                    "W114（表 {name} 行{} × 行{}）。排他を静的に証明できなかった行対",
                    "W114 (table {name}, row {} × row {}): a pair of rows whose exclusivity could not be proven statically",
                    i + 1,
                    j + 1
                )
            ));
            if go {
                o.push_str(&format!("{indent}if {joined} {{\n"));
            } else {
                o.push_str(&format!("{indent}if {joined}:\n"));
            }
            o.push_str(&raise(&name, i + 1, j + 1));
            if go {
                o.push_str(&format!("{indent}}}\n"));
            }
        }
        if !go_comment_ok(indent) {
            o = o.replace(&format!("// {guard}"), &format!("# {guard}"));
        }
        o
    }
}

/// The source spelling of a cell, used in diagnostics and in generated comments.
fn cell_src(c: &Cell) -> String {
    match c {
        Cell::DontCare => "-".into(),
        Cell::Nothing => crate::kw::NONE.into(),
        Cell::Lit(l) => lit_src(l),
        Cell::Set(ls) => ls.iter().map(lit_src).collect::<Vec<_>>().join(", "),
        Cell::Not(ls) => format!("{}: {}", crate::kw::NOT, ls.iter().map(lit_src).collect::<Vec<_>>().join(", ")),
        Cell::Cmp(cs) => cs
            .iter()
            .map(|(o, l)| {
                let op = match o {
                    CmpOp::Le => "<=",
                    CmpOp::Ge => ">=",
                    CmpOp::Lt => "<",
                    CmpOp::Gt => ">",
                };
                format!("{op}{}", lit_src(l))
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

fn out_src(o: &OutCell) -> String {
    match o {
        OutCell::Lit(l) => lit_src(l),
        OutCell::Name(n) => n.clone(),
    }
}

fn lit_src(l: &Lit) -> String {
    match l {
        Lit::Num(n) => n.raw.clone(),
        Lit::Word(w) => w.clone(),
        Lit::Str(s) => format!("\"{s}\""),
        Lit::Date(y, m, d) => format!("{y:04}-{m:02}-{d:02}"),
    }
}

// ---------------------------------------------------------------------------
// Go
// ---------------------------------------------------------------------------

impl<'a> Gen<'a> {
    fn go_value(&self, v: &str) -> String {
        match self.value_names.get(v) {
            Some((ty, alias)) => format!("{ty}{alias}"),
            None => format!("{v:?}"),
        }
    }

    /// Go's zero value for a return type, for the early return of a guard. `return 0` does
    /// not compile when the rule answers with a boolean or a string.
    fn go_zero(&self, ty: &Ty) -> String {
        match ty {
            Ty::Bool => "false".into(),
            Ty::Str => "\"\"".into(),
            Ty::Opt(_) => "nil".into(),
            _ => "0".into(),
        }
    }

    /// Whether anything downstream reads a table's output column. A column nothing reads is
    /// still assigned, so that the branch and the row stay 1:1 (§8.2), but Go refuses to
    /// compile a local that is never read. W111 reports the column itself.
    fn is_read(&self, name: &str) -> bool {
        self.c.used.contains(name) || self.f.outputs.iter().any(|o| o.name.text == name)
    }

    fn go_ty(&self, ty: &Ty) -> String {
        match ty {
            Ty::Enum(n) => self.enum_names.get(n).cloned().unwrap_or_else(|| "string".into()),
            Ty::Bool => "bool".into(),
            Ty::Date => "int64".into(),
            Ty::Str => "string".into(),
            Ty::Opt(t) => format!("*{}", self.go_ty(t)),
            _ => brand_of(ty),
        }
    }

    fn go_cell(&self, cell: &Cell, var: &str, ty: &Ty, col_scale: i128) -> Option<String> {
        let inner = match ty {
            Ty::Opt(t) => t.as_ref(),
            other => other,
        };
        let lit = |l: &Lit| -> String {
            match l {
                Lit::Word(w) if w == crate::kw::TRUE => "true".into(),
                Lit::Word(w) if w == crate::kw::FALSE => "false".into(),
                Lit::Word(w) => self.go_value(w),
                Lit::Num(n) => self.int_lit(n, inner, col_scale),
                Lit::Date(y, m, d) => format!("{}", crate::types::date_ord(*y, *m, *d).num),
                Lit::Str(s) => format!("{s:?}"),
            }
        };
        let set = |ls: &Vec<Lit>, neg: bool| -> String {
            let mut out: Vec<String> = Vec::new();
            for l in ls {
                if let Lit::Word(w) = l {
                    if let Some((_, ms)) = self.c.groups.get(w) {
                        out.extend(ms.iter().map(|m| self.go_value(m)));
                        continue;
                    }
                }
                out.push(lit(l));
            }
            let op = if neg { "!=" } else { "==" };
            let join = if neg { " && " } else { " || " };
            format!(
                "({})",
                out.iter().map(|v| format!("{var} {op} {v}")).collect::<Vec<_>>().join(join)
            )
        };
        Some(match cell {
            Cell::DontCare => return None,
            Cell::Nothing => format!("{var} == nil"),
            Cell::Lit(Lit::Word(w)) if self.c.groups.contains_key(w) => {
                set(&vec![Lit::Word(w.clone())], false)
            }
            Cell::Lit(Lit::Word(w)) if w == crate::kw::TRUE => var.to_string(),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::FALSE => format!("!{var}"),
            Cell::Lit(l) => format!("{var} == {}", lit(l)),
            Cell::Set(ls) => set(ls, false),
            Cell::Not(ls) => set(ls, true),
            Cell::Cmp(cs) => cs
                .iter()
                .map(|(o, l)| {
                    let op = match o {
                        CmpOp::Le => "<=",
                        CmpOp::Ge => ">=",
                        CmpOp::Lt => "<",
                        CmpOp::Gt => ">",
                    };
                    format!("{var} {op} {}", lit(l))
                })
                .collect::<Vec<_>>()
                .join(" && "),
        })
    }

    pub fn go(&self) -> String {
        let alias = pub_name(&self.f.name);
        let pkg = alias.replace('_', "").to_lowercase();
        let mut o = self.header("//");
        o.push_str(&format!("\npackage {pkg}\n\n{IMPORT_MARK}"));

        let mut brands: BTreeMap<String, String> = BTreeMap::new();
        for v in self.f.inputs.iter().map(|i| &i.name.text).chain(self.f.outputs.iter().map(|o| &o.name.text)) {
            let ty = self.ty_of(v);
            if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) {
                brands.insert(brand_of(&ty), format!("{ty}"));
            }
        }
        for (b, doc) in &brands {
            // A defined type (not an alias): the compiler rejects YenInclTax + YenExclTax.
            o.push_str(&format!("type {b} int64{CELL}// {doc}\n"));
        }
        if !brands.is_empty() {
            o.push('\n');
        }

        let mut emitted: Vec<String> = Vec::new();
        for (jp, ascii) in &self.enum_names {
            if emitted.contains(ascii) {
                continue;
            }
            emitted.push(ascii.clone());
            let Some(vals) = self.c.enums.get(jp) else { continue };
            o.push_str(&format!("type {ascii} int\n\nconst (\n"));
            for (i, v) in vals.iter().enumerate() {
                let n = self.go_value(v);
                if i == 0 {
                    o.push_str(&format!("\t{n}{CELL}{ascii} = iota{CELL}// {v}\n"));
                } else {
                    o.push_str(&format!("\t{n}{CELL}{CELL}// {v}\n"));
                }
            }
            o.push_str(")\n\n");
            o.push_str(&format!(
                "func (v {ascii}) Valid() bool {{ return v >= 0 && v < {} }}\n\n",
                vals.len()
            ));
            o.push_str(&format!("func (v {ascii}) String() string {{\n\tswitch v {{\n"));
            for v in vals {
                o.push_str(&format!("\tcase {}:\n\t\treturn {:?}\n", self.go_value(v), v));
            }
            o.push_str("\t}\n\treturn \"?\"\n}\n\n");
            // The wire format of §10 uses the Japanese names, so provide a way back from them.
            o.push_str(&format!("func Parse{ascii}(s string) ({ascii}, bool) {{\n\tswitch s {{\n"));
            for v in vals {
                o.push_str(&format!("\tcase {:?}:\n\t\treturn {}, true\n", v, self.go_value(v)));
            }
            o.push_str(&format!("\t}}\n\treturn {}(0), false\n}}\n\n", ascii));
        }

        // Groups. They are unexported, so the Japanese identifiers can stay (§8.1).
        for g in &self.f.groups {
            let ty = self
                .c
                .groups
                .get(&g.name.text)
                .and_then(|(owner, _)| self.enum_names.get(owner).cloned())
                .unwrap_or_else(|| "int".into());
            o.push_str(&format!("func is{}(v {ty}) bool {{\n\tswitch v {{\n\tcase ", g.name.text, ));
            let ms: Vec<String> = g.members.iter().map(|m| self.go_value(&m.text)).collect();
            o.push_str(&ms.join(", "));
            o.push_str(":\n\t\treturn true\n\t}\n\treturn false\n}\n\n");
        }

        o.push_str(&round_go());
        let body = self.go_fn();
        let needs_fmt = body.contains("fmt.Errorf");
        o.push_str(&body);
        let o = o.replace(IMPORT_MARK, if needs_fmt { "import \"fmt\"\n\n" } else { "" });
        align(&o)
    }

    fn go_fn(&self) -> String {
        let fname = pascal(&pub_name(&self.f.name));
        let outs = &self.f.outputs;
        let mut o = String::new();

        o.push_str(&format!(
            "// {}\ntype Input struct {{\n",
            tr!(
                "Input は入力をまとめて受ける。同型の int が並ぶのを避けるため（§8.3）。",
                "Input bundles the inputs, so that a parameter list of same-typed ints is avoided (§8.3)."
            )
        ));
        for i in &self.f.inputs {
            // The field holds the wire integer, so the range is shown in the same units as
            // the guard below it, not in the units the declaration was written in.
            let sc = self.c.wire_scale(&i.name.text);
            let doc = match self.c.ranges.get(&i.name.text) {
                Some((Some(lo), Some(hi))) => tr!(
                    "// {} 範囲 {}..{}",
                    "// {} range {}..{}",
                    i.name.text,
                    crate::types::wire_int(*lo, sc),
                    crate::types::wire_int(*hi, sc)
                ),
                _ => format!("// {}", i.name.text),
            };
            o.push_str(&format!("\t{}{CELL}{}{CELL}{doc}\n", pascal(&pub_name(&i.name)), self.go_ty(&self.ty_of(&i.name.text))));
        }
        o.push_str("}\n\n");

        let ret = if outs.len() == 1 {
            self.go_ty(&self.ty_of(&outs[0].name.text))
        } else {
            o.push_str("type Output struct {\n");
            for od in outs {
                o.push_str(&format!("\t{}{CELL}{}\n", pascal(&pub_name(&od.name)), self.go_ty(&self.ty_of(&od.name.text))));
            }
            o.push_str("}\n\n");
            "Output".into()
        };
        let zero = if outs.len() == 1 {
            self.go_zero(&self.ty_of(&outs[0].name.text))
        } else {
            "Output{}".to_string()
        };

        // Go's defined types cannot be mixed in arithmetic. Brands are enforced on the public
        // surface (Input and the return value) while internal arithmetic runs on plain int64 (the
        // flip side of the asymmetry in §8.3).
        let local = |n: &str| -> String {
            match self.f.inputs.iter().find(|i| i.name.text == n) {
                Some(i) => {
                    let f = format!("in.{}", pascal(&pub_name(&i.name)));
                    if matches!(self.ty_of(n), Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Date) {
                        format!("int64({f})")
                    } else {
                        f
                    }
                }
                None => n.to_string(),
            }
        };

        o.push_str(&tr!(
            "// {fname} は規則 {} v{} を評価する。分岐は原本の行と 1:1 に対応する。\n",
            "// {fname} evaluates rule {} v{}. Each branch corresponds 1:1 to a row of the rule source.\n",
            self.f.name.text, self.f.version
        ));
        o.push_str(&format!("func {fname}(in Input) ({ret}, error) {{\n"));

        for i in &self.f.inputs {
            let v = local(&i.name.text);
            let ty = self.ty_of(&i.name.text);
            match &ty {
                Ty::Enum(_) => o.push_str(&format!(
                    "\tif !{v}.Valid() {{\n\t\treturn {zero}, fmt.Errorf(\"{}\", {v})\n\t}}\n",
                    tr!("{} が列挙の値ではありません: %d", "{} is not a value of the enum: %d", i.name.text)
                )),
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Date => {
                    if let Some((Some(lo), Some(hi))) = self.c.ranges.get(&i.name.text) {
                        let sc = self.c.wire_scale(&i.name.text);
                        o.push_str(&format!(
                            "\tif {v} < {} || {v} > {} {{\n\t\treturn {zero}, fmt.Errorf(\"{}\", {v})\n\t}}\n",
                            crate::types::wire_int(*lo, sc),
                            crate::types::wire_int(*hi, sc),
                            tr!("{} が範囲の外です: %d", "{} is out of range: %d", i.name.text)
                        ));
                    }
                }
                _ => {}
            }
        }

        for it in &self.f.items {
            match it {
                Item::Derived(d) => {
                    let e = self.expr(&d.expr, &local);
                    o.push_str(&format!("\t{} := {}{CELL}// {}\n", d.name.text, go_expr(&e.text), tr!("導出", "derived value")));
                }
                Item::Define(d) => {
                    let e = self.expr(&d.expr, &local);
                    o.push_str(&format!("\t{} := {}{CELL}// {}\n", d.name.text, go_expr(&e.text), tr!("定義", "definition")));
                }
                Item::Table(t) => o.push_str(&self.go_table(t, &local)),
            }
        }

        let out_name = &outs[0].name.text;
        let res = match &self.f.result {
            Some(r) => self.expr(&r.expr, &local),
            None => Expr2 { text: local(out_name), scale: self.scale(out_name) },
        };
        let os = self.out_scale(out_name);
        let text = match &outs[0].rounding {
            Some(rd) => {
                let ty = self.ty_of(out_name);
                let g = crate::types::lit_value_in_pub(&rd.grid, &ty).unwrap_or(Rat::int(1));
                let m = RoundMode::parse(&rd.mode).unwrap_or(RoundMode::Down);
                let grid_i = g.num * res.scale / g.den;
                if res.scale != os {
                    o.push_str(&format!(
                        // The cast needs its own parentheses: an expression that begins with
                        // a call rather than a `(` glued itself to the type name.
                        "\traw := int64({}){CELL}// {}\n",
                        go_expr(&res.text),
                        tr!("単位: 1/{} {}", "unit: 1/{} {}", res.scale, ty)
                    ));
                    format!("round{}(raw, {}) / {}", pascal(mode_fn(m)), grid_i, res.scale / os)
                } else {
                    format!("round{}(int64({}), {})", pascal(mode_fn(m)), go_expr(&res.text), grid_i)
                }
            }
            // Only a number needs the widening cast; `int64(可否)` does not compile.
            None if !self.ty_of(out_name).is_numeric() => go_expr(&res.text),
            None => format!("int64({})", go_expr(&res.text)),
        };
        if outs.len() == 1 {
            o.push_str(&format!("\treturn {ret}({text}), nil\n}}\n"));
        } else {
            // Each output gets its own rounding, applied exactly once (§7.2).
            let fields: Vec<String> = outs
                .iter()
                .map(|od| {
                    let n = &od.name.text;
                    let g = pascal(&pub_name(&od.name));
                    let ty = self.ty_of(n);
                    let body = match &od.rounding {
                        Some(rd) => {
                            let q = crate::types::lit_value_in_pub(&rd.grid, &ty).unwrap_or(Rat::int(1));
                            let m = RoundMode::parse(&rd.mode).unwrap_or(RoundMode::Down);
                            format!(
                                "round{}({}, {})",
                                pascal(mode_fn(m)),
                                local(n),
                                q.num * self.scale(n) / q.den
                            )
                        }
                        None => local(n),
                    };
                    format!("{g}: {}({body})", self.go_ty(&ty))
                })
                .collect();
            o.push_str(&format!("\treturn Output{{{}}}, nil\n}}\n", fields.join(", ")));
        }
        o
    }

    fn go_table(&self, t: &Table, local: &dyn Fn(&str) -> String) -> String {
        let name = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
        let policy = if t.policy == Policy::Unique { crate::kw::UNIQUE } else { crate::kw::FIRST };
        let mut o = format!("\t// {}\n", tr!("表 {name}（{} {policy}）", "table {name} ({} {policy})", crate::kw::POLICY));
        // In Go a variable declared inside an if does not escape it, so declare them up front.
        for oc in &t.outputs {
            let t2 = self.ty_of(&oc.name.text);
            let ty = if matches!(t2, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Date) {
                "int64".to_string()
            } else {
                self.go_ty(&t2)
            };
            o.push_str(&format!("\tvar {} {ty}\n", oc.name.text));
        }
        for (ri, row) in t.rows.iter().enumerate() {
            let conds: Vec<String> = t
                .inputs
                .iter()
                .enumerate()
                .filter_map(|(ci, (col, _))| {
                    let ty = self.ty_of(col);
                    self.go_cell(row.cells.get(ci)?, &local(col), &ty, self.scale(col))
                })
                .collect();
            let cond = if conds.is_empty() { "true".into() } else { conds.join(" && ") };
            let kw = if ri == 0 { "\tif" } else { " else if" };
            let cells: Vec<String> = row.cells.iter().map(cell_src).chain(row.outs.iter().map(out_src)).collect();
            if ri == 0 {
                o.push_str(&format!("{kw} {cond} {{ // {}\n", tr!("行1: {}", "row 1: {}", cells.join(" | "))));
            } else {
                o.push_str(&format!("\t}}{kw} {cond} {{ // {}\n", tr!("行{}: {}", "row {}: {}", ri + 1, cells.join(" | "))));
            }
            for (oi, oc) in t.outputs.iter().enumerate() {
                let v = match row.outs.get(oi) {
                    Some(OutCell::Lit(Lit::Num(n))) => {
                        let ty = self.ty_of(&oc.name.text);
                        self.int_lit(n, &ty, self.scale(&oc.name.text))
                    }
                    Some(OutCell::Lit(l)) => match l {
                        Lit::Word(w) if w == crate::kw::TRUE => "true".into(),
                        Lit::Word(w) if w == crate::kw::FALSE => "false".into(),
                        Lit::Word(w) => self.go_value(w),
                        Lit::Date(y, m, d) => format!("{}", crate::types::date_ord(*y, *m, *d).num),
                        _ => "0".into(),
                    },
                    Some(OutCell::Name(w)) => {
                        if w == crate::kw::TRUE {
                            "true".into()
                        } else if w == crate::kw::FALSE {
                            "false".into()
                        } else if self.value_names.contains_key(w) {
                            self.go_value(w)
                        } else {
                            self.rescaled(w, &oc.name.text, local(w))
                        }
                    }
                    None => "0".into(),
                };
                o.push_str(&format!("\t\t{} = {v}\n", oc.name.text));
            }
        }
        o.push_str(&format!(
            "\t}} else {{\n\t\tpanic(\"{}\")\n\t}}\n",
            tr!("到達不能: 完全性は rulec が静的に検査済み", "unreachable: completeness was statically checked by rulec")
        ));
        // A column nothing downstream reads is still assigned above, so that the branch and
        // the row stay 1:1. Go will not compile a local that is never read, so say out loud
        // that it is on purpose. W111 has already named the column.
        for oc in &t.outputs {
            if !self.is_read(&oc.name.text) {
                o.push_str(&format!("\t_ = {}\n", oc.name.text));
            }
        }
        o.push_str(&self.guards(t, local, "\t", |name, i, j| {
            format!(
                "\t\treturn {}, fmt.Errorf(\"{}\")\n",
                if self.f.outputs.len() == 1 {
                    self.go_zero(&self.ty_of(&self.f.outputs[0].name.text))
                } else {
                    "Output{}".into()
                },
                tr!("表 {name}: 行{i} と 行{j} が同時に当たりました", "table {name}: row {i} and row {j} matched at the same time")
            )
        }));
        o
    }
}

/// Adapt an expression built for Python to Go's spelling. Only integer division differs.
fn go_expr(s: &str) -> String {
    let t = s
        .replace(" // ", " / ")
        .replace("True", "true")
        .replace("False", "false")
        .replace("_round_down(", "roundDown(")
        .replace("_round_up(", "roundUp(")
        .replace("_round_half(", "roundHalf(")
        .replace("_round_bankers(", "roundBankers(")
        .replace("_min(", "minInt(")
        .replace("_max(", "maxInt(");
    tighten_nested_products(&t)
}

/// Match one habit of `gofmt`: a product written inside a group that sits two or more call
/// arguments deep loses the spaces around its operator.
///
/// The generator formats its own Go rather than shelling out to `gofmt` (§8.5: the output
/// must not depend on the version of a tool installed on the machine), and a test then holds
/// it to `gofmt -l` being empty. That test is what turned this up: `roundDown(minInt(x, (int64(in.Cap) * 100)), (1 * 100))`
/// comes back from `gofmt` with the first product tightened and the second left alone. The
/// rule below reproduces that on the shapes the generator emits — at one call deep the spaces
/// stay, which is what `gofmt` does too.
fn tighten_nested_products(s: &str) -> String {
    let b: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    // For every open paren still on the stack: was it a call's, or a grouping's?
    let mut stack: Vec<bool> = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == '(' {
            let call = i > 0 && (b[i - 1].is_alphanumeric() || b[i - 1] == '_');
            stack.push(call);
        } else if c == ')' {
            stack.pop();
        }
        let calls = stack.iter().filter(|x| **x).count();
        let in_group = stack.last() == Some(&false);
        if in_group && calls >= 2 && c == ' ' && i + 2 < b.len() && (b[i + 1] == '*' || b[i + 1] == '/') && b[i + 2] == ' ' {
            out.push(b[i + 1]);
            i += 3;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

fn round_go() -> String {
    tr!(
        "// §7.3 の四モード。負の向きと半分ちょうどまで仕様どおりに固定する。\n\
         // 言語の素の除算に任せない（Python の // は −∞ 方向、Go は 0 方向）。\n",
        "// The four modes of §7.3. Negative values and exact halves are pinned to the spec as well;\n\
         // nothing is left to the language's native division (Python's // goes toward −∞, Go's toward 0).\n"
    ) + ROUND_GO_BODY
}

const ROUND_GO_BODY: &str = r#"
func absMod(x, g int64) (int64, int64, bool) {
	neg := x < 0
	if neg {
		x = -x
	}
	return x / g, x % g, neg
}

func roundDown(x, g int64) int64 {
	q, _, neg := absMod(x, g)
	v := q * g
	if neg {
		return -v
	}
	return v
}

func roundUp(x, g int64) int64 {
	q, r, neg := absMod(x, g)
	if r != 0 {
		q++
	}
	v := q * g
	if neg {
		return -v
	}
	return v
}

func roundHalf(x, g int64) int64 {
	q, r, neg := absMod(x, g)
	if 2*r >= g {
		q++
	}
	v := q * g
	if neg {
		return -v
	}
	return v
}

func minInt(a, b int64) int64 {
	if a < b {
		return a
	}
	return b
}

func maxInt(a, b int64) int64 {
	if a > b {
		return a
	}
	return b
}

func roundBankers(x, g int64) int64 {
	q, r, neg := absMod(x, g)
	if 2*r > g || (2*r == g && q%2 == 1) {
		q++
	}
	v := q * g
	if neg {
		return -v
	}
	return v
}

"#;

/// Our own column alignment, matching gofmt's tabwriter.
///
/// Running gofmt as a later stage would make the generated code depend on whichever gofmt is
/// installed in that environment, breaking the determinism of §8.5. Alignment is local to a run
/// of declarations, so doing it ourselves is enough. The control character used as a separator
/// marks consecutive lines with the same number of columns as one block to align together.
const CELL: char = '\u{1f}';
const IMPORT_MARK: &str = "\u{1e}IMPORTS\u{1e}";

/// Column width is measured in runes. That is how Go's text/tabwriter counts, so measuring by
/// display width (`diag::width`, which counts a full-width character as 2) would drift from gofmt
/// by one character wherever Japanese identifiers line up. §8.5 requires `gofmt -l` to print
/// nothing, so we follow gofmt's logic rather than the visual one.
fn cells_wide(s: &str) -> usize {
    s.chars().count()
}

/// Drop the parentheses wrapping the whole expression. The generator parenthesizes every
/// subexpression uniformly, which leaves one redundant outer pair on the right-hand side of an
/// assignment. Python's formatter wants it gone, so to keep `ruff format --check` green we drop
/// it at generation time (honoring §8.5's "do not rely on a later formatting stage" on the Python
/// side too). Go is left alone, since gofmt does not remove it.
fn unparen(s: &str) -> &str {
    let t = s.trim();
    let Some(inner) = t.strip_prefix('(').and_then(|x| x.strip_suffix(')')) else { return s };
    // Drop the outer pair only when it really matches itself. `(a) + (b)` is left alone.
    let mut depth = 0i32;
    for ch in inner.chars() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth < 0 {
                    return s;
                }
            }
            _ => {}
        }
    }
    if depth == 0 { inner } else { s }
}

/// Fix up PEP 8 blank lines (E301–E305) in one place. Output assembly is scattered across the
/// generator, and forgetting a single blank line turns the formatter red. Rather than being
/// careful at every emitting site, run one pass at the end: two blank lines around top-level
/// `class` / `def` / decorators, at most one before any other top-level statement.
fn pep8_blanks(src: &str) -> String {
    let lines: Vec<&str> = src.trim_end().split('\n').collect();
    let mut out: Vec<String> = Vec::new();
    let mut in_block = false; // whether the previous top-level construct was a def / class body
    for line in lines {
        let top = !line.is_empty() && !line.starts_with(char::is_whitespace);
        if line.trim().is_empty() {
            continue; // blank lines are dropped and reinserted where needed
        }
        if top {
            let starts = line.starts_with("class ")
                || line.starts_with("def ")
                || line.starts_with('@');
            let want = if starts || in_block { 2 } else { 1 };
            if !out.is_empty() {
                let have = out.iter().rev().take_while(|l| l.is_empty()).count();
                for _ in have..want {
                    out.push(String::new());
                }
            }
            if starts {
                in_block = true;
            } else if !line.starts_with('#') {
                in_block = false;
            }
        }
        out.push(line.to_string());
    }
    // Drop blank lines at the start of the file.
    while out.first().is_some_and(|l| l.is_empty()) {
        out.remove(0);
    }
    out.push(String::new());
    out.join("\n")
}

fn align(src: &str) -> String {
    let lines: Vec<&str> = src.split('\n').collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut i = 0;
    while i < lines.len() {
        if !lines[i].contains(CELL) {
            out.push(lines[i].to_string());
            i += 1;
            continue;
        }
        let n = lines[i].matches(CELL).count();
        let start = i;
        while i < lines.len() && lines[i].contains(CELL) && lines[i].matches(CELL).count() == n {
            i += 1;
        }
        let rows: Vec<Vec<&str>> = lines[start..i].iter().map(|l| l.split(CELL).collect()).collect();
        let mut w = vec![0usize; n];
        for r in &rows {
            for (k, c) in r.iter().take(n).enumerate() {
                w[k] = w[k].max(cells_wide(c));
            }
        }
        for r in rows {
            let mut line = String::new();
            for (k, c) in r.iter().enumerate() {
                line.push_str(c);
                if k < n && r[k + 1..].iter().any(|x| !x.trim().is_empty()) {
                    line.push_str(&" ".repeat(w[k] - cells_wide(c) + 1));
                }
            }
            out.push(line.trim_end().to_string());
        }
    }
    out.join("\n")
}

// ---------------------------------------------------------------------------
// Runners (§9.3: feed the vectors through both languages and check the three-way agreement)
// ---------------------------------------------------------------------------

impl<'a> Gen<'a> {
    /// A Python runner that reads JSONL from stdin and prints just the outputs, one record per line.
    pub fn python_runner(&self) -> String {
        let alias = pub_name(&self.f.name);
        let mut args: Vec<String> = Vec::new();
        for i in &self.f.inputs {
            let ty = self.ty_of(&i.name.text);
            let jp = &i.name.text;
            let conv = match &ty {
                Ty::Enum(n) => {
                    let cls = self.enum_names.get(n).cloned().unwrap_or_default();
                    format!("m.{cls}(d[{jp:?}])")
                }
                Ty::Bool => format!("bool(d[{jp:?}])"),
                Ty::Date => format!("_ord(d[{jp:?}])"),
                _ => format!("int(d[{jp:?}])"),
            };
            args.push(conv);
        }
        let dump = if self.f.outputs.len() == 1 {
            let n = &self.f.outputs[0].name.text;
            format!("{{{n:?}: _wire(r)}}")
        } else {
            let fs: Vec<String> = self
                .f
                .outputs
                .iter()
                .map(|o| format!("{:?}: _wire(r.{})", o.name.text, pub_name(&o.name)))
                .collect();
            format!("{{{}}}", fs.join(", "))
        };
        format!(
            "# Code generated by rulec {}. DO NOT EDIT.\n\
             import datetime\n\
             import json\n\
             import sys\n\n\
             import {alias} as m\n\n\
             def _ord(s):\n    \
                 y, mo, d = (int(x) for x in s.split(\"-\"))\n    \
                 return (datetime.date(y, mo, d) - datetime.date(1970, 1, 1)).days\n\n\
             def _wire(v):\n    \
                 return v.value if hasattr(v, \"value\") else v\n\n\
             for line in sys.stdin:\n    \
                 line = line.strip()\n    \
                 if not line:\n        \
                     continue\n    \
                 d = json.loads(line)[\"in\"]\n    \
                 r = m.{alias}({})\n    \
                 print(json.dumps({dump}, ensure_ascii=False, separators=(\",\", \":\")))\n",
            env!("CARGO_PKG_VERSION"),
            args.join(", ")
        )
    }

    /// A Go runner that does the same. encoding/json is in the standard library.
    pub fn go_runner(&self) -> String {
        let alias = pub_name(&self.f.name);
        let pkg = alias.replace('_', "").to_lowercase();
        let fname = pascal(&alias);
        let mut fields: Vec<String> = Vec::new();
        for i in &self.f.inputs {
            let ty = self.ty_of(&i.name.text);
            let jp = &i.name.text;
            let g = pascal(&pub_name(&i.name));
            let conv = match &ty {
                Ty::Enum(n) => {
                    let cls = self.enum_names.get(n).cloned().unwrap_or_default();
                    format!("\t\tv{g}, _ := r.Parse{cls}(str(d[{jp:?}]))\n\t\tin.{g} = v{g}\n")
                }
                Ty::Bool => format!("\t\tin.{g} = d[{jp:?}] == true\n"),
                Ty::Date => format!("\t\tin.{g} = ord(str(d[{jp:?}]))\n"),
                _ => format!("\t\tin.{g} = r.{}(num(d[{jp:?}]))\n", self.go_ty(&ty)),
            };
            fields.push(conv);
        }
        // The wire format uses the Japanese names (§10). Enums are emitted by name, not by number.
        let one = |expr: &str, ty: &Ty| match ty {
            Ty::Enum(_) => format!("{expr}.String()"),
            Ty::Bool => expr.to_string(),
            _ => format!("int64({expr})"),
        };
        let wire = if self.f.outputs.len() == 1 {
            let o = &self.f.outputs[0];
            format!("{:?}: {}", o.name.text, one("got", &self.ty_of(&o.name.text)))
        } else {
            self.f
                .outputs
                .iter()
                .map(|o| {
                    let ty = self.ty_of(&o.name.text);
                    format!(
                        "{:?}: {}",
                        o.name.text,
                        one(&format!("got.{}", pascal(&pub_name(&o.name))), &ty)
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        };
        format!(
            "// Code generated by rulec {}. DO NOT EDIT.\n\
             package main\n\n\
             import (\n\t\"bufio\"\n\t\"encoding/json\"\n\t\"fmt\"\n\t\"os\"\n\t\"time\"\n\n\tr \"{pkg}\"\n)\n\n\
             func str(v any) string {{ s, _ := v.(string); return s }}\n\n\
             func num(v any) int64 {{ f, _ := v.(float64); return int64(f) }}\n\n\
             func ord(s string) int64 {{\n\t\
                 t, _ := time.Parse(\"2006-01-02\", s)\n\t\
                 return int64(t.Sub(time.Date(1970, 1, 1, 0, 0, 0, 0, time.UTC)).Hours() / 24)\n}}\n\n\
             func main() {{\n\t\
                 sc := bufio.NewScanner(os.Stdin)\n\t\
                 sc.Buffer(make([]byte, 1<<20), 1<<20)\n\t\
                 for sc.Scan() {{\n\t\t\
                     if len(sc.Bytes()) == 0 {{\n\t\t\tcontinue\n\t\t}}\n\t\t\
                     var rec struct {{\n\t\t\tIn map[string]any `json:\"in\"`\n\t\t}}\n\t\t\
                     if err := json.Unmarshal(sc.Bytes(), &rec); err != nil {{\n\t\t\tpanic(err)\n\t\t}}\n\t\t\
                     d := rec.In\n\t\t\
                     var in r.Input\n{}\t\t\
                     got, err := r.{fname}(in)\n\t\t\
                     if err != nil {{\n\t\t\tpanic(err)\n\t\t}}\n\t\t\
                     b, _ := json.Marshal(map[string]any{{{wire}}})\n\t\t\
                     fmt.Println(string(b))\n\t}}\n}}\n",
            env!("CARGO_PKG_VERSION"),
            fields.join("")
        )
    }
}

// ---------------------------------------------------------------------------
// Unit vectors for the rounding helpers (§8.5)
// ---------------------------------------------------------------------------

/// Four modes × grids × values. Negative values and exact halves are always included.
/// Table-level agreement alone would hide a broken helper on a table that never produces a
/// fraction.
fn round_cases() -> Vec<(RoundMode, i128, i128, i128)> {
    let mut out = Vec::new();
    for m in [RoundMode::Down, RoundMode::Up, RoundMode::Half, RoundMode::Bankers] {
        for g in [1i128, 10, 100] {
            for x in [-25i128, -20, -15, -11, -10, -5, -1, 0, 1, 5, 10, 11, 15, 20, 25, 105, 150, 250] {
                let want = Rat::int(x).round_to(m, Rat::int(g));
                out.push((m, x, g, want.num / want.den));
            }
        }
    }
    out
}

pub fn round_tests_python() -> String {
    let mut o = format!(
        "# Code generated by rulec {}. DO NOT EDIT.\n\
         # {}\n\
         import sys\n\n",
        env!("CARGO_PKG_VERSION"),
        tr!(
            "§7.3 の四モード。負の向きと半分ちょうどまで、Rust の参照実装と突き合わせる。",
            "The four modes of §7.3, checked against the Rust reference implementation down to negative values and exact halves."
        )
    );
    o.push_str(round_py().trim_start_matches('\n'));
    o.push_str("\nCASES = [\n");
    for (m, x, g, want) in round_cases() {
        o.push_str(&format!("    (\"{}\", {x}, {g}, {want}),\n", mode_fn(m)));
    }
    o.push_str("]\n\nFN = {\n    \"down\": _round_down,\n    \"up\": _round_up,\n    \"half\": _round_half,\n    \"bankers\": _round_bankers,\n}\n\n");
    o.push_str(
        "bad = 0\nfor mode, x, g, want in CASES:\n    \
         got = FN[mode](x, g)\n    \
         if got != want:\n        \
             print(f\"NG {mode}({x}, {g}) = {got}, want {want}\")\n        \
             bad += 1\nif bad:\n    sys.exit(1)\n",
    );
    o.push_str(&format!("print(f\"{}\")\n", tr!("ok {{len(CASES)}} 件", "ok {{len(CASES)}} cases")));
    o
}

pub fn round_tests_go(pkg: &str) -> String {
    let mut o = format!(
        "// Code generated by rulec {}. DO NOT EDIT.\n\
         // {}\n\
         package {pkg}\n\nimport \"testing\"\n\n\
         func TestRoundingModes(t *testing.T) {{\n\t\
         cases := []struct {{\n\t\tmode{CELL}string\n\t\tx, g, want{CELL}int64\n\t}}{{\n",
        env!("CARGO_PKG_VERSION"),
        tr!(
            "§7.3 の四モード。負の向きと半分ちょうどまで、Rust の参照実装と突き合わせる。",
            "The four modes of §7.3, checked against the Rust reference implementation down to negative values and exact halves."
        )
    );
    for (m, x, g, want) in round_cases() {
        o.push_str(&format!("\t\t{{{:?}, {x}, {g}, {want}}},\n", mode_fn(m)));
    }
    o.push_str(
        "\t}\n\tfor _, c := range cases {\n\t\tvar got int64\n\t\tswitch c.mode {\n\t\t\
         case \"down\":\n\t\t\tgot = roundDown(c.x, c.g)\n\t\t\
         case \"up\":\n\t\t\tgot = roundUp(c.x, c.g)\n\t\t\
         case \"half\":\n\t\t\tgot = roundHalf(c.x, c.g)\n\t\t\
         case \"bankers\":\n\t\t\tgot = roundBankers(c.x, c.g)\n\t\t}\n\t\t\
         if got != c.want {\n\t\t\tt.Errorf(\"%s(%d, %d) = %d, want %d\", c.mode, c.x, c.g, got, c.want)\n\t\t}\n\t}\n}\n",
    );
    align(&o)
}

fn go_comment_ok(indent: &str) -> bool {
    indent.starts_with('\t')
}

// ── The API inventory (`rulec api`) ──────────────────────────────────────
//
// What a caller has to know to invoke the generated code: the module and function names,
// the parameters in order with their brands, units and ranges, the outputs with their
// rounding, the enum members under the spelling the generated code gives them, and the
// errors it can raise.
//
// It lives here, next to the generator, on purpose. Anywhere else it would be a second
// description of the same thing and would start drifting the first time a name changes; here
// it is built from the same `pub_name`, `pascal`, `py_ty` and `go_ty` the emitters use, and a
// test runs the generated Python and Go against it.

impl Gen<'_> {
    /// The unit as it is written in the rule (`円`, `g`, `%`), or absent for a type that has
    /// none.
    fn unit_of(ty: &Ty) -> Option<String> {
        match ty {
            Ty::Money { cur, .. } => Some(cur.clone()),
            Ty::Qty { unit, .. } => Some(unit.clone()),
            Ty::Rate => Some("%".into()),
            Ty::Opt(t) => Self::unit_of(t),
            _ => None,
        }
    }

    /// The range **the generated guard actually enforces**, not a second computation of it.
    /// Both entry guards above take `lo.num` and `hi.num`, so this does too: an inventory that
    /// disagreed with the guard would send a caller values the code then rejects. A test holds
    /// these two numbers to the text of the generated guard (§8.6).
    fn range_json(&self, name: &str) -> Option<String> {
        let (lo, hi) = self.c.ranges.get(name)?;
        let (lo, hi) = (lo.as_ref()?, hi.as_ref()?);
        let sc = self.c.wire_scale(name);
        Some(
            crate::json::Obj::new()
                .int("min", crate::types::wire_int(*lo, sc))
                .int("max", crate::types::wire_int(*hi, sc))
                .finish(),
        )
    }

    /// `{"mode":"down","grid":1}` — the grid as an integer in the canonical unit, so it can be
    /// compared against a value straight away.
    fn rounding_json(&self, od: &OutDecl) -> Option<String> {
        let rd = od.rounding.as_ref()?;
        let ty = self.ty_of(&od.name.text);
        let g = crate::types::lit_value_in_pub(&rd.grid, &ty)?;
        Some(
            crate::json::Obj::new()
                .str("mode", &rd.mode)
                .int("grid", g.num / g.den)
                .finish(),
        )
    }

    /// One parameter or field, described the same way on both sides.
    fn value_json(&self, jp: &str, alias: &str, ty_name: &str, ty: &Ty) -> String {
        crate::json::Obj::new()
            .str("name", jp)
            .str("alias", alias)
            .str("type", ty_name)
            .opt_str("unit", Self::unit_of(ty))
            .opt_raw("range", self.range_json(jp))
            .bool("optional", matches!(ty, Ty::Opt(_)))
            .finish()
    }

    /// The enums, under the spelling each language gives their members.
    fn enums_json(&self, member: impl Fn(&str, &str) -> String) -> String {
        let mut out: Vec<String> = Vec::new();
        let mut done: Vec<String> = Vec::new();
        for (jp, ascii) in &self.enum_names {
            if done.contains(ascii) {
                continue;
            }
            done.push(ascii.clone());
            let Some(vals) = self.c.enums.get(jp) else { continue };
            let members: Vec<String> = vals
                .iter()
                .map(|v| {
                    let a = self.value_names.get(v).map(|(_, a)| a.clone()).unwrap_or_else(|| v.clone());
                    crate::json::Obj::new()
                        .str("name", v)
                        .str("alias", member(ascii, &a))
                        .finish()
                })
                .collect();
            out.push(
                crate::json::Obj::new()
                    .str("name", jp)
                    .str("alias", ascii)
                    .raw("values", crate::json::arr(&members))
                    .finish(),
            );
        }
        crate::json::arr(&out)
    }

    /// `rulec api <file.rule> --format json`. Language independent throughout: every field is
    /// a name or a number that the generated code really uses.
    pub fn api(&self) -> String {
        let alias = pub_name(&self.f.name);
        let pkg = alias.replace('_', "").to_lowercase();
        let outs = &self.f.outputs;

        // --- Python
        let params: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| {
                let ty = self.ty_of(&i.name.text);
                self.value_json(&i.name.text, &pub_name(&i.name), &self.py_ty(&ty), &ty)
            })
            .collect();
        let py_outs: Vec<String> = outs
            .iter()
            .map(|od| {
                let ty = self.ty_of(&od.name.text);
                let v = self.value_json(&od.name.text, &pub_name(&od.name), &self.py_ty(&ty), &ty);
                // An output carries its rounding as well; `value_json` is shared with the
                // inputs, so it is spliced in here rather than made optional there.
                match self.rounding_json(od) {
                    Some(r) => format!("{},\"rounding\":{r}}}", v.trim_end_matches('}')),
                    None => v,
                }
            })
            .collect();
        let py_ret = if outs.len() == 1 {
            self.py_ty(&self.ty_of(&outs[0].name.text))
        } else {
            "Output".into()
        };
        let py_sig = format!(
            "def {alias}({}) -> {py_ret}:",
            self.f
                .inputs
                .iter()
                .map(|i| format!("{}: {}", pub_name(&i.name), self.py_ty(&self.ty_of(&i.name.text))))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let python = crate::json::Obj::new()
            .str("module", &alias)
            .str("function", &alias)
            .str("signature", &py_sig)
            .raw("params", crate::json::arr(&params))
            .str("returns", &py_ret)
            .raw("outputs", crate::json::arr(&py_outs))
            // A Python enum member is the alias in upper case (`CouponKind.PERCENT`).
            .raw("enums", self.enums_json(|_, a| a.to_uppercase()))
            .raw("errors", crate::json::strs(&["RuleInputError", "RuleContradictionError"]))
            .finish();

        // --- Go
        let go_in: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| {
                let ty = self.ty_of(&i.name.text);
                self.value_json(
                    &i.name.text,
                    &pascal(&pub_name(&i.name)),
                    &self.go_ty(&ty),
                    &ty,
                )
            })
            .collect();
        let go_outs: Vec<String> = outs
            .iter()
            .map(|od| {
                let ty = self.ty_of(&od.name.text);
                let v = self.value_json(&od.name.text, &pascal(&pub_name(&od.name)), &self.go_ty(&ty), &ty);
                match self.rounding_json(od) {
                    Some(r) => format!("{},\"rounding\":{r}}}", v.trim_end_matches('}')),
                    None => v,
                }
            })
            .collect();
        let go_fn = pascal(&alias);
        let go_ret = if outs.len() == 1 {
            self.go_ty(&self.ty_of(&outs[0].name.text))
        } else {
            "Output".into()
        };
        let go_sig = format!("func {go_fn}(in Input) ({go_ret}, error)");
        let go = crate::json::Obj::new()
            .str("package", &pkg)
            .str("func", &go_fn)
            .str("signature", &go_sig)
            .str("input_type", "Input")
            .raw("input_fields", crate::json::arr(&go_in))
            .str("output_type", &go_ret)
            .raw("output_fields", crate::json::arr(&go_outs))
            // A Go enum member is the type name followed by the alias (`CouponKindPercent`).
            .raw("enums", self.enums_json(|t, a| format!("{t}{a}")))
            .finish();

        crate::json::Obj::new()
            .str("rule", &self.f.name.text)
            .str("alias", &alias)
            .str("version", &self.f.version)
            .str("source_sha256", &self.src_hash)
            .raw("python", python)
            .raw("go", go)
            .finish()
    }
}
