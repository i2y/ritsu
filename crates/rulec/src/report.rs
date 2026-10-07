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
use crate::num::Rat;
use crate::types::Checked;
use crate::vectors;
use std::collections::BTreeMap;

/// A row that fired, as data. The cluster key is built from these and the label is rendered
/// from them, rather than the other way round: `row_tag` is prose and prose may change (§11
/// principle 5), so nothing downstream is allowed to take it apart.
#[derive(Debug, Clone, PartialEq)]
pub enum Fired {
    /// One side fired this row (`verify`, `replay`).
    One { table: String, row: usize },
    /// How the fired row moved between two versions (`diff`). `None` on a side means that
    /// version's table did not fire at all.
    Moved { table: String, from: Option<usize>, to: Option<usize> },
}

impl Fired {
    /// The form the text and markdown renderings show.
    pub fn label(&self) -> String {
        match self {
            Fired::One { table, row } => crate::eval::row_tag(table, *row),
            Fired::Moved { table, from: Some(a), to: Some(b) } if a == b => {
                crate::eval::row_tag(table, *a)
            }
            Fired::Moved { table, from: Some(a), to: Some(b) } => {
                tr!("{}→行{b}", "{}→row {b}", crate::eval::row_tag(table, *a))
            }
            Fired::Moved { table, from: Some(a), to: None } => {
                tr!("{}（旧のみ）", "{} (old only)", crate::eval::row_tag(table, *a))
            }
            Fired::Moved { table, from: None, to: Some(b) } => {
                tr!("{}（新のみ）", "{} (new only)", crate::eval::row_tag(table, *b))
            }
            Fired::Moved { table, .. } => table.clone(),
        }
    }

    /// `{"table":…,"row":…}` for one side, `{"table":…,"from":…,"to":…}` for a transition
    /// (docs/formats.md).
    pub fn json(&self) -> String {
        let n = |v: Option<usize>| match v {
            Some(v) => v.to_string(),
            None => "null".to_string(),
        };
        match self {
            Fired::One { table, row } => {
                crate::json::Obj::new().str("table", table).int("row", *row as i128).finish()
            }
            Fired::Moved { table, from, to } => crate::json::Obj::new()
                .str("table", table)
                .raw("from", n(*from))
                .raw("to", n(*to))
                .finish(),
        }
    }
}

pub struct Mismatch {
    /// Where the record came in, 1-based: the line of the fixtures file for `replay` and
    /// `diff`, the vector's place in the stream for `verify`. It is what names a record
    /// when the record carries no `tag` of its own.
    pub line: usize,
    /// The record's label (e.g. `order:1234567`). Empty when there is none.
    pub tag: String,
    pub input: BTreeMap<String, Val>,
    /// Every output in declaration order, the rule's answer beside the counterpart's. With
    /// several outputs, all of them are listed here.
    pub outs: Vec<Out>,
    pub err: Option<String>,
    /// The rows that fired. They form the cluster key.
    pub fired: Vec<Fired>,
    /// The inputs as `diff` shows them, made when the record was read: (name, as a person
    /// reads it, as the wire writes it). A record is read the way the old version reads it,
    /// and an input the new version counts in another unit, or does not take at all, is
    /// shown the way the record has it (§15.197). Empty for `verify` and `replay`, which show
    /// `input` in the rule's own terms.
    pub shown: Vec<(String, String, String)>,
    /// `diff` only: why the new version does not take this record's input. `outs` then holds
    /// the old version's answers alone.
    pub refusal: Option<Refusal>,
}

/// Why the new version does not take a record's input (§15.197): a stable kind for
/// `--format json`, the input at fault (empty for a constraint, which is about two), and the
/// reason as prose.
#[derive(Debug, Clone)]
pub struct Refusal {
    pub kind: &'static str,
    pub field: String,
    pub what: String,
}

/// One output of one record: the rule's answer beside the counterpart's (§15.199).
#[derive(Debug, Clone)]
pub struct Out {
    pub name: String,
    /// The rule's answer (for `diff`, the new version's), in the rule's own terms.
    pub ours: Option<Val>,
    /// The counterpart's answer as a value in the rule's terms, exactly: the old version's
    /// brought to the new version's unit, the record's or the adapter's as read. None when it
    /// gave none, or gave something that is no value of the rule's type.
    pub theirs: Option<Val>,
    /// The counterpart's answer as its own wire writes it, for `--format json`: the old
    /// version's step and unit for `diff` (as `diff` without records writes `old`), the rule's
    /// wire unit for a record — a decimal where the record held one — and the adapter's text
    /// as it came for `verify`.
    pub theirs_wire: Option<String>,
    /// The same as a person reads it, in the counterpart's own unit.
    pub theirs_shown: Option<String>,
    /// The rule's answer as a person reads it.
    pub ours_shown: Option<String>,
    /// Whether the two are the same value: exactly, never at one side's step.
    pub same: bool,
}

impl Out {
    /// One output of `replay` or `verify`: both answers in the rule's terms. `written` is the
    /// adapter's text as it came, where there is one.
    pub fn against(c: &Checked, name: &str, ours: Option<Val>, theirs: Option<Val>, written: Option<String>) -> Out {
        let same = same_answer(name, ours.as_ref(), c, theirs.as_ref(), c);
        let theirs_shown = theirs.as_ref().map(|v| shown(c, name, v)).or_else(|| written.clone());
        let theirs_wire = written.or_else(|| wire_exact(c, name, theirs.as_ref()));
        let ours_shown = ours.as_ref().map(|v| shown(c, name, v));
        Out { name: name.to_string(), ours, theirs, theirs_wire, theirs_shown, ours_shown, same }
    }

