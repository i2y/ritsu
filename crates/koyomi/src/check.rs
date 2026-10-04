//! `koyomi check` (DESIGN 3): five stages, each run only when the one before it found no
//! error.
//!
//! 1. Words and lines (E001–E006).
//! 2. Names and types, with the calendar a file uses (E007–E015, E304).
//! 3. The calendar and the sources (E101–E111, W101, W102).
//! 4. What is decided by what is written (E201, W201, W202).
//! 5. Every input of the range, one by one: the computations (E202–E204), then the claims
//!    and the examples (E301–E303), within a budget (E305).

use crate::ast::{Cmp, File, Kind, Span, Ty};
use crate::calendar::{Calendar, Loader};
use crate::date::{Day, Missing};
use crate::diag::{DiagExt, Diag, Run, Step};
use ritsu_base::text::{Text, count};
use crate::interp::{self, Ev, OpFail, Stop};
use crate::resolve::{self, A, CK, Model, R, ROp, Ref};
use crate::sources::{self, Origin};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The default budget, in input combinations (DESIGN 3.2). On an Apple M-series laptop a
/// check walks 13,587,300 combinations of a closing, a day of a month and a roll with three
/// claims in 0.92 s, and with a claim that counts 60 business days in 3.5 s: a full budget is
/// 7 to 26 seconds of checking.
pub const DEFAULT_BUDGET: u64 = 100_000_000;

/// The runs kept for one diagnostic. Past it they are counted, not kept: a claim that fails
/// on every other day of 10⁸ combinations would otherwise hold 5×10⁷ runs.
pub const MAX_RUNS: usize = 1_000_000;

#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub budget: u64,
}

impl Default for Options {
    fn default() -> Options {
        Options { budget: DEFAULT_BUDGET }
    }
}

/// What the walk over every input found, for the report, the tests, and (stage D) the page.
#[derive(Clone, Debug, Default)]
pub struct Report {
    pub combinations: u64,
    pub claims: Vec<ClaimReport>,
    pub dates: Vec<DateReport>,
    /// By date, by operation.
    pub ops: Vec<Vec<OpReport>>,
    pub examples: usize,
}

#[derive(Clone, Debug, Default)]
pub struct ClaimReport {
    /// Inputs checked (pairs of adjacent days, for `is monotonic`).
    pub checked: u64,
    pub fails: u64,
    pub runs: Vec<(Vec<i64>, Day, Day)>,
    /// Runs past [`MAX_RUNS`], counted.
    pub more_runs: u64,
    /// The first input it fails on.
    pub first: Option<Vec<i64>>,
    /// The failing input farthest from holding: (margin, input, left side, right side).
    pub farthest: Option<(i64, Vec<i64>, Day, Day)>,
    /// The holding input closest to failing.
    pub tightest: Option<(i64, Vec<i64>, Day, Day)>,
}

#[derive(Clone, Debug, Default)]
pub struct DateReport {
    /// The most days from the input's date to this date, and every input that gets there.
    pub longest: Option<(i64, Vec<Vec<i64>>)>,
    pub shortest: Option<(i64, Vec<Vec<i64>>)>,
    pub earliest: Option<Day>,
    pub latest: Option<Day>,
}

#[derive(Clone, Debug, Default)]
pub struct OpReport {
    /// Inputs on which the calendar moved the day.
    pub moved: u64,
    /// The distinct moves, in the order first seen (at most 64).
    pub moves: Vec<(Day, Day)>,
    /// Inputs on which `else` was used, and the first.
    pub elses: u64,
    pub else_first: Option<Vec<i64>>,
    /// Inputs on which `if closed` found the day closed.
    pub if_closed: u64,
}

/// What a check made of a file.
pub enum Checked {
    Calendar(Box<Calendar>),
    Dates(Box<Model>, Report),
}

pub struct Outcome {
    pub path: String,
    pub diags: Vec<Diag>,
    pub checked: Option<Checked>,
    /// What was checked, when nothing failed: the line after `ok —`.
    pub ok: Option<Text>,
}

impl Outcome {
    pub fn has_errors(&self) -> bool {
        self.diags.iter().any(|d| d.is_error())
    }
}

/// The first line of a `.cal`, for choosing what to do with it before parsing.
fn read(path: &str) -> Result<Vec<u8>, String> {
    ritsu_base::fs::read(path).map_err(|e| e.to_string())
}

/// Check a file on disk. `Err` is a file that cannot be read at all (exit 2).
pub fn check_file(path: &str, opts: &Options, loader: &mut Loader) -> Result<Outcome, String> {
    let bytes = read(path)?;
    Ok(check_bytes(path, &bytes, opts, loader))
}

pub fn check_bytes(path: &str, bytes: &[u8], opts: &Options, loader: &mut Loader) -> Outcome {
    let Ok(src) = std::str::from_utf8(bytes) else {
        let d = Diag::error("E001", path, 0, 0, tr!("ファイルが UTF-8 ではありません", "The file is not UTF-8"));
        return Outcome { path: path.into(), diags: vec![d], checked: None, ok: None };
    };
    let parsed = crate::parse::parse(path, src);
    let mut diags = parsed.diags;
    let Some(f) = parsed.file else {
        return Outcome { path: path.into(), diags, checked: None, ok: None };
    };
    match f.kind {
        Kind::Calendar => {
            let p = PathBuf::from(path);
            let (cal, mut ds) = loader.build(&f, &p, bytes);
            diags.append(&mut ds);
            let ok = cal.as_ref().filter(|_| !diags.iter().any(|d| d.is_error())).map(calendar_summary);
            Outcome { path: path.into(), diags, checked: cal.map(|c| Checked::Calendar(Box::new(c))), ok }
        }
        Kind::Dates => match prepare_file(f, path, bytes, loader, opts) {
            Err(mut ds) => {
                diags.append(&mut ds);
                Outcome { path: path.into(), diags, checked: None, ok: None }
            }
            Ok((m, mut ds)) => {
                diags.append(&mut ds);
                let (mut ds, report) = walk_and_report(&m, opts);
                diags.append(&mut ds);
                let ok = if diags.iter().any(|d| d.is_error()) { None } else { Some(dates_summary(&m, &report)) };
                Outcome { path: path.into(), diags, checked: Some(Checked::Dates(Box::new(m), report)), ok }
            }
        },
    }
}

/// Stages 1 to 4 of a dates file: what `eval` and `api` need before they compute. The
/// warnings come back beside the model.
pub fn prepare(path: &str, loader: &mut Loader) -> Result<(Model, Vec<Diag>), Result<Vec<Diag>, String>> {
    let bytes = read(path).map_err(Err)?;
    let src = std::str::from_utf8(&bytes).map_err(|_| Ok(vec![Diag::error("E001", path, 0, 0, tr!("ファイルが UTF-8 ではありません", "The file is not UTF-8"))]))?;
    let parsed = crate::parse::parse(path, src);
    let Some(f) = parsed.file else { return Err(Ok(parsed.diags)) };
    if f.kind != Kind::Dates {
        return Err(Ok(vec![]));
    }
    prepare_file(f, path, &bytes, loader, &Options::default()).map_err(Ok)
}

