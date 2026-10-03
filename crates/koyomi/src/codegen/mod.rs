//! Generating code (DESIGN 6): one dependency-free file for each target, and a runner beside
//! it that reads `koyomi vectors` on standard input and prints what the generated functions
//! give, one line an input, for the tests that hold the code to the reference interpreter
//! (DESIGN 6.4).
//!
//! This module turns a checked file into [`Unit`]: what every target writes, already decided
//! — the header, the calendar's data, each date's function as the operations of its chain in
//! order, and the helpers those operations need. The five targets (`typescript.rs`,
//! `python.rs`, `go.rs`, `rust.rs`, `sql.rs`) only spell it. The helpers are the procedures of
//! `date.rs` and `calendar.rs`, written out once per target on day numbers; no target calls
//! its language's date library for the arithmetic (DESIGN 1.7).

pub mod go;
pub mod python;
pub mod rust;
pub mod sql;
pub mod typescript;

use crate::api::relative;
use crate::ast::{At, Conv, Kind, Ty};
use crate::calendar::{Calendar, Info};
use crate::date::{Day, Missing};
use ritsu_base::text::{Lang, Text};
use crate::naming::Target;
use crate::resolve::{A, Model, ROp};
use crate::sources::{Law, Table};
use std::collections::BTreeSet;
use std::path::Path;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A number an operation takes: written out, or an integer input (by index).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Num {
    Lit(i64),
    /// `factor × input`: `- 月数 months` is −1 × 月数, `+ n years` 12 × n.
    Input(i64, usize),
}

impl Num {
    fn of(a: &A, factor: i64) -> Num {
        match a {
            A::Lit(n) => Num::Lit(factor * n),
            A::Input(k) => Num::Input(factor, *k),
        }
    }
}

/// What to do with a day the month does not have, as the helpers take it: the `else` written,
/// or `none` where the check worked out from what is written that the operation never lands
/// on one (a day of 28 or less, the first or the last of a month).
pub fn mode(m: Option<Missing>) -> &'static str {
    match m {
        Some(Missing::EndOfMonth) => "end_of_month",
        Some(Missing::StartOfNextMonth) => "start_of_next_month",
        Some(Missing::Reject) => "reject",
        Some(Missing::Never) | None => "none",
    }
}

pub fn conv(c: Conv) -> &'static str {
    match c {
        Conv::Following => "following",
        Conv::Preceding => "preceding",
        Conv::ModifiedFollowing => "modified_following",
        Conv::ModifiedPreceding => "modified_preceding",
    }
}

