//! Comparison results and how they are presented (§10.3, §10.4).
//!
//! "Rule vs legacy implementation" (verify), "rule vs past observations" (replay) and
//! "one version of a rule vs another" (diff) differ only in what the rule is compared
//! against; the shape — **cluster by fired row and report counts, amounts and witnesses**
//! — is the same. It lives here once and all three use it.
//!
//! The denominator of the match rate is always taken from the observed records alone
//! (§10.3). The whole point of this design is to cut off, at the level of the format, the
//! temptation to inflate the rate by mixing in filled records, so the place that adds to
//! the denominator is confined to one spot. On top of that, **the report itself always
//! records the default values used and the number of filled records per field**. A report
//! pasted into a PR becomes the record of what justified the fill.

use crate::ast::RuleFile;
use crate::eval::Val;
use crate::types::Checked;
use crate::vectors;
use std::collections::BTreeMap;

pub struct Mismatch {
    pub id: usize,
    /// The record's label (e.g. `order:1234567`). Empty when there is none.
    pub tag: String,
    pub input: BTreeMap<String, Val>,
    /// (output name, our value, the counterpart's value) in declaration order. With several
    /// outputs, all of them are listed here.
    pub outs: Vec<(String, Option<Val>, Option<String>)>,
    pub err: Option<String>,
    /// The fired rows that form the cluster key. diff stores "old → new" as one string.
    pub trace: Vec<String>,
}

impl Mismatch {
    /// Only the outputs that differ. When the counterpart gave no answer, all of them count
    /// as differing.
    pub fn differing(&self) -> Vec<&(String, Option<Val>, Option<String>)> {
        self.outs.iter().filter(|(_, a, b)| wire(a.as_ref()) != *b).collect()
    }
}

/// A value in its wire representation. JSON numbers are integers in the canonical unit
/// (§10.2).
pub fn wire(v: Option<&Val>) -> Option<String> {
    v.map(|o| match o {
        Val::Num(r) => format!("{}", r.num / r.den),
        Val::Bool(b) => format!("{b}"),
        other => vectors::show(other),
    })
}

pub struct Report {
    /// Whether there are two or more outputs. Decides whether amount differences carry the
    /// output name.
    pub multi: bool,
    /// Number of observed records compared. Filled records are not counted here (§10.3).
    pub total: usize,
    pub agreed: usize,
    /// Number of records the counterpart could not answer. Excluded from the denominator and
    /// reported with the reason.
    pub errored: usize,
    pub mismatches: Vec<Mismatch>,
    /// Identity of the counterpart (`legacy/shipping.py@a1b2c3d`, `replay/2025-08.jsonl`,
    /// `送料@v3`).
    pub impl_id: String,
    /// The word that names the counterpart in a witness: verify uses "旧" (legacy), replay
    /// "観測" (observed), diff "旧版" (old version).
    pub theirs: String,
    /// Filled records: the count per field, and the default values used (§10.3).
    pub filled: BTreeMap<String, usize>,
    pub fills_used: BTreeMap<String, String>,
    pub filled_total: usize,
    pub filled_agreed: usize,
    /// Number of records excluded for a broken format, with the reason.
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
    /// The headline match rate is computed from observed records only (§10.3).
    /// Records the counterpart declared unsupported are excluded from the denominator.
    pub fn rate(&self) -> f64 {
        let n = self.total - self.errored;
        if n == 0 {
            return 0.0;
        }
        self.agreed as f64 / n as f64
    }
}

/// Thousands separators. An amount means nothing if the reader cannot count its digits.
/// The sign is always written (for a difference, the direction is the substance).
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

/// The word after a record count: `件` in Japanese, `record`/`records` in English.
fn records(n: usize) -> &'static str {
    if crate::i18n::ja() { "件" } else if n == 1 { "record" } else { "records" }
}

