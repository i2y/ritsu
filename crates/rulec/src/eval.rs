//! 参照評価器（§0）。
//!
//! 意味論の原本はここ一つに置く。`例` の検証も、テストベクタの期待値も、過去再生も
//! 同じ評価器を通す。本番で走るのは生成コードだけで、この評価器は検証系にしか出ない。

use crate::ast::*;
use crate::diag::Diag;
use crate::num::{Rat, RoundMode};
use crate::types::{Checked, Ty, lit_value_in_pub};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Val {
    Enum(String),
    /// 宣言単位での値。金額なら円、質量なら宣言した単位。
    Num(Rat),
    Bool(bool),
    Str(String),
    Date(i32, u32, u32),
}

impl Val {
    fn show(&self, ty: &Ty) -> String {
        match self {
            Val::Enum(s) | Val::Str(s) => s.clone(),
            Val::Bool(b) => if *b { "真" } else { "偽" }.into(),
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
    /// 発火した行。§11 の E107 は「発火行つき」で報告する。
    pub fired: Vec<String>,
}

pub fn lit_to_val(l: &Lit, ty: &Ty) -> Option<Val> {
    Some(match l {
        Lit::Num(n) => Val::Num(lit_value_in_pub(n, ty)?),
        Lit::Word(w) => {
            if w == "真" {
                Val::Bool(true)
            } else if w == "偽" {
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
    /// セルが値に当たるか。§3 の七種の単項テストに一対一で対応する。
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
                    // 群は宣言された部分集合なので、メンバなら当たる。
                    self.c.groups.get(w).is_some_and(|(_, ms)| ms.contains(e))
                }
                (Lit::Word(w), Val::Bool(b)) => (w == "真") == *b,
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
                // 日付は順序数に落として、数値と同じ機構で比べる（§2.1 は比較と範囲だけ）。
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
                // 式の中のリテラルは、書かれた単位のまま基準単位に落とす。
                lit_to_val(l, &Ty::Money { cur: "円".into(), tax: None }).or_else(|| lit_to_val(l, &Ty::Rate))
            }
            Expr::Call(name, args, _) => {
                let a: Vec<Val> = args.iter().filter_map(|x| self.expr(x)).collect();
                match (name.as_str(), a.as_slice()) {
                    ("最小", [Val::Num(x), Val::Num(y)]) => {
                        Some(Val::Num(if x.cmp_to(*y) == std::cmp::Ordering::Less { *x } else { *y }))
                    }
                    ("最大", [Val::Num(x), Val::Num(y)]) => {
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
                // 日付は順序数に落として比べる。加減算は持たないので比較のみ（§2.1）。
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

    /// 表を一つ評価し、出力を束縛する。当たった行の番号を返す。
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
        self.fired.push(format!("表 {name} 行{}", hit + 1));
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

/// 入力の束縛から規則を最後まで走らせ、出力の値を返す。
pub fn run(f: &RuleFile, c: &Checked, inputs: HashMap<String, Val>) -> (Option<Val>, Vec<String>) {
    let (v, fired, _) = run_bindings(f, c, inputs);
    (v, fired)
}

/// §9.2 の被覆判定は「行のセルがその入力で成り立つか」を、導出や定義の列まで含めて
/// 見る必要がある。走らせ終えた束縛をそのまま返す入口。
pub fn run_bindings(
    f: &RuleFile,
    c: &Checked,
    inputs: HashMap<String, Val>,
) -> (Option<Val>, Vec<String>, HashMap<String, Val>) {
    let (outs, fired, binds) = run_all(f, c, inputs);
    (outs.into_iter().next().and_then(|(_, v)| v), fired, binds)
}

/// 出力を全部返す（§8.5 の複数出力）。宣言順で、丸めは出力ごとに一度掛かる。
/// `結果` は第一出力にだけ効く糖衣なので（§1.2）、二本目以降は同名の束縛から取る。
pub fn run_all(
    f: &RuleFile,
    c: &Checked,
    inputs: HashMap<String, Val>,
) -> (Vec<(String, Option<Val>)>, Vec<String>, HashMap<String, Val>) {
    let mut env = Env { vals: inputs, c, fired: Vec::new() };
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
        // 出力の丸め宣言は最後に効く。§7.2 の「丸めていない値は出力に届かない」。
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
    (outs, env.fired, env.vals)
}

/// セル一つの当たり判定。表の外（§9.2 の被覆判定）から使う。
pub fn cell_matches(c: &Checked, cell: &Cell, v: &Val, ty: &Ty) -> bool {
    Env { vals: HashMap::new(), c, fired: Vec::new() }.matches(cell, v, ty)
}

/// `例` は実行される仕様である（§1.2）。外れたら発火行つきで E107。
pub fn check_examples(f: &RuleFile, c: &Checked, path: &str) -> Vec<Diag> {
    let mut out = Vec::new();
    let Some(ex) = &f.examples else { return out };
    if f.outputs.is_empty() {
        return out;
    }

    // §1.2: 例は三者一致の盲点を破る楔である。三者が同じ誤りを共有すると
    // 一致は緑のままで、それを破れるのは人の書いた期待値しかない。だから
    // 出力は全部書かせる。列の欠けはエラー（E111）。
    let head = ex.rows.first().map(|r| r.span.clone());
    for od in &f.outputs {
        if !ex.outputs.iter().any(|o| o.name.text == od.name.text) {
            let Some(sp) = head.clone() else { continue };
            out.push(
                Diag::error("E111", format!("例に出力 {} の列がありません", od.name.text))
                    .at(format!("{path}:{} 例", sp.line))
                    .mark(sp, format!("{} の期待値がありません", od.name.text))
                    .note("例は三者一致では捕まらない誤りを捕まえる唯一の楔なので、出力は全部書きます。")
                    .note(format!("ヒント: 見出しに `{}` の列を足してください。", od.name.text)),
            );
        }
    }
    for row in &ex.rows {
        if row.outs.len() < ex.outputs.len() {
            out.push(
                Diag::error(
                    "E111",
                    format!("例の期待値が {} 列足りません", ex.outputs.len() - row.outs.len()),
                )
                .at(format!("{path}:{} 例", row.span.line))
                .mark(row.span.clone(), "")
                .note("宣言した出力の数だけ期待値を書いてください。"),
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
        let (got, fired, _) = run_all(f, c, env);
        // 出力は全部見る。第一出力しか見ないと、二本目の期待値をいくら間違えても
        // 例が黙って通る（型が解析できない列を黙って飛ばした E110 と同じ形）。
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
                    Diag::error(
                        "E107",
                        format!(
                            "例が合いません: {} は {} のはずが {} になりました",
                            od.name.text,
                            w.show(&oty),
                            g.show(&oty)
                        ),
                    )
                    .at(format!("{path}:{} 例", row.span.line))
                    .mark(row.span.clone(), "")
                    .note(format!("発火した行: {}", fired.join(" / ")))
                    .note("例は実行される仕様です。表を直すか、例のほうが間違っているなら例を直してください。"),
                ),
                (Some(w), None) => out.push(
                    Diag::error(
                        "E107",
                        format!("例が合いません: {} は {} のはずが、値が出ませんでした", od.name.text, w.show(&oty)),
                    )
                    .at(format!("{path}:{} 例", row.span.line))
                    .mark(row.span.clone(), "")
                    .note(if fired.is_empty() {
                        "どの表も発火しませんでした。".to_string()
                    } else {
                        format!("発火した行: {}", fired.join(" / "))
                    }),
                ),
                _ => {}
            }
        }
    }
    out
}

/// E104 の文面を作るための証人。各入力から一つ値を選ぶ。列挙は宣言順の最初、
/// 数値は宣言範囲の中で端数を作りやすい値を選ぶ（§11 原則 2）。
/// 数値入力をどこから取るか。端数は「相手の刻みとの噛み合わせ」で出るので、
/// 中央だけ見ても見つからない（上限に当たって丸まる、など）。
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
                // 宣言された刻みに載る値であること。刻みを外れた証人は、
                // そもそも起こりえない入力なので証人にならない。
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

/// 丸め宣言を無視して、出力に届く生の値を求める。E104 が「いくら動くか」を言うために使う。
pub fn unrounded_output(f: &RuleFile, c: &Checked) -> Option<Rat> {
    // 端数の出る証人を先に探す。見つからなければ最初の証人の値を返す
    // （そのときは本当に端数が出ない規則なので、E104 の文面はそちらへ分岐する）。
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
    let mut env = Env { vals: inputs, c, fired: Vec::new() };
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