/// Stages 2 to 4 of a parsed dates file.
fn prepare_file(f: File, path: &str, bytes: &[u8], loader: &mut Loader, opts: &Options) -> Result<(Model, Vec<Diag>), Vec<Diag>> {
    let p = PathBuf::from(path);
    let dir = p.parent().unwrap_or(Path::new("")).to_path_buf();
    let mut diags = Vec::new();
    let use_cal = f.use_calendar.clone();
    let model = resolve::resolve(f, p.clone(), ritsu_base::sha256::hex(bytes));
    let mut m = match model {
        Ok(m) => m,
        Err(mut ds) => {
            // Read the calendar too, so its own errors are said in the same run.
            if let Some((cp, sp)) = &use_cal {
                let parsed = crate::parse::parse(path, std::str::from_utf8(bytes).unwrap_or(""));
                if let Some(ff) = parsed.file
                    && let Err(d) = loader.use_calendar(&ff, &dir, cp, *sp).map(|(_, mut x)| {
                        ds.append(&mut x);
                    })
                {
                    ds.push(*d);
                }
            }
            return Err(ds);
        }
    };
    // Stage 2: the calendar it uses (E015, and the calendar's own diagnostics).
    if let Some((cp, sp)) = &use_cal {
        match loader.use_calendar(&m.file, &dir, cp, *sp) {
            Ok((cal, mut ds)) => {
                diags.append(&mut ds);
                m.cal = cal;
            }
            Err(d) => diags.push(*d),
        }
    }
    if diags.iter().any(|d| d.is_error()) {
        return Err(diags);
    }
    // Stage 3: the file's own laws, and what needs the calendar.
    let origin = Origin { file: &m.file, dir: &dir };
    let (laws, mut ds) = sources::check_laws(&origin);
    diags.append(&mut ds);
    m.laws = laws;
    diags.extend(needs_calendar(&m));
    if diags.iter().any(|d| d.is_error()) {
        return Err(diags);
    }
    // Stage 4.
    diags.extend(statics(&m, opts));
    if diags.iter().any(|d| d.is_error()) {
        return Err(diags);
    }
    Ok((m, diags))
}

/// E109 (an operation or a claim asks a calendar the file does not use) and E110 (`at`
/// without an offset).
fn needs_calendar(m: &Model) -> Vec<Diag> {
    let f = &m.file;
    let mut out = Vec::new();
    let err = |code: &'static str, s: Span, msg: Text| Diag::error(code, &f.path, s.line, s.col, msg).source(&f.src);
    if m.cal.is_none() && m.needs_calendar() {
        let first_op = m.dates.iter().flat_map(|d| d.ops.iter()).find(|o| o.op.needs_calendar()).map(|o| o.span);
        let first_claim = m
            .claims
            .iter()
            .find(|c| match &c.kind {
                CK::IsOpen(_) => true,
                CK::Compare(a, _, b) => [a, b].iter().any(|r| matches!(r.offset, Some((_, _, true)))),
                CK::Monotonic(_) => false,
            })
            .map(|c| c.span);
        let s = first_op.or(first_claim).unwrap_or_default();
        out.push(err("E109", s, tr!("この行は休みかどうかを調べますが、カレンダーがありません（`use calendar` がありません）", "This line asks which days are closed, and there is no calendar (no `use calendar`)")).note(tr!(
            "見出しのあとに `use calendar \"calendars/東京の営業日.cal\"` のように、読むカレンダーを書いてください。",
            "Name the calendar after the heading, like `use calendar \"calendars/tokyo.cal\"`."
        )));
    }
    for d in &m.dates {
        if let Some((_, s)) = d.at
            && m.cal.as_ref().and_then(|c| c.offset).is_none()
        {
            let msg = match &m.cal {
                Some(c) => tr!(
                    "`at` で時刻を出すには、カレンダー {} にオフセットが要ります",
                    "`at` gives a time, and the calendar {} has no offset",
                    c.info.name;
                    c.info.name
                ),
                None => tr!("`at` で時刻を出すには、オフセットのあるカレンダーが要ります", "`at` gives a time, which needs a calendar with an offset"),
            };
            out.push(err("E110", s, msg).note(tr!(
                "カレンダーに `offset +09:00` のように書いてください。夏時間のある地域なら、時刻は出さず日付だけにしてください。",
                "Write the calendar's offset, like `offset +09:00`. For a place with daylight saving time, give dates only."
            )));
        }
    }
    out
}

// ── Stage 4 ─────────────────────────────────────────────────────────────────

/// Whether an operation can land on a day the month does not have, from what is written
/// (DESIGN 1.7): adding months always can; the N-th of a month and closing on the N-th only
/// when N can be 29 or more.
fn can_miss(m: &Model, op: &ROp) -> bool {
    let n_can = |a: &A| match a {
        A::Lit(n) => *n >= 29,
        A::Input(k) => m.inputs[*k].hi >= 29,
    };
    match op {
        ROp::Months { .. } => true,
        ROp::DayOfMonth { n, .. } | ROp::CloseDay(n, _) => n_can(n),
        _ => false,
    }
}

fn written_missing(op: &ROp) -> Option<Option<Missing>> {
    match op {
        ROp::Months { missing, .. } | ROp::DayOfMonth { missing, .. } | ROp::CloseDay(_, missing) => Some(*missing),
        _ => None,
    }
}

/// Every input of the range, in the order the check walks them, as many as the budget allows.
pub struct Inputs<'a> {
    m: &'a Model,
    ints: Vec<usize>,
    vals: Vec<i64>,
    started: bool,
    done: bool,
}

