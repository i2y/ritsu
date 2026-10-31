//! The days of koyomi dates where a workflow gives them (DESIGN 7.5 (a), 7.8): to a rule's date
//! input (X3 (a), E202 and W202), and to a koyomi date's date input (X6, E205 and W205). koyomi
//! computes a date on every input of its range, so the days a date comes to are known exactly
//! (`Dates::values`); dandori says where each value a flow gives can come from (`Origin`). A value
//! that can only be the day of koyomi dates, or a day of a range the `.flow` writes, is decided
//! over those days ([`borders::days_given`]); one that can also come from an input or a task's
//! answer without a range, or `now` in a workflow that says no period, says nothing of what day it
//! is. A time with a range, and `now` in a workflow that says the period it runs in, are the days
//! their moments are at the calendar's offset ([`borders::moment_days`]): what the code dandori
//! writes gives koyomi.
//!
//! - X3 (a): each value given to a rule's date input that can be the day of a koyomi date, or a
//!   day of a range, is a border, held to the range the rule declares for the input
//!   (`Rules::date_range`). An input whose range is a koyomi date (`range from koyomi`) is not: its
//!   days are a precondition of the rule, and X2 holds the value's days to them, a day at a time.
//! - X6: each call of a koyomi date is a border, the day given to its date input held to the range
//!   of that input. The date's integer inputs are dandori's own to check (its E014), as a rule's are.
//!
//! The record of each border (`Border::detail`) says the value as the flow writes it (`given`),
//! the days the other side takes (`takes`), every place the value can come from (`from`, by its
//! `kind`: `day`, `range`, `moment`, `now`, `unknown`, `output`), the example's day and the place of
//! `from` it comes from (`example`), and why it is undecided (`why`).

use crate::borders::DayFrom;
use crate::{borders, named, Border, Borders, Came, Check, Flow, Side};
use ritsu_base::diag::Diag;
use ritsu_base::json::Json;
use ritsu_base::naming::Tool;
use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;
use ritsu_ports::{day_text, Answer, ColumnType, DateKind, Dates, DaySet, Finding, Found, Origin, Precondition, Rules};
use ritsu_project::{Joined, Project};
use std::path::PathBuf;

/// One koyomi date a value can be the day of: the file as the project names it, the date, the file
/// as dandori reaches it, and the days koyomi counts for it.
struct Date {
    file: String,
    date: String,
    disk: PathBuf,
    days: Found<DaySet>,
}

/// A range of days a value can be in, from a range the `.flow` writes: a date's, or a time's or the
/// period's read as days at the calendar's offset.
struct DayRange {
    days: (Option<i64>, Option<i64>),
    /// the range as the `.flow` writes it (`>=2026-01-01 <=2027-12-28 offset +09:00`)
    text: String,
    /// for a time's range, the calendar's offset its moments are read at (minutes east of UTC), and
    /// whether it is the period `now` is held to
    moments: Option<(i32, bool)>,
}

/// The days a value can be, by where it can come from: each koyomi date it can be the day of, each
/// range of days, and the first place it can come from that says nothing of what day it is; with
/// every place it can come from as the record of the border writes it, and the place in it of each
/// date and each range.
struct Days {
    dates: Vec<Date>,
    ranges: Vec<DayRange>,
    other: Option<Text>,
    from: Vec<Json>,
    at: Vec<(DayFrom, usize)>,
}

impl Days {
    /// The place in `from` an example of `days_given` comes from.
    fn place(&self, f: DayFrom) -> Option<usize> {
        self.at.iter().find(|(x, _)| *x == f).map(|(_, i)| *i)
    }
}

/// Days as the record of a border writes them: the first and the last (null at an open end).
fn days_json(r: (Option<i64>, Option<i64>)) -> Json {
    Json::obj([("from", Json::opt_str(r.0.map(day_text))), ("to", Json::opt_str(r.1.map(day_text)))])
}

