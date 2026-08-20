//! 突き合わせの結果と、その読ませ方（§10.3、§10.4）。
//!
//! 「規則 vs 旧実装」（verify）と「規則 vs 過去の観測」（replay）と
//! 「規則の版どうし」（diff）は、比べる相手が違うだけで、**発火行でクラスタして
//! 件数・金額・証人を出す**という形は同じである。ここに一度だけ置いて三者で使う。
//!
//! 一致率の分母は必ず実測系だけから取る（§10.3）。補完を混ぜて大きく見せる誘惑を、
//! 形式のレベルで断つのがこの設計の眼目なので、加算する場所を一箇所に閉じてある。
//! そのうえで、**使った既定値と欄ごとの補完件数はレポート自身が必ず刻む**。
//! PR に貼られたレポートが、補完の根拠の記録になる。

use crate::ast::RuleFile;
use crate::eval::Val;
use crate::types::Checked;
use crate::vectors;
use std::collections::BTreeMap;

pub struct Mismatch {
    pub id: usize,
    /// 記録の見出し（`order:1234567` など）。無ければ空。
    pub tag: String,
    pub input: BTreeMap<String, Val>,
    /// 宣言順に (出力名, こちらの値, 相手の値)。複数出力ならここに全部並ぶ。
    pub outs: Vec<(String, Option<Val>, Option<String>)>,
    pub err: Option<String>,
    /// クラスタの鍵になる発火行。diff は「旧 → 新」を一本の文字列にして入れる。
    pub trace: Vec<String>,
}

impl Mismatch {
    /// 食い違っている出力だけ。相手が答えなかった場合は全部を差分として扱う。
    pub fn differing(&self) -> Vec<&(String, Option<Val>, Option<String>)> {
        self.outs.iter().filter(|(_, a, b)| wire(a.as_ref()) != *b).collect()
    }
}

/// 値をワイヤの表現へ。JSON の数は正準単位の整数（§10.2）。
pub fn wire(v: Option<&Val>) -> Option<String> {
    v.map(|o| match o {
        Val::Num(r) => format!("{}", r.num / r.den),
        Val::Bool(b) => format!("{b}"),
        other => vectors::show(other),
    })
}

pub struct Report {
    /// 出力が二つ以上あるか。金額差に出力名を添えるかどうかがこれで決まる。
    pub multi: bool,
    /// 実測系の照合件数。補完系はここに入れない（§10.3）。
    pub total: usize,
    pub agreed: usize,
    /// 相手が答えられなかった件数。分母から外し、理由つきで報告する。
    pub errored: usize,
    pub mismatches: Vec<Mismatch>,
    /// 相手の識別（`legacy/shipping.py@a1b2c3d`、`replay/2025-08.jsonl`、`送料@v3`）。
    pub impl_id: String,
    /// 見出しの語。verify は「旧」、replay は「観測」、diff は「旧版」。
    pub theirs: String,
    /// 補完系の欄ごとの件数と、使った既定値（§10.3）。
    pub filled: BTreeMap<String, usize>,
    pub fills_used: BTreeMap<String, String>,
    pub filled_total: usize,
    pub filled_agreed: usize,
    /// 形式が壊れていて外した件数と、その理由。
    pub excluded: Vec<(String, usize)>,
}

impl Report {
    pub fn new(f: &RuleFile, theirs: &str) -> Report {
        Report {
            multi: f.outputs.len() > 1,
            total: 0,
            agreed: 0,
            errored: 0,
            mismatches: Vec::new(),
            impl_id: String::new(),
            theirs: theirs.into(),
            filled: BTreeMap::new(),
            fills_used: BTreeMap::new(),
            filled_total: 0,
            filled_agreed: 0,
            excluded: Vec::new(),
        }
    }
    /// 見出しの一致率は実測系だけから計算する（§10.3）。
    /// 相手が「対応していない」と言った件は分母から外す。
    pub fn rate(&self) -> f64 {
        let n = self.total - self.errored;
        if n == 0 {
            return 0.0;
        }
        self.agreed as f64 / n as f64
    }
}

