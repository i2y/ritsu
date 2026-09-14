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
            "mg" => "Milligram".into(),
            "g" => "Gram".into(),
            "kg" => "Kilogram".into(),
            "t" => "Tonne".into(),
            "oz" => "Ounce".into(),
            "lb" => "Pound".into(),
            "mm" => "Millimeter".into(),
            "cm" => "Cm".into(),
            "m" => "Meter".into(),
            "km" => "Kilometer".into(),
            "in" => "Inch".into(),
            "ft" => "Foot".into(),
            "yd" => "Yard".into(),
            "mi" => "Mile".into(),
            other => other.into(),
        },
        Ty::Rate => "Rate".into(),
        // A number carries no unit, so there is nothing to brand it apart from. Two counts
        // of different things would get the same brand anyway, and the caller would be left
        // wrapping every integer for no protection at all.
        Ty::Number => "int".into(),
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
    /// Declared name → the identifier the generated code uses for it (§1.3).
    idents: BTreeMap<String, String>,
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
        let mut w114: BTreeMap<String, Vec<(usize, usize)>> = BTreeMap::new();
        for it in &f.items {
            if let Item::Table(t) = it {
                let r = crate::region::check_table(t, c, f, "", crate::region::DEFAULT_BUDGET);
                if !r.w114.is_empty() {
                    w114.insert(t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default(), r.w114);
                }
            }
        }
        // §1.3: a name that declared an ASCII alias is written out under that alias, inside
        // the function as well as on the public face.
        let mut idents: BTreeMap<String, String> = BTreeMap::new();
        for n in f
            .inputs
            .iter()
            .map(|i| &i.name)
            .chain(f.outputs.iter().map(|o| &o.name))
            .chain(f.groups.iter().map(|g| &g.name))
            .chain(f.items.iter().flat_map(|it| -> Vec<&crate::ast::Name> {
                match it {
                    Item::Derived(d) => vec![&d.name],
                    Item::Define(d) => vec![&d.name],
                    Item::Table(t) => t.outputs.iter().map(|o| &o.name).collect(),
                }
            }))
        {
            if n.ascii.is_some() {
                idents.insert(n.text.clone(), pub_name(n));
            }
        }
        Gen { f, c, w114, enum_names, value_names, idents, src_hash: hash(src) }
    }

    fn ty_of(&self, n: &str) -> Ty {
        self.c.ty_of(n).unwrap_or(Ty::Unknown)
    }

    /// The identifier a declared name gets in the generated code. §1.3: the public face
    /// always uses its ASCII alias, and an internal name uses one when the author wrote it
    /// and the name itself when they did not. Every lookup in this file keys on the declared
    /// name; only what is *written out* goes through here.
    fn ident(&self, n: &str) -> String {
        self.idents.get(n).cloned().unwrap_or_else(|| n.to_string())
    }

    /// A name for a generated temporary that nothing in the rule already answers to. The
    /// value held before the last rounding used to be called `raw` unconditionally, which
    /// collided the day an output declared the alias `raw` — Python quietly rebound it and
    /// TypeScript refused to parse.
    fn temp(&self, base: &str) -> String {
        let mut n = base.to_string();
        while self.idents.contains_key(&n) || self.idents.values().any(|v| *v == n) {
            n.push('_');
        }
        n
    }

    /// Storage scale of a value (values are held as multiples of 1/k; this is that k). §7.1.
    fn scale(&self, n: &str) -> i128 {
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
             {comment} もと: {} (規則 {} v{}, sha256:{})\n",
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
                    Div => {
                        // Dividing by the constant k leaves the stored integer alone and
                        // multiplies the scale by k, so the result is exact. Emitting `//`
                        // would truncate here and Go's `/` would truncate the other way.
                        let k = b.text.parse::<i128>().ok().filter(|k| b.scale > 0 && k % b.scale == 0);
                        match k {
                            Some(k) => Expr2 { text: a.text, scale: a.scale * (k / b.scale) },
                            None => Expr2 { text: format!("({} // {})", a.text, b.text), scale: a.scale },
                        }
                    }
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

/// The name of the intermediate that holds one output's value before rounding. One output
/// keeps the plain `raw` of the example in §8.2; a second needs a name of its own, or the
/// two assignments would land on the same variable.
fn raw_base(oi: usize) -> String {
    if oi == 0 {
        "raw".into()
    } else {
        format!("raw{}", oi + 1)
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
            // A cell that names one group uses the set the module already declares, rather
            // than writing the members out again — otherwise that set is dead code.
            Cell::Lit(Lit::Word(w)) if self.c.groups.contains_key(w) => {
                format!("{var} in _{}", self.ident(w))
            }
            Cell::Lit(Lit::Word(w)) if w == crate::kw::TRUE => var.to_string(),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::FALSE => format!("not {var}"),
            Cell::Lit(l) => format!("{var} == {}", lit(l)),
            Cell::Set(ls) => format!("{var} in {}", members(ls)),
            Cell::Not(ls) if ls.len() == 1 && matches!(&ls[0], Lit::Word(w) if self.c.groups.contains_key(w)) => {
                let Lit::Word(w) = &ls[0] else { unreachable!() };
                format!("{var} not in _{}", self.ident(w))
            }
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
            // A number gets no brand: it is a plain integer on purpose, and branding it
            // would shadow the language's own `int`.
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
            o.push_str(&format!("_{} = frozenset({{{}}})\n", self.ident(&g.name.text), ms.join(", ")));
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
    v = abs(x) // g * g
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
            "    \"\"\"規則 {} v{}。分岐はもとの表の行と 1:1 に対応する。\"\"\"\n",
            "    \"\"\"Rule {} v{}. Each branch corresponds 1:1 to a row of the rule source.\"\"\"\n",
            self.f.name.text, self.f.version
        ));

        // Entry guards (§8.5). They enforce at runtime what the proof assumes: inputs lie within
        // their declared domains.
        let local = |n: &str| -> String { self.ident(n) };
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
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date => {
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
                    o.push_str(&format!("    {} = {}  # {}\n", self.ident(&d.name.text), unparen(&e.text), tr!("導出", "derived value")));
                }
                Item::Define(d) => {
                    let e = self.expr(&d.expr, &local);
                    o.push_str(&format!("    {} = {}  # {}\n", self.ident(&d.name.text), unparen(&e.text), tr!("定義", "definition")));
                }
                Item::Table(t) => o.push_str(&self.py_table(t, &local)),
            }
        }

        // Every output takes the same three steps: its source — the `result` expression for
        // the first output, the binding of its own name otherwise — brought to the wire
        // scale, then its declared rounding applied exactly once. The one-output and the
        // many-output returns each used to compute this, and the second copy had neither
        // step: a `result` was dropped from any rule with two outputs, and a value held in
        // hundredths came back a hundred times too large (§15.16).
        let mut finals: Vec<String> = Vec::new();
        for (oi, od) in outs.iter().enumerate() {
            let out_name = &od.name.text;
            let res = match (&self.f.result, oi) {
                (Some(r), 0) => self.expr(&r.expr, &local),
                _ => Expr2 { text: local(out_name), scale: self.scale(out_name) },
            };
            let os = self.out_scale(out_name);
            let ty = self.ty_of(out_name);
            finals.push(match &od.rounding {
                Some(rd) => {
                    let g = crate::types::lit_value_in_pub(&rd.grid, &ty).unwrap_or(Rat::int(1));
                    let m = RoundMode::parse(&rd.mode).unwrap_or(RoundMode::Down);
                    let grid_i = g.num * res.scale / g.den;
                    // Bind the expression to an intermediate instead of nesting it (the same shape as
                    // the example in §8.2). Criterion 1 demands readability, and deep nesting breaks
                    // the visual correspondence with the generated code.
                    if res.scale != os {
                        let raw = self.temp(&raw_base(oi));
                        o.push_str(&format!(
                            "    {raw} = {}  # {}\n",
                            unparen(&res.text),
                            tr!("単位: 1/{} {}", "unit: 1/{} {}", res.scale, ty)
                        ));
                        format!("_round_{}({raw}, {}) // {}", mode_fn(m), grid_i, res.scale / os)
                    } else {
                        format!("_round_{}({}, {})", mode_fn(m), res.text, grid_i)
                    }
                }
                None => res.text.clone(),
            });
        }
        // The brand has to be put back on. Arithmetic on a `NewType` yields the supertype —
        // `YenInclTax * int` is `int` — so the value that comes out of the rounding helper
        // is a plain `int` and assigning it to a branded output is exactly the mistake the
        // brand exists to catch. Eight of the thirteen corpus rules did not pass
        // `mypy --strict` because of this (§15.22). `NewType.__call__` returns its argument.
        let branded: Vec<String> = outs
            .iter()
            .zip(&finals)
            .map(|(od, f)| {
                let ty = self.ty_of(&od.name.text);
                if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) {
                    format!("{}({f})", brand_of(&ty))
                } else {
                    f.clone()
                }
            })
            .collect();
        if outs.len() == 1 {
            o.push_str(&format!("    return {}\n", branded[0]));
        } else {
            // Multiple outputs are a NamedTuple (§8.5); each carries its own rounding.
            o.push_str(&format!("    return Output({})\n", branded.join(", ")));
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
                o.push_str(&format!("        {} = {v}\n", self.ident(&oc.name.text)));
            }
        }
        o.push_str(&format!(
            "    else:\n        raise AssertionError(\"{}\")\n",
            tr!("到達不能: 完全性は rulec が静的に検査済み", "unreachable: completeness was statically checked by rulec")
        ));
        o.push_str(&self.guards(t, local, Lang::Py, "    ", |name, i, j| {
            format!(
                "        raise RuleContradictionError(\"{}\")\n",
                tr!("表 {name}: 行{i} と 行{j} が同時に当てはまりました", "table {name}: row {i} and row {j} matched at the same time")
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
        lang: Lang,
        indent: &str,
        raise: impl Fn(&str, usize, usize) -> String,
    ) -> String {
        let Some(pairs) = self.w114.get(&t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default()) else {
            return String::new();
        };
        let name = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
        let guard = tr!("ガード", "guard");
        let mut o = String::new();
        for (i, j) in pairs {
            let mut conds: Vec<String> = Vec::new();
            for (ci, (col, _)) in t.inputs.iter().enumerate() {
                let ty = self.ty_of(col);
                let sc = self.scale(col);
                for r in [*i, *j] {
                    let Some(cell) = t.rows[r].cells.get(ci) else { continue };
                    let c = self.cell(lang, cell, &local(col), &ty, sc);
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
            let sp = lang.spelling();
            let joined = conds.join(sp.and);
            o.push_str(&format!(
                "{indent}{} {guard}: {}\n",
                sp.comment,
                tr!(
                    "W114（表 {name} 行{} × 行{}）。重ならないことを静的に証明できなかった行の対",
                    "W114 (table {name}, row {} × row {}): a pair of rows whose exclusivity could not be proven statically",
                    i + 1,
                    j + 1
                )
            ));
            o.push_str(&format!("{indent}{}\n", (sp.if_head)(&joined)));
            o.push_str(&raise(&name, i + 1, j + 1));
            if !sp.close.is_empty() {
                o.push_str(&format!("{indent}{}\n", sp.close));
            }
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

    /// A blank assignment for a value nothing downstream reads. The value is still computed,
    /// so that the generated code and the rule stay line for line, but Go will not compile a
    /// local that is never read — so say out loud that it is on purpose. W111 has already
    /// named the declaration.
    fn go_unread(&self, name: &str) -> String {
        if self.is_read(name) {
            String::new()
        } else {
            format!("\t_ = {}\n", self.ident(name))
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
            Ty::Number | Ty::Date => "int64".into(),
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
            // A cell that names one group calls the predicate the package already declares,
            // rather than writing the members out again — otherwise that function is dead.
            Cell::Lit(Lit::Word(w)) if self.c.groups.contains_key(w) => {
                format!("is{}({var})", pascal(&self.ident(w)))
            }
            Cell::Lit(Lit::Word(w)) if w == crate::kw::TRUE => var.to_string(),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::FALSE => format!("!{var}"),
            Cell::Lit(l) => format!("{var} == {}", lit(l)),
            Cell::Set(ls) => set(ls, false),
            Cell::Not(ls) if ls.len() == 1 && matches!(&ls[0], Lit::Word(w) if self.c.groups.contains_key(w)) => {
                let Lit::Word(w) = &ls[0] else { unreachable!() };
                format!("!is{}({var})", pascal(&self.ident(w)))
            }
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
            // A number gets no brand: it is a plain integer on purpose, and branding it
            // would shadow the language's own `int`.
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
            o.push_str(&format!(
                "func is{}(v {ty}) bool {{\n\tswitch v {{\n\tcase ",
                pascal(&self.ident(&g.name.text))
            ));
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
                    if matches!(self.ty_of(n), Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
                        format!("int64({f})")
                    } else {
                        f
                    }
                }
                None => self.ident(n),
            }
        };

        o.push_str(&tr!(
            "// {fname} は規則 {} v{} を評価する。分岐はもとの表の行と 1:1 に対応する。\n",
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
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date => {
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
                    o.push_str(&format!("\t{} := {}{CELL}// {}\n", self.ident(&d.name.text), go_expr(&e.text), tr!("導出", "derived value")));
                    o.push_str(&self.go_unread(&d.name.text));
                }
                Item::Define(d) => {
                    let e = self.expr(&d.expr, &local);
                    o.push_str(&format!("\t{} := {}{CELL}// {}\n", self.ident(&d.name.text), go_expr(&e.text), tr!("定義", "definition")));
                    o.push_str(&self.go_unread(&d.name.text));
                }
                Item::Table(t) => o.push_str(&self.go_table(t, &local)),
            }
        }

        // Every output takes the same three steps: its source — the `result` expression for
        // the first output, the binding of its own name otherwise — brought to the wire
        // scale, then its declared rounding applied exactly once (§15.16).
        let mut finals: Vec<String> = Vec::new();
        for (oi, od) in outs.iter().enumerate() {
            let out_name = &od.name.text;
            let res = match (&self.f.result, oi) {
                (Some(r), 0) => self.expr(&r.expr, &local),
                _ => Expr2 { text: local(out_name), scale: self.scale(out_name) },
            };
            let os = self.out_scale(out_name);
            let ty = self.ty_of(out_name);
            finals.push(match &od.rounding {
                Some(rd) => {
                    let g = crate::types::lit_value_in_pub(&rd.grid, &ty).unwrap_or(Rat::int(1));
                    let m = RoundMode::parse(&rd.mode).unwrap_or(RoundMode::Down);
                    let grid_i = g.num * res.scale / g.den;
                    if res.scale != os {
                        let raw = self.temp(&raw_base(oi));
                        o.push_str(&format!(
                            // The cast needs its own parentheses: an expression that begins with
                            // a call rather than a `(` glued itself to the type name.
                            "\t{raw} := int64({}){CELL}// {}\n",
                            go_expr(&res.text),
                            tr!("単位: 1/{} {}", "unit: 1/{} {}", res.scale, ty)
                        ));
                        format!("round{}({raw}, {}) / {}", pascal(mode_fn(m)), grid_i, res.scale / os)
                    } else {
                        format!("round{}(int64({}), {})", pascal(mode_fn(m)), go_expr(&res.text), grid_i)
                    }
                }
                // Only a number needs the widening cast; `int64(可否)` does not compile.
                None if !ty.is_numeric() => go_expr(&res.text),
                None => format!("int64({})", go_expr(&res.text)),
            });
        }
        if outs.len() == 1 {
            o.push_str(&format!("\treturn {ret}({}), nil\n}}\n", finals[0]));
        } else {
            // Each output gets its own rounding, applied exactly once (§7.2).
            let fields: Vec<String> = outs
                .iter()
                .zip(&finals)
                .map(|(od, body)| {
                    format!("{}: {}({body})", pascal(&pub_name(&od.name)), self.go_ty(&self.ty_of(&od.name.text)))
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
            let ty = if matches!(t2, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
                "int64".to_string()
            } else {
                self.go_ty(&t2)
            };
            o.push_str(&format!("\tvar {} {ty}\n", self.ident(&oc.name.text)));
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
                o.push_str(&format!("\t\t{} = {v}\n", self.ident(&oc.name.text)));
            }
        }
        o.push_str(&format!(
            "\t}} else {{\n\t\tpanic(\"{}\")\n\t}}\n",
            tr!("到達不能: 完全性は rulec が静的に検査済み", "unreachable: completeness was statically checked by rulec")
        ));
        for oc in &t.outputs {
            o.push_str(&self.go_unread(&oc.name.text));
        }
        o.push_str(&self.guards(t, local, Lang::Go, "\t", |name, i, j| {
            format!(
                "\t\treturn {}, fmt.Errorf(\"{}\")\n",
                if self.f.outputs.len() == 1 {
                    self.go_zero(&self.ty_of(&self.f.outputs[0].name.text))
                } else {
                    "Output{}".into()
                },
                tr!("表 {name}: 行{i} と 行{j} が同時に当てはまりました", "table {name}: row {i} and row {j} matched at the same time")
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
// Runners (§9.3: feed the vectors through every generated language and compare the answers)
// ---------------------------------------------------------------------------

impl<'a> Gen<'a> {
    /// A Python runner that reads JSONL from stdin and prints just the outputs, one record
    /// per line. It is also the worked example of the calling convention, so it constructs
    /// each branded argument the way a caller has to — `mypy --strict` checks both (§15.22).
    pub fn python_runner(&self) -> String {
        let alias = pub_name(&self.f.name);
        let mut args: Vec<String> = Vec::new();
        for i in &self.f.inputs {
            let ty = self.ty_of(&i.name.text);
            let jp = &i.name.text;
            args.push(match &ty {
                Ty::Enum(n) => {
                    let cls = self.enum_names.get(n).cloned().unwrap_or_default();
                    format!("m.{cls}(d[{jp:?}])")
                }
                Ty::Bool => format!("bool(d[{jp:?}])"),
                Ty::Date => format!("_ord(d[{jp:?}])"),
                Ty::Str => format!("str(d[{jp:?}])"),
                // A branded input has to be constructed, exactly as a caller must.
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate => {
                    format!("m.{}(int(d[{jp:?}]))", brand_of(&ty))
                }
                _ => format!("int(d[{jp:?}])"),
            });
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
             import enum\n\
             import json\n\
             import sys\n\
             from typing import Any\n\n\
             import {alias} as m\n\n\
             def _ord(s: str) -> int:\n    \
                 y, mo, d = (int(x) for x in s.split(\"-\"))\n    \
                 return (datetime.date(y, mo, d) - datetime.date(1970, 1, 1)).days\n\n\
             def _wire(v: Any) -> Any:\n    \
                 return v.value if isinstance(v, enum.Enum) else v\n\n\
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
                // A number is a plain int64, not a type the package declares, so it must not
                // be qualified with the package name.
                Ty::Number => format!("\t\tin.{g} = int64(num(d[{jp:?}]))\n"),
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
            format!("{:?}, {}", o.name.text, one("got", &self.ty_of(&o.name.text)))
        } else {
            self.f
                .outputs
                .iter()
                .map(|o| {
                    let ty = self.ty_of(&o.name.text);
                    format!(
                        "{:?}, {}",
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
             // The outputs in the order they are declared. encoding/json sorts the keys of a\n\
             // map, so a rule whose output names do not happen to sort that way disagreed\n\
             // with the other three languages on key order alone.\n\
             func obj(kv ...any) string {{\n\t\
                 s := \"{{\"\n\t\
                 for i := 0; i < len(kv); i += 2 {{\n\t\t\
                     if i > 0 {{\n\t\t\ts += \",\"\n\t\t}}\n\t\t\
                     k, _ := json.Marshal(kv[i])\n\t\t\
                     v, _ := json.Marshal(kv[i+1])\n\t\t\
                     s += string(k) + \":\" + string(v)\n\t}}\n\t\
                 return s + \"}}\"\n}}\n\n\
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
                     fmt.Println(obj({wire}))\n\t}}\n}}\n",
            env!("CARGO_PKG_VERSION"),
            fields.join("")
        )
    }
}


// ---------------------------------------------------------------------------
// TypeScript (§8.3)
// ---------------------------------------------------------------------------

/// Turn the shared expression text into TypeScript. The same post-processing shape as
/// `go_expr`: one grammar is emitted once and each language adjusts the spellings it does
/// not share.
///
/// Every integer becomes a `bigint` literal. The overflow proof (E108) is against int64, and
/// a JavaScript `number` is exact only to 2^53, so using one would put a silent wrong answer
/// above nine quadrillion into a tool whose whole claim is that it does not do that.
fn ts_expr(s: &str) -> String {
    let t = s
        .replace(" // ", " / ")
        .replace("True", "true")
        .replace("False", "false")
        .replace("_round_down(", "_roundDown(")
        .replace("_round_up(", "_roundUp(")
        .replace("_round_half(", "_roundHalf(")
        .replace("_round_bankers(", "_roundBankers(")
        .replace("_min(", "_min(")
        .replace("_max(", "_max(");
    bigint_literals(&t)
}

/// Append `n` to every integer literal, and only to those. A run of digits that touches a
/// letter, `_` or `.` on either side belongs to a name (`SizeClass.S60`, `項目1`) and is left
/// alone.
fn bigint_literals(s: &str) -> String {
    let b: Vec<char> = s.chars().collect();
    let part = |c: char| c.is_alphanumeric() || c == '_' || c == '.';
    let mut out = String::with_capacity(s.len() + 8);
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() && (i == 0 || !part(b[i - 1])) {
            let start = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            let touches_name = i < b.len() && part(b[i]);
            out.extend(&b[start..i]);
            if !touches_name {
                out.push('n');
            }
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

/// The four modes of §7.3 on `bigint`. `/` on a bigint truncates toward zero, the same as
/// Go's integer division, so these mirror the Go helpers rather than the Python ones.
fn round_ts() -> String {
    format!(
        r#"
export class RuleInputError extends Error {{
  constructor(message: string) {{
    super(message);
    this.name = "RuleInputError";
  }}
}}

export class RuleContradictionError extends Error {{
  constructor(message: string) {{
    super(message);
    this.name = "RuleContradictionError";
  }}
}}

function _min(a: bigint, b: bigint): bigint {{
  return a < b ? a : b;
}}

function _max(a: bigint, b: bigint): bigint {{
  return a > b ? a : b;
}}

/** {down} */
function _roundDown(x: bigint, g: bigint): bigint {{
  const a = x < 0n ? -x : x;
  const v = (a / g) * g;
  return x < 0n ? -v : v;
}}

/** {up} */
function _roundUp(x: bigint, g: bigint): bigint {{
  const a = x < 0n ? -x : x;
  const v = a % g === 0n ? (a / g) * g : (a / g + 1n) * g;
  return x < 0n ? -v : v;
}}

/** {half} */
function _roundHalf(x: bigint, g: bigint): bigint {{
  const a = x < 0n ? -x : x;
  const v = 2n * (a % g) >= g ? (a / g + 1n) * g : (a / g) * g;
  return x < 0n ? -v : v;
}}

/** {bankers} */
function _roundBankers(x: bigint, g: bigint): bigint {{
  const a = x < 0n ? -x : x;
  let q = a / g;
  const r = a % g;
  if (2n * r > g || (2n * r === g && q % 2n === 1n)) {{
    q += 1n;
  }}
  const v = q * g;
  return x < 0n ? -v : v;
}}
"#,
        down = tr!("0 へ寄せる。-4.8円 → -4円。", "Toward zero: -4.8 yen → -4 yen."),
        up = tr!("0 から遠ざける。-4.2円 → -5円。", "Away from zero: -4.2 yen → -5 yen."),
        half = tr!("半分ちょうどは 0 から遠ざける。", "An exact half goes away from zero."),
        bankers = tr!("半分ちょうどは偶数へ。", "An exact half goes to the even neighbor."),
    )
}

/// Which language a shared emitter is writing for. `guards` is the one body three languages
/// share, and it used to tell Python from Go by whether the indent was a tab — which quietly
/// handed TypeScript the Python spelling.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Py,
    Go,
    Ts,
    Rs,
    Rb,
}

/// The few spellings shared code needs to know per language. Keeping them in one row each
/// is what lets `guards()` be written once: it used to carry three separate `match`es, and
/// a sixth language meant finding all three (§15.20).
struct Spelling {
    /// How a line comment starts.
    comment: &'static str,
    /// How two conditions are joined.
    and: &'static str,
    /// The condition as an `if` head, without the indent.
    if_head: fn(&str) -> String,
    /// What closes the block, or "" for a language that closes by indentation.
    close: &'static str,
}

impl Lang {
    fn spelling(self) -> Spelling {
        match self {
            Lang::Py => Spelling {
                comment: "#",
                and: " and ",
                if_head: |c| format!("if {c}:"),
                close: "",
            },
            Lang::Rb => Spelling {
                comment: "#",
                and: " && ",
                if_head: |c| format!("if {c}"),
                close: "end",
            },
            Lang::Ts => Spelling {
                comment: "//",
                and: " && ",
                if_head: |c| format!("if ({c}) {{"),
                close: "}",
            },
            Lang::Go | Lang::Rs => Spelling {
                comment: "//",
                and: " && ",
                if_head: |c| format!("if {c} {{"),
                close: "}",
            },
        }
    }
}

impl<'a> Gen<'a> {
    /// The TypeScript type for a value. Numbers are branded `bigint`s: a brand costs nothing
    /// at runtime and still refuses `YenInclTax` where `YenExclTax` was meant.
    fn ts_ty(&self, ty: &Ty) -> String {
        match ty {
            Ty::Enum(n) => self.enum_names.get(n).cloned().unwrap_or_else(|| "string".into()),
            Ty::Bool => "boolean".into(),
            Ty::Str => "string".into(),
            Ty::Number | Ty::Date => "bigint".into(),
            Ty::Opt(t) => format!("{} | null", self.ts_ty(t)),
            _ => brand_of(ty),
        }
    }

    fn ts_value(&self, v: &str) -> String {
        match self.value_names.get(v) {
            Some((ty, alias)) => format!("{ty}.{}", alias.to_uppercase()),
            None => format!("{v:?}"),
        }
    }

    /// Render a cell as a TypeScript condition. A don't-care yields None (no condition).
    fn ts_cell(&self, cell: &Cell, var: &str, ty: &Ty, col_scale: i128) -> Option<String> {
        let inner = match ty {
            Ty::Opt(t) => t.as_ref(),
            other => other,
        };
        let numeric = inner.is_numeric() || matches!(inner, Ty::Date);
        let lit = |l: &Lit| -> String {
            match l {
                Lit::Word(w) if w == crate::kw::TRUE => "true".into(),
                Lit::Word(w) if w == crate::kw::FALSE => "false".into(),
                Lit::Word(w) => self.ts_value(w),
                Lit::Num(n) => format!("{}n", self.int_lit(n, inner, col_scale)),
                Lit::Date(y, m, d) => format!("{}n", crate::types::date_ord(*y, *m, *d).num),
                Lit::Str(s) => format!("{s:?}"),
            }
        };
        let members = |ls: &Vec<Lit>| -> String {
            let mut out: Vec<String> = Vec::new();
            for l in ls {
                if let Lit::Word(w) = l {
                    if let Some((_, ms)) = self.c.groups.get(w) {
                        out.extend(ms.iter().map(|m| self.ts_value(m)));
                        continue;
                    }
                }
                out.push(lit(l));
            }
            format!("[{}]", out.join(", "))
        };
        // `===` on a bigint and on a string are both value comparisons, so one spelling does
        // for every type here.
        Some(match cell {
            Cell::DontCare => return None,
            Cell::Nothing => format!("{var} === null"),
            Cell::Lit(Lit::Word(w)) if self.c.groups.contains_key(w) => {
                format!("_{}.has({var})", self.ident(w))
            }
            Cell::Lit(Lit::Word(w)) if w == crate::kw::TRUE => var.to_string(),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::FALSE => format!("!{var}"),
            Cell::Lit(l) => format!("{var} === {}", lit(l)),
            Cell::Set(ls) => format!("{}.includes({var})", members(ls)),
            Cell::Not(ls) if ls.len() == 1 && matches!(&ls[0], Lit::Word(w) if self.c.groups.contains_key(w)) => {
                let Lit::Word(w) = &ls[0] else { unreachable!() };
                format!("!_{}.has({var})", self.ident(w))
            }
            Cell::Not(ls) => format!("!{}.includes({var})", members(ls)),
            Cell::Cmp(cs) => cs
                .iter()
                .map(|(o, l)| {
                    let op = match o {
                        CmpOp::Le => "<=",
                        CmpOp::Ge => ">=",
                        CmpOp::Lt => "<",
                        CmpOp::Gt => ">",
                    };
                    let _ = numeric;
                    format!("{var} {op} {}", lit(l))
                })
                .collect::<Vec<_>>()
                .join(" && "),
        })
    }

    pub fn typescript(&self) -> String {
        let mut o = self.header("//");
        o.push('\n');

        // Brands. A branded bigint is still a bigint at runtime; the brand exists only for
        // the type checker, exactly as `NewType` does on the Python side.
        let mut brands: BTreeMap<String, String> = BTreeMap::new();
        for v in self.f.inputs.iter().map(|i| &i.name.text).chain(self.f.outputs.iter().map(|o| &o.name.text)) {
            let ty = self.ty_of(v);
            if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) {
                brands.insert(brand_of(&ty), format!("{ty}"));
            }
        }
        for (b, doc) in &brands {
            o.push_str(&format!("export type {b} = bigint & {{ readonly __rulec: \"{b}\" }}; // {doc}\n"));
        }
        if !brands.is_empty() {
            o.push('\n');
        }

        // The error classes come first: the enum parsers below throw them.
        o.push_str(&round_ts());
        o.push('\n');

        // Enums. A frozen object plus a union type, not `enum`: that keeps the file to
        // erasable syntax, so `node file.ts` runs it with no build step at all.
        let mut emitted: Vec<String> = Vec::new();
        for (jp, ascii) in &self.enum_names {
            if emitted.contains(ascii) {
                continue;
            }
            emitted.push(ascii.clone());
            let Some(vals) = self.c.enums.get(jp) else { continue };
            o.push_str(&format!("export const {ascii} = {{\n"));
            for v in vals {
                let name = self.value_names.get(v).map(|(_, a)| a.to_uppercase()).unwrap_or_else(|| v.clone());
                o.push_str(&format!("  {name}: {v:?},\n"));
            }
            o.push_str("} as const;\n");
            o.push_str(&format!(
                "export type {ascii} = (typeof {ascii})[keyof typeof {ascii}];\n\n"
            ));
            o.push_str(&format!(
                "export function parse{ascii}(s: string): {ascii} {{\n  \
                 const v = Object.values({ascii}).find((x) => x === s);\n  \
                 if (v === undefined) {{\n    \
                 throw new RuleInputError(`{}`);\n  }}\n  \
                 return v;\n}}\n\n",
                tr!("${{s}} は列挙 {ascii} の値ではありません", "${{s}} is not a value of enum {ascii}")
            ));
        }

        for g in &self.f.groups {
            let ms: Vec<String> = g.members.iter().map(|m| self.ts_value(&m.text)).collect();
            o.push_str(&format!(
                "const _{}: ReadonlySet<string> = new Set([{}]);\n",
                self.ident(&g.name.text),
                ms.join(", ")
            ));
        }
        if !self.f.groups.is_empty() {
            o.push('\n');
        }

        o.push_str(&self.ts_fn());
        o
    }

    fn ts_fn(&self) -> String {
        let fname = pub_name(&self.f.name);
        let params: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("{}: {}", pub_name(&i.name), self.ts_ty(&self.ty_of(&i.name.text))))
            .collect();
        let outs = &self.f.outputs;
        let ret = if outs.len() == 1 {
            self.ts_ty(&self.ty_of(&outs[0].name.text))
        } else {
            "Output".into()
        };
        let mut o = String::new();

        if outs.len() > 1 {
            o.push_str("export interface Output {\n");
            for od in outs {
                o.push_str(&format!(
                    "  {}: {};\n",
                    pub_name(&od.name),
                    self.ts_ty(&self.ty_of(&od.name.text))
                ));
            }
            o.push_str("}\n\n");
        }

        o.push_str(&tr!(
            "/** 規則 {} v{}。分岐はもとの表の行と 1:1 に対応する。 */\n",
            "/** Rule {} v{}. Each branch corresponds 1:1 to a row of the rule source. */\n",
            self.f.name.text,
            self.f.version
        ));
        o.push_str(&format!("export function {fname}({}): {ret} {{\n", params.join(", ")));

        let local = |n: &str| -> String { self.ident(n) };
        for i in &self.f.inputs {
            let v = pub_name(&i.name);
            let ty = self.ty_of(&i.name.text);
            match &ty {
                Ty::Enum(n) => {
                    let cls = self.enum_names.get(n).cloned().unwrap_or_default();
                    o.push_str(&format!(
                        "  if (!Object.values({cls}).includes({v})) {{\n    \
                         throw new RuleInputError(`{}`);\n  }}\n",
                        tr!(
                            "{} が列挙 {cls} の値ではありません: ${{{v}}}",
                            "{} is not a value of enum {cls}: ${{{v}}}",
                            i.name.text
                        )
                    ));
                }
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date => {
                    if let Some((Some(lo), Some(hi))) = self.c.ranges.get(&i.name.text).map(|(a, b)| (*a, *b)) {
                        let sc = self.c.wire_scale(&i.name.text);
                        o.push_str(&format!(
                            "  if ({v} < {}n || {v} > {}n) {{\n    throw new RuleInputError(`{}`);\n  }}\n",
                            crate::types::wire_int(lo, sc),
                            crate::types::wire_int(hi, sc),
                            tr!("{} が範囲の外です: ${{{v}}}", "{} is out of range: ${{{v}}}", i.name.text)
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
                    o.push_str(&format!(
                        "  const {} = {}; // {}\n",
                        self.ident(&d.name.text),
                        ts_expr(unparen(&e.text)),
                        tr!("導出", "derived value")
                    ));
                }
                Item::Define(d) => {
                    let e = self.expr(&d.expr, &local);
                    o.push_str(&format!(
                        "  const {} = {}; // {}\n",
                        self.ident(&d.name.text),
                        ts_expr(unparen(&e.text)),
                        tr!("定義", "definition")
                    ));
                }
                Item::Table(t) => o.push_str(&self.ts_table(t, &local)),
            }
        }

        let cast = |ty: &Ty, body: String| -> String {
            match ty {
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate => format!("({body}) as {}", self.ts_ty(ty)),
                _ => body,
            }
        };
        // Every output takes the same three steps: its source — the `result` expression for
        // the first output, the binding of its own name otherwise — brought to the wire
        // scale, then its declared rounding applied exactly once (§15.16).
        let mut finals: Vec<String> = Vec::new();
        for (oi, od) in outs.iter().enumerate() {
            let out_name = &od.name.text;
            let res = match (&self.f.result, oi) {
                (Some(r), 0) => self.expr(&r.expr, &local),
                _ => Expr2 { text: local(out_name), scale: self.scale(out_name) },
            };
            let os = self.out_scale(out_name);
            let ty = self.ty_of(out_name);
            let body = match &od.rounding {
                Some(rd) => {
                    let g = crate::types::lit_value_in_pub(&rd.grid, &ty).unwrap_or(Rat::int(1));
                    let m = RoundMode::parse(&rd.mode).unwrap_or(RoundMode::Down);
                    let grid_i = g.num * res.scale / g.den;
                    if res.scale != os {
                        let raw = self.temp(&raw_base(oi));
                        o.push_str(&format!(
                            "  const {raw} = {}; // {}\n",
                            ts_expr(unparen(&res.text)),
                            tr!("単位: 1/{} {}", "unit: 1/{} {}", res.scale, ty)
                        ));
                        format!("_round{}({raw}, {grid_i}n) / {}n", pascal(mode_fn(m)), res.scale / os)
                    } else {
                        format!("_round{}({}, {grid_i}n)", pascal(mode_fn(m)), ts_expr(&res.text))
                    }
                }
                None => ts_expr(&res.text),
            };
            finals.push(cast(&ty, body));
        }
        if outs.len() == 1 {
            o.push_str(&format!("  return {};\n}}\n", finals[0]));
        } else {
            let fields: Vec<String> = outs
                .iter()
                .zip(&finals)
                .map(|(od, body)| format!("{}: {body}", pub_name(&od.name)))
                .collect();
            o.push_str(&format!("  return {{ {} }};\n}}\n", fields.join(", ")));
        }
        o
    }

    fn ts_table(&self, t: &Table, local: &dyn Fn(&str) -> String) -> String {
        let name = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
        let policy = if t.policy == Policy::Unique { crate::kw::UNIQUE } else { crate::kw::FIRST };
        let mut o = format!("  // {}\n", tr!("表 {name}（{} {policy}）", "table {name} ({} {policy})", crate::kw::POLICY));
        // `let` up front: a binding made inside a branch does not leave it.
        for oc in &t.outputs {
            let ty = self.ty_of(&oc.name.text);
            let decl = match ty {
                Ty::Bool => "boolean".to_string(),
                Ty::Str => "string".to_string(),
                Ty::Enum(_) => self.ts_ty(&ty),
                _ => "bigint".to_string(),
            };
            o.push_str(&format!("  let {}: {decl};\n", self.ident(&oc.name.text)));
        }
        for (ri, row) in t.rows.iter().enumerate() {
            let conds: Vec<String> = t
                .inputs
                .iter()
                .enumerate()
                .filter_map(|(ci, (col, _))| {
                    let ty = self.ty_of(col);
                    self.ts_cell(row.cells.get(ci)?, &local(col), &ty, self.scale(col))
                })
                .collect();
            let cond = if conds.is_empty() { "true".into() } else { conds.join(" && ") };
            let cells: Vec<String> = row.cells.iter().map(cell_src).chain(row.outs.iter().map(out_src)).collect();
            let head = if ri == 0 { "  if" } else { " else if" };
            let line = tr!("行{}: {}", "row {}: {}", ri + 1, cells.join(" | "));
            if ri == 0 {
                o.push_str(&format!("{head} ({cond}) {{ // {line}\n"));
            } else {
                o.push_str(&format!("  }}{head} ({cond}) {{ // {line}\n"));
            }
            for (oi, oc) in t.outputs.iter().enumerate() {
                let v = match row.outs.get(oi) {
                    Some(OutCell::Lit(Lit::Num(n))) => {
                        let oty = self.ty_of(&oc.name.text);
                        format!("{}n", self.int_lit(n, &oty, self.scale(&oc.name.text)))
                    }
                    Some(OutCell::Lit(l)) => match l {
                        Lit::Word(w) if w == crate::kw::TRUE => "true".into(),
                        Lit::Word(w) if w == crate::kw::FALSE => "false".into(),
                        Lit::Word(w) => self.ts_value(w),
                        Lit::Date(y, m, d) => format!("{}n", crate::types::date_ord(*y, *m, *d).num),
                        _ => "0n".into(),
                    },
                    Some(OutCell::Name(w)) => {
                        if w == crate::kw::TRUE {
                            "true".into()
                        } else if w == crate::kw::FALSE {
                            "false".into()
                        } else if self.value_names.contains_key(w) {
                            self.ts_value(w)
                        } else {
                            ts_expr(&self.rescaled(w, &oc.name.text, local(w)))
                        }
                    }
                    None => "0n".into(),
                };
                o.push_str(&format!("    {} = {v};\n", self.ident(&oc.name.text)));
            }
        }
        o.push_str(&format!(
            "  }} else {{\n    throw new Error(\"{}\");\n  }}\n",
            tr!("到達不能: 完全性は rulec が静的に検査済み", "unreachable: completeness was statically checked by rulec")
        ));
        o.push_str(&self.guards(t, local, Lang::Ts, "  ", |name, i, j| {
            format!(
                "      throw new RuleContradictionError(\"{}\");\n",
                tr!("表 {name}: 行{i} と 行{j} が同時に当てはまりました", "table {name}: row {i} and row {j} matched at the same time")
            )
        }));
        o
    }

    pub fn ts_runner(&self) -> String {
        let alias = pub_name(&self.f.name);
        let mut args: Vec<String> = Vec::new();
        let mut imports: Vec<String> = vec![alias.clone()];
        // A brand is a type, and node's type stripping can only erase a whole `import type`
        // statement — a type name mixed into a value import is a syntax error there.
        let mut type_imports: Vec<String> = Vec::new();
        for i in &self.f.inputs {
            let ty = self.ty_of(&i.name.text);
            let jp = &i.name.text;
            args.push(match &ty {
                Ty::Enum(n) => {
                    let cls = self.enum_names.get(n).cloned().unwrap_or_default();
                    if !imports.contains(&format!("parse{cls}")) {
                        imports.push(format!("parse{cls}"));
                    }
                    format!("parse{cls}(String(d[{jp:?}]))")
                }
                Ty::Bool => format!("d[{jp:?}] === true"),
                Ty::Date => format!("_ord(String(d[{jp:?}]))"),
                Ty::Number => format!("BigInt(d[{jp:?}] as number)"),
                _ => {
                    let brand = self.ts_ty(&ty);
                    if !type_imports.contains(&brand) {
                        type_imports.push(brand.clone());
                    }
                    format!("BigInt(d[{jp:?}] as number) as {brand}")
                }
            });
        }
        // The wire format of §10.2, written out by hand: `JSON.stringify` refuses a bigint,
        // and turning one into a `number` first would round it above 2^53.
        let one = |expr: &str, ty: &Ty| match ty {
            Ty::Enum(_) | Ty::Str => format!("JSON.stringify({expr})"),
            Ty::Bool => format!("String({expr})"),
            _ => format!("String({expr})"),
        };
        let fields: Vec<String> = if self.f.outputs.len() == 1 {
            let od = &self.f.outputs[0];
            vec![format!(
                "{} + \":\" + {}",
                format!("{:?}", format!("{:?}", od.name.text)).replace("\\\"", "\\\""),
                one("r", &self.ty_of(&od.name.text))
            )]
        } else {
            self.f
                .outputs
                .iter()
                .map(|od| {
                    format!(
                        "{} + \":\" + {}",
                        format!("{:?}", format!("{:?}", od.name.text)).replace("\\\"", "\\\""),
                        one(&format!("r.{}", pub_name(&od.name)), &self.ty_of(&od.name.text))
                    )
                })
                .collect()
        };
        format!(
            "// Code generated by rulec {}. DO NOT EDIT.\n\
             import {{ readFileSync }} from \"node:fs\";\n\
             import {{ {} }} from \"./{alias}.ts\";\n{}\n\
             function _ord(s: string): bigint {{\n  \
                 const [y, m, d] = s.split(\"-\").map(Number);\n  \
                 return BigInt(Math.round(Date.UTC(y, m - 1, d) / 86400000));\n}}\n\n\
             const lines = readFileSync(0, \"utf8\").split(\"\\n\");\n\
             for (const line of lines) {{\n  \
                 if (line.trim() === \"\") {{\n    continue;\n  }}\n  \
                 const d = JSON.parse(line).in as Record<string, unknown>;\n  \
                 const r = {alias}({});\n  \
                 console.log(\"{{\" + [{}].join(\",\") + \"}}\");\n}}\n",
            env!("CARGO_PKG_VERSION"),
            imports.join(", "),
            if type_imports.is_empty() {
                String::new()
            } else {
                format!("import type {{ {} }} from \"./{alias}.ts\";\n", type_imports.join(", "))
            },
            args.join(", "),
            fields.join(", ")
        )
    }
}


// ---------------------------------------------------------------------------
// Rust (§8.3)
// ---------------------------------------------------------------------------

/// Turn the shared expression text into Rust. The same post-processing shape as `go_expr`
/// and `ts_expr`. Integer division truncates toward zero here, exactly as it does in Go and
/// on a JavaScript bigint, so the rounding helpers are the Go ones transliterated.
fn rs_expr(s: &str) -> String {
    s.replace(" // ", " / ")
        .replace("True", "true")
        .replace("False", "false")
        .replace("_round_down(", "round_down(")
        .replace("_round_up(", "round_up(")
        .replace("_round_half(", "round_half(")
        .replace("_round_bankers(", "round_bankers(")
        .replace("_min(", "min_i64(")
        .replace("_max(", "max_i64(")
}

/// The error type and the four modes of §7.3.
fn round_rs() -> String {
    format!(
        r#"
/// {err}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleError {{
    /// {input}
    Input(String),
    /// {contra}
    Contradiction(String),
}}

impl std::fmt::Display for RuleError {{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{
        match self {{
            RuleError::Input(m) | RuleError::Contradiction(m) => f.write_str(m),
        }}
    }}
}}

impl std::error::Error for RuleError {{}}

fn min_i64(a: i64, b: i64) -> i64 {{
    if a < b {{ a }} else {{ b }}
}}

fn max_i64(a: i64, b: i64) -> i64 {{
    if a > b {{ a }} else {{ b }}
}}

/// {down}
fn round_down(x: i64, g: i64) -> i64 {{
    let v = x.abs() / g * g;
    if x < 0 {{ -v }} else {{ v }}
}}

/// {up}
fn round_up(x: i64, g: i64) -> i64 {{
    let a = x.abs();
    let v = if a % g == 0 {{ a / g * g }} else {{ (a / g + 1) * g }};
    if x < 0 {{ -v }} else {{ v }}
}}

/// {half}
fn round_half(x: i64, g: i64) -> i64 {{
    let a = x.abs();
    let v = if 2 * (a % g) >= g {{ (a / g + 1) * g }} else {{ a / g * g }};
    if x < 0 {{ -v }} else {{ v }}
}}

/// {bankers}
fn round_bankers(x: i64, g: i64) -> i64 {{
    let a = x.abs();
    let (mut q, r) = (a / g, a % g);
    if 2 * r > g || (2 * r == g && q % 2 == 1) {{
        q += 1;
    }}
    let v = q * g;
    if x < 0 {{ -v }} else {{ v }}
}}
"#,
        err = tr!("この規則が返しうる誤り。", "Everything this rule can go wrong with."),
        input = tr!("宣言された入力域の外。呼び出し側の契約違反。", "Outside the declared input domain: a contract violation by the caller."),
        contra = tr!("規則そのものの矛盾。呼び出し側の誤りではない。", "A contradiction in the rule itself, not a mistake by the caller."),
        down = tr!("0 へ寄せる。-4.8円 → -4円。", "Toward zero: -4.8 yen → -4 yen."),
        up = tr!("0 から遠ざける。-4.2円 → -5円。", "Away from zero: -4.2 yen → -5 yen."),
        half = tr!("半分ちょうどは 0 から遠ざける。", "An exact half goes away from zero."),
        bankers = tr!("半分ちょうどは偶数へ。", "An exact half goes to the even neighbor."),
    )
}

impl<'a> Gen<'a> {
    /// The Rust type for a value. A unit is a newtype over `i64`: the compiler refuses
    /// `YenExclTax` where `YenInclTax` was meant, and it costs nothing at run time.
    fn rs_ty(&self, ty: &Ty) -> String {
        match ty {
            Ty::Enum(n) => self.enum_names.get(n).cloned().unwrap_or_else(|| "String".into()),
            Ty::Bool => "bool".into(),
            Ty::Str => "String".into(),
            Ty::Number | Ty::Date => "i64".into(),
            Ty::Opt(t) => format!("Option<{}>", self.rs_ty(t)),
            _ => brand_of(ty),
        }
    }

    fn rs_value(&self, v: &str) -> String {
        match self.value_names.get(v) {
            Some((ty, alias)) => format!("{ty}::{}", pascal(alias)),
            None => format!("{v:?}"),
        }
    }

    /// Render a cell as a Rust condition. A don't-care yields None (no condition).
    fn rs_cell(&self, cell: &Cell, var: &str, ty: &Ty, col_scale: i128) -> Option<String> {
        let inner = match ty {
            Ty::Opt(t) => t.as_ref(),
            other => other,
        };
        let lit = |l: &Lit| -> String {
            match l {
                Lit::Word(w) if w == crate::kw::TRUE => "true".into(),
                Lit::Word(w) if w == crate::kw::FALSE => "false".into(),
                Lit::Word(w) => self.rs_value(w),
                Lit::Num(n) => self.int_lit(n, inner, col_scale),
                Lit::Date(y, m, d) => format!("{}", crate::types::date_ord(*y, *m, *d).num),
                Lit::Str(s) => format!("{s:?}"),
            }
        };
        let any_of = |ls: &Vec<Lit>, neg: bool| -> String {
            let mut out: Vec<String> = Vec::new();
            for l in ls {
                if let Lit::Word(w) = l {
                    if let Some((_, ms)) = self.c.groups.get(w) {
                        out.extend(ms.iter().map(|m| self.rs_value(m)));
                        continue;
                    }
                }
                out.push(lit(l));
            }
            let (op, join) = if neg { ("!=", " && ") } else { ("==", " || ") };
            format!("({})", out.iter().map(|v| format!("{var} {op} {v}")).collect::<Vec<_>>().join(join))
        };
        Some(match cell {
            Cell::DontCare => return None,
            Cell::Nothing => format!("{var}.is_none()"),
            Cell::Lit(Lit::Word(w)) if self.c.groups.contains_key(w) => {
                format!("is_{}({var})", self.ident(w))
            }
            Cell::Lit(Lit::Word(w)) if w == crate::kw::TRUE => var.to_string(),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::FALSE => format!("!{var}"),
            Cell::Lit(l) => format!("{var} == {}", lit(l)),
            Cell::Set(ls) => any_of(ls, false),
            Cell::Not(ls) if ls.len() == 1 && matches!(&ls[0], Lit::Word(w) if self.c.groups.contains_key(w)) => {
                let Lit::Word(w) = &ls[0] else { unreachable!() };
                format!("!is_{}({var})", self.ident(w))
            }
            Cell::Not(ls) => any_of(ls, true),
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

    pub fn rust(&self) -> String {
        let mut o = self.header("//");
        o.push_str(
            "\n#![allow(non_snake_case, uncommon_codepoints, unused_parens)]\n\n",
        );

        // Brands: a newtype over i64, which is what the overflow proof is stated in.
        let mut brands: BTreeMap<String, String> = BTreeMap::new();
        for v in self.f.inputs.iter().map(|i| &i.name.text).chain(self.f.outputs.iter().map(|o| &o.name.text)) {
            let ty = self.ty_of(v);
            if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) {
                brands.insert(brand_of(&ty), format!("{ty}"));
            }
        }
        for (b, doc) in &brands {
            o.push_str(&format!(
                "/// {doc}\n#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]\npub struct {b}(pub i64);\n\n"
            ));
        }

        // Enums. Each carries its Japanese name, which is what the wire format uses (§10).
        let mut emitted: Vec<String> = Vec::new();
        for (jp, ascii) in &self.enum_names {
            if emitted.contains(ascii) {
                continue;
            }
            emitted.push(ascii.clone());
            let Some(vals) = self.c.enums.get(jp) else { continue };
            let members: Vec<String> = vals
                .iter()
                .map(|v| self.value_names.get(v).map(|(_, a)| pascal(a)).unwrap_or_else(|| v.clone()))
                .collect();
            o.push_str(&format!(
                "#[derive(Clone, Copy, PartialEq, Eq, Debug)]\npub enum {ascii} {{\n{}}}\n\n",
                members.iter().map(|m| format!("    {m},\n")).collect::<String>()
            ));
            o.push_str(&format!("impl {ascii} {{\n    pub fn as_str(self) -> &'static str {{\n        match self {{\n"));
            for (m, v) in members.iter().zip(vals) {
                o.push_str(&format!("            {ascii}::{m} => {v:?},\n"));
            }
            o.push_str("        }\n    }\n\n");
            o.push_str(&format!("    pub fn parse(s: &str) -> Option<{ascii}> {{\n        match s {{\n"));
            for (m, v) in members.iter().zip(vals) {
                o.push_str(&format!("            {v:?} => Some({ascii}::{m}),\n"));
            }
            o.push_str("            _ => None,\n        }\n    }\n}\n\n");
        }

        o.push_str(round_rs().trim_start_matches('\n'));
        o.push('\n');

        for g in &self.f.groups {
            let ty = self
                .c
                .groups
                .get(&g.name.text)
                .and_then(|(owner, _)| self.enum_names.get(owner).cloned())
                .unwrap_or_else(|| "i64".into());
            let ms: Vec<String> = g.members.iter().map(|m| self.rs_value(&m.text)).collect();
            o.push_str(&format!(
                "fn is_{}(v: {ty}) -> bool {{\n    matches!(v, {})\n}}\n\n",
                self.ident(&g.name.text),
                ms.join(" | ")
            ));
        }

        o.push_str(&self.rs_fn());
        o
    }

    fn rs_fn(&self) -> String {
        let fname = pub_name(&self.f.name);
        let outs = &self.f.outputs;
        let mut o = String::new();

        if outs.len() > 1 {
            o.push_str("#[derive(Clone, Copy, PartialEq, Eq, Debug)]\npub struct Output {\n");
            for od in outs {
                o.push_str(&format!(
                    "    pub {}: {},\n",
                    pub_name(&od.name),
                    self.rs_ty(&self.ty_of(&od.name.text))
                ));
            }
            o.push_str("}\n\n");
        }

        let ret = if outs.len() == 1 {
            self.rs_ty(&self.ty_of(&outs[0].name.text))
        } else {
            "Output".into()
        };
        let params: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| format!("{}: {}", pub_name(&i.name), self.rs_ty(&self.ty_of(&i.name.text))))
            .collect();

        o.push_str(&tr!(
            "/// 規則 {} v{}。分岐はもとの表の行と 1:1 に対応する。\n",
            "/// Rule {} v{}. Each branch corresponds 1:1 to a row of the rule source.\n",
            self.f.name.text,
            self.f.version
        ));
        o.push_str(&format!(
            "pub fn {fname}({}) -> Result<{ret}, RuleError> {{\n",
            params.join(", ")
        ));

        // A branded input is an i64 inside; unwrap it once, where it is read.
        let local = |n: &str| -> String {
            match self.f.inputs.iter().find(|i| i.name.text == n) {
                Some(i) => {
                    let v = pub_name(&i.name);
                    if matches!(self.ty_of(n), Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate) {
                        format!("{v}.0")
                    } else {
                        v
                    }
                }
                None => self.ident(n),
            }
        };
        // Entry guards. An enum needs none: in Rust a value of an enum type is one of its
        // variants by construction, so the check the other three languages have to make at
        // run time is already made by the compiler.
        for i in &self.f.inputs {
            let ty = self.ty_of(&i.name.text);
            if !matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date) {
                continue;
            }
            let Some((Some(lo), Some(hi))) = self.c.ranges.get(&i.name.text).map(|(a, b)| (*a, *b)) else {
                continue;
            };
            let sc = self.c.wire_scale(&i.name.text);
            let v = local(&i.name.text);
            // Rust's inline format arguments take a name, not a field access, so the value
            // goes in as a positional argument.
            o.push_str(&format!(
                "    if {v} < {} || {v} > {} {{\n        return Err(RuleError::Input(format!(\"{}\", {v})));\n    }}\n",
                crate::types::wire_int(lo, sc),
                crate::types::wire_int(hi, sc),
                tr!("{} が範囲の外です: {{}}", "{} is out of range: {{}}", i.name.text)
            ));
        }

        for it in &self.f.items {
            match it {
                Item::Derived(d) => o.push_str(&format!(
                    "    let {} = {}; // {}\n",
                    self.ident(&d.name.text),
                    rs_expr(unparen(&self.expr(&d.expr, &local).text)),
                    tr!("導出", "derived value")
                )),
                Item::Define(d) => o.push_str(&format!(
                    "    let {} = {}; // {}\n",
                    self.ident(&d.name.text),
                    rs_expr(unparen(&self.expr(&d.expr, &local).text)),
                    tr!("定義", "definition")
                )),
                Item::Table(t) => o.push_str(&self.rs_table(t, &local)),
            }
        }

        let wrap = |ty: &Ty, body: String| -> String {
            match ty {
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate => format!("{}({body})", self.rs_ty(ty)),
                _ => body,
            }
        };
        // Every output takes the same three steps: its source — the `result` expression for
        // the first output, the binding of its own name otherwise — brought to the wire
        // scale, then its declared rounding applied exactly once (§15.16).
        let mut finals: Vec<String> = Vec::new();
        for (oi, od) in outs.iter().enumerate() {
            let out_name = &od.name.text;
            let res = match (&self.f.result, oi) {
                (Some(r), 0) => self.expr(&r.expr, &local),
                _ => Expr2 { text: local(out_name), scale: self.scale(out_name) },
            };
            let os = self.out_scale(out_name);
            let ty = self.ty_of(out_name);
            let body = match &od.rounding {
                Some(rd) => {
                    let g = crate::types::lit_value_in_pub(&rd.grid, &ty).unwrap_or(Rat::int(1));
                    let m = RoundMode::parse(&rd.mode).unwrap_or(RoundMode::Down);
                    let grid_i = g.num * res.scale / g.den;
                    if res.scale != os {
                        let raw = self.temp(&raw_base(oi));
                        o.push_str(&format!(
                            "    let {raw} = {}; // {}\n",
                            rs_expr(unparen(&res.text)),
                            tr!("単位: 1/{} {}", "unit: 1/{} {}", res.scale, ty)
                        ));
                        format!("round_{}({raw}, {grid_i}) / {}", mode_fn(m), res.scale / os)
                    } else {
                        format!("round_{}({}, {grid_i})", mode_fn(m), rs_expr(&res.text))
                    }
                }
                None => rs_expr(&res.text),
            };
            finals.push(wrap(&ty, body));
        }
        if outs.len() == 1 {
            o.push_str(&format!("    Ok({})\n}}\n", finals[0]));
        } else {
            let fields: Vec<String> = outs
                .iter()
                .zip(&finals)
                .map(|(od, body)| format!("{}: {body}", pub_name(&od.name)))
                .collect();
            o.push_str(&format!("    Ok(Output {{ {} }})\n}}\n", fields.join(", ")));
        }
        o
    }

    fn rs_table(&self, t: &Table, local: &dyn Fn(&str) -> String) -> String {
        let name = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
        let policy = if t.policy == Policy::Unique { crate::kw::UNIQUE } else { crate::kw::FIRST };
        let mut o = format!("    // {}\n", tr!("表 {name}（{} {policy}）", "table {name} ({} {policy})", crate::kw::POLICY));
        for oc in &t.outputs {
            let ty = self.ty_of(&oc.name.text);
            let decl = match ty {
                Ty::Bool => "bool".to_string(),
                Ty::Str => "String".to_string(),
                Ty::Enum(_) => self.rs_ty(&ty),
                _ => "i64".to_string(),
            };
            o.push_str(&format!("    let {}: {decl};\n", self.ident(&oc.name.text)));
        }
        for (ri, row) in t.rows.iter().enumerate() {
            let conds: Vec<String> = t
                .inputs
                .iter()
                .enumerate()
                .filter_map(|(ci, (col, _))| {
                    let ty = self.ty_of(col);
                    self.rs_cell(row.cells.get(ci)?, &local(col), &ty, self.scale(col))
                })
                .collect();
            let cond = if conds.is_empty() { "true".into() } else { conds.join(" && ") };
            let cells: Vec<String> = row.cells.iter().map(cell_src).chain(row.outs.iter().map(out_src)).collect();
            let line = tr!("行{}: {}", "row {}: {}", ri + 1, cells.join(" | "));
            let head = if ri == 0 { "    if" } else { " else if" };
            if ri == 0 {
                o.push_str(&format!("{head} {cond} {{ // {line}\n"));
            } else {
                o.push_str(&format!("    }}{head} {cond} {{ // {line}\n"));
            }
            for (oi, oc) in t.outputs.iter().enumerate() {
                let v = match row.outs.get(oi) {
                    Some(OutCell::Lit(Lit::Num(n))) => {
                        let oty = self.ty_of(&oc.name.text);
                        self.int_lit(n, &oty, self.scale(&oc.name.text))
                    }
                    Some(OutCell::Lit(l)) => match l {
                        Lit::Word(w) if w == crate::kw::TRUE => "true".into(),
                        Lit::Word(w) if w == crate::kw::FALSE => "false".into(),
                        Lit::Word(w) => self.rs_value(w),
                        Lit::Date(y, m, d) => format!("{}", crate::types::date_ord(*y, *m, *d).num),
                        _ => "0".into(),
                    },
                    Some(OutCell::Name(w)) => {
                        if w == crate::kw::TRUE {
                            "true".into()
                        } else if w == crate::kw::FALSE {
                            "false".into()
                        } else if self.value_names.contains_key(w) {
                            self.rs_value(w)
                        } else {
                            rs_expr(&self.rescaled(w, &oc.name.text, local(w)))
                        }
                    }
                    None => "0".into(),
                };
                o.push_str(&format!("        {} = {v};\n", self.ident(&oc.name.text)));
            }
        }
        o.push_str(&format!(
            "    }} else {{\n        unreachable!(\"{}\");\n    }}\n",
            tr!("到達不能: 完全性は rulec が静的に検査済み", "unreachable: completeness was statically checked by rulec")
        ));
        o.push_str(&self.guards(t, local, Lang::Rs, "    ", |name, i, j| {
            format!(
                "        return Err(RuleError::Contradiction(\"{}\".into()));\n",
                tr!("表 {name}: 行{i} と 行{j} が同時に当てはまりました", "table {name}: row {i} and row {j} matched at the same time")
            )
        }));
        o
    }

    /// The runner. Rust has no JSON in its standard library, and the generated code takes no
    /// dependencies, so the reader below is written out here: the wire format is one flat
    /// object of numbers, strings and booleans (§10.2), which is small enough to scan.
    pub fn rs_runner(&self) -> String {
        let alias = pub_name(&self.f.name);
        let mut args: Vec<String> = Vec::new();
        for i in &self.f.inputs {
            let ty = self.ty_of(&i.name.text);
            let jp = &i.name.text;
            args.push(match &ty {
                Ty::Enum(n) => {
                    let cls = self.enum_names.get(n).cloned().unwrap_or_default();
                    format!("r::{cls}::parse(s(&d, {jp:?})).expect({jp:?})")
                }
                Ty::Bool => format!("b(&d, {jp:?})"),
                Ty::Date => format!("ord(s(&d, {jp:?}))"),
                Ty::Number => format!("n(&d, {jp:?})"),
                _ => format!("r::{}(n(&d, {jp:?}))", self.rs_ty(&ty)),
            });
        }
        let one = |expr: &str, ty: &Ty| match ty {
            Ty::Enum(_) => format!("q({expr}.as_str())"),
            Ty::Str => format!("q(&{expr})"),
            Ty::Bool => format!("{expr}.to_string()"),
            Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate => format!("{expr}.0.to_string()"),
            _ => format!("{expr}.to_string()"),
        };
        let fields: Vec<String> = if self.f.outputs.len() == 1 {
            let od = &self.f.outputs[0];
            vec![format!("q({:?}) + \":\" + &{}", od.name.text, one("got", &self.ty_of(&od.name.text)))]
        } else {
            self.f
                .outputs
                .iter()
                .map(|od| {
                    format!(
                        "q({:?}) + \":\" + &{}",
                        od.name.text,
                        one(&format!("got.{}", pub_name(&od.name)), &self.ty_of(&od.name.text))
                    )
                })
                .collect()
        };
        format!(
            r#"// Code generated by rulec {ver}. DO NOT EDIT.
#![allow(non_snake_case, uncommon_codepoints, unused_parens)]

#[path = "{alias}.rs"]
mod r;

use std::io::Read;

/// One flat JSON object as (key, value) pairs, values kept as their source text.
fn fields(line: &str) -> Vec<(String, String)> {{
    let b: Vec<char> = line.chars().collect();
    let (mut i, mut out) = (0usize, Vec::new());
    // Step into the object under "in" and read the pairs of the one level below it.
    while i < b.len() && !(b[i] == '"' && b[i..].starts_with(&['"', 'i', 'n', '"'])) {{
        i += 1;
    }}
    while i < b.len() && b[i] != '{{' {{
        i += 1;
    }}
    i += 1;
    while i < b.len() && b[i] != '}}' {{
        while i < b.len() && b[i] != '"' && b[i] != '}}' {{
            i += 1;
        }}
        if i >= b.len() || b[i] == '}}' {{
            break;
        }}
        let (k, ni) = string_at(&b, i);
        i = ni;
        while i < b.len() && b[i] != ':' {{
            i += 1;
        }}
        i += 1;
        while i < b.len() && b[i] == ' ' {{
            i += 1;
        }}
        let v = if b[i] == '"' {{
            let (v, ni) = string_at(&b, i);
            i = ni;
            v
        }} else {{
            let s = i;
            while i < b.len() && b[i] != ',' && b[i] != '}}' {{
                i += 1;
            }}
            b[s..i].iter().collect::<String>().trim().to_string()
        }};
        out.push((k, v));
    }}
    out
}}

/// The string starting at `i`, and the index just past its closing quote.
fn string_at(b: &[char], i: usize) -> (String, usize) {{
    let (mut i, mut s) = (i + 1, String::new());
    while i < b.len() && b[i] != '"' {{
        if b[i] == '\\' && i + 1 < b.len() {{
            i += 1;
            s.push(match b[i] {{
                'n' => '\n',
                't' => '\t',
                c => c,
            }});
        }} else {{
            s.push(b[i]);
        }}
        i += 1;
    }}
    (s, i + 1)
}}

fn get<'a>(d: &'a [(String, String)], k: &str) -> &'a str {{
    d.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str()).unwrap_or("")
}}

fn n(d: &[(String, String)], k: &str) -> i64 {{
    get(d, k).parse().unwrap_or(0)
}}

fn s<'a>(d: &'a [(String, String)], k: &str) -> &'a str {{
    get(d, k)
}}

fn b(d: &[(String, String)], k: &str) -> bool {{
    get(d, k) == "true"
}}

/// A date as its day number from 1970-01-01, by the same civil-date arithmetic the tool uses.
fn ord(s: &str) -> i64 {{
    let p: Vec<i64> = s.split('-').map(|x| x.parse().unwrap_or(0)).collect();
    let (y, m, d) = (p[0], p[1], p[2]);
    let y2 = if m <= 2 {{ y - 1 }} else {{ y }};
    let era = if y2 >= 0 {{ y2 }} else {{ y2 - 399 }} / 400;
    let yoe = y2 - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}}

/// A JSON string. The wire format keeps Japanese as itself (§10.2), so only the two
/// characters JSON requires are escaped.
fn q(s: &str) -> String {{
    format!("\"{{}}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}}

fn main() {{
    let mut src = String::new();
    std::io::stdin().read_to_string(&mut src).unwrap();
    for line in src.lines() {{
        if line.trim().is_empty() {{
            continue;
        }}
        let d = fields(line);
        let got = r::{alias}({args}).unwrap();
        println!("{{{{{{}}}}}}", [{fields}].join(","));
    }}
}}
"#,
            ver = env!("CARGO_PKG_VERSION"),
            args = args.join(", "),
            fields = fields.join(", ")
        )
    }
}

pub fn round_tests_rust() -> String {
    let mut o = format!(
        "// Code generated by rulec {}. DO NOT EDIT.\n// {}\n#![allow(unused_parens)]\n",
        env!("CARGO_PKG_VERSION"),
        tr!(
            "§7.3 の四モード。負の向きと半分ちょうどまで、Rust の参照実装と突き合わせる。",
            "The four modes of §7.3, checked against the Rust reference implementation down to negative values and exact halves."
        )
    );
    o.push_str(round_rs().trim_start_matches('\n'));
    o.push_str("\nconst CASES: &[(&str, i64, i64, i64)] = &[\n");
    for (m, x, g, want) in round_cases() {
        o.push_str(&format!("    (\"{}\", {x}, {g}, {want}),\n", mode_fn(m)));
    }
    o.push_str(
        "];\n\nfn main() {\n    let mut bad = 0;\n    for (mode, x, g, want) in CASES {\n        \
         let got = match *mode {\n            \"down\" => round_down(*x, *g),\n            \
         \"up\" => round_up(*x, *g),\n            \"half\" => round_half(*x, *g),\n            \
         _ => round_bankers(*x, *g),\n        };\n        if got != *want {\n            \
         println!(\"NG {mode}({x}, {g}) = {got}, want {want}\");\n            bad += 1;\n        \
         }\n    }\n    if bad > 0 {\n        std::process::exit(1);\n    }\n",
    );
    let line = tr!("ok {{}} 件", "ok {{}} cases");
    o.push_str(&format!("    println!(\"{line}\", CASES.len());\n}}\n"));
    o
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

pub fn round_tests_typescript() -> String {
    let mut o = format!(
        "// Code generated by rulec {}. DO NOT EDIT.\n// {}\n",
        env!("CARGO_PKG_VERSION"),
        tr!(
            "§7.3 の四モード。負の向きと半分ちょうどまで、Rust の参照実装と突き合わせる。",
            "The four modes of §7.3, checked against the Rust reference implementation down to negative values and exact halves."
        )
    );
    o.push_str(round_ts().trim_start_matches('\n'));
    o.push_str("\nconst CASES: [string, bigint, bigint, bigint][] = [\n");
    for (m, x, g, want) in round_cases() {
        o.push_str(&format!("  [\"{}\", {x}n, {g}n, {want}n],\n", mode_fn(m)));
    }
    o.push_str(
        "];\n\nconst FN: Record<string, (x: bigint, g: bigint) => bigint> = {\n  \
         down: _roundDown,\n  up: _roundUp,\n  half: _roundHalf,\n  bankers: _roundBankers,\n};\n\n\
         let bad = 0;\n\
         for (const [mode, x, g, want] of CASES) {\n  \
             const got = FN[mode](x, g);\n  \
             if (got !== want) {\n    \
                 console.log(`NG ${mode}(${x}, ${g}) = ${got}, want ${want}`);\n    \
                 bad += 1;\n  }\n}\n\
         if (bad > 0) {\n  process.exit(1);\n}\n",
    );
    o.push_str(&format!("console.log(`{}`);\n", tr!("ok ${{CASES.length}} 件", "ok ${{CASES.length}} cases")));
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
// test runs the generated code of every language against it.

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

        // --- TypeScript
        let ts_in: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| {
                let ty = self.ty_of(&i.name.text);
                self.value_json(&i.name.text, &pub_name(&i.name), &self.ts_ty(&ty), &ty)
            })
            .collect();
        let ts_outs: Vec<String> = outs
            .iter()
            .map(|od| {
                let ty = self.ty_of(&od.name.text);
                let v = self.value_json(&od.name.text, &pub_name(&od.name), &self.ts_ty(&ty), &ty);
                match self.rounding_json(od) {
                    Some(r) => format!("{},\"rounding\":{r}}}", v.trim_end_matches('}')),
                    None => v,
                }
            })
            .collect();
        let ts_ret = if outs.len() == 1 {
            self.ts_ty(&self.ty_of(&outs[0].name.text))
        } else {
            "Output".into()
        };
        let ts_sig = format!(
            "export function {alias}({}): {ts_ret}",
            self.f
                .inputs
                .iter()
                .map(|i| format!("{}: {}", pub_name(&i.name), self.ts_ty(&self.ty_of(&i.name.text))))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let typescript = crate::json::Obj::new()
            .str("module", &format!("{alias}.ts"))
            .str("function", &alias)
            .str("signature", &ts_sig)
            .raw("params", crate::json::arr(&ts_in))
            .str("returns", &ts_ret)
            .raw("outputs", crate::json::arr(&ts_outs))
            // A TypeScript enum member is the alias in upper case on a frozen object
            // (`CouponKind.PERCENT`), the same spelling Python uses.
            .raw("enums", self.enums_json(|_, a| a.to_uppercase()))
            .raw("errors", crate::json::strs(&["RuleInputError", "RuleContradictionError"]))
            .finish();

        // --- Rust
        let rs_in: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| {
                let ty = self.ty_of(&i.name.text);
                self.value_json(&i.name.text, &pub_name(&i.name), &self.rs_ty(&ty), &ty)
            })
            .collect();
        let rs_outs: Vec<String> = outs
            .iter()
            .map(|od| {
                let ty = self.ty_of(&od.name.text);
                let v = self.value_json(&od.name.text, &pub_name(&od.name), &self.rs_ty(&ty), &ty);
                match self.rounding_json(od) {
                    Some(r) => format!("{},\"rounding\":{r}}}", v.trim_end_matches('}')),
                    None => v,
                }
            })
            .collect();
        let rs_ret = if outs.len() == 1 {
            self.rs_ty(&self.ty_of(&outs[0].name.text))
        } else {
            "Output".into()
        };
        let rs_sig = format!(
            "pub fn {alias}({}) -> Result<{rs_ret}, RuleError>",
            self.f
                .inputs
                .iter()
                .map(|i| format!("{}: {}", pub_name(&i.name), self.rs_ty(&self.ty_of(&i.name.text))))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let rust = crate::json::Obj::new()
            .str("module", &format!("{alias}.rs"))
            .str("function", &alias)
            .str("signature", &rs_sig)
            .raw("params", crate::json::arr(&rs_in))
            .str("returns", &rs_ret)
            .raw("outputs", crate::json::arr(&rs_outs))
            // A Rust enum member is the alias in PascalCase on the enum's own type.
            .raw("enums", self.enums_json(|_, a| pascal(a)))
            .raw("errors", crate::json::strs(&["RuleError::Input", "RuleError::Contradiction"]))
            .finish();

        // --- Ruby. The unit is not in the type here (§15.20), so the entry states it
        // the way the others do and the generated comment repeats it for a reader.
        let rb_in: Vec<String> = self
            .f
            .inputs
            .iter()
            .map(|i| {
                let ty = self.ty_of(&i.name.text);
                self.value_json(&i.name.text, &pub_name(&i.name), &self.rb_api_ty(&ty), &ty)
            })
            .collect();
        let rb_outs: Vec<String> = outs
            .iter()
            .map(|od| {
                let ty = self.ty_of(&od.name.text);
                let v = self.value_json(&od.name.text, &pub_name(&od.name), &self.rb_api_ty(&ty), &ty);
                match self.rounding_json(od) {
                    Some(r) => format!("{},\"rounding\":{r}}}", v.trim_end_matches('}')),
                    None => v,
                }
            })
            .collect();
        let rb_ret = if outs.len() == 1 {
            self.rb_api_ty(&self.ty_of(&outs[0].name.text))
        } else {
            "Output".into()
        };
        let rb_sig = format!(
            "{}.{alias}({})",
            self.rb_module(),
            self.f.inputs.iter().map(|i| pub_name(&i.name)).collect::<Vec<_>>().join(", ")
        );
        let ruby = crate::json::Obj::new()
            .str("module", &self.rb_module())
            .str("function", &alias)
            .str("signature", &rb_sig)
            .raw("params", crate::json::arr(&rb_in))
            .str("returns", &rb_ret)
            .raw("outputs", crate::json::arr(&rb_outs))
            // A Ruby enum member is a constant under the type's module (`Band::SHORT`).
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
            .raw("typescript", typescript)
            .raw("rust", rust)
            .raw("ruby", ruby)
            .raw("go", go)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Ruby (§15.20). The closest relative of the Python backend: dynamically typed,
// and `/` on Integers rounds toward −∞ exactly as Python's `//` does. What is
// different is the constant rule — a Ruby constant must begin with an uppercase
// ASCII letter, so a Japanese name cannot be one and groups take a prefix.

/// Turn a name into something Ruby will accept as a constant.
fn rb_const(n: &str) -> String {
    if n.starts_with(|c: char| c.is_ascii_uppercase()) {
        n.to_string()
    } else {
        format!("C_{n}")
    }
}

fn rb_expr(s: &str) -> String {
    // `//` becomes `/`: Ruby's integer division already floors toward −∞.
    s.replace(" // ", " / ")
        .replace("True", "true")
        .replace("False", "false")
}

/// The four modes of §7.3, and the two comparisons. Nothing is left to Ruby's own
/// division: `abs` first, then the sign is carried back, so every mode means the
/// same thing it means in the other four.
fn round_rb() -> String {
    format!(
        r#"
  def self._min(a, b)
    a < b ? a : b
  end

  def self._max(a, b)
    a > b ? a : b
  end

  # {down}
  def self._round_down(x, g)
    v = x.abs / g * g
    x < 0 ? -v : v
  end

  # {up}
  def self._round_up(x, g)
    q, r = x.abs / g, x.abs % g
    v = r.zero? ? q * g : (q + 1) * g
    x < 0 ? -v : v
  end

  # {half}
  def self._round_half(x, g)
    q, r = x.abs / g, x.abs % g
    v = 2 * r >= g ? (q + 1) * g : q * g
    x < 0 ? -v : v
  end

  # {bankers}
  def self._round_bankers(x, g)
    q, r = x.abs / g, x.abs % g
    q += 1 if 2 * r > g || (2 * r == g && q.odd?)
    v = q * g
    x < 0 ? -v : v
  end

  private_class_method :_min, :_max, :_round_down, :_round_up, :_round_half, :_round_bankers
"#,
        down = tr!("0 へ寄せる。-4.8円 → -4円。", "Toward zero: -4.8 yen -> -4 yen."),
        up = tr!("0 から遠ざける。-4.2円 → -5円。", "Away from zero: -4.2 yen -> -5 yen."),
        half = tr!("半分ちょうどは 0 から遠ざける。", "An exact half goes away from zero."),
        bankers = tr!("半分ちょうどは偶数へ。", "An exact half goes to the even neighbor."),
    )
}

impl<'a> Gen<'a> {
    /// An enum value, written as the constant the module declares for it.
    fn rb_value(&self, v: &str) -> String {
        match self.value_names.get(v) {
            Some((ty, alias)) => format!("{}::{}", rb_const(ty), rb_const(&alias.to_uppercase())),
            None => format!("{v:?}"),
        }
    }

    /// Render a cell as a Ruby condition. A don't-care yields None (no condition).
    fn rb_cell(&self, cell: &Cell, var: &str, ty: &Ty, col_scale: i128) -> Option<String> {
        let inner = match ty {
            Ty::Opt(t) => t.as_ref(),
            other => other,
        };
        let lit = |l: &Lit| -> String {
            match l {
                Lit::Word(w) if w == crate::kw::TRUE => "true".into(),
                Lit::Word(w) if w == crate::kw::FALSE => "false".into(),
                Lit::Word(w) => self.rb_value(w),
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
                        out.extend(ms.iter().map(|m| self.rb_value(m)));
                        continue;
                    }
                }
                out.push(lit(l));
            }
            format!("[{}]", out.join(", "))
        };
        Some(match cell {
            Cell::DontCare => return None,
            Cell::Nothing => format!("{var}.nil?"),
            // A cell naming one group calls the constant the module already declares,
            // rather than writing the members out again.
            Cell::Lit(Lit::Word(w)) if self.c.groups.contains_key(w) => {
                format!("GROUP_{}.include?({var})", self.ident(w))
            }
            Cell::Lit(Lit::Word(w)) if w == crate::kw::TRUE => var.to_string(),
            Cell::Lit(Lit::Word(w)) if w == crate::kw::FALSE => format!("!{var}"),
            Cell::Lit(l) => format!("{var} == {}", lit(l)),
            Cell::Set(ls) => format!("{}.include?({var})", members(ls)),
            Cell::Not(ls) if ls.len() == 1 && matches!(&ls[0], Lit::Word(w) if self.c.groups.contains_key(w)) => {
                let Lit::Word(w) = &ls[0] else { unreachable!() };
                format!("!GROUP_{}.include?({var})", self.ident(w))
            }
            Cell::Not(ls) => format!("!{}.include?({var})", members(ls)),
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
}

impl<'a> Gen<'a> {
    /// The module name: the rule's ASCII alias in PascalCase, like the Go package.
    fn rb_module(&self) -> String {
        rb_const(&pascal(&pub_name(&self.f.name)))
    }

    /// How a value's type is written for a reader. Ruby has no zero-cost brand, so the
    /// unit is documented rather than enforced (§15.20); `Integer` is exact at any size.
    fn rb_doc_ty(&self, ty: &Ty) -> String {
        match ty {
            Ty::Enum(n) => rb_const(&self.enum_names.get(n).cloned().unwrap_or_else(|| "String".into())),
            Ty::Bool => "true/false".into(),
            Ty::Str => "String".into(),
            Ty::Opt(t) => format!("{} | nil", self.rb_doc_ty(t)),
            _ => format!("Integer  # {ty}"),
        }
    }

    pub fn ruby(&self) -> String {
        let mut o = self.header("#");
        o.insert_str(0, "# frozen_string_literal: true\n");
        o.push_str(&format!("module {} {}\n", self.rb_module(), "".trim()));
        // `module X` on its own line; the format above keeps it simple.
        o = o.replace(&format!("module {} \n", self.rb_module()), &format!("module {}\n", self.rb_module()));

        // Enums. A frozen constant per member, carrying the source name, which is also the
        // wire value (§10.2) — so the runner needs no conversion in either direction.
        let mut emitted: Vec<String> = Vec::new();
        for (jp, ascii) in &self.enum_names {
            if emitted.contains(ascii) {
                continue;
            }
            emitted.push(ascii.clone());
            let Some(vals) = self.c.enums.get(jp) else { continue };
            o.push_str(&format!("  module {}\n", rb_const(ascii)));
            let mut names: Vec<String> = Vec::new();
            for v in vals {
                let name = rb_const(
                    &self.value_names.get(v).map(|(_, a)| a.to_uppercase()).unwrap_or_else(|| v.clone()),
                );
                o.push_str(&format!("    {name} = {v:?}\n"));
                names.push(name);
            }
            o.push_str(&format!("    ALL = [{}].freeze\n  end\n\n", names.join(", ")));
        }

        o.push_str(&format!(
            "  # {}\n  class RuleInputError < ArgumentError; end\n\n",
            tr!("宣言された入力域の外。呼び出し側の契約違反。", "Outside the declared input domain: a contract violation by the caller.")
        ));
        o.push_str(&format!(
            "  # {}\n  class RuleContradictionError < RuntimeError; end\n\n",
            tr!("規則そのものの矛盾。呼び出し側の誤りではない。", "A contradiction in the rule itself, not a mistake by the caller.")
        ));

        // Groups. A Ruby constant has to begin with an uppercase ASCII letter, so a
        // Japanese group name cannot be one on its own; the prefix is what makes it legal.
        for g in &self.f.groups {
            let ms: Vec<String> = g.members.iter().map(|m| self.rb_value(&m.text)).collect();
            o.push_str(&format!("  GROUP_{} = [{}].freeze\n", self.ident(&g.name.text), ms.join(", ")));
        }
        if !self.f.groups.is_empty() {
            o.push('\n');
        }

        o.push_str(&round_rb());
        o.push('\n');
        o.push_str(&self.rb_fn());
        o.push_str("end\n");
        o
    }

    fn rb_fn(&self) -> String {
        let fname = pub_name(&self.f.name);
        let outs = &self.f.outputs;
        let mut o = String::new();

        // Multiple outputs come back as a Struct; one output is the value itself (§8.5).
        if outs.len() > 1 {
            let fs: Vec<String> = outs.iter().map(|od| format!(":{}", pub_name(&od.name))).collect();
            o.push_str(&format!("  Output = Struct.new({})\n\n", fs.join(", ")));
        }

        // The signature, with each parameter's declared type as a comment: this is where
        // the unit lives, since Ruby cannot hold it (§15.20).
        o.push_str(&format!(
            "  # {}\n",
            tr!("規則 {} v{}。分岐はもとの表の行と 1:1 に対応する。", "Rule {} v{}. Each branch corresponds 1:1 to a row of the rule source.", self.f.name.text, self.f.version)
        ));
        for i in &self.f.inputs {
            o.push_str(&format!(
                "  #   {} : {}\n",
                pub_name(&i.name),
                self.rb_doc_ty(&self.ty_of(&i.name.text)).replace("Integer  # ", "")
            ));
        }
        for od in outs {
            o.push_str(&format!(
                "  # -> {} : {}\n",
                pub_name(&od.name),
                self.rb_doc_ty(&self.ty_of(&od.name.text)).replace("Integer  # ", "")
            ));
        }

        let params: Vec<String> = self.f.inputs.iter().map(|i| pub_name(&i.name)).collect();
        o.push_str(&format!("  def self.{fname}({})\n", params.join(", ")));

        // Entry guards (§8.5).
        let local = |n: &str| -> String { self.ident(n) };
        for i in &self.f.inputs {
            let v = pub_name(&i.name);
            let ty = self.ty_of(&i.name.text);
            match &ty {
                Ty::Enum(n) => {
                    let cls = rb_const(&self.enum_names.get(n).cloned().unwrap_or_default());
                    o.push_str(&format!(
                        "    raise RuleInputError, \"{}\" unless {cls}::ALL.include?({v})\n",
                        tr!("{} が列挙 {} の値ではありません: #{{{v}.inspect}}", "{} is not a value of enum {}: #{{{v}.inspect}}", i.name.text, cls)
                    ));
                }
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Number | Ty::Date => {
                    if let Some((lo, hi)) = self.c.ranges.get(&i.name.text) {
                        if let (Some(lo), Some(hi)) = (lo, hi) {
                            let sc = self.c.wire_scale(&i.name.text);
                            o.push_str(&format!(
                                "    raise RuleInputError, \"{}\" unless ({}..{}).cover?({v})\n",
                                tr!("{} が範囲の外です: #{{{v}}}", "{} is out of range: #{{{v}}}", i.name.text),
                                crate::types::wire_int(*lo, sc),
                                crate::types::wire_int(*hi, sc),
                            ));
                        }
                    }
                }
                _ => {}
            }
        }

        for it in &self.f.items {
            match it {
                Item::Derived(d) => {
                    let e = self.expr(&d.expr, &local);
                    o.push_str(&format!("    {} = {}  # {}\n", self.ident(&d.name.text), rb_expr(unparen(&e.text)), tr!("導出", "derived value")));
                }
                Item::Define(d) => {
                    let e = self.expr(&d.expr, &local);
                    o.push_str(&format!("    {} = {}  # {}\n", self.ident(&d.name.text), rb_expr(unparen(&e.text)), tr!("定義", "definition")));
                }
                Item::Table(t) => o.push_str(&self.rb_table(t, &local)),
            }
        }

        // Every output: its source, brought to the wire scale, then its rounding once.
        let mut finals: Vec<String> = Vec::new();
        for (oi, od) in outs.iter().enumerate() {
            let out_name = &od.name.text;
            let res = match (&self.f.result, oi) {
                (Some(r), 0) => self.expr(&r.expr, &local),
                _ => Expr2 { text: local(out_name), scale: self.scale(out_name) },
            };
            let os = self.out_scale(out_name);
            let ty = self.ty_of(out_name);
            finals.push(match &od.rounding {
                Some(rd) => {
                    let g = crate::types::lit_value_in_pub(&rd.grid, &ty).unwrap_or(Rat::int(1));
                    let m = RoundMode::parse(&rd.mode).unwrap_or(RoundMode::Down);
                    let grid_i = g.num * res.scale / g.den;
                    if res.scale != os {
                        let raw = self.temp(&raw_base(oi));
                        o.push_str(&format!(
                            "    {raw} = {}  # {}\n",
                            rb_expr(unparen(&res.text)),
                            tr!("単位: 1/{} {}", "unit: 1/{} {}", res.scale, ty)
                        ));
                        format!("_round_{}({raw}, {}) / {}", mode_fn(m), grid_i, res.scale / os)
                    } else {
                        format!("_round_{}({}, {})", mode_fn(m), rb_expr(&res.text), grid_i)
                    }
                }
                None => rb_expr(&res.text),
            });
        }
        if outs.len() == 1 {
            o.push_str(&format!("    {}\n", finals[0]));
        } else {
            o.push_str(&format!("    Output.new({})\n", finals.join(", ")));
        }
        o.push_str("  end\n");
        o
    }

    fn rb_table(&self, t: &Table, local: &dyn Fn(&str) -> String) -> String {
        let name = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
        let policy = if t.policy == Policy::Unique { crate::kw::UNIQUE } else { crate::kw::FIRST };
        let mut o = format!("    # {}\n", tr!("表 {name}（{} {policy}）", "table {name} ({} {policy})", crate::kw::POLICY));
        for (ri, row) in t.rows.iter().enumerate() {
            let conds: Vec<String> = t
                .inputs
                .iter()
                .enumerate()
                .filter_map(|(ci, (col, _))| {
                    let ty = self.ty_of(col);
                    self.rb_cell(row.cells.get(ci)?, &local(col), &ty, self.scale(col))
                })
                .collect();
            let cond = if conds.is_empty() { "true".into() } else { conds.join(" && ") };
            let kw = if ri == 0 { "if" } else { "elsif" };
            let cells: Vec<String> = row.cells.iter().map(cell_src).chain(row.outs.iter().map(out_src)).collect();
            o.push_str(&format!("    {kw} {cond}  # {}\n", tr!("行{}: {}", "row {}: {}", ri + 1, cells.join(" | "))));
            for (oi, oc) in t.outputs.iter().enumerate() {
                let v = match row.outs.get(oi) {
                    Some(OutCell::Lit(Lit::Num(n))) => {
                        let ty = self.ty_of(&oc.name.text);
                        self.int_lit(n, &ty, self.scale(&oc.name.text))
                    }
                    Some(OutCell::Lit(l)) => match l {
                        Lit::Word(w) if w == crate::kw::TRUE => "true".into(),
                        Lit::Word(w) if w == crate::kw::FALSE => "false".into(),
                        Lit::Word(w) => self.rb_value(w),
                        Lit::Date(y, m, d) => format!("{}", crate::types::date_ord(*y, *m, *d).num),
                        _ => "0".into(),
                    },
                    Some(OutCell::Name(w)) => {
                        if w == crate::kw::TRUE {
                            "true".into()
                        } else if w == crate::kw::FALSE {
                            "false".into()
                        } else if self.value_names.contains_key(w) {
                            self.rb_value(w)
                        } else {
                            rb_expr(&self.rescaled(w, &oc.name.text, local(w)))
                        }
                    }
                    None => "0".into(),
                };
                o.push_str(&format!("      {} = {v}\n", self.ident(&oc.name.text)));
            }
        }
        o.push_str(&format!(
            "    else\n      raise RuleContradictionError, \"{}\"\n    end\n",
            tr!("到達不能: 完全性は rulec が静的に検査済み", "unreachable: completeness was statically checked by rulec")
        ));
        o.push_str(&self.guards(t, local, Lang::Rb, "    ", |name, i, j| {
            format!(
                "      raise RuleContradictionError, \"{}\"\n",
                tr!("表 {name}: 行{i} と 行{j} が同時に当てはまりました", "table {name}: row {i} and row {j} matched at the same time")
            )
        }));
        o
    }
}

impl<'a> Gen<'a> {
    /// A Ruby runner that reads JSONL from stdin and prints just the outputs, one record
    /// per line. `json` and `date` are both standard library, so this needs no gem.
    pub fn ruby_runner(&self) -> String {
        let alias = pub_name(&self.f.name);
        let m = self.rb_module();
        let mut args: Vec<String> = Vec::new();
        for i in &self.f.inputs {
            let jp = &i.name.text;
            args.push(match &self.ty_of(&i.name.text) {
                // An enum member is the source string, which is what the wire carries.
                Ty::Enum(_) | Ty::Str => format!("d[{jp:?}]"),
                Ty::Bool => format!("d[{jp:?}] ? true : false"),
                Ty::Date => format!("_ord(d[{jp:?}])"),
                _ => format!("d[{jp:?}].to_i"),
            });
        }
        let dump = if self.f.outputs.len() == 1 {
            format!("{{ {:?} => r }}", self.f.outputs[0].name.text)
        } else {
            let fs: Vec<String> = self
                .f
                .outputs
                .iter()
                .map(|o| format!("{:?} => r.{}", o.name.text, pub_name(&o.name)))
                .collect();
            format!("{{ {} }}", fs.join(", "))
        };
        format!(
            "# Code generated by rulec {}. DO NOT EDIT.\n\
             # frozen_string_literal: true\n\n\
             require \"date\"\n\
             require \"json\"\n\
             require_relative \"{alias}\"\n\n\
             def _ord(s)\n  \
               Date.iso8601(s).jd - Date.new(1970, 1, 1).jd\n\
             end\n\n\
             STDIN.each_line do |line|\n  \
               line = line.strip\n  \
               next if line.empty?\n  \
               d = JSON.parse(line)[\"in\"]\n  \
               r = {m}.{alias}({})\n  \
               puts JSON.generate({dump})\n\
             end\n",
            env!("CARGO_PKG_VERSION"),
            args.join(", ")
        )
    }
}

/// The four modes of §7.3 in Ruby, checked against the same reference cases the other
/// others are checked against. `rulec test` runs it beside the generated module.
pub fn round_tests_ruby() -> String {
    let mut o = format!(
        "# Code generated by rulec {}. DO NOT EDIT.\n\
         # frozen_string_literal: true\n\
         # {}\n\n\
         module R\n",
        env!("CARGO_PKG_VERSION"),
        tr!(
            "§7.3 の四モード。負の向きと半分ちょうどまで、Rust の参照実装と突き合わせる。",
            "The four modes of §7.3, checked against the Rust reference implementation down to negative values and exact halves."
        )
    );
    o.push_str(round_rb().trim_start_matches('\n'));
    o.push_str("  public_class_method :_round_down, :_round_up, :_round_half, :_round_bankers\nend\n\nCASES = [\n");
    for (m, x, g, want) in round_cases() {
        o.push_str(&format!("  [{:?}, {x}, {g}, {want}],\n", mode_fn(m)));
    }
    o.push_str("].freeze\n\nbad = 0\nCASES.each do |mode, x, g, want|\n  ");
    o.push_str("got = R.send(\"_round_#{mode}\", x, g)\n  next if got == want\n  ");
    o.push_str("puts \"NG #{mode}(#{x}, #{g}) = #{got}, want #{want}\"\n  bad += 1\nend\n");
    o.push_str("exit(1) unless bad.zero?\n");
    o.push_str(&format!("puts \"{}\"\n", tr!("ok #{{CASES.size}} 件", "ok #{{CASES.size}} cases")));
    o
}

impl<'a> Gen<'a> {
    /// The type as the inventory states it for Ruby. There are no brands here (§15.20), so
    /// a number is `Integer` and the unit travels in the entry's own `unit` field.
    fn rb_api_ty(&self, ty: &Ty) -> String {
        match ty {
            Ty::Enum(n) => rb_const(&self.enum_names.get(n).cloned().unwrap_or_else(|| "String".into())),
            Ty::Bool => "Boolean".into(),
            Ty::Str => "String".into(),
            Ty::Opt(t) => format!("{} | nil", self.rb_api_ty(t)),
            _ => "Integer".into(),
        }
    }
}

impl<'a> Gen<'a> {
    /// A cell as a condition in one language. The one place that maps a `Lang` to its cell
    /// renderer, so that shared code never has to know the set (§15.20).
    fn cell(&self, lang: Lang, cell: &Cell, var: &str, ty: &Ty, scale: i128) -> Option<String> {
        match lang {
            Lang::Py => self.py_cell(cell, var, ty, scale),
            Lang::Go => self.go_cell(cell, var, ty, scale),
            Lang::Ts => self.ts_cell(cell, var, ty, scale),
            Lang::Rs => self.rs_cell(cell, var, ty, scale),
            Lang::Rb => self.rb_cell(cell, var, ty, scale),
        }
    }
}
