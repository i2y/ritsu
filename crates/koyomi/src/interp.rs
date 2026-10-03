//! The reference interpreter (DESIGN 0.3 P1, 2.2): what a `.cal` means is what this file
//! computes. `check`, `eval` and, in later stages, `vectors` and `doc` all compute through it.
//!
//! Two ways in. [`run`] computes every date of one input quickly, for the check's walk over
//! every input; [`trace`] computes the same and keeps a step for every line, for `eval` and
//! for the example a diagnostic shows. Both call [`apply`], so the two cannot disagree.

use crate::ast::{At, Cmp, Conv};
use crate::calendar::{Calendar, CalError, Reason, same_month};
use crate::date::{self, Day, DateError, Missing};
use crate::diag::Step;
use ritsu_base::text::Text;
use crate::paraphrase;
use crate::resolve::{A, CK, D, Model, R, ROp, Ref, Start};

/// Something an operation did that a report wants to know.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ev {
    /// The calendar moved the day (`roll`, `if closed`, `business days`).
    Moved { from: Day, to: Day },
    /// The month had no such day, and `else` said what to do.
    Else { y: i32, m: u32, d: u32, how: Missing },
    /// `if closed`: whether the day was closed.
    IfClosed { closed: bool },
}

/// Why an operation gives no day.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpFail {
    /// `else reject` on a day the month does not have (E202).
    Missing { y: i32, m: u32, d: u32 },
    /// The calendar was asked about a day outside its data range (E203).
    Outside(Day),
    /// Outside 0001-01-01..9999-12-31 (E204).
    OutOfRange,
    Bug(String),
}

impl From<DateError> for OpFail {
    fn from(e: DateError) -> OpFail {
        match e {
            DateError::Missing { y, m, d } => OpFail::Missing { y, m, d },
            DateError::OutOfRange => OpFail::OutOfRange,
            DateError::Bug(s) => OpFail::Bug(s),
        }
    }
}

impl From<CalError> for OpFail {
    fn from(e: CalError) -> OpFail {
        match e {
            CalError::Outside(d) => OpFail::Outside(d),
            CalError::Date(e) => e.into(),
        }
    }
}

/// Where a computation stopped: the date, the operation (None for its `at` line), and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stop {
    pub date: usize,
    pub op: Option<usize>,
    pub fail: OpFail,
}

/// A computed date's time, at `at` with the calendar's offset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Time {
    /// `2026-05-08T09:00:00+09:00`.
    pub local: String,
    /// `2026-05-08T00:00:00Z`: RFC 3339 in UTC, the form dandori's `timestamp` takes.
    pub utc: String,
}

pub fn value(a: &A, vals: &[i64]) -> i64 {
    match a {
        A::Lit(n) => *n,
        A::Input(k) => vals[*k],
    }
}

fn cal_of(cal: Option<&Calendar>) -> Result<&Calendar, OpFail> {
    cal.ok_or_else(|| OpFail::Bug("an operation asks a calendar the file does not have".into()))
}

