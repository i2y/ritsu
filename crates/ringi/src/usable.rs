//! The days a request can be used on (DESIGN 4.6, 5.10, 8.3): from the day it takes effect
//! (`effective from`, else the first day every piece of data it reads is there), through the last
//! day every piece of it is there. One place decides both, and everything that reads the days uses
//! them: the holes in a master (E107), a calendar's data (E404), and the range of the day a stage
//! starts in the lowered `terms.cal`.
//!
//! - The first day: `effective from`; else the latest of the first version of the organization, the
//!   first day a row of each master it chooses from may be chosen on, and the first day of each
//!   calendar's data.
//! - The last day: the earliest of the last day a row of each master may be chosen on (none for a
//!   row with no end), the last day a stage can start on whose deadline, reminder and deadline
//!   after it goes up koyomi can count in the calendar's data, and the last day a date of the form
//!   counted in business days can be counted from. A calendar with no table of closed days has no
//!   end. The last day is asked of koyomi itself: the lowered dates are laid with the day a stage
//!   starts ranging over the calendar's whole data, and the days koyomi says it cannot count from
//!   (its E203, every day of them) are the days past the end.
//!
//! The days are the request's: those its first calendar that writes a UTC offset reads (DESIGN
//! 7.5). A calendar at another offset counts from its own day of an event, which may be the day
//! before or after ([`spread`]): its first day and its last are moved by those days, so that every
//! event of a day the request can be used on is on a day each calendar can count from.
//!
//! A day inside the range that some data does not have is a mistake: a day no row of a master may
//! be chosen on (E107), the day the request takes effect before the organization's first version
//! (E502) or before a calendar's data (E404). The end of the range is not a mistake: it is said as
//! a fact with the data that decides it (`check` prints it under its `ok` line, the JSON's `usable`,
//! the port `Requests::usable`). With `--through <day>`, a request that cannot be used through that
//! day is a mistake (E107 for a master that ends before it, E404 for a calendar).

use crate::ast::{ExprKind, Field, How, TypeExpr, UseKind};
use crate::check::Options;
use crate::diag::Diag;
use crate::lower::{self, Ctx, LFile, Part};
use crate::names::Scope;
use crate::org::masters::MasterData;
use crate::suite::Suite;
use ritsu_base::json::Json;
use ritsu_base::text::{Lang, Text};
use std::rc::Rc;

/// The first day a request is taken to start on when nothing says one: as the deadlines were
/// counted before there was a first day.
pub const FIRST: i64 = 10_957; // 2000-01-01
/// A whole turn of the Gregorian calendar, in days: how far past the first day the deadlines are
/// checked when nothing ends.
pub const TURN: i64 = 146_097;
/// The last day of a calendar with no table of closed days (koyomi's 9999-12-31).
const NO_TABLE_END: i64 = 2_932_896;

/// What decides an end of the days.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ByKind {
    EffectiveFrom,
    Organization,
    Master,
    /// The deadlines counted in a calendar.
    Calendar,
    /// The reminders counted in a calendar that counts no deadline, or the reminders counted in
    /// another calendar than their deadline after the stage goes up.
    Reminder,
    /// A date of the form counted in business days in a calendar.
    FormDate,
    /// The days to resubmit counted in a calendar.
    Resubmit,
}

impl ByKind {
    pub fn word(self) -> &'static str {
        match self {
            ByKind::EffectiveFrom => "effective from",
            ByKind::Organization => "organization",
            ByKind::Master => "master",
            ByKind::Calendar => "calendar",
            ByKind::Reminder => "reminder",
            ByKind::FormDate => "form date",
            ByKind::Resubmit => "resubmit",
        }
    }
}

/// One piece of data that decides an end.
#[derive(Clone, Debug)]
pub struct By {
    pub kind: ByKind,
    /// What the request reads it by: the alias of its `use` line (`org`, `m`, `office`), with the
    /// master's name (`m.accounts`), or the field (`back_on`).
    pub name: String,
    /// The file the data is in, as a diagnostic names it; for an organization, the directory of its
    /// data.
    pub file: String,
    /// The line of that file, when a row decides it.
    pub line: Option<usize>,
    /// The day the data itself starts or ends on, when it is not the end itself (a calendar's data
    /// ends past the last day a stage can start on).
    pub data: Option<i64>,
    /// What it is, in words.
    pub text: Text,
}

impl By {
    /// The name the JSON takes: the ASCII name of the alias, of the master or of the field.
    pub fn ascii<'x>(&'x self, names: &'x crate::engine::Names) -> &'x str {
        match self.kind {
            ByKind::Master => names.master(&self.name),
            ByKind::FormDate => names.field(&self.name),
            ByKind::EffectiveFrom => &self.name,
            _ => names.used(&self.name),
        }
    }
}

/// An end of the days, and what decides it.
#[derive(Clone, Debug)]
pub struct End {
    pub day: i64,
    pub by: Vec<By>,
}

/// The days a request can be used on: None at an end that nothing decides (from any day, with no
/// end).
#[derive(Clone, Debug, Default)]
pub struct Usable {
    pub from: Option<End>,
    pub through: Option<End>,
}

