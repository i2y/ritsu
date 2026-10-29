//! A rule's output given to a chobo transfer as its amount (DESIGN 7.6, X4). For each operation a
//! flow runs on a transfer with an amount that can be the output of a rule:
//!
//! - each such amount is a border, held to the amounts chobo takes, 0 to 2⁶³ − 1
//!   ([`borders::amounts_given`]: E203 and W203). chobo fails a call with any other amount rather
//!   than refusing it, so the call would fail only when the workflow runs. An example is an input
//!   from the rule's vectors that comes to an amount outside; where rulec's range of the output
//!   reaches outside and no input does, it is undecided, and the notes say why
//!   ([`borders::amounts_unshown`]). The output is counted over every input of the rule: a branch
//!   of the flow before the transfer, or the range of what the flow gives the rule, does not
//!   narrow it.
//! - for `do` and `hold`, whose refusals can turn on the amounts (an account's bound), the
//!   refusals are one border more: chobo's search, with every amount held to the range of the
//!   amounts the call gives (`Books::refusals`), against the errors the task declares, both held to
//!   the reasons of the book's bounds ([`borders::refusals_met`]: E204 and W204). A key used again
//!   with other arguments, or after a refusal, is refused whatever the amount; a `post` and a
//!   `void` are refused for the state their hold is in, which dandori follows with the case (its
//!   E022), not for their amounts.
//!
//! An amount that never comes from a rule is dandori's to hold to the task's ranges (its E014),
//! and its refusals are the ones chobo's check finds, which dandori holds the task to (its E022).

use crate::{borders, named, Borders, Flow};
use ritsu_base::diag::Diag;
use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;
use ritsu_ports::{Answer, Books, Finding, Found, Origin, OutputValues, Rules, Value};
use ritsu_project::{Joined, Project};

/// The places an amount can come from, as `borders::amounts_given` reads them, with what each rule
/// output is named as for a note.
struct Amounts {
    outputs: Vec<(String, String, Found<OutputValues>)>,
    ranges: Vec<(Option<i128>, Option<i128>)>,
    other: Option<Text>,
}

fn amounts(project: &Project, joined: &Joined, shown: &str, from: &[Origin]) -> Amounts {
    let mut a = Amounts { outputs: Vec::new(), ranges: Vec::new(), other: None };
    for o in from {
        match o {
            Origin::Output { rule, output } => {
                let named = named(project, rule);
                let values = joined.rulec.output_values(rule, output).unwrap_or_else(|_| Found::Undecided(tr!("rulec から \"{named}\" の情報を得られません", "rulec does not answer for \"{named}\"")));
                a.outputs.push((named, output.clone(), values));
            }
            Origin::Range(lo, hi) => a.ranges.push((*lo, *hi)),
            Origin::Unknown(t) => {
                a.other.get_or_insert(tr!("`{shown}` の範囲が分かりません（{}）", "nothing says what range `{shown}` is in ({} has no range)", crate::then_ja(&t.ja, "に範囲がありません"); t.en));
            }
            Origin::Day { .. } | Origin::Now => {
                a.other.get_or_insert(tr!("`{shown}` は数ではないところから来ることがあります", "`{shown}` can come from somewhere that is not a number"));
            }
        }
    }
    a
}