/// Statistics of Δ within a cluster (§10.4). Always carries the count, the total and the
/// min/max.
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
    /// Folded into one line when every record has the same value. Otherwise the min/max are
    /// shown, so that the very fact of the spread is visible (this recovers what is given up
    /// by not splitting the key on the amount; §10.4).
    fn text(&self) -> String {
        if self.uniform() {
            tr!(
                "差 {} 一様  合計 {}",
                "difference {} uniform  total {}",
                group(self.lo),
                group(self.sum)
            )
        } else {
            tr!(
                "合計 {}  Δ {}..{}",
                "total {}  Δ {}..{}",
                group(self.sum),
                group(self.lo),
                group(self.hi)
            )
        }
    }
}

/// Per cluster: output name → Δ statistics.
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
            Some(tr!(
                "規則 {n}={} / {theirs} {n}={b}",
                "rule {n}={} / {theirs} {n}={b}",
                vectors::show(a)
            ))
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
        // §10.4: the key is the set of fired rows only; the legacy output is not part of it.
        // For table-lookup rows the set of rows already fixes the set of amounts, so nothing
        // would be gained, and for computed outputs the clusters would split once per distinct
        // value and the summary would die. What the split would show is recovered by the
        // min/max of Δ.
        let key = match &m.err {
            Some(e) => tr!("答えられない: {e}", "could not answer: {e}"),
            None => m.trace.join(" / "),
        };
        out.entry(key).or_default().push(m);
    }
    out
}

/// The record of fills and exclusions (§10.3). Wherever a number appears, its basis is
/// shown next to it.
fn provenance(rep: &Report) -> Vec<String> {
    let mut o = Vec::new();
    if rep.errored > 0 {
        o.push(tr!(
            "相手が答えられなかった {} 件は、一致率の分母から外しています（§10.3）",
            "The counterpart could not answer {} of the records; those are excluded from the match-rate denominator (§10.3)",
            rep.errored
        ));
    }
    for (why, n) in &rep.excluded {
        let unit = records(*n);
        o.push(tr!("{why}記録を {n} {unit}外しました", "Excluded {n} {unit} ({why})"));
    }
    if rep.filled_total > 0 {
        let by: Vec<String> =
            rep.filled.iter().map(|(n, k)| tr!("{n} {k} 件", "{n}: {k}")).collect();
        o.push(tr!(
            "補完系 {} 件（{}）。一致 {} 件。見出しの一致率には入れていません",
            "Filled records: {} ({}); matched {}. Not included in the headline match rate",
            rep.filled_total,
            by.join(if crate::i18n::ja() { "、" } else { ", " }),
            rep.filled_agreed
        ));
        let used: Vec<String> =
            rep.fills_used.iter().map(|(n, v)| format!("{n} = {v}")).collect();
        if !used.is_empty() {
            o.push(tr!("使った既定値: {}", "Default values used: {}", used.join(", ")));
        }
    }
    o
}

/// The §10.4 headline. Not just the count: the total amount that moves is always attached.
/// "How many records move" and "how much money moves" are different questions, and it is
/// the latter that sways an approval.
fn impact(rep: &Report) -> String {
    let n = rep.total - rep.errored;
    let pct = if n == 0 { 0.0 } else { rep.mismatches.len() as f64 * 100.0 / n as f64 };
    let all: Vec<&Mismatch> = rep.mismatches.iter().collect();
    let money: Vec<String> = deltas(&all)
        .iter()
        .map(|(name, d)| {
            let label = if rep.multi { format!("{name} ") } else { String::new() };
            tr!("  金額 {label}{}", "  amount {label}{}", group(d.sum))
        })
        .collect();
    tr!(
        "影響 {} 件 ({pct:.3}%){}",
        "Affected {} ({pct:.3}%){}",
        rep.mismatches.len(),
        money.join("")
    )
}