/// What koyomi says of the days of the date a value can come from, and of everything else it can
/// come from. `offset` is the UTC offset of the calendar of the dates file the value is given to
/// (minutes east), which a time's days are read at; None where the value goes to a rule, which
/// takes no time, or to a calendar that says none.
fn days(project: &Project, joined: &Joined, shown: &str, from: &[Origin], offset: Option<i32>, lang: Lang) -> Days {
    let mut d = Days { dates: Vec::new(), ranges: Vec::new(), other: None, from: Vec::new(), at: Vec::new() };
    for o in from {
        match o {
            Origin::Day { file, date } => {
                let named = named(project, file);
                let days = match joined.koyomi.values(file, date) {
                    Ok(Found::Undecided(why)) => Found::Undecided(tr!("koyomi は \"{named}\" の {date} の日を数えません（{}）", "koyomi does not count the days of {date} in \"{named}\": {}", why.ja; why.en)),
                    Ok(v) => v,
                    Err(_) => Found::Undecided(tr!("koyomi から \"{named}\" の情報を得られません", "koyomi does not answer for \"{named}\"")),
                };
                let counted = match &days {
                    Found::Value(set) => vec![
                        ("days", Json::obj([("from", Json::opt_str(set.first().copied().map(day_text))), ("to", Json::opt_str(set.last().copied().map(day_text))), ("count", Json::from(set.len()))])),
                        ("why", Json::Null),
                    ],
                    Found::Undecided(why) => vec![("days", Json::Null), ("why", Json::str(why.get(lang)))],
                };
                let mut rec = vec![("kind", Json::str("day")), ("file", Json::str(&named)), ("date", Json::str(date))];
                rec.extend(counted);
                d.at.push((DayFrom::Date(d.dates.len()), d.from.len()));
                d.from.push(Json::obj(rec));
                d.dates.push(Date { file: named, date: date.clone(), disk: file.clone(), days });
            }
            Origin::Range(lo, hi) => {
                let days = (lo.and_then(|x| i64::try_from(x).ok()), hi.and_then(|x| i64::try_from(x).ok()));
                let text = range_text(days);
                d.at.push((DayFrom::Range(d.ranges.len()), d.from.len()));
                d.from.push(Json::obj([("kind", Json::str("range")), ("range", Json::str(&text)), ("days", days_json(days))]));
                d.ranges.push(DayRange { days, text, moments: None });
            }
            Origin::Moment { lo, hi, text, now } => {
                let kind = if *now { "now" } else { "moment" };
                let key = if *now { "period" } else { "range" };
                match offset {
                    Some(off) => {
                        let days = borders::moment_days((*lo, *hi), off);
                        d.at.push((DayFrom::Range(d.ranges.len()), d.from.len()));
                        d.from.push(Json::obj([("kind", Json::str(kind)), (key, Json::str(text)), ("days", days_json(days))]));
                        d.ranges.push(DayRange { days, text: text.clone(), moments: Some((off, *now)) });
                    }
                    None => {
                        d.from.push(Json::obj([("kind", Json::str(kind)), (key, Json::str(text)), ("days", Json::Null)]));
                        d.other.get_or_insert(tr!(
                            "`{shown}` は時刻で、渡す先はそれを何日に読むかを言いません",
                            "`{shown}` is a time, and where it is given says no day it reads it as"
                        ));
                    }
                }
            }
            Origin::Now => {
                d.from.push(Json::obj([("kind", Json::str("now")), ("period", Json::Null), ("days", Json::Null)]));
                d.other.get_or_insert(if shown == "now" {
                    tr!("`now` はどの日にもなりえます", "`now` can be any day")
                } else {
                    tr!("`{shown}` は `now` のことがあり、どの日にもなりえます", "`{shown}` can be `now`, which can be any day")
                });
            }
            Origin::Unknown(t) => {
                d.from.push(Json::obj([("kind", Json::str("unknown")), ("place", Json::str(t.get(lang)))]));
                d.other.get_or_insert(tr!("`{shown}` が何日になるかは分かりません（{}）", "nothing says what day `{shown}` is ({} gives no range of days)", crate::then_ja(&t.ja, "には日付の範囲がありません"); t.en));
            }
            Origin::Output { rule, output } => {
                d.from.push(Json::obj([("kind", Json::str("output")), ("rule", Json::str(named(project, rule))), ("output", Json::str(output))]));
                d.other.get_or_insert(tr!("`{shown}` は koyomi の日付でないところから来ることがあります", "`{shown}` can come from somewhere other than a koyomi date"));
            }
        }
    }
    d
}