impl<'a> Inputs<'a> {
    pub fn new(m: &'a Model) -> Inputs<'a> {
        let vals: Vec<i64> = m.inputs.iter().map(|i| i.lo).collect();
        let done = m.inputs.iter().any(|i| i.lo > i.hi);
        Inputs { m, ints: m.int_inputs(), vals, started: false, done }
    }

    pub fn next_input(&mut self) -> Option<&[i64]> {
        if self.done {
            return None;
        }
        if !self.started {
            self.started = true;
            return Some(&self.vals);
        }
        let di = self.m.date_input;
        if self.vals[di] < self.m.inputs[di].hi {
            self.vals[di] += 1;
            return Some(&self.vals);
        }
        self.vals[di] = self.m.inputs[di].lo;
        // The integer inputs: the last declared turns fastest.
        for &k in self.ints.iter().rev() {
            if self.vals[k] < self.m.inputs[k].hi {
                self.vals[k] += 1;
                return Some(&self.vals);
            }
            self.vals[k] = self.m.inputs[k].lo;
        }
        self.done = true;
        None
    }
}

/// What the line `else` gives for one input, under each of the three ways.
fn under(m: &Model, op: &ROp, d: Day, vals: &[i64], p: Missing) -> Option<Day> {
    let op2 = match op {
        ROp::Months { sign, n, per, .. } => ROp::Months { sign: *sign, n: *n, per: *per, missing: Some(p) },
        ROp::DayOfMonth { n, sign, k, .. } => ROp::DayOfMonth { n: *n, sign: *sign, k: *k, missing: Some(p) },
        ROp::CloseDay(n, _) => ROp::CloseDay(*n, Some(p)),
        _ => return None,
    };
    interp::apply(m.cal.as_ref(), d, &op2, vals, &mut Vec::new()).ok()
}

/// The missing day an operation lands on, for one input.
fn lands_on(m: &Model, op: &ROp, d: Day, vals: &[i64]) -> Option<(i32, u32, u32)> {
    let mut ev = Vec::new();
    let _ = under(m, op, d, vals, Missing::EndOfMonth);
    let op2 = match op {
        ROp::Months { sign, n, per, .. } => ROp::Months { sign: *sign, n: *n, per: *per, missing: Some(Missing::EndOfMonth) },
        ROp::DayOfMonth { n, sign, k, .. } => ROp::DayOfMonth { n: *n, sign: *sign, k: *k, missing: Some(Missing::EndOfMonth) },
        ROp::CloseDay(n, _) => ROp::CloseDay(*n, Some(Missing::EndOfMonth)),
        _ => return None,
    };
    interp::apply(m.cal.as_ref(), d, &op2, vals, &mut ev).ok()?;
    ev.iter().find_map(|e| match e {
        Ev::Else { y, m, d, .. } => Some((*y, *m, *d)),
        _ => None,
    })
}

/// The inputs as `name value` pairs: `受領日 2026-01-29`, `月数 3`.
fn inputs_text(m: &Model, vals: &[i64]) -> Text {
    let parts: Vec<String> = m.inputs.iter().enumerate().map(|(k, i)| format!("{} {}", i.name, i.show(vals[k]))).collect();
    Text::new(parts.join("、"), parts.join(", "))
}

/// The inputs as `koyomi eval` takes them.
fn eval_inputs(m: &Model, vals: &[i64]) -> Vec<(String, String)> {
    m.inputs.iter().enumerate().map(|(k, i)| (i.name.clone(), i.show(vals[k]))).collect()
}

/// E201, W201 and W202.
fn statics(m: &Model, opts: &Options) -> Vec<Diag> {
    let f = &m.file;
    let mut out = Vec::new();
    for (di, d) in m.dates.iter().enumerate() {
        for (oi, o) in d.ops.iter().enumerate() {
            let inner = match &o.op {
                ROp::IfClosed(x) => x.as_ref(),
                x => x,
            };
            if let Some(stray) = o.stray_else {
                out.push(
                    Diag::warning("W201", &f.path, stray.line, stray.col, tr!(
                        "`{}` は無い日に当たらないので、`else` は効きません",
                        "`{}` never lands on a day the month does not have, so its `else` does nothing",
                        o.text;
                        o.text
                    ))
                    .source(&f.src)
                    .note(tr!("`else …` を消してください。", "Delete the `else …`.")),
                );
            }
            let Some(written) = written_missing(inner) else { continue };
            let can = can_miss(m, inner);
            match (written, can) {
                (Some(_), false) => {
                    let s = o.else_span.unwrap_or(o.span);
                    out.push(
                        Diag::warning("W201", &f.path, s.line, s.col, tr!(
                            "`{}` の日は 28 日までなので無い日に当たらず、`else` は効きません",
                            "`{}` takes a day of 28 or less, which every month has, so its `else` does nothing",
                            o.text;
                            o.text
                        ))
                        .source(&f.src)
                        .note(tr!("`else …` を消してください。", "Delete the `else …`.")),
                    );
                }
                (None, true) => out.push(e201(m, di, oi, inner, opts)),
                _ => {}
            }
            // W202: a longer way to say what a shorter line says.
            let shorter = match inner {
                ROp::DayOfMonth { n: A::Lit(31), sign, k, missing: Some(Missing::EndOfMonth) } => Some(format!("end of month{}", away(m, *sign, k))),
                ROp::DayOfMonth { n: A::Lit(1), sign, k, missing: None } => Some(format!("start of month{}", away(m, *sign, k))),
                ROp::CloseDay(A::Lit(31), Some(Missing::EndOfMonth)) => Some("close end of month".into()),
                _ => None,
            };
            if let Some(s) = shorter {
                let fix = if matches!(o.op, ROp::IfClosed(_)) { format!("if closed {s}") } else { s.clone() };
                let indent = " ".repeat(o.span.col - 1);
                out.push(
                    Diag::warning("W202", &f.path, o.span.line, o.span.col, tr!(
                        "`{}` は `{s}` と同じです。短いほうで書いてください",
                        "`{}` means the same as `{s}`; write the shorter one",
                        o.text;
                        o.text
                    ))
                    .source(&f.src)
                    .note(tr!(
                        "同じ意味の書き方が二つあると、grep と diff の両方で困ります。",
                        "Two ways to write one thing get in the way of both grep and diff."
                    ))
                    .fix_in_notes(format!("{indent}{fix}")),
                );
            }
        }
    }
    out
}

/// ` +1`, ` -2`, ` +支払の月`, or nothing for `+0`.
fn away(m: &Model, sign: i64, k: &A) -> String {
    match k {
        A::Lit(0) => String::new(),
        A::Lit(n) => format!(" {}{n}", if sign < 0 { '-' } else { '+' }),
        A::Input(i) => format!(" {}{}", if sign < 0 { '-' } else { '+' }, m.inputs[*i].name),
    }
}

/// E201, with the first input of the range that lands on a missing day (DESIGN 4.3).
fn e201(m: &Model, di: usize, oi: usize, inner: &ROp, opts: &Options) -> Diag {
    let f = &m.file;
    let o = &m.dates[di].ops[oi];
    let text = &o.text;
    let mut d = Diag::error("E201", &f.path, o.span.line, o.span.col, tr!(
        "`{text}` はその月に無い日に当たることがありますが、そのときどうするかが書かれていません",
        "`{text}` can land on a day the month does not have, and the line does not say what to do then"
    ))
    .source(&f.src);
    let mut found = None;
    let mut it = Inputs::new(m);
    let mut n = 0u64;
    while let Some(vals) = it.next_input() {
        n += 1;
        if n > opts.budget {
            break;
        }
        let Ok(day) = interp::before_op(m, vals, di, oi) else { continue };
        let day = if matches!(o.op, ROp::IfClosed(_)) {
            // `if closed` does the operation only on a closed day.
            match m.cal.as_ref().map(|c| c.is_open(day)) {
                Some(Ok(false)) => day,
                _ => continue,
            }
        } else {
            day
        };
        if let Some(hit) = lands_on(m, inner, day, vals) {
            found = Some((vals.to_vec(), day, hit));
            break;
        }
    }
    let base = o.text.trim().to_string();
    let base = base.strip_prefix("if closed ").unwrap_or(&base).to_string();
    let indent = " ".repeat(o.span.col - 1);
    let prefix = if matches!(o.op, ROp::IfClosed(_)) { "if closed " } else { "" };
    match found {
        Some((vals, day, (y, mm, dd))) => {
            let eom = under(m, inner, day, &vals, Missing::EndOfMonth).map(|x| x.to_string()).unwrap_or_default();
            let sonm = under(m, inner, day, &vals, Missing::StartOfNextMonth).map(|x| x.to_string()).unwrap_or_default();
            let missing = interp::ymd(y, mm, dd);
            let who = inputs_text(m, &vals);
            d = d
                .note(tr!("{} なら {missing} になります", "for {} it would be {missing}", who.ja; who.en))
                .note(tr!(
                    "直し方: 次のどれかを書いてください。`{base} else end_of_month`（{eom} にする）、`{base} else start_of_next_month`（{sonm} にする）、`{base} else reject`（範囲の中で起きないことを検査が確かめる）",
                    "To fix it, write one of: `{base} else end_of_month` (giving {eom}), `{base} else start_of_next_month` (giving {sonm}), `{base} else reject` (the check makes sure it never happens in the range)"
                ));
            let mut steps = interp::trace(m, &vals, &[di], false).steps;
            // Show the computation up to the line that needs `else`.
            if let Some(pos) = steps.iter().position(|s| s.line() == o.span.line) {
                steps.truncate(pos);
            }
            steps.push(Step::Value {
                line: o.span.line,
                name: if oi == 0 { m.dates[di].name.clone() } else { String::new() },
                day: None,
                label: crate::paraphrase::label(m, &o.op, &o.text, &vals),
                note: Some(tr!("{missing} は無い", "{missing} does not exist")),
                detail: None,
            });
            d = d.example(tr!("そうなる例", "the input that gets there"), steps, eval_inputs(m, &vals));
        }
        None => {
            d = d.note(tr!(
                "範囲の中では起きませんが、`else reject` と書けば、起きないことを検査が確かめます",
                "It does not happen in the range, but write `else reject` and the check makes sure it never does"
            ));
        }
    }
    d.fix_in_notes(format!("{indent}{prefix}{base} else end_of_month"))
}

// ── Stage 5 ─────────────────────────────────────────────────────────────────

/// A computation that stopped, with every input it stopped on.
struct Stopped {
    first: Vec<i64>,
    stop: Stop,
    /// The claim that asked, when it was a claim.
    claim: Option<usize>,
    count: u64,
    runs: Vec<(Vec<i64>, Day, Day)>,
    more_runs: u64,
}

/// Add one input to the runs: the last run grows when it is the day after, with the same
/// integers; past [`MAX_RUNS`] a new run is only counted.
fn push_run(runs: &mut Vec<(Vec<i64>, Day, Day)>, more: &mut u64, params: &[i64], z: Day) {
    if let Some(last) = runs.last_mut()
        && last.0 == params
        && last.2.0 + 1 == z.0
    {
        last.2 = z;
        return;
    }
    if runs.len() >= MAX_RUNS {
        // The last kept run cannot grow again, so a new one starts with each gap.
        *more += 1;
        return;
    }
    runs.push((params.to_vec(), z, z));
}

/// The walk over every input, and the diagnostics it gives.
pub fn walk_and_report(m: &Model, opts: &Options) -> (Vec<Diag>, Report) {
    let combos = m.combinations();
    let mut rep = Report {
        combinations: combos.min(u64::MAX as u128) as u64,
        claims: vec![ClaimReport::default(); m.claims.len()],
        dates: vec![DateReport::default(); m.dates.len()],
        ops: m.dates.iter().map(|d| vec![OpReport::default(); d.ops.len()]).collect(),
        examples: m.examples.len(),
    };
    if combos > opts.budget as u128 {
        return (vec![e305(m, combos, opts.budget)], rep);
    }
    let ints = m.int_inputs();
    let di = m.date_input;
    let mut out = vec![Day(0); m.dates.len()];
    let mut prev = vec![Day(0); m.dates.len()];
    let mut have_prev = false;
    let mut evs: Vec<(u32, u32, Ev)> = Vec::new();
    let mut stopped: BTreeMap<(u8, usize, usize), Stopped> = BTreeMap::new();
    let mut params: Vec<i64> = Vec::new();
    let mut it = Inputs::new(m);
    let mut last_params: Option<Vec<i64>> = None;
    while let Some(vals) = it.next_input() {
        params.clear();
        params.extend(ints.iter().map(|k| vals[*k]));
        if last_params.as_deref() != Some(&params[..]) {
            have_prev = false;
            last_params = Some(params.clone());
        }
        let z = Day(vals[di] as i32);
        evs.clear();
        if let Err(stop) = interp::run(m, vals, &mut out, &mut evs) {
            let key = match stop.fail {
                OpFail::Missing { .. } => (2u8, stop.date, stop.op.unwrap_or(usize::MAX)),
                OpFail::Outside(_) => (3, 0, 0),
                OpFail::OutOfRange | OpFail::Bug(_) => (4, stop.date, stop.op.unwrap_or(usize::MAX)),
            };
            let e = stopped.entry(key).or_insert_with(|| Stopped { first: vals.to_vec(), stop: stop.clone(), claim: None, count: 0, runs: vec![], more_runs: 0 });
            e.count += 1;
            push_run(&mut e.runs, &mut e.more_runs, &params, z);
            have_prev = false;
            continue;
        }
        for (k, dr) in rep.dates.iter_mut().enumerate() {
            let x = out[k];
            let dist = (x.0 - z.0) as i64;
            match &mut dr.longest {
                Some((best, who)) if dist == *best => {
                    if who.len() < 16 {
                        who.push(vals.to_vec());
                    }
                }
                Some((best, _)) if dist < *best => {}
                _ => dr.longest = Some((dist, vec![vals.to_vec()])),
            }
            match &mut dr.shortest {
                Some((best, who)) if dist == *best => {
                    if who.len() < 16 {
                        who.push(vals.to_vec());
                    }
                }
                Some((best, _)) if dist > *best => {}
                _ => dr.shortest = Some((dist, vec![vals.to_vec()])),
            }
            dr.earliest = Some(dr.earliest.map_or(x, |e| e.min(x)));
            dr.latest = Some(dr.latest.map_or(x, |e| e.max(x)));
        }
        for (dk, ok, e) in &evs {
            let r = &mut rep.ops[*dk as usize][*ok as usize];
            match e {
                Ev::Moved { from, to } => {
                    r.moved += 1;
                    if r.moves.len() < 64 && !r.moves.contains(&(*from, *to)) {
                        r.moves.push((*from, *to));
                    }
                }
                Ev::Else { .. } => {
                    r.elses += 1;
                    if r.else_first.is_none() {
                        r.else_first = Some(vals.to_vec());
                    }
                }
                Ev::IfClosed { closed: true } => r.if_closed += 1,
                Ev::IfClosed { closed: false } => {}
            }
        }
        // `if closed` reports a move inside its own count too; one input moves once.
        for (ci, c) in m.claims.iter().enumerate() {
            let cr = &mut rep.claims[ci];
            match &c.kind {
                CK::Monotonic(k) => {
                    if have_prev {
                        cr.checked += 1;
                        if out[*k] < prev[*k] {
                            cr.fails += 1;
                            if cr.first.is_none() {
                                cr.first = Some(vals.to_vec());
                            }
                            push_run(&mut cr.runs, &mut cr.more_runs, &params, z);
                        }
                    }
                }
                kind => match interp::claim(m, kind, vals, &out) {
                    Ok(v) => {
                        cr.checked += 1;
                        if let (CK::Compare(_, cmp, _), Some((l, r))) = (kind, v.sides) {
                            let mg = interp::margin(*cmp, l, r);
                            let slot = if v.holds { &mut cr.tightest } else { &mut cr.farthest };
                            if slot.as_ref().is_none_or(|(best, ..)| mg > *best) {
                                *slot = Some((mg, vals.to_vec(), l, r));
                            }
                        }
                        if !v.holds {
                            cr.fails += 1;
                            if cr.first.is_none() {
                                cr.first = Some(vals.to_vec());
                            }
                            push_run(&mut cr.runs, &mut cr.more_runs, &params, z);
                        }
                    }
                    Err(fail) => {
                        // A claim asks the calendar (E203), or its offset leaves the dates there
                        // are (E204); it never meets a missing day.
                        let key = match fail {
                            OpFail::Outside(_) => (3u8, 0, 0),
                            _ => (4u8, usize::MAX, ci),
                        };
                        let stop = Stop { date: usize::MAX, op: None, fail };
                        let e = stopped.entry(key).or_insert_with(|| Stopped { first: vals.to_vec(), stop, claim: Some(ci), count: 0, runs: vec![], more_runs: 0 });
                        e.count += 1;
                        push_run(&mut e.runs, &mut e.more_runs, &params, z);
                        break;
                    }
                },
            }
        }
        std::mem::swap(&mut prev, &mut out);
        have_prev = true;
    }
    let mut diags = Vec::new();
    if !stopped.is_empty() {
        for ((code, _, _), s) in &stopped {
            diags.push(match code {
                2 => e202(m, s),
                3 => e203(m, s),
                _ => e204(m, s),
            });
        }
        return (diags, rep);
    }
    for (ci, c) in m.claims.iter().enumerate() {
        let cr = &rep.claims[ci];
        if cr.fails > 0 {
            diags.push(match c.kind {
                CK::Monotonic(k) => e302(m, ci, k, cr),
                _ => e301(m, ci, cr),
            });
        }
    }
    diags.extend(examples(m));
    (diags, rep)
}

/// `受領日 669 日` or `起点と月数の 4,380 通り`: what the inputs come to.
fn of_inputs(m: &Model, n: u64) -> Text {
    let n = count(n);
    if m.inputs.len() == 1 {
        let name = &m.inputs[0].name;
        tr!("{name} {n} 日", "{n} days of {name}")
    } else {
        let names: Vec<Text> = m.inputs.iter().map(|i| Text::same(i.name.clone())).collect();
        let l = Text::list(&names);
        let ja = l.ja.replace('、', "と");
        tr!("{ja}の {n} 通り", "{n} combinations of {}", ; l.en)
    }
}

fn params_text(m: &Model, ps: &[i64]) -> Text {
    let ints = m.int_inputs();
    let parts: Vec<String> = ints.iter().zip(ps).map(|(k, v)| format!("{} {v}", m.inputs[*k].name)).collect();
    Text::new(parts.join("、"), parts.join(", "))
}

/// `2026-01-01〜2026-01-29（29 日）、…ほか 4 か所`: the first six runs, the ones with the same
/// integer inputs together under them. A lone run goes without its count, which the
/// sentence before it has already said.
fn runs_text(m: &Model, runs: &[(Vec<i64>, Day, Day)], more_runs: u64) -> Text {
    let has_params = !m.int_inputs().is_empty();
    let lone = runs.len() == 1;
    let span = |a: &Day, b: &Day| -> Text {
        let n = (b.0 - a.0) as i64 + 1;
        if a == b {
            Text::same(a.to_string())
        } else if lone {
            tr!("{a}〜{b}", "{a}..{b}")
        } else {
            tr!("{a}〜{b}（{n} 日）", "{a}..{b} ({n} days)")
        }
    };
    let mut ja = String::new();
    let mut en = String::new();
    let mut last: Option<&Vec<i64>> = None;
    for (i, (ps, a, b)) in runs.iter().take(6).enumerate() {
        let t = span(a, b);
        if has_params && last != Some(ps) {
            let p = params_text(m, ps);
            if i > 0 {
                ja.push('、');
                en.push_str("; ");
            }
            ja.push_str(&format!("{} の {}", p.ja, t.ja));
            en.push_str(&format!("{}: {}", p.en, t.en));
        } else {
            if i > 0 {
                ja.push('、');
                en.push_str(", ");
            }
            ja.push_str(&t.ja);
            en.push_str(&t.en);
        }
        last = Some(ps);
    }
    let more = runs.len().saturating_sub(6) as u64 + more_runs;
    if more_runs > 0 {
        let kept = count(runs.len() as u64);
        let more = count(more);
        Text {
            ja: format!("{ja}、ほか {more} か所。--format json は最初の {kept} か所を出します"),
            en: format!("{en}, and {more} more runs; --format json lists the first {kept}"),
        }
    } else if more > 0 {
        Text { ja: format!("{ja}、ほか {more} か所。全部は --format json で出ます"), en: format!("{en}, and {more} more runs; --format json lists them all") }
    } else {
        Text { ja, en }
    }
}

/// The number of failures by the integer inputs' values, when there are any.
fn by_params(m: &Model, runs: &[(Vec<i64>, Day, Day)]) -> Option<Text> {
    let ints = m.int_inputs();
    if ints.is_empty() {
        return None;
    }
    let mut counts: Vec<(Vec<i64>, i64)> = Vec::new();
    for (ps, a, b) in runs {
        let n = (b.0 - a.0) as i64 + 1;
        match counts.last_mut() {
            Some((p, c)) if p == ps => *c += n,
            _ => counts.push((ps.clone(), n)),
        }
    }
    let shown: Vec<Text> = counts
        .iter()
        .take(12)
        .map(|(ps, n)| {
            let p = params_text(m, ps);
            tr!("{} で {n}", "{} → {n}", p.ja; p.en)
        })
        .collect();
    let more = counts.len().saturating_sub(12);
    let list = Text::join(&shown, "、", ", ");
    Some(if more > 0 {
        tr!("入力の組み合わせごとの数: {}、ほか {more} 組", "by input: {}, and {more} more", list.ja; list.en)
    } else {
        tr!("入力の組み合わせごとの数: {}", "by input: {}", list.ja; list.en)
    })
}

fn to_runs(m: &Model, runs: &[(Vec<i64>, Day, Day)]) -> Vec<Run> {
    let ints = m.int_inputs();
    runs.iter()
        .map(|(ps, a, b)| Run { from: *a, to: *b, params: ints.iter().zip(ps).map(|(k, v)| (m.inputs[*k].name.clone(), *v)).collect() })
        .collect()
}

/// How a side of a claim is written: `支払日`, `受領日 + 60 days`.
fn side_text(m: &Model, r: &R) -> String {
    match &r.offset {
        None => r.name.clone(),
        Some((fwd, n, business)) => {
            let n = match n {
                A::Lit(v) => v.to_string(),
                A::Input(k) => m.inputs[*k].name.clone(),
            };
            format!("{} {} {n} {}days", r.name, if *fwd { '+' } else { '-' }, if *business { "business " } else { "" })
        }
    }
}

/// `支払日は受領日の 89 日後で、条件は 60 日後まで`: what a comparison found on one input.
pub fn compare_text(m: &Model, a: &R, cmp: Cmp, b: &R, l: Day, r: Day, vals: &[i64]) -> Text {
    let (is, limit) = compare_parts(m, a, cmp, b, l, r, vals, false);
    match limit {
        Some(lim) => tr!("{}で、条件は {}", "{}, and the claim allows {}", is.ja, lim.ja; is.en, lim.en),
        None => is,
    }
}

/// What a comparison found, in two parts: how the two sides stand (`支払日は受領日の 89 日後`),
/// and what the claim allows (`60 日後まで`), when the claim is of that simple shape.
/// `dated` puts the left side's date in.
#[allow(clippy::too_many_arguments)]
pub fn compare_parts(m: &Model, a: &R, cmp: Cmp, b: &R, l: Day, r: Day, vals: &[i64], dated: bool) -> (Text, Option<Text>) {
    let simple = a.offset.is_none() && !matches!(b.offset, Some((_, _, true)));
    if !simple {
        let (x, y) = (side_text(m, a), side_text(m, b));
        let sym = cmp.symbol();
        return (
            tr!(
                "{x} は {l}、{y} は {r} で、{l} {sym} {r} ではない",
                "{x} is {l} and {y} is {r}, and {l} {sym} {r} does not hold"
            ),
            None,
        );
    }
    let bound = match &b.offset {
        Some((fwd, n, _)) => {
            let v = interp::value(n, vals);
            if *fwd { v } else { -v }
        }
        None => 0,
    };
    let base = r.0 as i64 - bound;
    let diff = l.0 as i64 - base;
    let days_ja = |k: i64| -> String {
        match k {
            0 => "同じ日".into(),
            k if k > 0 => format!("{k} 日後"),
            k => format!("{} 日前", -k),
        }
    };
    let days_en = |k: i64| -> String {
        match k {
            0 => "the same day".into(),
            1 => "1 day after".into(),
            -1 => "1 day before".into(),
            k if k > 0 => format!("{k} days after"),
            k => format!("{} days before", -k),
        }
    };
    let (an, bn) = (&a.name, &b.name);
    let (an_ja, an_en) = if dated { (format!("{an} {l} "), format!("{an} {l}")) } else { (an.clone(), an.clone()) };
    let is_ja = match diff {
        0 => format!("{an_ja}は{bn}と同じ日"),
        _ => format!("{an_ja}は{bn}の {}", days_ja(diff)),
    };
    let is_en = match diff {
        0 => format!("{an_en} is the same day as {bn}"),
        _ => format!("{an_en} is {} {bn}", days_en(diff)),
    };
    let limit = match (cmp, bound) {
        (Cmp::Eq, 0) => None,
        (Cmp::Le, _) => Some((format!("{}まで", days_ja(bound)), format!("at most {}", days_en(bound)))),
        (Cmp::Lt, _) => Some((format!("{}まで", days_ja(bound - 1)), format!("at most {}", days_en(bound - 1)))),
        (Cmp::Ge, _) => Some((format!("{}から", days_ja(bound)), format!("at least {}", days_en(bound)))),
        (Cmp::Gt, _) => Some((format!("{}から", days_ja(bound + 1)), format!("at least {}", days_en(bound + 1)))),
        (Cmp::Eq, _) => Some((format!("ちょうど {}", days_ja(bound)), format!("exactly {}", days_en(bound)))),
    };
    (Text { ja: is_ja, en: is_en }, limit.map(|(j, e)| Text { ja: j, en: e }))
}

/// The dates a claim looks at.
fn claim_dates(c: &CK) -> Vec<usize> {
    let mut out = Vec::new();
    let mut side = |r: &R| {
        if let Ref::Date(k) = r.what {
            out.push(k);
        }
    };
    match c {
        CK::IsOpen(r) => side(r),
        CK::Compare(a, _, b) => {
            side(a);
            side(b);
        }
        CK::Monotonic(k) => out.push(*k),
    }
    out
}

/// E301: a claim fails on some inputs.
fn e301(m: &Model, ci: usize, cr: &ClaimReport) -> Diag {
    let f = &m.file;
    let c = &m.claims[ci];
    let name = &c.name;
    let what = of_inputs(m, cr.checked);
    let fails = count(cr.fails);
    let unit_ja = if m.inputs.len() == 1 { "日" } else { "通り" };
    let mut d = Diag::error("E301", &f.path, c.span.line, c.span.col, tr!(
        "条件「{name}」が、{}のうち {fails} {unit_ja}で成り立ちません",
        "The claim {name} fails for {fails} of the {}",
        what.ja;
        what.en
    ))
    .source(&f.src);
    let runs = runs_text(m, &cr.runs, cr.more_runs);
    d = if m.inputs.len() == 1 {
        d.note(tr!("成り立たない日: {}", "It fails on {}", runs.ja; runs.en))
    } else {
        d.note(tr!("成り立たない入力: {}", "It fails on {}", runs.ja; runs.en))
    };
    if let Some(t) = by_params(m, &cr.runs) {
        d = d.note(t);
    }
    if let (CK::Compare(a, cmp, b), Some((_, vals, l, r))) = (&c.kind, &cr.farthest) {
        let who = inputs_text(m, vals);
        let (t, _) = compare_parts(m, a, *cmp, b, *l, *r, vals, true);
        d = d.note(tr!("いちばん外れるのは{} のときで、{}", "The farthest is {}, where {}", who.ja, t.ja; who.en, t.en));
    }
    if let Some(vals) = &cr.first {
        let tr_ = interp::trace(m, vals, &claim_dates(&c.kind), false);
        let mut steps = tr_.steps;
        let dates: Vec<Day> = tr_.dates.iter().map(|x| x.unwrap_or(Day(0))).collect();
        match &c.kind {
            CK::Compare(a, cmp, b) => {
                if let Ok(v) = interp::claim(m, &c.kind, vals, &dates)
                    && let Some((l, r)) = v.sides
                {
                    steps.push(Step::Say { line: c.span.line, text: compare_text(m, a, *cmp, b, l, r, vals) });
                }
            }
            CK::IsOpen(r) => {
                if let Ok(x) = interp::side(m.cal.as_ref(), r, vals, m, &dates)
                    && let Some(cal) = &m.cal
                {
                    let t = interp::closed_days_text(cal, &[x]);
                    let s = side_text(m, r);
                    steps.push(Step::Say { line: c.span.line, text: tr!("{s}の {}", "{s}: {}", t.ja; t.en) });
                }
            }
            CK::Monotonic(_) => {}
        }
        d = d.example(tr!("そうなる例（最初の入力）", "the first input it fails on"), steps, eval_inputs(m, vals));
    }
    d.fails(to_runs(m, &cr.runs))
}

/// E302: a date that comes earlier for a later input.
fn e302(m: &Model, ci: usize, k: usize, cr: &ClaimReport) -> Diag {
    let f = &m.file;
    let c = &m.claims[ci];
    let x = &m.dates[k].name;
    let din = &m.date_in().name;
    let (pairs, fails) = (count(cr.checked), count(cr.fails));
    let mut d = Diag::error("E302", &f.path, c.span.line, c.span.col, tr!(
        "{x}が単調ではありません。{din}の隣り合う {pairs} 組のうち {fails} 組で、後の日のほうが早い{x}になります",
        "{x} is not monotonic: on {fails} of the {pairs} pairs of adjacent days of {din}, the later day gives an earlier {x}"
    ))
    .source(&f.src);
    let runs = runs_text(m, &cr.runs, cr.more_runs);
    d = d.note(tr!("前の日より早くなる後の日: {}", "The later days that come out earlier: {}", runs.ja; runs.en));
    if let Some(t) = by_params(m, &cr.runs) {
        d = d.note(t);
    }
    if let Some(vals) = &cr.first {
        let mut before = vals.clone();
        before[m.date_input] -= 1;
        let a = interp::trace(m, &before, &[k], false);
        let b = interp::trace(m, vals, &[k], false);
        let mut steps = a.steps;
        steps.extend(b.steps);
        if let (Some(p), Some(q)) = (a.dates[k], b.dates[k]) {
            let d0 = Day(before[m.date_input] as i32);
            let d1 = Day(vals[m.date_input] as i32);
            steps.push(Step::Say { line: c.span.line, text: tr!("{din}は {d0} から {d1} へ遅くなったのに、{x}は {p} から {q} へ早くなる", "{din} goes from {d0} to {d1}, later, and {x} from {p} to {q}, earlier") });
        }
        d = d.example(tr!("そうなる例（最初の組）", "the first pair it fails on"), steps, eval_inputs(m, vals));
    }
    d.fails(to_runs(m, &cr.runs))
}

/// E202: `else reject` meets a missing day.
fn e202(m: &Model, s: &Stopped) -> Diag {
    let f = &m.file;
    let o = &m.dates[s.stop.date].ops[s.stop.op.unwrap_or(0)];
    let text = &o.text;
    let n = count(s.count);
    let inputs = if s.count == 1 { "input" } else { "inputs" };
    let mut d = Diag::error("E202", &f.path, o.span.line, o.span.col, tr!(
        "`{text}` が、範囲の中の {n} 通りの入力で、その月に無い日に当たります",
        "`{text}` lands on a day the month does not have, for {n} {inputs} in the range"
    ))
    .source(&f.src);
    let runs = runs_text(m, &s.runs, s.more_runs);
    d = d.note(tr!("当たる入力: {}", "It does on {}", runs.ja; runs.en)).note(tr!(
        "`else reject` は、範囲の中で一度も当たらないことを確かめる書き方です。当たるなら、`else end_of_month` か `else start_of_next_month` で扱いを書くか、範囲を狭めてください。",
        "`else reject` says it never happens in the range. Since it does, say what to do with `else end_of_month` or `else start_of_next_month`, or narrow the range."
    ));
    let t = interp::trace(m, &s.first, &[s.stop.date], false);
    d.example(tr!("そうなる例（最初の入力）", "the first input it happens on"), t.steps, eval_inputs(m, &s.first)).fails(to_runs(m, &s.runs))
}

/// The table whose `covers` ends where the calendar's data does, on the side `d` falls.
fn bounding_table(cal: &Calendar, d: Day) -> Option<&sources::Table> {
    if d > cal.data.1 {
        cal.tables.iter().find(|t| t.covers.1 == cal.data.1)
    } else {
        cal.tables.iter().find(|t| t.covers.0 == cal.data.0)
    }
}

/// E203: a computation asks about a day the calendar does not know (DESIGN 2.4, 4.3).
fn e203(m: &Model, s: &Stopped) -> Diag {
    let f = &m.file;
    let di = m.date_in();
    let OpFail::Outside(asked) = s.stop.fail else { unreachable!() };
    let cal = m.cal.as_ref().expect("a calendar was asked");
    let z = Day(s.first[m.date_input] as i32);
    let who = inputs_text(m, &s.first);
    let (lo, hi) = cal.data;
    let table = bounding_table(cal, asked);
    let knows = match table {
        Some(t) => tr!("祝日の表「{}」が知っているのは {lo}〜{hi} です", "the table {} knows only {lo}..{hi}", t.name; t.name),
        None => tr!("カレンダーが知っているのは {lo}〜{hi} です", "the calendar knows only {lo}..{hi}"),
    };
    let mut d = Diag::error("E203", &f.path, di.span.line, di.span.col, tr!(
        "{} の計算が、{asked} が営業日かを問いますが、{}",
        "The computation for {} asks whether {asked} is a business day, and {}",
        who.ja,
        knows.ja;
        who.en,
        knows.en
    ))
    .source(&f.src);
    let n = count(s.count);
    let runs = runs_text(m, &s.runs, s.more_runs);
    d = if m.inputs.len() == 1 {
        d.note(tr!("同じことが起きる{}は {n} 日（{}）", "The same happens for {n} days of {}: {}", di.name, runs.ja; di.name, runs.en))
    } else {
        d.note(tr!("同じことが起きる入力の組み合わせは {n} 通り（{}）", "The same happens for {n} input combinations: {}", runs.ja; runs.en))
    };
    // The fix: the range without the dates it happens on, when they are at one end or both.
    let (rlo, rhi) = (di.lo, di.hi);
    let mut bad: Vec<(i64, i64)> = s.runs.iter().map(|(_, a, b)| (a.0 as i64, b.0 as i64)).collect();
    bad.sort();
    let mut merged: Vec<(i64, i64)> = Vec::new();
    for (a, b) in bad {
        match merged.last_mut() {
            Some(l) if a <= l.1 + 1 => l.1 = l.1.max(b),
            _ => merged.push((a, b)),
        }
    }
    let mut new_lo = rlo;
    let mut new_hi = rhi;
    let mut ok = true;
    for (a, b) in &merged {
        if *a == new_lo {
            new_lo = b + 1;
        } else if *b == rhi {
            new_hi = new_hi.min(a - 1);
        } else {
            ok = false;
        }
    }
    let fetch = tr!(
        "または、新しい表が出てから写しを取り直してください（koyomi source fetch）",
        "or take the copy again once a newer table is out (koyomi source fetch)"
    );
    if ok && new_lo <= new_hi {
        let range = format!("range >={} <={}", Day(new_lo as i32), Day(new_hi as i32));
        d = d.note(tr!("直し方: 範囲を `{range}` にしてください。{}", "To fix it: make the range `{range}`, {}", fetch.ja; fetch.en));
        let line = f.src.lines().nth(di.span.line - 1).unwrap_or("");
        if let Some(pos) = line.find("range") {
            d = d.fix_in_notes(format!("{}{range}", &line[..pos]));
        }
    } else {
        d = d.note(tr!("直し方: 範囲を、表の分かっている日に収まるよう狭めてください。{}", "To fix it: narrow the range to what the table knows, {}", fetch.ja; fetch.en));
    }
    let mut steps = match s.claim {
        None => interp::trace(m, &s.first, &[s.stop.date], false).steps,
        Some(ci) => {
            let c = &m.claims[ci];
            let mut st = interp::trace(m, &s.first, &claim_dates(&c.kind), false).steps;
            st.push(Step::Say { line: c.span.line, text: tr!("条件「{}」: {}", "claim {}: {}", c.name, interp::fail_text(&s.stop.fail).ja; c.name, interp::fail_text(&s.stop.fail).en) });
            st
        }
    };
    if steps.is_empty() {
        steps.push(Step::Say { line: 0, text: Text::same(z.to_string()) });
    }
    d.example(tr!("そうなる例（最初の入力）", "the first input it happens on"), steps, eval_inputs(m, &s.first)).fails(to_runs(m, &s.runs))
}

/// E204: a date outside 0001-01-01..9999-12-31.
fn e204(m: &Model, s: &Stopped) -> Diag {
    let f = &m.file;
    let (span, text) = match (s.claim, s.stop.op) {
        (Some(ci), _) => (m.claims[ci].span, m.claims[ci].text.clone()),
        (None, Some(i)) => (m.dates[s.stop.date].ops[i].span, m.dates[s.stop.date].ops[i].text.clone()),
        (None, None) => {
            let date = &m.dates[s.stop.date];
            (date.at.map(|a| a.1).unwrap_or(date.span), "at".to_string())
        }
    };
    let n = count(s.count);
    let inputs = if s.count == 1 { "input" } else { "inputs" };
    let mut d = Diag::error("E204", &f.path, span.line, span.col, tr!(
        "`{text}` の結果が、{n} 通りの入力で 0001-01-01〜9999-12-31 の外に出ます",
        "`{text}` goes outside 0001-01-01..9999-12-31 for {n} {inputs}"
    ))
    .source(&f.src);
    if let OpFail::Bug(b) = &s.stop.fail {
        d = d.note(tr!("koyomi の不具合です: {b}", "This is a bug in koyomi: {b}"));
    }
    let runs = runs_text(m, &s.runs, s.more_runs);
    d = d.note(tr!("外に出る入力: {}", "It does on {}", runs.ja; runs.en));
    let steps = match s.claim {
        Some(ci) => {
            let c = &m.claims[ci];
            let mut st = interp::trace(m, &s.first, &claim_dates(&c.kind), false).steps;
            let f = interp::fail_text(&s.stop.fail);
            st.push(Step::Say { line: c.span.line, text: tr!("条件「{}」: {}", "claim {}: {}", c.name, f.ja; c.name, f.en) });
            st
        }
        None => interp::trace(m, &s.first, &[s.stop.date], false).steps,
    };
    d.example(tr!("そうなる例（最初の入力）", "the first input it happens on"), steps, eval_inputs(m, &s.first)).fails(to_runs(m, &s.runs))
}

/// E305: more combinations than the budget.
fn e305(m: &Model, combos: u128, budget: u64) -> Diag {
    let f = &m.file;
    let di = m.date_in();
    let parts: Vec<String> = m
        .inputs
        .iter()
        .map(|i| match i.ty {
            Ty::Date => format!("{} {} days", i.name, count(i.size())),
            Ty::Int => format!("{} {}", i.name, count(i.size())),
        })
        .collect();
    let parts_ja: Vec<String> = m
        .inputs
        .iter()
        .map(|i| match i.ty {
            Ty::Date => format!("{} {} 日", i.name, count(i.size())),
            Ty::Int => format!("{} {}", i.name, count(i.size())),
        })
        .collect();
    let c = count(combos.min(u64::MAX as u128) as u64);
    let b = count(budget);
    Diag::error("E305", &f.path, di.span.line, di.span.col, tr!(
        "検査には入力の組み合わせ {c} 通りの計算が要り、予算の {b} を超えるので、何も確かめていません",
        "Checking needs {c} input combinations, over the budget of {b}, so nothing was checked"
    ))
    .source(&f.src)
    .note(tr!("内訳: {}", "That is {}", parts_ja.join(" × "); parts.join(" × ")))
    .note(tr!(
        "koyomi は、一部の入力だけを試して通すことはしません。範囲を狭めるか、ファイルを分けるか、`--budget` で予算を上げてください。",
        "It never tries some of the inputs and passes; narrow a range, split the file, or raise the budget with `--budget`."
    ))
}

/// E303: an example row that the computation does not give.
fn examples(m: &Model) -> Vec<Diag> {
    let f = &m.file;
    let mut out = Vec::new();
    for (ri, ex) in m.examples.iter().enumerate() {
        let row = ri + 1;
        let mut vals: Vec<i64> = m.inputs.iter().map(|i| i.lo).collect();
        let mut outside = None;
        for (k, v) in &ex.inputs {
            vals[*k] = *v;
            let i = &m.inputs[*k];
            if *v < i.lo || *v > i.hi {
                outside = Some(*k);
            }
        }
        if let Some(k) = outside {
            let i = &m.inputs[k];
            let (v, lo, hi) = (i.show(vals[k]), i.show(i.lo), i.show(i.hi));
            out.push(
                Diag::error("E303", &f.path, ex.span.line, ex.span.col, tr!(
                    "例の {row} 行目の {} {v} は範囲 {lo}〜{hi} の外です",
                    "Example row {row}: {} {v} is outside its range {lo}..{hi}",
                    i.name;
                    i.name
                ))
                .source(&f.src),
            );
            continue;
        }
        let t = interp::trace(m, &vals, &(0..m.dates.len()).collect::<Vec<_>>(), false);
        let line = f.src.lines().nth(ex.span.line - 1).unwrap_or("").to_string();
        let mut fixed: Vec<char> = line.chars().collect();
        let mut wrong = Vec::new();
        for (k, want, s) in &ex.outputs {
            let got = t.dates[*k];
            if got != Some(*want) {
                wrong.push((*k, *want, got, *s));
                if let Some(g) = got {
                    let gs: Vec<char> = g.to_string().chars().collect();
                    let at = s.col - 1;
                    if at + 10 <= fixed.len() {
                        fixed.splice(at..at + 10, gs);
                    }
                }
            }
        }
        if let Some((k, want, got, s)) = wrong.first() {
            let name = &m.dates[*k].name;
            let got_s = got.map(|g| g.to_string()).unwrap_or_else(|| "-".into());
            let mut d = Diag::error("E303", &f.path, s.line, s.col, tr!(
                "例の {row} 行目の{name}は {want} と書かれていますが、計算すると {got_s} です",
                "Example row {row} says {name} is {want}, and it computes to {got_s}"
            ))
            .source(&f.src);
            if wrong.len() > 1 {
                let others: Vec<String> = wrong[1..].iter().map(|(k, w, g, _)| format!("{} {} → {}", m.dates[*k].name, w, g.map(|x| x.to_string()).unwrap_or_default())).collect();
                d = d.note(tr!("同じ行でほかにも違います: {}", "The row differs on more: {}", others.join("、"); others.join(", ")));
            }
            d = d.example(tr!("計算", "the computation"), t.steps, eval_inputs(m, &vals));
            out.push(d.fix_line(fixed.iter().collect::<String>()));
        }
    }
    out
}

// ── What a passing check says ─────────────────────────────────────────────

fn dates_summary(m: &Model, r: &Report) -> Text {
    let n = m.claims.len();
    let range = |i: &resolve::In| -> (String, String) { (format!("{}〜{}", i.show(i.lo), i.show(i.hi)), format!("{}..{}", i.show(i.lo), i.show(i.hi))) };
    let what = if m.inputs.len() == 1 {
        let i = &m.inputs[0];
        let (rj, re) = range(i);
        let c = count(r.combinations);
        tr!("{} {rj} の {c} 日", "all {c} days of {} ({re})", i.name; i.name)
    } else {
        let ja: Vec<String> = m.inputs.iter().map(|i| format!("{} {}", i.name, range(i).0)).collect();
        let en: Vec<String> = m.inputs.iter().map(|i| format!("{} ({})", i.name, range(i).1)).collect();
        let c = count(r.combinations);
        let en_list = Text::list(&en.iter().map(|s| Text::same(s.clone())).collect::<Vec<_>>()).en;
        Text { ja: format!("{} の {c} 通り", ja.join("、")), en: format!("all {c} combinations of {en_list}") }
    };
    let mut t = if n == 0 {
        tr!("{}のすべてで、どの日付も計算できます", "every date computes on {}", what.ja; what.en)
    } else {
        let claims = if n == 1 { "1 claim holds".to_string() } else { format!("{n} claims hold") };
        tr!("{n} つの条件が、{}のすべてで成り立ちます", "{claims} on {}", what.ja; what.en)
    };
    if r.examples > 0 {
        let e = r.examples;
        t = t.then(&tr!("。例 {e} 行も合っています", "; {e} examples match"));
    }
    t
}

fn calendar_summary(c: &Calendar) -> Text {
    let (lo, hi) = c.data;
    let tables: Vec<String> = c.tables.iter().map(|t| format!("{} {}", t.name, count(t.rows.len() as u64))).collect();
    if tables.is_empty() {
        tr!("カレンダー {}。表を使わないので、データの範囲は {lo}〜{hi} です", "calendar {}: no table, so the data range is {lo}..{hi}", c.info.name; c.info.name)
    } else {
        let tj = tables.iter().map(|t| format!("{t} 行")).collect::<Vec<_>>().join("、");
        let te = tables.iter().map(|t| format!("{t} rows")).collect::<Vec<_>>().join(", ");
        tr!(
            "カレンダー {}。表 {tj}、データの範囲 {lo}〜{hi}",
            "calendar {}: table {te}, data range {lo}..{hi}",
            c.info.name;
            c.info.name
        )
    }
}

/// What a person reads for one file: the diagnostics, then `ok — …` when there were no errors.
pub fn render(o: &Outcome, lang: ritsu_base::text::Lang) -> String {
    let mut s: String = o.diags.iter().map(|d| d.render(lang)).collect();
    if let Some(ok) = &o.ok {
        s.push_str(&format!("{}: ok — {}\n", o.path, ok.get(lang)));
    }
    s
}

/// The `--format json` of one file.
pub fn to_json(o: &Outcome, lang: ritsu_base::text::Lang) -> serde_json::Value {
    serde_json::json!({
        "file": o.path,
        "ok": !o.has_errors(),
        "summary": o.ok.as_ref().map(|t| t.get(lang).to_string()),
        "diagnostics": o.diags.iter().map(|d| crate::diag::value(&d.to_json(lang))).collect::<Vec<_>>(),
    })
}

/// Shorthand for a test or a tool: check one file with a fresh loader.
pub fn check(path: &str) -> Result<Outcome, String> {
    check_file(path, &Options::default(), &mut Loader::default())
}

/// Same, from text (the path names where `use calendar` and the copies are looked for).
pub fn check_text(path: &str, src: &str, opts: &Options) -> Outcome {
    check_bytes(path, src.as_bytes(), opts, &mut Loader::default())
}
