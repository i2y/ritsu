//! `koyomi diff` (ritsu's DESIGN 17.9.3): what changes between two versions of a calendar, or of a
//! dates file, over everything both versions answer.
//!
//! A calendar is compared on every day both versions know (the overlap of [`Calendar::data`]):
//! whether the day is open, and why it is closed. Days whose answer changes the same way, for the
//! same reasons, one after another, are one interval; a day closed in both whose reasons are named
//! otherwise is listed apart. The days known and the offset are compared too.
//!
//! A dates file is compared on every input both versions take: the inputs both have with one type
//! are the axes, each narrowed to the overlap of its two ranges, and every combination is computed
//! by both, as the check computes them (within `--budget`, 10⁸ by default; over it, nothing is
//! compared and the diff stops, exit 2). Each date both versions have is compared; the combinations
//! where it changes are put together along the date input, the other inputs held, while the shift
//! (how many days the new value is from the old) stays the same. An input only one version has
//! leaves no input both can be given: only the inputs are compared then. Claims and examples are not
//! compared: they check the answers, they are not answers.
//!
//! Each side is checked first, read through its own files ([`side`]: a git revision, a version of
//! a dependency, ritsu's DESIGN 17.9.1); one that does not pass check stops the diff (exit 2).

use crate::ast::Ty;
use crate::calendar::{Calendar, Loader, Reason};
use crate::check::{self, Checked, Options, Outcome};
use crate::date::Day;
use crate::resolve::Model;
use crate::vectors::{self, Expect};
use ritsu_base::text::{Lang, Text, count};
use serde_json::{Map, Value, json};
use std::rc::Rc;

/// How many intervals text and Markdown show for each date (for a calendar, in all); the JSON has
/// every one.
pub const SHOWN: usize = 200;

/// A side of a diff: how it is shown, and the check of its file.
pub struct Side {
    pub shown: String,
    pub outcome: Outcome,
}

/// A side read and checked: the file `path`, read through `files` (a git revision, a version of a
/// dependency; None for the files as they are), shown as `shown`. Err, with what to say (exit 2),
/// for a file that cannot be read, and for a tree of modules that stops (a dependency to fetch).
pub fn side(path: &str, shown: &str, files: Option<Rc<dyn ritsu_base::fs::Files>>, budget: u64) -> Result<Side, Text> {
    let run = || -> Result<Side, Text> {
        if let Some(t) = ritsu_base::modules::address::stopped(path) {
            return Err(t);
        }
        let mut loader = Loader::default();
        // the check walks what `koyomi check` walks, and more when the diff is given more
        let outcome = check::check_file(path, &Options { budget: budget.max(check::DEFAULT_BUDGET) }, &mut loader).map_err(|e| tr!("`{shown}` を読めません: {e}", "cannot read `{shown}`: {e}"))?;
        Ok(Side { shown: shown.to_string(), outcome })
    };
    match files {
        Some(f) => ritsu_base::fs::with(f, run),
        None => run(),
    }
}

/// A side as the command line writes it (ritsu's DESIGN 17.9.1): a path, `<path>@<git revision>`,
/// `@<name>/<path>` or `@<name>@<version>/<path>`.
pub fn spelled(s: &str, budget: u64) -> Result<Side, Text> {
    let a = ritsu_base::modules::address::parse(s)?;
    let o = ritsu_base::modules::address::open(&a)?;
    side(&o.path, s, o.files.clone(), budget)
}

/// The head of a side: its file as shown, the name and the version the file gives itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Head {
    pub file: String,
    pub name: String,
    pub version: String,
}

/// Days whose answer changed the same way, for the same reasons.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CalChange {
    pub from: Day,
    pub to: Day,
    /// Open in the old version, and in the new.
    pub old: bool,
    pub new: bool,
    pub old_reasons: Vec<Reason>,
    pub new_reasons: Vec<Reason>,
}

/// Days closed in both versions whose reasons changed the same way.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Renamed {
    pub from: Day,
    pub to: Day,
    pub old_reasons: Vec<Reason>,
    pub new_reasons: Vec<Reason>,
}

#[derive(Clone, Debug)]
pub struct CalDiff {
    pub old: Head,
    pub new: Head,
    /// The days both versions know; None when they know no day in common.
    pub compared: Option<(Day, Day)>,
    pub changes: Vec<CalChange>,
    pub renamed: Vec<Renamed>,
    pub known: ((Day, Day), (Day, Day)),
    /// Minutes east of UTC, in the old version and in the new.
    pub offset: (Option<i32>, Option<i32>),
}

/// An input both versions take with one type: its range in each, and the overlap the diff walks.
#[derive(Clone, Debug)]
pub struct Axis {
    pub name: String,
    pub ty: Ty,
    pub old: (i64, i64),
    pub new: (i64, i64),
    pub lo: i64,
    pub hi: i64,
}

impl Axis {
    fn show(&self, v: i64) -> String {
        match self.ty {
            Ty::Date => Day(v as i32).to_string(),
            Ty::Int => v.to_string(),
        }
    }

    fn size(&self) -> u128 {
        (self.hi - self.lo + 1).max(0) as u128
    }
}

/// What one version gives for a date on one input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Val {
    /// The day, and its time (RFC 3339 in UTC) when the date has `at`.
    Day(Day, Option<String>),
    /// The computation stops: the kind of error the generated code raises.
    Stop(&'static str),
}

/// The combinations, along the date input with the other inputs held, where a date changed by one
/// shift.
#[derive(Clone, Debug)]
pub struct DateChange {
    pub date: String,
    /// The date input's first and last value.
    pub first: (i64, i64),
    /// The other inputs' values, by the index of their axis.
    pub others: Vec<(usize, i64)>,
    /// The old values at the first and the last combination, and the new.
    pub old: (Val, Val),
    pub new: (Val, Val),
    /// How many days the new value is from the old; None when either stops.
    pub shift: Option<i64>,
    pub combinations: u64,
}