pub fn render(rep: &Report, f: &RuleFile, c: &Checked) -> String {
    let mut o = tr!(
        "照合 {} 件 / 一致 {} ({:.3}%)\n",
        "Compared {} / matched {} ({:.3}%)\n",
        rep.total,
        rep.agreed,
        rep.rate() * 100.0
    );
    if !rep.impl_id.is_empty() {
        o.push_str(&tr!("相手: {}\n", "Counterpart: {}\n", rep.impl_id));
    }
    for l in provenance(rep) {
        o.push_str(&l);
        o.push('\n');
    }
    if rep.mismatches.is_empty() {
        o.push_str(&tr!("不一致はありません。\n", "No mismatches.\n"));
        return o;
    }
    o.push_str(&format!("\n{}\n", impact(rep)));
    for (k, ms) in &cluster(rep) {
        let money = money_text(&deltas(ms), rep.multi);
        o.push_str(&format!("  {:<48} {:>5} {}{money}\n", k, ms.len(), records(ms.len())));
        if let Some(q) = sub_grid(ms, f, c) {
            // §10.4: a cluster made up solely of differences below the output grid is most
            // likely a difference in rounding convention, not in the values themselves. Flag
            // it automatically.
            o.push_str(&tr!(
                "    丸め差異の疑い（出力格子 {q} 未満の端数のみ）\n",
                "    Suspected rounding difference (only fractions below the output grid {q})\n"
            ));
        }
        o.push_str(&tr!("    例: {}\n", "    Example: {}\n", witness(ms[0], &rep.theirs)));
    }
    o
}

/// Markdown to paste into a PR (§12). Posting is left to one line of CI; the tool owns only
/// the formatting.
pub fn markdown(rep: &Report, f: &RuleFile, c: &Checked, title: &str) -> String {
    let esc = |s: &str| s.replace('|', "\\|");
    let mut o = tr!(
        "### 規則 {} v{} — {title}\n\n",
        "### Rule {} v{} — {title}\n\n",
        f.name.text,
        f.version
    );
    o.push_str("| | |\n|---|---:|\n");
    o.push_str(&tr!(
        "| 照合（実測系） | {} 件 |\n",
        "| Compared (observed records) | {} |\n",
        rep.total - rep.errored
    ));
    o.push_str(&tr!(
        "| 一致 | {} 件 ({:.3}%) |\n",
        "| Matched | {} ({:.3}%) |\n",
        rep.agreed,
        rep.rate() * 100.0
    ));
    o.push_str(&tr!("| 不一致 | {} 件 |\n", "| Mismatches | {} |\n", rep.mismatches.len()));
    if !rep.impl_id.is_empty() {
        o.push_str(&tr!("| 相手 | `{}` |\n", "| Counterpart | `{}` |\n", esc(&rep.impl_id)));
    }
    let prov = provenance(rep);
    if !prov.is_empty() {
        o.push('\n');
        for l in &prov {
            o.push_str(&format!("- {}\n", esc(l)));
        }
    }
    if rep.mismatches.is_empty() {
        o.push_str(&tr!("\n不一致はありません。\n", "\nNo mismatches.\n"));
        return o;
    }
    o.push_str(&format!("\n**{}**\n", impact(rep)));
    o.push_str(&tr!(
        "\n#### 不一致の内訳\n\n| 発火行 | 件数 | 差 | 証人 |\n|---|---:|---|---|\n",
        "\n#### Mismatch breakdown\n\n| Fired rows | Count | Difference | Witness |\n|---|---:|---|---|\n"
    ));
    for (k, ms) in &cluster(rep) {
        let mut money = money_text(&deltas(ms), rep.multi).trim().to_string();
        if let Some(q) = sub_grid(ms, f, c) {
            money.push_str(&tr!(
                "<br>丸め差異の疑い（格子 {q} 未満）",
                "<br>suspected rounding difference (below grid {q})"
            ));
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

/// §10.4: a suspected rounding difference is when every record in the cluster shows a
/// "non-zero difference smaller than the output grid". Returns the grid as displayed
/// (e.g. `10円`). If even one record is at or above the grid, or cannot be compared
/// numerically, the cluster is not flagged. The decision is a conjunction over the whole
/// cluster so that values that really differ are never blamed on rounding. With several
/// outputs, every differing output must be below its grid.
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
            // Compare |d| < q in integers by clearing the denominator.
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
