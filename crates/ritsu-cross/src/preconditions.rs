//! X2: a rule's preconditions, where a workflow calls it (DESIGN 7.4). rulec writes what the
//! shape of an input cannot say as preconditions (a relation between two inputs, a bound on the
//! total or the length of the list it walks, the days of a koyomi date), and the rule's generated
//! code refuses a call that breaks one at its door. dandori knows the range of every value a call
//! gives (every value put in it anywhere in the flow, joined: dandori's DESIGN 1.3) and holds the
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
//! of koyomi dates, each of their days must be one of the rule's; the example is the first that is
//! not. A value that can also come from an input, a task's answer or `now` says nothing of what day
//! it is, and leaves it undecided, to be checked the same way when the workflow runs.
//!
//! A value given to both inputs of a relation (`x` to `a` and to `b`) is the same value on both
//! sides: `<=` and `>=` hold, `<` and `>` break at any value it takes.

use crate::Borders;
use ritsu_base::diag::Diag;
use ritsu_base::naming::Tool;
use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;
use ritsu_ports::{day_text, Answer, CallArg, Dates, Finding, Flows, Found, Origin, Precondition, Rules, UndecidedPrecondition, Value};
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

/// What a value on the wire reads as in a message.
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

/// A range as a `.flow` writes it: `>=1 <=30`.
fn range_text(r: (Option<i128>, Option<i128>)) -> String {
    let mut parts = Vec::new();
    if let Some(lo) = r.0 {
        parts.push(format!(">={lo}"));
    }
    if let Some(hi) = r.1 {
        parts.push(format!("<={hi}"));
    }
    if parts.is_empty() { "any".into() } else { parts.join(" ") }
}

/// A precondition that a date input takes only the days of a koyomi date, at one call: the value
/// given can be the day of koyomi dates (`from`), each of whose days must be one of `days`.
fn days_kept(dates: &dyn Dates, a: &CallArg, days: &ritsu_ports::DaySet, file: &str, date: &str) -> Answer<Text> {
    let shown = &a.shown;
    let mut other: Option<Text> = None;
    if a.from.is_empty() {
        other = Some(tr!("`{shown}` が何日になるかは分かりません", "nothing says what day `{shown}` is"));
    }
    for o in &a.from {
        match o {
            Origin::Day { file: from, date: d } => match dates.values(from, d) {
                Ok(Found::Value(set)) => {
                    if let Some(day) = set.iter().copied().find(|x| !days.contains(x)) {
                        let day = day_text(day);
                        return Answer::Fails(tr!(
                            "`{shown}` は koyomi の日付 {d} の日で、{day} になることがありますが、{day} は koyomi \"{file}\" date {date} の日ではありません",
                            "`{shown}` is a day of the koyomi date {d} and can be {day}, which is not a day of koyomi \"{file}\" date {date}"
                        ));
                    }
                }
                Ok(Found::Undecided(why)) => {
                    other.get_or_insert(tr!("koyomi は日付 {d} の日を数えません（{}）", "koyomi does not count the days of the date {d}: {}", why.ja; why.en));
                }
                Err(_) => {
                    other.get_or_insert(tr!("koyomi から日付 {d} の情報を得られません", "koyomi does not answer for the date {d}"));
                }
            },
            Origin::Now => {
                other.get_or_insert(if shown == "now" { tr!("`now` はどの日にもなりえます", "`now` can be any day") } else { tr!("`{shown}` は `now` のことがあり、どの日にもなりえます", "`{shown}` can be `now`, which can be any day") });
            }
            Origin::Unknown(t) => {
                other.get_or_insert(tr!("`{shown}` が何日になるかは分かりません（{}）", "nothing says what day `{shown}` is ({} gives no range of days)", crate::then_ja(&t.ja, "には日付の範囲がありません"); t.en));
            }
            Origin::Output { .. } | Origin::Range(..) => {
                other.get_or_insert(tr!("`{shown}` は koyomi の日付でないところから来ることがあります", "`{shown}` can come from somewhere other than a koyomi date"));
            }
        }
    }
    match other {
        Some(why) => Answer::Undecided(why),
        None => Answer::Holds,
    }
}

