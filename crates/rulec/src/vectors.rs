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

#[derive(Clone)]
pub struct Vector {
    pub input: BTreeMap<String, Val>,
    /// 宣言順の全出力（§8.5 の複数出力）。ワイヤも golden もこの順で並ぶ。
    pub outputs: Vec<(String, Option<Val>)>,
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
                // §9.1: セル群が誘導する同値類ごとに代表 1 値。47 都道府県を全部
                // 舐めるのではなく、表が区別している区分だけを踏む。同値類は
                // 「どのセルに当たるか」の並びで決まるので、群の重なりや `以外` も
                // 自動で正しく畳まれる。どのセルも名指ししない値は一つの類に落ち、
                // これが `以外` の側の代表になる。
                let all = c.enums.get(en).cloned().unwrap_or_default();
                let cells = enum_cells(f, name);
                let mut seen: BTreeSet<Vec<bool>> = BTreeSet::new();
                for v in &all {
                    let val = Val::Enum(v.clone());
                    let sig: Vec<bool> =
                        cells.iter().map(|cell| eval::cell_matches(c, cell, &val, &inner)).collect();
                    if seen.insert(sig) {
                        vs.push(val);
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
/// 列 `col` に現れるセルを全部集める。同値類の署名はこの並びで決まる。
/// 定義や導出の中で列挙値と比べている箇所も、セル一つとして数える
/// （`種別 = 率引き` のような原子は表に現れないが、値を区別している）。
fn enum_cells(f: &RuleFile, col: &str) -> Vec<Cell> {
    fn atoms(e: &Expr, col: &str, out: &mut Vec<Cell>) {
        match e {
            Expr::Bin(l, BinOp::Eq, r, _) => {
                for (a, b) in [(&**l, &**r), (&**r, &**l)] {
                    if let (Expr::Name(n, _), Expr::Lit(li, _)) = (a, b) {
                        if n == col {
                            out.push(Cell::Lit(li.clone()));
                        }
                    }
                }
            }
            Expr::Bin(l, _, r, _) => {
                atoms(l, col, out);
                atoms(r, col, out);
            }
            Expr::Call(_, args, _) => args.iter().for_each(|a| atoms(a, col, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for it in &f.items {
        match it {
            Item::Table(t) => {
                let Some(ci) = t.inputs.iter().position(|(n, _)| n == col) else { continue };
                for row in &t.rows {
                    if let Some(cell) = row.cells.get(ci) {
                        out.push(cell.clone());
                    }
                }
            }
            Item::Define(d) => atoms(&d.expr, col, &mut out),
            Item::Derived(d) => atoms(&d.expr, col, &mut out),
        }
    }
    out
}

#[allow(dead_code)]
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

    // 行被覆：各行を勝たせる割り当てを作る。導出や中間値の列は入力へ写す（§9.1）。
    // 素朴に「入力のセルを満たす値を置く」だけでは、導出が決める列に一切触れず、
    // `上から` の先行行に負けたままになる。それが被覆判定器に行被覆の欠けとして
    // 出ていた（クーポン併用 行3、適用順序 行4、素の割引 行3）。
    for it in &f.items {
        let Item::Table(t) = it else { continue };
        let tname = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
        for (ri, row) in t.rows.iter().enumerate() {
            let mut a = base.clone();
            for (ci, (col, _)) in t.inputs.iter().enumerate() {
                if !cands.contains_key(col) {
                    continue;
                }
                let Some(cell) = row.cells.get(ci) else { continue };
                let ty = c.ty_of(col).unwrap_or(Ty::Unknown);
                if let Some(v) = satisfying(cell, &cands[col], &ty, c) {
                    a.insert(col.clone(), v);
                }
            }
            let seed = win_row(f, c, cands, &a, t, ri).unwrap_or(a);
            out.push((seed.clone(), format!("行狙い: 表 {tname} 行{}", ri + 1)));

            // 境界両側被覆：行の他列を固定したまま、境界の両側を踏む**対**を作る。
            // 内側と外側で動く入力が同じになるよう、place は入力を宣言順に選ぶ。
            for (ci, (col, _)) in t.inputs.iter().enumerate() {
                let Some(ty) = c.ty_of(col) else { continue };
                if !matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Date) {
                    continue;
                }
                let Some(cell) = row.cells.get(ci) else { continue };
                let q = crate::coverage::quantum(c, col, &ty);
                for (b, inside, outside) in crate::coverage::thresholds_pub(cell, &ty, q) {
                    let (Some(ain), Some(aout)) =
                        (place(f, c, &seed, col, inside), place(f, c, &seed, col, outside))
                    else {
                        continue;
                    };
                    let why = format!("境界両側: 表 {tname} 行{} {col} {b} の", ri + 1);
                    out.push((ain, format!("{why}内側")));
                    out.push((aout, format!("{why}外側")));
                }
            }
        }

        // 遮蔽対被覆：交差の内側（行 i が勝ち、行 j の条件も成り立つ点）。
        if t.policy == Policy::TopDown {
            for j in 1..t.rows.len() {
                for i in 0..j {
                    let Some(a) = reach_row(f, c, cands, &base, t, &t.rows[j]) else { continue };
                    let Some(a) = win_row(f, c, cands, &a, t, i) else { continue };
                    if row_holds(f, c, t, &t.rows[j], &a) {
                        out.push((a, format!("遮蔽対: 表 {tname} 行{}∩行{}", i + 1, j + 1)));
                    }
                }
            }
        }
    }

    // §9.1: 名前どうしの比較（`A期限 <= B期限`）は、同着とその両側を直接踏む。
    // リテラルの境界を持たないので、候補値の側からは決して現れない。
    for (x, y) in name_pairs(f) {
        let (Some(xty), Some(_)) = (c.ty_of(&x), c.ty_of(&y)) else { continue };
        let q = crate::coverage::quantum(c, &x, &xty);
        for d in [-1i128, 0, 1] {
            let Some(xv) = base.get(&x).and_then(as_rat) else { continue };
            let t = xv.add(q.mul(Rat::int(d)));
            if let Some(a) = place(f, c, &base, &y, t) {
                out.push((a, format!("同着: {x} と {y} の {d:+}")));
            }
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
        let (outs, trace, _) = eval::run_all(f, c, a.clone().into_iter().collect());
        evaluated.push(Vector { input: a, outputs: outs, trace, why });
    }

    // §9.2 の三基準を実際に片づけたベクタを残す。狙いを付けた側の言い分ではなく、
    // 判定器が「これで義務が片づいた」と認めたものだけを鍵にする。対の義務は
    // 二本まとめて入るので、点数付けの貪欲では落ちるところ。
    let audit = crate::coverage::audit(f, c, "", &evaluated);
    let mut keep: BTreeSet<usize> = audit.witness.clone();

    // 保険のペアワイズ（§9.2）。三基準を満たしたうえで、二列の組合せのうち
    // まだ現れていないものを貪欲に足す。
    let mut seen2: BTreeSet<(String, String, String, String)> = BTreeSet::new();
    let names: Vec<String> = f.inputs.iter().map(|i| i.name.text.clone()).collect();
    let pairs_of = |v: &Vector, out: &mut BTreeSet<(String, String, String, String)>| {
        for i in 0..names.len() {
            for j in (i + 1)..names.len() {
                let (Some(a), Some(b)) = (v.input.get(&names[i]), v.input.get(&names[j])) else {
                    continue;
                };
                out.insert(key2(&names[i], a, &names[j], b));
            }
        }
    };
    for &k in &keep {
        pairs_of(&evaluated[k], &mut seen2);
    }
    loop {
        let mut best: Option<(usize, usize)> = None;
        for (i, v) in evaluated.iter().enumerate() {
            if keep.contains(&i) {
                continue;
            }
            let mut mine = BTreeSet::new();
            pairs_of(v, &mut mine);
            let gain = mine.difference(&seen2).count();
            if gain > 0 && best.map(|(g, _)| gain > g).unwrap_or(true) {
                best = Some((gain, i));
            }
        }
        let Some((_, i)) = best else { break };
        keep.insert(i);
        pairs_of(&evaluated[i], &mut seen2);
    }

    evaluated
        .into_iter()
        .enumerate()
        .filter(|(i, _)| keep.contains(i))
        .map(|(_, v)| v)
        .collect()
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
    format!(
        "{{\"in\":{{{}}},\"out\":{},\"trace\":[{}],\"why\":\"{}\"}}",
        ins.join(","),
        out_object(v),
        v.trace.iter().map(|t| format!("\"{}\"", esc(t))).collect::<Vec<_>>().join(","),
        esc(&v.why)
    )
}

/// 出力の JSON オブジェクト。宣言順に並べる（BTreeMap の名前順ではない）ので、
/// 読み手が原本の `出力` と目で対応できる。
fn out_object(v: &Vector) -> String {
    let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
    let one = |o: &Option<Val>| match o {
        Some(Val::Num(r)) => format!("{}", r.num / r.den),
        Some(Val::Bool(b)) => format!("{b}"),
        Some(other) => format!("\"{}\"", esc(&show(other))),
        None => "null".into(),
    };
    let body: Vec<String> =
        v.outputs.iter().map(|(n, val)| format!("\"{}\":{}", esc(n), one(val))).collect();
    format!("{{{}}}", body.join(","))
}

/// 期待値だけを、ランナーと同じ形で出す。三者一致はこのバイト列で判定する。
pub fn expected_json(_f: &RuleFile, v: &Vector) -> String {
    out_object(v)
}

// ── §9.1 の写像 ────────────────────────────────────────────────────────────
//
// 導出・定義・上流の表が決める列は入力ではないので、値を直接置けない。
// 設計は「線形式を一つの入力について解いて入力ベクタへ写す」と言っている。
// ここでは記号を解かず、数値微分で傾きを取ってから解き、**必ず評価し直して
// 確かめる**。式が一次でなければ解が合わないので、そのとき捨てる。
// 記号を解く実装は一次式にしか使えないが、この形は上流の表や定義が挟まっても
// 「効いたかどうか」を評価器が答えるので、同じ一つの機構で足りる。

/// 割り当てを走らせて、導出・定義・中間出力まで含めた束縛を得る。
fn bind(f: &RuleFile, c: &Checked, a: &BTreeMap<String, Val>) -> BTreeMap<String, Val> {
    let (_, _, b) = eval::run_bindings(f, c, a.clone().into_iter().collect());
    b.into_iter().collect()
}

fn as_rat(v: &Val) -> Option<Rat> {
    match v {
        Val::Num(r) => Some(*r),
        Val::Date(y, m, d) => Some(crate::types::date_ord(*y, *m, *d)),
        _ => None,
    }
}

fn to_val(r: Rat, ty: &Ty) -> Val {
    if matches!(ty, Ty::Date) {
        let (y, m, d) = crate::types::ord_to_date(r);
        Val::Date(y, m, d)
    } else {
        Val::Num(r)
    }
}

fn within(c: &Checked, col: &str, v: Rat) -> bool {
    let Some((lo, hi)) = c.ranges.get(col) else { return true };
    lo.is_none_or(|l| v.cmp_to(l) != std::cmp::Ordering::Less)
        && hi.is_none_or(|h| v.cmp_to(h) != std::cmp::Ordering::Greater)
}

/// 列 `col` を値 `target` にする割り当てを、seed から作る。入力ならそのまま置く。
/// 導出なら入力を一つ選んで解く。**変える入力はちょうど一つ**で、選び方は
/// 入力の宣言順に固定してあるので、内側と外側で同じ入力が動く。
/// §9.2 の「境界両側被覆」はベクタ対を要求するので、これが要る。
fn place(
    f: &RuleFile,
    c: &Checked,
    seed: &BTreeMap<String, Val>,
    col: &str,
    target: Rat,
) -> Option<BTreeMap<String, Val>> {
    let ty = c.ty_of(col)?;
    if seed.contains_key(col) {
        if !within(c, col, target) {
            return None;
        }
        let mut a = seed.clone();
        a.insert(col.into(), to_val(target, &ty));
        return Some(a);
    }
    let e0 = as_rat(bind(f, c, seed).get(col)?)?;
    for x in f.inputs.iter().map(|i| i.name.text.clone()) {
        let xty = c.ty_of(&x)?;
        let Some(x0) = seed.get(&x).and_then(as_rat) else { continue };
        let q = crate::coverage::quantum(c, &x, &xty);
        let mut probe = seed.clone();
        probe.insert(x.clone(), to_val(x0.add(q), &xty));
        let Some(e1) = bind(f, c, &probe).get(col).and_then(as_rat) else { continue };
        let slope = e1.sub(e0).div(q);
        if slope.num == 0 {
            continue;
        }
        let xn = x0.add(target.sub(e0).div(slope));
        // 入力の格子に載らない解は捨てる（1円 単位の入力に 0.5円 を置かない）。
        if !xn.div(q).is_int() || !within(c, &x, xn) {
            continue;
        }
        let mut a = seed.clone();
        a.insert(x.clone(), to_val(xn, &xty));
        // 一次でなければここで外れる。推測ではなく評価で確かめる。
        if bind(f, c, &a).get(col).and_then(as_rat).is_some_and(|v| v.cmp_to(target) == std::cmp::Ordering::Equal) {
            return Some(a);
        }
    }
    None
}

/// セル一つを満たす方へ一手動かす。数値は解いて当て、それ以外は入力を振る。
fn satisfy_cell(
    f: &RuleFile,
    c: &Checked,
    cands: &BTreeMap<String, Vec<Val>>,
    seed: &BTreeMap<String, Val>,
    col: &str,
    cell: &Cell,
) -> Option<BTreeMap<String, Val>> {
    let ty = c.ty_of(col)?;
    let binds = bind(f, c, seed);
    if let Some(v) = binds.get(col) {
        if eval::cell_matches(c, cell, v, &ty) {
            return Some(seed.clone());
        }
    }
    // 数値・日付：セルの内側の点へ解く。
    if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Date) {
        let q = crate::coverage::quantum(c, col, &ty);
        for (_, inside, _) in crate::coverage::thresholds_pub(cell, &ty, q) {
            if let Some(a) = place(f, c, seed, col, inside) {
                return Some(a);
            }
        }
        return None;
    }
    // 列挙・真偽：入力を一つずつ振って、当たるものを探す（決定的な順で先着）。
    if let Some(vs) = cands.get(col) {
        for v in vs {
            if eval::cell_matches(c, cell, v, &ty) {
                let mut a = seed.clone();
                a.insert(col.into(), v.clone());
                return Some(a);
            }
        }
        return None;
    }
    for x in f.inputs.iter().map(|i| i.name.text.clone()) {
        for v in cands.get(&x).into_iter().flatten() {
            let mut a = seed.clone();
            a.insert(x.clone(), v.clone());
            if bind(f, c, &a).get(col).is_some_and(|got| eval::cell_matches(c, cell, got, &ty)) {
                return Some(a);
            }
        }
    }
    None
}

/// 行のセルを全部満たす割り当て。既に満たしている列は動かさない。
fn reach_row(
    f: &RuleFile,
    c: &Checked,
    cands: &BTreeMap<String, Vec<Val>>,
    seed: &BTreeMap<String, Val>,
    t: &Table,
    row: &Row,
) -> Option<BTreeMap<String, Val>> {
    let mut a = seed.clone();
    // 二周する。前の列を直したせいで後ろの列が崩れることがあるので、
    // 一周で足りたかを確かめてから返す。
    for _ in 0..2 {
        for (ci, (col, _)) in t.inputs.iter().enumerate() {
            let Some(cell) = row.cells.get(ci) else { continue };
            if matches!(cell, Cell::DontCare) {
                continue;
            }
            a = satisfy_cell(f, c, cands, &a, col, cell)?;
        }
        if row_holds(f, c, t, row, &a) {
            return Some(a);
        }
    }
    None
}

fn row_holds(f: &RuleFile, c: &Checked, t: &Table, row: &Row, a: &BTreeMap<String, Val>) -> bool {
    let binds = bind(f, c, a);
    t.inputs.iter().enumerate().all(|(ci, (col, _))| {
        let Some(cell) = row.cells.get(ci) else { return true };
        if matches!(cell, Cell::DontCare) {
            return true;
        }
        let (Some(v), Some(ty)) = (binds.get(col), c.ty_of(col)) else { return false };
        eval::cell_matches(c, cell, v, &ty)
    })
}

/// 行 ri を**勝たせる**割り当て。`上から` ではセルを満たすだけでは足りず、
/// 先行行を外さなければならない。外すのは、その行が `-` にしていて先行行が
/// 名指ししている列に限る（行 ri が名指ししている列を動かすと ri 自身が崩れる）。
fn win_row(
    f: &RuleFile,
    c: &Checked,
    cands: &BTreeMap<String, Vec<Val>>,
    seed: &BTreeMap<String, Val>,
    t: &Table,
    ri: usize,
) -> Option<BTreeMap<String, Val>> {
    let tag = format!("表 {} 行{}", t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default(), ri + 1);
    let mut a = reach_row(f, c, cands, seed, t, &t.rows[ri])?;
    for _ in 0..t.rows.len() + 1 {
        let (_, fired, _) = eval::run_bindings(f, c, a.clone().into_iter().collect());
        if fired.contains(&tag) {
            return Some(a);
        }
        // 先に勝っている行を一つ外す。
        let mut moved = false;
        for e in 0..ri {
            if !row_holds(f, c, t, &t.rows[e], &a) {
                continue;
            }
            for (ci, (col, _)) in t.inputs.iter().enumerate() {
                if matches!(t.rows[ri].cells.get(ci), Some(Cell::DontCare) | None) {
                    let Some(ecell) = t.rows[e].cells.get(ci) else { continue };
                    if matches!(ecell, Cell::DontCare) {
                        continue;
                    }
                    if let Some(b) = violate_cell(f, c, cands, &a, col, ecell) {
                        if row_holds(f, c, t, &t.rows[ri], &b) {
                            a = b;
                            moved = true;
                            break;
                        }
                    }
                }
            }
            if moved {
                break;
            }
        }
        if !moved {
            return None;
        }
    }
    None
}

/// セルを**外す**方へ一手動かす。境界の外側へ解くか、当たらない候補を置く。
fn violate_cell(
    f: &RuleFile,
    c: &Checked,
    cands: &BTreeMap<String, Vec<Val>>,
    seed: &BTreeMap<String, Val>,
    col: &str,
    cell: &Cell,
) -> Option<BTreeMap<String, Val>> {
    let ty = c.ty_of(col)?;
    if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Date) {
        let q = crate::coverage::quantum(c, col, &ty);
        for (_, _, outside) in crate::coverage::thresholds_pub(cell, &ty, q) {
            if let Some(a) = place(f, c, seed, col, outside) {
                if !eval::cell_matches(c, cell, &to_val(outside, &ty), &ty) {
                    return Some(a);
                }
            }
        }
        return None;
    }
    if let Some(vs) = cands.get(col) {
        for v in vs {
            if !eval::cell_matches(c, cell, v, &ty) {
                let mut a = seed.clone();
                a.insert(col.into(), v.clone());
                return Some(a);
            }
        }
        return None;
    }
    for x in f.inputs.iter().map(|i| i.name.text.clone()) {
        for v in cands.get(&x).into_iter().flatten() {
            let mut a = seed.clone();
            a.insert(x.clone(), v.clone());
            if bind(f, c, &a).get(col).is_some_and(|got| !eval::cell_matches(c, cell, got, &ty)) {
                return Some(a);
            }
        }
    }
    None
}

/// §9.1 の「名前どうしの比較なら同着 A = B とその両側」。
/// 定義の中の `A期限 <= B期限` のような原子は、リテラルの境界を持たないので
/// `numeric_bounds` に何も現れない。ここで入力の組として直接踏む。
fn name_pairs(f: &RuleFile) -> Vec<(String, String)> {
    fn walk(e: &Expr, out: &mut Vec<(String, String)>) {
        match e {
            Expr::Bin(l, op, r, _)
                if matches!(op, BinOp::Le | BinOp::Lt | BinOp::Ge | BinOp::Gt | BinOp::Eq) =>
            {
                if let (Expr::Name(a, _), Expr::Name(b, _)) = (&**l, &**r) {
                    out.push((a.clone(), b.clone()));
                }
                walk(l, out);
                walk(r, out);
            }
            Expr::Bin(l, _, r, _) => {
                walk(l, out);
                walk(r, out);
            }
            Expr::Call(_, args, _) => args.iter().for_each(|a| walk(a, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for it in &f.items {
        match it {
            Item::Define(d) => walk(&d.expr, &mut out),
            Item::Derived(d) => walk(&d.expr, &mut out),
            _ => {}
        }
    }
    out
}

// ── §6.2「定義軸の証人」 ────────────────────────────────────────────────────
//
// 領域解析は真偽定義を自由軸として扱うので、交差箱の座標は「定義が真」と
// 言っているだけで、その真を作る入力が在るかは見ていない。ここで実際に入力を
// 構成し、評価器に定義まで計算させて確かめる。構成できた重なりだけが実在の
// 確認された矛盾（`一意` では E105）で、構成できなければ W114 と番人へ降ろす。
// 依存の共有だけを見て機械的に Unknown へ落とす形は採らない（§8.5）。

/// 列が取りうる値の下限と上限。`None` は制限なし。解析できないセルは `None` を返す。
fn cell_span(cell: &Cell, ty: &Ty, q: Rat) -> Option<(Option<Rat>, Option<Rat>)> {
    let lit = |l: &Lit| -> Option<Rat> {
        match l {
            Lit::Num(n) => crate::types::lit_value_in_pub(n, ty),
            Lit::Date(y, m, d) => Some(crate::types::date_ord(*y, *m, *d)),
            _ => None,
        }
    };
    match cell {
        Cell::DontCare => Some((None, None)),
        Cell::Lit(l) => lit(l).map(|b| (Some(b), Some(b))),
        Cell::Cmp(cs) => {
            let (mut lo, mut hi): (Option<Rat>, Option<Rat>) = (None, None);
            for (op, l) in cs {
                let b = lit(l)?;
                let (nl, nh) = match op {
                    CmpOp::Ge => (Some(b), None),
                    CmpOp::Gt => (Some(b.add(q)), None),
                    CmpOp::Le => (None, Some(b)),
                    CmpOp::Lt => (None, Some(b.sub(q))),
                };
                if let Some(x) = nl {
                    lo = Some(lo.map_or(x, |c: Rat| if x.cmp_to(c) == std::cmp::Ordering::Greater { x } else { c }));
                }
                if let Some(x) = nh {
                    hi = Some(hi.map_or(x, |c: Rat| if x.cmp_to(c) == std::cmp::Ordering::Less { x } else { c }));
                }
            }
            Some((lo, hi))
        }
        _ => None,
    }
}

/// 列 `col` について、渡されたセルを**全部**満たす値へ一手動かす。
fn satisfy_all(
    f: &RuleFile,
    c: &Checked,
    cands: &BTreeMap<String, Vec<Val>>,
    seed: &BTreeMap<String, Val>,
    col: &str,
    cells: &[&Cell],
) -> Option<BTreeMap<String, Val>> {
    let ty = c.ty_of(col)?;
    let hit = |v: &Val| cells.iter().all(|cell| eval::cell_matches(c, cell, v, &ty));
    if bind(f, c, seed).get(col).is_some_and(hit) {
        return Some(seed.clone());
    }
    if matches!(ty, Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate | Ty::Date) {
        // 区間の交わりを取る。空なら、この対はこの列で同時に成り立たない。
        let (mut lo, mut hi): (Option<Rat>, Option<Rat>) = match c.ranges.get(col) {
            Some((l, h)) => (*l, *h),
            None => (None, None),
        };
        let q = crate::coverage::quantum(c, col, &ty);
        for cell in cells {
            let (l, h) = cell_span(cell, &ty, q)?;
            if let Some(x) = l {
                lo = Some(lo.map_or(x, |c: Rat| if x.cmp_to(c) == std::cmp::Ordering::Greater { x } else { c }));
            }
            if let Some(x) = h {
                hi = Some(hi.map_or(x, |c: Rat| if x.cmp_to(c) == std::cmp::Ordering::Less { x } else { c }));
            }
        }
        if let (Some(l), Some(h)) = (lo, hi) {
            if l.cmp_to(h) == std::cmp::Ordering::Greater {
                return None;
            }
        }
        for target in [lo, hi].into_iter().flatten() {
            if let Some(a) = place(f, c, seed, col, target) {
                if bind(f, c, &a).get(col).is_some_and(hit) {
                    return Some(a);
                }
            }
        }
        return None;
    }
    if let Some(vs) = cands.get(col) {
        let v = vs.iter().find(|v| hit(v))?;
        let mut a = seed.clone();
        a.insert(col.into(), v.clone());
        return Some(a);
    }
    // 上流の表や定義が決める列。入力を一つずつ振って寄せる（決定的な順で先着）。
    for x in f.inputs.iter().map(|i| i.name.text.clone()) {
        for v in cands.get(&x).into_iter().flatten() {
            let mut a = seed.clone();
            a.insert(x.clone(), v.clone());
            if bind(f, c, &a).get(col).is_some_and(hit) {
                return Some(a);
            }
        }
    }
    None
}

/// 行 i と行 j の**両方**に当たる入力を実際に構成する（§6.2「定義軸の証人」）。
/// 見つかれば重なりは実在で、`一意` なら E105。見つからなければ未確認のまま
/// W114 と番人へ降ろす。**見つからないことは非存在の証明ではない**ので、
/// 呼ぶ側はそれを断定しない。
pub fn pair_witness(
    f: &RuleFile,
    c: &Checked,
    t: &Table,
    i: usize,
    j: usize,
) -> Option<BTreeMap<String, Val>> {
    let cands = candidates(f, c);
    let mut a: BTreeMap<String, Val> = f
        .inputs
        .iter()
        .map(|x| Some((x.name.text.clone(), cands.get(&x.name.text)?.first()?.clone())))
        .collect::<Option<BTreeMap<_, _>>>()?;
    let hold = |a: &BTreeMap<String, Val>| {
        row_holds(f, c, t, &t.rows[i], a) && row_holds(f, c, t, &t.rows[j], a)
    };
    // 列を順に寄せる。一つ直すと前の列が崩れることがあるので、崩れなくなるまで回す。
    for _ in 0..3 {
        if hold(&a) {
            return Some(a);
        }
        for (ci, (col, _)) in t.inputs.iter().enumerate() {
            let cells: Vec<&Cell> = [i, j]
                .iter()
                .filter_map(|&r| t.rows[r].cells.get(ci))
                .filter(|cell| !matches!(cell, Cell::DontCare))
                .collect();
            if cells.is_empty() {
                continue;
            }
            a = satisfy_all(f, c, &cands, &a, col, &cells)?;
        }
    }
    hold(&a).then_some(a)
}
