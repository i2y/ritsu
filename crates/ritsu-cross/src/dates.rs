//! The days of koyomi dates where a workflow gives them (DESIGN 7.5 (a), 7.8): to a rule's date
//! input (X3 (a), E202 and W202), and to a koyomi date's date input (X6, E205 and W205). koyomi
//! computes a date on every input of its range, so the days a date comes to are known exactly
//! (`Dates::values`); dandori says where each value a flow gives can come from (`Origin`). A value
//! that can only be the day of koyomi dates is decided over their days ([`borders::days_given`]);
//! one that can also come from an input, a task's answer or `now` says nothing of what day it is.
//!
//! - X3 (a): each value given to a rule's date input that can be the day of a koyomi date is a
//!   border, held to the range the rule declares for the input (`Rules::date_range`). An input
//!   whose range is a koyomi date (`range from koyomi`) is not: its days are a precondition of the
//!   rule, and X2 holds the value's days to them, a day at a time.
//! - X6: each call of a koyomi date is a border, the day given to its date input held to the range
//!   of that input. The date's integer inputs are dandori's own to check (its E014), as a rule's are.

use crate::{borders, named, Borders, Flow};
use ritsu_base::diag::Diag;
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

/// The days a value can be, by where it can come from: each koyomi date it can be the day of, and
/// the first place it can come from that says nothing of what day it is.
struct Days {
    dates: Vec<Date>,
    other: Option<Text>,
}