/// One operation, as a call of a helper on the day `z`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Call {
    /// `_add_days(z, n)`.
    AddDays(Num),
    /// `_add_business(z, n, forward)`.
    AddBusiness(Num, bool),
    /// `_add_months(z, k, mode)`.
    AddMonths(Num, &'static str),
    /// `_day_of_month(z, n, k, mode)`.
    DayOfMonth(Num, Num, &'static str),
    /// `_start_of_month(z, k)`.
    StartOfMonth(Num),
    /// `_end_of_month(z, k)`.
    EndOfMonth(Num),
    /// `_close_day(z, n, mode)`.
    CloseDay(Num, &'static str),
    /// `_close_end_of_month(z)`.
    CloseEndOfMonth,
    /// `_roll(z, convention)`.
    Roll(&'static str),
    /// `if closed <call>`.
    IfClosed(Box<Call>),
}

impl Call {
    fn of(op: &ROp) -> Call {
        match op {
            ROp::Days(sign, n) => Call::AddDays(Num::of(n, *sign)),
            ROp::Business(fwd, n) => Call::AddBusiness(Num::of(n, 1), *fwd),
            ROp::Months { sign, n, per, missing } => Call::AddMonths(Num::of(n, sign * per), mode(*missing)),
            ROp::DayOfMonth { n, sign, k, missing } => Call::DayOfMonth(Num::of(n, 1), Num::of(k, *sign), mode(*missing)),
            ROp::StartOfMonth(sign, k) => Call::StartOfMonth(Num::of(k, *sign)),
            ROp::EndOfMonth(sign, k) => Call::EndOfMonth(Num::of(k, *sign)),
            ROp::CloseDay(n, missing) => Call::CloseDay(Num::of(n, 1), mode(*missing)),
            ROp::CloseEndOfMonth => Call::CloseEndOfMonth,
            ROp::Roll(c) => Call::Roll(conv(*c)),
            ROp::IfClosed(inner) => Call::IfClosed(Box::new(Call::of(inner))),
        }
    }

    /// The helpers this call uses directly.
    fn helpers(&self, out: &mut BTreeSet<H>) {
        match self {
            Call::AddDays(_) => {
                out.insert(H::AddDays);
            }
            Call::AddBusiness(..) => {
                out.insert(H::AddBusiness);
            }
            Call::AddMonths(..) => {
                out.insert(H::AddMonths);
            }
            Call::DayOfMonth(..) => {
                out.insert(H::DayOfMonth);
            }
            Call::StartOfMonth(_) => {
                out.insert(H::StartOfMonth);
            }
            Call::EndOfMonth(_) => {
                out.insert(H::EndOfMonth);
            }
            Call::CloseDay(..) => {
                out.insert(H::CloseDay);
            }
            Call::CloseEndOfMonth => {
                out.insert(H::CloseEndOfMonth);
            }
            Call::Roll(_) => {
                out.insert(H::Roll);
            }
            Call::IfClosed(inner) => {
                out.insert(H::IsOpen);
                inner.helpers(out);
            }
        }
    }
}

/// The helpers a generated file may hold, in the order they are written. Each is the
/// procedure of the function of `date.rs` or `calendar.rs` with the same name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum H {
    ShiftMonth,
    Place,
    AddDays,
    AddMonths,
    DayOfMonth,
    StartOfMonth,
    EndOfMonth,
    CloseDay,
    CloseEndOfMonth,
    Weekday,
    IsOpen,
    Seek,
    Roll,
    AddBusiness,
    At,
}

impl H {
    /// The helpers this one calls (besides the conversions every file has).
    fn needs(self, cal: Option<&CalData>) -> Vec<H> {
        match self {
            H::Place => vec![H::ShiftMonth],
            H::AddMonths | H::DayOfMonth | H::StartOfMonth | H::CloseDay => vec![H::ShiftMonth, H::Place],
            H::EndOfMonth => vec![H::ShiftMonth],
            H::IsOpen => match cal {
                Some(c) if c.weekly.iter().any(|w| *w) => vec![H::Weekday],
                _ => vec![],
            },
            H::Seek => vec![H::IsOpen, H::AddDays],
            H::Roll | H::AddBusiness => vec![H::Seek, H::IsOpen, H::AddDays],
            _ => vec![],
        }
    }
}

/// The calendar a generated file embeds (DESIGN 6.1).
#[derive(Clone, Debug)]
pub struct CalData {
    pub name: String,
    /// Closed days of the week, Monday first.
    pub weekly: [bool; 7],
    /// `closed every`: month × 100 + day, both ends included; `from > to` wraps the year end.
    pub every: Vec<(u32, u32, String)>,
    /// `closed <day>..<day>`, as day numbers, with what the line says.
    pub days: Vec<(i32, i32, String)>,
    /// `open <day>..<day>`.
    pub opens: Vec<(i32, i32, String)>,
    /// The days of the tables inside the data range, ascending, each with `YYYY-MM-DD 名前`.
    pub holidays: Vec<(i32, String)>,
    /// The days the calendar knows.
    pub data: (i32, i32),
    /// Minutes east of UTC.
    pub offset: Option<i32>,
}

impl CalData {
    pub fn of(c: &Calendar) -> CalData {
        let named = |n: &Option<String>, text: String| match n {
            Some(n) => format!("{text} {n}"),
            None => text,
        };
        let mut holidays: Vec<(i32, String)> = Vec::new();
        let mut seen = BTreeSet::new();
        for t in &c.tables {
            for r in &t.rows {
                if r.day >= c.data.0 && r.day <= c.data.1 && seen.insert(r.day) {
                    let label = if r.name.is_empty() { r.day.to_string() } else { format!("{} {}", r.day, r.name) };
                    holidays.push((r.day.0, label));
                }
            }
        }
        holidays.sort_by_key(|h| h.0);
        CalData {
            name: c.info.name.clone(),
            weekly: c.weekly,
            every: c.every.iter().map(|e| (e.from.0 * 100 + e.from.1, e.to.0 * 100 + e.to.1, named(&e.name, format!("every {}", e.text())))).collect(),
            days: c.days.iter().map(|s| (s.from.0, s.to.0, named(&s.name, s.text()))).collect(),
            opens: c.opens.iter().map(|s| (s.from.0, s.to.0, named(&s.name, s.text()))).collect(),
            holidays,
            data: (c.data.0.0, c.data.1.0),
            offset: c.offset,
        }
    }
}

/// An input a function takes, with the guard at its entrance.
#[derive(Clone, Debug)]
pub struct InputG {
    pub name: String,
    pub alias: String,
    pub ty: Ty,
    pub lo: i64,
    pub hi: i64,
}

impl InputG {
    /// `range >=2026-01-01 <=2027-11-20`, for the comment on the guard.
    pub fn range_text(&self) -> String {
        format!("range >={} <={}", self.show(self.lo), self.show(self.hi))
    }

    pub fn show(&self, v: i64) -> String {
        match self.ty {
            Ty::Date => Day(v as i32).to_string(),
            Ty::Int => v.to_string(),
        }
    }
}

/// One operation of a function, with the line of the `.cal` it comes from.
#[derive(Clone, Debug)]
pub struct StepG {
    pub call: Call,
    /// The operation as the `.cal` writes it, and, on the first operation of a date the
    /// function computes on the way, that date's name.
    pub comment: String,
}

/// A date's function.
#[derive(Clone, Debug)]
pub struct DateG {
    pub name: String,
    pub alias: String,
    /// The inputs it takes, by index, in the order they are declared.
    pub params: Vec<usize>,
    pub steps: Vec<StepG>,
    /// `at`: minutes after midnight (1440 for the end of the day), and the line.
    pub at: Option<(u32, String)>,
    /// `@民法 第141条, 第143条` and the like, from the date's line and its operations.
    pub cites: Vec<String>,
}

/// Everything a target writes for one `.cal`.
pub struct Unit {
    pub lang: Lang,
    pub kind: Kind,
    /// The file's name and its alias (the module, package or schema).
    pub name: String,
    pub alias: String,
    pub description: Option<String>,
    /// The header's lines, without the comment marker.
    pub header: Vec<String>,
    pub cal: Option<CalData>,
    pub inputs: Vec<InputG>,
    pub dates: Vec<DateG>,
    /// The helpers the operations need, with what those call.
    pub helpers: BTreeSet<H>,
}

impl Unit {
    pub fn has(&self, h: H) -> bool {
        self.helpers.contains(&h)
    }

    /// The text in the language the code is generated in.
    pub fn t(&self, t: Text) -> String {
        t.get(self.lang).to_string()
    }

    /// Whether any function takes an integer input.
    pub fn has_ints(&self) -> bool {
        self.dates.iter().any(|d| d.params.iter().any(|k| self.inputs[*k].ty == Ty::Int))
    }

    pub fn has_at(&self) -> bool {
        self.dates.iter().any(|d| d.at.is_some())
    }

    /// The values the runner prints for an input, in order: each date, and its time after
    /// it. The same order as `vectors::out_keys`.
    pub fn outputs(&self) -> Vec<(usize, bool)> {
        let mut v = Vec::new();
        for (k, d) in self.dates.iter().enumerate() {
            v.push((k, false));
            if d.at.is_some() {
                v.push((k, true));
            }
        }
        v
    }

    /// The input that is a date (a dates file has exactly one).
    pub fn date_input(&self) -> usize {
        self.inputs.iter().position(|i| i.ty == Ty::Date).unwrap_or(0)
    }
}

fn cite_text(c: &crate::ast::Cite) -> String {
    let frs: Vec<&str> = c.fragments.iter().map(|(f, _)| f.as_str()).collect();
    format!("@{} {}", c.source, frs.join(", "))
}

/// The header lines (DESIGN 6.1): koyomi's version, the `.cal`, each calendar file, and each
/// source, with the digests the file was generated from. Paths are from the `.cal`'s
/// directory; no path of the machine that ran `gen` is written.
/// The `.cal` a file is generated from, as the header names it.
struct Head<'a> {
    kind: Kind,
    path: &'a Path,
    name: &'a str,
    version: &'a str,
    sha: &'a str,
}

fn header(lang: Lang, h0: &Head, cal: Option<&Calendar>, laws: &[Law]) -> Vec<String> {
    let Head { kind, path, name, version, sha } = *h0;
    let base = crate::calendar::clean(path.parent().unwrap_or(Path::new("")));
    let file = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
    let word = if kind == Kind::Dates { crate::kw::DATES } else { crate::kw::CALENDAR };
    let mut h = vec![ritsu_emit::header::generated(&format!("by koyomi {VERSION}"))];
    let short = |s: &str| s[..16.min(s.len())].to_string();
    h.push(Text::new(format!("もと: {file}（{word} {name} v{version}、sha256:{}）", short(sha)), format!("Source: {file} ({word} {name} v{version}, sha256:{})", short(sha))).get(lang).to_string());
    if let Some(c) = cal {
        let mut infos: Vec<&Info> = Vec::new();
        if kind == Kind::Dates {
            infos.push(&c.info);
        }
        infos.extend(c.used.iter());
        for i in infos {
            let p = relative(&base, &i.path);
            h.push(
                Text::new(
                    format!("カレンダー: {p}（calendar {} v{}、sha256:{}）", i.name, i.version, short(&i.sha256)),
                    format!("Calendar: {p} (calendar {} v{}, sha256:{})", i.name, i.version, short(&i.sha256)),
                )
                .get(lang)
                .to_string(),
            );
        }
        for t in &c.tables {
            h.push(table_cite(lang, t, &base));
        }
        for l in &c.laws {
            h.push(law_cite(lang, l));
        }
    }
    for l in laws {
        h.push(law_cite(lang, l));
    }
    h
}

fn table_cite(lang: Lang, t: &Table, base: &Path) -> String {
    let p = relative(base, &crate::calendar::clean(&t.path));
    let url = t.url.as_ref().map(|u| format!(" ({u})")).unwrap_or_default();
    let url_ja = t.url.as_ref().map(|u| format!("（{u}）")).unwrap_or_default();
    let covers = if t.listed_years {
        format!("covers listed years = {}..{}", t.covers.0, t.covers.1)
    } else {
        format!("covers {}..{}", t.covers.0, t.covers.1)
    };
    Text::new(
        format!("出典: {} = file {p} sha256:{}{url_ja}、{covers}", t.name, t.pin),
        format!("Cites: {} = file {p} sha256:{}{url}, {covers}", t.name, t.pin),
    )
    .get(lang)
    .to_string()
}

fn law_cite(lang: Lang, l: &Law) -> String {
    let pins: Vec<String> = l.pins.iter().map(|p| format!("{} sha256:{}", p.fragment, p.pin)).collect();
    let rev = l.revision.as_ref().map(|r| format!(", revision {r}")).unwrap_or_default();
    let rev_ja = l.revision.as_ref().map(|r| format!("、版 {r}")).unwrap_or_default();
    Text::new(
        format!("出典: {} = law {} asof {}（{}）{rev_ja}", l.name, l.id, l.asof, pins.join("、")),
        format!("Cites: {} = law {} asof {} ({}){rev}", l.name, l.id, l.asof, pins.join(", ")),
    )
    .get(lang)
    .to_string()
}

/// The helpers `needed` call, added to it.
fn close(mut needed: BTreeSet<H>, cal: Option<&CalData>) -> BTreeSet<H> {
    loop {
        let more: Vec<H> = needed.iter().flat_map(|h| h.needs(cal)).filter(|h| !needed.contains(h)).collect();
        if more.is_empty() {
            return needed;
        }
        needed.extend(more);
    }
}

/// What a dates file generates.
pub fn dates_unit(m: &Model, lang: Lang) -> Unit {
    let cal = m.cal.as_ref().map(CalData::of);
    let head = Head { kind: Kind::Dates, path: &m.path, name: &m.file.name.text, version: &m.file.version, sha: &m.sha256 };
    let header = header(lang, &head, m.cal.as_ref(), &m.laws);
    let inputs: Vec<InputG> = m.inputs.iter().map(|i| InputG { name: i.name.clone(), alias: i.alias.clone(), ty: i.ty, lo: i.lo, hi: i.hi }).collect();
    let mut helpers = BTreeSet::new();
    let timed = cal.as_ref().and_then(|c| c.offset).is_some();
    let dates: Vec<DateG> = m
        .dates
        .iter()
        .enumerate()
        .map(|(k, d)| {
            let mut steps = Vec::new();
            for &c in &d.chain {
                let cd = &m.dates[c];
                for (i, o) in cd.ops.iter().enumerate() {
                    let call = Call::of(&o.op);
                    call.helpers(&mut helpers);
                    let comment = if i == 0 && c != k { format!("{}  ({})", o.text, cd.name) } else { o.text.clone() };
                    steps.push(StepG { call, comment });
                }
            }
            let at = match (d.at, timed) {
                (Some((a, _)), true) => {
                    helpers.insert(H::At);
                    let text = match a {
                        At::Time(t) => format!("at {:02}:{:02}", t / 60, t % 60),
                        At::EndOfDay => "at end of day".to_string(),
                    };
                    Some((crate::vectors::at_minutes(a), text))
                }
                _ => None,
            };
            let ast = &m.file.dates[k];
            let mut cites: Vec<String> = ast.cite.iter().map(cite_text).collect();
            cites.extend(ast.ops.iter().filter_map(|o| o.cite.as_ref()).map(cite_text));
            if let Some((_, _, Some(c))) = &ast.at {
                cites.push(cite_text(c));
            }
            DateG { name: d.name.clone(), alias: d.alias.clone(), params: d.params.clone(), steps, at, cites }
        })
        .collect();
    if cal.is_some() {
        // A dates file with a calendar exports `is_open` too (DESIGN 8).
        helpers.insert(H::IsOpen);
    }
    let helpers = close(helpers, cal.as_ref());
    Unit {
        lang,
        kind: Kind::Dates,
        name: m.file.name.text.clone(),
        alias: m.file.name.ascii().unwrap_or("").to_string(),
        description: m.file.description.clone(),
        header,
        cal,
        inputs,
        dates,
        helpers,
    }
}

/// What a calendar file generates: `is_open`.
pub fn calendar_unit(c: &Calendar, lang: Lang) -> Unit {
    let data = CalData::of(c);
    let head = Head { kind: Kind::Calendar, path: &c.info.path, name: &c.info.name, version: &c.info.version, sha: &c.info.sha256 };
    let header = header(lang, &head, Some(c), &[]);
    let mut helpers = BTreeSet::new();
    helpers.insert(H::IsOpen);
    let helpers = close(helpers, Some(&data));
    Unit {
        lang,
        kind: Kind::Calendar,
        name: c.info.name.clone(),
        alias: c.info.alias.clone().unwrap_or_default(),
        description: None,
        header,
        cal: Some(data),
        inputs: vec![],
        dates: vec![],
        helpers,
    }
}

/// The files a target writes for a unit, as paths under the output directory and contents.
pub fn files(u: &Unit, t: Target) -> Vec<(String, String)> {
    let (module, runner) = match t {
        Target::TypeScript => (typescript::module(u), typescript::runner(u)),
        Target::Python => (python::module(u), python::runner(u)),
        Target::Go => (go::module(u), go::runner(u)),
        Target::Rust => (rust::module(u), rust::runner(u)),
        Target::Sql => (sql::module(u), sql::runner(u)),
    };
    vec![(t.file(&u.alias), module), (runner_file(t, &u.alias), runner)]
}

/// Where a target's runner goes (DESIGN 6.4). Go's is a test file of the package, so that it
/// needs no import path: `go test -c` builds it into a program that reads the vectors.
pub fn runner_file(t: Target, alias: &str) -> String {
    match t {
        Target::TypeScript => format!("typescript/{alias}_runner.ts"),
        Target::Python => format!("python/{alias}_runner.py"),
        Target::Go => {
            let p = crate::naming::go_package(alias);
            format!("go/{p}/{p}_runner_test.go")
        }
        Target::Rust => format!("rust/{alias}_runner.rs"),
        Target::Sql => format!("sql/{alias}_runner.sql"),
    }
}

// ── Messages ─────────────────────────────────────────────────────────────────

/// A piece of a message the generated code builds when it raises: words, or a value the code
/// has at hand (named as in the helper that raises).
#[derive(Clone, Copy, Debug)]
pub enum Piece {
    Lit(&'static str),
    Var(&'static str),
}

/// The messages of the four kinds of error (DESIGN 6.1), in the language the code is generated
/// in. Every variable is a string in the helper that raises.
pub fn message(lang: Lang, which: Msg) -> Vec<Piece> {
    use Piece::{Lit, Var};
    match (which, lang) {
        (Msg::Range, Lang::En) => vec![Var("name"), Lit(" "), Var("value"), Lit(" is outside its range "), Var("lo"), Lit(".."), Var("hi")],
        (Msg::Range, Lang::Ja) => vec![Var("name"), Lit(" "), Var("value"), Lit(" は範囲 "), Var("lo"), Lit("〜"), Var("hi"), Lit(" の外です")],
        (Msg::Data, Lang::En) => vec![Lit("whether "), Var("day"), Lit(" is a business day is outside what the calendar knows, "), Var("lo"), Lit(".."), Var("hi")],
        (Msg::Data, Lang::Ja) => vec![Var("day"), Lit(" が営業日かは、カレンダーが知っている範囲 "), Var("lo"), Lit("〜"), Var("hi"), Lit(" の外なので分かりません")],
        (Msg::Reject, Lang::En) => vec![Var("day"), Lit(" does not exist, and the .cal says else reject")],
        (Msg::Reject, Lang::Ja) => vec![Var("day"), Lit(" は無い日で、.cal は else reject と書いています")],
        (Msg::Outside, Lang::En) => vec![Lit("the date goes outside 0001-01-01..9999-12-31")],
        (Msg::Outside, Lang::Ja) => vec![Lit("日付が 0001-01-01〜9999-12-31 の外に出ます")],
        (Msg::NotADate, Lang::En) => vec![Var("text"), Lit(" is not a date of 0001-01-01..9999-12-31 written YYYY-MM-DD")],
        (Msg::NotADate, Lang::Ja) => vec![Var("text"), Lit(" は 0001-01-01〜9999-12-31 の日付（YYYY-MM-DD の形）ではありません")],
        (Msg::Bug, Lang::En) => vec![Lit("a bug in koyomi: the check said this operation never lands on a day the month does not have")],
        (Msg::Bug, Lang::Ja) => vec![Lit("koyomi の不具合: 検査は、この操作が無い日に当たらないとしていました")],
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Msg {
    Range,
    Data,
    Reject,
    Outside,
    NotADate,
    Bug,
}

/// A doc line in the language the code is generated in: English starts with a capital (but
/// not a line that starts with a name, `payment_terms.payment gives`), and Japanese has a
/// space between an ASCII word and the Japanese around it (DESIGN 4.1).
fn said(u: &Unit, t: Text) -> String {
    match u.lang {
        Lang::En if t.en.starts_with(|c: char| c.is_ascii_lowercase()) && t.en.split_whitespace().nth(1).is_some_and(|w| w == "gives" || w == "says") => t.en,
        _ => ritsu_base::text::spaced(&t, u.lang),
    }
}

/// `FirstDay は` / `FirstDay gives`, where a target starts a doc comment with the function's
/// name (Go's convention, and SQL, where the comment stands apart from the function).
fn subject(name: Option<&str>) -> (String, String) {
    match name {
        Some(n) => (format!("{n} は"), format!("{n} gives")),
        None => (String::new(), "gives".into()),
    }
}

/// A name with its alias, `支払日 (payment)`, or the name alone when it is its own alias
/// (`payment`). `quote` puts the Japanese name in 「」.
pub fn named(name: &str, alias: &str, quote: bool) -> Text {
    let ja_name = if quote { format!("「{name}」") } else { name.to_string() };
    if name == alias || alias.is_empty() {
        Text::new(ja_name, name)
    } else {
        Text::new(format!("{ja_name}（{alias}）"), format!("{name} ({alias})"))
    }
}

/// The doc comment of a date's function: what it gives, the range of its inputs, and the
/// error outside it.
pub fn date_doc(u: &Unit, d: &DateG, name: Option<&str>) -> Vec<String> {
    let ranges: Vec<(String, String)> = d
        .params
        .iter()
        .map(|k| {
            let i = &u.inputs[*k];
            (format!("{} {}..{}", i.name, i.show(i.lo), i.show(i.hi)), format!("{} {}〜{}", i.name, i.show(i.lo), i.show(i.hi)))
        })
        .collect();
    let en_ranges: Vec<&str> = ranges.iter().map(|r| r.0.as_str()).collect();
    let ja_ranges: Vec<&str> = ranges.iter().map(|r| r.1.as_str()).collect();
    let (ja, en) = subject(name);
    let n = named(&d.name, &d.alias, false);
    let mut lines = vec![said(u, Text::new(format!("{ja}{}を返す。", n.ja), format!("{en} {}.", n.en)))];
    lines.push(said(
        u,
        Text::new(
            format!("受け取る範囲は{} で、koyomi check はこの範囲のすべての入力を確かめた。範囲の外は KoyomiError（range）になる。", ja_ranges.join("、")),
            format!("It takes {}, the range koyomi check checked every input of; outside it, the error is a KoyomiError of kind range.", en_ranges.join(" and ")),
        ),
    ));
    if !d.cites.is_empty() {
        lines.push(said(u, Text::new(format!("引用: {}", d.cites.join(" ")), format!("Cites: {}", d.cites.join(" ")))));
    }
    lines
}

/// The doc comment of a date's `_at` function.
pub fn at_doc(u: &Unit, d: &DateG, name: Option<&str>, date_fn: &str) -> Vec<String> {
    let (_, text) = d.at.as_ref().unwrap();
    let off = u.cal.as_ref().and_then(|c| c.offset).map(crate::calendar::offset_text).unwrap_or_default();
    let (ja, en) = subject(name);
    vec![said(
        u,
        Text::new(
            format!("{ja}{date_fn} の日の時刻（{text}、オフセット {off}）を、RFC 3339 の UTC（Z で終わる形）で返す。"),
            format!("{en} the time of {date_fn}'s day ({text}, offset {off}) in RFC 3339, in UTC, ending in Z."),
        ),
    )]
}

/// The doc comment of `is_open`.
pub fn is_open_doc(u: &Unit, name: Option<&str>) -> Vec<String> {
    let c = u.cal.as_ref().unwrap();
    let (lo, hi) = (Day(c.data.0), Day(c.data.1));
    let (ja, _) = subject(name);
    let en = match name {
        Some(n) => format!("{n} says"),
        None => "says".into(),
    };
    vec![
        said(u, Text::new(format!("{ja}その日がカレンダー「{}」の営業日かを返す。", c.name), format!("{en} whether a day is a business day of the calendar {}.", c.name))),
        said(
            u,
            Text::new(
                format!("カレンダーが知っている範囲は {lo}〜{hi} で、その外の日は KoyomiError（data）になる。"),
                format!("The calendar knows {lo}..{hi}; outside it, the error is a KoyomiError of kind data."),
            ),
        ),
    ]
}

/// `Monday first: closed weekly sat, sun`, the comment on the days of the week.
pub fn weekly_comment(u: &Unit, names: &[&str]) -> String {
    u.t(tr!("月曜から順に。closed weekly {}", "Monday first: closed weekly {}", names.join(", ")))
}

/// The lines of a block with trailing comments, the comments lined up one space after the
/// longest line, the way gofmt lines them up. Widths count characters.
pub fn aligned(lines: &[(String, String)], indent: &str, marker: &str) -> String {
    let w = lines.iter().map(|(c, _)| c.chars().count()).max().unwrap_or(0);
    let mut o = String::new();
    for (code, comment) in lines {
        if comment.is_empty() {
            o.push_str(&format!("{indent}{code}\n"));
        } else {
            let pad = w - code.chars().count() + 1;
            o.push_str(&format!("{indent}{code}{}{marker} {comment}\n", " ".repeat(pad)));
        }
    }
    o
}

// ── The command ──────────────────────────────────────────────────────────────

/// What a checked file generates, by target.
pub fn unit_of(checked: &crate::check::Checked, lang: Lang) -> Unit {
    match checked {
        crate::check::Checked::Dates(m, _) => dates_unit(m, lang),
        crate::check::Checked::Calendar(c) => calendar_unit(c, lang),
    }
}

/// The day number of `YYYY-MM-DD`, for the generators' comments.
pub fn day_text(z: i32) -> String {
    Day(z).to_string()
}

/// `0001-01-01` and `9999-12-31` as day numbers.
pub const MIN: i32 = crate::date::MIN.0;
pub const MAX: i32 = crate::date::MAX.0;