    /// One output of `diff`: the new version's answer, and the old version's in its own
    /// version's terms (§15.199).
    pub fn between(name: &str, ours: Option<Val>, new: &Checked, was: Option<Val>, old: &Checked) -> Out {
        let same = same_answer(name, was.as_ref(), old, ours.as_ref(), new);
        let theirs = was.as_ref().and_then(|v| bring(name, v, old, new));
        let theirs_wire = wire_exact(old, name, was.as_ref());
        let (theirs_shown, ours_shown) = shown_pair(name, was.as_ref(), old, ours.as_ref(), new);
        Out { name: name.to_string(), ours, theirs, theirs_wire, theirs_shown, ours_shown, same }
    }

    /// `ours - theirs` in the rule's wire unit, exactly: a whole number of its steps when the
    /// two sit on the rule's step, a decimal where the counterpart's answer is finer (§15.199).
    fn delta(&self, c: &Checked) -> Option<Rat> {
        match (&self.ours, &self.theirs) {
            (Some(Val::Num(a)), Some(Val::Num(b))) => Some(a.sub(*b).mul(Rat::int(c.wire_scale(&self.name)))),
            _ => None,
        }
    }
}

/// Whether two answers to one output are the same value (§15.199). Numbers are compared
/// exactly, never at either side's step: money and quantities as amounts of their dimension
/// (yen and sen, kilograms and grams), rates and numbers as they are. An answer and no answer
/// differ; two absent answers are the same.
pub fn same_answer(name: &str, a: Option<&Val>, ac: &Checked, b: Option<&Val>, bc: &Checked) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(Val::Num(x)), Some(Val::Num(y))) => match (ac.ty_of(name), bc.ty_of(name)) {
            (Some(ta), Some(tb)) => crate::fixtures::convert(*x, &ta, &tb) == Some(*y),
            _ => x == y,
        },
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

/// A value of `from`'s output, in `to`'s terms: an amount brought to `to`'s unit; anything
/// else as it is. None for an amount no conversion carries.
fn bring(name: &str, v: &Val, from: &Checked, to: &Checked) -> Option<Val> {
    match (v, from.ty_of(name), to.ty_of(name)) {
        (Val::Num(x), Some(a), Some(b)) => crate::fixtures::convert(*x, &a, &b).map(Val::Num),
        (Val::Num(_), _, _) => None,
        (other, _, _) => Some(other.clone()),
    }
}

impl Mismatch {
    /// Only the outputs that differ. When the counterpart gave no answer, all of them count
    /// as differing.
    pub fn differing(&self) -> Vec<&Out> {
        self.outs.iter().filter(|o| !o.same).collect()
    }

    /// The inputs as (name, as a person reads it, as the wire writes it): the ones made when
    /// the record was read, or `input` in `c`'s terms.
    fn inputs(&self, c: &Checked) -> Vec<(String, String, String)> {
        if !self.shown.is_empty() {
            return self.shown.clone();
        }
        self.input
            .iter()
            .map(|(n, v)| (n.clone(), vectors::show_named(c, n, v), wire(c, n, Some(v)).unwrap_or_default()))
            .collect()
    }
}

/// A value in its wire representation. JSON numbers are integers in the canonical unit, and
/// a rate travels as the number of steps (§10.2), so the conversion needs the name.
pub fn wire(c: &Checked, name: &str, v: Option<&Val>) -> Option<String> {
    v.map(|o| match o {
        Val::Num(r) => format!("{}", crate::types::wire_int(*r, c.wire_scale(name))),
        Val::Bool(b) => format!("{b}"),
        other => vectors::show(other),
    })
}

/// [`wire`], without cutting a value that is not a whole number of the step: `123.4` where
/// an implementation answered 12.34% at a step of 0.1% (§15.199). A value on the step is the
/// integer `wire` gives.
pub fn wire_exact(c: &Checked, name: &str, v: Option<&Val>) -> Option<String> {
    v.map(|o| match o {
        Val::Num(r) => exact_text(r.mul(Rat::int(c.wire_scale(name)))),
        Val::Bool(b) => format!("{b}"),
        other => vectors::show(other),
    })
}

/// An answer as a person reads it, exactly and in the rule's own terms: what
/// [`vectors::show_named`] writes — a rate with its `%` — with the sign kept on a value between
/// -1 and 0, which `Rat`'s own `Display` drops (§15.199). A decimal an implementation answered,
/// `-0.5`, shows as it was.
pub fn shown(c: &Checked, name: &str, v: &Val) -> String {
    match (v, c.ty_of(name)) {
        (Val::Num(r), Some(crate::types::Ty::Rate)) => format!("{}%", exact_text(r.mul(Rat::int(100)))),
        (Val::Num(r), _) => exact_text(*r),
        _ => vectors::show_named(c, name, v),
    }
}

/// The unit an amount of `name` is counted in, as the rule writes it after a number: `円`,
/// `銭`, `kg`. None for anything but money and quantities.
fn unit_of(c: &Checked, name: &str) -> Option<String> {
    let ty = match c.ty_of(name)? {
        crate::types::Ty::Opt(t) => *t,
        t => t,
    };
    match ty {
        crate::types::Ty::Money { cur, .. } => Some(cur),
        crate::types::Ty::Qty { unit, .. } => Some(unit),
        _ => None,
    }
}

/// Two versions' answers to one output as a person reads them, each in its own version's terms
/// (§15.199). Where the versions count the output in different units, each number carries its
/// unit — `2000円` beside `200050銭` — because the two numbers alone would read as one unit.
pub fn shown_pair(name: &str, a: Option<&Val>, ac: &Checked, b: Option<&Val>, bc: &Checked) -> (Option<String>, Option<String>) {
    let (ua, ub) = (unit_of(ac, name), unit_of(bc, name));
    let label = |v: &Val, c: &Checked, u: &Option<String>| {
        let t = shown(c, name, v);
        match (v, u) {
            (Val::Num(_), Some(u)) if ua != ub => format!("{t}{u}"),
            _ => t,
        }
    };
    (a.map(|v| label(v, ac, &ua)), b.map(|v| label(v, bc, &ub)))
}

