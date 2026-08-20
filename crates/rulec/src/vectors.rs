//! 境界からのテストケース生成（§9）。
//!
//! 候補値を列ごとに決め、そこから作った母集団を参照評価器に通して、
//! 行被覆・境界両側被覆・遮蔽対被覆を満たす部分集合を決定的に選ぶ。
//! 期待値と発火行は評価器が付ける。

use crate::ast::*;
use crate::eval::{self, Val};
use crate::num::Rat;
use crate::types::{Checked, Ty};
use std::collections::{BTreeMap, BTreeSet};

pub struct Vector {
    pub input: BTreeMap<String, Val>,
    pub output: Option<Val>,
    pub trace: Vec<String>,
    pub why: String,
}

/// 列ごとの候補値（§9.1）。
fn candidates(f: &RuleFile, c: &Checked) -> BTreeMap<String, Vec<Val>> {
    let mut out: BTreeMap<String, Vec<Val>> = BTreeMap::new();
    for i in &f.inputs {
        let name = &i.name.text;
        let ty = c.ty_of(name).unwrap_or(Ty::Unknown);
        let inner = match &ty {
            Ty::Opt(t) => (**t).clone(),
            other => other.clone(),
        };
        let mut vs: Vec<Val> = Vec::new();
        if matches!(ty, Ty::Opt(_)) {
            // optional は「無し」と、在る側の候補（§9.1）。
            vs.push(Val::Enum("無し".into()));
        }
        match &inner {
            Ty::Enum(en) => {
                // 表が区別している区分だけを踏む。47 都道府県を全部舐めない。
                let all = c.enums.get(en).cloned().unwrap_or_default();
                let named = named_values(f, name, c);
                for v in &all {
                    if named.contains(v) {
                        vs.push(Val::Enum(v.clone()));
                    }
                }
                // どのセルも名指ししていない値の代表を一つ（「以外」の側）。
                if let Some(v) = all.iter().find(|v| !named.contains(*v)) {
                    vs.push(Val::Enum(v.clone()));
                }
                if vs.is_empty() {
                    if let Some(v) = all.first() {
                        vs.push(Val::Enum(v.clone()));
                    }
                }
            }
            Ty::Bool => {
                vs.push(Val::Bool(true));
                vs.push(Val::Bool(false));
            }
            Ty::Date => {
                let q = Rat::int(1);
                for b in numeric_bounds(f, name, &inner, c) {
                    for d in [-1i128, 0, 1] {
                        let v = b.add(q.mul(Rat::int(d)));
                        let (y, m, dd) = crate::types::ord_to_date(v);
                        vs.push(Val::Date(y, m, dd));
                    }
                }
            }
            Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate => {
                let q = match &inner {
                    Ty::Rate => Rat::new(1, *c.scales.get(name).unwrap_or(&100)),
                    _ => Rat::int(1),
                };
                for b in numeric_bounds(f, name, &inner, c) {
                    for d in [-1i128, 0, 1] {
                        vs.push(Val::Num(b.add(q.mul(Rat::int(d)))));
                    }
                }
            }
            _ => {}
        }
        // 宣言範囲の外は落とす。入口ガードが弾くので、域内だけが三者比較の対象（§8.5）。
        if let Some((lo, hi)) = c.ranges.get(name) {
            vs.retain(|v| match v {
                Val::Num(x) => {
                    lo.is_none_or(|l| x.cmp_to(l) != std::cmp::Ordering::Less)
                        && hi.is_none_or(|h| x.cmp_to(h) != std::cmp::Ordering::Greater)
                }
                Val::Date(y, m, d) => {
                    let x = crate::types::date_ord(*y, *m, *d);
                    lo.is_none_or(|l| x.cmp_to(l) != std::cmp::Ordering::Less)
                        && hi.is_none_or(|h| x.cmp_to(h) != std::cmp::Ordering::Greater)
                }
                _ => true,
            });
        }
        vs.dedup_by(|a, b| a == b);
        if vs.is_empty() {
            vs.push(default_val(&inner, c));
        }
        out.insert(name.clone(), vs);
    }
    out
}