/// 三桁区切り。金額は読み手が桁を数えられないと意味を持たない。
/// 符号は必ず付ける（差は向きが本体なので）。
fn group(n: i128) -> String {
    let d = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, ch) in d.chars().enumerate() {
        if i > 0 && (d.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    format!("{}{out}", if n < 0 { "-" } else { "+" })
}

/// クラスタ内の Δ の統計（§10.4）。件数・合計・最小最大を必ず持つ。
struct Delta {
    n: usize,
    sum: i128,
    lo: i128,
    hi: i128,
}

impl Delta {
    fn push(&mut self, d: i128) {
        if self.n == 0 {
            self.lo = d;
            self.hi = d;
        } else {
            self.lo = self.lo.min(d);
            self.hi = self.hi.max(d);
        }
        self.n += 1;
        self.sum += d;
    }
    fn uniform(&self) -> bool {
        self.lo == self.hi
    }
    /// 全件同値なら一行に畳む。一様でなければ最小最大を出して、割れているという
    /// 事実そのものを見せる（鍵を金額で割らない代わりの回収。§10.4）。
    fn text(&self) -> String {
        if self.uniform() {
            format!("差 {} 一様  合計 {}", group(self.lo), group(self.sum))
        } else {
            format!("合計 {}  Δ {}..{}", group(self.sum), group(self.lo), group(self.hi))
        }
    }
}

/// クラスタごとの、出力名 → Δ 統計。
fn deltas(ms: &[&Mismatch]) -> BTreeMap<String, Delta> {
    let mut out: BTreeMap<String, Delta> = BTreeMap::new();
    for m in ms {
        for (n, a, b) in m.differing() {
            let (Some(Val::Num(a)), Some(b)) = (a, b) else { continue };
            let Ok(b) = b.parse::<i128>() else { continue };
            out.entry(n.clone())
                .or_insert(Delta { n: 0, sum: 0, lo: 0, hi: 0 })
                .push((a.num / a.den) - b);
        }
    }
    out.retain(|_, d| d.n > 0);
    out
}

fn money_text(ds: &BTreeMap<String, Delta>, multi: bool) -> String {
    ds.iter()
        .map(|(n, d)| {
            let label = if multi { format!("{n} ") } else { String::new() };
            format!("  {label}{}", d.text())
        })
        .collect()
}

fn witness(ex: &Mismatch, theirs: &str) -> String {
    let inp: Vec<String> =
        ex.input.iter().map(|(n, v)| format!("{n}={}", vectors::show(v))).collect();
    let diff: Vec<String> = ex
        .differing()
        .iter()
        .filter_map(|(n, a, b)| {
            let (a, b) = (a.as_ref()?, b.as_ref()?);
            Some(format!("規則 {n}={} / {theirs} {n}={b}", vectors::show(a)))
        })
        .collect();
    if diff.is_empty() {
        inp.join(", ")
    } else {
        format!("{} → {}", inp.join(", "), diff.join(", "))
    }
}

fn cluster<'a>(rep: &'a Report) -> BTreeMap<String, Vec<&'a Mismatch>> {
    let mut out: BTreeMap<String, Vec<&Mismatch>> = BTreeMap::new();
    for m in &rep.mismatches {
        // §10.4: 鍵は発火行の組だけ。旧出力は入れない。表引きの行なら行の組が
        // 金額の組を一意に決めるので情報が増えず、計算出力では値の数だけ
        // クラスタが割れて要約が死ぬ。割れの利益は Δ の最小最大で回収する。
        let key = match &m.err {
            Some(e) => format!("答えられない: {e}"),
            None => m.trace.join(" / "),
        };
        out.entry(key).or_default().push(m);
    }
    out
}

/// 補完と除外の刻印（§10.3）。数字の出る場所に根拠を必ず出す。
fn provenance(rep: &Report) -> Vec<String> {
    let mut o = Vec::new();
    if rep.errored > 0 {
        o.push(format!(
            "相手が答えられなかった {} 件は、一致率の分母から外しています（§10.3）",
            rep.errored
        ));
    }
    for (why, n) in &rep.excluded {
        o.push(format!("{why}記録を {n} 件外しました"));
    }
    if rep.filled_total > 0 {
        let by: Vec<String> =
            rep.filled.iter().map(|(n, k)| format!("{n} {k} 件")).collect();
        o.push(format!(
            "補完系 {} 件（{}）。一致 {} 件。見出しの一致率には入れていません",
            rep.filled_total,
            by.join("、"),
            rep.filled_agreed
        ));
        let used: Vec<String> =
            rep.fills_used.iter().map(|(n, v)| format!("{n} = {v}")).collect();
        if !used.is_empty() {
            o.push(format!("使った既定値: {}", used.join(", ")));
        }
    }
    o
}

/// §10.4 の見出し。件数だけでなく、動く金額の合計を必ず添える。
/// 「何件動くか」と「いくら動くか」は別の問いで、決裁に効くのは後者である。
fn impact(rep: &Report) -> String {
    let n = rep.total - rep.errored;
    let pct = if n == 0 { 0.0 } else { rep.mismatches.len() as f64 * 100.0 / n as f64 };
    let all: Vec<&Mismatch> = rep.mismatches.iter().collect();
    let money: Vec<String> = deltas(&all)
        .iter()
        .map(|(name, d)| {
            let label = if rep.multi { format!("{name} ") } else { String::new() };
            format!("  金額 {label}{}", group(d.sum))
        })
        .collect();
    format!("影響 {} 件 ({pct:.3}%){}", rep.mismatches.len(), money.join(""))
}

