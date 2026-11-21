//! X2: a rule's preconditions, where a workflow calls it (DESIGN 7.4). rulec writes what the
//! shape of an input cannot say as preconditions (a relation between two inputs, a bound on the
//! total or the length of the list it walks, the days of a koyomi date, the range a value it works
//! out from the inputs is accepted in), and the rule's generated
//! code refuses a call that breaks one at its door. dandori knows the range of every value a call
//! gives (a variable is what the runs that reach the call last put in it: dandori's DESIGN 1.3)
//! and holds the
//! rule's ranges to them itself (its E014); this check holds the preconditions to the same ranges,
//! asking rulec through its port (`Rules::preconditions_hold`), so a call that can break one is
//! found before the workflow runs.
//!
//! Each precondition at each call is one border, and comes to one of three (P5):
//!
//! - shown to hold: nothing is said, and it counts as held;
//! - an example where it does not (E201): the values at the corner of the two ranges that breaks
//!   it, which dandori's ranges say the call can give, as dandori's own E014 reads them (each value
//!   from the places it comes from, taken one at a time);
//! - undecided (W201): a value with a place it comes from that has no range, or a bound on a list
//!   (dandori calls no rule that walks one, E005, and knows no list's length). `ritsu dandori` hands
//!   these to dandori ([`UndecidedCalls`], ritsu's port `Undecided`), and the code it writes for the
//!   workflow checks each when the workflow runs, as soon as the values are made (DESIGN 7.4 item 3;
//!   dandori's DESIGN 1.17).
//!
//! A precondition that a date input takes only the days of a koyomi date (`range from koyomi`) is
//! decided over the days the value given can be (X3 (a), DESIGN 7.5): when it can only be the day
//! of koyomi dates, or a day of a range the `.flow` writes, each of their days must be one of the
//! rule's; the example is the first that is not (of a range with an open low end, the day before
//! the rule's first). A value that can also come from an input or a task's answer without a range,
//! or `now`, says nothing of what day it is, and leaves it undecided, to be checked the same way
//! when the workflow runs. A date's range comes as day numbers, as rulec's port takes a date, so a
//! relation between two dates (`check_in <= check_out`) is decided over them as over numbers.
//!
//! A value given to both inputs of a relation (`x` to `a` and to `b`) is the same value on both
//! sides: `<=` and `>=` hold, `<` and `>` break at any value it takes.

use crate::{numbers, Border, Borders, Came, Check, Side};
use ritsu_base::diag::Diag;
use ritsu_base::naming::Tool;
use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;
use ritsu_ports::{day_text, Answer, CallArg, ColumnType, Dates, Finding, Flows, Found, Origin, Precondition, Rules, UndecidedPrecondition, Value};
use ritsu_units::written;
use ritsu_project::{Joined, Project};
use std::path::Path;
use std::rc::Rc;

/// The preconditions X2 cannot decide at a flow's calls of rules, as ritsu's port `Undecided`
/// answers them: what `ritsu dandori` hands dandori, so that the code it writes for the workflow
/// checks each when the workflow runs (DESIGN 7.4 item 3).
pub struct UndecidedCalls {
    ports: ritsu_ports::Ports,
    flows: Rc<dyn Flows>,
}

impl UndecidedCalls {
    /// The preconditions of the flows' calls, asked of rulec, koyomi, chobo and dandori as `joined`
    /// joins them.
    pub fn new(joined: &Joined) -> UndecidedCalls {
        UndecidedCalls { ports: joined.ports(), flows: joined.dandori.clone() }
    }
}

impl ritsu_ports::Undecided for UndecidedCalls {
    /// Each precondition of each rule the flow at `file` calls that is neither shown to hold nor
    /// broken by an example at the call, decided as `ritsu check` decides it; none for a flow that
    /// does not pass dandori's check, or a rule that does not pass rulec's.
    fn preconditions(&self, file: &Path) -> Vec<UndecidedPrecondition> {
        // read as `ritsu check` reads the flow, with the dates files and the books, so that the days
        // a value can be are known
        let Ok(calls) = self.flows.crossings(file, &self.ports).map(|c| c.rules) else { return vec![] };
        let mut out = Vec::new();
        for call in calls {
            let Ok(facts) = self.ports.rules.facts(&call.rule) else { continue };
            for p in facts.preconditions {
                if let Answer::Undecided(_) = decide(self.ports.rules.as_ref(), self.ports.dates.as_ref(), &call.rule, &p, &call.args) {
                    out.push(UndecidedPrecondition { line: call.line, rule: call.rule.clone(), precondition: p });
                }
            }
        }
        out
    }
}

