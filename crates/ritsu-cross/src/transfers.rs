//! A rule's output given to a chobo transfer as its amount (DESIGN 7.6, X4). For each operation a
//! flow runs on a transfer with an amount that can be the output of a rule:
//!
//! - each such amount is a border, held to the amounts chobo takes, 0 to 2⁶³ − 1
//!   ([`borders::amounts_given`]: E203 and W203). chobo fails a call with any other amount rather
//!   than refusing it, so the call would fail only when the workflow runs. An example is an input
//!   from the rule's vectors that comes to an amount outside; where rulec's range of the output
//!   reaches outside and no input does, it is undecided, and the notes say why
//!   ([`borders::amounts_unshown`]). Where dandori follows the runs of the flow to the transfer
//!   (`Amount::calls`), each call of a rule whose output the amount can be is asked over what
//!   those runs can give it (`Rules::output_values_over`): the range dandori gives a number, and
//!   the values of an enum or a bool the arms of a `match` before the transfer leave. An amount no
//!   run that reaches the transfer gives a rule's output is no border. Elsewhere the output is
//!   counted over every input of the rule.
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
use ritsu_ports::{Amount, Answer, Books, ColumnType, Finding, Found, Held, Origin, OutputCall, OutputValues, Rules, Value};
use ritsu_project::{Joined, Project};

/// One rule's output an amount can be: the rule as a note names it, the output, what it comes to,
/// and the call of the rule the runs to the transfer make, where dandori follows them.
struct Output {
    rule: String,
    output: String,
    values: Found<OutputValues>,
    call: Option<OutputCall>,
}

/// The places an amount can come from, as `borders::amounts_given` reads them.
struct Amounts {
    outputs: Vec<Output>,
    ranges: Vec<(Option<i128>, Option<i128>)>,
    other: Option<Text>,
}

/// Whether an amount can be a rule's output on a run that reaches the transfer: one of the calls
/// dandori follows to it, or, where it does not follow them, a place it can come from.
fn from_rule(a: &Amount) -> bool {
    match &a.calls {
        Some(calls) => !calls.is_empty(),
        None => a.from.iter().any(|o| matches!(o, Origin::Output { .. })),
    }
}

/// What a call gives the rule's inputs, as `Rules::output_values_over` takes it: a number on the
/// wire, an enum by the places of its values in the rule's order (a run of places as one range, a
/// value apart from the others as the input named again), a bool as 0 and 1. An input whose value
/// the rule does not name is held to nothing narrower.
fn held_ranges(joined: &Joined, call: &OutputCall) -> Vec<(String, Option<i128>, Option<i128>)> {
    let facts = joined.rulec.facts(&call.rule).ok();
    let mut out = Vec::new();
    for i in &call.inputs {
        match &i.held {
            None => {}
            Some(Held::Range(lo, hi)) => out.push((i.input.clone(), *lo, *hi)),
            Some(Held::Values(values)) => {
                let ty = facts.as_ref().and_then(|f| f.inputs.iter().find(|c| c.name == i.input)).map(|c| match &c.ty {
                    ColumnType::Opt(t) => (**t).clone(),
                    t => t.clone(),
                });
                let order: Vec<String> = match &ty {
                    Some(ColumnType::Bool) => vec!["false".into(), "true".into()],
                    Some(ColumnType::Enum { name, .. }) => facts.as_ref().and_then(|f| f.enums.iter().find(|e| e.name == *name)).map(|e| e.values.iter().map(|v| v.name.clone()).collect()).unwrap_or_default(),
                    _ => Vec::new(),
                };
                let places: Option<Vec<i128>> = values.iter().map(|v| order.iter().position(|o| o == v).map(|p| p as i128)).collect();
                let Some(mut places) = places.filter(|p| !p.is_empty()) else { continue };
                places.sort_unstable();
                let mut runs: Vec<(i128, i128)> = Vec::new();
                for p in places {
                    match runs.last_mut() {
                        Some((_, hi)) if *hi + 1 == p => *hi = p,
                        _ => runs.push((p, p)),
                    }
                }
                out.extend(runs.into_iter().map(|(lo, hi)| (i.input.clone(), Some(lo), Some(hi))));
            }
        }
    }
    out
}