#[derive(Clone, Debug)]
pub struct DatesDiff {
    pub old: Head,
    pub new: Head,
    pub axes: Vec<Axis>,
    /// The index of the date input among the axes.
    pub date_axis: Option<usize>,
    /// The inputs only one version takes (or takes with another type), with their type.
    pub only_old: Vec<(String, Ty)>,
    pub only_new: Vec<(String, Ty)>,
    pub comparable: bool,
    pub combinations: u128,
    pub dates: Vec<String>,
    pub dates_only_old: Vec<String>,
    pub dates_only_new: Vec<String>,
    pub changes: Vec<DateChange>,
    /// The combinations on which some date compared changed.
    pub changed: u64,
}

impl DatesDiff {
    /// Whether the inputs differ: an input only one version takes, or a range that is not the same.
    pub fn inputs_differ(&self) -> bool {
        !self.only_old.is_empty() || !self.only_new.is_empty() || self.axes.iter().any(|a| a.old != a.new)
    }
}

pub enum Diff {
    Calendar(CalDiff),
    Dates(DatesDiff),
}

impl Diff {
    /// Whether anything differs (exit 1).
    pub fn differs(&self) -> bool {
        match self {
            Diff::Calendar(c) => !c.changes.is_empty() || !c.renamed.is_empty() || c.known.0 != c.known.1 || c.offset.0 != c.offset.1,
            Diff::Dates(d) => d.inputs_differ() || !d.changes.is_empty() || !d.dates_only_old.is_empty() || !d.dates_only_new.is_empty(),
        }
    }
}

/// What stops a diff before it compares (exit 2): two of different kinds, a side that does not pass
/// check, more combinations than the budget.
fn not_passing(s: &Side) -> Option<Text> {
    let errors = s.outcome.diags.iter().filter(|d| d.is_error()).count();
    if errors == 0 && s.outcome.checked.is_some() {
        return None;
    }
    let shown = &s.shown;
    let n = count(errors.max(1) as u64);
    Some(tr!(
        "`{shown}` は検査を通りません（エラー {n} 件）。`koyomi check` で確かめてください",
        "`{shown}` does not pass check ({n} {}); run `koyomi check` on it",
        ; if errors == 1 { "error" } else { "errors" }
    ))
}

/// The diff of two sides (old, new).
pub fn diff(old: &Side, new: &Side, budget: u64) -> Result<Diff, Text> {
    for s in [old, new] {
        if let Some(t) = not_passing(s) {
            return Err(t);
        }
    }
    match (&old.outcome.checked, &new.outcome.checked) {
        (Some(Checked::Calendar(o)), Some(Checked::Calendar(n))) => Ok(Diff::Calendar(calendars(old, o, new, n))),
        (Some(Checked::Dates(o, _)), Some(Checked::Dates(n, _))) => dates(old, o, new, n, budget).map(Diff::Dates),
        _ => Err(tr!(
            "`{}` と `{}` は種類が違います（カレンダーと日付のファイル）。比べられるのは、同じ種類の二つです",
            "`{}` and `{}` are of different kinds (a calendar and a dates file); a diff compares two of a kind",
            old.shown,
            new.shown
        )),
    }
}

fn calendars(os: &Side, o: &Calendar, ns: &Side, n: &Calendar) -> CalDiff {
    let head = |s: &Side, c: &Calendar| Head { file: s.shown.clone(), name: c.info.name.clone(), version: c.info.version.clone() };
    let (from, to) = (o.data.0.max(n.data.0), o.data.1.min(n.data.1));
    let compared = (from <= to).then_some((from, to));
    let mut changes: Vec<CalChange> = Vec::new();
    let mut renamed: Vec<Renamed> = Vec::new();
    if let Some((from, to)) = compared {
        for z in from.0..=to.0 {
            let d = Day(z);
            let (oo, no) = (o.is_open(d).unwrap_or(true), n.is_open(d).unwrap_or(true));
            if oo && no {
                continue;
            }
            let or = if oo { Vec::new() } else { o.reasons(d) };
            let nr = if no { Vec::new() } else { n.reasons(d) };
            if oo != no {
                match changes.last_mut() {
                    Some(c) if c.to.0 + 1 == z && c.old == oo && c.new == no && c.old_reasons == or && c.new_reasons == nr => c.to = d,
                    _ => changes.push(CalChange { from: d, to: d, old: oo, new: no, old_reasons: or, new_reasons: nr }),
                }
            } else if or != nr {
                match renamed.last_mut() {
                    Some(r) if r.to.0 + 1 == z && r.old_reasons == or && r.new_reasons == nr => r.to = d,
                    _ => renamed.push(Renamed { from: d, to: d, old_reasons: or, new_reasons: nr }),
                }
            }
        }
    }
    CalDiff { old: head(os, o), new: head(ns, n), compared, changes, renamed, known: (o.data, n.data), offset: (o.offset, n.offset) }
}

/// Where each date's value is among the values [`vectors::expect`] gives: its index, and whether its
/// time follows it.
fn places(m: &Model) -> Vec<(usize, bool)> {
    let timed = m.cal.as_ref().and_then(|c| c.offset).is_some();
    let mut out = Vec::new();
    let mut at = 0;
    for d in &m.dates {
        let t = d.at.is_some() && timed;
        out.push((at, t));
        at += if t { 2 } else { 1 };
    }
    out
}

fn ty_word(t: Ty) -> &'static str {
    match t {
        Ty::Date => "date",
        Ty::Int => "int",
    }
}

/// A run of changed combinations being put together, for one date.
struct Run {
    change: DateChange,
    last: i64,
    key: (Option<i64>, Option<&'static str>, Option<&'static str>),
}

