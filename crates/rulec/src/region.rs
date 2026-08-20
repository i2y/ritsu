//! 領域 IR と箱代数（§6）。
//!
//! 各入力列を独立の軸に取り、列に現れる境界値で座標を圧縮する。§3 がセルを
//! 自列への単項テストに限っているので、行は「軸ごとの部分集合の直積」＝箱になり、
//! 完全性・冗長・重複がすべて有限の集合演算に落ちる。

use crate::ast::*;
use crate::diag::{Diag, Span};
use crate::num::Rat;
use crate::types::{Checked, Ty};
use std::collections::BTreeSet;

#[derive(Debug, Clone)]
enum Coord {
    /// ちょうどこの値。
    Point(Rat),
    /// 二つの境界の間（両端を含まない）。端が None なら宣言範囲の外側。
    Open(Option<Rat>, Option<Rat>),
}

#[derive(Debug, Clone)]
enum Axis {
    Enum { values: Vec<String> },
    /// 数値と日付。日付は順序数（y*10000+m*100+d）で持つので、区間の機構が
    /// そのまま使える。`unit` が空なら日付として書き戻す。
    Num { unit: String, coords: Vec<Coord> },
    Bool,
}

impl Axis {
    fn len(&self) -> usize {
        match self {
            Axis::Enum { values } => values.len(),
            Axis::Num { coords, .. } => coords.len(),
            Axis::Bool => 2,
        }
    }
    /// 証人にする具体値。§11 原則 2 は列挙なら宣言順、数値なら境界値を優先する。
    fn witness(&self, i: usize) -> String {
        match self {
            Axis::Enum { values } => values.get(i).cloned().unwrap_or_default(),
            Axis::Bool => if i == 0 { "真".into() } else { "偽".into() },
            Axis::Num { unit, coords } if unit.is_empty() => match coords.get(i) {
                // 日付は境界（実在する暦日）を優先する。区間の中点は暦日とは限らない。
                Some(Coord::Point(v)) => {
                    let (y, m, d) = crate::types::ord_to_date(*v);
                    format!("{y:04}-{m:02}-{d:02}")
                }
                Some(Coord::Open(a, b)) => {
                    // 通算日なので、開区間の中に必ず実在する暦日がある。
                    // 「前後」と濁さずに、その日を出す。
                    let v = match (a, b) {
                        (Some(a), _) => a.add(Rat::int(1)),
                        (None, Some(b)) => b.sub(Rat::int(1)),
                        (None, None) => Rat::zero(),
                    };
                    let (y, m, d) = crate::types::ord_to_date(v);
                    format!("{y:04}-{m:02}-{d:02}")
                }
                None => String::new(),
            },
            Axis::Num { unit, coords } => match coords.get(i) {
                Some(Coord::Point(v)) => format!("{v}{unit}"),
                Some(Coord::Open(a, b)) => {
                    let v = match (a, b) {
                        (Some(a), Some(b)) => a.add(*b).div(Rat::int(2)),
                        (Some(a), None) => a.add(Rat::int(1)),
                        (None, Some(b)) => b.sub(Rat::int(1)),
                        (None, None) => Rat::zero(),
                    };
                    format!("{v}{unit}")
                }
                None => String::new(),
            },
        }
    }
}

/// §6.2 の実現可能性。導出を独立軸にしたぶん、空間には実在しない点が混ざる。
/// 篩はそれを報告段で落とすが、従属が絡むと判定しきれない。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Feasible {
    /// 実現不能と証明できた。報告しない。
    No,
    /// 証人を構成できた。エラーとして出す。
    Yes,
    /// 判定できなかった。証明していないことを証明済みの顔で出さない（W114）。
    Unknown,
}

pub struct TableRegion {
    axes: Vec<Axis>,
    col_names: Vec<String>,
    /// 軸 → 上流が出しうる値の座標。None なら制限なし（入力や導出の軸）。
    reachable: Vec<Option<Vec<bool>>>,
    /// 行 → 上流の到達不能な値だけを名指ししているか（E102 の変種）。
    unreachable_row: Vec<bool>,
    /// 軸 → 表での元の列位置。探索は細い軸から見るが、証人は表の見た目の順で出す。
    display_of: Vec<usize>,
    /// 軸が真偽の定義かどうか。篩は定義軸を見ないので（M1 で検証優先の形で入れる）、
    /// 定義列が絡む重なりの証人は「定義の中身との整合が未確認」であることを言う。
    is_define: Vec<bool>,
    /// 軸が導出なら、その到達区間と依存する入力名。
    derived: Vec<Option<((Option<Rat>, Option<Rat>), Vec<String>)>>,
    /// 行 → 軸 → 座標の採否。
    masks: Vec<Vec<Vec<bool>>>,
}

fn lit_rat(l: &Lit, want: &Ty) -> Option<Rat> {
    match l {
        Lit::Num(n) => crate::types::lit_value_in_pub(n, want),
        Lit::Date(y, m, d) if *want == Ty::Date => Some(crate::types::date_ord(*y, *m, *d)),
        _ => None,
    }
}

