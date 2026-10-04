//! The decisions of the borders a workflow crosses into koyomi and chobo (DESIGN 7.5 (a), 7.6–7.8:
//! X3 (a), X4, X5, X6), each over what the languages hand over through their ports. The checks
//! that reach them from a project (`dates`, `transfers`, `holds`) only gather the facts: where a
//! value can come from, as dandori says (`Flows::crossings`), and what koyomi, chobo and rulec say
//! of it. What each decides, and how, is here, so that a model can hold to it (Lean's RitsuCross).
//!
//! - [`days_given`] (X3 (a), X6): whether every day a value can be lies inside a range. The value
//!   can be the day of koyomi dates, each of whose days koyomi counts exactly (`Dates::values`), or
//!   come from somewhere that says nothing of what day it is (an input, `now`). An example is the
//!   first day outside, of the first koyomi date that has one; with none, a place that says nothing
//!   leaves it undecided. [`days_fit`] is the decision for one set of days.
//! - [`amounts_given`] (X4): whether every amount a value can be is one chobo takes, 1 to 2⁶³ − 1
//!   (an amount outside is a failure of the call, not a refusal: chobo's DESIGN 1.5). The value can
//!   be the output of rules, whose fewest and most rulec counts (`Rules::output_values`, with an
//!   input from the rule's vectors for each value), or a number dandori knows the range of.
//!   [`amount_fits`] is the decision for one output.
//! - [`refusals_met`] (X4): the refusals chobo's search finds for the operation with the amounts
//!   held to their range (`Books::refusals`) against the errors the task handles, both held to the
//!   reasons of the book's bounds (the refusals that turn on the amounts): one it can come to that
//!   the task does not handle is an example; one the task handles that the search finds no run
//!   for is undecided, since the search goes only as deep as chobo's check does.
//! - [`held_until`] (X5): whether a hold is still held when a call comes, from the fewest and the
//!   most seconds the call can come after the hold (as dandori counts them along the flow, with
//!   koyomi's days for a wait until a date) against the seconds it expires after (chobo's
//!   `pending expires after`).

use ritsu_base::text::Text;
use ritsu_base::tr;
use ritsu_ports::{Answer, DateFacts, DaySet, Found, OutputValues, Values};

/// A day number as a date, `YYYY-MM-DD` (days since 1970-01-01, as rulec and koyomi count days).
pub use ritsu_ports::day_text;

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