/// One operation on one day (DESIGN 2.2).
pub fn apply(cal: Option<&Calendar>, d: Day, op: &ROp, vals: &[i64], ev: &mut Vec<Ev>) -> Result<Day, OpFail> {
    let p_of = |m: &Option<Missing>| m.unwrap_or(Missing::Never);
    Ok(match op {
        ROp::Days(s, n) => date::add_days(d, s * value(n, vals))?,
        ROp::Business(fwd, n) => {
            let r = cal_of(cal)?.add_business(d, value(n, vals), *fwd)?;
            if r != d {
                ev.push(Ev::Moved { from: d, to: r });
            }
            r
        }
        ROp::Months { sign, n, per, missing } => {
            let k = sign * per * value(n, vals);
            let (y, m, dd) = date::civil_from_days(d.0 as i64);
            let (y2, m2) = date::shift_month(y, m, k);
            let r = date::place(y2, m2, dd, p_of(missing))?;
            if (1..=9999).contains(&y2) && dd > date::month_len(y2, m2) {
                ev.push(Ev::Else { y: y2 as i32, m: m2, d: dd, how: p_of(missing) });
            }
            r
        }
        ROp::DayOfMonth { n, sign, k, missing } => {
            let nv = value(n, vals) as u32;
            let kv = sign * value(k, vals);
            let (y, m, _) = date::civil_from_days(d.0 as i64);
            let (y2, m2) = date::shift_month(y, m, kv);
            let r = date::place(y2, m2, nv, p_of(missing))?;
            if (1..=9999).contains(&y2) && nv > date::month_len(y2, m2) {
                ev.push(Ev::Else { y: y2 as i32, m: m2, d: nv, how: p_of(missing) });
            }
            r
        }
        ROp::StartOfMonth(sign, k) => date::start_of_month(d, sign * value(k, vals))?,
        ROp::EndOfMonth(sign, k) => date::end_of_month(d, sign * value(k, vals))?,
        ROp::CloseDay(n, missing) => {
            let nv = value(n, vals) as u32;
            let p = p_of(missing);
            let r = date::close_day(d, nv, p)?;
            let (ry, rm, rd) = r.ymd();
            if rd != nv {
                // The closing day was placed by `else`: the end of a month that has no such day,
                // or the first of the month after one.
                let (my, mm) = if p == Missing::StartOfNextMonth {
                    let (a, b) = date::shift_month(ry as i64, rm, -1);
                    (a as i32, b)
                } else {
                    (ry, rm)
                };
                ev.push(Ev::Else { y: my, m: mm, d: nv, how: p });
            }
            r
        }
        ROp::CloseEndOfMonth => date::close_end_of_month(d),
        ROp::Roll(c) => {
            let r = cal_of(cal)?.roll(d, *c)?;
            if r != d {
                ev.push(Ev::Moved { from: d, to: r });
            }
            r
        }
        ROp::IfClosed(inner) => {
            let closed = !cal_of(cal)?.is_open(d).map_err(|o| OpFail::Outside(o.0))?;
            ev.push(Ev::IfClosed { closed });
            if closed {
                let r = apply(cal, d, inner, vals, ev)?;
                if r != d {
                    ev.push(Ev::Moved { from: d, to: r });
                }
                r
            } else {
                d
            }
        }
    })
}

/// The time a date gives at `at`, with the calendar's offset (DESIGN 1.9).
pub fn time(d: Day, at: At, offset: i32) -> Result<Time, OpFail> {
    let local = d.0 as i64 * 1440
        + match at {
            At::Time(t) => t as i64,
            At::EndOfDay => 1440,
        };
    let utc = local - offset as i64;
    let fmt = |mins: i64| -> Result<(Day, i64), OpFail> {
        let day = mins.div_euclid(1440);
        if day < date::MIN.0 as i64 || day > date::MAX.0 as i64 {
            return Err(OpFail::OutOfRange);
        }
        Ok((Day(day as i32), mins.rem_euclid(1440)))
    };
    let (ld, lm) = fmt(local)?;
    let (ud, um) = fmt(utc)?;
    Ok(Time {
        local: format!("{ld}T{:02}:{:02}:00{}", lm / 60, lm % 60, crate::calendar::offset_text(offset)),
        utc: format!("{ud}T{:02}:{:02}:00Z", um / 60, um % 60),
    })
}

/// Every date of one input, in `out` by index. `ev` collects what the operations did, as
/// (date, operation, event).
pub fn run(m: &Model, vals: &[i64], out: &mut [Day], ev: &mut Vec<(u32, u32, Ev)>) -> Result<(), Stop> {
    let cal = m.cal.as_ref();
    let mut buf: Vec<Ev> = Vec::new();
    for &k in &m.order {
        let d = &m.dates[k];
        let mut x = match d.start {
            Start::Input => Day(vals[m.date_input] as i32),
            Start::Date(s) => out[s],
        };
        for (i, o) in d.ops.iter().enumerate() {
            buf.clear();
            x = apply(cal, x, &o.op, vals, &mut buf).map_err(|fail| Stop { date: k, op: Some(i), fail })?;
            for e in &buf {
                ev.push((k as u32, i as u32, *e));
            }
        }
        if let (Some((at, _)), Some(off)) = (d.at, cal.and_then(|c| c.offset)) {
            time(x, at, off).map_err(|fail| Stop { date: k, op: None, fail })?;
        }
        out[k] = x;
    }
    Ok(())
}

/// One side of a claim.
pub fn side(cal: Option<&Calendar>, r: &R, vals: &[i64], m: &Model, dates: &[Day]) -> Result<Day, OpFail> {
    let base = match r.what {
        Ref::Input => Day(vals[m.date_input] as i32),
        Ref::Date(k) => dates[k],
    };
    match &r.offset {
        None => Ok(base),
        Some((fwd, n, false)) => Ok(date::add_days(base, if *fwd { value(n, vals) } else { -value(n, vals) })?),
        Some((fwd, n, true)) => Ok(cal_of(cal)?.add_business(base, value(n, vals), *fwd)?),
    }
}