fn dates(os: &Side, o: &Model, ns: &Side, n: &Model, budget: u64) -> Result<DatesDiff, Text> {
    let head = |s: &Side, m: &Model| Head { file: s.shown.clone(), name: m.file.name.text.clone(), version: m.file.version.clone() };
    let mut axes = Vec::new();
    let (mut only_old, mut only_new) = (Vec::new(), Vec::new());
    for i in &n.inputs {
        match o.inputs.iter().find(|x| x.name == i.name) {
            Some(x) if x.ty == i.ty => axes.push(Axis { name: i.name.clone(), ty: i.ty, old: (x.lo, x.hi), new: (i.lo, i.hi), lo: x.lo.max(i.lo), hi: x.hi.min(i.hi) }),
            _ => only_new.push((i.name.clone(), i.ty)),
        }
    }
    for x in &o.inputs {
        if !axes.iter().any(|a| a.name == x.name) {
            only_old.push((x.name.clone(), x.ty));
        }
    }
    let comparable = only_old.is_empty() && only_new.is_empty();
    let date_axis = axes.iter().position(|a| a.ty == Ty::Date);
    let mut dates = Vec::new();
    let mut pairs = Vec::new();
    for (kn, d) in n.dates.iter().enumerate() {
        if let Some(ko) = o.dates.iter().position(|x| x.name == d.name) {
            dates.push(d.name.clone());
            pairs.push((ko, kn));
        }
    }
    let dates_only_old: Vec<String> = o.dates.iter().filter(|x| !n.dates.iter().any(|d| d.name == x.name)).map(|x| x.name.clone()).collect();
    let dates_only_new: Vec<String> = n.dates.iter().filter(|d| !o.dates.iter().any(|x| x.name == d.name)).map(|d| d.name.clone()).collect();
    let combinations: u128 = if comparable { axes.iter().map(Axis::size).product() } else { 0 };
    let mut out = DatesDiff {
        old: head(os, o),
        new: head(ns, n),
        axes,
        date_axis,
        only_old,
        only_new,
        comparable,
        combinations,
        dates,
        dates_only_old,
        dates_only_new,
        changes: Vec::new(),
        changed: 0,
    };
    if !comparable || combinations == 0 {
        return Ok(out);
    }
    if combinations > budget as u128 {
        let (c, b) = (count(combinations.min(u64::MAX as u128) as u64), count(budget));
        return Err(tr!(
            "両方の版が受け取る入力の組み合わせは {c} 通りで、上限 {b} を超えます。`--budget` を大きくしてください",
            "the inputs both versions take have {c} combinations, over the budget of {b}; give a larger `--budget`"
        ));
    }
    let Some(da) = date_axis else { return Ok(out) };
    // each version's inputs, by the axis that gives them
    let from_axes = |m: &Model| -> Vec<usize> { m.inputs.iter().map(|i| out.axes.iter().position(|a| a.name == i.name).unwrap_or(0)).collect() };
    let (to_o, to_n) = (from_axes(o), from_axes(n));
    let (po, pn) = (places(o), places(n));
    let others: Vec<usize> = (0..out.axes.len()).filter(|&k| k != da).collect();
    let mut vals: Vec<i64> = out.axes.iter().map(|a| a.lo).collect();
    let (mut vo, mut vn) = (vec![0i64; o.inputs.len()], vec![0i64; n.inputs.len()]);
    let (mut out_o, mut out_n) = (vec![Day(0); o.dates.len()], vec![Day(0); n.dates.len()]);
    let mut runs: Vec<Option<Run>> = (0..pairs.len()).map(|_| None).collect();
    let mut by_date: Vec<Vec<DateChange>> = vec![Vec::new(); pairs.len()];
    let value = |e: &Expect, out: &[Day], p: (usize, bool), k: usize| -> Val {
        match e {
            Expect::Values(vs) => Val::Day(out[k], p.1.then(|| vs.get(p.0 + 1).cloned().unwrap_or_default())),
            Expect::Error(kind) => Val::Stop(kind),
            Expect::Open(_) => Val::Stop("bug"),
        }
    };
    let flush = |runs: &mut Vec<Option<Run>>, by_date: &mut Vec<Vec<DateChange>>, i: usize| {
        if let Some(r) = runs[i].take() {
            by_date[i].push(r.change);
        }
    };
    loop {
        for z in out.axes[da].lo..=out.axes[da].hi {
            vals[da] = z;
            for (k, a) in to_o.iter().enumerate() {
                vo[k] = vals[*a];
            }
            for (k, a) in to_n.iter().enumerate() {
                vn[k] = vals[*a];
            }
            let eo = vectors::expect(o, &vo, &mut out_o);
            let en = vectors::expect(n, &vn, &mut out_n);
            let mut any = false;
            for (i, &(ko, kn)) in pairs.iter().enumerate() {
                let (a, b) = (value(&eo, &out_o, po[ko], ko), value(&en, &out_n, pn[kn], kn));
                if a == b {
                    flush(&mut runs, &mut by_date, i);
                    continue;
                }
                any = true;
                let key = match (&a, &b) {
                    (Val::Day(x, _), Val::Day(y, _)) => (Some((y.0 - x.0) as i64), None, None),
                    (Val::Stop(k), Val::Day(..)) => (None, Some(*k), None),
                    (Val::Day(..), Val::Stop(k)) => (None, None, Some(*k)),
                    (Val::Stop(k), Val::Stop(l)) => (None, Some(*k), Some(*l)),
                };
                match &mut runs[i] {
                    Some(r) if r.last + 1 == z && r.key == key => {
                        r.last = z;
                        r.change.first.1 = z;
                        r.change.old.1 = a;
                        r.change.new.1 = b;
                        r.change.combinations += 1;
                    }
                    _ => {
                        flush(&mut runs, &mut by_date, i);
                        let change = DateChange {
                            date: out.dates[i].clone(),
                            first: (z, z),
                            others: others.iter().map(|&k| (k, vals[k])).collect(),
                            old: (a.clone(), a),
                            new: (b.clone(), b),
                            shift: key.0,
                            combinations: 1,
                        };
                        runs[i] = Some(Run { change, last: z, key });
                    }
                }
            }
            if any {
                out.changed += 1;
            }
        }
        for i in 0..pairs.len() {
            flush(&mut runs, &mut by_date, i);
        }
        // the other inputs: the last declared turns fastest
        let mut moved = false;
        for &k in others.iter().rev() {
            if vals[k] < out.axes[k].hi {
                vals[k] += 1;
                moved = true;
                break;
            }
            vals[k] = out.axes[k].lo;
        }
        if !moved {
            break;
        }
    }
    out.changes = by_date.into_iter().flatten().collect();
    Ok(out)
}

