//! The checks of the borders a workflow crosses into koyomi and chobo (DESIGN 7.5 (a), 7.6, 7.8:
//! X3 (a), X4, X6), each a decision over what the languages hand over through their ports. A flow
//! calls a koyomi date or a chobo transfer once dandori can write it (`use dates`, `use book`,
//! PLAN E.5); until then nothing in a project reaches these, and their codes (E202–E205, W202–W205)
//! have no reproduction yet. What each decides, and how, is written here so that the call sites
//! dandori will have only hand the facts over.
//!
//! - [`days_fit`] (X3 (a)): the days a koyomi date comes to, given to a rule's date input, are
//!   inside the range the rule declares for it. koyomi counts the days exactly (`Dates::values`),
//!   so the answer is exact: the first day outside is the example.
//! - [`amount_fits`] (X4): a rule's numeric output, given to a transfer as its amount, is one chobo
//!   takes: 1 to 2⁶³ − 1 (an amount outside is a failure of the call, not a refusal: chobo's
//!   DESIGN 1.5). The output's fewest and most are rulec's (`Rules::output_values`), and an input
//!   from the rule's vectors that comes to the offending amount is the example.
//! - [`refusals_met`] (X4): the refusals chobo's search finds for the transfer with its amounts
//!   held to the output's range (`Books::refusals`) against the errors the task handles: one it can
//!   come to and the task does not handle is an error; one the task handles and the search finds
//!   no run for is a warning, since the search goes only as deep as chobo's check does.
//! - [`date_fits`] (X6): the dates a flow gives a koyomi date are inside the range of its input and
//!   of the days its calendar has data for (`Dates::facts`).

use ritsu_base::text::Text;
use ritsu_base::tr;
use ritsu_ports::{Answer, DateFacts, DaySet, Found, OutputValues, Values};

/// A day number as a date, `YYYY-MM-DD` (days since 1970-01-01).
pub fn day_text(d: i64) -> String {
    // Howard Hinnant's civil_from_days, as rulec and koyomi count days
    let z = d + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let dd = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{m:02}-{dd:02}", if m <= 2 { y + 1 } else { y })
}

/// X3 (a): whether every day of `days` lies inside `range` (both ends included where given). The
/// example is the first day outside.
pub fn days_fit(days: &Found<DaySet>, range: (Option<i64>, Option<i64>)) -> Answer<i64> {
    let set = match days {
        Found::Value(s) => s,
        Found::Undecided(why) => return Answer::Undecided(why.clone()),
    };
    match set.iter().copied().find(|d| range.0.is_some_and(|lo| *d < lo) || range.1.is_some_and(|hi| *d > hi)) {
        Some(d) => Answer::Fails(d),
        None => Answer::Holds,
    }
}

/// The largest amount chobo takes (chobo's DESIGN 1.5): an amount is a whole number from 1 to
/// 2⁶³ − 1 in the unit's smallest step.
pub const MAX_AMOUNT: i128 = i64::MAX as i128;

/// X4: whether every value a rule's output comes to is an amount chobo takes. The example is the
/// offending amount with an input that comes to it, when the vectors have one.
pub fn amount_fits(out: &Found<OutputValues>) -> Answer<(i128, Option<Values>)> {
    let o = match out {
        Found::Value(o) => o,
        Found::Undecided(why) => return Answer::Undecided(why.clone()),
    };
    let (Some(lo), Some(hi)) = (o.min, o.max) else {
        return Answer::Undecided(tr!("出力の範囲に端が無いので、chobo が受け取る額に収まるとは言えません", "the output's range has an open end, so it cannot be shown to stay an amount chobo takes"));
    };
    let bad = if lo < 1 {
        Some(o.values.as_ref().and_then(|vs| vs.iter().copied().find(|v| *v < 1)).unwrap_or(lo))
    } else if hi > MAX_AMOUNT {
        Some(hi)
    } else {
        None
    };
    match bad {
        None => Answer::Holds,
        Some(v) => Answer::Fails((v, o.examples.iter().find(|(x, _)| *x == v).map(|(_, inputs)| inputs.clone()))),
    }
}

/// What [`refusals_met`] found.
#[derive(Clone, Debug, PartialEq)]
pub struct Unmet {
    /// Refusals the operation can come to that the task does not handle.
    pub unhandled: Vec<String>,
    /// Errors the task handles that the search found no run for.
    pub unfound: Vec<String>,
}

/// X4: the refusals chobo's search finds for the operation `op` against the errors the task
/// handles. Holds when they are the same; fails with what is missing on either side.
pub fn refusals_met(found: &Found<Vec<(String, Vec<String>)>>, op: &str, handled: &[String]) -> Answer<Unmet> {
    let ops = match found {
        Found::Value(v) => v,
        Found::Undecided(why) => return Answer::Undecided(why.clone()),
    };
    let Some((_, reasons)) = ops.iter().find(|(o, _)| o == op) else {
        return Answer::Undecided(tr!("振替に操作 `{op}` がありません", "the transfer has no operation `{op}`"));
    };
    let unhandled: Vec<String> = reasons.iter().filter(|r| !handled.contains(r)).cloned().collect();
    let unfound: Vec<String> = handled.iter().filter(|h| !reasons.contains(h)).cloned().collect();
    if unhandled.is_empty() && unfound.is_empty() { Answer::Holds } else { Answer::Fails(Unmet { unhandled, unfound }) }
}

/// X6: whether the dates a flow gives the koyomi date `function` lie inside the range of each
/// input it reads and of the days its calendar has data for. `given` is the range of the dates
/// given, by input; an input given no range is undecided. The example is the input, the date
/// outside, and what it is outside of.
pub fn date_fits(facts: &DateFacts, function: &str, given: &[(String, Option<(i64, i64)>)]) -> Answer<(String, i64, Text)> {
    let Some(f) = facts.functions.iter().find(|x| x.name == function) else {
        return Answer::Undecided(tr!("日付 `{function}` がありません", "there is no date `{function}`"));
    };
    for p in &f.params {
        let Some(input) = facts.inputs.iter().find(|i| i.name == *p) else { continue };
        let Some((_, range)) = given.iter().find(|(n, _)| n == p) else {
            return Answer::Undecided(tr!("入力 `{p}` が渡されていません", "the input `{p}` is not given"));
        };
        let Some((lo, hi)) = range else {
            return Answer::Undecided(tr!("`{p}` に渡す日付の範囲が分かりません", "nothing says what range the date given to `{p}` is in"));
        };
        if *lo < input.min {
            return Answer::Fails((p.clone(), *lo, tr!("入力 {p} の範囲（{}〜{}）", "the range of the input {p} ({} to {})", day_text(input.min), day_text(input.max))));
        }
        if *hi > input.max {
            return Answer::Fails((p.clone(), *hi, tr!("入力 {p} の範囲（{}〜{}）", "the range of the input {p} ({} to {})", day_text(input.min), day_text(input.max))));
        }
        if let Some(cal) = &facts.calendar {
            if *lo < cal.data.0 || *hi > cal.data.1 {
                let at = if *lo < cal.data.0 { *lo } else { *hi };
                return Answer::Fails((p.clone(), at, tr!("カレンダー {} のデータの範囲（{}〜{}）", "the days the calendar {} has data for ({} to {})", cal.name, day_text(cal.data.0), day_text(cal.data.1))));
            }
        }
    }
    Answer::Holds
}