pub fn render(rep: &Report, f: &RuleFile, c: &Checked) -> String {
    let mut o = format!(
        "照合 {} 件 / 一致 {} ({:.3}%)\n",
        rep.total,
        rep.agreed,
        rep.rate() * 100.0
    );
    if !rep.impl_id.is_empty() {
        o.push_str(&format!("相手: {}\n", rep.impl_id));
    }
    for l in provenance(rep) {
        o.push_str(&l);
        o.push('\n');
    }
    if rep.mismatches.is_empty() {
        o.push_str("不一致はありません。\n");
        return o;
    }
    o.push_str(&format!("\n{}\n", impact(rep)));
    for (k, ms) in &cluster(rep) {
        let money = money_text(&deltas(ms), rep.multi);
        o.push_str(&format!("  {:<48} {:>5} 件{money}\n", k, ms.len()));
        if let Some(q) = sub_grid(ms, f, c) {
            // §10.4: 出力格子未満のずれだけで固まっているクラスタは、値そのものの
            // 食い違いではなく丸めの規約差である見込みが高い。自動で括る。
            o.push_str(&format!("    丸め差異の疑い（出力格子 {q} 未満の端数のみ）\n"));
        }
        o.push_str(&format!("    例: {}\n", witness(ms[0], &rep.theirs)));
    }
    o
}

/// PR に貼るための Markdown（§12）。投稿は CI の一行に任せ、整形だけ道具が持つ。
pub fn markdown(rep: &Report, f: &RuleFile, c: &Checked, title: &str) -> String {
    let esc = |s: &str| s.replace('|', "\\|");
    let mut o = format!("### 規則 {} v{} — {title}\n\n", f.name.text, f.version);
    o.push_str("| | |\n|---|---:|\n");
    o.push_str(&format!("| 照合（実測系） | {} 件 |\n", rep.total - rep.errored));
    o.push_str(&format!("| 一致 | {} 件 ({:.3}%) |\n", rep.agreed, rep.rate() * 100.0));
    o.push_str(&format!("| 不一致 | {} 件 |\n", rep.mismatches.len()));
    if !rep.impl_id.is_empty() {
        o.push_str(&format!("| 相手 | `{}` |\n", esc(&rep.impl_id)));
    }
    let prov = provenance(rep);
    if !prov.is_empty() {
        o.push('\n');
        for l in &prov {
            o.push_str(&format!("- {}\n", esc(l)));
        }
    }
    if rep.mismatches.is_empty() {
        o.push_str("\n不一致はありません。\n");
        return o;
    }
    o.push_str(&format!("\n**{}**\n", impact(rep)));
    o.push_str("\n#### 不一致の内訳\n\n| 発火行 | 件数 | 差 | 証人 |\n|---|---:|---|---|\n");
    for (k, ms) in &cluster(rep) {
        let mut money = money_text(&deltas(ms), rep.multi).trim().to_string();
        if let Some(q) = sub_grid(ms, f, c) {
            money.push_str(&format!("<br>丸め差異の疑い（格子 {q} 未満）"));
        }
        o.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            esc(k),
            ms.len(),
            esc(&money),
            esc(&witness(ms[0], &rep.theirs))
        ));
    }
    o
}

/// §10.4: クラスタの全件が「出力格子より小さい、ゼロでないずれ」なら丸め差異の疑い。
/// 返すのは格子の表示（`10円` など）。一件でも格子以上、あるいは数値で比べられない
/// 件が混ざっていれば括らない。値が本当に違うものを丸めのせいにしないため、
/// 判定はクラスタ全体の連言にしてある。複数出力なら、食い違っている出力が
/// すべて格子未満であることを求める。
fn sub_grid(ms: &[&Mismatch], f: &RuleFile, c: &Checked) -> Option<String> {
    let mut grids: BTreeMap<String, String> = BTreeMap::new();
    for m in ms {
        let diff = m.differing();
        if diff.is_empty() {
            return None;
        }
        for (n, a, b) in diff {
            let od = f.outputs.iter().find(|o| o.name.text == *n)?;
            let rd = od.rounding.as_ref()?;
            let ty = c.ty_of(n)?;
            let q = crate::types::lit_value_in_pub(&rd.grid, &ty)?;
            if q.num <= 0 {
                return None;
            }
            let (Some(Val::Num(a)), Some(b)) = (a, b) else { return None };
            let d = (a.num / a.den) - b.parse::<i128>().ok()?;
            // |d| < q を分母を払って整数で比べる。
            if d == 0 || d.abs() * q.den >= q.num {
                return None;
            }
            grids.insert(n.clone(), rd.grid.raw.clone());
        }
    }
    if grids.is_empty() {
        return None;
    }
    Some(grids.values().cloned().collect::<Vec<_>>().join(" / "))
}