/// `r` written exactly: an integer, a decimal where the decimals end, else a fraction (a third
/// of a yen has no decimal). The sign is kept for a value between -1 and 0.
fn exact_text(r: Rat) -> String {
    if r.is_int() {
        return r.num.to_string();
    }
    let sign = if r.num < 0 { "-" } else { "" };
    let a = Rat::new(r.num.abs(), r.den);
    match decimals(a) {
        Some((int, frac)) => format!("{sign}{int}.{frac}"),
        None => format!("{sign}{}/{}", a.num, a.den),
    }
}

/// A value that is not negative as its whole part and its decimals, when the decimals end.
fn decimals(r: Rat) -> Option<(i128, String)> {
    let (mut d, mut twos, mut fives) = (r.den, 0u32, 0u32);
    while d % 2 == 0 {
        d /= 2;
        twos += 1;
    }
    while d % 5 == 0 {
        d /= 5;
        fives += 1;
    }
    if d != 1 {
        return None;
    }
    let places = twos.max(fives);
    let scale = 10i128.checked_pow(places)?;
    let v = r.num.checked_mul(scale)? / r.den;
    Some((v / scale, format!("{:0width$}", v % scale, width = places as usize)))
}

/// A number for `--format json`, exactly: an integer or a decimal as a JSON number, and the
/// rare value no decimal writes (a fraction) as the string of the fraction.
fn num_json(r: Rat) -> String {
    let t = exact_text(r);
    if t.contains('/') { crate::json::quote(&t) } else { t }
}

/// Whether a wire value goes into the JSON bare: a number (whole or decimal) or a boolean.
fn json_scalar(v: &str) -> bool {
    let digits = v.strip_prefix('-').unwrap_or(v);
    let number = match digits.split_once('.') {
        Some((i, f)) => !i.is_empty() && !f.is_empty() && i.bytes().all(|b| b.is_ascii_digit()) && f.bytes().all(|b| b.is_ascii_digit()),
        None => !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()),
    };
    number || v == "true" || v == "false"
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
    /// The word that names the counterpart in a witness: verify uses "現行" (the implementation
    /// running today, whoever wrote it), replay
    /// "観測" (observed), diff "旧版" (old version).
    pub theirs: String,
    /// Filled records: the count per field, and the default values used (§10.3).
    pub filled: BTreeMap<String, usize>,
    pub fills_used: BTreeMap<String, String>,
    pub filled_total: usize,
    pub filled_agreed: usize,
    /// Records excluded before the comparison: a stable kind (`missing_field`, `bad_format`),
    /// the reason as **prose**, and how many. The kind is what `--format json` reports, so a
    /// caller never has to match on the sentence.
    pub excluded: Vec<(&'static str, String, usize)>,
    /// Records whose values agreed but whose recorded rows differ from the rule's (§15.35).
    /// Only a record that carries a `trace` can land here. They count as matched — the amount
    /// is right — and are reported apart, clustered by the move.
    pub moved: Vec<Mismatch>,
    /// `diff` only: records whose input the old version takes and the new one does not
    /// (§15.197). They are compared — counted in `total`, never in `agreed` — because a version
    /// that turns an input away answers it differently; the reason is in each one's `refusal`.
    pub refused: Vec<Mismatch>,
    /// `replay` and `verify`: per output, how many records the counterpart answered with a
    /// value outside what the rule can produce (§15.199). They are compared and counted as
    /// mismatches — the counterpart and the rule disagree there — and the count says so.
    pub out_of_reach: BTreeMap<String, usize>,
    /// How many records hold at least one such value.
    pub beyond: usize,
    /// Whether the counterpart is a file of records (`replay`, `diff`) rather than a running
    /// implementation (`verify`): only records can have been written at another step.
    pub records: bool,
    /// A machine's records read as cases (§15.148). `None` for a rule that is not a machine,
    /// or records that carry no tag.
    pub cases: Option<Cases>,
}