/// Where a day of an example from a range comes from, as a note says it after the value: `a day of
/// `>=2025-12-01 <=2026-12-31``, or for a time the days its moments are at the calendar's offset.
/// The Japanese is put right after `は`, with the space a code span takes before it.
fn range_from(r: &DayRange, file: &str) -> Text {
    let days = range_text(r.days);
    match r.moments {
        None => tr!(" `{}` の日", "a day of `{}`", r.text),
        Some((off, now)) => {
            let off = offset_text(off);
            let utc = if r.text.contains("offset") { "" } else { " (UTC)" };
            let utc_ja = if r.text.contains("offset") { " " } else { "（UTC）" };
            if now {
                tr!(
                    "ワークフローが動く期間 `{}`{utc_ja}の時刻で、\"{file}\" のカレンダー（{off}）では `{days}` の日",
                    "a moment of the period the workflow runs in, `{}`{utc}, which is a day of `{days}` in the calendar of \"{file}\" ({off})",
                    r.text; r.text
                )
            } else {
                tr!(
                    " `{}`{utc_ja}の時刻で、\"{file}\" のカレンダー（{off}）では `{days}` の日",
                    "a moment of `{}`{utc}, which is a day of `{days}` in the calendar of \"{file}\" ({off})",
                    r.text; r.text
                )
            }
        }
    }
}

/// What to narrow where a day from a range is outside, after "Widen …, or": the range, or the
/// period.
fn narrow(r: &DayRange) -> Text {
    match r.moments {
        Some((_, true)) => tr!(
            "ワークフローが動く期間を狭めてください（`offset` を書けば、カレンダーと同じ日で書けます）",
            "narrow the period the workflow runs in (with `offset`, it is written in the calendar's days)"
        ),
        _ => tr!("値の出どころの範囲 `{}` を狭めてください", "narrow the range `{}` where the value comes from", r.text; r.text),
    }
}

/// A UTC offset in minutes east, as koyomi and dandori write it: `+09:00`.
fn offset_text(off: i32) -> String {
    let sign = if off < 0 { '-' } else { '+' };
    let a = off.unsigned_abs();
    format!("{sign}{:02}:{:02}", a / 60, a % 60)
}

/// The record of a date border, after its six keys: the value as the flow writes it, the days the
/// other side takes, every place the value can come from, the example (its day, and the place of
/// `from` it comes from) and why it is undecided.
fn detail(shown: &str, takes: (Option<i64>, Option<i64>), d: &Days, answer: &Answer<(DayFrom, i64)>, lang: Lang) -> Vec<(String, Json)> {
    let (example, why) = match answer {
        Answer::Holds => (Json::Null, Json::Null),
        Answer::Fails((f, day)) => (Json::obj([("day", Json::str(day_text(*day))), ("from", d.place(*f).map_or(Json::Null, Json::from))]), Json::Null),
        Answer::Undecided(why) => (Json::Null, Json::str(why.get(lang))),
    };
    vec![
        ("given".into(), Json::str(shown)),
        ("takes".into(), days_json(takes)),
        ("from".into(), Json::Arr(d.from.clone())),
        ("example".into(), example),
        ("why".into(), why),
    ]
}

/// The record of the border of a rule's precondition that an input takes the days `set` of a
/// koyomi date (X2), after its six keys, as [`detail`] writes it for the other borders of days: the
/// days it takes (with how many there are), and the example, by its place in `from`.
pub(crate) fn kept_detail(project: &Project, joined: &Joined, a: &ritsu_ports::CallArg, set: &DaySet, answer: &Answer<Text>, example: Option<(usize, i64)>, lang: Lang) -> Vec<(String, Json)> {
    let d = days(project, joined, &a.shown, &a.from, None, lang);
    let takes = Json::obj([("from", Json::opt_str(set.first().copied().map(day_text))), ("to", Json::opt_str(set.last().copied().map(day_text))), ("count", Json::from(set.len()))]);
    let example = example.map_or(Json::Null, |(i, day)| Json::obj([("day", Json::str(day_text(day))), ("from", Json::from(i))]));
    let why = match answer {
        Answer::Undecided(why) => Json::str(why.get(lang)),
        _ => Json::Null,
    };
    vec![("given".into(), Json::str(&a.shown)), ("takes".into(), takes), ("from".into(), Json::Arr(d.from)), ("example".into(), example), ("why".into(), why)]
}