impl Usable {
    /// The lines `check` prints under the `ok` line: none when nothing the request reads decides
    /// either end (no organization, no master, no calendar with an end).
    pub fn lines(&self) -> Vec<Text> {
        let mut out = Vec::new();
        if self.from.is_none() && self.through.is_none() {
            return out;
        }
        match &self.from {
            Some(e) => {
                let d = crate::lex::date_text(e.day);
                let by = said_by(&e.by);
                out.push(tr!("{d} から使えます: {}", "used from {d}: {}", by.ja; by.en));
            }
            None => out.push(tr!("いつからでも使えます: 読むデータに始まりがありません", "used from any day: no data it reads has a first day")),
        }
        match &self.through {
            Some(e) => {
                let d = crate::lex::date_text(e.day);
                let by = said_by(&e.by);
                out.push(tr!("{d} まで使えます: {}", "used through {d}: {}", by.ja; by.en));
            }
            None => out.push(tr!("終わり無く使えます: 読むデータに終わりがありません", "used with no end: no data it reads ends")),
        }
        out
    }

    /// The JSON's `usable`: `from` and `through` (a date, or null), each with what decides it, the
    /// files written from `root`.
    pub fn to_json(&self, names: &crate::engine::Names, root: &std::path::Path, lang: Lang) -> Json {
        let rel = |f: &str| ritsu_base::paths::from_root(root, std::path::Path::new(f)).unwrap_or_else(|| f.to_string());
        let end = |e: &Option<End>| match e {
            None => (Json::Null, Json::arr(Vec::<Json>::new())),
            Some(e) => (
                Json::str(crate::lex::date_text(e.day)),
                Json::arr(e.by.iter().map(|b| {
                    Json::obj([
                        ("kind", Json::str(b.kind.word())),
                        ("name", Json::str(b.ascii(names))),
                        ("written", Json::str(&b.name)),
                        ("file", Json::str(rel(&b.file))),
                        ("line", b.line.map(|l| Json::int(l as i128)).unwrap_or(Json::Null)),
                        ("data", b.data.map(|d| Json::str(crate::lex::date_text(d))).unwrap_or(Json::Null)),
                        ("text", Json::str(b.text.get(lang))),
                    ])
                })),
            ),
        };
        let (from, from_by) = end(&self.from);
        let (through, through_by) = end(&self.through);
        Json::obj([("from", from), ("from_by", from_by), ("through", through), ("through_by", through_by)])
    }

    /// As the port hands it over.
    pub fn handed(&self) -> ritsu_ports::RequestUsable {
        let end = |e: &Option<End>| {
            e.as_ref().map(|e| ritsu_ports::UsableEnd {
                day: e.day,
                by: e.by.iter().map(|b| ritsu_ports::UsableBy { kind: b.kind.word().to_string(), name: b.name.clone(), file: b.file.clone(), line: b.line, data: b.data, text: b.text.clone() }).collect(),
            })
        };
        ritsu_ports::RequestUsable { from: end(&self.from), through: end(&self.through) }
    }
}

/// What the data that decide an end say, joined.
pub fn said_by(by: &[By]) -> Text {
    let ja: Vec<String> = by.iter().map(|b| b.text.ja.clone()).collect();
    let en: Vec<String> = by.iter().map(|b| b.text.en.clone()).collect();
    Text::new(ja.join("。"), en.join("; "))
}

/// A choice from a master, or a lookup in one, of the request: where it is written, and the
/// master's data.
struct Chooses<'s> {
    line: usize,
    col: usize,
    /// `m.accounts`.
    name: String,
    what: Text,
    data: Option<&'s MasterData>,
}

fn chooses<'s>(s: &'s Scope) -> Vec<Chooses<'s>> {
    let mut out: Vec<Chooses> = Vec::new();
    let mut add = |alias: &str, master: &str, line: usize, col: usize, what: Text| {
        let name = format!("{alias}.{master}");
        if out.iter().any(|c| c.name == name) {
            return;
        }
        let Some(md) = s.masters_data(alias) else { return };
        out.push(Chooses { line, col, name, what, data: md.master(master) });
    };
    let mut all: Vec<&Field> = Vec::new();
    for f in s.fields() {
        all.push(f);
        all.extend(f.children.iter());
    }
    for f in all {
        if let TypeExpr::ChoiceFrom(p) = &f.ty
            && p.len() == 2
            && s.is_alias(&p[0].text, UseKind::Masters)
        {
            let n = &f.name.text;
            add(&p[0].text, &p[1].text, f.line, f.name.span.col, tr!("フィールド `{n}`", "the field `{n}`"));
        }
    }
    for w in &s.file.who {
        if let How::Lookup { path, .. } = &w.how
            && path.len() == 3
            && s.is_alias(&path[0].text, UseKind::Masters)
        {
            let n = &w.name.text;
            add(&path[0].text, &path[1].text, w.line, w.name.span.col, tr!("承認者の決め方 `{n}`", "the way to choose `{n}`"));
        }
    }
    out
}

/// The `use` line of the calendar `alias`, as written: in the file, else in the shared route it
/// uses.
fn calendar_written(s: &Scope, alias: &str) -> String {
    if let Some((u, _)) = s.alias_of(alias, UseKind::Calendar) {
        return u.path.clone();
    }
    for f in s.shared_routes().into_iter().flatten() {
        if let Some(u) = f.uses.iter().find(|u| u.kind == UseKind::Calendar && u.alias.as_ref().is_some_and(|a| a.text == alias)) {
            return u.path.clone();
        }
    }
    String::new()
}