// ── Saying it ───────────────────────────────────────────────────────────

fn span(from: Day, to: Day, lang: Lang) -> String {
    if from == to {
        from.to_string()
    } else {
        match lang {
            Lang::Ja => format!("{from}〜{to}"),
            Lang::En => format!("{from}..{to}"),
        }
    }
}

fn days(from: Day, to: Day) -> u64 {
    (to.0 - from.0 + 1).max(0) as u64
}

fn days_text(n: u64) -> Text {
    let c = count(n);
    tr!("{c} 日", "{c} {}", ; if n == 1 { "day" } else { "days" })
}

fn open_text(open: bool) -> Text {
    if open { tr!("開いている", "open") } else { tr!("閉じている", "closed") }
}

fn reasons_text(rs: &[Reason], lang: Lang) -> String {
    let parts: Vec<String> = rs.iter().map(|r| r.text().get(lang).to_string()).collect();
    parts.join(if lang == Lang::Ja { "、" } else { ", " })
}

/// `open`, `closed (Christmas Day)`.
fn answer(open: bool, rs: &[Reason], lang: Lang) -> String {
    let a = open_text(open).get(lang).to_string();
    if rs.is_empty() {
        a
    } else {
        match lang {
            Lang::Ja => format!("{a}（{}）", reasons_text(rs, lang)),
            Lang::En => format!("{a} ({})", reasons_text(rs, lang)),
        }
    }
}

fn offset_text(o: Option<i32>) -> Text {
    match o {
        None => tr!("なし", "none"),
        Some(m) => {
            let sign = if m < 0 { '-' } else { '+' };
            let m = m.abs();
            Text::same(format!("{sign}{:02}:{:02}", m / 60, m % 60))
        }
    }
}

fn colon(lang: Lang) -> &'static str {
    if lang == Lang::Ja { "：" } else { ": " }
}

fn stop_text(kind: &str) -> Text {
    let why = match kind {
        "data" => tr!("カレンダーが知らない日", "a day the calendar does not know"),
        "reject" => tr!("無い日", "a day that is not there"),
        "date" => tr!("扱える日付の外", "outside the dates there are"),
        "range" => tr!("範囲の外", "outside the range"),
        _ => tr!("計算の誤り", "a fault of the computation"),
    };
    tr!("止まる（{}）", "stops ({})", why.ja; why.en)
}

fn val_text(v: &Val, lang: Lang) -> String {
    match v {
        Val::Day(d, None) => d.to_string(),
        Val::Day(d, Some(t)) => match lang {
            Lang::Ja => format!("{d}（{t}）"),
            Lang::En => format!("{d} ({t})"),
        },
        Val::Stop(k) => stop_text(k).get(lang).to_string(),
    }
}

/// The old or the new values of a run: one value, or the first and the last.
fn vals_text(v: &(Val, Val), lang: Lang) -> String {
    if v.0 == v.1 {
        return val_text(&v.0, lang);
    }
    match (&v.0, &v.1) {
        (Val::Day(a, None), Val::Day(b, None)) => span(*a, *b, lang),
        _ => format!("{}{}{}", val_text(&v.0, lang), if lang == Lang::Ja { "〜" } else { ".." }, val_text(&v.1, lang)),
    }
}

fn shift_text(s: Option<i64>) -> Option<Text> {
    let s = s?;
    let n = count(s.unsigned_abs());
    let unit = if s.abs() == 1 { "day" } else { "days" };
    Some(match s {
        0 => tr!("同じ日で、時刻が変わる", "the same day, at another time"),
        s if s < 0 => tr!("{n} 日早くなる", "{n} {unit} earlier"),
        _ => tr!("{n} 日遅くなる", "{n} {unit} later"),
    })
}

fn kind_text(d: &Diff) -> Text {
    match d {
        Diff::Calendar(_) => tr!("カレンダー", "calendar"),
        Diff::Dates(_) => tr!("日付のファイル", "dates"),
    }
}

/// `office v1 → v2`, or both names when the versions name themselves otherwise; `q` quotes a name.
fn title(old: &Head, new: &Head, q: &str) -> String {
    if old.name == new.name {
        format!("{q}{}{q} v{} → v{}", old.name, old.version, new.version)
    } else {
        format!("{q}{}{q} v{} → {q}{}{q} v{}", old.name, old.version, new.name, new.version)
    }
}

fn heads(d: &Diff) -> (&Head, &Head) {
    match d {
        Diff::Calendar(c) => (&c.old, &c.new),
        Diff::Dates(x) => (&x.old, &x.new),
    }
}

fn range_text(a: &Axis, r: (i64, i64), lang: Lang) -> String {
    match a.ty {
        Ty::Date => span(Day(r.0 as i32), Day(r.1 as i32), lang),
        Ty::Int => format!("{}{}{}", r.0, if lang == Lang::Ja { "〜" } else { ".." }, r.1),
    }
}