/// The input of the dates file at `disk` at which `date` comes to `day`, as `received = 2026-01-20`.
fn input_text(joined: &Joined, disk: &std::path::Path, date: &str, day: i64) -> Option<String> {
    let facts = joined.koyomi.facts(disk).ok()?;
    let inputs = joined.koyomi.input_for(disk, date, day).ok()??;
    let shown: Vec<String> = inputs
        .iter()
        .map(|(n, v)| match facts.inputs.iter().find(|i| i.name == *n).map(|i| i.kind) {
            Some(DateKind::Date) => format!("{n} = {}", day_text(*v)),
            _ => format!("{n} = {v}"),
        })
        .collect();
    Some(shown.join(", "))
}

/// Where a day of the example comes from, as a note says it: `a day koyomi "terms.cal" date payment
/// comes to (at received = 2026-01-20)`.
fn day_from(joined: &Joined, d: &Date, day: i64) -> Text {
    match input_text(joined, &d.disk, &d.date, day) {
        Some(at) => tr!("koyomi \"{}\" date {} が {at} のときに返す日", "a day koyomi \"{}\" date {} comes to (at {at})", d.file, d.date),
        None => tr!("koyomi \"{}\" date {} が返す日", "a day koyomi \"{}\" date {} comes to", d.file, d.date),
    }
}

/// A range of days as rulec and koyomi write it: `>=2026-03-01 <=2027-01-31`.
fn range_text(r: (Option<i64>, Option<i64>)) -> String {
    let mut parts = Vec::new();
    if let Some(lo) = r.0 {
        parts.push(format!(">={}", day_text(lo)));
    }
    if let Some(hi) = r.1 {
        parts.push(format!("<={}", day_text(hi)));
    }
    if parts.is_empty() { "any day".into() } else { parts.join(" ") }
}