/// A machine's records read as cases (§15.148): the records that share a `tag`, in the order
/// they came in, are one case's calls. Each case is played again from its first record, the
/// version carrying its own answer from one call to the next.
#[derive(Default, Debug, Clone)]
pub struct Cases {
    pub total: usize,
    /// Played through with every call answered as it was before (the record's answer for
    /// `replay`, the old version's for `diff`).
    pub followed: usize,
    /// (tag, line): the first call where the answer parts from before.
    pub diverged: Vec<(String, usize)>,
    /// (tag, line): a call the version refuses — a state it no longer has, a value it does not
    /// take.
    pub refused: Vec<(String, usize)>,
    /// (tag, state): the state a case is left in, from which the version reaches no final state.
    pub stranded: Vec<(String, String)>,
    /// How many cases end in a final state.
    pub ended: usize,
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
            moved: Vec::new(),
            refused: Vec::new(),
            out_of_reach: BTreeMap::new(),
            beyond: 0,
            records: true,
            cases: None,
        }
    }

    /// Count a record whose answers include values outside what the rule can produce, by
    /// output (§15.199).
    pub fn count_beyond<'a>(&mut self, outputs: impl IntoIterator<Item = &'a str>) {
        let mut any = false;
        for n in outputs {
            *self.out_of_reach.entry(n.to_string()).or_insert(0) += 1;
            any = true;
        }
        if any {
            self.beyond += 1;
        }
    }

    /// Whether anything differs: a mismatch, or a record the new version does not take. The
    /// exit code of `verify`, `replay` and `diff` is this, or not one record compared.
    pub fn differs(&self) -> bool {
        !self.mismatches.is_empty() || !self.refused.is_empty()
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

    /// Whether not one record was compared: every one excluded, left unanswered, or none
    /// there. "No mismatches" over nothing used to end the run with exit 0, so a CI job fed
    /// records the rule could no longer read stayed green (§15.144).
    pub fn compared_nothing(&self) -> bool {
        self.total <= self.errored && self.filled_total == 0
    }

    /// What to say when records were thrown out for their format and nothing was left, or
    /// when what they say came out is something the rule cannot produce: the likeliest cause
    /// of either is a step or a unit that changed since they were written (§15.145, §15.199).
    fn read_as_hint(&self) -> Option<String> {
        let thrown = self.compared_nothing() && self.excluded.iter().any(|(k, ..)| *k == "bad_format");
        (thrown || (self.records && self.beyond > 0)).then(|| {
            tr!(
                "刻みや単位を変える前に書いた記録なら、`--read-as <規則>@<版>` でその版の読み方で読めます。",
                "If the records were written before a step or a unit changed, `--read-as <rule>@<rev>` reads them the way that version wrote them."
            )
        })
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

/// An exact difference, the way `group` writes a whole one: the sign always and thousands
/// separators, with the decimals it has where the counterpart answered finer than the step,
/// and as a fraction where no decimal writes it (§15.199). Never rounded.
fn group_rat(r: Rat) -> String {
    if r.is_int() {
        return group(r.num);
    }
    let sign = if r.num < 0 { "-" } else { "+" };
    let a = Rat::new(r.num.abs(), r.den);
    match decimals(a) {
        Some((int, frac)) => format!("{sign}{}.{frac}", &group(int)[1..]),
        None => format!("{sign}{}/{}", a.num, a.den),
    }
}

/// The word after a record count: `件` in Japanese, `record`/`records` in English.
fn records(n: usize) -> &'static str {
    if crate::i18n::ja() { "件" } else if n == 1 { "record" } else { "records" }
}

/// Statistics of Δ within a cluster (§10.4). Always carries the count, the total and the
/// min/max. Δ is counted in the rule's wire unit — steps of its rate, units of its money — and
/// exactly: a whole number when both answers sit on the rule's step, a decimal where the
/// counterpart's answer is finer (§15.199).
pub struct Delta {
    pub n: usize,
    pub sum: Rat,
    pub lo: Rat,
    pub hi: Rat,
}

impl Delta {
    fn push(&mut self, d: Rat) {
        if self.n == 0 {
            self.lo = d;
            self.hi = d;
        } else {
            if d.cmp_to(self.lo) == std::cmp::Ordering::Less {
                self.lo = d;
            }
            if d.cmp_to(self.hi) == std::cmp::Ordering::Greater {
                self.hi = d;
            }
        }
        self.n += 1;
        self.sum = self.sum.add(d);
    }
    pub fn uniform(&self) -> bool {
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
                group_rat(self.lo),
                group_rat(self.sum)
            )
        } else {
            tr!(
                "合計 {}  Δ {}..{}",
                "total {}  Δ {}..{}",
                group_rat(self.sum),
                group_rat(self.lo),
                group_rat(self.hi)
            )
        }
    }
}

/// Per cluster: output name → Δ statistics.
fn deltas(ms: &[&Mismatch], c: &Checked) -> BTreeMap<String, Delta> {
    let mut out: BTreeMap<String, Delta> = BTreeMap::new();
    for m in ms {
        for o in m.differing() {
            // Both answers are values; the difference is counted in the rule's wire unit, so a
            // rate's Δ is in its steps.
            let Some(d) = o.delta(c) else { continue };
            out.entry(o.name.clone()).or_insert(Delta { n: 0, sum: Rat::zero(), lo: Rat::zero(), hi: Rat::zero() }).push(d);
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

fn witness(ex: &Mismatch, theirs: &str, c: &Checked) -> String {
    let inp: Vec<String> = ex.inputs(c).iter().map(|(n, shown, _)| format!("{n}={shown}")).collect();
    if ex.refusal.is_some() {
        // The new version gave no answer: what is shown is the input and the old version's.
        let old: Vec<String> = ex.outs.iter().filter_map(|o| Some(format!("{theirs} {}={}", o.name, o.theirs_shown.as_ref()?))).collect();
        return if old.is_empty() { inp.join(", ") } else { format!("{} → {}", inp.join(", "), old.join(", ")) };
    }
    // Each answer as its own side writes it, unrounded (§15.199): 12% beside 12.3%.
    let diff: Vec<String> = ex
        .differing()
        .iter()
        .filter_map(|o| {
            let (a, b, n) = (o.ours_shown.as_ref()?, o.theirs_shown.as_ref()?, &o.name);
            Some(tr!("規則 {n}={a} / {theirs} {n}={b}", "rule {n}={a} / {theirs} {n}={b}"))
        })
        .collect();
    if diff.is_empty() {
        inp.join(", ")
    } else {
        format!("{} → {}", inp.join(", "), diff.join(", "))
    }
}

fn cluster(ms: &[Mismatch]) -> BTreeMap<String, Vec<&Mismatch>> {
    let mut out: BTreeMap<String, Vec<&Mismatch>> = BTreeMap::new();
    for m in ms {
        // §10.4: the key is the set of fired rows only; the legacy output is not part of it.
        // For table-lookup rows the set of rows already fixes the set of amounts, so nothing
        // would be gained, and for computed outputs the clusters would split once per distinct
        // value and the summary would die. What the split would show is recovered by the
        // min/max of Δ.
        out.entry(key_of(m)).or_default().push(m);
    }
    out
}

fn key_of(m: &Mismatch) -> String {
    match &m.err {
        Some(e) => tr!("答えられない: {e}", "could not answer: {e}"),
        None => m.fired.iter().map(|x| x.label()).collect::<Vec<_>>().join(" / "),
    }
}

/// One cluster of mismatches, as data. The three renderings (text, markdown, JSON) are all
/// built from this, so they cannot drift apart.
pub struct Cluster<'a> {
    /// The fired rows joined, as displayed. **Prose.**
    pub label: String,
    pub fired: Vec<Fired>,
    pub count: usize,
    /// Output name → how far it moved across the cluster.
    pub deltas: BTreeMap<String, Delta>,
    /// The record shown as the example.
    pub example: &'a Mismatch,
    /// Every record in the cluster, in the order they came in. The example is the first of
    /// them; a gate that has to act on the records themselves needs all of them.
    pub members: Vec<&'a Mismatch>,
    /// The output grid, when every difference in the cluster is below it (§10.4).
    pub suspect_grid: Option<String>,
    /// Set when the counterpart declared it could not answer. **Prose.**
    pub error: Option<String>,
}

/// Every cluster of mismatches, in the order the renderings show them.
pub fn clusters<'a>(rep: &'a Report, f: &RuleFile, c: &Checked) -> Vec<Cluster<'a>> {
    build(&rep.mismatches, f, c)
}