/// 数値軸の境界を集める。宣言範囲の両端も境界に入れて、宇宙を有限に閉じる。
fn num_bounds(rows: &[Row], ci: usize, want: &Ty, range: &Option<Range>) -> (Vec<Rat>, Option<Rat>, Option<Rat>) {
    let mut set: BTreeSet<(i128, i128)> = BTreeSet::new();
    let mut push = |r: Rat, s: &mut BTreeSet<(i128, i128)>| {
        s.insert((r.num, r.den));
    };
    let (mut lo, mut hi) = (None, None);
    if let Some(rg) = range {
        for (op, l) in &rg.bounds {
            if let Some(v) = lit_rat(l, want) {
                match op {
                    CmpOp::Ge | CmpOp::Gt => lo = Some(v),
                    CmpOp::Le | CmpOp::Lt => hi = Some(v),
                }
                push(v, &mut set);
            }
        }
    }
    for row in rows {
        if let Some(Cell::Cmp(cs)) = row.cells.get(ci) {
            for (_, l) in cs {
                if let Some(v) = lit_rat(l, want) {
                    push(v, &mut set);
                }
            }
        }
        if let Some(Cell::Lit(l)) = row.cells.get(ci) {
            if let Some(v) = lit_rat(l, want) {
                push(v, &mut set);
            }
        }
    }
    let mut v: Vec<Rat> = set.into_iter().map(|(n, d)| Rat { num: n, den: d }).collect();
    v.sort_by(|a, b| a.cmp_to(*b));
    (v, lo, hi)
}

/// 座標を作る。`quantum` は値が取りうる最小の刻み（金額[円] なら 1、日付なら 1 日、
/// 率[刻み 0.1%] なら 1/1000）。
///
/// 隣り合う境界の差が刻みちょうどなら、そのあいだの開区間には**値が一つも無い**。
/// 空の座標を作ると、`<=2026-03-31` と `>=2026-04-01` で敷き詰めた表が
/// 覆いきれていないことになり、偽の E101 が出る。両端含みの敷き詰めは業務の
/// 自然な書き方なので、この判定は必ず要る。
fn num_coords(bounds: &[Rat], lo: Option<Rat>, hi: Option<Rat>, quantum: Rat) -> Vec<Coord> {
    let mut out = Vec::new();
    if bounds.is_empty() {
        return vec![Coord::Open(lo, hi)];
    }
    // 宣言下限より下は宇宙の外。下限が無いときだけ開区間を先頭に置く。
    if lo.is_none() {
        out.push(Coord::Open(None, Some(bounds[0])));
    }
    for (i, b) in bounds.iter().enumerate() {
        out.push(Coord::Point(*b));
        if i + 1 < bounds.len() {
            let gap = bounds[i + 1].sub(*b);
            if gap.cmp_to(quantum) == std::cmp::Ordering::Greater {
                out.push(Coord::Open(Some(*b), Some(bounds[i + 1])));
            }
        }
    }
    if hi.is_none() {
        out.push(Coord::Open(Some(*bounds.last().unwrap()), None));
    }
    out
}

fn cmp_holds(op: CmpOp, v: Rat, bound: Rat) -> bool {
    use std::cmp::Ordering::*;
    match (op, v.cmp_to(bound)) {
        (CmpOp::Le, Less | Equal) => true,
        (CmpOp::Lt, Less) => true,
        (CmpOp::Ge, Greater | Equal) => true,
        (CmpOp::Gt, Greater) => true,
        _ => false,
    }
}

/// この座標のすべての値が比較を満たすか。境界はすべて座標に取ってあるので、
/// 開区間は必ずどちらか一方に丸ごと入る（これが圧縮座標の効き目）。
fn coord_satisfies(c: &Coord, op: CmpOp, bound: Rat) -> bool {
    match c {
        Coord::Point(v) => cmp_holds(op, *v, bound),
        Coord::Open(a, b) => match op {
            CmpOp::Le | CmpOp::Lt => b.is_some_and(|b| cmp_holds(CmpOp::Le, b, bound)),
            CmpOp::Ge | CmpOp::Gt => a.is_some_and(|a| cmp_holds(CmpOp::Ge, a, bound)),
        },
    }
}