/// What koyomi says of the days of the date a value can come from, and of everything else it can come from.
fn days(project: &Project, joined: &Joined, shown: &str, from: &[Origin]) -> Days {
    let mut dates = Vec::new();
    let mut other: Option<Text> = None;
    for o in from {
        match o {
            Origin::Day { file, date } => {
                let named = named(project, file);
                let days = match joined.koyomi.values(file, date) {
                    Ok(Found::Undecided(why)) => Found::Undecided(tr!("koyomi は \"{named}\" の {date} の日を数えません（{}）", "koyomi does not count the days of {date} in \"{named}\": {}", why.ja; why.en)),
                    Ok(v) => v,
                    Err(_) => Found::Undecided(tr!("koyomi が \"{named}\" に答えません", "koyomi does not answer for \"{named}\"")),
                };
                dates.push(Date { file: named, date: date.clone(), disk: file.clone(), days });
            }
            Origin::Now => {
                other.get_or_insert(if shown == "now" {
                    tr!("`now` はどの日にもなりえます", "`now` can be any day")
                } else {
                    tr!("`{shown}` は `now` のことがあり、どの日にもなりえます", "`{shown}` can be `now`, which can be any day")
                });
            }
            Origin::Unknown(t) => {
                other.get_or_insert(tr!("`{shown}` が何日になるかは分かりません（{}）", "nothing says what day `{shown}` is ({} gives no range of days)", crate::then_ja(&t.ja, "には日付の範囲がありません"); t.en));
            }
            Origin::Output { .. } | Origin::Range(..) => {
                other.get_or_insert(tr!("`{shown}` は koyomi の日付でないところから来ることがあります", "`{shown}` can come from somewhere other than a koyomi date"));
            }
        }
    }
    Days { dates, other }
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
                if !a.from.iter().any(|o| matches!(o, Origin::Day { .. })) {
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
                let d = days(project, joined, &a.shown, &a.from);
                let sets: Vec<Found<DaySet>> = d.dates.iter().map(|x| x.days.clone()).collect();
                let (rule, input, shown) = (&call.name, &a.input, &a.shown);
                match borders::days_given(&sets, d.other.as_ref(), range) {
                    Answer::Holds => borders.held += 1,
                    Answer::Fails((i, day)) => {
                        borders.failed += 1;
                        let from = day_from(joined, &d.dates[i], day);
                        let (day, range) = (day_text(day), range_text(range));
                        let x = &d.dates[i];
                        let diag: Diag = Diag::at("E202", &f.file.shown, call.line, 1, tr!(
                            "規則 `{rule}` の `{input}` に渡す日が、規則の宣言した範囲を外れることがあります",
                            "The day given to `{input}` of the rule `{rule}` can be outside the range the rule declares"
                        ))
                        .source(&f.src)
                        .rel(&f.file.rel)
                        .note(tr!("`{shown}` は {day} になることがあります（{}）。`{input}` が受け取るのは `{range}` です", "`{shown}` can be {day}, {}, and `{input}` takes `{range}`", from.ja; from.en))
                        .note(tr!(
                            "規則の生成したコードは、範囲の外の日を入口で断ります。この呼び出しは、ワークフローを走らせたときに初めて落ちます。koyomi は、入力の範囲のすべてでその日付を数えています。",
                            "The rule's generated code refuses a day outside its range at its door, so this call fails only when the workflow runs. koyomi counts the days the date comes to over the whole range of its inputs."
                        ))
                        .note(tr!(
                            "入力の範囲を広げるか、`range from koyomi \"{}\" date {}` にして、koyomi の日をそのまま範囲にしてください。",
                            "Widen the input's range, or make it `range from koyomi \"{}\" date {}`, so that the rule takes the days koyomi gives.",
                            x.file, x.date
                        ));
                        out.push(Finding::of(&diag, Some(f.file.rel.clone()), lang));
                    }
                    Answer::Undecided(why) => {
                        borders.undecided += 1;
                        let diag: Diag = Diag::at("W202", &f.file.shown, call.line, 1, tr!(
                            "規則 `{rule}` の `{input}` に渡す日が、範囲に収まるかを決められません",
                            "Whether the day given to `{input}` of the rule `{rule}` stays inside its range cannot be decided"
                        ))
                        .source(&f.src)
                        .rel(&f.file.rel)
                        .note(why)
                        .note(tr!(
                            "規則の生成したコードが、ワークフローを走らせたときに入口で日を確かめます。範囲の外の日なら、この呼び出しはそこで落ちます。",
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
            let d = days(project, joined, &call.shown, &call.from);
            let sets: Vec<Found<DaySet>> = d.dates.iter().map(|x| x.days.clone()).collect();
            let (date, input, shown, file) = (format!("{}.{}", call.name, call.date), &call.input, &call.shown, named(project, &call.file));
            let (min, max) = (day_text(lo), day_text(hi));
            match borders::days_given(&sets, d.other.as_ref(), (Some(lo), Some(hi))) {
                Answer::Holds => borders.held += 1,
                Answer::Fails((i, day)) => {
                    borders.failed += 1;
                    let from = day_from(joined, &d.dates[i], day);
                    let day = day_text(day);
                    let diag: Diag = Diag::at("E205", &f.file.shown, call.line, 1, tr!(
                        "koyomi の日付 `{date}` の `{input}` に渡す日が、入力の範囲を外れることがあります",
                        "The day given to `{input}` of the koyomi date `{date}` can be outside the input's range"
                    ))
                    .source(&f.src)
                    .rel(&f.file.rel)
                    .note(tr!("`{shown}` は {day} になることがあります（{}）。\"{file}\" が `{input}` に受け取るのは {min} から {max} までです", "`{shown}` can be {day}, {}, and \"{file}\" takes {min} to {max} for `{input}`", from.ja; from.en))
                    .note(tr!(
                        "koyomi の生成したコードは範囲の外の日を断ります。この呼び出しは、ワークフローを走らせたときに初めて落ちます。",
                        "koyomi's generated code refuses a day outside the range, so this call fails only when the workflow runs."
                    ))
                    .note(tr!(
                        "\"{file}\" の `{input}` の範囲を広げるか（カレンダーのデータも足します）、範囲に収まる日を渡してください。",
                        "Widen the range of `{input}` in \"{file}\" (and the data of its calendar), or give it a day that stays inside."
                    ));
                    out.push(Finding::of(&diag, Some(f.file.rel.clone()), lang));
                }
                Answer::Undecided(why) => {
                    borders.undecided += 1;
                    let diag: Diag = Diag::at("W205", &f.file.shown, call.line, 1, tr!(
                        "koyomi の日付 `{date}` の `{input}` に渡す日が、入力の範囲に収まるかを決められません",
                        "Whether the day given to `{input}` of the koyomi date `{date}` stays inside the input's range cannot be decided"
                    ))
                    .source(&f.src)
                    .rel(&f.file.rel)
                    .note(tr!("{}。\"{file}\" が `{input}` に受け取るのは {min} から {max} までです", "{}, and \"{file}\" takes {min} to {max} for `{input}`", why.ja; why.en))
                    .note(tr!(
                        "範囲の外の日は、ワークフローを走らせたときに koyomi の生成したコードが断ります。",
                        "koyomi's generated code refuses a day outside the range when the workflow runs."
                    ));
                    out.push(Finding::of(&diag, Some(f.file.rel.clone()), lang));
                }
            }
        }
    }
    out
}