/// What a claim says of one input: whether it holds, and for a comparison the two sides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Verdict {
    pub holds: bool,
    pub sides: Option<(Day, Day)>,
}

/// A claim on one input. `is monotonic` compares two inputs, so the check does it.
pub fn claim(m: &Model, c: &CK, vals: &[i64], dates: &[Day]) -> Result<Verdict, OpFail> {
    let cal = m.cal.as_ref();
    match c {
        CK::IsOpen(r) => {
            let x = side(cal, r, vals, m, dates)?;
            let open = cal_of(cal)?.is_open(x).map_err(|o| OpFail::Outside(o.0))?;
            Ok(Verdict { holds: open, sides: None })
        }
        CK::Compare(a, cmp, b) => {
            let x = side(cal, a, vals, m, dates)?;
            let y = side(cal, b, vals, m, dates)?;
            Ok(Verdict { holds: cmp.holds(x, y), sides: Some((x, y)) })
        }
        CK::Monotonic(_) => Ok(Verdict { holds: true, sides: None }),
    }
}

/// How far a failing comparison is from holding, in days (larger is farther), and how much
/// room a holding one has (smaller is tighter).
pub fn margin(cmp: Cmp, l: Day, r: Day) -> i64 {
    let diff = (l.0 - r.0) as i64;
    match cmp {
        Cmp::Le | Cmp::Lt => diff,
        Cmp::Ge | Cmp::Gt => -diff,
        Cmp::Eq => diff.abs(),
    }
}

// ── Tracing ─────────────────────────────────────────────────────────────────

/// A computation with its steps, for `eval` and the examples of diagnostics.
#[derive(Clone, Debug)]
pub struct Traced {
    pub steps: Vec<Step>,
    pub dates: Vec<Option<Day>>,
    pub times: Vec<Option<Time>>,
    pub stop: Option<Stop>,
}

/// `2026-02-30`, for a day the month does not have.
pub fn ymd(y: i32, m: u32, d: u32) -> String {
    format!("{y:04}-{m:02}-{d:02}")
}

fn reasons_text(rs: &[Reason]) -> Text {
    let parts: Vec<Text> = rs.iter().map(|r| r.text()).collect();
    Text::join(&parts, "と", " and ")
}

/// `2026-05-10 は日曜、2026-05-09 は土曜で休み`: the closed days a move went over, in the
/// order it went over them, with runs of the same reason together.
pub fn closed_days_text(cal: &Calendar, days: &[Day]) -> Text {
    let mut groups: Vec<(Day, Day, Text)> = Vec::new();
    for d in days {
        let r = reasons_text(&cal.reasons(*d));
        match groups.last_mut() {
            Some((a, b, t)) if *t == r && ((b.0 - d.0).abs() == 1 || (a.0 - d.0).abs() == 1) => {
                if d < a {
                    *a = *d;
                } else if d > b {
                    *b = *d;
                }
            }
            _ => groups.push((*d, *d, r)),
        }
    }
    let ja: Vec<String> = groups
        .iter()
        .map(|(a, b, t)| if a == b { format!("{a} は{}", t.ja) } else { format!("{a}〜{b} は{}", t.ja) })
        .collect();
    let en: Vec<String> = groups
        .iter()
        .map(|(a, b, t)| if a == b { format!("{a} ({})", t.en) } else { format!("{a}..{b} ({})", t.en) })
        .collect();
    let total: i64 = groups.iter().map(|(a, b, _)| (b.0 - a.0) as i64 + 1).sum();
    let en = if en.len() == 1 {
        format!("{} {} closed", en[0], if total == 1 { "is" } else { "are" })
    } else {
        format!("{} and {} are closed", en[..en.len() - 1].join(", "), en[en.len() - 1])
    };
    Text { ja: format!("{}で休み", ja.join("、")), en }
}

/// The closed days between `from` and `to` that a move from one to the other went over.
fn skipped(cal: &Calendar, from: Day, to: Day, include_from: bool) -> Vec<Day> {
    let mut out = Vec::new();
    let step = if to > from { 1 } else { -1 };
    let mut d = if include_from { from } else { Day(from.0 + step) };
    while d != to {
        if cal.is_open(d) == Ok(false) {
            out.push(d);
        }
        d = Day(d.0 + step);
    }
    out
}