/// The first line of the request that counts business days in the calendar `alias` for a
/// deadline (an `every stage` line, or a stage's own), when there is one.
fn deadline_line(cx: &Ctx, alias: &str) -> usize {
    let s = cx.s;
    let first = cx.calendars.first().map(|c| c.alias.as_str());
    let counts_in = |c: &Option<crate::ast::Word>| match c {
        Some(w) => w.text == alias,
        None => first == Some(alias),
    };
    let mut lines: Vec<usize> = Vec::new();
    let mut look = |d: &crate::ast::Deadlines| {
        for l in &d.lines {
            match l {
                crate::ast::Deadline::Due { calendar, line, .. } | crate::ast::Deadline::Remind { calendar, line, .. } if counts_in(calendar) => lines.push(*line),
                _ => {}
            }
        }
    };
    if let Some(d) = &s.file.every_stage {
        look(d);
    }
    if let Some(crate::ast::Route::Stages(ls)) = &s.file.route {
        for l in ls {
            if let Some(d) = &l.deadlines {
                look(d);
            }
        }
    }
    lines.into_iter().min().unwrap_or_else(|| match &s.file.route {
        Some(crate::ast::Route::Shared { line, .. }) => *line,
        _ => s.file.name.span.line,
    })
}

/// The field of the form a date counted in business days in `form.cal` is, with the field it
/// counts from.
fn form_counts<'s>(s: &'s Scope) -> Option<(&'s Field, &'s Field)> {
    for f in s.file.form.iter().filter(|f| f.computed.is_some() && matches!(f.ty, TypeExpr::Date)) {
        let e = f.computed.as_ref()?;
        if let ExprKind::Bin(_, a, b) = &e.kind
            && let (ExprKind::Path(dp), ExprKind::BusinessDays(..)) = (&a.kind, &b.kind)
            && let Some(d) = s.field(&dp[0].text)
        {
            return Some((f, d));
        }
    }
    None
}

/// What koyomi says of each of the lowered dates files `files`, laid where the check lays them: the
/// runs of the day input it cannot count from (its E203), each as (first day, last day), with the
/// integer inputs.
fn probe(cx: &Ctx, suite: &Suite, files: &[LFile]) -> Vec<(String, Vec<(i64, i64)>)> {
    let Some(dates) = &suite.dates else { return Vec::new() };
    let dir = lower::dir_of(cx.s.path);
    let mut ov = ritsu_base::fs::Overlay::new(ritsu_base::fs::current());
    for f in files {
        ov.put(&ritsu_base::paths::opened(&dir.join(&f.name)), f.text.clone());
    }
    let ov: Rc<dyn ritsu_base::fs::Files> = Rc::new(ov);
    ritsu_base::fs::with(ov, || {
        files
            .iter()
            .map(|f| {
                let runs = match dates.findings(&dir.join(&f.name), cx.lang) {
                    Ok(ritsu_ports::Found::Value(found)) => found
                        .iter()
                        .filter(|x| x.code == "E203")
                        .flat_map(|x| x.json.get("fails").and_then(|j| j.as_arr()).map(|a| a.to_vec()).unwrap_or_default())
                        .filter_map(|r| {
                            let a = ritsu_ports::day_of(r.get("from")?.as_str()?)?;
                            let b = ritsu_ports::day_of(r.get("to")?.as_str()?)?;
                            Some((a, b))
                        })
                        .collect(),
                    _ => Vec::new(),
                };
                (f.name.clone(), runs)
            })
            .collect()
    })
}