/// The inputs of a change as the text writes them: `received 2026-04-21..2026-05-20, days 3`.
fn inputs_text(d: &DatesDiff, c: &DateChange, lang: Lang) -> String {
    let mut parts = Vec::new();
    if let Some(da) = d.date_axis {
        let a = &d.axes[da];
        parts.push(format!("{} {}", a.name, range_text(a, c.first, lang)));
    }
    for (k, v) in &c.others {
        parts.push(format!("{} {}", d.axes[*k].name, d.axes[*k].show(*v)));
    }
    parts.join(if lang == Lang::Ja { "、" } else { ", " })
}

fn change_text(d: &DatesDiff, c: &DateChange, lang: Lang) -> String {
    let mut s = format!("{}{}{} → {}", inputs_text(d, c, lang), colon(lang), vals_text(&c.old, lang), vals_text(&c.new, lang));
    if let Some(t) = shift_text(c.shift) {
        s.push_str(&match lang {
            Lang::Ja => format!("（{}）", t.ja),
            Lang::En => format!(" ({})", t.en),
        });
    }
    s
}

/// The diff as text, `--lang` `lang`.
pub fn text(d: &Diff, lang: Lang) -> String {
    let mut o = String::new();
    let (old, new) = heads(d);
    let kind = kind_text(d);
    o.push_str(&format!("koyomi diff{}{} {}\n", colon(lang), kind.get(lang), title(old, new, "")));
    o.push_str(&format!("  {}{}{}\n", tr!("古い版", "old").get(lang), colon(lang), old.file));
    o.push_str(&format!("  {}{}{}\n\n", tr!("新しい版", "new").get(lang), colon(lang), new.file));
    match d {
        Diff::Calendar(c) => calendar_text(c, lang, &mut o),
        Diff::Dates(x) => dates_text(x, lang, &mut o),
    }
    o
}

fn more(n: usize, lang: Lang) -> String {
    let c = count(n as u64);
    format!("  {}\n", tr!("ほかに {c} 区間（JSON に全部あります）", "and {c} more (the JSON has them all)").get(lang))
}

fn calendar_text(c: &CalDiff, lang: Lang, o: &mut String) {
    match c.compared {
        Some((f, t)) => {
            let n = days_text(days(f, t));
            o.push_str(&tr!(
                "比べた日：{}（{}。両方の版が知っている日）\n",
                "compared: {} ({}, the days both versions know)\n",
                span(f, t, lang), n.ja; span(f, t, lang), n.en
            ).get(lang).to_string());
        }
        None => o.push_str(tr!("両方の版が知っている日がありません\n", "the two versions know no day in common\n").get(lang)),
    }
    let mut shown = 0;
    if !c.changes.is_empty() {
        let total: u64 = c.changes.iter().map(|x| days(x.from, x.to)).sum();
        let (k, n) = (count(c.changes.len() as u64), days_text(total));
        o.push('\n');
        o.push_str(&tr!(
            "開いているか閉じているかが変わった日（{k} 区間、{}）：\n",
            "open or closed changed ({k} {}, {}):\n",
            n.ja; if c.changes.len() == 1 { "interval" } else { "intervals" }, n.en
        ).get(lang).to_string());
        for x in &c.changes {
            if shown == SHOWN {
                break;
            }
            shown += 1;
            o.push_str(&format!("  {}{}{} → {}\n", span(x.from, x.to, lang), colon(lang), answer(x.old, &x.old_reasons, lang), answer(x.new, &x.new_reasons, lang)));
        }
        if c.changes.len() > shown {
            o.push_str(&more(c.changes.len() - shown, lang));
        }
    }
    if !c.renamed.is_empty() {
        let total: u64 = c.renamed.iter().map(|x| days(x.from, x.to)).sum();
        let (k, n) = (count(c.renamed.len() as u64), days_text(total));
        o.push('\n');
        o.push_str(&tr!(
            "閉じたままで、理由だけが変わった日（{k} 区間、{}）：\n",
            "closed in both, for other reasons ({k} {}, {}):\n",
            n.ja; if c.renamed.len() == 1 { "interval" } else { "intervals" }, n.en
        ).get(lang).to_string());
        let before = shown;
        for x in &c.renamed {
            if shown == SHOWN {
                break;
            }
            shown += 1;
            o.push_str(&format!("  {}{}{} → {}\n", span(x.from, x.to, lang), colon(lang), reasons_text(&x.old_reasons, lang), reasons_text(&x.new_reasons, lang)));
        }
        if c.renamed.len() > shown - before {
            o.push_str(&more(c.renamed.len() - (shown - before), lang));
        }
    }
    let mut said = false;
    if c.known.0 != c.known.1 {
        let ((a, b), (x, y)) = c.known;
        o.push('\n');
        said = true;
        o.push_str(&format!("{}{}{} → {}\n", tr!("知っている日", "the days known").get(lang), colon(lang), span(a, b, lang), span(x, y, lang)));
    }
    if c.offset.0 != c.offset.1 {
        if !said {
            o.push('\n');
        }
        o.push_str(&format!("{}{}{} → {}\n", tr!("時差", "offset").get(lang), colon(lang), offset_text(c.offset.0).get(lang), offset_text(c.offset.1).get(lang)));
    }
    if let Some((f, t)) = c.compared {
        o.push('\n');
        o.push_str(&same_days(c, f, t, lang, false));
    }
}