impl TableRegion {
    pub fn build(t: &Table, c: &Checked, inputs: &[VarDecl]) -> Option<TableRegion> {
        let mut axes = Vec::new();
        let mut col_names = Vec::new();
        for (ci, (name, _)) in t.inputs.iter().enumerate() {
            let ty = c.ty_of(name)?;
            col_names.push(name.clone());
            let axis = match &ty {
                Ty::Enum(en) => Axis::Enum { values: c.enums.get(en)?.clone() },
                Ty::Bool => Axis::Bool,
                Ty::Date => {
                    let range = inputs.iter().find(|i| i.name.text == *name).and_then(|i| i.range.clone());
                    let (b, lo, hi) = num_bounds(&t.rows, ci, &ty, &range);
                    // 日付の刻みは 1 日。通算日で持っているので、隣接は差が 1。
                    Axis::Num { unit: String::new(), coords: num_coords(&b, lo, hi, Rat::int(1)) }
                }
                Ty::Money { .. } | Ty::Qty { .. } | Ty::Rate => {
                    let range = inputs.iter().find(|i| i.name.text == *name).and_then(|i| i.range.clone());
                    let (b, lo, hi) = num_bounds(&t.rows, ci, &ty, &range);
                    let unit = match &ty {
                        Ty::Money { cur, .. } => cur.clone(),
                        Ty::Qty { unit, .. } => unit.clone(),
                        _ => "%".into(),
                    };
                    // 実行時表現は宣言単位の整数一本（§7.1）なので、刻みは
                    // 金額・数量なら 1、率なら宣言された刻み。
                    let q = match &ty {
                        Ty::Rate => Rat::new(1, *c.scales.get(name).unwrap_or(&100)),
                        _ => Rat::int(1),
                    };
                    Axis::Num { unit, coords: num_coords(&b, lo, hi, q) }
                }
                _ => return None,
            };
            axes.push(axis);
        }

        // §6.3: 圧縮座標数の昇順に軸を並べる。細い軸から割ると枝刈りが早く効く。
        let mut order: Vec<usize> = (0..axes.len()).collect();
        order.sort_by_key(|&i| axes[i].len());
        let axes: Vec<Axis> = order.iter().map(|&i| axes[i].clone()).collect();
        let col_names: Vec<String> = order.iter().map(|i| col_names[*i].clone()).collect();
        let cell_of: Vec<usize> = order.clone();

        // 軸の並べ替えの**後**で作る。前で作ると、マスクの添字とずれる。
        let is_define: Vec<bool> = col_names.iter().map(|n| c.define_deps.contains_key(n)).collect();
        let derived: Vec<Option<((Option<Rat>, Option<Rat>), Vec<String>)>> = col_names
            .iter()
            .map(|n| c.derived_deps.get(n).map(|deps| (c.ranges.get(n).copied().unwrap_or((None, None)), deps.clone())))
            .collect();


        // 上流が出しうる値の座標。列挙軸だけが持つ。
        let reachable: Vec<Option<Vec<bool>>> = (0..axes.len())
            .map(|ai| match (&axes[ai], c.out_values.get(&col_names[ai])) {
                (Axis::Enum { values }, Some(vs)) => {
                    Some(values.iter().map(|v| vs.contains(v)).collect())
                }
                // 真偽も上流の出力になりうる。座標は 0=真、1=偽。
                (Axis::Bool, Some(vs)) => Some(vec![
                    vs.iter().any(|v| v == "真"),
                    vs.iter().any(|v| v == "偽"),
                ]),
                _ => None,
            })
            .collect();

        let mut unreachable_row = Vec::new();
        let mut masks = Vec::new();
        for row in &t.rows {
            let mut m = Vec::new();
            for (ai, axis) in axes.iter().enumerate() {
                let n = axis.len();
                let cell = row.cells.get(cell_of[ai]);
                let mut v = vec![false; n];
                let ty = c.ty_of(&col_names[ai])?;
                match cell {
                    None | Some(Cell::DontCare) | Some(Cell::Nothing) => v.iter_mut().for_each(|x| *x = true),
                    Some(Cell::Lit(l)) => match (axis, l) {
                        (Axis::Enum { values }, Lit::Word(w)) => {
                            for (i, val) in values.iter().enumerate() {
                                if val == w {
                                    v[i] = true;
                                }
                            }
                            if let Some((_, members)) = c.groups.get(w) {
                                for (i, val) in values.iter().enumerate() {
                                    if members.contains(val) {
                                        v[i] = true;
                                    }
                                }
                            }
                        }
                        (Axis::Bool, Lit::Word(w)) => v[if w == "真" { 0 } else { 1 }] = true,
                        (Axis::Num { coords, .. }, l) => {
                            if let Some(x) = lit_rat(l, &ty) {
                                for (i, cd) in coords.iter().enumerate() {
                                    if matches!(cd, Coord::Point(p) if p.cmp_to(x) == std::cmp::Ordering::Equal) {
                                        v[i] = true;
                                    }
                                }
                            }
                        }
                        _ => {}
                    },
                    Some(Cell::Set(ls)) | Some(Cell::Not(ls)) => {
                        if let Axis::Enum { values } = axis {
                            for l in ls {
                                if let Lit::Word(w) = l {
                                    for (i, val) in values.iter().enumerate() {
                                        if val == w {
                                            v[i] = true;
                                        }
                                    }
                                    if let Some((_, members)) = c.groups.get(w) {
                                        for (i, val) in values.iter().enumerate() {
                                            if members.contains(val) {
                                                v[i] = true;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        if matches!(cell, Some(Cell::Not(_))) {
                            v.iter_mut().for_each(|x| *x = !*x);
                        }
                    }
                    Some(Cell::Cmp(cs)) => {
                        if let Axis::Num { coords, .. } = axis {
                            v.iter_mut().for_each(|x| *x = true);
                            for (op, l) in cs {
                                if let Some(b) = lit_rat(l, &ty) {
                                    for (i, cd) in coords.iter().enumerate() {
                                        if !coord_satisfies(cd, *op, b) {
                                            v[i] = false;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                m.push(v);
            }
            // 上流が出さない値だけを名指ししている行は、下流では死ぬ。
            let mut only_unreachable = false;
            for (ai, r) in reachable.iter().enumerate() {
                let Some(r) = r else { continue };
                let had = m[ai].iter().any(|x| *x);
                for (k, ok) in r.iter().enumerate() {
                    if !ok {
                        m[ai][k] = false;
                    }
                }
                if had && !m[ai].iter().any(|x| *x) {
                    only_unreachable = true;
                }
            }
            unreachable_row.push(only_unreachable);
            masks.push(m);
        }
        Some(TableRegion { axes, col_names, reachable, unreachable_row, display_of: cell_of, is_define, derived, masks })
    }

    fn intersects(&self, i: usize, j: usize) -> bool {
        self.masks[i]
            .iter()
            .zip(&self.masks[j])
            .all(|(a, b)| a.iter().zip(b).any(|(x, y)| *x && *y))
    }

    /// 行 `inner` の領域が行 `outer` に丸ごと含まれるか。
    ///
    /// 行は「軸ごとの部分集合の直積」＝単一の箱なので、包含は軸ごとの部分集合と
    /// **厳密に一致する**（射影による近似ではない）。領域の減算に落とす必要が
    /// あるのは行が箱の和になったときで、いまの表現ではそうならない。
    /// 減算に落とすと軸数に対して指数的に効くので、ここは直積の性質を使う。
    fn contains(&self, outer: usize, inner: usize) -> bool {
        self.masks[inner]
            .iter()
            .zip(&self.masks[outer])
            .all(|(a, b)| a.iter().zip(b).all(|(x, y)| !*x || *y))
    }

    fn empty(&self, i: usize) -> bool {
        self.masks[i].iter().any(|m| m.iter().all(|x| !*x))
    }

    /// 未被覆の座標をひとつ返す。§6.3 の再帰分割で、残り軸が全部 don't-care の行が
    /// 生き残っている枝は被覆確定として刈る。
    fn find_hole(&self, rows: &[usize], budget: &mut i64) -> Option<Vec<usize>> {
        self.hole_rec(rows, 0, budget, &mut Vec::new())
    }

    fn hole_rec(
        &self,
        rows: &[usize],
        ai: usize,
        budget: &mut i64,
        path: &mut Vec<usize>,
    ) -> Option<Vec<usize>> {
        *budget -= 1;
        if *budget < 0 {
            return None;
        }
        if rows.is_empty() {
            let mut p = path.clone();
            while p.len() < self.axes.len() {
                p.push(0);
            }
            // 実現不能と証明できた穴は報告しない。判定できない穴は残す
            // （過剰報告の向きにだけ外す。§6.1）。
            if self.feasible(&p) == Feasible::No {
                return None;
            }
            return Some(p);
        }
        if ai == self.axes.len() {
            return None;
        }
        // 残りの軸がすべて全域の行があれば、この部分木は覆われている。
        if rows
            .iter()
            .any(|&r| self.masks[r][ai..].iter().all(|m| m.iter().all(|x| *x)))
        {
            return None;
        }
        for c in 0..self.axes[ai].len() {
            let sub: Vec<usize> = rows.iter().copied().filter(|&r| self.masks[r][ai][c]).collect();
            path.push(c);
            if let Some(h) = self.hole_rec(&sub, ai + 1, budget, path) {
                path.pop();
                return Some(h);
            }
            path.pop();
        }
        None
    }

    /// 二つの行が重なる領域を、業務の言葉の連言で書く。
    ///
    /// W114 では点を出してはいけない。導出軸上の一点は実現されていない座標なので、
    /// 具体値の顔をすると読み手が「その注文」を探しに行く。未検証のものを
    /// 証人の顔で出さない、という原則の適用箇所（§11 原則 2）。
    fn overlap_text(&self, a: &Row, b: &Row) -> String {
        let mut items: Vec<(usize, String)> = Vec::new();
        for ai in 0..self.axes.len() {
            let ci = self.display_of[ai];
            let (ca, cb) = (a.cells.get(ci), b.cells.get(ci));
            let mut parts: Vec<String> = Vec::new();
            for c in [ca, cb].into_iter().flatten() {
                if matches!(c, Cell::DontCare) {
                    continue;
                }
                let t = cell_text(c);
                if !parts.contains(&t) {
                    parts.push(t);
                }
            }
            if parts.is_empty() {
                continue;
            }
            items.push((ci, format!("{} {}", self.col_names[ai], parts.join(" かつ "))));
        }
        items.sort_by_key(|(d, _)| *d);
        if items.is_empty() {
            return "（どの列も制約していません）".into();
        }
        items.into_iter().map(|(_, s)| s).collect::<Vec<_>>().join(" かつ ")
    }

    /// 証人は表の列順で書く。探索の都合（細い軸から見る）を読み手に見せない。
    fn witness_text(&self, path: &[usize]) -> String {
        let mut items: Vec<(usize, String)> = (0..self.axes.len())
            .map(|ai| {
                (
                    self.display_of[ai],
                    format!("{} = {}", self.col_names[ai], self.axes[ai].witness(path.get(ai).copied().unwrap_or(0))),
                )
            })
            .collect();
        items.sort_by_key(|(d, _)| *d);
        items.into_iter().map(|(_, s)| s).collect::<Vec<_>>().join(", ")
    }
}

/// 遮蔽の三分類（§4）。構造的と同値は件数だけ、要確認だけが一覧に出る。
#[derive(Debug, Default, Clone, Copy)]
pub struct Shadow {
    /// 後の行が前の行の領域を包含する。階段の通常の姿。
    pub structural: usize,
    /// 包含でない部分交差だが、出力が同じ。どちらが勝っても値は変わらない。
    pub equivalent: usize,
    /// 包含でない部分交差で、出力が異なる。ここだけが「意図通りですか」に値する。
    pub confirm: usize,
}

impl Shadow {
    pub fn total(&self) -> usize {
        self.structural + self.equivalent + self.confirm
    }
}

/// §6.3 の訪問ノード予算。超えたら「未証明」として止める。近似検査へ静かに縮退はしない。
///
/// 既定値は §15-2 の合成ベンチの実測から決めた（`tests/budget.rs`）。
///
/// | 列（列挙+数値） | 行 | ノード | ms(release) |
/// |---|---|---|---|
/// | 8+4（設計目標の 12 列） | 500 | 10,042,885 | 164 |
/// | 6+6 | 500 | 10,821,337 | 197 |
/// | 6+6 | 1000 | 41,530,393 | 680 |
///
/// 速度は約 6 万ノード/ms（release）で、行数に対して二乗で伸びる。設計目標
/// （列 12、行 500）は 10M で収まるので、外れていたのは仮置きの 10⁶ のほうだった。
/// 5,000 万は目標に 5 倍の余裕を持たせつつ、使い切っても 1 秒弱で止まる値。
/// 実表のコーパスは 522 / 267 / 7 ノードで、五桁下にいる。
pub const DEFAULT_BUDGET: i64 = 50_000_000;

pub struct TableCheck {
    pub diags: Vec<Diag>,
    /// この表の検査で訪問したノード数。予算を決めるための実測値。
    pub nodes: i64,
    /// `--show-shadow` のときだけ出す、構造的と同値の一覧。
    pub quiet: Vec<Diag>,
    pub shadow: Shadow,
}

/// セルの正規形。`--diff-base` の鍵に使うので、整形にも行番号にも依存しない。
fn cell_key(c: &Cell) -> String {
    fn lit(l: &Lit) -> String {
        match l {
            Lit::Num(n) => n.raw.replace(' ', ""),
            Lit::Word(w) => w.clone(),
            Lit::Str(s) => format!("\"{s}\""),
            Lit::Date(y, m, d) => format!("{y:04}-{m:02}-{d:02}"),
        }
    }
    match c {
        Cell::DontCare => "-".into(),
        Cell::Nothing => "無し".into(),
        Cell::Lit(l) => lit(l),
        Cell::Set(ls) => ls.iter().map(lit).collect::<Vec<_>>().join("・"),
        Cell::Not(ls) => format!("以外:{}", ls.iter().map(lit).collect::<Vec<_>>().join("・")),
        Cell::Cmp(cs) => cs
            .iter()
            .map(|(o, l)| {
                let op = match o {
                    CmpOp::Le => "<=",
                    CmpOp::Ge => ">=",
                    CmpOp::Lt => "<",
                    CmpOp::Gt => ">",
                };
                format!("{op}{}", lit(l))
            })
            .collect::<Vec<_>>()
            .join(" "),
    }
}

/// セルを業務の言葉で書く。W114 は点ではなく領域で出すので、条件の字面が要る。
fn cell_text(c: &Cell) -> String {
    fn lit(l: &Lit) -> String {
        match l {
            Lit::Num(n) => n.raw.clone(),
            Lit::Word(w) => w.clone(),
            Lit::Str(s) => format!("\"{s}\""),
            Lit::Date(y, m, d) => format!("{y:04}-{m:02}-{d:02}"),
        }
    }
    match c {
        Cell::DontCare => "-".into(),
        Cell::Nothing => "無し".into(),
        Cell::Lit(l) => format!("= {}", lit(l)),
        Cell::Set(ls) => format!("∈ {{{}}}", ls.iter().map(lit).collect::<Vec<_>>().join(", ")),
        Cell::Not(ls) => format!("∉ {{{}}}", ls.iter().map(lit).collect::<Vec<_>>().join(", ")),
        Cell::Cmp(cs) => cs
            .iter()
            .map(|(o, l)| {
                let op = match o {
                    CmpOp::Le => "<=",
                    CmpOp::Ge => ">=",
                    CmpOp::Lt => "<",
                    CmpOp::Gt => ">",
                };
                format!("{op} {}", lit(l))
            })
            .collect::<Vec<_>>()
            .join(" かつ "),
    }
}

fn row_key(r: &Row) -> String {
    r.cells.iter().map(cell_key).collect::<Vec<_>>().join("|")
}

fn pair_key(a: &Row, b: &Row) -> String {
    format!("{}\u{1}{}", row_key(a), row_key(b))
}

/// 出力セルが構文的に同一か。§4 の「同値」の判定に使う。
fn outs_equal(a: &Row, b: &Row) -> bool {
    if a.outs.len() != b.outs.len() {
        return false;
    }
    a.outs.iter().zip(&b.outs).all(|(x, y)| match (x, y) {
        (OutCell::Name(p), OutCell::Name(q)) => p == q,
        (OutCell::Lit(Lit::Num(p)), OutCell::Lit(Lit::Num(q))) => p == q,
        (OutCell::Lit(Lit::Word(p)), OutCell::Lit(Lit::Word(q))) => p == q,
        _ => false,
    })
}

/// E101 / E102 / E105 / W105 / W110 を出す。
pub fn check_table(t: &Table, c: &Checked, inputs: &[VarDecl], path: &str, budget: i64) -> TableCheck {
    let mut out = Vec::new();
    let mut quiet = Vec::new();
    let mut shadow = Shadow::default();
    let mut nodes = 0i64;
    let empty = TableCheck { diags: Vec::new(), quiet: Vec::new(), shadow, nodes: 0 };
    let Some(reg) = TableRegion::build(t, c, inputs) else { return empty };
    if reg.axes.is_empty() || t.rows.is_empty() {
        return empty;
    }
    let tname = t.name.as_ref().map(|n| n.text.clone()).unwrap_or_default();
    let at = |line: usize| format!("{path}:{line} 表 {tname}");
    let head_span: Span = t.name.as_ref().map(|n| n.span.clone()).unwrap_or(t.span.clone());

    // --- 重複と遮蔽
    let mut shadowed = vec![false; t.rows.len()];
    for i in 0..t.rows.len() {
        for j in (i + 1)..t.rows.len() {
            if !reg.intersects(i, j) {
                continue;
            }
            let mut sub = vec![vec![false; 0]; 0];
            let _ = &mut sub;
            let mut wpath = Vec::new();
            for ai in 0..reg.axes.len() {
                let c0 = (0..reg.axes[ai].len())
                    .find(|&c| reg.masks[i][ai][c] && reg.masks[j][ai][c])
                    .unwrap_or(0);
                wpath.push(c0);
            }
            let w = reg.witness_text(&wpath);
            let feas = reg.feasible(&wpath);
            // 実現不能と証明できた重なりは報告しない（§6.2）。
            if feas == Feasible::No {
                continue;
            }
            match t.policy {
                Policy::Unique if feas == Feasible::Unknown => {
                    // 証人を構成できなかった重なり。証明していないことを
                    // 証明済みの顔で出さない（§6.2）。生成コードの番人が対になる。
                    let dnames: Vec<String> = reg.derived_names();
                    let outs = |r: &Row| -> String {
                        r.outs
                            .iter()
                            .map(|o| match o {
                                OutCell::Name(n) => n.clone(),
                                OutCell::Lit(Lit::Num(n)) => n.raw.clone(),
                                OutCell::Lit(Lit::Word(n)) => n.clone(),
                                _ => String::new(),
                            })
                            .collect::<Vec<_>>()
                            .join(", ")
                    };
                    out.push(
                        Diag::warning(
                            "W114",
                            format!("未確認の重なり: 行{} と 行{} の両方に当たる入力が有り得ます", i + 1, j + 1),
                        )
                        .at(at(t.rows[j].span.line))
                        .mark(t.rows[i].span.clone(), format!("行{}", i + 1))
                        .mark(t.rows[j].span.clone(), format!("行{}", j + 1))
                        .note(format!("重なる条件: {}", reg.overlap_text(&t.rows[i], &t.rows[j])))
                        .note(format!(
                            "導出（{}）が入力を共有しているため、この条件を同時に満たす注文が存在するかどうかを、検査は判定できませんでした。",
                            dnames.join("、")
                        ))
                        .note(format!(
                            "存在するなら: 行を直してください。出力が異なる（{} と {}）ので、当たれば矛盾です。",
                            outs(&t.rows[i]),
                            outs(&t.rows[j])
                        ))
                        .note(format!(
                            "存在しないなら: このままで構いません。生成コードには、万一この条件に当たる入力が来たとき黙って 行{} を選ばずエラーを返す番人が入ります。",
                            i + 1
                        ))
                        .note("この警告は check --diff-base では新規分だけ表示されます。")
                        .key(pair_key(&t.rows[i], &t.rows[j])),
                    );
                }
                Policy::Unique => {
                    let same = t.rows[i].outs.len() == t.rows[j].outs.len();
                    // 定義列が交差に絡むなら、証人は定義の中身と突き合わせていない。
                    // 篩が定義軸を見るのは M1（§8.5）。それまでは黙らずに言う。
                    let unverified: Vec<String> = (0..reg.axes.len())
                        .filter(|&ai| {
                            reg.is_define[ai]
                                && (0..reg.axes[ai].len())
                                    .any(|c| reg.masks[i][ai][c] && reg.masks[j][ai][c])
                                && !(0..reg.axes[ai].len()).all(|c| reg.masks[i][ai][c] && reg.masks[j][ai][c])
                        })
                        .map(|ai| reg.col_names[ai].clone())
                        .collect();
                    out.push(
                        Diag::error("E105", format!("行の重なり: 同じ入力が 行{} と 行{} の両方に当たります", i + 1, j + 1))
                            .at(at(t.rows[j].span.line))
                            .mark(t.rows[i].span.clone(), format!("行{}", i + 1))
                            .mark(t.rows[j].span.clone(), format!("行{}", j + 1))
                            .note(format!("両方に当たる例: {w}"))
                            .note(if same {
                                "方式 一意 では重なりは許されません。どちらが正しいか決めるか、順序に意味を持たせるなら 方式 上から を宣言してください。".to_string()
                            } else {
                                "方式 一意 では重なりは許されません。".to_string()
                            })
                            .note(if unverified.is_empty() {
                                String::new()
                            } else {
                                format!(
                                    "なお、この例は定義（{}）の中身との整合を確認していません。",
                                    unverified.join("、")
                                )
                            }),
                    );
                }
                Policy::TopDown => {
                    shadowed[j] = true;
                    // §4 の三分類。包含の判定は領域で行う（軸ごとの射影ではなく、
                    // 行 i の領域から行 j の領域を引いて空かどうか）。篩は掛けない。
                    let contained = reg.contains(j, i);
                    nodes += (reg.axes.len() * 4) as i64;
                    let same_out = outs_equal(&t.rows[i], &t.rows[j]);
                    let d = Diag::warning(
                        "W105",
                        format!("行の重なり: 同じ入力が 行{} と 行{} の両方に当たります", i + 1, j + 1),
                    )
                    .at(at(t.rows[j].span.line))
                    .mark(t.rows[i].span.clone(), format!("行{}", i + 1))
                    .mark(t.rows[j].span.clone(), format!("行{}", j + 1))
                    .note(format!("両方に当たる例: {w}"))
                    .note(format!("方式 上から のため 行{} が勝ちます。意図通りですか。", i + 1))
                    .key(pair_key(&t.rows[i], &t.rows[j]));
                    if contained {
                        shadow.structural += 1;
                        quiet.push(d.note("行の領域が後の行に丸ごと含まれています（階段の通常の姿）。"));
                    } else if same_out {
                        shadow.equivalent += 1;
                        quiet.push(d.note("出力が同じなので、どちらが勝っても値は変わりません。"));
                    } else {
                        shadow.confirm += 1;
                        out.push(d.note(
                            "意図通りならこのままで構いません。CI の check --diff-base は新たに生じた分だけを報告します。",
                        ));
                    }
                }
            }
        }
    }

    // --- 死行
    for i in 0..t.rows.len() {
        let dead = if reg.empty(i) {
            true
        } else if t.policy == Policy::TopDown && i > 0 {
            // まず単独の先行行に含まれるかを見る。階段の死行はここで全部落ちるし、
            // 判定は軸ごとの部分集合なので安い。和との包含が要るのは、
            // 複数行が寄り集まって初めて覆う場合だけ。
            if (0..i).any(|e| reg.contains(e, i)) {
                nodes += (i * reg.axes.len() * 4) as i64;
                true
            } else {
                // 交わらない先行行は和に寄与しないので落とす。
                let earlier: Vec<usize> = (0..i).filter(|&e| reg.intersects(e, i)).collect();
                if earlier.is_empty() {
                    false
                } else {
                    let mut b = budget;
                    let r = reg.hole_within(&earlier, i, &mut b).is_none() && b >= 0;
                    nodes += budget - b;
                    r
                }
            }
        } else {
            false
        };
        if dead {
            out.push(
                Diag::error("E102", format!("冗長な行: 行{} は決して当たりません", i + 1))
                    .at(at(t.rows[i].span.line))
                    .mark(t.rows[i].span.clone(), format!("行{}: ここに到達する入力はありません", i + 1))
                    .note(if reg.unreachable_row[i] {
                        "この行が名指ししている値を、上流の表は決して出しません。".to_string()
                    } else if t.policy == Policy::TopDown {
                        "方式 上から のため、この行の範囲は先行する行がすべて先に取ります。".to_string()
                    } else {
                        "この行の条件を同時に満たす入力がありません。".to_string()
                    })
                    .note(if reg.unreachable_row[i] {
                        "ヒント: 上流の表がこの値を出すようにするか、この行を削除してください。".to_string()
                    } else {
                        "ヒント: 新しい仕様なら上へ移してください。不要なら削除してください。".to_string()
                    }),
            );
        }
    }

    // --- 完全性
    let mut left = budget;
    let all: Vec<usize> = (0..t.rows.len()).collect();
    let hole = reg.find_hole(&all, &mut left);
    nodes += budget - left;
    if let Some(hole) = hole {
        out.push(
            Diag::error("E101", "完全性の欠落: どの行にも当たらない入力があります")
                .at(at(head_span.line))
                .mark(head_span.clone(), "入力空間を覆いきっていません")
                .note(format!("当たらない例: {}", reg.witness_text(&hole)))
                .note("ヒント: この入力に当たる行を足してください。"),
        );
    } else if left < 0 {
        out.push(
            Diag::error("E109", "検査の予算を超えたので、完全性を証明できませんでした")
                .at(at(head_span.line))
                .mark(head_span.clone(), "")
                .note(format!("支配的なのは {}。", reg.dominant_axes()))
                .note("列を群でまとめるか、表を分けてください（§6.3）。近似では通しません。"),
        );
    }

    // --- W110: 重なりのない 上から
    if t.policy == Policy::TopDown && !shadowed.iter().any(|x| *x) && t.rows.len() > 1 {
        out.push(
            Diag::warning("W110", "この表には重なりがありません")
                .at(at(head_span.line))
                .mark(head_span.clone(), "")
                .note("方式 一意 にすると、行の並べ替えが意味を変えないことを検査が保証します。"),
        );
    }
    TableCheck { diags: out, quiet, shadow, nodes }
}

impl TableRegion {
    /// 行 `target` の領域のうち、`rows` に覆われていない座標を探す。
    fn hole_within(&self, rows: &[usize], target: usize, budget: &mut i64) -> Option<Vec<usize>> {
        let mut path = Vec::new();
        self.within_rec(rows, target, 0, budget, &mut path)
    }

    fn within_rec(
        &self,
        rows: &[usize],
        target: usize,
        ai: usize,
        budget: &mut i64,
        path: &mut Vec<usize>,
    ) -> Option<Vec<usize>> {
        *budget -= 1;
        if *budget < 0 {
            return None;
        }
        if rows.is_empty() {
            let mut p = path.clone();
            while p.len() < self.axes.len() {
                p.push(0);
            }
            return Some(p);
        }
        if ai == self.axes.len() {
            return None;
        }
        if rows
            .iter()
            .any(|&r| self.masks[r][ai..].iter().all(|m| m.iter().all(|x| *x)))
        {
            return None;
        }
        for c in 0..self.axes[ai].len() {
            if !self.masks[target][ai][c] {
                continue;
            }
            let sub: Vec<usize> = rows.iter().copied().filter(|&r| self.masks[r][ai][c]).collect();
            path.push(c);
            if let Some(h) = self.within_rec(&sub, target, ai + 1, budget, path) {
                path.pop();
                return Some(h);
            }
            path.pop();
        }
        None
    }
}

impl TableRegion {
    fn coord_span(&self, ai: usize, ci: usize) -> Option<(Option<Rat>, Option<Rat>)> {
        match &self.axes[ai] {
            Axis::Num { coords, .. } => match coords.get(ci)? {
                Coord::Point(v) => Some((Some(*v), Some(*v))),
                Coord::Open(a, b) => Some((*a, *b)),
            },
            _ => None,
        }
    }

    /// §6.2 の篩。導出軸ごとに到達区間と座標の区間が交わるかを見る。交わらなければ
    /// 実現不能。制約された導出が二本以上あって入力を共有していれば従属を見られない。
    pub fn feasible(&self, path: &[usize]) -> Feasible {
        let mut constrained: Vec<(usize, Vec<String>)> = Vec::new();
        for (ai, d) in self.derived.iter().enumerate() {
            let Some(((rl, rh), deps)) = d else { continue };
            let Some(&ci) = path.get(ai) else { continue };
            let Some((cl, ch)) = self.coord_span(ai, ci) else { continue };
            let disjoint = matches!((ch, rl), (Some(a), Some(b)) if a.cmp_to(*b) == std::cmp::Ordering::Less)
                || matches!((cl, rh), (Some(a), Some(b)) if a.cmp_to(*b) == std::cmp::Ordering::Greater);
            if disjoint {
                return Feasible::No;
            }
            if !(cl.is_none() && ch.is_none()) {
                constrained.push((ai, deps.clone()));
            }
        }
        for i in 0..constrained.len() {
            for j in (i + 1)..constrained.len() {
                if constrained[i].1.iter().any(|x| constrained[j].1.contains(x)) {
                    return Feasible::Unknown;
                }
            }
        }
        Feasible::Yes
    }
}

impl TableRegion {
    pub fn derived_names(&self) -> Vec<String> {
        self.col_names
            .iter()
            .enumerate()
            .filter(|(i, _)| self.derived[*i].is_some())
            .map(|(_, n)| n.clone())
            .collect()
    }
}

impl TableRegion {
    /// §6.3: 予算を使い切ったとき、どの列の境界数が支配的かを添える。
    /// 「群でまとめる」「表を分ける」のどちらを狙うかが、これで決まる。
    pub fn dominant_axes(&self) -> String {
        let mut v: Vec<(usize, usize)> = (0..self.axes.len()).map(|i| (self.axes[i].len(), i)).collect();
        v.sort_by(|a, b| b.0.cmp(&a.0));
        v.iter()
            .take(3)
            .map(|(n, i)| format!("{}（{n} 区分）", self.col_names[*i]))
            .collect::<Vec<_>>()
            .join("、")
    }
}