/// The days the request in `cx` can be used on, with the ranges of the lowered dates set in `cx`
/// for them; the holes in the days, and the ends `--through` asks past, said to `diags`.
pub fn find(cx: &mut Ctx, suite: &Suite, opts: &Options, diags: &mut Vec<Diag>) -> Usable {
    let s = cx.s;
    let org = s.org_data();
    let org_file = s.uses_of(UseKind::Organization).next().map(|u| u.path.clone()).unwrap_or_default();
    let ms = chooses(s);
    for c in ms.iter().filter(|c| c.data.is_none()) {
        let n = &c.name;
        diags.push(Diag::warning("W500", s.path, c.line, c.col, tr!("マスタ `{n}` にデータが無いので、どの日にも選べる行があるかを決められません", "The master `{n}` has no data, so whether a row can be chosen on every day is not decided")).source(s.src).note(tr!(
            "マスタのファイルに `data` の行を書き、データを置いてください。",
            "Write a `data` line in the masters' file, and put the data there."
        )));
    }
    let cal_text = |alias: &str| -> (String, String) {
        let w = calendar_written(s, alias);
        (w.clone(), crate::names::beside(s.path, &w).map(|p| crate::org::data::fold(&p).display().to_string()).unwrap_or(w))
    };
    // ── The first day ──
    let mut starts: Vec<(i64, By)> = Vec::new();
    if let Some(o) = org
        && let Some(first) = o.first()
    {
        let (w, dir) = (&o.written, &o.dir);
        starts.push((first, By { kind: ByKind::Organization, name: org_alias(s), file: dir.clone(), line: None, data: None, text: tr!("組織 `{org_file}` の最初の版（データ `{w}`）", "the first version of the organization `{org_file}` (data `{w}`)") }));
    }
    for c in &ms {
        if let Some(md) = c.data
            && let Some((d, r)) = md.first()
        {
            let (n, file, line) = (&c.name, &md.file, r.line);
            starts.push((d, By { kind: ByKind::Master, name: c.name.clone(), file: md.file.clone(), line: Some(line), data: None, text: tr!("マスタ `{n}` の最初の行（`{file}` の {line} 行目）", "the first row of the master `{n}` (`{file}`, line {line})") }));
        }
    }
    let ours = cx.calendars.iter().find_map(|c| c.offset).unwrap_or(0);
    let offset_of = |alias: &str| cx.calendars.iter().find(|c| c.alias == alias).and_then(|c| c.offset).unwrap_or(ours);
    let spread_of = |alias: &str| spread(ours, offset_of(alias));
    for c in &cx.calendars {
        if let Some((lo, hi)) = c.data
            && hi != NO_TABLE_END
        {
            let (_, file) = cal_text(&c.alias);
            let a = &c.alias;
            let (before, _) = spread_of(a);
            let text = if before < 0 {
                let (ld, pd) = (crate::lex::date_text(lo), crate::lex::date_text(lo - 1));
                tr!(
                    "カレンダー `{a}`（`{file}`）のデータは {ld} からで、申請の種類の {ld} の初めは、カレンダーのオフセットで読むとまだ {pd} です",
                    "the data of the calendar `{a}` (`{file}`) starts on {ld}, and the first hours of the request's {ld} are still {pd} in the calendar's offset"
                )
            } else {
                tr!("カレンダー `{a}`（`{file}`）のデータの最初の日", "the first day of the data of the calendar `{a}` (`{file}`)")
            };
            starts.push((lo - before, By { kind: ByKind::Calendar, name: c.alias.clone(), file: file.clone(), line: None, data: Some(lo), text }));
        }
    }
    let from = match s.file.effective_from {
        Some((d, span)) => Some(End { day: d, by: vec![By { kind: ByKind::EffectiveFrom, name: "effective from".into(), file: s.path.to_string(), line: Some(span.line), data: None, text: tr!("`effective from`", "`effective from`") }] }),
        None => starts.iter().map(|(d, _)| *d).max().map(|d| End { day: d, by: starts.iter().filter(|(x, _)| *x == d).map(|(_, b)| b.clone()).collect() }),
    };
    let s_day = from.as_ref().map(|e| e.day);
    // the day it takes effect before the data is there
    if let Some((ef, span)) = s.file.effective_from {
        let at = |code: &'static str, msg: Text| Diag::error(code, s.path, span.line, span.col, msg).source(s.src);
        let efd = crate::lex::date_text(ef);
        if let Some(o) = org
            && let Some(first) = o.first()
            && ef < first
        {
            let (fd, before) = (crate::lex::date_text(first), crate::lex::date_text(first - 1));
            diags.push(at("E502", tr!("{efd}（`effective from`）から {before} までは、組織の版がありません。どのステップにも承認者がいません", "From {efd} (`effective from`) to {before}, no version of the organization is in effect: no stage has an approver"))
                .note(tr!("組織 `{org_file}` の最初の版は {fd} からです。", "The first version of the organization `{org_file}` takes effect on {fd}."))
                .note(tr!("`effective from` を {fd} 以後にするか、{efd} から効く組織の版を置いてください。", "Make `effective from` {fd} or later, or put a version of the organization that takes effect on {efd}.")));
        }
        for c in &cx.calendars {
            if let Some((lo, hi)) = c.data
                && hi != NO_TABLE_END
                && ef + spread_of(&c.alias).0 < lo
            {
                let (_, file) = cal_text(&c.alias);
                let (a, ld) = (&c.alias, crate::lex::date_text(lo));
                diags.push(at("E404", tr!("{efd}（`effective from`）から使いますが、カレンダー `{a}` のデータは {ld} からです", "The request is used from {efd} (`effective from`), and the data of the calendar `{a}` starts on {ld}"))
                    .note(tr!("`{file}` は {ld} より前の日が休みかを知らないので、その日々の期限を数えられません。", "`{file}` does not know which days before {ld} are closed, so the deadlines of those days cannot be counted."))
                    .note(tr!("カレンダーの休みの表を {efd} まで広げるか、`effective from` を {ld} 以後にしてください。", "Extend the calendar's table of closed days back to {efd}, or make `effective from` {ld} or later.")));
            }
        }
    }
    // ── The last day ──
    let mut ends: Vec<(i64, By, Option<(usize, usize, &'static str)>)> = Vec::new();
    for c in &ms {
        if let Some(md) = c.data
            && let Some((d, r)) = md.last()
        {
            let (n, file, line, dd) = (&c.name, &md.file, r.line, crate::lex::date_text(d));
            ends.push((d, By { kind: ByKind::Master, name: c.name.clone(), file: md.file.clone(), line: Some(line), data: None, text: tr!("マスタ `{n}` の行は {dd} まで（`{file}` の {line} 行目）", "the rows of the master `{n}` end on {dd} (`{file}`, line {line})") }, Some((c.line, c.col, "E107"))));
        }
    }
    // the deadlines: lay `terms.cal` with the day a stage starts over each calendar's whole data,
    // from its own day of the request's first
    let lo_of = |alias: &str, data: (i64, i64)| (s_day.unwrap_or(FIRST) + spread_of(alias).0).max(data.0);
    cx.windows = cx.calendars.iter().filter_map(|c| c.data.filter(|d| d.1 != NO_TABLE_END).map(|d| (c.alias.clone(), (lo_of(&c.alias, d), d.1)))).filter(|(_, (a, b))| a <= b).collect();
    let mut terms_ends: Vec<(String, i64)> = Vec::new();
    if !cx.windows.is_empty() {
        let files: Vec<LFile> = lower::terms::terms(cx).into_iter().filter(|f| matches!(&f.part, Part::Terms(a) if cx.windows.iter().any(|(w, _)| w == a))).collect();
        for (name, runs) in probe(cx, suite, &files) {
            let Some(f) = files.iter().find(|f| f.name == name) else { continue };
            let Part::Terms(alias) = &f.part else { continue };
            let Some((_, (lo, hi))) = cx.windows.iter().find(|(a, _)| a == alias).cloned() else { continue };
            let end = runs.iter().filter(|(_, b)| *b == hi).map(|(a, _)| a - 1).min().unwrap_or(hi);
            terms_ends.push((alias.clone(), end.max(lo - 1)));
        }
    }
    // a reminder counted in another calendar than its deadline: after the stage goes up, the machine
    // counts it in the reminder's calendar from the day the deadline passes there (DESIGN 5.10, 7.5),
    // which a lowered file does not hold; the last day a stage starts on for it to be counted, a day
    // of the deadline's calendar (the one that counts the deadline it goes up at), with the
    // reminder's calendar
    let mut apart_ends: Vec<(String, String, i64)> = Vec::new();
    {
        let apart = crate::apart::stages(s, cx.calendars.first().map(|c| c.alias.as_str()));
        let all = if apart.iter().any(|a| a.ups > 0) { crate::engine::calendars(s, suite) } else { Vec::new() };
        let facts = |named: Option<&crate::ast::Word>| match named {
            Some(n) => all.iter().find(|(a, _)| *a == n.text),
            None => all.first(),
        };
        for a in apart.iter().filter(|a| a.ups > 0) {
            let (Some((due_alias, due_in)), Some((remind_alias, remind_in))) = (facts(a.due.1), facts(a.remind.1)) else { continue };
            // the reminder's calendar ends the days where its data ends
            let Some((_, (_, remind_hi))) = cx.windows.iter().find(|(w, _)| w == remind_alias).cloned() else { continue };
            if !terms_ends.iter().any(|(w, _)| w == remind_alias) {
                continue;
            }
            let (o_d, o_r) = (offset_of(due_alias), offset_of(remind_alias));
            // the days of the deadline's calendar a stage can start on: from the request's first,
            // through the last whose deadline can pass on a day the reminder's calendar has
            let due_data = cx.calendars.iter().find(|c| &c.alias == due_alias).and_then(|c| c.data);
            let lo = due_data.map_or(s_day.unwrap_or(FIRST) + spread_of(due_alias).0, |d| lo_of(due_alias, d));
            let mut hi = remind_hi - spread(o_d, o_r).0;
            if let Some((_, (_, h))) = cx.windows.iter().find(|(w, _)| w == due_alias) {
                hi = hi.min(*h);
            }
            if let Some((_, e)) = terms_ends.iter().find(|(w, _)| w == due_alias) {
                hi = hi.min(*e);
            }
            apart_ends.push((remind_alias.clone(), due_alias.clone(), crate::apart::last_start(a, due_in, remind_in, (o_d, o_r), lo, hi)));
        }
    }
    // a paused stage has its clocks counted from a time later by the days the business days it was
    // paused for take in the pause's calendar, from the day it started there; each calendar counts
    // from its own day of that time (DESIGN 5.7, 7.5): with `pause at most N business days`, the last
    // day a stage starts on is the last day whose every time, moved so, is on a day each calendar
    // can count from
    let pause = s.file.ticket.as_ref().and_then(|t| t.lines.iter().find_map(|l| if let crate::ast::TicketLine::Pause { days, calendar, .. } = l { Some((*days, calendar.as_ref().map(|w| w.text.clone()))) } else { None })).filter(|(days, _)| *days > 0);
    let paused = pause.as_ref().and_then(|(days, named)| {
        let all = crate::engine::calendars(s, suite);
        let (alias, facts) = match named {
            Some(n) => all.iter().find(|(a, _)| a == n),
            None => all.first(),
        }?;
        Some((*days, facts.clone(), offset_of(alias)))
    });
    // each day a time of the request's day `day` is on where the calendar at `offset` is, with the
    // days the most a stage may be paused for moves it later by
    let moves = |day: i64, offset: i32| -> Vec<(i64, i64)> {
        match &paused {
            None => phases(day, ours, offset, offset).into_iter().map(|(c, _)| (c, 0)).collect(),
            Some((days, facts, held_in)) => phases(day, ours, offset, *held_in).into_iter().map(|(c, p)| (c, crate::apart::nth_open_loose(facts, p, *days) - p)).collect(),
        }
    };
    // the last of the request's days a stage can start on, for the last day `end` the calendar
    // `alias` can count from: each time of it, moved later by the most a stage may be paused for,
    // on a day through `end` where the calendar is
    let back = |alias: &str, end: i64| -> i64 {
        let (before, after) = spread_of(alias);
        let first = cx.calendars.iter().find(|c| c.alias == alias).and_then(|c| c.data).map_or(s_day.unwrap_or(FIRST) + before, |d| lo_of(alias, d));
        let floor = first - before - 1;
        let mut day = end - after;
        while day > floor && moves(day, offset_of(alias)).iter().any(|(c, m)| c + m > end) {
            day -= 1;
        }
        day.max(floor)
    };
    for (alias, end) in &terms_ends {
        let Some(c) = cx.calendars.iter().find(|c| &c.alias == alias) else { continue };
        let Some((_, hi)) = c.data else { continue };
        let (_, file) = cal_text(alias);
        let hd = crate::lex::date_text(hi);
        let line = deadline_line(cx, alias);
        // the last day the calendar's own dates can be counted from, and the last day the reminders it
        // counts after a stage goes up can be, where that is earlier
        let own = back(alias, *end);
        let after = apart_ends.iter().filter(|(r, ..)| r == alias).map(|(_, d, last)| back(d, *last)).min();
        let (end, up) = match after {
            Some(a) if a < own => (a, true),
            _ => (own, false),
        };
        // what of a stage the calendar counts last: its deadlines, its reminders where it counts no
        // deadline, or the reminder after the stage goes up
        let reminders = crate::apart::counts_reminders_alone(s, cx.calendars.first().map(|c| c.alias.as_str()), alias);
        let kind = if up || reminders { ByKind::Reminder } else { ByKind::Calendar };
        let mut text = match (up, reminders) {
            (true, _) => tr!(
                "カレンダー `{alias}`（`{file}`）のデータが {hd} までなので、その後に始まるステップの、上へ回したあとの催促を数えられません",
                "the data of the calendar `{alias}` (`{file}`) ends on {hd}, and the reminders of a stage started later cannot be counted in it after the stage goes up"
            ),
            (false, true) => tr!(
                "カレンダー `{alias}`（`{file}`）のデータが {hd} までなので、その後に始まるステップの催促を数えられません",
                "the data of the calendar `{alias}` (`{file}`) ends on {hd}, and the reminders of a stage started later cannot be counted in it"
            ),
            (false, false) => tr!(
                "カレンダー `{alias}`（`{file}`）のデータが {hd} までなので、その後に始まるステップの期限を数えられません",
                "the data of the calendar `{alias}` (`{file}`) ends on {hd}, and the deadlines of a stage started later cannot be counted in it"
            ),
        };
        if let Some((days, _)) = &pause {
            text = match (up, reminders) {
                (true, _) => tr!(
                    "カレンダー `{alias}`（`{file}`）のデータが {hd} までなので、その後に始まり、保留を {days} 営業日使ったステップの、上へ回したあとの催促を数えられません（`pause at most {days} business days`）",
                    "the data of the calendar `{alias}` (`{file}`) ends on {hd}, and the reminders of a stage started later and paused for {days} business days (`pause at most {days} business days`) cannot be counted in it after the stage goes up"
                ),
                (false, true) => tr!(
                    "カレンダー `{alias}`（`{file}`）のデータが {hd} までなので、その後に始まり、保留を {days} 営業日使ったステップの催促を数えられません（`pause at most {days} business days`）",
                    "the data of the calendar `{alias}` (`{file}`) ends on {hd}, and the reminders of a stage started later and paused for {days} business days (`pause at most {days} business days`) cannot be counted in it"
                ),
                (false, false) => tr!(
                    "カレンダー `{alias}`（`{file}`）のデータが {hd} までなので、その後に始まり、保留を {days} 営業日使ったステップの期限を数えられません（`pause at most {days} business days`）",
                    "the data of the calendar `{alias}` (`{file}`) ends on {hd}, and the deadlines of a stage started later and paused for {days} business days (`pause at most {days} business days`) cannot be counted in it"
                ),
            };
        }
        ends.push((end, By { kind, name: alias.clone(), file: file.clone(), line: None, data: Some(hi), text }, Some((line, 1, "E404"))));
    }
    // a ticket that goes back to its requester has the days to resubmit counted from the day it
    // goes back (DESIGN 5.6): with `resubmit within N business days`, the last day a ticket goes
    // back on is the day before the Nth business day back from the end of the calendar's data
    let lines = s.file.ticket.as_ref().map(|t| t.lines.as_slice()).unwrap_or_default();
    let goes_back = lines.iter().any(|l| matches!(l, crate::ast::TicketLine::SendBack { .. } | crate::ast::TicketLine::Withdraw { .. }));
    let resubmit = lines.iter().find_map(|l| if let crate::ast::TicketLine::Resubmit { days, calendar, line } = l { Some((*days, calendar.as_ref().map(|w| w.text.clone()), *line)) } else { None });
    if let Some((days, named, line)) = resubmit.filter(|(days, ..)| goes_back && *days > 0) {
        let all = crate::engine::calendars(s, suite);
        let found = match &named {
            Some(n) => all.iter().find(|(a, _)| a == n),
            None => all.first(),
        };
        if let Some((alias, facts)) = found
            && let Some((lo, hi)) = cx.calendars.iter().find(|c| &c.alias == alias).and_then(|c| c.data)
            && hi != NO_TABLE_END
        {
            let (mut d, mut left) = (hi, days);
            while left > 0 && d >= lo_of(alias, (lo, hi)) {
                if !facts.closed.contains(&d) {
                    left -= 1;
                }
                d -= 1;
            }
            let end = d.max(lo_of(alias, (lo, hi)) - 1) - spread_of(alias).1;
            let (_, file) = cal_text(alias);
            let hd = crate::lex::date_text(hi);
            let text = tr!(
                "カレンダー `{alias}`（`{file}`）のデータが {hd} までなので、その後に申請者へ戻ったチケットの再申請の期限を数えられません（`resubmit within {days} business days`）",
                "the data of the calendar `{alias}` (`{file}`) ends on {hd}, and the days to resubmit a ticket that goes back to its requester later cannot be counted in it (`resubmit within {days} business days`)"
            );
            ends.push((end, By { kind: ByKind::Resubmit, name: alias.clone(), file, line: None, data: Some(hi), text }, Some((line, 1, "E404"))));
        }
    }
    // a date of the form counted in business days: over the days the field takes and the
    // calendar's data covers
    if let Some((computed, counted)) = form_counts(s)
        && let Some(cal) = form_calendar(cx, computed)
        && let Some((clo, chi)) = cal.data
    {
        let (flo, fhi) = lower::words::ends(counted);
        let flo = flo.filter(|r| r.is_int()).map(|r| r.num as i64);
        let fhi = fhi.filter(|r| r.is_int()).map(|r| r.num as i64);
        let (alias, (_, file)) = (cal.alias.clone(), cal_text(&cal.alias));
        let (cn, dn) = (&computed.name.text, &counted.name.text);
        if let Some(l) = flo
            && l < clo
        {
            let (ld, cd) = (crate::lex::date_text(l), crate::lex::date_text(clo));
            diags.push(Diag::error("E404", s.path, computed.line, computed.name.span.col, tr!(
                "`{cn}` は `{dn}` から営業日を数えますが、`{dn}` は {ld} からを取り、カレンダー `{alias}` のデータは {cd} からです",
                "`{cn}` counts business days from `{dn}`, which takes days from {ld}, and the data of the calendar `{alias}` starts on {cd}"
            )).source(s.src).note(tr!(
                "`{file}` は {cd} より前の日が休みかを知りません。カレンダーの休みの表を広げるか、`{dn}` の範囲を {cd} からにしてください。",
                "`{file}` does not know which days before {cd} are closed: extend the calendar's table of closed days, or make the range of `{dn}` start on {cd}."
            )));
        }
        if chi != NO_TABLE_END {
            let lo = flo.unwrap_or(clo).max(clo);
            let hi = fhi.unwrap_or(chi).min(chi);
            if lo <= hi {
                cx.form_window = Some((lo, hi));
                let files: Vec<LFile> = lower::form::form(cx).into_iter().filter(|f| f.part == Part::FormDates).collect();
                let runs: Vec<(i64, i64)> = probe(cx, suite, &files).into_iter().flat_map(|(_, r)| r).collect();
                let end = runs.iter().filter(|(_, b)| *b == hi).map(|(a, _)| a - 1).min();
                let hd = crate::lex::date_text(chi);
                if let Some(end) = end {
                    cx.form_window = Some((lo, end.max(lo)));
                    if end < lo {
                        cx.form_window = None;
                    }
                    ends.push((end, By { kind: ByKind::FormDate, name: computed.name.text.clone(), file: file.clone(), line: None, data: Some(chi), text: tr!(
                        "カレンダー `{alias}`（`{file}`）のデータが {hd} までなので、`{cn}` を、それより後の `{dn}` から数えられません",
                        "the data of the calendar `{alias}` (`{file}`) ends on {hd}, and `{cn}` cannot be counted from a later `{dn}`"
                    ) }, Some((computed.line, computed.name.span.col, "E404"))));
                } else {
                    cx.form_window = Some((lo, hi));
                }
            }
        }
    }
    let through = ends.iter().map(|(d, ..)| *d).min().map(|d| End { day: d, by: ends.iter().filter(|(x, ..)| *x == d).map(|(_, b, _)| b.clone()).collect() });
    let e_day = through.as_ref().map(|e| e.day);
    // the data ends before the request is first used: it cannot be used on any day
    if let (Some(sd), Some(ed)) = (s_day, e_day)
        && ed < sd
    {
        let sdt = crate::lex::date_text(sd);
        for (d, by, at) in &ends {
            let Some((line, col, code)) = at else { continue };
            if *d >= sd {
                continue;
            }
            let dd = crate::lex::date_text(*d);
            diags.push(Diag::error(code, s.path, *line, *col, tr!("{sdt} から使いますが、{dd} のあとは使えないので、使える日がありません", "The request is used from {sdt}, and cannot be used after {dd}: there is no day it can be used on"))
                .source(s.src)
                .note(tr!("{}。", "{}.", by.text.ja; by.text.en))
                .note(tr!("データを、{sdt} より後まで足してください。", "Extend the data past {sdt}.")));
        }
    }
    // ── The holes ──
    for c in &ms {
        let Some(md) = c.data else { continue };
        let from_day = s_day.unwrap_or(i64::MIN);
        for h in md.holes(from_day, e_day) {
            let n = &c.name;
            let a = if h.from == i64::MIN { String::new() } else { crate::lex::date_text(h.from) };
            let b = h.to.map(crate::lex::date_text).unwrap_or_default();
            let mut d = Diag::error("E107", s.path, c.line, c.col, tr!("マスタ `{n}` には、{a}〜{b} に選べる行がありません", "The master `{n}` has no row to choose on {a} to {b}")).source(s.src);
            let used = match (&from, &through) {
                (Some(f), Some(t)) => {
                    let (x, y) = (crate::lex::date_text(f.day), crate::lex::date_text(t.day));
                    tr!("この申請は {x}〜{y} に使い、その毎日、{} がこのマスタを読みます。", "The request is used from {x} through {y}, and {} reads the master on each of those days.", c.what.ja; c.what.en)
                }
                (Some(f), None) => {
                    let x = crate::lex::date_text(f.day);
                    tr!("この申請は {x} から使い、その毎日、{} がこのマスタを読みます。", "The request is used from {x} on, and {} reads the master on each of those days.", c.what.ja; c.what.en)
                }
                _ => tr!("{} がこのマスタを読みます。", "{} reads the master.", c.what.ja; c.what.en),
            };
            d = d.note(used);
            let file = &md.file;
            if let Some(r) = h.before {
                let (l, k) = (r.line, &r.key);
                d = d.note(tr!("その前の行: `{file}` の {l} 行目（`{k}`、{}）。`valid_to` を {b} にすれば埋まります。", "The row before: `{file}`, line {l} (`{k}`, {}); making its `valid_to` {b} fills it.", crate::org::masters::row_days(r).ja; crate::org::masters::row_days(r).en));
            }
            if let Some(r) = h.after {
                let (l, k) = (r.line, &r.key);
                d = d.note(tr!("その後の行: `{file}` の {l} 行目（`{k}`、{}）。`valid_from` を {a} にすれば埋まります。", "The row after: `{file}`, line {l} (`{k}`, {}); making its `valid_from` {a} fills it.", crate::org::masters::row_days(r).ja; crate::org::masters::row_days(r).en));
            }
            if h.before.is_none() && h.after.is_none() {
                d = d.note(tr!("`{file}` に、その日々に効く行を足してください。", "Add rows that are in effect on those days to `{file}`."));
            }
            diags.push(d);
        }
    }
    // ── `--through` ──
    if let Some(want) = opts.through {
        let wd = crate::lex::date_text(want);
        for (d, by, at) in &ends {
            if *d >= want {
                continue;
            }
            let Some((line, col, code)) = at else { continue };
            let dd = crate::lex::date_text(*d);
            let msg = tr!("{wd}（`--through`）まで使えません。使える最後の日は {dd} です", "The request cannot be used through {wd} (`--through`): it can be used through {dd}");
            let fix = match by.kind {
                ByKind::Master => tr!("マスタの行を、{wd} まで選べるように足すか延ばしてください。", "Add rows to the master, or extend them, so that a row may be chosen through {wd}."),
                ByKind::FormDate => tr!(
                    "カレンダーの休みの表を、{wd} までの日から数えた日が入るところまで延ばしてください。",
                    "Extend the calendar's table of closed days, so that the days counted from a day through {wd} fall in it."
                ),
                ByKind::Resubmit => tr!(
                    "カレンダーの休みの表を、{wd} に申請者へ戻ったチケットの再申請の期限を数えられるところまで延ばしてください。",
                    "Extend the calendar's table of closed days, so that the days to resubmit a ticket that goes back on {wd} can be counted in it."
                ),
                ByKind::Reminder => tr!(
                    "カレンダーの休みの表を、{wd} に始まるステップの催促を数えられるところまで延ばしてください。",
                    "Extend the calendar's table of closed days, so that the reminders of a stage started on {wd} can be counted in it."
                ),
                _ => tr!(
                    "カレンダーの休みの表を、{wd} に始まるステップの期限を数えられるところまで延ばしてください。",
                    "Extend the calendar's table of closed days, so that the deadlines of a stage started on {wd} can be counted in it."
                ),
            };
            diags.push(Diag::error(code, s.path, *line, *col, msg).source(s.src).note(tr!("{}。", "{}.", by.text.ja; by.text.en)).note(fix));
        }
    }
    // ── The ranges of the lowered dates ──
    // each calendar's own days of the request's: the day a stage's clocks are counted from is the
    // day it starts there, and as many business days later as it was paused for; past no day the
    // deadlines can be counted from
    let window = |alias: &str, data: Option<(i64, i64)>| {
        let (before, after) = spread_of(alias);
        let lo = s_day.unwrap_or(FIRST) + before;
        let hi = match e_day {
            Some(e) => moves(e, offset_of(alias)).iter().map(|(c, m)| c + m).max().unwrap_or(e + after),
            None => s_day.unwrap_or(FIRST) + TURN - 1 + after,
        };
        let hi = terms_ends.iter().find(|(a, _)| a == alias).map_or(hi, |(_, end)| hi.min(*end).max(e_day.map_or(hi, |e| e + after)));
        (data.map(|d| lo.max(d.0)).unwrap_or(lo), hi)
    };
    cx.windows = cx.calendars.iter().map(|c| (c.alias.clone(), window(&c.alias, c.data))).collect();
    Usable { from, through }
}

/// The days an event of one of the request's days can be on where a calendar at the UTC offset
/// `theirs` is, the request reading its days at `ours` (DESIGN 7.5): from `.0` days after the
/// request's day to `.1` (0 and 0 at one offset, 0 and 1 for a calendar ahead of the request's by
/// less than a day, -1 and 0 for one behind it).
pub fn spread(ours: i32, theirs: i32) -> (i64, i64) {
    let d = i64::from(theirs - ours) * 60;
    (d.div_euclid(86_400), (86_399 + d).div_euclid(86_400))
}

/// The days the times of the request's day `day` are on where two calendars at the UTC offsets `a`
/// and `b` are, the request reading its days at `ours`: each pair once, at most three (a pair holds
/// from a time either calendar's day turns at to the next).
fn phases(day: i64, ours: i32, a: i32, b: i32) -> Vec<(i64, i64)> {
    let start = day * 86_400 - i64::from(ours) * 60;
    let mut times = vec![start];
    for o in [a, b] {
        let turn = ((start + i64::from(o) * 60).div_euclid(86_400) + 1) * 86_400 - i64::from(o) * 60;
        if turn < start + 86_400 {
            times.push(turn);
        }
    }
    let mut out: Vec<(i64, i64)> = times.into_iter().map(|t| (crate::simulate::day_at(t, a), crate::simulate::day_at(t, b))).collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// The alias of the `use organization` line.
fn org_alias(s: &Scope) -> String {
    s.uses_of(UseKind::Organization).next().and_then(|u| u.alias.as_ref()).map(|a| a.text.clone()).unwrap_or_default()
}

/// The calendar the computed date `f` counts business days in.
fn form_calendar<'c>(cx: &'c Ctx, f: &Field) -> Option<&'c lower::UsedCal> {
    let e = f.computed.as_ref()?;
    let ExprKind::Bin(_, _, b) = &e.kind else { return None };
    let ExprKind::BusinessDays(_, cal) = &b.kind else { return None };
    cx.calendar(cal.as_ref().map(|c| c.text.as_str()))
}