/// The closing sentence of a calendar's diff: the other days of the overlap get the same answer.
fn same_days(c: &CalDiff, f: Day, t: Day, lang: Lang, capital: bool) -> String {
    let t = if c.changes.is_empty() && c.renamed.is_empty() {
        tr!("{} のどの日も、同じ答えです。\n", "every day of {} gets the same answer.\n", span(f, t, lang))
    } else {
        tr!("{} のほかの日は、どれも同じ答えです。\n", "every other day of {} gets the same answer.\n", span(f, t, lang))
    };
    if capital { ritsu_base::text::capitalize(t.get(lang)) } else { t.get(lang).to_string() }
}

/// The closing sentence of a dates file's diff: the other combinations get the same values.
fn same_values(d: &DatesDiff, lang: Lang, capital: bool) -> String {
    let t = if d.changes.is_empty() {
        tr!("どの組み合わせでも、どの日付も同じ値になります。\n", "for every combination, every date gets the same value.\n")
    } else {
        tr!("ほかの組み合わせでは、どの日付も同じ値になります。\n", "for every other combination, every date gets the same value.\n")
    };
    if capital { ritsu_base::text::capitalize(t.get(lang)) } else { t.get(lang).to_string() }
}

fn input_lines(d: &DatesDiff, lang: Lang) -> Vec<String> {
    let mut out = Vec::new();
    for a in &d.axes {
        if a.old != a.new {
            out.push(format!("  {}{}{} → {}", a.name, colon(lang), range_text(a, a.old, lang), range_text(a, a.new, lang)));
        }
    }
    let list = |xs: &[(String, Ty)]| match lang {
        Lang::Ja => xs.iter().map(|(n, t)| format!("{n}（{}）", ty_word(*t))).collect::<Vec<_>>().join("、"),
        Lang::En => xs.iter().map(|(n, t)| format!("{n} ({})", ty_word(*t))).collect::<Vec<_>>().join(", "),
    };
    if !d.only_old.is_empty() {
        out.push(format!("  {}{}{}", tr!("古い版だけの入力", "only in the old version").get(lang), colon(lang), list(&d.only_old)));
    }
    if !d.only_new.is_empty() {
        out.push(format!("  {}{}{}", tr!("新しい版だけの入力", "only in the new version").get(lang), colon(lang), list(&d.only_new)));
    }
    out
}

fn dates_text(d: &DatesDiff, lang: Lang, o: &mut String) {
    let inputs = input_lines(d, lang);
    if !inputs.is_empty() {
        o.push_str(&format!("{}\n", tr!("入力の違い：", "the inputs differ:").get(lang)));
        for l in &inputs {
            o.push_str(l);
            o.push('\n');
        }
        o.push('\n');
    }
    if !d.comparable {
        o.push_str(tr!(
            "二つの版が受け取る入力が違うので、両方に渡せる入力がありません。答えは比べず、入力の違いだけを出しました。\n",
            "the two versions take different inputs, so no input can be given to both: the answers are not compared, only the inputs.\n"
        ).get(lang));
        return;
    }
    let axes: Vec<String> = d.axes.iter().map(|a| format!("{} {}", a.name, range_text(a, (a.lo, a.hi), lang))).collect();
    let c = count(d.combinations.min(u64::MAX as u128) as u64);
    o.push_str(&tr!(
        "比べた入力：{}（組み合わせ {c} 通り）\n",
        "inputs compared: {} ({c} combinations)\n",
        axes.join("、"); axes.join(", ")
    ).get(lang).to_string());
    o.push_str(&format!("{}{}{}\n", tr!("比べた日付", "dates compared").get(lang), colon(lang), d.dates.join(if lang == Lang::Ja { "、" } else { ", " })));
    if !d.dates_only_old.is_empty() {
        o.push_str(&format!("{}{}{}\n", tr!("古い版だけの日付", "dates only in the old version").get(lang), colon(lang), d.dates_only_old.join(if lang == Lang::Ja { "、" } else { ", " })));
    }
    if !d.dates_only_new.is_empty() {
        o.push_str(&format!("{}{}{}\n", tr!("新しい版だけの日付", "dates only in the new version").get(lang), colon(lang), d.dates_only_new.join(if lang == Lang::Ja { "、" } else { ", " })));
    }
    for date in &d.dates {
        let cs: Vec<&DateChange> = d.changes.iter().filter(|c| &c.date == date).collect();
        if cs.is_empty() {
            continue;
        }
        let combos: u64 = cs.iter().map(|c| c.combinations).sum();
        let (k, n) = (count(cs.len() as u64), count(combos));
        o.push('\n');
        o.push_str(&tr!(
            "{date}：{k} 区間、{n} 通り\n",
            "{date}: {k} {}, {n} {}\n",
            ; if cs.len() == 1 { "interval" } else { "intervals" }, if combos == 1 { "combination" } else { "combinations" }
        ).get(lang).to_string());
        for c in cs.iter().take(SHOWN) {
            o.push_str(&format!("  {}\n", change_text(d, c, lang)));
        }
        if cs.len() > SHOWN {
            o.push_str(&more(cs.len() - SHOWN, lang));
        }
    }
    o.push('\n');
    o.push_str(&same_values(d, lang, false));
}