/// X3 (a): the days of koyomi dates given to rules' date inputs.
pub(crate) fn days_to_rules(project: &Project, flows: &[Flow], joined: &Joined, lang: Lang, borders: &mut Borders) -> Vec<Finding> {
    let mut out = Vec::new();
    for f in flows {
        for call in &f.calls.rules {
            let Ok(facts) = joined.rulec.facts(&call.rule) else { continue };
            for a in &call.args {
                // a value that can be the day of a koyomi date, or a day of a range: one from places
                // without a range only is left to the rule's code, as it was
                if !a.from.iter().any(|o| matches!(o, Origin::Day { .. } | Origin::Range(..))) {
                    continue;
                }
                if !facts.inputs.iter().any(|c| c.name == a.input && matches!(c.ty, ColumnType::Date)) {
                    continue;
                }
                // an input whose range is a koyomi date is the rule's precondition, which X2 holds
                if facts.preconditions.iter().any(|p| matches!(p, Precondition::Days { input, .. } if *input == a.input)) {
                    continue;
                }
                let range = joined.rulec.date_range(&call.rule, &a.input).unwrap_or((None, None));
                // a rule takes no time: a time given to it says no day here
                let d = days(project, joined, &a.shown, &a.from, None, lang);
                let sets: Vec<Found<DaySet>> = d.dates.iter().map(|x| x.days.clone()).collect();
                let ranges: Vec<(Option<i64>, Option<i64>)> = d.ranges.iter().map(|r| r.days).collect();
                let (rule, input, shown) = (&call.name, &a.input, &a.shown);
                let answer = borders::days_given(&sets, &ranges, d.other.as_ref(), range);
                let code = match &answer {
                    Answer::Holds => None,
                    Answer::Fails(_) => Some("E202"),
                    Answer::Undecided(_) => Some("W202"),
                };
                let (at, to) = (Side::at(Tool::Dandori, &f.file.rel, call.line), Side::reached(project, Tool::Rulec, &call.rule, None));
                let mut border = Border::new(Check::RuleDateInput, input.clone(), at, Some(to), Came::of(&answer), code);
                border.detail = detail(shown, range, &d, &answer, lang);
                borders.add(border);
                match answer {
                    Answer::Holds => {}
                    Answer::Fails((from, at_day)) => {
                        let (day, range) = (day_text(at_day), range_text(range));
                        let diag: Diag = Diag::at("E202", &f.file.shown, call.line, 1, tr!(
                            "規則 `{rule}` の `{input}` に渡す日が、規則の宣言した範囲を外れることがあります",
                            "The day given to `{input}` of the rule `{rule}` can be outside the range the rule declares"
                        ))
                        .source(&f.src)
                        .rel(&f.file.rel);
                        let diag = match from {
                            DayFrom::Date(i) => {
                                let x = &d.dates[i];
                                let from = day_from(joined, x, at_day);
                                diag.note(tr!("`{shown}` は {day} になることがあります（{}）。`{input}` が受け取るのは `{range}` です", "`{shown}` can be {day}, {}, and `{input}` takes `{range}`", from.ja; from.en))
                                    .note(tr!(
                                        "規則から生成したコードは、範囲の外の日を入口で受け付けません。この呼び出しは、ワークフローを走らせたときに初めて落ちます。koyomi は、入力の範囲のすべてでその日付を数えています。",
                                        "The rule's generated code refuses a day outside its range at its door, so this call fails only when the workflow runs. koyomi counts the days the date comes to over the whole range of its inputs."
                                    ))
                                    .note(tr!(
                                        "入力の範囲を広げるか、`range from koyomi \"{}\" date {}` にして、koyomi の日をそのまま範囲にしてください。",
                                        "Widen the input's range, or make it `range from koyomi \"{}\" date {}`, so that the rule takes the days koyomi gives.",
                                        x.file, x.date
                                    ))
                            }
                            DayFrom::Range(i) => {
                                let r = &d.ranges[i];
                                let from = range_from(r, "");
                                diag.note(tr!("`{shown}` は{}で、{day} になることがあります。`{input}` が受け取るのは `{range}` です", "`{shown}` is {} and can be {day}, and `{input}` takes `{range}`", from.ja; from.en))
                                    .note(tr!(
                                        "規則から生成したコードは、範囲の外の日を入口で受け付けません。この呼び出しは、ワークフローを走らせたときに初めて落ちます。範囲は、dandori が走らせたときに確かめるもので、その中のどの日も来ることがあります。",
                                        "The rule's generated code refuses a day outside its range at its door, so this call fails only when the workflow runs. dandori checks the range when the workflow runs, and any day of it can come."
                                    ))
                                    .note({
                                        let n = narrow(r);
                                        tr!("規則の `{input}` の範囲を広げるか、{}。", "Widen the range of `{input}` in the rule, or {}.", n.ja; n.en)
                                    })
                            }
                        };
                        out.push(Finding::of(&diag, Some(f.file.rel.clone()), lang));
                    }
                    Answer::Undecided(why) => {
                        let diag: Diag = Diag::at("W202", &f.file.shown, call.line, 1, tr!(
                            "規則 `{rule}` の `{input}` に渡す日が、範囲に収まるかを決められません",
                            "Whether the day given to `{input}` of the rule `{rule}` stays inside its range cannot be decided"
                        ))
                        .source(&f.src)
                        .rel(&f.file.rel)
                        .note(why)
                        .note(tr!(
                            "規則から生成したコードが、ワークフローを走らせたときに入口で日を確かめます。範囲の外の日なら、この呼び出しはそこで落ちます。",
                            "The rule's generated code checks the day at its door when the workflow runs; a day outside its range fails the call there."
                        ));
                        out.push(Finding::of(&diag, Some(f.file.rel.clone()), lang));
                    }
                }
            }
        }
    }
    out
}