/// The clusters of moved rows (§15.35): the same shape, with nothing in `deltas`.
pub fn moved_clusters<'a>(rep: &'a Report, f: &RuleFile, c: &Checked) -> Vec<Cluster<'a>> {
    build(&rep.moved, f, c)
}

/// The records the new version does not take, under one reason (§15.197). The three
/// renderings are built from these, as they are from [`Cluster`].
pub struct RefusedCluster<'a> {
    pub kind: &'static str,
    /// The input at fault; empty for a constraint.
    pub field: String,
    /// The reason. **Prose.**
    pub what: String,
    pub count: usize,
    /// The record shown as the example: the first of `members`.
    pub example: &'a Mismatch,
    /// Every record refused for this reason, in the order they came in.
    pub members: Vec<&'a Mismatch>,
}

/// The records the new version does not take, grouped by the reason, in the order the
/// renderings show them. The reason leaves out a value that varies from record to record, so
/// a narrowed range is one line however many amounts fell outside it.
pub fn refused_clusters(rep: &Report) -> Vec<RefusedCluster<'_>> {
    let mut by: BTreeMap<&str, Vec<&Mismatch>> = BTreeMap::new();
    for m in &rep.refused {
        if let Some(r) = &m.refusal {
            by.entry(r.what.as_str()).or_default().push(m);
        }
    }
    by.into_iter()
        .filter_map(|(what, ms)| {
            let r = ms[0].refusal.as_ref()?;
            Some(RefusedCluster { kind: r.kind, field: r.field.clone(), what: what.to_string(), count: ms.len(), example: ms[0], members: ms })
        })
        .collect()
}

/// The headline of the records the new version does not take, beside §10.4's for the answers
/// that moved, over the same denominator.
fn refused_head(rep: &Report) -> String {
    let n = rep.total - rep.errored;
    let k = rep.refused.len();
    let pct = if n == 0 { 0.0 } else { k as f64 * 100.0 / n as f64 };
    tr!("新しい版が受け付けない入力 {k} 件 ({pct:.3}%)", "Not accepted by the new version {k} ({pct:.3}%)")
}

fn build<'a>(ms: &'a [Mismatch], f: &RuleFile, c: &Checked) -> Vec<Cluster<'a>> {
    cluster(ms)
        .into_iter()
        .map(|(label, ms)| Cluster {
            label,
            fired: ms[0].fired.clone(),
            count: ms.len(),
            deltas: deltas(&ms, c),
            suspect_grid: sub_grid(&ms, f, c),
            error: ms[0].err.clone(),
            example: ms[0],
            members: ms,
        })
        .collect()
}