/// The diff as Markdown, `--lang` `lang`.
pub fn markdown(d: &Diff, lang: Lang) -> String {
    let mut o = String::new();
    let (old, new) = heads(d);
    let kind = kind_text(d);
    o.push_str(&format!("### koyomi diff{}{} {}\n\n", colon(lang), kind.get(lang), title(old, new, "`")));
    o.push_str(&format!("- {}{}`{}`\n", tr!("古い版", "Old").get(lang), colon(lang), old.file));
    o.push_str(&format!("- {}{}`{}`\n", tr!("新しい版", "New").get(lang), colon(lang), new.file));
    match d {
        Diff::Calendar(c) => {
            if let Some((f, t)) = c.compared {
                let n = days_text(days(f, t));
                o.push_str(&format!("- {}{}{}（{}）\n", tr!("比べた日", "Compared").get(lang), colon(lang), span(f, t, lang), n.get(lang)).replace("（", if lang == Lang::Ja { "（" } else { " (" }).replace("）", if lang == Lang::Ja { "）" } else { ")" }));
            }
            if c.known.0 != c.known.1 {
                let ((a, b), (x, y)) = c.known;
                o.push_str(&format!("- {}{}{} → {}\n", tr!("知っている日", "The days known").get(lang), colon(lang), span(a, b, lang), span(x, y, lang)));
            }
            if c.offset.0 != c.offset.1 {
                o.push_str(&format!("- {}{}{} → {}\n", tr!("時差", "Offset").get(lang), colon(lang), offset_text(c.offset.0).get(lang), offset_text(c.offset.1).get(lang)));
            }
            let mut shown = 0;
            if !c.changes.is_empty() {
                o.push_str(&format!("\n#### {}\n\n", tr!("開いているか閉じているかが変わった日", "Open or closed changed").get(lang)));
                o.push_str(&format!("| {} | {} | {} |\n|---|---|---|\n", tr!("日", "Days").get(lang), tr!("古い版", "Old").get(lang), tr!("新しい版", "New").get(lang)));
                for x in &c.changes {
                    if shown == SHOWN {
                        break;
                    }
                    shown += 1;
                    o.push_str(&format!("| {} | {} | {} |\n", span(x.from, x.to, lang), cell(&answer(x.old, &x.old_reasons, lang)), cell(&answer(x.new, &x.new_reasons, lang))));
                }
                if c.changes.len() > shown {
                    o.push_str(&format!("\n{}", more(c.changes.len() - shown, lang).trim_start()));
                }
            }
            if !c.renamed.is_empty() {
                let before = shown;
                o.push_str(&format!("\n#### {}\n\n", tr!("閉じたままで、理由だけが変わった日", "Closed in both, for other reasons").get(lang)));
                o.push_str(&format!("| {} | {} | {} |\n|---|---|---|\n", tr!("日", "Days").get(lang), tr!("古い版の理由", "Old reasons").get(lang), tr!("新しい版の理由", "New reasons").get(lang)));
                for x in &c.renamed {
                    if shown == SHOWN {
                        break;
                    }
                    shown += 1;
                    o.push_str(&format!("| {} | {} | {} |\n", span(x.from, x.to, lang), cell(&reasons_text(&x.old_reasons, lang)), cell(&reasons_text(&x.new_reasons, lang))));
                }
                if c.renamed.len() > shown - before {
                    o.push_str(&format!("\n{}", more(c.renamed.len() - (shown - before), lang).trim_start()));
                }
            }
            if let Some((f, t)) = c.compared {
                o.push('\n');
                o.push_str(&same_days(c, f, t, lang, true));
            }
        }
        Diff::Dates(x) => {
            if x.comparable {
                let axes: Vec<String> = x.axes.iter().map(|a| format!("{} {}", a.name, range_text(a, (a.lo, a.hi), lang))).collect();
                let c = count(x.combinations.min(u64::MAX as u128) as u64);
                o.push_str(&tr!(
                    "- 比べた入力：{}（組み合わせ {c} 通り）\n",
                    "- Inputs compared: {} ({c} combinations)\n",
                    axes.join("、"); axes.join(", ")
                ).get(lang).to_string());
                let join = if lang == Lang::Ja { "、" } else { ", " };
                o.push_str(&format!("- {}{}{}\n", tr!("比べた日付", "Dates compared").get(lang), colon(lang), x.dates.join(join)));
                if !x.dates_only_old.is_empty() {
                    o.push_str(&format!("- {}{}{}\n", tr!("古い版だけの日付", "Dates only in the old version").get(lang), colon(lang), x.dates_only_old.join(join)));
                }
                if !x.dates_only_new.is_empty() {
                    o.push_str(&format!("- {}{}{}\n", tr!("新しい版だけの日付", "Dates only in the new version").get(lang), colon(lang), x.dates_only_new.join(join)));
                }
            }
            let inputs = input_lines(x, lang);
            if !inputs.is_empty() {
                o.push_str(&format!("\n#### {}\n\n", tr!("入力の違い", "The inputs differ").get(lang)));
                for l in &inputs {
                    o.push_str(&format!("- {}\n", l.trim_start()));
                }
            }
            if !x.comparable {
                o.push('\n');
                o.push_str(tr!(
                    "二つの版が受け取る入力が違うので、両方に渡せる入力がありません。答えは比べず、入力の違いだけを出しました。\n",
                    "The two versions take different inputs, so no input can be given to both: the answers are not compared, only the inputs.\n"
                ).get(lang));
                return o;
            }
            for date in &x.dates {
                let cs: Vec<&DateChange> = x.changes.iter().filter(|c| &c.date == date).collect();
                if cs.is_empty() {
                    continue;
                }
                o.push_str(&format!("\n#### `{date}`\n\n"));
                o.push_str(&format!(
                    "| {} | {} | {} | {} | {} |\n|---|---|---|---|---|\n",
                    tr!("入力", "Inputs").get(lang),
                    tr!("通り", "Combinations").get(lang),
                    tr!("古い版", "Old").get(lang),
                    tr!("新しい版", "New").get(lang),
                    tr!("ずれ", "Shift").get(lang)
                ));
                for c in cs.iter().take(SHOWN) {
                    let shift = shift_text(c.shift).map(|t| t.get(lang).to_string()).unwrap_or_default();
                    o.push_str(&format!(
                        "| {} | {} | {} | {} | {} |\n",
                        cell(&inputs_text(x, c, lang)),
                        count(c.combinations),
                        cell(&vals_text(&c.old, lang)),
                        cell(&vals_text(&c.new, lang)),
                        cell(&shift)
                    ));
                }
                if cs.len() > SHOWN {
                    o.push_str(&format!("\n{}", more(cs.len() - SHOWN, lang).trim_start()));
                }
            }
            o.push('\n');
            o.push_str(&same_values(x, lang, true));
        }
    }
    o
}