/// Why a step moved the day or left it, from what the operation did.
fn note_for(m: &Model, op: &ROp, from: Day, to: Day, evs: &[Ev], vals: &[i64]) -> Option<Text> {
    let cal = m.cal.as_ref();
    let mut notes: Vec<Text> = Vec::new();
    for e in evs {
        if let Ev::Else { y, m: mm, d, how } = e {
            let day = ymd(*y, *mm, *d);
            notes.push(match how {
                Missing::EndOfMonth => tr!("{day} は無いので、その月の末日", "{day} does not exist; the end of that month"),
                Missing::StartOfNextMonth => tr!("{day} は無いので、次の月の 1 日", "{day} does not exist; the first of the next month"),
                _ => tr!("{day} は無い", "{day} does not exist"),
            });
        }
    }
    match op {
        ROp::Roll(c) => {
            let cal = cal?;
            if from == to {
                notes.push(tr!("営業日なので動かない", "a business day, stays"));
            } else {
                let modified_back = matches!(c, Conv::ModifiedFollowing | Conv::ModifiedPreceding) && {
                    let natural = if *c == Conv::ModifiedFollowing { Conv::Following } else { Conv::Preceding };
                    cal.roll(from, natural).map(|n| !same_month(n, from)).unwrap_or(false)
                };
                if modified_back {
                    let natural = if *c == Conv::ModifiedFollowing { Conv::Following } else { Conv::Preceding };
                    let n = cal.roll(from, natural).ok()?;
                    let days = skipped(cal, from, to, true);
                    let t = closed_days_text(cal, &days);
                    notes.push(match c {
                        Conv::ModifiedFollowing => tr!(
                            "{}。翌営業日 {n} は月が変わるので、前営業日",
                            "{}; the next business day, {n}, is in the next month, so the business day before",
                            t.ja;
                            t.en
                        ),
                        _ => tr!(
                            "{}。前営業日 {n} は月が変わるので、翌営業日",
                            "{}; the business day before, {n}, is in the month before, so the next business day",
                            t.ja;
                            t.en
                        ),
                    });
                } else {
                    notes.push(closed_days_text(cal, &skipped(cal, from, to, true)));
                }
            }
        }
        ROp::Business(_, n) => {
            let cal = cal?;
            if from != to {
                // `+ 0 business days` rolls, so the day itself is one it goes over; otherwise
                // counting starts the day after.
                let days = skipped(cal, from, to, value(n, vals) == 0);
                if !days.is_empty() {
                    let t = closed_days_text(cal, &days);
                    notes.push(tr!("数えない休み: {}", "not counted: {}", t.ja.trim_end_matches("で休み"); t.en));
                }
            }
        }
        ROp::IfClosed(_) => {
            let cal = cal?;
            let closed = evs.iter().any(|e| matches!(e, Ev::IfClosed { closed: true }));
            if closed {
                notes.push(closed_days_text(cal, &[from]));
            } else {
                notes.push(tr!("営業日なので何もしない", "a business day, nothing to do"));
            }
        }
        _ => {}
    }
    if notes.is_empty() {
        None
    } else {
        Some(Text::join(&notes, "。", "; "))
    }
}

/// What `eval` adds to a closing day: the period it closes.
fn detail_for(op: &ROp, vals: &[i64], to: Day) -> Option<Text> {
    let start = match op {
        ROp::CloseDay(n, missing) => date::period_start(to, value(n, vals) as u32, missing.unwrap_or(Missing::Never)),
        ROp::CloseEndOfMonth => {
            let (y, m, _) = to.ymd();
            Day::from_ymd(y as i64, m, 1)?
        }
        _ => return None,
    };
    Some(tr!("{start}〜{to} の期間の締め日", "closes the period {start}..{to}"))
}

/// Why an operation gave no day, as a step's note.
pub fn fail_text(f: &OpFail) -> Text {
    match f {
        OpFail::Missing { y, m, d } => {
            let day = ymd(*y, *m, *d);
            tr!("{day} は無い日で、`else reject` なので断る", "{day} does not exist, and the line says `else reject`")
        }
        OpFail::Outside(d) => tr!("{d} が営業日かは、表の外なので分からない", "whether {d} is a business day is outside what the calendar knows"),
        OpFail::OutOfRange => tr!("0001-01-01〜9999-12-31 の外に出る", "it goes outside 0001-01-01..9999-12-31"),
        OpFail::Bug(s) => tr!("koyomi の不具合: {s}", "a bug in koyomi: {s}"),
    }
}

