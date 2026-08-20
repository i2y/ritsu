//! コード生成（§8）。
//!
//! 生成物の可読性は §8.1 の五つで定義される。とくに「表の 1 行が分岐 1 本」と
//! 「先行分岐で真とわかる条件も消さない」は、`.rule` の行と生成物の行を目で
//! 対応させるための約束なので、賢い最適化をしない。

use crate::ast::*;
use crate::num::{Rat, RoundMode};
use crate::types::{Checked, Ty};
use std::collections::BTreeMap;

/// 単位から ASCII の brand 名を作る。単位は閉じた集合なので固定表でよい。
fn brand_of(ty: &Ty) -> String {
    match ty {
        Ty::Money { cur, tax } => {
            let c = match cur.as_str() {
                "円" => "Yen",
                "銭" => "Sen",
                other => other,
            };
            let t = match tax.as_deref() {
                Some("税込") => "InclTax",
                Some("税抜") => "ExclTax",
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

/// `member_kind` → `MemberKind`。
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

/// 公開面は ASCII 別名、内部は日本語のまま（§8.1）。
fn pub_name(n: &Name) -> String {
    n.ascii.clone().unwrap_or_else(|| n.text.clone())
}

pub struct Gen<'a> {
    f: &'a RuleFile,
    c: &'a Checked,
    /// 表名 → W114 の行対。番人はここにだけ入る。
    w114: BTreeMap<String, Vec<(usize, usize)>>,
    /// 表の出力列 → 格納スケール。率の列は刻みを宣言しないので、その列の
    /// リテラルの分母の最小公倍数で決める。列ごとに一つに固定しないと、
    /// 同じ列の 50% と 100% が別のスケールで出て値が壊れる。
    col_scales: BTreeMap<String, i128>,
    /// 型名 → その列挙の ASCII 別名（Pascal）。
    enum_names: BTreeMap<String, String>,
    /// 列挙値 → (型名, ASCII 別名)。
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
                    if let Some(OutCell::Lit(Lit::Num(n))) = row.outs.get(oi) {
                        if let Some(v) = crate::types::lit_value_in_pub(n, &ty) {
                            sc = lcm(sc, v.den);
                        }
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

    /// 値の格納スケール（1/k の倍数で持つ、その k）。§7.1。
    fn scale(&self, n: &str) -> i128 {
        if let Some(s) = self.col_scales.get(n) {
            return *s;
        }
        *self.c.scales.get(n).unwrap_or(&1)
    }

    /// 型と列のスケールに合わせた整数リテラル。
    fn int_lit(&self, n: &crate::lex::Num, ty: &Ty, scale: i128) -> String {
        let v = crate::types::lit_value_in_pub(n, ty).unwrap_or(Rat::zero());
        format!("{}", v.mul(Rat::int(scale)).num)
    }

    fn header(&self, comment: &str) -> String {
        format!(
            "{comment} Code generated by rulec {}. DO NOT EDIT.\n\
             {comment} 原本: {} (規則 {} v{}, sha256:{})\n",
            env!("CARGO_PKG_VERSION"),
            self.f.name.text,
            self.f.name.text,
            self.f.version,
            &self.src_hash[..12]
        )
    }
}

/// 依存を増やしたくないので、決定的な短いハッシュを自前で持つ（FNV-1a 128）。
fn hash(s: &str) -> String {
    let mut h: u128 = 0x6c62272e07bb014262b821756295c58d;
    for b in s.as_bytes() {
        h ^= *b as u128;
        h = h.wrapping_mul(0x0000000001000000000000000000013b);
    }
    format!("{h:032x}")
}

// ---------------------------------------------------------------------------
// 式
// ---------------------------------------------------------------------------

/// 式を、そのスケールとともに文字列にする。両言語で同じ整数演算になる。
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
            Expr::Lit(Lit::Word(w), _) if w == "真" || w == "偽" => {
                Expr2 { text: (if w == "真" { "True" } else { "False" }).into(), scale: 1 }
            }
            Expr::Lit(..) => Expr2 { text: "0".into(), scale: 1 },
            Expr::Call(name, args, _) => {
                let a: Vec<Expr2> = args.iter().map(|x| self.expr(x, local)).collect();
                match (name.as_str(), a.as_slice()) {
                    ("最小", [x, y]) | ("最大", [x, y]) => {
                        let s = lcm(x.scale, y.scale);
                        let f = if name == "最小" { "_min" } else { "_max" };
                        Expr2 {
                            text: format!("{f}({}, {})", rescale(x, s), rescale(y, s)),
                            scale: s,
                        }
                    }
                    (m, [x, g]) if RoundMode::parse(m).is_some() => {
                        // 丸めは格子の倍数へ寄せる。格子は x と同じスケールに揃える。
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

    /// セルを Python の条件式にする。don't-care は None（条件なし）。
    fn py_cell(&self, cell: &Cell, var: &str, ty: &Ty, col_scale: i128) -> Option<String> {
        let inner = match ty {
            Ty::Opt(t) => t.as_ref(),
            other => other,
        };
        let lit = |l: &Lit| -> String {
            match l {
                Lit::Word(w) if w == "真" => "True".into(),
                Lit::Word(w) if w == "偽" => "False".into(),
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
            Cell::Lit(Lit::Word(w)) if w == "真" => var.to_string(),
            Cell::Lit(Lit::Word(w)) if w == "偽" => format!("not {var}"),
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
        // typing の取り込みは使うものだけ。複数出力のときだけ NamedTuple が要る。
        let typing = if self.f.outputs.len() > 1 { "NamedTuple, NewType" } else { "NewType" };
        o.push_str(&format!("from __future__ import annotations\n\nimport enum\nfrom typing import {typing}\n\n"));

        // brand。mypy と pyright に効き、実行時コストは無い。
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

        // 列挙。値に和名を持たせてログとワイヤ形式に使う（§10）。
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

        o.push_str("class RuleInputError(ValueError):\n    \"\"\"宣言された入力域の外。呼び出し側の契約違反。\"\"\"\n\n");
        o.push_str("class RuleContradictionError(AssertionError):\n    \"\"\"規則そのものの矛盾。呼び出し側の誤りではない。\"\"\"\n\n");

        // 群
        for g in &self.f.groups {
            let ms: Vec<String> = g.members.iter().map(|m| self.py_value(&m.text)).collect();
            o.push_str(&format!("_{} = frozenset({{{}}})\n", g.name.text, ms.join(", ")));
        }
        if !self.f.groups.is_empty() {
            o.push('\n');
        }

        o.push_str(ROUND_PY);
        o.push_str(&self.py_fn());
        pep8_blanks(&o)
    }
}

/// §7.3 の四モード。負の向きと半分ちょうどまで仕様どおりに固定する。
/// 言語の素の除算に任せない（Python の `//` は −∞ 方向、Go は 0 方向）。
const ROUND_PY: &str = r#"
# 生成コードは組み込みを裸で呼ばない。入力の ASCII 別名が `min` や `list` の
# ような名前でも壊れないようにするため（衝突の族ごと消す）。
_isinstance = isinstance


def _min(a: int, b: int) -> int:
    return a if a < b else b


def _max(a: int, b: int) -> int:
    return a if a > b else b


def _round_down(x: int, g: int) -> int:
    """0 へ寄せる。-4.8円 → -4円。"""
    q, r = abs(x) // g, abs(x) % g
    v = q * g
    return -v if x < 0 else v


def _round_up(x: int, g: int) -> int:
    """0 から遠ざける。-4.2円 → -5円。"""
    q, r = abs(x) // g, abs(x) % g
    v = (q + 1) * g if r else q * g
    return -v if x < 0 else v


def _round_half(x: int, g: int) -> int:
    """半分ちょうどは 0 から遠ざける。"""
    q, r = abs(x) // g, abs(x) % g
    v = (q + 1) * g if 2 * r >= g else q * g
    return -v if x < 0 else v


def _round_bankers(x: int, g: int) -> int:
    """半分ちょうどは偶数へ。"""
    q, r = abs(x) // g, abs(x) % g
    if 2 * r > g or (2 * r == g and q % 2 == 1):
        q += 1
    v = q * g
    return -v if x < 0 else v

"#;

impl<'a> Gen<'a> {
    /// 型の Python 注釈。公開面は brand、列挙はクラス。
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

    /// 出力の格納スケール。宣言単位の整数一本（§7.1）。
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

        // 複数出力は NamedTuple（§8.5）。
        if outs.len() > 1 {
            o.push_str("class Output(NamedTuple):\n");
            for od in outs {
                o.push_str(&format!("    {}: {}\n", pub_name(&od.name), self.py_ty(&self.ty_of(&od.name.text))));
            }
            o.push('\n');
        }

        o.push_str(&format!("def {fname}({}) -> {ret}:\n", params.join(", ")));
        o.push_str(&format!(
            "    \"\"\"規則 {} v{}。分岐は原本の行と 1:1 に対応する。\"\"\"\n",
            self.f.name.text, self.f.version
        ));

        // 入口ガード（§8.5）。証明の前提（入力が宣言域内）を実行時に守る。
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
                    "    if not _isinstance({v}, {}):\n        raise RuleInputError(f\"{} が列挙 {} の値ではありません: {{{v}!r}}\")\n",
                    self.py_ty(&ty), i.name.text, self.py_ty(&ty)
                )),
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate => {
                    if let Some((lo, hi)) = self.c.ranges.get(&i.name.text) {
                        if let (Some(lo), Some(hi)) = (lo, hi) {
                            o.push_str(&format!(
                                "    if not {} <= {v} <= {}:\n        raise RuleInputError(f\"{} が範囲の外です: {{{v}}}\")\n",
                                lo.num, hi.num, i.name.text
                            ));
                        }
                    }
                }
                _ => {}
            }
        }

        // 導出と定義（宣言順。§5.1 の define-before-use）。
        for it in &self.f.items {
            match it {
                Item::Derived(d) => {
                    let e = self.expr(&d.expr, &local);
                    o.push_str(&format!("    {} = {}  # 導出\n", d.name.text, unparen(&e.text)));
                }
                Item::Define(d) => {
                    let e = self.expr(&d.expr, &local);
                    o.push_str(&format!("    {} = {}  # 定義\n", d.name.text, unparen(&e.text)));
                }
                Item::Table(t) => o.push_str(&self.py_table(t, &local)),
            }
        }

        // 結果と最終の丸め。
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
                // 式を入れ子にせず中間へ束ねる（§8.2 の例と同じ形）。読みやすさが
                // 基準 1 の要求で、深い入れ子は生成物の目視対応を壊す。
                if res.scale != os {
                    o.push_str(&format!("    raw = {}  # 単位: 1/{} {}\n", unparen(&res.text), res.scale, ty));
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
            // 出力ごとに、その出力の丸めが一度だけ掛かる（§7.2）。
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
        let policy = if t.policy == Policy::Unique { "一意" } else { "上から" };
        let mut o = format!("    # 表 {name}（方式 {policy}）\n");
        for (ri, row) in t.rows.iter().enumerate() {
            // §8.1 基準 1: 全セルを省略せずに書く。先行分岐で真とわかる条件も消さない。
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
            o.push_str(&format!("    {kw} {cond}:  # 行{}: {}\n", ri + 1, cells.join(" | ")));
            for (oi, oc) in t.outputs.iter().enumerate() {
                let v = match row.outs.get(oi) {
                    Some(OutCell::Lit(Lit::Num(n))) => {
                        let ty = self.ty_of(&oc.name.text);
                        self.int_lit(n, &ty, self.scale(&oc.name.text))
                    }
                    Some(OutCell::Lit(l)) => match l {
                        Lit::Word(w) if w == "真" => "True".into(),
                        Lit::Word(w) if w == "偽" => "False".into(),
                        Lit::Word(w) => self.py_value(w),
                        Lit::Date(y, m, d) => format!("{}", crate::types::date_ord(*y, *m, *d).num),
                        _ => "0".into(),
                    },
                    Some(OutCell::Name(w)) => {
                        if w == "真" {
                            "True".into()
                        } else if w == "偽" {
                            "False".into()
                        } else if self.value_names.contains_key(w) {
                            self.py_value(w)
                        } else {
                            local(w)
                        }
                    }
                    None => "0".into(),
                };
                o.push_str(&format!("        {} = {v}\n", oc.name.text));
            }
        }
        o.push_str("    else:\n        raise AssertionError(\"到達不能: 完全性は rulec が静的に検査済み\")\n");
        o.push_str(&self.guards(t, local, "    ", |name, i, j| {
            format!("        raise RuleContradictionError(\"表 {name}: 行{i} と 行{j} が同時に当たりました\")\n")
        }));
        o
    }

    /// 番人（§8.1）。排他を静的に証明できなかった行対にだけ入る。
    /// この分岐を踏む入力はベクタに存在しない（構成できたなら E105 になっている）。
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
                "{indent}// 番人: W114（表 {name} 行{} × 行{}）。排他を静的に証明できなかった行対\n",
                i + 1,
                j + 1
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
            o = o.replace("// 番人", "# 番人");
        }
        o
    }
}

/// 診断と生成コメントで使う、セルの原文表記。
fn cell_src(c: &Cell) -> String {
    match c {
        Cell::DontCare => "-".into(),
        Cell::Nothing => "無し".into(),
        Cell::Lit(l) => lit_src(l),
        Cell::Set(ls) => ls.iter().map(lit_src).collect::<Vec<_>>().join(" ・ "),
        Cell::Not(ls) => format!("以外: {}", ls.iter().map(lit_src).collect::<Vec<_>>().join(" ・ ")),
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
                Lit::Word(w) if w == "真" => "true".into(),
                Lit::Word(w) if w == "偽" => "false".into(),
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
            Cell::Lit(Lit::Word(w)) if w == "真" => var.to_string(),
            Cell::Lit(Lit::Word(w)) if w == "偽" => format!("!{var}"),
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
            // defined type（alias ではない）。YenInclTax + YenExclTax はコンパイラが弾く。
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
            // §10 のワイヤ形式は和名なので、そこから戻す口を持たせる。
            o.push_str(&format!("func Parse{ascii}(s string) ({ascii}, bool) {{\n\tswitch s {{\n"));
            for v in vals {
                o.push_str(&format!("\tcase {:?}:\n\t\treturn {}, true\n", v, self.go_value(v)));
            }
            o.push_str(&format!("\t}}\n\treturn {}(0), false\n}}\n\n", ascii));
        }

        // 群。非公開なので日本語識別子のままでよい（§8.1）。
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

        o.push_str(ROUND_GO);
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

        o.push_str("// Input は入力をまとめて受ける。同型の int が並ぶのを避けるため（§8.3）。\ntype Input struct {\n");
        for i in &self.f.inputs {
            let doc = match self.c.ranges.get(&i.name.text) {
                Some((Some(lo), Some(hi))) => format!("// {} 範囲 {}..{}", i.name.text, lo.num, hi.num),
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
        let zero = if outs.len() == 1 { "0".to_string() } else { "Output{}".to_string() };

        // Go の defined type は混ぜて計算できない。brand は Input と戻り値という
        // 公開面で守り、内部の算術は int64 一本で通す（§8.3 の非対称の裏返し）。
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

        o.push_str(&format!(
            "// {fname} は規則 {} v{} を評価する。分岐は原本の行と 1:1 に対応する。\n",
            self.f.name.text, self.f.version
        ));
        o.push_str(&format!("func {fname}(in Input) ({ret}, error) {{\n"));

        for i in &self.f.inputs {
            let v = local(&i.name.text);
            let ty = self.ty_of(&i.name.text);
            match &ty {
                Ty::Enum(_) => o.push_str(&format!(
                    "\tif !{v}.Valid() {{\n\t\treturn {zero}, fmt.Errorf(\"{} が列挙の値ではありません: %d\", {v})\n\t}}\n",
                    i.name.text
                )),
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate => {
                    if let Some((Some(lo), Some(hi))) = self.c.ranges.get(&i.name.text) {
                        o.push_str(&format!(
                            "\tif {v} < {} || {v} > {} {{\n\t\treturn {zero}, fmt.Errorf(\"{} が範囲の外です: %d\", {v})\n\t}}\n",
                            lo.num, hi.num, i.name.text
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
                    o.push_str(&format!("\t{} := {}{CELL}// 導出\n", d.name.text, go_expr(&e.text)));
                }
                Item::Define(d) => {
                    let e = self.expr(&d.expr, &local);
                    o.push_str(&format!("\t{} := {}{CELL}// 定義\n", d.name.text, go_expr(&e.text)));
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
                    o.push_str(&format!("\traw := int64{}{CELL}// 単位: 1/{} {}\n", go_expr(&res.text), res.scale, ty));
                    format!("round{}(raw, {}) / {}", pascal(mode_fn(m)), grid_i, res.scale / os)
                } else {
                    format!("round{}(int64({}), {})", pascal(mode_fn(m)), go_expr(&res.text), grid_i)
                }
            }
            None => format!("int64({})", go_expr(&res.text)),
        };
        if outs.len() == 1 {
            o.push_str(&format!("\treturn {ret}({text}), nil\n}}\n"));
        } else {
            // 出力ごとに、その出力の丸めが一度だけ掛かる（§7.2）。
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
        let policy = if t.policy == Policy::Unique { "一意" } else { "上から" };
        let mut o = format!("\t// 表 {name}（方式 {policy}）\n");
        // Go は if の中で宣言した変数が外に出ないので、先に宣言しておく。
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
                o.push_str(&format!("{kw} {cond} {{ // 行1: {}\n", cells.join(" | ")));
            } else {
                o.push_str(&format!("\t}}{kw} {cond} {{ // 行{}: {}\n", ri + 1, cells.join(" | ")));
            }
            for (oi, oc) in t.outputs.iter().enumerate() {
                let v = match row.outs.get(oi) {
                    Some(OutCell::Lit(Lit::Num(n))) => {
                        let ty = self.ty_of(&oc.name.text);
                        self.int_lit(n, &ty, self.scale(&oc.name.text))
                    }
                    Some(OutCell::Lit(l)) => match l {
                        Lit::Word(w) if w == "真" => "true".into(),
                        Lit::Word(w) if w == "偽" => "false".into(),
                        Lit::Word(w) => self.go_value(w),
                        Lit::Date(y, m, d) => format!("{}", crate::types::date_ord(*y, *m, *d).num),
                        _ => "0".into(),
                    },
                    Some(OutCell::Name(w)) => {
                        if w == "真" {
                            "true".into()
                        } else if w == "偽" {
                            "false".into()
                        } else if self.value_names.contains_key(w) {
                            self.go_value(w)
                        } else {
                            local(w)
                        }
                    }
                    None => "0".into(),
                };
                o.push_str(&format!("\t\t{} = {v}\n", oc.name.text));
            }
        }
        o.push_str("\t} else {\n\t\tpanic(\"到達不能: 完全性は rulec が静的に検査済み\")\n\t}\n");
        o.push_str(&self.guards(t, local, "\t", |name, i, j| {
            format!(
                "\t\treturn {}, fmt.Errorf(\"表 {name}: 行{i} と 行{j} が同時に当たりました\")\n",
                if self.f.outputs.len() == 1 { "0" } else { "Output{}" }
            )
        }));
        o
    }
}

/// Python 向けに作った式を Go の字面へ寄せる。整数除算だけが違う。
fn go_expr(s: &str) -> String {
    s.replace(" // ", " / ")
        .replace("True", "true")
        .replace("False", "false")
        .replace("_round_down(", "roundDown(")
        .replace("_round_up(", "roundUp(")
        .replace("_round_half(", "roundHalf(")
        .replace("_round_bankers(", "roundBankers(")
        .replace("_min(", "minInt(")
        .replace("_max(", "maxInt(")
}

const ROUND_GO: &str = r#"// §7.3 の四モード。負の向きと半分ちょうどまで仕様どおりに固定する。
// 言語の素の除算に任せない（Python の // は −∞ 方向、Go は 0 方向）。

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

/// gofmt の tabwriter と同じ列合わせを自前で持つ。
///
/// gofmt を後段で走らせると、生成物が「その環境に入っている gofmt」に依存して
/// §8.5 の決定性が壊れる。整列は宣言の並びの中で閉じるので、自前で足りる。
/// 区切りに使う制御文字は、同じ列数の連続する行をひと塊として揃えるための印。
const CELL: char = '\u{1f}';
const IMPORT_MARK: &str = "\u{1e}IMPORTS\u{1e}";

/// 列の幅はルーン数で測る。Go の text/tabwriter がそう数えるので、表示幅
/// （全角を 2 と数える `diag::width`）で測ると和名の識別子が並んだところで
/// gofmt と一文字ずれる。§8.5 は「gofmt -l が空」を要求しているので、
/// 見た目の理屈ではなく gofmt の理屈に合わせる。
fn cells_wide(s: &str) -> usize {
    s.chars().count()
}

/// 式全体を包む丸括弧を落とす。生成側は部分式に一律で括弧を付けているが、
/// 代入の右辺では最外の一組が余る。Python の整形器はこれを消したがるので、
/// `ruff format --check` を緑に保つには生成の時点で落としておく（§8.5 の
/// 「整形を後段に頼らない」を Python 側でも守る）。Go は gofmt が消さないので
/// そのまま。
fn unparen(s: &str) -> &str {
    let t = s.trim();
    let Some(inner) = t.strip_prefix('(').and_then(|x| x.strip_suffix(')')) else { return s };
    // 最外の括弧が本当に対応しているときだけ落とす。`(a) + (b)` は落とさない。
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

/// PEP 8 の空行（E301〜E305）を一箇所で整える。出力の組み立ては各所に散っていて、
/// 足す場所を一つ忘れるだけで整形器が赤くなる。書く側で気を付けるのをやめて、
/// 最後に一度通す。トップレベルの `class` / `def` / デコレータの前後は空行 2 行、
/// それ以外のトップレベル文の前は 1 行以下。
fn pep8_blanks(src: &str) -> String {
    let lines: Vec<&str> = src.trim_end().split('\n').collect();
    let mut out: Vec<String> = Vec::new();
    let mut in_block = false; // 直前のトップレベル構文が def / class の本体だったか
    for line in lines {
        let top = !line.is_empty() && !line.starts_with(char::is_whitespace);
        if line.trim().is_empty() {
            continue; // 空行は捨てて、必要なところで入れ直す
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
    // ファイル先頭の空行は落とす。
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
// ランナー（§9.3: ベクタを両言語に流して三者一致を見る）
// ---------------------------------------------------------------------------

impl<'a> Gen<'a> {
    /// JSONL を stdin から読み、1 行 1 件で出力だけを返す Python ランナー。
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

    /// 同じことをする Go ランナー。encoding/json は標準ライブラリ。
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
        // ワイヤは和名（§10）。列挙は数値ではなく名前で出す。
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
// 丸めヘルパの単体ベクタ（§8.5）
// ---------------------------------------------------------------------------

/// 四モード × 格子 × 値。負値と半分ちょうどを必ず含む。
/// 表レベルの一致だけでは、端数の出ない表でヘルパの誤りが隠れる。
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
         # §7.3 の四モード。負の向きと半分ちょうどまで、Rust の参照実装と突き合わせる。\n\
         import sys\n\n",
        env!("CARGO_PKG_VERSION")
    );
    o.push_str(ROUND_PY.trim_start_matches('\n'));
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
             bad += 1\nif bad:\n    sys.exit(1)\nprint(f\"ok {len(CASES)} 件\")\n",
    );
    o
}

pub fn round_tests_go(pkg: &str) -> String {
    let mut o = format!(
        "// Code generated by rulec {}. DO NOT EDIT.\n\
         // §7.3 の四モード。負の向きと半分ちょうどまで、Rust の参照実装と突き合わせる。\n\
         package {pkg}\n\nimport \"testing\"\n\n\
         func TestRoundingModes(t *testing.T) {{\n\t\
         cases := []struct {{\n\t\tmode{CELL}string\n\t\tx, g, want{CELL}int64\n\t}}{{\n",
        env!("CARGO_PKG_VERSION")
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