/// What a call gives the inputs the flow holds, as a note says it: `` `event_kind` = handback or
/// workshop ``, `` `asked` >=0 <=50 ``.
fn held_text(call: &OutputCall) -> Option<Text> {
    let mut ja = Vec::new();
    let mut en = Vec::new();
    for i in &call.inputs {
        let input = &i.input;
        match &i.held {
            None => {}
            Some(Held::Values(vs)) => {
                ja.push(format!("`{input}` = {}", vs.join(" か ")));
                let words = match vs.split_last() {
                    Some((last, rest)) if !rest.is_empty() => format!("{} or {last}", rest.join(", ")),
                    _ => vs.join(""),
                };
                en.push(format!("`{input}` = {words}"));
            }
            Some(Held::Range(lo, hi)) => {
                let ends = [lo.map(|n| format!(">={n}")), hi.map(|n| format!("<={n}"))].into_iter().flatten().collect::<Vec<_>>().join(" ");
                ja.push(format!("`{input}` {ends}"));
                en.push(format!("`{input}` {ends}"));
            }
        }
    }
    (!en.is_empty()).then(|| Text::new(ja.join("、"), en.join(", ")))
}

fn amounts(project: &Project, joined: &Joined, a: &Amount) -> Amounts {
    let shown = &a.shown;
    let mut got = Amounts { outputs: Vec::new(), ranges: Vec::new(), other: None };
    let unread = |named: &str| Found::Undecided(tr!("rulec から \"{named}\" の情報を得られません", "rulec does not answer for \"{named}\""));
    // the calls the runs to the transfer make, each over what they give it
    for call in a.calls.iter().flatten() {
        let named = named(project, &call.rule);
        let ranges = held_ranges(joined, call);
        let values = if ranges.is_empty() { joined.rulec.output_values(&call.rule, &call.output) } else { joined.rulec.output_values_over(&call.rule, &call.output, &ranges) };
        got.outputs.push(Output { rule: named.clone(), output: call.output.clone(), values: values.unwrap_or_else(|_| unread(&named)), call: Some(call.clone()) });
    }
    for o in &a.from {
        match o {
            // where dandori follows the runs, the calls above stand for the outputs
            Origin::Output { .. } if a.calls.is_some() => {}
            Origin::Output { rule, output } => {
                let named = named(project, rule);
                let values = joined.rulec.output_values(rule, output).unwrap_or_else(|_| unread(&named));
                got.outputs.push(Output { rule: named, output: output.clone(), values, call: None });
            }
            Origin::Range(lo, hi) => got.ranges.push((*lo, *hi)),
            Origin::Unknown(t) => {
                got.other.get_or_insert(tr!("`{shown}` の範囲が分かりません（{}）", "nothing says what range `{shown}` is in ({} has no range)", crate::then_ja(&t.ja, "に範囲がありません"); t.en));
            }
            Origin::Day { .. } | Origin::Now => {
                got.other.get_or_insert(tr!("`{shown}` は数ではないところから来ることがあります", "`{shown}` can come from somewhere that is not a number"));
            }
        }
    }
    got
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
            if !call.amounts.iter().any(from_rule) {
                continue;
            }
            let task = &call.task;
            let mut all_outputs: Vec<Found<OutputValues>> = Vec::new();
            let mut all_ranges = Vec::new();
            let mut open: Option<Text> = None;
            for a in &call.amounts {
                let got = amounts(project, joined, a);
                all_outputs.extend(got.outputs.iter().map(|x| x.values.clone()));
                all_ranges.extend(got.ranges.iter().copied());
                if let Some(o) = &got.other {
                    open.get_or_insert(o.clone());
                }
                if !from_rule(a) {
                    continue;
                }
                let outs: Vec<Found<OutputValues>> = got.outputs.iter().map(|x| x.values.clone()).collect();
                let (param, given) = (&a.param, &a.shown);
                match borders::amounts_given(&outs, &got.ranges, got.other.as_ref()) {
                    Answer::Holds => borders.held += 1,
                    Answer::Fails((v, at)) => {
                        borders.failed += 1;
                        let mut held: Option<Text> = None;
                        let mut fix = tr!(
                            "規則の返す額を 0 以上にしてください（負の額は、向きの違う振替に分けてください）。",
                            "Make the rule's amounts 0 or more (a negative one is a transfer the other way)."
                        );
                        let from = match &at {
                            borders::AmountFrom::Output(i, inputs) => {
                                let o = &got.outputs[*i];
                                let (rule, output) = (&o.rule, &o.output);
                                let at = inputs.iter().map(|(n, x)| format!("{n} = {}", shown(x))).collect::<Vec<_>>().join(", ");
                                if let Some(c) = &o.call {
                                    let line = c.line;
                                    held = held_text(c).map(|h| {
                                        tr!("この振替に至る実行では、{line} 行目の呼び出しが規則に渡すのは {} です。", "On the runs that reach this transfer, the call on line {line} gives the rule {}.", h.ja; h.en)
                                    });
                                    // the first input of the example, in the rule's order, that a
                                    // `match` before the transfer narrows
                                    let branch = inputs.iter().find_map(|(n, x)| {
                                        let value = match x {
                                            Value::Enum(w) => w.clone(),
                                            Value::Bool(b) => b.to_string(),
                                            _ => return None,
                                        };
                                        c.inputs.iter().find(|i| i.input == *n && i.matchable).map(|i| (i.shown.clone(), value))
                                    });
                                    if let Some((name, value)) = branch {
                                        fix = tr!(
                                            "規則の返す額を 0 以上にしてください（負の額は、向きの違う振替に分けてください）。または、振替の前に `{name}` を `match` で分け、`{value}` の分岐では出力を振替に渡さないでください。",
                                            "Make the rule's amounts 0 or more (a negative one is a transfer the other way), or branch on `{name}` with `match` before the transfer and give the transfer the output only in the arms without `{value}`."
                                        );
                                    }
                                }
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
                        .note(from);
                        let diag = match held {
                            Some(h) => diag.note(h),
                            None => diag,
                        };
                        let diag = diag
                            .note(tr!(
                                "chobo が受け取る額は、単位のいちばん小さい刻みで 0 から 9223372036854775807 までです。それ以外の額の呼び出しは、拒否されるのではなく失敗します。",
                                "chobo takes an amount from 0 to 9223372036854775807 in the unit's smallest step, and a call with any other fails rather than being refused."
                            ))
                            .note(fix);
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
                                let o = &got.outputs[i];
                                let (rule, output) = (&o.rule, &o.output);
                                let counted = match o.call.as_ref().and_then(|c| held_text(c).map(|h| (c.line, h))) {
                                    Some((line, h)) => tr!(
                                        "{line} 行目の呼び出しが規則に渡す値（{}）で rulec が数えた範囲では、`{given}`（規則 \"{rule}\" の出力 `{output}`）は >={lo} <={hi} です。その値の中の規則のベクタには、chobo が受け取らない額になる入力がありませんでした。",
                                        "rulec counts `{given}`, the output `{output}` of the rule \"{rule}\", as >={lo} <={hi} over what the call on line {line} gives it ({}), and none of the rule's vectors there gives it an amount chobo does not take.",
                                        h.ja; h.en
                                    ),
                                    None => tr!(
                                        "rulec の数えた範囲では、`{given}`（規則 \"{rule}\" の出力 `{output}`）は >={lo} <={hi} です。規則のベクタには、chobo が受け取らない額になる入力がありませんでした。",
                                        "rulec counts `{given}`, the output `{output}` of the rule \"{rule}\", as >={lo} <={hi}, and none of the rule's vectors gives it an amount chobo does not take."
                                    ),
                                };
                                diag.note(counted)
                                .note(tr!(
                                    "rulec は出力の範囲を、入力をそれぞれの範囲で動かして数えます。二つの入力を結ぶ `constraint`（`holding <= asked` など）では狭めないので、出力が実際にはそういう額にならないことがあります。",
                                    "rulec bounds an output by moving each input over its own range, and does not narrow the bound by a `constraint` that ties two inputs (`holding <= asked`, say), so the output may never come to such an amount."
                                ))
                                .note(fails_then)
                                .note(tr!(
                                    "rulec が出力を 0 以上と数えるように規則を書くか（`max(…, 0)` など）、負の額を向きの違う振替に分ければ、決められます。フローが規則に渡す値を、そういう額にならない入力に絞っても（振替の前の `match`、数の来るところの `range`）、決められることがあります。",
                                    "Write the rule so that rulec counts the output as 0 or more (`max(…, 0)`, say), or make a negative amount a transfer the other way, and this can be decided. Holding what the flow gives the rule to inputs that do not come to such an amount (a `match` before the transfer, the `range` of where a number comes from) can decide it too."
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