/// The dates to compute for `want`, each after the one it starts from.
pub fn chain_of(m: &Model, want: &[usize]) -> Vec<usize> {
    let mut need = vec![false; m.dates.len()];
    for w in want {
        for k in &m.dates[*w].chain {
            need[*k] = true;
        }
    }
    m.order.iter().copied().filter(|k| need[*k]).collect()
}

/// Compute `want` (with the dates they start from) on one input, a step a line. `upto` stops
/// after that date's operation, for the example of E201.
pub fn trace(m: &Model, vals: &[i64], want: &[usize], with_detail: bool) -> Traced {
    let cal = m.cal.as_ref();
    let mut steps = Vec::new();
    let di = m.date_in();
    steps.push(Step::Value {
        line: di.span.line,
        name: di.name.clone(),
        day: Some(Day(vals[m.date_input] as i32)),
        label: Text::default(),
        note: None,
        detail: None,
    });
    let ints: Vec<String> = m.int_inputs().iter().map(|k| format!("{} = {}", m.inputs[*k].name, vals[*k])).collect();
    if !ints.is_empty() {
        steps.push(Step::Say { line: 0, text: Text::new(ints.join("、"), ints.join(", ")) });
    }
    let mut dates: Vec<Option<Day>> = vec![None; m.dates.len()];
    let mut times: Vec<Option<Time>> = vec![None; m.dates.len()];
    for k in chain_of(m, want) {
        let d: &D = &m.dates[k];
        let mut x = match d.start {
            Start::Input => Day(vals[m.date_input] as i32),
            Start::Date(s) => dates[s].expect("computed before"),
        };
        if d.ops.is_empty() {
            steps.push(Step::Value {
                line: d.span.line,
                name: d.name.clone(),
                day: Some(x),
                label: tr!("{}と同じ", "same as {}", d.start_name; d.start_name),
                note: None,
                detail: None,
            });
        }
        for (i, o) in d.ops.iter().enumerate() {
            let mut evs = Vec::new();
            let label = paraphrase::label(m, &o.op, &o.text, vals);
            let name = if i == 0 { d.name.clone() } else { String::new() };
            match apply(cal, x, &o.op, vals, &mut evs) {
                Ok(r) => {
                    let note = note_for(m, &o.op, x, r, &evs, vals);
                    let detail = if with_detail { detail_for(&o.op, vals, r) } else { None };
                    steps.push(Step::Value { line: o.span.line, name, day: Some(r), label, note, detail });
                    x = r;
                }
                Err(fail) => {
                    steps.push(Step::Value { line: o.span.line, name, day: None, label, note: Some(fail_text(&fail)), detail: None });
                    return Traced { steps, dates, times, stop: Some(Stop { date: k, op: Some(i), fail }) };
                }
            }
        }
        if let (Some((at, s)), Some(off)) = (d.at, cal.and_then(|c| c.offset)) {
            match time(x, at, off) {
                Ok(t) => {
                    steps.push(Step::Say {
                        line: s.line,
                        text: tr!("時刻 {}（UTC で {}）", "time {} ({} in UTC)", t.local, t.utc; t.local, t.utc),
                    });
                    times[k] = Some(t);
                }
                Err(fail) => {
                    steps.push(Step::Say { line: s.line, text: fail_text(&fail) });
                    dates[k] = Some(x);
                    return Traced { steps, dates, times, stop: Some(Stop { date: k, op: None, fail }) };
                }
            }
        }
        dates[k] = Some(x);
    }
    Traced { steps, dates, times, stop: None }
}

/// The day an operation is applied to, for the input given: every operation before it.
pub fn before_op(m: &Model, vals: &[i64], date: usize, op: usize) -> Result<Day, Stop> {
    let cal = m.cal.as_ref();
    let mut out = vec![Day(0); m.dates.len()];
    let mut buf = Vec::new();
    for &k in &chain_of(m, &[date]) {
        let d = &m.dates[k];
        let mut x = match d.start {
            Start::Input => Day(vals[m.date_input] as i32),
            Start::Date(s) => out[s],
        };
        let last = if k == date { op } else { d.ops.len() };
        for (i, o) in d.ops.iter().enumerate().take(last) {
            buf.clear();
            x = apply(cal, x, &o.op, vals, &mut buf).map_err(|fail| Stop { date: k, op: Some(i), fail })?;
        }
        if k == date {
            return Ok(x);
        }
        out[k] = x;
    }
    Err(Stop { date, op: Some(op), fail: OpFail::Bug("the date is not in its own chain".into()) })
}