/// A value on the wire as the flow would write it out, unit aside: what a value the call writes is
/// compared with. A sentence says a value with [`numbers::value`].
fn shown(v: &Value) -> String {
    match v {
        Value::Int(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Str(s) => format!("\"{s}\""),
        Value::Date(d) => d.clone(),
        Value::Enum(e) => e.clone(),
        Value::None => "none".into(),
        Value::List(xs) => format!("[{} elements]", xs.len()),
    }
}

/// Whether the input of the rule is a date (a date that may be none too), whose range comes as day
/// numbers.
fn is_date(rules: &dyn Rules, rule: &Path, input: &str) -> bool {
    rules.facts(rule).is_ok_and(|f| f.inputs.iter().any(|c| c.name == input && c.ty.is_date()))
}

/// A precondition that a date input takes only the days of a koyomi date, at one call: the value
/// given can be the day of koyomi dates, or a day of a range (`from`), each of whose days must be
/// one of `days`.
fn days_kept(dates: &dyn Dates, a: &CallArg, days: &ritsu_ports::DaySet, file: &str, date: &str) -> Answer<Text> {
    days_kept_at(dates, a, days, file, date).0
}

/// [`days_kept`], with the example's day and the place in `from` it comes from, where it fails.
pub(crate) fn days_kept_at(dates: &dyn Dates, a: &CallArg, days: &ritsu_ports::DaySet, file: &str, date: &str) -> (Answer<Text>, Option<(usize, i64)>) {
    let shown = &a.shown;
    let mut other: Option<Text> = None;
    if a.from.is_empty() {
        other = Some(tr!("`{shown}` が何日になるかは分かりません", "nothing says what day `{shown}` is"));
    }
    for (i, o) in a.from.iter().enumerate() {
        match o {
            Origin::Day { file: from, date: d } => match dates.values(from, d) {
                Ok(Found::Value(set)) => {
                    if let Some(at) = set.iter().copied().find(|x| !days.contains(x)) {
                        let day = day_text(at);
                        return (
                            Answer::Fails(tr!(
                                "`{shown}` は koyomi の日付 {d} の日で、{day} になることがありますが、{day} は koyomi \"{file}\" date {date} の日ではありません",
                                "`{shown}` is a day of the koyomi date {d} and can be {day}, which is not a day of koyomi \"{file}\" date {date}"
                            )),
                            Some((i, at)),
                        );
                    }
                }
                Ok(Found::Undecided(why)) => {
                    other.get_or_insert(tr!("koyomi は日付 {d} の日を数えません（{}）", "koyomi does not count the days of the date {d}: {}", why.ja; why.en));
                }
                Err(_) => {
                    other.get_or_insert(tr!("koyomi から日付 {d} の情報を得られません", "koyomi does not answer for the date {d}"));
                }
            },
            // a day of a range the `.flow` writes, any of which can come
            Origin::Range(lo, hi) => {
                let r = (lo.and_then(|x| i64::try_from(x).ok()), hi.and_then(|x| i64::try_from(x).ok()));
                if let Some(at) = crate::borders::day_not_in(days, r) {
                    let (day, range) = (day_text(at), numbers::range((*lo, *hi), true, None));
                    return (
                        Answer::Fails(tr!(
                            "`{shown}` は `{range}` の日で、{day} になることがありますが、{day} は koyomi \"{file}\" date {date} の日ではありません",
                            "`{shown}` is a day of `{range}` and can be {day}, which is not a day of koyomi \"{file}\" date {date}"
                        )),
                        Some((i, at)),
                    );
                }
            }
            Origin::Now => {
                other.get_or_insert(if shown == "now" { tr!("`now` はどの日にもなりえます", "`now` can be any day") } else { tr!("`{shown}` は `now` のことがあり、どの日にもなりえます", "`{shown}` can be `now`, which can be any day") });
            }
            // a rule takes no time
            Origin::Moment { .. } => {
                other.get_or_insert(tr!("`{shown}` は時刻で、渡す先はそれを何日に読むかを言いません", "`{shown}` is a time, and where it is given says no day it reads it as"));
            }
            Origin::Unknown(t) => {
                other.get_or_insert(tr!("`{shown}` が何日になるかは分かりません（{}）", "nothing says what day `{shown}` is ({} gives no range of days)", crate::then_ja(&t.ja, "には日付の範囲がありません"); t.en));
            }
            Origin::Output { .. } => {
                other.get_or_insert(tr!("`{shown}` は koyomi の日付でないところから来ることがあります", "`{shown}` can come from somewhere other than a koyomi date"));
            }
        }
    }
    match other {
        Some(why) => (Answer::Undecided(why), None),
        None => (Answer::Holds, None),
    }
}

/// One precondition at one call, decided. kaname's calls are decided with it too, their values
/// handed over as a flow's (`programs`).
pub(crate) fn decide(rules: &dyn Rules, dates: &dyn Dates, rule: &std::path::Path, p: &Precondition, args: &[CallArg]) -> Answer<Text> {
    let arg = |n: &str| args.iter().find(|a| a.input == n);
    // the rule's units, which a sentence says each value in
    let facts = rules.facts(rule).ok();
    let unit = |n: &str| facts.as_ref().and_then(|f| numbers::input_unit(f, n));
    match p {
        Precondition::Relation { left, op, right } => {
            let (Some(l), Some(r)) = (arg(left), arg(right)) else {
                return Answer::Undecided(tr!("呼び出しが `{left}` か `{right}` を渡していません", "the call does not give `{left}` or `{right}`"));
            };
            let (ld, rd) = (is_date(rules, rule, left), is_date(rules, rule, right));
            // the same value on both sides
            if l.shown == r.shown {
                return if op.contains('=') {
                    Answer::Holds
                } else {
                    let one = |v: i128| if ld { i64::try_from(v).map(day_text).unwrap_or_else(|_| written::digits(v)) } else { written::value(v, unit(left).as_ref()) };
                    let at = l.range.and_then(|r| r.0.or(r.1)).map(one).unwrap_or_else(|| "…".into());
                    Answer::Fails(tr!(
                        "`{}` を両方に渡しているので、`{left} {op} {right}` はどの値でも成り立ちません（たとえば {left} = {right} = {at}）",
                        "`{}` is given to both, so `{left} {op} {right}` holds for no value (for one, {left} = {right} = {at})",
                        l.shown
                    ))
                };
            }
            for a in [l, r] {
                if a.range.is_none() {
                    let from = a.unknown.clone().unwrap_or_else(|| tr!("範囲の無いところ", "a place with no range"));
                    return Answer::Undecided(tr!(
                        "`{}` の範囲が分かりません（{}）",
                        "nothing says what range `{}` is in ({} has no range)",
                        a.shown, crate::then_ja(&from.ja, "に範囲がありません"); a.shown, from.en
                    ));
                }
            }
            let (lr, rr) = (l.range.unwrap_or((None, None)), r.range.unwrap_or((None, None)));
            let ranges = [(left.clone(), lr.0, lr.1), (right.clone(), rr.0, rr.1)];
            match rules.preconditions_hold(rule, &ranges, None, None) {
                Err(_) => Answer::Undecided(tr!("rulec からこの規則の情報を得られません", "rulec does not answer for this rule")),
                Ok(answers) => match answers.into_iter().find(|(q, _)| q == p).map(|(_, a)| a) {
                    Some(Answer::Holds) => Answer::Holds,
                    Some(Answer::Fails(ex)) => {
                        let at = numbers::inputs(facts.as_ref(), &ex);
                        Answer::Fails(tr!(
                            "`{}` は `{}`、`{}` は `{}` で、{at} のとき `{left} {op} {right}` が成り立ちません",
                            "`{}` is `{}` and `{}` is `{}`, and at {at} `{left} {op} {right}` does not hold",
                            l.shown,
                            numbers::range(lr, ld, unit(left).as_ref()),
                            r.shown,
                            numbers::range(rr, rd, unit(right).as_ref())
                        ))
                    }
                    Some(Answer::Undecided(why)) => Answer::Undecided(why),
                    None => Answer::Undecided(tr!("rulec から、この前提が成り立つかどうかの結果を得られません", "rulec does not answer for this precondition")),
                },
            }
        }
        // A side that adds inputs up (rulec's §15.216): every input the call gives has a range,
        // and rulec decides the sum of them over those ranges.
        Precondition::SumRelation { left, op, right } => {
            let said = format!("{} {op} {}", left.join(" + "), right.join(" + "));
            let mut given: Vec<&CallArg> = Vec::new();
            for n in left.iter().chain(right.iter()) {
                let Some(a) = arg(n) else {
                    return Answer::Undecided(tr!("呼び出しが `{n}` を渡していません", "the call does not give `{n}`"));
                };
                if a.range.is_none() {
                    let from = a.unknown.clone().unwrap_or_else(|| tr!("範囲の無いところ", "a place with no range"));
                    return Answer::Undecided(tr!(
                        "`{}` の範囲が分かりません（{}）",
                        "nothing says what range `{}` is in ({} has no range)",
                        a.shown, crate::then_ja(&from.ja, "に範囲がありません"); a.shown, from.en
                    ));
                }
                given.push(a);
            }
            let ranges: Vec<(String, Option<i128>, Option<i128>)> = left
                .iter()
                .chain(right.iter())
                .zip(&given)
                .map(|(n, a)| {
                    let r = a.range.unwrap_or((None, None));
                    (n.clone(), r.0, r.1)
                })
                .collect();
            match rules.preconditions_hold(rule, &ranges, None, None) {
                Err(_) => Answer::Undecided(tr!("rulec からこの規則の情報を得られません", "rulec does not answer for this rule")),
                Ok(answers) => match answers.into_iter().find(|(q, _)| q == p).map(|(_, a)| a) {
                    Some(Answer::Holds) => Answer::Holds,
                    Some(Answer::Fails(ex)) => {
                        let at = numbers::inputs(facts.as_ref(), &ex);
                        let each = |a: &&CallArg| numbers::range(a.range.unwrap_or((None, None)), false, unit(&a.input).as_ref());
                        let each_ja: Vec<String> = given.iter().map(|a| format!("`{}` は `{}`", a.shown, each(a))).collect();
                        let each_en: Vec<String> = given.iter().map(|a| format!("`{}` is `{}`", a.shown, each(a))).collect();
                        Answer::Fails(tr!(
                            "{} で、{} のとき `{}` が成り立ちません",
                            "{}, and at {} `{}` does not hold",
                            each_ja.join("、"), at, said; each_en.join(", "), at, said
                        ))
                    }
                    Some(Answer::Undecided(why)) => Answer::Undecided(why),
                    None => Answer::Undecided(tr!("rulec から、この前提が成り立つかどうかの結果を得られません", "rulec does not answer for this precondition")),
                },
            }
        }
        Precondition::Sum { name, over, min: None, .. } => Answer::Undecided(tr!(
            "`{name}` は並び `{over}` の合計の上限ですが、dandori は並びの長さを知りません",
            "`{name}` bounds a total over the list `{over}`, and dandori knows no list's length"
        )),
        Precondition::Sum { name, over, min: Some(_), .. } => Answer::Undecided(tr!(
            "`{name}` は並び `{over}` の合計の上限と下限ですが、dandori は並びの長さを知りません",
            "`{name}` bounds a total over the list `{over}` from above and from below, and dandori knows no list's length"
        )),
        Precondition::Count { name, over, .. } => Answer::Undecided(tr!(
            "`{name}` は並び `{over}` を数えた数の下限ですが、dandori は並びの中身を知りません",
            "`{name}` bounds from below how many elements of the list `{over}` are counted, and dandori knows nothing of what a list holds"
        )),
        Precondition::Length { sequence, .. } => Answer::Undecided(tr!(
            "並び `{sequence}` の長さの上限ですが、dandori は並びの長さを知りません",
            "it bounds the length of the list `{sequence}`, and dandori knows no list's length"
        )),
        Precondition::Unique { sequence, field } => Answer::Undecided(tr!(
            "並び `{sequence}` の要素どうしで `{field}` が同じにならないという前提ですが、dandori は並びの中身を知りません",
            "it asks that no two elements of the list `{sequence}` have the same `{field}`, and dandori knows nothing of what a list holds"
        )),
        Precondition::Days { input, file, date, days } => match arg(input) {
            Some(a) => days_kept(dates, a, days, file, date),
            None => Answer::Undecided(tr!("呼び出しが `{input}` を渡していません", "the call does not give `{input}`")),
        },
        // A `never` line (rulec's §15.282): rulec decides it over the ranges of the numbers the
        // call gives, and over the values of the truth values and the enums it gives, as far as
        // the flow says them (a value written out, the values the arms of a `match` let through
        // to the call, the values a `range` lets in; a variable, by what the runs that reach the
        // call last put in it). A condition none of them meets keeps the
        // line; with every condition met — a number inside a closed range, a truth value or an
        // enum by every value it can be — the call breaks it. A truth value or an enum that can
        // meet its condition or not leaves the line undecided: nothing says which value comes.
        Precondition::Never { said: line, conds } => {
            let mut ranges = Vec::new();
            let mut values = Vec::new();
            let mut open = None;
            for k in conds {
                let Some(a) = arg(&k.name) else {
                    let name = &k.name;
                    return Answer::Undecided(tr!("呼び出しが `{name}` を渡していません", "the call does not give `{name}`"));
                };
                match (&a.values, a.range) {
                    (Some(vs), _) => values.push((k.name.clone(), vs.clone())),
                    (None, Some((lo, hi))) => ranges.push((k.name.clone(), lo, hi)),
                    (None, None) => open = open.or(Some(a.shown.clone())),
                }
            }
            match rules.preconditions_hold_given(rule, &ranges, &values, None, None) {
                Err(_) => Answer::Undecided(tr!("rulec からこの規則の情報を得られません", "rulec does not answer for this rule")),
                Ok(answers) => match answers.into_iter().find(|(q, _)| q == p).map(|(_, a)| a) {
                    Some(Answer::Holds) => Answer::Holds,
                    Some(Answer::Fails(_)) if open.is_some() => {
                        let shown = open.unwrap_or_default();
                        Answer::Undecided(tr!("`{shown}` の範囲が分かりません", "nothing says what range `{shown}` is in"))
                    }
                    Some(Answer::Fails(ex)) => {
                        let at = numbers::inputs(facts.as_ref(), &ex);
                        Answer::Fails(tr!("{at} のとき、`{line}` の組み合わせになります", "at {at} the values are the combination of `{line}`"))
                    }
                    Some(Answer::Undecided(why)) => Answer::Undecided(why),
                    None => Answer::Undecided(tr!("rulec から、この前提が成り立つかどうかの結果を得られません", "rulec does not answer for this precondition")),
                },
            }
        }
        // A value the rule works out and accepts only inside a range (rulec's §15.313): rulec
        // decides what it reaches over the ranges of the inputs the call gives. A field of an
        // element it is worked out from (on each element, or through a total) is no input of the
        // call, and keeps the rule's own range.
        Precondition::Define { name, range, from, .. } => {
            let inputs: Vec<String> = facts.as_ref().map(|f| f.inputs.iter().map(|c| c.name.clone()).collect()).unwrap_or_default();
            let mut ranges = Vec::new();
            let mut given: Vec<&CallArg> = Vec::new();
            for n in from.iter().filter(|n| inputs.contains(n)) {
                let Some(a) = arg(n) else {
                    return Answer::Undecided(tr!("呼び出しが `{n}` を渡していません", "the call does not give `{n}`"));
                };
                let Some((lo, hi)) = a.range else {
                    let from = a.unknown.clone().unwrap_or_else(|| tr!("範囲の無いところ", "a place with no range"));
                    return Answer::Undecided(tr!(
                        "`{}` の範囲が分かりません（{}）",
                        "nothing says what range `{}` is in ({} has no range)",
                        a.shown, crate::then_ja(&from.ja, "に範囲がありません"); a.shown, from.en
                    ));
                };
                ranges.push((n.clone(), lo, hi));
                given.push(a);
            }
            match rules.preconditions_hold(rule, &ranges, None, None) {
                Err(_) => Answer::Undecided(tr!("rulec からこの規則の情報を得られません", "rulec does not answer for this rule")),
                Ok(answers) => match answers.into_iter().find(|(q, _)| q == p).map(|(_, a)| a) {
                    Some(Answer::Holds) => Answer::Holds,
                    Some(Answer::Fails(ex)) => {
                        let at = numbers::inputs(facts.as_ref(), &ex);
                        let each = |a: &&CallArg| (a.shown.clone(), numbers::range(a.range.unwrap_or((None, None)), is_date(rules, rule, &a.input), unit(&a.input).as_ref()));
                        let each_ja: Vec<String> = given.iter().map(each).map(|(s, r)| format!("`{s}` は `{r}`")).collect();
                        let each_en: Vec<String> = given.iter().map(each).map(|(s, r)| format!("`{s}` is `{r}`")).collect();
                        let past = tr!(
                            "{at} のとき、規則が計算する `{name}` が受け付ける範囲 `{range}` の外になります",
                            "at {at} the `{name}` the rule works out falls outside the range it accepts, `{range}`"
                        );
                        Answer::Fails(if given.is_empty() {
                            past
                        } else {
                            tr!("{} で、{}", "{}, and {}", each_ja.join("、"), past.ja; each_en.join(", "), past.en)
                        })
                    }
                    Some(Answer::Undecided(why)) => Answer::Undecided(why),
                    None => Answer::Undecided(tr!("rulec から、この前提が成り立つかどうかの結果を得られません", "rulec does not answer for this precondition")),
                },
            }
        }
    }
}

/// What a call breaks a `never` line with where it gives one value that meets a condition of it:
/// one it writes out (`tool_check(has_tool: true, all_parts: true)`), or a variable that is one
/// value on the runs that reach the call (`let parts = true` before `all_parts: parts`). The note
/// says those values and what to give instead, each value by the values that meet no condition of
/// the line (so the line holds, whatever the other values are), and whether every condition is met
/// by one such value. None where no condition is: what the arms of a `match` or a range leave is
/// what breaks it.
fn written_never(facts: &ritsu_ports::RuleFacts, line: &str, conds: &[ritsu_ports::NeverCond], args: &[CallArg]) -> Option<(Text, bool)> {
    use ritsu_ports::NeverTest;
    let mut writes: Vec<Text> = Vec::new();
    let mut instead: Vec<Text> = Vec::new();
    let mut all = true;
    for k in conds {
        let a = args.iter().find(|a| a.input == k.name)?;
        // one value: a truth value, a value of an enum or a number, written out or held
        let one = match (&a.values, a.range) {
            (Some(vs), _) if vs.len() == 1 => Some(vs[0].clone()),
            (None, Some((Some(lo), Some(hi)))) if lo == hi => Some(Value::Int(lo)),
            _ => None,
        };
        let Some(v) = one.filter(|v| k.test.holds(v) == Some(true)) else {
            all = false;
            continue;
        };
        let name = &a.input;
        let unit = numbers::input_unit(facts, name);
        let written = shown(&v) == a.shown || (matches!(v, Value::Int(_)) && a.shown.trim_start_matches('-').starts_with(|c: char| c.is_ascii_digit()));
        writes.push(if written {
            Text::same(format!("`{name}: {}`", a.shown))
        } else {
            let (s, x) = (&a.shown, numbers::value(&v, unit.as_ref()));
            tr!("`{name}: {s}`（ここでは `{x}`）", "`{name}: {s}` (`{x}` here)")
        });
        // what meets no condition of the test
        let other = match &k.test {
            NeverTest::Among { values, yes: true } => {
                let every: Vec<Value> = match &v {
                    Value::Bool(_) => vec![Value::Bool(false), Value::Bool(true)],
                    Value::Enum(_) => match facts.inputs.iter().find(|c| c.name == *name).map(|c| &c.ty) {
                        Some(ColumnType::Enum { name: e, range }) => facts.enums.iter().find(|x| x.name == *e).map(|x| x.values.iter().filter(|n| range.as_ref().is_none_or(|r| r.contains(&n.name))).map(|n| Value::Enum(n.name.clone())).collect()).unwrap_or_default(),
                        _ => Vec::new(),
                    },
                    _ => Vec::new(),
                };
                let left: Vec<String> = every.iter().filter(|x| !values.contains(x)).map(|x| format!("`{}`", numbers::value(x, unit.as_ref()))).collect();
                (!left.is_empty()).then(|| {
                    let (ja, en) = (left.join("・"), or_en(&left));
                    tr!("`{name}` に {ja}", "`{name}` given {en}")
                })
            }
            NeverTest::Among { values, yes: false } => (!values.is_empty()).then(|| {
                let left: Vec<String> = values.iter().map(|x| format!("`{}`", numbers::value(x, unit.as_ref()))).collect();
                let (ja, en) = (left.join("・"), or_en(&left));
                tr!("`{name}` に {ja}", "`{name}` given {en}")
            }),
            NeverTest::Compare(ops) => (!ops.is_empty()).then(|| {
                let flipped: Vec<String> = ops
                    .iter()
                    .map(|(op, n)| {
                        let back = match op.as_str() {
                            "<=" => ">",
                            "<" => ">=",
                            ">=" => "<",
                            _ => "<=",
                        };
                        format!("`{back} {}`", written::value(*n, unit.as_ref()))
                    })
                    .collect();
                let (ja, en) = (flipped.join(" か "), or_en(&flipped));
                tr!("`{name}` に {ja} の値", "`{name}` given a value {en}")
            }),
            NeverTest::Other => None,
        };
        instead.extend(other);
    }
    if writes.is_empty() {
        return None;
    }
    let w = Text::list(&writes);
    let note = if instead.is_empty() {
        tr!(
            "呼び出しが渡す {}、`{line}` の組み合わせに当たります。",
            "The values the call gives, {}, are the combination `{line}` refuses.",
            crate::then_ja(&w.ja, "が"); w.en
        )
    } else {
        // the Japanese with a space after a value in backticks, as the other notes write it
        let ja = instead.iter().enumerate().map(|(k, t)| if k + 1 < instead.len() { crate::then_ja(&t.ja, "か、") } else { t.ja.clone() }).collect::<String>();
        let i = Text { ja, en: Text::join(&instead, "か、", ", or ").en };
        tr!(
            "呼び出しが渡す {}、`{line}` の組み合わせに当たります。{}渡せば、この行は保たれます。",
            "The values the call gives, {}, are the combination `{line}` refuses. With {}, the line holds.",
            crate::then_ja(&w.ja, "が"), crate::then_ja(&i.ja, "を"); w.en, i.en
        )
    };
    Some((note, all))
}

/// Words joined by `or`, as English says a choice: `a`, `a or b`, `a, b or c`.
fn or_en(words: &[String]) -> String {
    match words.len() {
        0 => String::new(),
        1 => words[0].clone(),
        n => format!("{} or {}", words[..n - 1].join(", "), words[n - 1]),
    }
}

/// A precondition as the record of its border keeps it (`about`): a bound on a list with the whole
/// number on the wire (`amount <= 100000000`). A sentence says it with [`said_in`].
pub(crate) fn said(p: &Precondition) -> String {
    match p {
        Precondition::Relation { left, op, right } => format!("{left} {op} {right}"),
        Precondition::SumRelation { left, op, right } => format!("{} {op} {}", left.join(" + "), right.join(" + ")),
        Precondition::Sum { name, max, min: None, .. } => format!("{name} <= {max}"),
        Precondition::Sum { name, max, min: Some(min), .. } => format!("{min} <= {name} <= {max}"),
        Precondition::Count { name, min, .. } => format!("{name} >= {min}"),
        Precondition::Length { sequence, max } => format!("len({sequence}) <= {max}"),
        Precondition::Unique { sequence, field } => format!("{sequence}.{field} unique"),
        Precondition::Days { input, file, date, .. } => format!("{input} in koyomi \"{file}\" date {date}"),
        Precondition::Never { said, .. } => said.clone(),
        Precondition::Define { name, range, .. } => format!("{name} range {range}"),
    }
}

/// A precondition as a sentence says it: a bound on a total in the total's unit, which the
/// precondition carries (`amount <= 100_000_000JPY`, rulec's §15.333), a count and a length in
/// digits (DESIGN 6.2 item 8). A total of a `define` on each element is said in its unit too.
pub(crate) fn said_in(p: &Precondition) -> String {
    match p {
        Precondition::Sum { name, max, min, unit, .. } => {
            let top = total_end(*max, unit.as_ref());
            match min {
                None => format!("{name} <= {top}"),
                Some(min) => format!("{} <= {name} <= {top}", total_end(*min, unit.as_ref())),
            }
        }
        Precondition::Count { name, min, .. } => format!("{name} >= {}", written::digits(*min)),
        Precondition::Length { sequence, max } => format!("len({sequence}) <= {}", written::digits(*max)),
        _ => said(p),
    }
}

/// One end of a total on the wire, in the total's unit. Its integer counts the unit's step where
/// the total is held finer than a whole unit (`Unit::step`: a total of a value worked out on each
/// element that keeps its fractions), and the end is then said as the value it stands for, as the
/// rule writes one (`1250.5JPY` for 125050 hundredths, `100_000JPY` for 10000000); a rate's
/// integer counts the rate's step, as `written::value` reads it.
fn total_end(n: i128, unit: Option<&ritsu_units::Unit>) -> String {
    use ritsu_units::{Dim, Rat};
    let Some(u) = unit else { return written::digits(n) };
    let Some(step) = u.step.filter(|s| *s != Rat::int(1) && !matches!(u.dim, Dim::Rate)) else { return written::value(n, unit) };
    let Some(v) = Rat::int(n).checked_mul(step) else { return written::digits(n) };
    // the value as `Rat` writes it, its whole part grouped as `written::digits` groups one
    let t = v.to_string();
    let (sign, body) = t.strip_prefix('-').map_or(("", t.as_str()), |b| ("-", b));
    let (whole, part) = body.split_once('.').map_or((body, None), |(w, p)| (w, Some(p)));
    let said = match (whole.parse::<i128>(), part) {
        (Ok(w), Some(p)) => format!("{sign}{}.{p}", written::digits(w)),
        (Ok(w), None) => format!("{sign}{}", written::digits(w)),
        _ => t.clone(),
    };
    match u.dim {
        Dim::Number | Dim::Count(_) => said,
        _ => format!("{said}{}", u.unit),
    }
}

/// X2 over every flow of the project that passes dandori's check: every precondition of every
/// rule each flow calls, decided at the call. A flow dandori does not pass, or a rule rulec does
/// not, is the language's to say, and is passed over here.
pub fn check(project: &Project, joined: &Joined, lang: Lang, borders: &mut Borders) -> Vec<Finding> {
    let mut out = Vec::new();
    for f in project.of(Tool::Dandori) {
        let disk = ritsu_base::paths::on_disk(&project.root, &f.rel);
        // read with the dates files and the books too, so that a flow that uses them is checked, and
        // the days a value can be are known
        let Ok(calls) = joined.dandori.crossings(&disk, &joined.ports()).map(|c| c.rules) else { continue };
        let src = ritsu_base::fs::read_to_string(&disk).unwrap_or_default();
        for call in calls {
            let Ok(facts) = joined.rulec.facts(&call.rule) else { continue };
            for p in &facts.preconditions {
                // the record keeps the whole numbers on the wire, and a sentence writes them
                let (about, pre) = (said(p), said_in(p));
                let rule = &call.name;
                let answer = decide(joined.rulec.as_ref(), joined.koyomi.as_ref(), &call.rule, p, &call.args);
                let code = match &answer {
                    Answer::Holds => None,
                    Answer::Fails(_) => Some("E201"),
                    Answer::Undecided(_) => Some("W201"),
                };
                let (at, to) = (Side::at(Tool::Dandori, &f.rel, call.line), Side::reached(project, Tool::Rulec, &call.rule, None));
                let mut border = Border::new(Check::Precondition, about, at, Some(to), Came::of(&answer), code);
                // the days of a koyomi date a rule's input takes: where the value can come from, as at the other borders of days
                if let Precondition::Days { input, file, date, days } = p {
                    if let Some(a) = call.args.iter().find(|a| a.input == *input) {
                        let (_, example) = days_kept_at(joined.koyomi.as_ref(), a, days, file, date);
                        border.detail = crate::dates::kept_detail(project, joined, a, days, &answer, example, lang);
                    }
                }
                borders.add(border);
                match answer {
                    Answer::Holds => {}
                    Answer::Fails(why) => {
                        let d: Diag = Diag::at("E201", &f.shown, call.line, 1, tr!(
                            "規則 {rule} を呼ぶところで、前提 `{pre}` を破る値を渡すことがあります",
                            "The call of the rule {rule} can give it values that break its precondition `{pre}`"
                        ))
                        .source(&src)
                        .rel(&f.rel)
                        .note(why);
                        let d = if let Precondition::Days { file, date, .. } = p {
                            d.note(tr!(
                                "規則から生成したコードは、前提を破る呼び出しを入口で受け付けません。この呼び出しは、ワークフローを走らせたときに初めて落ちます。koyomi は、入力の範囲のすべてで日付の日を数えています。",
                                "The rule's generated code refuses a call that breaks a precondition at its door, so this call fails only when the workflow runs. koyomi counts the days of a date over the whole range of its inputs."
                            ))
                            .note(tr!(
                                "koyomi \"{file}\" date {date} の日を渡すか、規則の範囲を直してください。",
                                "Give it the days of koyomi \"{file}\" date {date}, or correct the rule's range."
                            ))
                        } else {
                            // A value the rule works out is refused where it is worked out, not at
                            // the door (rulec's §15.313).
                            let refused = if matches!(p, Precondition::Define { .. }) {
                                tr!(
                                    "規則から生成したコードは、計算した値が範囲の外になる呼び出しを、その値を計算したところで受け付けません。この呼び出しは、ワークフローを走らせたときに初めて落ちます。値の範囲は、dandori が、この呼び出しに届く実行が最後に入れた値から集めたものです。",
                                    "Where the rule's generated code works the value out, it refuses a call whose value falls outside the range, so this call fails only when the workflow runs. The ranges are dandori's, from what the runs that reach this call last put in the values."
                                )
                            } else {
                                tr!(
                                    "規則から生成したコードは、前提を破る呼び出しを入口で受け付けません。この呼び出しは、ワークフローを走らせたときに初めて落ちます。値の範囲は、dandori が、この呼び出しに届く実行が最後に入れた値から集めたものです。",
                                    "The rule's generated code refuses a call that breaks a precondition at its door, so this call fails only when the workflow runs. The ranges are dandori's, from what the runs that reach this call last put in the values."
                                )
                            };
                            let d = d.note(refused);
                            // a `never` line met by values the call writes: those are what to change
                            let written = match p {
                                Precondition::Never { said: line, conds } => written_never(&facts, line, conds, &call.args),
                                _ => None,
                            };
                            let branch = tr!(
                                "値を渡す前に前提を保つよう分岐するか、範囲を狭めてください（入力やタスクの結果の `range`）。",
                                "Branch so that the precondition holds before the call, or narrow the ranges (the `range` of an input or a task's result)."
                            );
                            match written {
                                Some((note, true)) => d.note(note),
                                Some((note, false)) => d.note(note).note(branch),
                                None => d.note(branch),
                            }
                        };
                        out.push(Finding::of(&d, Some(f.rel.clone()), lang));
                    }
                    Answer::Undecided(why) => {
                        let d: Diag = Diag::at("W201", &f.shown, call.line, 1, tr!(
                            "規則 {rule} を呼ぶところで、前提 `{pre}` が保たれるかを決められません",
                            "Whether the call of the rule {rule} keeps its precondition `{pre}` cannot be decided"
                        ))
                        .source(&src)
                        .rel(&f.rel)
                        .note(why)
                        .note(if matches!(p, Precondition::Define { .. }) {
                            // the flow does not work out what the rule works out (dandori's prechecks)
                            tr!(
                                "ワークフローのコードは、規則が計算する値までは計算しません。計算した値が範囲の外になる呼び出しは、規則から生成したコードが、その値を計算したところで受け付けません。",
                                "The workflow's code does not work out what the rule works out: where the rule's generated code works the value out, it refuses a call whose value falls outside the range."
                            )
                        } else {
                            tr!(
                                "`ritsu dandori gen` が書くワークフローのコードも `ritsu run` も、値ができたところですぐに確かめます。破る実行は、そこで `Dandori.BrokenPrecondition` で失敗します。",
                                "The workflow's code that `ritsu dandori gen` writes checks it as soon as the values are made, and so does `ritsu run`; a run that breaks it fails there with `Dandori.BrokenPrecondition`."
                            )
                        });
                        out.push(Finding::of(&d, Some(f.rel.clone()), lang));
                    }
                }
            }
        }
    }
    out
}