/// X6: the day given to each koyomi date a flow calls.
pub(crate) fn days_to_dates(project: &Project, flows: &[Flow], joined: &Joined, lang: Lang, borders: &mut Borders) -> Vec<Finding> {
    let mut out = Vec::new();
    for f in flows {
        for call in &f.calls.dates {
            let Ok(facts) = joined.koyomi.facts(&call.file) else { continue };
            let Some((lo, hi)) = borders::input_range(&facts, &call.input) else { continue };
            // a time given to the date is the day it falls on in the calendar's offset
            let offset = facts.calendar.as_ref().and_then(|c| c.offset);
            let d = days(project, joined, &call.shown, &call.from, offset, lang);
            let sets: Vec<Found<DaySet>> = d.dates.iter().map(|x| x.days.clone()).collect();
            let ranges: Vec<(Option<i64>, Option<i64>)> = d.ranges.iter().map(|r| r.days).collect();
            let (date, input, shown, file) = (format!("{}.{}", call.name, call.date), &call.input, &call.shown, named(project, &call.file));
            let (min, max) = (day_text(lo), day_text(hi));
            let answer = borders::days_given(&sets, &ranges, d.other.as_ref(), (Some(lo), Some(hi)));
            let code = match &answer {
                Answer::Holds => None,
                Answer::Fails(_) => Some("E205"),
                Answer::Undecided(_) => Some("W205"),
            };
            let (at, to) = (Side::at(Tool::Dandori, &f.file.rel, call.line), Side::reached(project, Tool::Koyomi, &call.file, None));
            let mut border = Border::new(Check::KoyomiDateInput, input.clone(), at, Some(to), Came::of(&answer), code);
            border.detail = detail(shown, (Some(lo), Some(hi)), &d, &answer, lang);
            borders.add(border);
            match answer {
                Answer::Holds => {}
                Answer::Fails((from, day)) => {
                    let diag: Diag = Diag::at("E205", &f.file.shown, call.line, 1, tr!(
                        "koyomi の日付 `{date}` の `{input}` に渡す日が、入力の範囲を外れることがあります",
                        "The day given to `{input}` of the koyomi date `{date}` can be outside the input's range"
                    ))
                    .source(&f.src)
                    .rel(&f.file.rel);
                    let widen = |or: Text| tr!(
                        "\"{file}\" の `{input}` の範囲を広げるか（カレンダーのデータも足してください）、{}。",
                        "Widen the range of `{input}` in \"{file}\" (and the data of its calendar), or {}.",
                        or.ja; or.en
                    );
                    let refused = tr!(
                        "koyomi が生成したコードは、範囲の外の日を受け付けません。この呼び出しは、ワークフローを走らせたときに初めて落ちます。",
                        "koyomi's generated code refuses a day outside the range, so this call fails only when the workflow runs."
                    );
                    let diag = match from {
                        DayFrom::Date(i) => {
                            let from = day_from(joined, &d.dates[i], day);
                            let day = day_text(day);
                            diag.note(tr!("`{shown}` は {day} になることがあります（{}）。\"{file}\" が `{input}` に受け取るのは {min} から {max} までです", "`{shown}` can be {day}, {}, and \"{file}\" takes {min} to {max} for `{input}`", from.ja; from.en))
                                .note(refused)
                                .note(widen(tr!("範囲に収まる日を渡してください", "give it a day that stays inside")))
                        }
                        DayFrom::Range(i) => {
                            let r = &d.ranges[i];
                            let from = range_from(r, &file);
                            let day = day_text(day);
                            diag.note(tr!("`{shown}` は{}で、{day} になることがあります。\"{file}\" が `{input}` に受け取るのは {min} から {max} までです", "`{shown}` is {} and can be {day}, and \"{file}\" takes {min} to {max} for `{input}`", from.ja; from.en))
                                .note(refused)
                                .note(tr!(
                                    "範囲を書いた日付や時刻、動く期間を書いたワークフローの `now` から来る日は、その範囲の全部で確かめます。時刻と `now` は、日付のファイルのカレンダーのオフセットで日に読みます。",
                                    "A day from a date or a time with a range, or from `now` in a workflow that says the period it runs in, is checked over the whole range; a time and `now` are read as days at the offset of the dates file's calendar."
                                ))
                                .note(widen(narrow(r)))
                        }
                    };
                    out.push(Finding::of(&diag, Some(f.file.rel.clone()), lang));
                }
                Answer::Undecided(why) => {
                    let diag: Diag = Diag::at("W205", &f.file.shown, call.line, 1, tr!(
                        "koyomi の日付 `{date}` の `{input}` に渡す日が、入力の範囲に収まるかを決められません",
                        "Whether the day given to `{input}` of the koyomi date `{date}` stays inside the input's range cannot be decided"
                    ))
                    .source(&f.src)
                    .rel(&f.file.rel)
                    .note(tr!("{}。\"{file}\" が `{input}` に受け取るのは {min} から {max} までです", "{}, and \"{file}\" takes {min} to {max} for `{input}`", why.ja; why.en))
                    .note(tr!(
                        "範囲の外の日は、ワークフローを走らせたときに、koyomi が生成したコードが受け付けません。",
                        "koyomi's generated code refuses a day outside the range when the workflow runs."
                    ));
                    out.push(Finding::of(&diag, Some(f.file.rel.clone()), lang));
                }
            }
        }
    }
    out
}