fn default_val(ty: &Ty, c: &Checked) -> Val {
    match ty {
        Ty::Enum(en) => Val::Enum(c.enums.get(en).and_then(|v| v.first()).cloned().unwrap_or_default()),
        Ty::Bool => Val::Bool(true),
        Ty::Date => Val::Date(2026, 1, 1),
        _ => Val::Num(Rat::zero()),
    }
}

/// その列のセルが名指ししている列挙値（群は展開する）。
fn named_values(f: &RuleFile, col: &str, c: &Checked) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for it in &f.items {
        let Item::Table(t) = it else { continue };
        let Some(ci) = t.inputs.iter().position(|(n, _)| n == col) else { continue };
        for row in &t.rows {
            let Some(cell) = row.cells.get(ci) else { continue };
            let ls: Vec<&Lit> = match cell {
                Cell::Lit(l) => vec![l],
                Cell::Set(ls) | Cell::Not(ls) => ls.iter().collect(),
                _ => vec![],
            };
            for l in ls {
                if let Lit::Word(w) = l {
                    match c.groups.get(w) {
                        Some((_, ms)) => out.extend(ms.iter().cloned()),
                        None => {
                            out.insert(w.clone());
                        }
                    }
                }
            }
        }
    }
    out
}

/// その列のセルと宣言範囲に現れる境界値。
fn numeric_bounds(f: &RuleFile, col: &str, ty: &Ty, c: &Checked) -> Vec<Rat> {
    let mut set: BTreeSet<(i128, i128)> = BTreeSet::new();
    let mut push = |r: Rat, s: &mut BTreeSet<(i128, i128)>| {
        s.insert((r.num, r.den));
    };
    if let Some((lo, hi)) = c.ranges.get(col) {
        for b in [lo, hi].into_iter().flatten() {
            push(*b, &mut set);
        }
    }
    let lit = |l: &Lit| -> Option<Rat> {
        match l {
            Lit::Num(n) => crate::types::lit_value_in_pub(n, ty),
            Lit::Date(y, m, d) => Some(crate::types::date_ord(*y, *m, *d)),
            _ => None,
        }
    };
    for it in &f.items {
        match it {
            Item::Table(t) => {
                if let Some(ci) = t.inputs.iter().position(|(n, _)| n == col) {
                    for row in &t.rows {
                        match row.cells.get(ci) {
                            Some(Cell::Cmp(cs)) => {
                                for (_, l) in cs {
                                    if let Some(v) = lit(l) {
                                        push(v, &mut set);
                                    }
                                }
                            }
                            Some(Cell::Lit(l)) => {
                                if let Some(v) = lit(l) {
                                    push(v, &mut set);
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            // §9.1: 解析は真偽定義を自由軸として見るので、原子の境界はここで拾う。
            // 解析に見えない境界こそ、ベクタが踏まないと誰も踏まない。
            Item::Define(d) => atom_bounds(&d.expr, col, ty, &mut set),
            Item::Derived(_) => {}
        }
    }
    let mut v: Vec<Rat> = set.into_iter().map(|(n, d)| Rat { num: n, den: d }).collect();
    v.sort_by(|a, b| a.cmp_to(*b));
    v
}

fn atom_bounds(e: &Expr, col: &str, ty: &Ty, set: &mut BTreeSet<(i128, i128)>) {
    match e {
        Expr::Bin(l, op, r, _) if matches!(op, BinOp::Le | BinOp::Lt | BinOp::Ge | BinOp::Gt | BinOp::Eq) => {
            let mut take = |name: &Expr, lit: &Expr| {
                if let (Expr::Name(n, _), Expr::Lit(li, _)) = (name, lit) {
                    if n == col {
                        let v = match li {
                            Lit::Num(x) => crate::types::lit_value_in_pub(x, ty),
                            Lit::Date(y, m, d) => Some(crate::types::date_ord(*y, *m, *d)),
                            _ => None,
                        };
                        if let Some(v) = v {
                            set.insert((v.num, v.den));
                        }
                    }
                }
            };
            take(l, r);
            take(r, l);
        }
        Expr::Bin(l, _, r, _) => {
            atom_bounds(l, col, ty, set);
            atom_bounds(r, col, ty, set);
        }
        _ => {}
    }
}

/// セルを満たす最初の候補値。§9.2 の行被覆は、これで行を狙い撃つ。
fn satisfying(cell: &Cell, cands: &[Val], ty: &Ty, c: &Checked) -> Option<Val> {
    let env = eval::Env { vals: BTreeMap::new().into_iter().collect(), c, fired: Vec::new() };
    cands.iter().find(|v| env.matches_pub(cell, v, ty)).cloned()
}

/// 母集団を作る。狙い撃ち → 一列ずつの振り → 二列の組合せ、の順に足す。
fn pool(f: &RuleFile, c: &Checked, cands: &BTreeMap<String, Vec<Val>>) -> Vec<(BTreeMap<String, Val>, String)> {
    let names: Vec<String> = f.inputs.iter().map(|i| i.name.text.clone()).collect();
    let base: BTreeMap<String, Val> = names
        .iter()
        .map(|n| (n.clone(), cands[n].first().cloned().unwrap()))
        .collect();
    let mut out: Vec<(BTreeMap<String, Val>, String)> = vec![(base.clone(), "基準".into())];

    // 行被覆：各行のセルを満たす値を置く。
    for it in &f.items {
        let Item::Table(t) = it else { continue };
        let tname = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
        for (ri, row) in t.rows.iter().enumerate() {
            let mut a = base.clone();
            for (ci, (col, _)) in t.inputs.iter().enumerate() {
                if !cands.contains_key(col) {
                    continue; // 導出や中間値の列は入力から間接に決まる
                }
                let Some(cell) = row.cells.get(ci) else { continue };
                let ty = c.ty_of(col).unwrap_or(Ty::Unknown);
                if let Some(v) = satisfying(cell, &cands[col], &ty, c) {
                    a.insert(col.clone(), v);
                }
            }
            out.push((a, format!("行狙い: 表 {tname} 行{}", ri + 1)));
        }
    }

    // 境界両側：一列ずつ、全候補へ振る。候補は境界の ±刻み で作ってある。
    for n in &names {
        for v in &cands[n] {
            let mut a = base.clone();
            a.insert(n.clone(), v.clone());
            out.push((a, format!("境界: {n}")));
        }
    }

    // ペアワイズ：二列の組合せを貪欲に足す（§9.2 の保険）。
    let mut seen: BTreeSet<(String, String, String, String)> = BTreeSet::new();
    for (a, _) in &out {
        for i in 0..names.len() {
            for j in (i + 1)..names.len() {
                seen.insert(key2(&names[i], &a[&names[i]], &names[j], &a[&names[j]]));
            }
        }
    }
    for i in 0..names.len() {
        for j in (i + 1)..names.len() {
            for vi in &cands[&names[i]] {
                for vj in &cands[&names[j]] {
                    let k = key2(&names[i], vi, &names[j], vj);
                    if seen.contains(&k) {
                        continue;
                    }
                    let mut a = base.clone();
                    a.insert(names[i].clone(), vi.clone());
                    a.insert(names[j].clone(), vj.clone());
                    for x in 0..names.len() {
                        for y in (x + 1)..names.len() {
                            seen.insert(key2(&names[x], &a[&names[x]], &names[y], &a[&names[y]]));
                        }
                    }
                    out.push((a, format!("ペアワイズ: {} × {}", names[i], names[j])));
                }
            }
        }
    }
    out
}

fn key2(a: &str, va: &Val, b: &str, vb: &Val) -> (String, String, String, String) {
    (a.into(), show(va), b.into(), show(vb))
}

pub fn show(v: &Val) -> String {
    match v {
        Val::Enum(s) | Val::Str(s) => s.clone(),
        Val::Bool(b) => if *b { "真" } else { "偽" }.into(),
        Val::Num(r) => format!("{}", r.num / r.den),
        Val::Date(y, m, d) => format!("{y:04}-{m:02}-{d:02}"),
    }
}

/// 母集団を評価し、被覆を満たす部分集合を決定的に選ぶ。
pub fn generate(f: &RuleFile, c: &Checked) -> Vec<Vector> {
    let cands = candidates(f, c);
    let raw = pool(f, c, &cands);

    let mut evaluated: Vec<Vector> = Vec::new();
    let mut seen_in: BTreeSet<String> = BTreeSet::new();
    for (a, why) in raw {
        let key = a.iter().map(|(k, v)| format!("{k}={}", show(v))).collect::<Vec<_>>().join(",");
        if !seen_in.insert(key) {
            continue;
        }
        let (out, trace) = eval::run(f, c, a.clone().into_iter().collect());
        evaluated.push(Vector { input: a, output: out, trace, why });
    }

    // 貪欲に選ぶ。狙いは行の全網羅と、候補値の全出現。
    let mut need_rows: BTreeSet<String> = BTreeSet::new();
    for v in &evaluated {
        need_rows.extend(v.trace.iter().cloned());
    }
    let mut need_vals: BTreeSet<String> = BTreeSet::new();
    for (n, vs) in &cands {
        for v in vs {
            need_vals.insert(format!("{n}={}", show(v)));
        }
    }

    let mut chosen: Vec<Vector> = Vec::new();
    let mut rows_left = need_rows.clone();
    let mut vals_left = need_vals.clone();
    let mut used = vec![false; evaluated.len()];
    loop {
        let mut best: Option<(usize, usize)> = None;
        for (i, v) in evaluated.iter().enumerate() {
            if used[i] {
                continue;
            }
            let r = v.trace.iter().filter(|t| rows_left.contains(*t)).count();
            let s = v
                .input
                .iter()
                .filter(|(n, x)| vals_left.contains(&format!("{n}={}", show(x))))
                .count();
            let score = r * 10 + s;
            if score > 0 && best.map(|(b, _)| score > b).unwrap_or(true) {
                best = Some((score, i));
            }
        }
        let Some((_, i)) = best else { break };
        used[i] = true;
        for t in &evaluated[i].trace {
            rows_left.remove(t);
        }
        for (n, x) in &evaluated[i].input {
            vals_left.remove(&format!("{n}={}", show(x)));
        }
        chosen.push(Vector {
            input: evaluated[i].input.clone(),
            output: evaluated[i].output.clone(),
            trace: evaluated[i].trace.clone(),
            why: evaluated[i].why.clone(),
        });
    }
    chosen
}

/// 正準 JSON。三者一致はこのバイト列で判定する（§8.5）。
pub fn to_json(f: &RuleFile, v: &Vector) -> String {
    let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    let mut ins: Vec<String> = Vec::new();
    for i in &f.inputs {
        let Some(val) = v.input.get(&i.name.text) else { continue };
        let body = match val {
            Val::Num(r) => format!("{}", r.num / r.den),
            Val::Bool(b) => format!("{b}"),
            other => format!("\"{}\"", esc(&show(other))),
        };
        ins.push(format!("\"{}\":{body}", esc(&i.name.text)));
    }
    let out = match &v.output {
        Some(Val::Num(r)) => format!("{}", r.num / r.den),
        Some(Val::Bool(b)) => format!("{b}"),
        Some(other) => format!("\"{}\"", esc(&show(other))),
        None => "null".into(),
    };
    let oname = f.outputs.first().map(|o| o.name.text.clone()).unwrap_or_default();
    format!(
        "{{\"in\":{{{}}},\"out\":{{\"{}\":{out}}},\"trace\":[{}],\"why\":\"{}\"}}",
        ins.join(","),
        esc(&oname),
        v.trace.iter().map(|t| format!("\"{}\"", esc(t))).collect::<Vec<_>>().join(","),
        esc(&v.why)
    )
}

/// 期待値だけを、ランナーと同じ形で出す。三者一致はこのバイト列で判定する。
pub fn expected_json(f: &RuleFile, v: &Vector) -> String {
    let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    let oname = f.outputs.first().map(|o| o.name.text.clone()).unwrap_or_default();
    let body = match &v.output {
        Some(Val::Num(r)) => format!("{}", r.num / r.den),
        Some(Val::Bool(b)) => format!("{b}"),
        Some(other) => format!("\"{}\"", esc(&show(other))),
        None => "null".into(),
    };
    format!("{{\"{}\":{body}}}", esc(&oname))
}