/// A value as a note writes it.
fn shown(v: &Value) -> String {
    match v {
        Value::Int(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Str(s) => format!("\"{s}\""),
        Value::Date(d) | Value::Enum(d) => d.clone(),
        Value::None => "none".into(),
        Value::List(xs) => format!("[{} elements]", xs.len()),
    }
}

/// Reasons as a message lists them: `` `out_of_stock`, `too_many` ``.
fn listed(reasons: &[String]) -> String {
    reasons.iter().map(|r| format!("`{r}`")).collect::<Vec<_>>().join(", ")
}

pub(crate) fn check(project: &Project, flows: &[Flow], joined: &Joined, lang: Lang, borders: &mut Borders) -> Vec<Finding> {
    let mut out = Vec::new();
    for f in flows {
        for call in &f.calls.transfers {
            if !call.amounts.iter().any(|a| a.from.iter().any(|o| matches!(o, Origin::Output { .. }))) {
                continue;
            }
            let task = &call.task;
            let mut all_outputs: Vec<Found<OutputValues>> = Vec::new();
            let mut all_ranges = Vec::new();
            let mut open: Option<Text> = None;
            for a in &call.amounts {
                let got = amounts(project, joined, &a.shown, &a.from);
                all_outputs.extend(got.outputs.iter().map(|x| x.2.clone()));
                all_ranges.extend(got.ranges.iter().copied());
                if let Some(o) = &got.other {
                    open.get_or_insert(o.clone());
                }
                if !a.from.iter().any(|o| matches!(o, Origin::Output { .. })) {
                    continue;
                }
                let outs: Vec<Found<OutputValues>> = got.outputs.iter().map(|x| x.2.clone()).collect();
                let (param, given) = (&a.param, &a.shown);
                match borders::amounts_given(&outs, &got.ranges, got.other.as_ref()) {
                    Answer::Holds => borders.held += 1,
                    Answer::Fails((v, at)) => {
                        borders.failed += 1;
                        let from = match &at {
                            borders::AmountFrom::Output(i, inputs) => {
                                let (rule, output, _) = &got.outputs[*i];
                                let at = inputs.iter().map(|(n, x)| format!("{n} = {}", shown(x))).collect::<Vec<_>>().join(", ");
                                tr!("`{given}` は規則 \"{rule}\" の出力 `{output}` で、{at} のとき {v} になります", "`{given}` is the output `{output}` of the rule \"{rule}\", which comes to {v} at {at}")
                            }
                            borders::AmountFrom::Range(_) => tr!("`{given}` は {v} になることがあります", "`{given}` can be {v}"),
                        };
                        let diag: Diag = Diag::at("E203", &f.file.shown, call.line, 1, tr!(
                            "タスク `{task}` が `{param}` に渡す額が {v} になることがあり、chobo はその額を受け取りません",
                            "The task `{task}` can give `{param}` the amount {v}, which chobo does not take"
                        ))
                        .source(&f.src)
                        .rel(&f.file.rel)
                        .note(from)
                        .note(tr!(
                            "chobo が受け取る額は、単位のいちばん小さい刻みで 0 から 9223372036854775807 までです。それ以外の額の呼び出しは、拒否されるのではなく失敗します。",
                            "chobo takes an amount from 0 to 9223372036854775807 in the unit's smallest step, and a call with any other fails rather than being refused."
                        ))
                        .note(tr!(
                            "規則の返す額を 0 以上にしてください（負の額は、向きの違う振替に分けてください）。",
                            "Make the rule's amounts 0 or more (a negative one is a transfer the other way)."
                        ));
                        out.push(Finding::of(&diag, Some(f.file.rel.clone()), lang));
                    }
                    Answer::Undecided(why) => {
                        borders.undecided += 1;
                        let fails_then = tr!(
                            "受け取らない額なら、ワークフローを走らせたときに chobo がその呼び出しを失敗させます。",
                            "chobo fails a call with an amount it does not take when the workflow runs."
                        );
                        let diag: Diag = Diag::at("W203", &f.file.shown, call.line, 1, tr!(
                            "タスク `{task}` が `{param}` に渡す額を chobo が受け取るかを決められません",
                            "Whether chobo takes the amount the task `{task}` gives `{param}` cannot be decided"
                        ))
                        .source(&f.src)
                        .rel(&f.file.rel);
                        // rulec's range of an output reaches outside, but no input from the rule's vectors does
                        let diag = match borders::amounts_unshown(&outs, got.other.as_ref()) {
                            Some((i, lo, hi)) => {
                                let (rule, output, _) = &got.outputs[i];
                                diag.note(tr!(
                                    "rulec の数えた範囲では、`{given}`（規則 \"{rule}\" の出力 `{output}`）は >={lo} <={hi} です。規則のベクタには、chobo が受け取らない額になる入力がありませんでした。",
                                    "rulec counts `{given}`, the output `{output}` of the rule \"{rule}\", as >={lo} <={hi}, and none of the rule's vectors gives it an amount chobo does not take."
                                ))
                                .note(tr!(
                                    "rulec は出力の範囲を、入力をそれぞれの範囲で動かして数えます。二つの入力を結ぶ `constraint`（`holding <= asked` など）では狭めないので、出力が実際にはそういう額にならないことがあります。",
                                    "rulec bounds an output by moving each input over its own range, and does not narrow the bound by a `constraint` that ties two inputs (`holding <= asked`, say), so the output may never come to such an amount."
                                ))
                                .note(fails_then)
                                .note(tr!(
                                    "rulec が出力を 0 以上と数えるように規則を書くか（`max(…, 0)` など）、負の額を向きの違う振替に分ければ、決められます。",
                                    "Write the rule so that rulec counts the output as 0 or more (`max(…, 0)`, say), or make a negative amount a transfer the other way, and this can be decided."
                                ))
                            }
                            None => diag.note(why).note(fails_then),
                        };
                        out.push(Finding::of(&diag, Some(f.file.rel.clone()), lang));
                    }
                }
            }
            // the refusals the amounts can meet, for the operations whose refusals turn on them
            if call.op != "do" && call.op != "hold" {
                continue;
            }
            let (transfer, op) = (&call.transfer, &call.op);
            let range = borders::amounts_hull(&all_outputs, &all_ranges);
            let found = match (open, range) {
                (Some(why), _) => Found::Undecided(why),
                (None, None) => Found::Undecided(tr!("渡す額の範囲が分からないか、chobo が受け取る額を含みません", "the range of the amounts given is not known, or holds no amount chobo takes")),
                (None, Some(range)) => match joined.chobo.refusals(&call.book, transfer, range) {
                    Ok(v) => v,
                    Err(_) => Found::Undecided(tr!("chobo から \"{}\" の情報を得られません", "chobo does not answer for \"{}\"", named(project, &call.book))),
                },
            };
            let (lo, hi) = range.unwrap_or((1, borders::MAX_AMOUNT));
            // the reasons of the book's bounds: the refusals that turn on the amounts
            let bounds: Vec<String> = joined.chobo.facts(&call.book).map(|b| b.accounts.iter().flat_map(|a| a.lower.iter().chain(a.upper.iter()).map(|x| x.refusal.clone())).collect()).unwrap_or_default();
            match borders::refusals_met(&found, op, &call.handles, &bounds) {
                Answer::Holds => borders.held += 1,
                Answer::Fails(unmet) => {
                    borders.failed += 1;
                    let reasons = listed(&unmet.unhandled);
                    let mut diag: Diag = Diag::at("E204", &f.file.shown, call.line, 1, tr!(
                        "帳簿は `{transfer}.{op}` を {reasons} で拒否することがありますが、タスク `{task}` はそれを処理していません",
                        "The book can refuse `{transfer}.{op}` with {reasons}, which the task `{task}` does not handle"
                    ))
                    .source(&f.src)
                    .rel(&f.file.rel)
                    .note(tr!(
                        "額を {lo} から {hi} まで（呼び出しが渡す額の範囲）に限った chobo の探索で、`{transfer}.{op}` が {reasons} で拒否される例が見つかりました",
                        "chobo's search, with the amounts held to {lo} to {hi} (the range of the amounts the call gives), finds runs in which `{transfer}.{op}` is refused with {reasons}"
                    ));
                    if !unmet.unfound.is_empty() {
                        let unfound = listed(&unmet.unfound);
                        diag = diag.note(tr!("タスクが処理する {unfound} で拒否される例は、探索で見つかりません", "the search finds no run that is refused with {unfound}, which the task handles"));
                    }
                    let first = &unmet.unhandled[0];
                    diag = diag.note(tr!(
                        "タスクのエラーとして宣言し、処理してください（`errors {first}` と `on {first} =>`）。",
                        "Declare it as an error of the task and handle it (`errors {first}` and `on {first} =>`)."
                    ));
                    out.push(Finding::of(&diag, Some(f.file.rel.clone()), lang));
                }
                Answer::Undecided(why) => {
                    borders.undecided += 1;
                    let diag: Diag = Diag::at("W204", &f.file.shown, call.line, 1, tr!(
                        "タスク `{task}` が呼ぶ `{transfer}.{op}` を、帳簿がどの理由で拒否しうるかを決められません",
                        "Which reasons the book can refuse `{transfer}.{op}` with, as the task `{task}` calls it, cannot be decided"
                    ))
                    .source(&f.src)
                    .rel(&f.file.rel)
                    .note(why)
                    .note(tr!(
                        "拒否されれば、その理由はタスクの宣言したエラーとして返ってきます。宣言していない理由なら `failure` です。",
                        "A refusal comes back as the error of the task its reason names; a reason the task does not declare comes back as `failure`."
                    ));
                    out.push(Finding::of(&diag, Some(f.file.rel.clone()), lang));
                }
            }
        }
    }
    out
}
