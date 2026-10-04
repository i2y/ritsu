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
//! - undecided (W201): a value with a place it comes from that has no range, a bound on a list
//!   (dandori calls no rule that walks one, E005, and knows no list's length), or a set of days
//!   (dandori carries no range of dates). The rule's generated code still refuses the call at its
//!   door when the workflow runs, and the warning says so.
//!
//! A value given to both inputs of a relation (`x` to `a` and to `b`) is the same value on both
//! sides: `<=` and `>=` hold, `<` and `>` break at any value it takes.

use crate::Borders;
use ritsu_base::diag::Diag;
use ritsu_base::naming::Tool;
use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;
use ritsu_ports::{Answer, CallArg, Finding, Flows, Precondition, Rules, Value};
use ritsu_project::{Joined, Project};

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

/// One precondition at one call, decided.
fn decide(rules: &dyn Rules, rule: &std::path::Path, p: &Precondition, args: &[CallArg]) -> Answer<Text> {
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
                Err(_) => Answer::Undecided(tr!("rulec がこの規則に答えません", "rulec does not answer for this rule")),
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
                    None => Answer::Undecided(tr!("rulec がこの前提に答えません", "rulec does not answer for this precondition")),
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
        Precondition::Days { input, file, date, .. } => Answer::Undecided(tr!(
            "`{input}` がとるのは koyomi \"{file}\" date {date} の日だけですが、dandori は日付の範囲を運びません",
            "`{input}` takes only the days of koyomi \"{file}\" date {date}, and dandori carries no range of dates"
        )),
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
        let Ok(calls) = joined.dandori.rule_calls(&disk, joined.rules()) else { continue };
        let src = std::fs::read_to_string(&disk).unwrap_or_default();
        for call in calls {
            let Ok(facts) = joined.rulec.facts(&call.rule) else { continue };
            for p in &facts.preconditions {
                let pre = said(p);
                let rule = &call.name;
                match decide(joined.rulec.as_ref(), &call.rule, p, &call.args) {
                    Answer::Holds => borders.held += 1,
                    Answer::Fails(why) => {
                        borders.failed += 1;
                        let d: Diag = Diag::at("E201", &f.shown, call.line, 1, tr!(
                            "規則 {rule} を呼ぶところで、前提 `{pre}` を破る値を渡すことがあります",
                            "The call of the rule {rule} can give it values that break its precondition `{pre}`"
                        ))
                        .source(&src)
                        .rel(&f.rel)
                        .note(why)
                        .note(tr!(
                            "規則の生成したコードは、前提を破る呼び出しを入口で断ります。この呼び出しは、ワークフローを走らせたときに初めて落ちます。値の範囲は、dandori がその値を入れるすべての場所から集めたものです。",
                            "The rule's generated code refuses a call that breaks a precondition at its door, so this call fails only when the workflow runs. The ranges are dandori's, gathered from every place the values come from."
                        ))
                        .note(tr!(
                            "値を渡す前に前提を保つよう分岐するか、範囲を狭めてください（入力やタスクの結果の `range`）。",
                            "Branch so that the precondition holds before the call, or narrow the ranges (the `range` of an input or a task's result)."
                        ));
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
                            "規則の生成したコードが、ワークフローを走らせたときに入口で確かめます。破れば、この呼び出しはそこで落ちます。",
                            "The rule's generated code checks it at its door when the workflow runs; a call that breaks it fails there."
                        ));
                        out.push(Finding::of(&d, Some(f.rel.clone()), lang));
                    }
                }
            }
        }
    }
    out
}