/// X3 (a) and X6: whether every day a value can be lies inside `range`, by where it can come from:
/// the days of each koyomi date it can be the day of, in order, and the first place it can come
/// from that says nothing of what day it is, when there is one. The example is the first koyomi
/// date with a day outside, by its place in `dates`, and the day; with none, `other`, or a koyomi
/// date koyomi could not count, leaves it undecided.
pub fn days_given(dates: &[Found<DaySet>], other: Option<&Text>, range: (Option<i64>, Option<i64>)) -> Answer<(usize, i64)> {
    let mut undecided = other.cloned();
    for (i, d) in dates.iter().enumerate() {
        match days_fit(d, range) {
            Answer::Fails(day) => return Answer::Fails((i, day)),
            Answer::Undecided(why) => {
                undecided.get_or_insert(why);
            }
            Answer::Holds => {}
        }
    }
    match undecided {
        Some(why) => Answer::Undecided(why),
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

/// Where the amount of an example of [`amounts_given`] comes from: an output of a rule, by its
/// place, with an input of the rule that comes to it when the vectors have one; or a range dandori
/// knows, by its place.
#[derive(Clone, Debug, PartialEq)]
pub enum AmountFrom {
    Output(usize, Option<Values>),
    Range(usize),
}

/// X4: whether every amount a value can be is one chobo takes, by where it can come from: the
/// outputs of rules, each as rulec counts it, in order; the numbers dandori knows the range of; and
/// the first place it can come from that says nothing, when there is one. The example is the first
/// offending amount, of the first output with one, else of the first range with one.
pub fn amounts_given(outputs: &[Found<OutputValues>], ranges: &[(Option<i128>, Option<i128>)], other: Option<&Text>) -> Answer<(i128, AmountFrom)> {
    let mut undecided = other.cloned();
    for (i, o) in outputs.iter().enumerate() {
        match amount_fits(o) {
            Answer::Fails((v, input)) => return Answer::Fails((v, AmountFrom::Output(i, input))),
            Answer::Undecided(why) => {
                undecided.get_or_insert(why);
            }
            Answer::Holds => {}
        }
    }
    for (i, r) in ranges.iter().enumerate() {
        match r {
            (Some(lo), _) if *lo < 1 => return Answer::Fails((*lo, AmountFrom::Range(i))),
            (_, Some(hi)) if *hi > MAX_AMOUNT => return Answer::Fails((*hi, AmountFrom::Range(i))),
            (Some(_), Some(_)) => {}
            _ => {
                undecided.get_or_insert(tr!("範囲に端が無いので、chobo が受け取る額に収まるとは言えません", "a range has an open end, so it cannot be shown to stay an amount chobo takes"));
            }
        }
    }
    match undecided {
        Some(why) => Answer::Undecided(why),
        None => Answer::Holds,
    }
}

/// The fewest and the most of every amount a value can be, as [`amounts_given`] reads them, held
/// to the amounts chobo takes: what chobo's search is held to for the refusals (X4). None when one
/// has an open end or is not counted, or when no amount chobo takes is left.
pub fn amounts_hull(outputs: &[Found<OutputValues>], ranges: &[(Option<i128>, Option<i128>)]) -> Option<(i128, i128)> {
    let mut ends: Vec<(i128, i128)> = Vec::new();
    for o in outputs {
        match o {
            Found::Value(OutputValues { min: Some(lo), max: Some(hi), .. }) => ends.push((*lo, *hi)),
            _ => return None,
        }
    }
    for r in ranges {
        match r {
            (Some(lo), Some(hi)) => ends.push((*lo, *hi)),
            _ => return None,
        }
    }
    let lo = ends.iter().map(|e| e.0).min()?.max(1);
    let hi = ends.iter().map(|e| e.1).max()?.min(MAX_AMOUNT);
    (lo <= hi).then_some((lo, hi))
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
/// handles, both held to `bounds`: the reasons of the book's bounds, the refusals that turn on the
/// amounts (the others — a key used again with other arguments, a key refused before — turn on
/// the calls made before, not on the amounts). Holds when they are the same. The example is a
/// refusal the operation can come to that the task does not handle (with what the task handles
/// that the search does not find, if any). When the task only handles more than the search finds,
/// it is undecided: the search goes only as deep as chobo's check does, so all it shows is that it
/// does not happen within that depth.
pub fn refusals_met(found: &Found<Vec<(String, Vec<String>)>>, op: &str, handled: &[String], bounds: &[String]) -> Answer<Unmet> {
    let ops = match found {
        Found::Value(v) => v,
        Found::Undecided(why) => return Answer::Undecided(why.clone()),
    };
    let Some((_, reasons)) = ops.iter().find(|(o, _)| o == op) else {
        return Answer::Undecided(tr!("振替に操作 `{op}` がありません", "the transfer has no operation `{op}`"));
    };
    let unhandled: Vec<String> = reasons.iter().filter(|r| bounds.contains(r) && !handled.contains(r)).cloned().collect();
    let unfound: Vec<String> = handled.iter().filter(|h| bounds.contains(h) && !reasons.contains(h)).cloned().collect();
    if !unhandled.is_empty() {
        return Answer::Fails(Unmet { unhandled, unfound });
    }
    if unfound.is_empty() {
        return Answer::Holds;
    }
    let list = unfound.iter().map(|r| format!("`{r}`")).collect::<Vec<_>>().join(", ");
    Answer::Undecided(tr!(
        "タスクが処理する {list} で断られる例を、chobo の探索は見つけません。探索は chobo の検査と同じ深さまでしかたどらないので、起きないと言えるのはその深さまでです",
        "chobo's search finds no run that is refused with {list}, which the task handles; the search goes only as deep as chobo's check does, so all it shows is that it does not happen within that depth"
    ))
}

/// X6: the range of the date input of a dates file (both ends in, as day numbers), which every day
/// given to it is held to with [`days_given`]. koyomi's own check holds every input of that range
/// to the days its calendar has data for wherever it asks the calendar (koyomi's E203), so a day
/// inside the range is one koyomi computes, and the range is all there is to hold a day to.
pub fn input_range(facts: &DateFacts, input: &str) -> Option<(i64, i64)> {
    facts.inputs.iter().find(|i| i.name == input && i.kind == ritsu_ports::DateKind::Date).map(|i| (i.min, i.max))
}

/// X5: whether a hold that expires `expiry` seconds after it is made is still held when a call on
/// it comes, `least` to `most` seconds after the hold (`most` is why nothing bounds it, when
/// nothing does). A hold whose expiry the clock reaches has expired, so a call at that very second
/// is refused (chobo's DESIGN 2.5). It holds when the call always comes before (`most < expiry`);
/// the example is the fewest seconds, when the call always comes after (`least >= expiry`); else it
/// is undecided.
pub fn held_until(least: u64, most: &Result<u64, Text>, expiry: u64) -> Answer<u64> {
    if least >= expiry {
        return Answer::Fails(least);
    }
    match most {
        Ok(m) if *m < expiry => Answer::Holds,
        Ok(m) => Answer::Undecided(tr!(
            "呼び出しは仮押さえを作ってから {}から{}のあいだに来ます。仮押さえは作ってから {}で期限が切れます",
            "the call comes {} to {} after the hold is made, and the hold expires {} after it is made",
            ritsu_ports::seconds_text(least).ja, ritsu_ports::seconds_text(*m).ja, ritsu_ports::seconds_text(expiry).ja;
            ritsu_ports::seconds_text(least).en, ritsu_ports::seconds_text(*m).en, ritsu_ports::seconds_text(expiry).en
        )),
        Err(why) => Answer::Undecided(why.clone()),
    }
}