/// The record of fills and exclusions (§10.3). Wherever a number appears, its basis is
/// shown next to it.
fn provenance(rep: &Report) -> Vec<String> {
    let mut o = Vec::new();
    if rep.errored > 0 {
        o.push(tr!(
            "相手が答えられなかった {} 件は、一致率の分母から外しています",
            "The counterpart could not answer {} of the records; those are excluded from the match-rate denominator",
            rep.errored
        ));
    }
    for (_, why, n) in &rep.excluded {
        let unit = records(*n);
        o.push(tr!("{why}記録を {n} {unit}外しました", "Excluded {n} {unit} ({why})"));
    }
    // §15.199: what the counterpart answered that the rule cannot produce is compared, not
    // left out, and the count stands beside the headline.
    if rep.beyond > 0 {
        let by: Vec<String> = rep.out_of_reach.iter().map(|(n, k)| tr!("{n} {k} 件", "{n}: {k}")).collect();
        let by = by.join(if crate::i18n::ja() { "、" } else { ", " });
        let (theirs, n) = (&rep.theirs, rep.beyond);
        o.push(if rep.records {
            tr!(
                "{theirs}の値が規則の取りうる範囲の外にある記録 {n} 件（{by}）。不一致に数えています",
                "Records whose {theirs} value is outside what the rule can produce: {n} ({by}); counted as mismatches"
            )
        } else {
            tr!(
                "{theirs}の答えが規則の取りうる範囲の外だったもの {n} 件（{by}）。不一致に数えています",
                "Answers of the {theirs} implementation outside what the rule can produce: {n} ({by}); counted as mismatches"
            )
        });
    }
    if rep.filled_total > 0 {
        let by: Vec<String> =
            rep.filled.iter().map(|(n, k)| tr!("{n} {k} 件", "{n}: {k}")).collect();
        o.push(tr!(
            "補った記録 {} 件（{}）。一致 {} 件。見出しの一致率には入れていません",
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
fn impact(rep: &Report, c: &Checked) -> String {
    // A record the counterpart could not answer is listed apart and is out of the
    // denominator, so it is out of the numerator too: otherwise a counterpart that answers
    // eighteen records and declines the rest is "affected" a thousand percent.
    let n = rep.total - rep.errored;
    let all: Vec<&Mismatch> = rep.mismatches.iter().filter(|m| m.err.is_none()).collect();
    let pct = if n == 0 { 0.0 } else { all.len() as f64 * 100.0 / n as f64 };
    let money: Vec<String> = deltas(&all, c)
        .iter()
        .map(|(name, d)| {
            let label = if rep.multi { format!("{name} ") } else { String::new() };
            tr!("  差の合計 {label}{}", "  amount {label}{}", group_rat(d.sum))
        })
        .collect();
    tr!(
        "影響 {} 件 ({pct:.3}%){}",
        "Affected {} ({pct:.3}%){}",
        all.len(),
        money.join("")
    )
}

/// `terse` leaves the witnesses out (§15.42): the counts and the amounts are the finding, and
/// the values of a production record are not for a pull request everyone can read.
pub fn render(rep: &Report, f: &RuleFile, c: &Checked, terse: bool) -> String {
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
    let hint = rep.read_as_hint();
    if rep.compared_nothing() {
        o.push_str(&tr!("照合できた記録はありません。\n", "Not one record was compared.\n"));
        if let Some(h) = &hint {
            o.push_str(h);
            o.push('\n');
        }
    } else {
        // Records whose answers the rule cannot produce were compared: the reading that
        // explains them most often is said right under the count.
        if let Some(h) = &hint {
            o.push_str(h);
            o.push('\n');
        }
        if !rep.differs() {
            o.push_str(&tr!("不一致はありません。\n", "No mismatches.\n"));
        }
    }
    if !rep.compared_nothing() && !rep.mismatches.is_empty() {
        o.push_str(&format!("\n{}\n", impact(rep, c)));
        for cl in clusters(rep, f, c) {
            let money = money_text(&cl.deltas, rep.multi);
            o.push_str(&format!("  {:<48} {:>5} {}{money}\n", cl.label, cl.count, records(cl.count)));
            if let Some(q) = &cl.suspect_grid {
                // §10.4: a cluster made up solely of differences below the output grid is most
                // likely a difference in rounding convention, not in the values themselves. Flag
                // it automatically.
                o.push_str(&tr!(
                    "    丸め方の違いの疑い（出力の刻み {q} 未満の端数だけ）\n",
                    "    Suspected rounding difference (only fractions below the output grid {q})\n"
                ));
            }
            if !terse {
                o.push_str(&tr!("    例: {}\n", "    Example: {}\n", witness(cl.example, &rep.theirs, c)));
            }
        }
    }
    // §15.197: the inputs the new version turns away, apart from the answers that moved. They
    // have no amount, so they are kept out of the total above.
    if !rep.refused.is_empty() {
        o.push_str(&format!("\n{}\n", refused_head(rep)));
        for cl in refused_clusters(rep) {
            o.push_str(&format!("  {:<48} {:>5} {}\n", cl.what, cl.count, records(cl.count)));
            if !terse {
                o.push_str(&tr!("    例: {}\n", "    Example: {}\n", witness(cl.example, &rep.theirs, c)));
            }
        }
    }
    if let Some(cs) = &rep.cases {
        o.push('\n');
        o.push_str(&cases_text(cs));
    }
    // §15.35: the amount agreed, the row did not. Reported apart from the mismatches, so the
    // headline stays about values, and clustered by the move so a renumbering reads as one line.
    if !rep.moved.is_empty() {
        o.push_str(&tr!(
            "\n値は同じで、当てはまった行が記録と違う記録 {} 件\n",
            "\nRecords whose values match but whose rows differ from the record: {}\n",
            rep.moved.len()
        ));
        for cl in moved_clusters(rep, f, c) {
            o.push_str(&format!("  {:<48} {:>5} {}\n", cl.label, cl.count, records(cl.count)));
            if !terse {
                o.push_str(&tr!("    例: {}\n", "    Example: {}\n", witness(cl.example, &rep.theirs, c)));
            }
        }
    }
    o
}

/// The cases, as lines (§15.148).
pub fn cases_text(cs: &Cases) -> String {
    let first = |v: &Vec<(String, usize)>| -> String {
        v.first().map(|(t, l)| tr!("（例: {t} の {l} 行目）", " (e.g. {t}, line {l})")).unwrap_or_default()
    };
    let mut o = tr!(
        "案件（同じ tag の記録を、一つの案件の呼び出しの並びとして）: {} 件\n",
        "Cases (the records that share a tag, as one case's calls): {}\n",
        cs.total
    );
    o.push_str(&tr!("  前と同じに進んだ: {} 件\n", "  played as before: {}\n", cs.followed));
    if !cs.diverged.is_empty() {
        o.push_str(&tr!("  途中で答えが変わる: {} 件{}\n", "  answered differently partway: {}{}\n", cs.diverged.len(), first(&cs.diverged)));
    }
    if !cs.refused.is_empty() {
        o.push_str(&tr!("  途中でエラーになる: {} 件{}\n", "  refused partway: {}{}\n", cs.refused.len(), first(&cs.refused)));
    }
    if !cs.stranded.is_empty() {
        let (t, st) = &cs.stranded[0];
        o.push_str(&tr!(
            "  終わりの状態に着けなくなる: {} 件（例: {t} は {st} で止まる）\n",
            "  left where no final state can be reached: {} (e.g. {t} stops at {st})\n",
            cs.stranded.len()
        ));
    }
    o.push_str(&tr!("  終わりの状態で終わる: {} 件\n", "  ending in a final state: {}\n", cs.ended));
    o
}

/// Markdown to paste into a PR (§12). Posting is left to one line of CI; the tool owns only
/// the formatting.
pub fn markdown(rep: &Report, f: &RuleFile, c: &Checked, title: &str, terse: bool) -> String {
    let esc = |s: &str| s.replace('|', "\\|");
    // With `terse` the witness column is not blanked but absent, so the table stays a table.
    let ex = |m: &Mismatch| if terse { String::new() } else { format!(" {} |", esc(&witness(m, &rep.theirs, c))) };
    let mut o = tr!(
        "### 規則 {} v{} — {title}\n\n",
        "### Rule {} v{} — {title}\n\n",
        f.name.text,
        f.version
    );
    o.push_str("| | |\n|---|---:|\n");
    o.push_str(&tr!(
        "| 照合（そのままの記録） | {} 件 |\n",
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
    if !rep.refused.is_empty() {
        o.push_str(&tr!("| 新しい版が受け付けない | {} 件 |\n", "| Not accepted by the new version | {} |\n", rep.refused.len()));
    }
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
    let hint = rep.read_as_hint();
    if rep.compared_nothing() {
        o.push_str(&tr!("\n照合できた記録はありません。\n", "\nNot one record was compared.\n"));
        if let Some(h) = &hint {
            o.push('\n');
            o.push_str(h);
            o.push('\n');
        }
    } else {
        if let Some(h) = &hint {
            o.push('\n');
            o.push_str(h);
            o.push('\n');
        }
        if !rep.differs() {
            o.push_str(&tr!("\n不一致はありません。\n", "\nNo mismatches.\n"));
        }
    }
    if !rep.compared_nothing() && !rep.mismatches.is_empty() {
        o.push_str(&format!("\n**{}**\n", impact(rep, c)));
        o.push_str(&tr!("\n#### 不一致の内訳\n\n", "\n#### Mismatch breakdown\n\n"));
        o.push_str(if terse {
            tr!("| 当てはまった行 | 件数 | 差 |\n|---|---:|---|\n", "| Rows that matched | Count | Difference |\n|---|---:|---|\n")
        } else {
            tr!(
                "| 当てはまった行 | 件数 | 差 | 入力例 |\n|---|---:|---|---|\n",
                "| Rows that matched | Count | Difference | Witness |\n|---|---:|---|---|\n"
            )
        }
        .as_str());
        for cl in clusters(rep, f, c) {
            let mut money = money_text(&cl.deltas, rep.multi).trim().to_string();
            if let Some(q) = &cl.suspect_grid {
                money.push_str(&tr!(
                    "<br>丸め方の違いの疑い（刻み {q} 未満）",
                    "<br>suspected rounding difference (below grid {q})"
                ));
            }
            o.push_str(&format!("| {} | {} | {} |{}\n", esc(&cl.label), cl.count, esc(&money), ex(cl.example)));
        }
    }
    if !rep.refused.is_empty() {
        o.push_str(&format!("\n**{}**\n", refused_head(rep)));
        o.push_str(&tr!("\n#### 新しい版が受け付けない入力の内訳\n\n", "\n#### Not accepted by the new version, by reason\n\n"));
        o.push_str(if terse {
            tr!("| 理由 | 件数 |\n|---|---:|\n", "| Reason | Count |\n|---|---:|\n")
        } else {
            tr!("| 理由 | 件数 | 入力例 |\n|---|---:|---|\n", "| Reason | Count | Witness |\n|---|---:|---|\n")
        }
        .as_str());
        for cl in refused_clusters(rep) {
            o.push_str(&format!("| {} | {} |{}\n", esc(&cl.what), cl.count, ex(cl.example)));
        }
    }
    if !rep.moved.is_empty() {
        o.push_str(&tr!(
            "\n#### 行の移動（値は同じ、{} 件）\n\n",
            "\n#### Moved rows (values match, {})\n\n",
            rep.moved.len()
        ));
        o.push_str(if terse {
            tr!("| 記録の行 → 規則の行 | 件数 |\n|---|---:|\n", "| Recorded row → rule's row | Count |\n|---|---:|\n")
        } else {
            tr!(
                "| 記録の行 → 規則の行 | 件数 | 入力例 |\n|---|---:|---|\n",
                "| Recorded row → rule's row | Count | Witness |\n|---|---:|---|\n"
            )
        }
        .as_str());
        for cl in moved_clusters(rep, f, c) {
            o.push_str(&format!("| {} | {} |{}\n", esc(&cl.label), cl.count, ex(cl.example)));
        }
    }
    if let Some(cs) = &rep.cases {
        o.push('\n');
        for (k, line) in cases_text(cs).lines().enumerate() {
            o.push_str(&if k == 0 { format!("**{}**\n\n", line.trim()) } else { format!("- {}\n", line.trim()) });
        }
    }
    o
}

/// `--format json` for `verify`, `replay` and `diff` (docs/formats.md). All three share one
/// shape, because all three are "the rule against a counterpart".
pub fn render_json(rep: &Report, f: &RuleFile, c: &Checked) -> String {
    let pairs = |ps: &[(String, String)]| {
        let mut o = crate::json::Obj::new();
        for (n, v) in ps {
            // A wire value is already the canonical integer — or, for a counterpart that
            // answered finer than the step, a decimal — a boolean, or a name. Numbers and
            // booleans go in bare; anything else is a string.
            o = if json_scalar(v) { o.raw(n, v) } else { o.str(n, v) };
        }
        o.finish()
    };
    let one = |cl: &Cluster| {
            let rows: Vec<String> = cl.fired.iter().map(|x| x.json()).collect();
            // Every record of the cluster by name, not only the example. A gate that has to
            // do something with the records — hold a version back, re-quote an order — needs
            // to name them; counting them is not enough.
            let members: Vec<String> = cl
                .members
                .iter()
                .map(|m| crate::json::Obj::new().int("line", m.line as i128).str("tag", &m.tag).finish())
                .collect();
            let mut delta = crate::json::Obj::new();
            for (n, d) in &cl.deltas {
                delta = delta.raw(
                    n,
                    crate::json::Obj::new()
                        .raw("min", num_json(d.lo))
                        .raw("max", num_json(d.hi))
                        .bool("uniform", d.uniform())
                        .raw("total", num_json(d.sum))
                        .finish(),
                );
            }
            let ex = cl.example;
            let ins: Vec<(String, String)> = ex.inputs(c).into_iter().map(|(n, _, w)| (n, w)).collect();
            // Each answer in its own side's step and unit (§15.199): the rule's wire, and the
            // counterpart's as it writes it — `diff`'s `old` and `new` are written the same way.
            let ours: Vec<(String, String)> =
                ex.differing().iter().filter_map(|o| Some((o.name.clone(), wire(c, &o.name, o.ours.as_ref())?))).collect();
            let theirs: Vec<(String, String)> =
                ex.differing().iter().filter_map(|o| Some((o.name.clone(), o.theirs_wire.clone()?))).collect();
            let wit = crate::json::Obj::new()
                .raw("in", pairs(&ins))
                .raw("ours", pairs(&ours))
                .raw("theirs", pairs(&theirs))
                .finish();
            crate::json::Obj::new()
                .raw("rows", crate::json::arr(&rows))
                .int("count", cl.count as i128)
                .raw("delta", delta.finish())
                .raw("witness", wit)
                .raw("records", format!("[{}]", members.join(",")))
                .bool("suspect_rounding", cl.suspect_grid.is_some())
                .opt_str("error", cl.error.as_deref())
                .finish()
    };
    let cls: Vec<String> = clusters(rep, f, c).iter().map(one).collect();
    let moved: Vec<String> = moved_clusters(rep, f, c).iter().map(one).collect();
    // §15.197: the records the new version does not take, by reason. Every one by name, as in
    // `clusters`; the witness is the input and the old version's answer, the new version
    // having given none.
    let refused: Vec<String> = refused_clusters(rep)
        .iter()
        .map(|cl| {
            let members: Vec<String> =
                cl.members.iter().map(|m| crate::json::Obj::new().int("line", m.line as i128).str("tag", &m.tag).finish()).collect();
            let ex = cl.example;
            let ins: Vec<(String, String)> = ex.inputs(c).into_iter().map(|(n, _, w)| (n, w)).collect();
            let theirs: Vec<(String, String)> = ex.outs.iter().filter_map(|o| Some((o.name.clone(), o.theirs_wire.clone()?))).collect();
            let mut o = crate::json::Obj::new().str("kind", cl.kind);
            if !cl.field.is_empty() {
                o = o.str("field", &cl.field);
            }
            o.int("count", cl.count as i128)
                .raw("witness", crate::json::Obj::new().raw("in", pairs(&ins)).raw("theirs", pairs(&theirs)).finish())
                .raw("records", format!("[{}]", members.join(",")))
                .str("what", &cl.what)
                .finish()
        })
        .collect();

    let mut excluded = crate::json::Obj::new();
    for (kind, _, n) in &rep.excluded {
        excluded = excluded.int(kind, *n as i128);
    }
    let mut by_field = crate::json::Obj::new();
    for (n, k) in &rep.filled {
        by_field = by_field.int(n, *k as i128);
    }
    let mut defaults = crate::json::Obj::new();
    for (n, v) in &rep.fills_used {
        defaults = defaults.str(n, v);
    }
    let filled = crate::json::Obj::new()
        .int("count", rep.filled_total as i128)
        .raw("by_field", by_field.finish())
        .raw("defaults", defaults.finish())
        .finish();

    crate::json::Obj::new()
        .int("compared", rep.total as i128)
        .int("matched", rep.agreed as i128)
        .raw("rate", format!("{:.5}", rep.rate()))
        .str("counterpart", &rep.impl_id)
        .int("unanswered", rep.errored as i128)
        .raw("clusters", crate::json::arr(&cls))
        .raw("moved", crate::json::arr(&moved))
        .raw("refused", crate::json::arr(&refused))
        .raw("excluded", excluded.finish())
        .raw("out_of_reach", {
            let mut o = crate::json::Obj::new();
            for (n, k) in &rep.out_of_reach {
                o = o.int(n, *k as i128);
            }
            o.finish()
        })
        .raw("filled", filled)
        .raw("cases", match &rep.cases {
            None => "null".into(),
            Some(cs) => {
                let at = |v: &Vec<(String, usize)>| -> String {
                    crate::json::arr(&v.iter().map(|(t, l)| crate::json::Obj::new().str("tag", t).int("line", *l as i128).finish()).collect::<Vec<_>>())
                };
                crate::json::Obj::new()
                    .int("total", cs.total as i128)
                    .int("followed", cs.followed as i128)
                    .raw("diverged", at(&cs.diverged))
                    .raw("refused", at(&cs.refused))
                    .raw(
                        "stranded",
                        crate::json::arr(&cs.stranded.iter().map(|(t, st)| crate::json::Obj::new().str("tag", t).str("state", st).finish()).collect::<Vec<_>>()),
                    )
                    .int("ended", cs.ended as i128)
                    .finish()
            }
        })
        .finish()
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
        for o in diff {
            let n = &o.name;
            let od = f.outputs.iter().find(|x| x.name.text == *n)?;
            let rd = od.rounding.as_ref()?;
            let ty = c.ty_of(n)?;
            let q = crate::types::lit_value_in_pub(&rd.grid, &ty)?;
            if q.num <= 0 {
                return None;
            }
            // The two answers as values, exactly (§15.199): a counterpart that answered finer
            // than the grid shows its fraction here instead of having it cut away first.
            let (Some(Val::Num(a)), Some(Val::Num(b))) = (&o.ours, &o.theirs) else { return None };
            let d = a.sub(*b);
            let d = if d.num < 0 { Rat::new(-d.num, d.den) } else { d };
            if d.num == 0 || d.cmp_to(q) != std::cmp::Ordering::Less {
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