/// One precondition at one call, decided.
fn decide(rules: &dyn Rules, dates: &dyn Dates, rule: &std::path::Path, p: &Precondition, args: &[CallArg]) -> Answer<Text> {
    let arg = |n: &str| args.iter().find(|a| a.input == n);
    match p {
        Precondition::Relation { left, op, right } => {
            let (Some(l), Some(r)) = (arg(left), arg(right)) else {
                return Answer::Undecided(tr!("呼び出しが `{left}` か `{right}` を渡していません", "the call does not give `{left}` or `{right}`"));
            };
            // the same value on both sides
            if l.shown == r.shown {
                return if op.contains('=') {
                    Answer::Holds
                } else {
                    let at = l.range.and_then(|r| r.0.or(r.1)).map(|v| v.to_string()).unwrap_or_else(|| "…".into());
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
                        "`{}` の範囲が分かりません（{}に範囲がありません）",
                        "nothing says what range `{}` is in ({} has no range)",
                        a.shown, from.ja; a.shown, from.en
                    ));
                }
            }
            let (lr, rr) = (l.range.unwrap_or((None, None)), r.range.unwrap_or((None, None)));
            let ranges = [(left.clone(), lr.0, lr.1), (right.clone(), rr.0, rr.1)];
            match rules.preconditions_hold(rule, &ranges, None) {
                Err(_) => Answer::Undecided(tr!("rulec からこの規則の情報を得られません", "rulec does not answer for this rule")),
                Ok(answers) => match answers.into_iter().find(|(q, _)| q == p).map(|(_, a)| a) {
                    Some(Answer::Holds) => Answer::Holds,
                    Some(Answer::Fails(ex)) => {
                        let at: Vec<String> = ex.iter().map(|(n, v)| format!("{n} = {}", shown(v))).collect();
                        let at = at.join(", ");
                        Answer::Fails(tr!(
                            "`{}` は `{}`、`{}` は `{}` で、{at} のとき `{left} {op} {right}` が成り立ちません",
                            "`{}` is `{}` and `{}` is `{}`, and at {at} `{left} {op} {right}` does not hold",
                            l.shown,
                            range_text(lr),
                            r.shown,
                            range_text(rr)
                        ))
                    }
                    Some(Answer::Undecided(why)) => Answer::Undecided(why),
                    None => Answer::Undecided(tr!("rulec から、この前提が成り立つかどうかの結果を得られません", "rulec does not answer for this precondition")),
                },
            }
        }
        Precondition::Sum { name, over, .. } => Answer::Undecided(tr!(
            "`{name}` は並び `{over}` の合計の上限ですが、dandori は並びの長さを知りません",
            "`{name}` bounds a total over the list `{over}`, and dandori knows no list's length"
        )),
        Precondition::Length { sequence, .. } => Answer::Undecided(tr!(
            "並び `{sequence}` の長さの上限ですが、dandori は並びの長さを知りません",
            "it bounds the length of the list `{sequence}`, and dandori knows no list's length"
        )),
        Precondition::Days { input, file, date, days } => match arg(input) {
            Some(a) => days_kept(dates, a, days, file, date),
            None => Answer::Undecided(tr!("呼び出しが `{input}` を渡していません", "the call does not give `{input}`")),
        },
    }
}

/// How a precondition reads in a message.
fn said(p: &Precondition) -> String {
    match p {
        Precondition::Relation { left, op, right } => format!("{left} {op} {right}"),
        Precondition::Sum { name, max, .. } => format!("{name} <= {max}"),
        Precondition::Length { sequence, max } => format!("len({sequence}) <= {max}"),
        Precondition::Days { input, file, date, .. } => format!("{input} in koyomi \"{file}\" date {date}"),
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
                let pre = said(p);
                let rule = &call.name;
                match decide(joined.rulec.as_ref(), joined.koyomi.as_ref(), &call.rule, p, &call.args) {
                    Answer::Holds => borders.held += 1,
                    Answer::Fails(why) => {
                        borders.failed += 1;
                        let d: Diag = Diag::at("E201", &f.shown, call.line, 1, tr!(
                            "規則 {rule} を呼ぶところで、前提 `{pre}` を破る値を渡すことがあります",
                            "The call of the rule {rule} can give it values that break its precondition `{pre}`"
                        ))
                        .source(&src)
                        .rel(&f.rel)
                        .note(why);
                        let d = if let Precondition::Days { file, date, .. } = p {
                            d.note(tr!(
                                "規則から生成したコードは、前提を破る呼び出しを入口で断ります。この呼び出しは、ワークフローを走らせたときに初めて落ちます。koyomi は、入力の範囲のすべてで日付の日を数えています。",
                                "The rule's generated code refuses a call that breaks a precondition at its door, so this call fails only when the workflow runs. koyomi counts the days of a date over the whole range of its inputs."
                            ))
                            .note(tr!(
                                "koyomi \"{file}\" date {date} の日を渡すか、規則の範囲を直してください。",
                                "Give it the days of koyomi \"{file}\" date {date}, or correct the rule's range."
                            ))
                        } else {
                            d.note(tr!(
                                "規則から生成したコードは、前提を破る呼び出しを入口で断ります。この呼び出しは、ワークフローを走らせたときに初めて落ちます。値の範囲は、dandori がその値を入れるすべての場所から集めたものです。",
                                "The rule's generated code refuses a call that breaks a precondition at its door, so this call fails only when the workflow runs. The ranges are dandori's, gathered from every place the values come from."
                            ))
                            .note(tr!(
                                "値を渡す前に前提を保つよう分岐するか、範囲を狭めてください（入力やタスクの結果の `range`）。",
                                "Branch so that the precondition holds before the call, or narrow the ranges (the `range` of an input or a task's result)."
                            ))
                        };
                        out.push(Finding::of(&d, Some(f.rel.clone()), lang));
                    }
                    Answer::Undecided(why) => {
                        borders.undecided += 1;
                        let d: Diag = Diag::at("W201", &f.shown, call.line, 1, tr!(
                            "規則 {rule} を呼ぶところで、前提 `{pre}` が保たれるかを決められません",
                            "Whether the call of the rule {rule} keeps its precondition `{pre}` cannot be decided"
                        ))
                        .source(&src)
                        .rel(&f.rel)
                        .note(why)
                        .note(tr!(
                            "`ritsu dandori build` が書くワークフローのコードが、走らせたときに、値ができたところですぐに確かめます。破る実行は、そこで `Dandori.BrokenPrecondition` で失敗します。",
                            "The workflow's code that `ritsu dandori build` writes checks it when the workflow runs, as soon as the values are made; a run that breaks it fails there with `Dandori.BrokenPrecondition`."
                        ));
                        out.push(Finding::of(&d, Some(f.rel.clone()), lang));
                    }
                }
            }
        }
    }
    out
}