/// A table cell: `|` escaped.
fn cell(s: &str) -> String {
    s.replace('|', "\\|")
}

fn head_json(h: &Head) -> Value {
    json!({"file": h.file, "name": h.name, "version": h.version})
}

fn day_range(f: Day, t: Day) -> Value {
    json!({"from": f.to_string(), "to": t.to_string()})
}

fn reasons_json(rs: &[Reason], lang: Lang) -> Value {
    Value::Array(rs.iter().map(|r| Value::String(r.text().get(lang).to_string())).collect())
}

fn val_json(v: &Val) -> Value {
    match v {
        Val::Day(d, None) => json!(d.to_string()),
        Val::Day(d, Some(t)) => json!({"date": d.to_string(), "at": t}),
        Val::Stop(k) => json!({"error": k}),
    }
}

fn axis_value(a: &Axis, v: i64) -> Value {
    match a.ty {
        Ty::Date => json!(Day(v as i32).to_string()),
        Ty::Int => json!(v),
    }
}

/// The diff as JSON (ritsu's DESIGN 17.21): every interval, the reasons in `lang`.
pub fn to_json(d: &Diff, lang: Lang) -> Value {
    let version = env!("CARGO_PKG_VERSION");
    match d {
        Diff::Calendar(c) => {
            let mut o = Map::new();
            o.insert("koyomi".into(), json!(version));
            o.insert("kind".into(), json!("calendar"));
            o.insert("old".into(), head_json(&c.old));
            o.insert("new".into(), head_json(&c.new));
            o.insert("compared".into(), match c.compared {
                Some((f, t)) => json!({"from": f.to_string(), "to": t.to_string(), "days": days(f, t)}),
                None => Value::Null,
            });
            let word = |open: bool| if open { "open" } else { "closed" };
            o.insert(
                "changes".into(),
                Value::Array(
                    c.changes
                        .iter()
                        .map(|x| {
                            json!({"from": x.from.to_string(), "to": x.to.to_string(), "days": days(x.from, x.to), "old": word(x.old), "new": word(x.new),
                                   "old_reasons": reasons_json(&x.old_reasons, lang), "new_reasons": reasons_json(&x.new_reasons, lang)})
                        })
                        .collect(),
                ),
            );
            o.insert(
                "renamed".into(),
                Value::Array(
                    c.renamed
                        .iter()
                        .map(|x| json!({"from": x.from.to_string(), "to": x.to.to_string(), "old_reasons": reasons_json(&x.old_reasons, lang), "new_reasons": reasons_json(&x.new_reasons, lang)}))
                        .collect(),
                ),
            );
            o.insert("known".into(), json!({"old": day_range(c.known.0.0, c.known.0.1), "new": day_range(c.known.1.0, c.known.1.1)}));
            o.insert("offset".into(), if c.offset.0 != c.offset.1 { json!({"old": offset_text(c.offset.0).en, "new": offset_text(c.offset.1).en}) } else { Value::Null });
            o.insert("same_elsewhere".into(), json!(c.compared.is_some()));
            o.insert("total".into(), json!({"changed_days": c.changes.iter().map(|x| days(x.from, x.to)).sum::<u64>()}));
            Value::Object(o)
        }
        Diff::Dates(x) => {
            let mut o = Map::new();
            o.insert("koyomi".into(), json!(version));
            o.insert("kind".into(), json!("dates"));
            o.insert("old".into(), head_json(&x.old));
            o.insert("new".into(), head_json(&x.new));
            let compared: Vec<Value> = x
                .axes
                .iter()
                .map(|a| {
                    json!({"name": a.name, "type": ty_word(a.ty), "from": axis_value(a, a.lo), "to": axis_value(a, a.hi),
                           "old": {"from": axis_value(a, a.old.0), "to": axis_value(a, a.old.1)}, "new": {"from": axis_value(a, a.new.0), "to": axis_value(a, a.new.1)}})
                })
                .collect();
            let names = |xs: &[(String, Ty)]| Value::Array(xs.iter().map(|(n, t)| json!({"name": n, "type": ty_word(*t)})).collect());
            o.insert("inputs".into(), json!({"compared": compared, "only_old": names(&x.only_old), "only_new": names(&x.only_new)}));
            o.insert("comparable".into(), json!(x.comparable));
            o.insert("combinations".into(), json!(x.combinations.min(u64::MAX as u128) as u64));
            o.insert("dates".into(), json!({"compared": x.dates, "only_old": x.dates_only_old, "only_new": x.dates_only_new}));
            let changes: Vec<Value> = x
                .changes
                .iter()
                .map(|c| {
                    let mut inputs = Map::new();
                    if let Some(da) = x.date_axis {
                        let a = &x.axes[da];
                        inputs.insert(a.name.clone(), json!({"from": axis_value(a, c.first.0), "to": axis_value(a, c.first.1)}));
                    }
                    for (k, v) in &c.others {
                        inputs.insert(x.axes[*k].name.clone(), axis_value(&x.axes[*k], *v));
                    }
                    json!({"date": c.date, "inputs": inputs, "old": {"from": val_json(&c.old.0), "to": val_json(&c.old.1)}, "new": {"from": val_json(&c.new.0), "to": val_json(&c.new.1)},
                           "shift_days": c.shift, "combinations": c.combinations})
                })
                .collect();
            o.insert("changes".into(), Value::Array(changes));
            o.insert("same_elsewhere".into(), json!(x.comparable));
            o.insert("total".into(), json!({"combinations_changed": x.changed}));
            Value::Object(o)
        }
    }
}
