//! Scenarios that take every arm: the execution's input and the answers of the calls,
//! chosen so that between them the runs go down every arm of every match, into every
//! handler, and through every way the machine lets a case move. They are found by running
//! the workflow again and again with a record of the choices made so far, taking the next
//! untried choice each time (a depth-first walk by replay), and keeping the runs that
//! reach something no earlier run reached.
//!
//! The runs here only choose answers; what the answers mean is the reference
//! interpreter's business. A choice made here that the interpreter takes differently only
//! costs coverage, never a wrong verdict.

use crate::computed::{Computed, Takes};
use crate::diag::Text;
use crate::interp::{matches_error, Answers, Clock, Passing};
use crate::model::*;
use crate::render;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

const MAX_RUNS: usize = 5000;
const MAX_STEPS: usize = 2000;
const HOLE: &str = "\u{1}hole:";

enum Ctl {
    Next,
    Break,
    Stop,
}

/// What `steer_rule` asks rulec once in a walk, whichever run asks it first: the inputs of each
/// rule's vectors, and for each call as a run gives it (the rule, the values the call gives that are
/// not left open, and the inputs that are), each answer the rule comes to with the open values
/// taken from a vector, with them.
#[derive(Default)]
struct Aims {
    /// the outputs of each call of a rule (by the site of its statement) that a `match` reads, the
    /// enums and truth values the call steers toward each answer of; None where dandori does not
    /// follow a `match` on a rule's output to its calls, and each call steers toward every enum
    /// and truth value of its rule (`crossings::matched_outputs`)
    aimed: Option<BTreeMap<usize, BTreeSet<String>>>,
    vectors: BTreeMap<usize, std::rc::Rc<Vec<Map<String, Value>>>>,
    answers: BTreeMap<String, std::rc::Rc<Vec<Aim>>>,
}

/// The values a vector gives the inputs of a call that are left open, and what the rule's enums and
/// truth values come to with them.
type Aim = (Vec<(String, Value)>, Vec<Value>);

struct Ex<'a> {
    m: &'a Model,
    /// what the calls of rules asked rulec in the runs before (`steer_rule`)
    aims: &'a std::cell::RefCell<Aims>,
    /// how often each alternative of each choice has been taken, over all runs so far
    counts: BTreeMap<(String, usize), usize>,
    /// the choices to make first, in order
    prefix: Vec<usize>,
    /// every choice made in this run: (the alternative taken, how many there were)
    trail: Vec<(usize, usize)>,
    /// in a sticky run, a choice made once is made the same way again in the same run,
    /// which is what it takes for a loop to run out
    sticky: bool,
    /// after the prefix, every choice takes its first alternative, the way the flow goes first
    firsts: bool,
    made: BTreeMap<String, usize>,
    /// in a sticky run, where in `trail` each choice was first made again the same way: a loop that
    /// does not run out goes around on these
    again: BTreeMap<String, usize>,
    /// the run went on for more than `MAX_STEPS` steps, and was cut there
    cut: bool,
    labels: BTreeSet<String>,
    vars: BTreeMap<String, Value>,
    /// the values left open: the type, what was chosen, and the range of the numbers in it
    holes: Vec<(Ty, Option<Value>, Option<Range>)>,
    cases: Vec<Option<usize>>,
    answers: Vec<Value>,
    in_on_failure: bool,
    /// a cancellation stopped the run, and `on cancel` runs next
    cancelled: bool,
    in_on_cancel: bool,
    steps: usize,
    stopped: bool,
    /// how many parallel rounds are running around the current statement
    par_depth: usize,
    /// the current parallel round failed: by a task's error (true) or otherwise (false)
    pending: Option<bool>,
    /// a workflow with a period: whether the moment every `now` after the start reads is in it,
    /// which the first check of the period after the start chose (DESIGN 20)
    later: Option<bool>,
    /// the `for` loops that went through a list with something in it, in this run
    done_loops: Vec<String>,
    /// how many values have been made in this run, so that no two are alike and a target
    /// that mixes up the rounds of a loop, or two answers, shows it
    made_values: usize,
    /// a workflow that implements a service: the input has its fields at their zero values, which
    /// it goes without, as protobuf's JSON writes the request
    sparse: bool,
    /// the flow checks a rule's precondition (`TK::Check`), koyomi answers its calls of dates, or
    /// rulec its calls of rules with an enum or a truth value among their outputs: its numbers and
    /// days are made as values left open with what was made in them, which a check or a call of a
    /// date or a rule changes to take the way it chose
    steered: bool,
    /// on a clock that moves, in a workflow whose period has an end and that waits until a moment: its
    /// times are made as values left open, which a `wait until` steers (`steer_wait`)
    moments: bool,
    /// the moments the run may start at, the first unless the walk chooses another
    /// (`start_moments`)
    starts: &'a [String],
    /// the calls of rules are rulec's: answered by the port of rules from the values the call gives,
    /// as `ritsu run` answers them, and not written in the scenario (`generate_computed`)
    computed: bool,
    /// rulec gave a call of a rule no answer at all, which stops a run of `ritsu run` short: the run
    /// is not written
    dead: bool,
    /// what `now` reads: a clock that moves, as `ritsu run` and `dandori run` run the scenarios
    /// `generate_computed` writes, or one that does not, as the runners run the platforms
    clock: Clock,
    /// the calls of dates and the operations of books koyomi and chobo answer, through the ports, as
    /// `ritsu run` answers them (`generate_computed`, with the ports): the books kept from the
    /// start of the run, and told the time as it goes by
    oracle: Option<Computed>,
    /// what this run does to each book before it starts (its `books`), as written
    before_ops: Vec<Vec<Value>>,
    /// the accounts an operation of a book has named in this run (but one chobo answered as done
    /// before, which changed nothing), and the keys of the transfers done or held, by the book's
    /// place: an operation that names none of them can be given what it takes before the run without
    /// changing what came before it
    touched: BTreeSet<(usize, String, Vec<String>)>,
    keys: BTreeSet<(usize, String, Vec<String>)>,
    /// what each hold this run made was given, by the book's place, the transfer and the key
    held: BTreeMap<(usize, String, Vec<String>), Map<String, Value>>,
    /// the values left open that the run has decided something on: a `match` read them, a `wait
    /// until` waited for them, or a language answered a call given them. Nothing steers them after,
    /// which would change what was decided
    kept: BTreeSet<usize>,
    /// each check of a precondition the run has made, and whether it held: a value one of them
    /// read is steered after only where every one of them still comes out as it did
    checked: Vec<(PreTest, bool)>,
    /// how the run ends: `succeed`, `fail` or `cancel`, as the reference interpreter ends it
    ended: Option<&'static str>,
}

/// Scenarios whose `answers` answer every call, the calls of rules, of dates and of books among
/// them, with the answers chosen here, on a clock that does not move: what the runners of the
/// platforms run, and the reference interpreter with them on the same clock (`interp::run`).
pub fn generate(m: &Model) -> Vec<Value> {
    generate_in(m, false)
}

/// Scenarios for `ritsu run` and `dandori run`, on a clock that moves as time goes on in the run:
/// what `dandori scenarios` writes (§15.209 of rulec's DESIGN). The calls of local rules are
/// rulec's, through the port of rules the program that runs dandori hands over, and with the ports
/// of dates and books (`ritsu dandori scenarios`) the calls of dates are koyomi's and the
/// operations of books chobo's, as `ritsu run` answers them; no answer for them is written, and
/// each scenario says so (`"rules": "rulec"`, `"dates": "koyomi"`, and `books`, what is done to
/// each book before the run). The values a call gives them are chosen at the call, where they were
/// left open, and what the language answers is what the run goes on with: to change it, a run
/// changes where its values come from — the answers of the tasks before it, and what the books
/// hold before the run. Each scenario is run as `ritsu run` runs it before it is written, and one
/// that does not come to the end the walk here came to is not written. The walk asks the ports
/// again and again, and each asks whether its check still holds by reading what the check read:
/// within the walk, the disk is asked each question once (`ritsu_base::fs::steady`, DESIGN 36.3).
pub fn generate_computed(m: &Model) -> Vec<Value> {
    ritsu_base::fs::steady(|| computed_in(m))
}

/// [`generate_computed`], with the disk as it is.
fn computed_in(m: &Model) -> Vec<Value> {
    let mut out = generate_in(m, true);
    let ports = crate::sources::held().and_then(|h| h.ports());
    let rules = m.rules.iter().any(|r| matches!(r.kind, RuleKind::Rule));
    let dates = ports.is_some() && m.rules.iter().any(|r| matches!(r.kind, RuleKind::Date(_)));
    let books = ports.is_some() && !m.books.is_empty();
    for sc in out.iter_mut() {
        if rules {
            sc["rules"] = json!(crate::computed::RULES_COMPUTED);
        }
        if dates {
            sc["dates"] = json!(crate::computed::DATES_COMPUTED);
        }
        if books {
            let mut given = Map::new();
            for (b, bu) in m.books.iter().enumerate() {
                let ops = sc.get("books").and_then(|o| o.get(b.to_string())).cloned().unwrap_or_else(|| json!([]));
                given.insert(bu.name.clone(), ops);
            }
            sc["books"] = Value::Object(given);
        }
    }
    // every scenario written runs as `ritsu run` runs it (without the three ports, as `dandori run`
    // does with the ports it has), to the end the walk came to, through every place it says it covers
    let mut wrong = Vec::new();
    out.retain(|sc| match runs_as_walked(m, sc, ports.as_ref()) {
        Ok(()) => true,
        Err(why) => {
            wrong.push(format!("{}: {why}", sc["name"].as_str().unwrap_or("a run")));
            false
        }
    });
    debug_assert!(wrong.is_empty(), "a scenario the walk wrote does not run as `ritsu run` runs it:\n{}", wrong.join("\n"));
    for (i, sc) in out.iter_mut().enumerate() {
        sc["name"] = json!(format!("run {}", i + 1));
    }
    for sc in out.iter_mut() {
        if let Some(o) = sc.as_object_mut() {
            o.remove(WALKED_END);
        }
    }
    out
}

/// A scenario `generate_computed` wrote, run as `ritsu run` runs it, with the ports `ritsu dandori`
/// hands over (without the three, as `dandori run` runs it with the ports it has), in the words of
/// `view`: its trace, what it went through, and the answers of its calls in the order the run took
/// them, those the languages computed among them. None where the run stops, which the walk that
/// wrote it keeps from happening.
pub fn run_written(m: &Model, sc: &Value, view: render::View) -> Option<(Value, Vec<crate::interp::Visit>, Vec<Value>)> {
    let ran = match crate::sources::held().and_then(|h| h.ports()) {
        Some((r, d, b)) => crate::computed::run(m, sc, view, r, d, b).ok()?,
        None => match crate::computed::said_ports(sc) {
            Some((r, d, b)) => crate::computed::run_said(m, sc, view, r, d, b).ok()?,
            None => {
                let ran = crate::interp::run_moving_visits(m, sc, view).ok()?;
                return Some((ran.trace, ran.visits, sc["answers"].as_array().cloned().unwrap_or_default()));
            }
        },
    };
    let answers = ran.replay["answers"].as_array().cloned().unwrap_or_default();
    Some((ran.trace, ran.visits, answers))
}

/// An arm of a `match`, a handler of an error at a call, or the `on failure` or the `on cancel` of
/// the workflow, that no scenario of a list takes (`untaken`).
#[derive(Clone, Debug, PartialEq)]
pub struct Untaken {
    /// the line it is written on, from 1
    pub line: usize,
    /// what it is
    pub what: Text,
    /// why no run takes it, where rulec says: the rule whose answer the `match` reads never comes to
    /// what the arm takes, with what the workflow can give it at the call
    pub never: Option<Text>,
}

impl Untaken {
    /// As `scenarios` and the page say it: what no scenario takes, and why, where rulec says. Without
    /// rulec's word, only that the walk found no run that comes to it, which says nothing of whether
    /// one can.
    pub fn said(&self) -> Text {
        let what = &self.what;
        match &self.never {
            Some(n) => ritsu_base::tr!("{} を通るシナリオはありません。{}", "no scenario takes {}: {}", what.ja, n.ja; what.en, n.en),
            None => ritsu_base::tr!(
                "{} を通るシナリオはありません。シナリオを作る歩き方が、そこへ届く実行を見つけていません",
                "no scenario takes {}: the walk that makes the scenarios finds no run that comes to it",
                what.ja;
                what.en
            ),
        }
    }
}

/// The arms of the `match`es, the handlers of errors at the calls, and the `on failure` and the `on
/// cancel` of the workflow `m`, that no scenario of `list` takes (by the names of their `covers`),
/// in the order of their lines. An arm of a `match` on the answer of a rule says why, where rulec
/// says the rule never comes to what the arm takes: every place the value can come from is a rule's
/// output, dandori follows it to each call it can be the answer of (`crossings::matched_outputs`),
/// and rulec, asked over what the runs that reach the call can give the rule
/// (`Rules::outputs_together`, one question for each rule and what it is given, with the outputs
/// those `match`es read), answers exactly, without the arm's values.
pub fn untaken(m: &Model, list: &[Value]) -> Vec<Untaken> {
    let taken: BTreeSet<&str> = list.iter().flat_map(|sc| sc["covers"].as_array().into_iter().flatten().filter_map(|c| c.as_str())).collect();
    let mut out: Vec<Untaken> = Vec::new();
    let mut arms: Vec<(usize, usize, &TArm)> = Vec::new();
    for s in m.all_stmts() {
        match &s.kind {
            TK::Match { arms: xs, .. } => {
                for (i, a) in xs.iter().enumerate() {
                    if !taken.contains(format!("{}:arm{i}", s.site).as_str()) {
                        arms.push((s.site, s.line, a));
                    }
                }
            }
            TK::Call { callee, handlers, .. } => {
                // the language that answers the call in the runs written, where one does (as
                // `generate_computed` leaves the calls to them): its answer comes at once, and the
                // call does not time out
                let ports = crate::sources::held().and_then(|h| h.ports()).is_some();
                let answers = match callee {
                    Callee::Rule(r) => match m.rules[*r].kind {
                        RuleKind::Rule => Some("rulec"),
                        RuleKind::Date(_) if ports => Some("koyomi"),
                        _ => None,
                    },
                    Callee::Task(t) if ports && m.tasks[*t].book().is_some() => Some("chobo"),
                    Callee::Task(_) => None,
                };
                for h in handlers {
                    if !taken.contains(format!("{}:{}", s.site, handler_way(h)).as_str()) {
                        let errors = h
                            .errors
                            .iter()
                            .map(|e| match e {
                                HErr::Declared(n) => n.clone(),
                                HErr::Timeout => "timeout".into(),
                                HErr::Failure => "failure".into(),
                            })
                            .collect::<Vec<_>>()
                            .join(", ");
                        let line = s.line;
                        let never = answers.filter(|_| h.errors.iter().all(|e| *e == HErr::Timeout)).map(|lang| ritsu_base::tr!("実行では {lang} がその場で答えるので、この呼び出しは時間切れになりません", "{lang} answers the call at once in a run, so it does not time out"));
                        out.push(Untaken { line: h.line, what: ritsu_base::tr!("{line} 行目の呼び出しの `on {errors}`", "the `on {errors}` of the call on line {line}"), never });
                    }
                }
            }
            _ => {}
        }
    }
    for (block, name) in [(&m.on_failure, "on failure"), (&m.on_cancel, "on cancel")] {
        if let Some(b) = block {
            if !taken.contains(name) {
                // the line of the block's first statement, which comes right after it
                let line = b.first().map_or(0, |s| s.line);
                out.push(Untaken { line, what: ritsu_base::tr!("ワークフローの `{name}`", "the workflow's `{name}`"), never: None });
            }
        }
    }
    if !arms.is_empty() {
        let matched = if m.rules.iter().any(|r| matches!(r.kind, RuleKind::Rule)) { crate::crossings::matched_outputs(m) } else { BTreeMap::new() };
        let proved = |a: &TArm| !a.none && a.some.is_none() && !a.values.is_empty();
        // what rulec knows of each rule asked of, and each question: a rule, what the calls give it,
        // and the outputs the `match`es with an arm no scenario takes read of it
        let mut facts: BTreeMap<std::path::PathBuf, Option<ritsu_ports::RuleFacts>> = BTreeMap::new();
        let mut questions: BTreeMap<Asked, BTreeSet<String>> = BTreeMap::new();
        for (site, _, a) in &arms {
            let Some(calls) = matched.get(site).filter(|r| r.only_outputs && proved(a)).and_then(|r| r.calls.as_ref()) else { continue };
            for (_, call) in calls {
                let f = facts.entry(call.rule.clone()).or_insert_with(|| crate::sources::rule(&call.rule).ok());
                if let Some(f) = f {
                    questions.entry((call.rule.clone(), call.held_ranges(Some(f)))).or_default().insert(call.output.clone());
                }
            }
        }
        let answers: BTreeMap<Asked, BTreeMap<String, BTreeSet<String>>> = questions.into_iter().filter_map(|(q, outputs)| {
            let f = facts.get(&q.0)?.as_ref()?;
            let outputs: Vec<String> = outputs.into_iter().collect();
            let ritsu_ports::Found::Value(combos) = crate::sources::outputs_together(&q.0, &outputs, &q.1).ok()? else { return None };
            let mut each: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
            for (k, o) in outputs.iter().enumerate() {
                let set = each.entry(o.clone()).or_default();
                for (combo, _) in &combos {
                    match combo.get(k) {
                        Some(ritsu_ports::Value::Bool(b)) => {
                            set.insert(b.to_string());
                        }
                        // an enum's value by its public name, as rulec answers it: by the rule's name of it
                        Some(ritsu_ports::Value::Enum(p)) => {
                            set.insert(f.enums.iter().flat_map(|e| e.values.iter()).find(|v| v.public == *p).map_or_else(|| p.clone(), |v| v.name.clone()));
                        }
                        _ => {}
                    }
                }
            }
            Some((q, each))
        }).collect();
        for (site, line, a) in arms {
            let shown = a.values.iter().cloned().chain(a.none.then(|| "none".to_string())).chain(a.some.as_ref().map(|x| format!("some {x}"))).collect::<Vec<_>>().join(", ");
            let what = ritsu_base::tr!("{line} 行目の `match` の分岐 `{shown}`", "the arm `{shown}` of the `match` on line {line}");
            let never = matched.get(&site).filter(|r| r.only_outputs && proved(a)).and_then(|r| never_comes(m, r.calls.as_ref()?, &a.values, &facts, &answers));
            out.push(Untaken { line: a.line, what, never });
        }
    }
    out.sort_by_key(|u| u.line);
    out
}

/// A question `untaken` asks rulec: the rule's file, and what the calls give its inputs, as
/// `Rules::outputs_together` takes it.
type Asked = (std::path::PathBuf, Vec<(String, Option<i128>, Option<i128>)>);

/// Why no run comes to an arm that takes `values` of a `match` on the answers of `calls` (each by
/// the site of its statement): rulec, asked over what the runs that reach each call can give the
/// rule, answers exactly (`answers`), and none of its answers are among `values`. None where one of
/// them is, or rulec does not answer exactly.
fn never_comes(m: &Model, calls: &[(usize, ritsu_ports::OutputCall)], values: &[String], facts: &BTreeMap<std::path::PathBuf, Option<ritsu_ports::RuleFacts>>, answers: &BTreeMap<Asked, BTreeMap<String, BTreeSet<String>>>) -> Option<Text> {
    let stmts: BTreeMap<usize, &TStmt> = m.all_stmts().into_iter().map(|s| (s.site, s)).collect();
    let mut said_ja: Vec<String> = Vec::new();
    let mut said_en: Vec<String> = Vec::new();
    for (site, call) in calls {
        let TK::Call { callee: Callee::Rule(r), .. } = &stmts.get(site)?.kind else { return None };
        let f = facts.get(&call.rule)?.as_ref()?;
        let comes = answers.get(&(call.rule.clone(), call.held_ranges(Some(f))))?.get(&call.output)?;
        if values.iter().any(|v| comes.contains(v)) {
            return None;
        }
        let (rule, output, line) = (&m.rules[*r].name, &call.output, call.line);
        said_ja.push(format!("{line} 行目でワークフローが渡しうる値では、規則 {rule} の `{output}` は {} になりません", values.join(" にも ")));
        let or = match values.split_last() {
            Some((last, rest)) if !rest.is_empty() => format!("{} or {last}", rest.join(", ")),
            _ => values.join(""),
        };
        said_en.push(format!("the rule {rule} never comes to `{output}` = {or} with what the workflow can give it on line {line}"));
    }
    if said_en.is_empty() {
        return None;
    }
    Some(Text::new(format!("rulec によると、{}", said_ja.join("。また、")), format!("rulec says {}", said_en.join(", and "))))
}

/// The ports of rules, dates and books, as `ritsu dandori` hands them over (`sources::Held::ports`).
type Ports = (std::rc::Rc<dyn ritsu_ports::Rules>, std::rc::Rc<dyn ritsu_ports::Dates>, std::rc::Rc<dyn ritsu_ports::Books>);

/// Whether `ritsu run` runs the scenario to the end the walk came to (without the three ports,
/// `dandori run` with the ports it has: the rules of a program that joins rulec only, or none, where
/// every answer comes from the scenario): it does not stop, it reads every answer the scenario has,
/// it ends as the walk did (`end`, which the walk wrote), and it passes every place the scenario
/// says it `covers` (`unmet`). If not, what differs.
fn runs_as_walked(m: &Model, sc: &Value, ports: Option<&Ports>) -> Result<(), String> {
    let mut sc = sc.clone();
    let walked = sc.as_object_mut().and_then(|o| o.remove(WALKED_END));
    let given = sc["answers"].as_array().map_or(0, |a| a.len());
    let computed = match ports {
        Some((r, d, b)) => Some(crate::computed::run(m, &sc, render::View::Asl, r.clone(), d.clone(), b.clone())),
        None => crate::computed::said_ports(&sc).map(|(r, d, b)| crate::computed::run_said(m, &sc, render::View::Asl, r, d, b)),
    };
    let (trace, visits, read) = match computed {
        Some(ran) => {
            let ran = ran.map_err(|e| match e {
                crate::computed::Stopped::Scenario(t) => format!("the scenario cannot be run: {}", t.en),
                crate::computed::Stopped::Run(e) => format!("the run stops: {}", e.en),
            })?;
            let read = ran.events.iter().filter(|e| matches!(e, crate::computed::Event::Answer { who: crate::computed::Who::Scenario, .. })).count();
            (ran.trace, ran.visits, if ran.unused.is_some() { usize::MAX } else { read })
        }
        None => {
            let ran = crate::interp::run_moving_visits(m, &sc, render::View::Asl).map_err(|e| format!("the run stops: {}", e.en))?;
            let read = given - ran.unused.map_or(0, |u| u.answers.len());
            (ran.trace, ran.visits, read)
        }
    };
    if read != given {
        return Err(format!("the run does not take its {given} answers, and no more"));
    }
    let end = trace["end"].as_object().and_then(|o| o.keys().next().cloned()).unwrap_or_default();
    if let Some(w) = walked.filter(|w| *w != json!(end)) {
        return Err(format!("the run ends with {end}, the walk with {w}"));
    }
    match unmet(m, &sc, &visits, &trace["end"]) {
        un if un.is_empty() => Ok(()),
        un => Err(format!("the run does not pass {}", un.join(", "))),
    }
}

/// The names of the scenario's `covers` that the run did not pass, from what the reference
/// interpreter says it went through (`interp::Visit`) and how the run ended (`end`, as the trace
/// has it) — each name a place of the flow and the way the walk took it there:
/// - `S:armI`: the match at S took its arm I;
/// - `S:succeed`, `S:fail`: the run came to the `succeed` or the `fail` at S;
/// - `S:outside`: the check of the period before S found `now` outside it;
/// - `S:holds`, `S:breaks`: the check of a rule's precondition at S found the values keep it, or
///   break it;
/// - `S:empty`, `S:too many`, `S:break`, `S:ran`, `S:ran again`, `S:exhausted`: a loop at S over no
///   item, over more items than it takes, left by a `break`, that ran a round to its end, two
///   rounds, every round of a `repeat`;
/// - `S:a round fails`, `S:every round done`, `S:every one of several rounds done`, `S:cancelled
///   after a round failed`: the rounds of a parallel loop at S;
/// - the ways of a call at S: an answer the run went on with (`ok`, a case's state, `done before`,
///   the zero values of an answer, `recased`; and a rule's, a date's or a book's answer, `rulec`,
///   `koyomi`, `chobo`), a try that failed before one that did not (`retried`), an error a handler
///   took (`on:K`, by the first error the handler names) or nothing at the call took
///   (`unhandled`), an answer that fails the call (`refused:…`, `less sure:…`), one its checks
///   refuse (`malformed`, `out of range`, `unexpected state`, `rule enum left out`, `partial
///   date`), a cancellation (`cancelled`);
/// - `on failure`, `on cancel`; and `A then B`, a loop's name and an end, both passed;
/// - the input and the start: the run fails at the door, before anything, with an input outside
///   its range (`input:outside`), without a field it needs (`input:absent`) or with a date short
///   of its day (`input:partial date`), each `Dandori.BadInput`, or with a start outside the
///   period (`now:outside`,
///   `Dandori.OutOfPeriod`); or it goes in with an input whose JSON leaves out its zero values
///   (`input:zeros`), its `json` values and what is none (`input:left out`), or with a date or a
///   time at an end of its range (`input:at an end`).
///
/// What an answer holds beyond the way it came back (which state, which zero values) is not
/// compared here: it shows in what the run does next, and in how it ends.
fn unmet(m: &Model, sc: &Value, visits: &[crate::interp::Visit], end: &Value) -> Vec<String> {
    let stmts: BTreeMap<usize, &TStmt> = m.all_stmts().into_iter().map(|s| (s.site, s)).collect();
    let error = end["fail"]["error"].as_str();
    sc["covers"].as_array().into_iter().flatten().filter_map(|c| c.as_str()).filter(|c| !passes(m, &stmts, sc, visits, error, c)).map(String::from).collect()
}

fn passes(m: &Model, stmts: &BTreeMap<usize, &TStmt>, sc: &Value, visits: &[crate::interp::Visit], error: Option<&str>, label: &str) -> bool {
    use crate::interp::Visit as V;
    if let Some((a, b)) = label.split_once(" then ") {
        return passes(m, stmts, sc, visits, error, a) && passes(m, stmts, sc, visits, error, b);
    }
    // the input goes through the door, or the run fails there, before any statement
    let at_door = |e: &str| error == Some(e) && !visits.iter().any(|v| matches!(v, V::Answer(..)));
    let went_in = error != Some("Dandori.BadInput");
    match label {
        "on failure" => return visits.contains(&V::OnFailure),
        "on cancel" => return visits.contains(&V::OnCancel),
        "input:outside" | "input:absent" | "input:partial date" => return at_door("Dandori.BadInput"),
        "now:outside" => return at_door(OUT_OF_PERIOD),
        // a zero value left out, not a date written as protobuf's JSON writes it
        "input:zeros" => return went_in && m.service.as_ref().is_some_and(|s| crate::apis::fill(&sc["input"], &crate::apis::without_days(&s.input_zeros)) != sc["input"]),
        "input:left out" => return went_in && m.inputs.iter().any(|(n, t)| sc["input"].get(n).map_or(matches!(t, Ty::Json | Ty::Opt(_)), |v| leaves_out(m, v, t))),
        "input:at an end" => return went_in,
        _ => {}
    }
    let Some((site, way)) = label.split_once(':') else { return false };
    let Ok(site) = site.parse::<usize>() else { return false };
    let Some(s) = stmts.get(&site) else { return false };
    // each time the statement runs: what was gone through from its start to its next start
    let times: Vec<&[V]> = {
        let starts: Vec<usize> = visits.iter().enumerate().filter(|(_, v)| **v == V::Stmt(site)).map(|(i, _)| i).collect();
        starts.iter().enumerate().map(|(k, &i)| &visits[i..starts.get(k + 1).copied().unwrap_or(visits.len())]).collect()
    };
    let rounds = |t: &[V]| t.iter().filter(|v| matches!(v, V::Round(x, _) if *x == site)).count();
    let has = |t: &[V], v: &V| t.contains(v);
    match (&s.kind, way) {
        (TK::Match { .. }, w) if w.starts_with("arm") => w[3..].parse().is_ok_and(|i| visits.contains(&V::Arm(site, i))),
        (TK::Succeed { .. }, "succeed") | (TK::Fail { .. }, "fail") => !times.is_empty(),
        (TK::Period { start: false }, "outside") => visits.contains(&V::Unhandled(site)),
        (TK::Check(_), "holds") => visits.contains(&V::Check(site, true)),
        (TK::Check(_), "breaks") => visits.contains(&V::Check(site, false)),
        (TK::For { .. }, "too many") => visits.contains(&V::TooMany(site)),
        (TK::For { .. }, "empty") => times.iter().any(|t| rounds(t) == 0 && has(t, &V::Done(site))),
        (TK::For { .. } | TK::Repeat { .. }, "break") => visits.contains(&V::Broke(site)),
        (TK::Repeat { .. }, "exhausted") => visits.contains(&V::Done(site)),
        // a round ran to its end: the one after it started, or the loop ran out of items
        (TK::For { parallel: None, .. }, "ran" | "ran again") => {
            let at_least = if way == "ran" { 1 } else { 2 };
            times.iter().any(|t| rounds(t) - usize::from(!has(t, &V::Done(site)) && rounds(t) > 0) >= at_least)
        }
        (TK::For { parallel: Some(_), .. }, "a round fails") => visits.iter().any(|v| matches!(v, V::RoundFailed(x, _) if *x == site)),
        (TK::For { parallel: Some(_), .. }, "every round done" | "every one of several rounds done") => times.iter().any(|t| {
            let failed = t.iter().any(|v| matches!(v, V::RoundFailed(x, _) if *x == site));
            !failed && has(t, &V::Done(site)) && if way == "every round done" { rounds(t) == 1 } else { rounds(t) > 1 }
        }),
        (TK::For { parallel: Some(_), .. }, "cancelled after a round failed") => times.iter().any(|t| {
            let failed = t.iter().position(|v| matches!(v, V::RoundFailed(x, _) if *x == site));
            failed.is_some_and(|f| t[f..].iter().any(|v| matches!(v, V::Answer(_, k) if k == "cancel")))
        }),
        (TK::Call { handlers, .. }, w) => times.iter().any(|t| {
            let answers: Vec<&str> = t.iter().filter_map(|v| if let V::Answer(x, k) = v { (*x == site).then_some(k.as_str()) } else { None }).collect();
            let handled = |j: usize| has(t, &V::Handler(site, j));
            let failed = (0..handlers.len()).any(handled) || has(t, &V::Unhandled(site));
            let bad = has(t, &V::Bad(site));
            match w {
                "rulec" | "koyomi" | "chobo" => !answers.is_empty(),
                "retried" => answers.len() >= 2 && answers.last() == Some(&"ok") && answers[..answers.len() - 1].iter().any(|k| *k != "ok"),
                "unhandled" => has(t, &V::Unhandled(site)),
                "cancelled" => answers.contains(&"cancel"),
                "malformed" | "out of range" | "unexpected state" | "rule enum left out" | "partial date" => bad,
                w if w.starts_with("on:") => (0..handlers.len()).any(|j| handled(j) && handler_way(&handlers[j]) == w),
                w if w.starts_with("refused:") || w.starts_with("less sure:") => failed,
                // an answer the run went on with
                _ => answers.last() == Some(&"ok") && !bad && !failed,
            }
        }),
        _ => false,
    }
}

/// How the walk names the way a call's error goes to a handler: by the first error it names.
fn handler_way(h: &THandler) -> String {
    match h.errors.first() {
        Some(HErr::Declared(n)) => format!("on:{n}"),
        Some(HErr::Timeout) => "on:timeout".into(),
        _ => "on:failure".into(),
    }
}

/// Where a scenario of `generate_in` keeps the way its walk ended (`succeed`, `fail`, `cancel`),
/// until `generate_computed` holds `ritsu run` to it.
const WALKED_END: &str = "\u{1}end";

fn generate_in(m: &Model, computed: bool) -> Vec<Value> {
    let mut out = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut counts: BTreeMap<(String, usize), usize> = BTreeMap::new();
    // a run is a prefix of choices to replay, then its own choices; every run that reaches
    // something new becomes the ground for runs that change one of its choices
    // (with koyomi and chobo, a run that reaches nothing new is walked once more, after the choice it
    // changed, the way the flow goes first: what a language answers further on decides whether
    // something new is there, as the choice of an answer did before)
    // (a run that stuck to its choices in a loop that did not run out is walked again from each place
    // it first made a choice again, with the other alternatives there, once: `again`)
    let mut queue: std::collections::VecDeque<(Vec<usize>, bool, bool, bool)> = std::collections::VecDeque::new();
    queue.push_back((vec![], false, false, false));
    queue.push_back((vec![], true, false, false));
    let mut runs = 0;
    // the runs whose `now` is not the one `now_in` gives: a moment of its own for the start, or the
    // moments a run that leaves its period reads
    let mut nows: BTreeMap<usize, Value> = BTreeMap::new();
    // with the ports of dates and books (`ritsu dandori scenarios`), koyomi and chobo answer the
    // calls of dates and the operations of books, as `ritsu run` answers them
    let ports = if computed && (!m.books.is_empty() || m.rules.iter().any(|r| matches!(r.kind, RuleKind::Date(_)))) {
        crate::sources::held().and_then(|h| h.ports())
    } else {
        None
    };
    let start = json!({ "now": now_in(m) });
    let starts = if computed { start_moments(m, ports.as_ref().map(|(_, d, _)| d)) } else { vec![now_in(m)] };
    // with rulec, the outputs of the calls of rules that a `match` reads (`steer_rule`)
    let aimed = if computed && m.rules.iter().any(|r| matches!(r.kind, RuleKind::Rule)) {
        crate::crossings::matched_outputs(m).values().try_fold(BTreeMap::new(), |mut aimed: BTreeMap<usize, BTreeSet<String>>, read| {
            for (site, call) in read.calls.as_ref()? {
                aimed.entry(*site).or_default().insert(call.output.clone());
            }
            Some(aimed)
        })
    } else {
        Some(BTreeMap::new())
    };
    // the values a check of a rule's precondition reads, with koyomi the values a date is given,
    // and with rulec the values a rule is given whose answer a `match` reads, are left open where
    // they are made, so that the check or the call can steer them
    let ruled = aimed.as_ref().is_none_or(|a| !a.is_empty());
    let steered = m.all_stmts().iter().any(|s| matches!(s.kind, TK::Check(_))) || (ports.is_some() && m.rules.iter().any(|r| matches!(r.kind, RuleKind::Date(_)))) || ruled;
    let aims = std::cell::RefCell::new(Aims { aimed, ..Aims::default() });
    // on a clock that moves, a wait until a moment past the end of the period leaves it: the moment,
    // where it was left open, is steered in it or past it
    let moments = computed && m.period.as_ref().is_some_and(|p| p.range.hi.is_some()) && m.all_stmts().iter().any(|s| matches!(s.kind, TK::WaitUntil { .. }));
    while let Some((prefix, sticky, firsts, again)) = queue.pop_front() {
        if runs >= MAX_RUNS {
            break;
        }
        runs += 1;
        let mut ex = Ex {
            m,
            aims: &aims,
            counts: std::mem::take(&mut counts),
            prefix: prefix.clone(),
            trail: vec![],
            sticky,
            firsts,
            made: BTreeMap::new(),
            again: BTreeMap::new(),
            cut: false,
            labels: BTreeSet::new(),
            vars: BTreeMap::new(),
            holes: vec![],
            cases: vec![None; m.cases.len()],
            answers: vec![],
            in_on_failure: false,
            cancelled: false,
            in_on_cancel: false,
            steps: 0,
            stopped: false,
            par_depth: 0,
            pending: None,
            later: None,
            done_loops: vec![],
            made_values: 0,
            sparse: false,
            steered,
            moments,
            starts: &starts,
            computed,
            dead: false,
            clock: Clock::of(&start, computed).expect("the moment a run starts at is a time"),
            oracle: ports.as_ref().and_then(|(r, d, b)| Computed::new(m, &start, r.clone(), d.clone(), b.clone(), Takes { rules: false, dates: true, books: true }, Takes::default()).ok()),
            before_ops: m.books.iter().map(|_| Vec::new()).collect(),
            touched: BTreeSet::new(),
            keys: BTreeSet::new(),
            held: BTreeMap::new(),
            kept: BTreeSet::new(),
            checked: Vec::new(),
            ended: None,
        };
        ex.run();
        counts = std::mem::take(&mut ex.counts);
        if ex.dead {
            // a sticky run in a loop that went on with the same choices until it was cut, which
            // `ritsu run` cannot run to an end: the other alternatives where it first made a choice
            // again, so that a round can go another way than the one before it (the arm of a `match`
            // that goes around once more, then the one that leaves the loop)
            if ex.cut && sticky && !again {
                for j in ex.again.values().copied().filter(|j| *j >= prefix.len()).collect::<BTreeSet<_>>() {
                    let (choice, n) = ex.trail[j];
                    for a in (0..n).filter(|a| *a != choice) {
                        let mut p: Vec<usize> = ex.trail[..j].iter().map(|(c, _)| *c).collect();
                        p.push(a);
                        queue.push_back((p, true, false, true));
                    }
                }
            }
            continue;
        }
        let new: Vec<String> = ex.labels.iter().filter(|l| !seen.contains(*l)).cloned().collect();
        if new.is_empty() {
            if ports.is_some() && !firsts && !prefix.is_empty() {
                queue.push_back((prefix, sticky, true, again));
            }
            continue;
        }
        seen.extend(new);
        for i in prefix.len()..ex.trail.len() {
            let (choice, n) = ex.trail[i];
            for a in 0..n {
                if a != choice {
                    let mut p: Vec<usize> = ex.trail[..i].iter().map(|(c, _)| *c).collect();
                    p.push(a);
                    queue.push_back((p, true, false, again));
                }
            }
        }
        let input = ex.finish_input();
        let answers: Vec<Value> = ex
            .answers
            .iter()
            .map(|a| {
                let filled = ex.fill(a);
                // a Claude agent's answer with its enum values in another case, as Claude may give them
                if let Some(t) = a.get("recase").and_then(|t| t.as_u64()) {
                    return json!({ "ok": render::recase(m, &filled["ok"], m.tasks[t as usize].result.as_ref().unwrap_or(&Ty::Json)) });
                }
                // Jev's response, whose answers read as the value chosen: as sure as the task asks, or less
                if let Some(j) = a.get("jev").and_then(|t| t.as_u64()).and_then(|t| m.tasks[t as usize].jev()) {
                    return json!({ "ok": render::jev_wire(j, &filled["ok"], a.get("low") == Some(&json!(true)), a.get("refused") == Some(&json!(true))) });
                }
                // a date's time: the day chosen, at the time of the day the date says, at its calendar's
                // offset, as koyomi's code makes it
                if let Some(d) = a.get("date").and_then(|r| r.as_u64()).and_then(|r| m.rules[r as usize].date()) {
                    let mut v = filled["ok"].clone();
                    if let (true, Some(mins), Some(day)) = (v.get("at").is_some(), d.minutes, v["day"].as_str().and_then(crate::computed::day_of)) {
                        v["at"] = json!(crate::computed::time_of_day(day, mins, d.offset.unwrap_or(0)));
                    }
                    return json!({ "ok": v });
                }
                // a rule's service writes the record chosen as protobuf's JSON: zero values left out, numbers as strings
                if let Some(c) = a.get("rule").and_then(|r| r.as_u64()).and_then(|r| m.rules[r as usize].connect.as_ref()) {
                    return json!({ "ok": render::rule_wire(c, &filled["ok"]) });
                }
                // the value of an event or a callback, as protobuf's JSON writes the request a method of the service sends
                if let Some(z) = a.get("omit") {
                    return json!({ "ok": crate::apis::omit_zeros(&filled["ok"], z) });
                }
                // a Connect answer's dates, as protobuf's JSON writes them: `{year, month, day}`
                if let Some(z) = a.get("days") {
                    return json!({ "ok": crate::apis::wire_days(&filled["ok"], z) });
                }
                filled
            })
            .collect();
        let mut sc = json!({ "name": format!("run {}", out.len() + 1), "input": input, "answers": answers, "covers": ex.labels.iter().collect::<Vec<_>>() });
        // what the run does to the books before it starts, by the book's place (`generate_computed`
        // writes it by the book's name)
        if ex.oracle.is_some() && !m.books.is_empty() {
            sc["books"] = Value::Object(ex.before_ops.iter().enumerate().map(|(b, ops)| (b.to_string(), Value::Array(ops.clone()))).collect());
        }
        if computed {
            sc[WALKED_END] = json!(ex.ended.unwrap_or("succeed"));
        }
        out.push(sc);
        // a run whose `now` after the start is outside the period: it starts in it, and reads the
        // moment after it; and a run that starts at a moment of its own
        match ex.clock.given() {
            [one] if *one == starts[0] => {}
            [one] => {
                nows.insert(out.len() - 1, json!(one));
            }
            given => {
                nows.insert(out.len() - 1, json!(given));
            }
        }
    }
    // the runs the walk kept, before the ones made from them
    let walked = out.len();
    // a workflow that implements a service: a request without a field the workflow needs, which
    // says whether it is set, fails the run at once (DESIGN 1.14)
    if let Some(n) = m.service.as_ref().and_then(|s| needed_presence(m, &s.input_zeros)) {
        let whole = out.iter().find(|r| !r["covers"].as_array().is_some_and(|c| c.iter().any(|l| l == "input:zeros")));
        if let Some(mut input) = whole.map(|r| r["input"].clone()) {
            if let Some(o) = input.as_object_mut() {
                o.remove(&n);
            }
            out.push(json!({ "name": format!("run {}", out.len() + 1), "input": input, "answers": [], "covers": ["input:absent"] }));
        }
    }
    // a workflow that implements a service and takes a `google.type.Date`: a request whose date is
    // short of its day, as protobuf's JSON writes one whose day is 0, which is no date, and which
    // every platform turns away at once (`Dandori.BadInput`, DESIGN 44)
    if let Some(s) = &m.service {
        let days: Vec<&str> = s.input_zeros["d"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        let whole = out[..walked].iter().find(|r| !r["covers"].as_array().is_some_and(|c| c.iter().any(|l| l == "input:zeros")));
        if let Some(mut input) = whole.map(|r| r["input"].clone()) {
            let short = days.iter().any(|d| match input.get_mut(*d) {
                Some(Value::Object(o)) => o.remove("day").is_some(),
                _ => false,
            });
            if short {
                out.push(json!({ "name": format!("run {}", out.len() + 1), "input": input, "answers": [], "covers": ["input:partial date"] }));
            }
        }
    }
    // a workflow that holds an input to a range, a number or an ordered enum, or a field of an input's
    // record: for each of them, a run's input with that one alone just outside its range, which every
    // platform turns away at once (`Dandori.BadInput`), so that each check of a range at the door is
    // tried on every platform (DESIGN 9)
    for (at, x) in ranged_inputs(m) {
        if let Some(input) = out[..walked].iter().find_map(|r| put_at(&r["input"], &at, &x)) {
            out.push(json!({ "name": format!("run {}", out.len() + 1), "input": input, "answers": [], "covers": ["input:outside"] }));
        }
    }
    // a workflow that takes a `json`, as an input or as a field of one: the run that goes furthest again,
    // its input as protobuf's JSON writes it when nothing is set. What may be absent is left out, a `json`
    // and a `T?` that is none, and it reads as null on every platform (DESIGN 1.2)
    if m.inputs.iter().any(|(_, t)| *t == Ty::Json || has_json_field(m, t)) {
        let calls = |r: &Value| r["answers"].as_array().map_or(0, |a| a.len());
        let furthest = out.iter().enumerate().max_by(|(i, a), (j, b)| calls(a).cmp(&calls(b)).then(j.cmp(i))).map(|(i, r)| (i, r.clone()));
        if let Some((from, run)) = furthest {
            let mut input = Map::new();
            for (n, t) in &m.inputs {
                let v = &run["input"][n.as_str()];
                match t {
                    Ty::Json => {}
                    Ty::Opt(_) if v.is_null() => {}
                    _ => {
                        input.insert(n.clone(), without_json(m, v, t, true));
                    }
                }
            }
            let mut sc = json!({ "name": format!("run {}", out.len() + 1), "input": input, "answers": run["answers"], "covers": ["input:left out"] });
            // what the run did to the books before it, and when it started, are the run's
            if let Some(b) = run.get("books") {
                sc["books"] = b.clone();
            }
            out.push(sc);
            if let Some(at) = nows.get(&from).cloned() {
                nows.insert(out.len() - 1, at);
            }
        }
    }
    // a workflow that holds a date or a time of its input to a range: for each end of each, the
    // first run again with that one value at the end, which every platform lets in, so that a
    // platform that takes an end for outside shows. The run calls what the first run does, as a
    // date or a time decides no arm; one that checks a rule's precondition, or that rulec answers
    // the calls of rules of, may go another way with it, and has none of these runs
    let steered = m.all_stmts().iter().any(|s| matches!(s.kind, TK::Check(_)));
    if !computed && !steered {
        for (at, x) in edge_inputs(m) {
            if let Some(r) = out[..walked].first() {
                if let Some(input) = put_at(&r["input"], &at, &x) {
                    let answers = r["answers"].clone();
                    out.push(json!({ "name": format!("run {}", out.len() + 1), "input": input, "answers": answers, "covers": ["input:at an end"] }));
                }
            }
        }
    }
    // a workflow with a period: a run that starts just outside it, which every platform fails at
    // once (`Dandori.OutOfPeriod`), before it calls anything, as an input outside its range
    if let (Some(at), Some(r)) = (now_outside(m), out[..walked].first()) {
        let input = r["input"].clone();
        out.push(json!({ "name": format!("run {}", out.len() + 1), "input": input, "answers": [], "covers": ["now:outside"] }));
        nows.insert(out.len() - 1, json!(at));
    }
    // the moment `now` reads, which the runners give the platforms' clocks
    if m.uses_now() {
        let at = now_in(m);
        for (i, sc) in out.iter_mut().enumerate() {
            sc["now"] = nows.get(&i).cloned().unwrap_or_else(|| json!(at));
        }
    }
    out
}

/// A value of the enum `e` that the range `rg` (places in the enum's order) does not let in: the one
/// just past its high end, else the one just before its low end. None for a range that lets in every
/// value.
fn enum_outside(m: &Model, e: EnumId, rg: Range) -> Option<String> {
    let values = &m.enums[e].values;
    let n = values.len() as i64;
    let (lo, hi) = (rg.lo.unwrap_or(0), rg.hi.unwrap_or(n - 1));
    let at = if hi + 1 < n {
        hi + 1
    } else if lo > 0 {
        lo - 1
    } else {
        return None;
    };
    values.get(at as usize).cloned()
}

/// A number just outside the range `rg`, as an enum's value is chosen: the one just past its high
/// end, else the one just before its low end (a range with one end has room only beyond it). None
/// for a range with no end.
fn number_outside(rg: Range) -> Option<i64> {
    rg.hi.and_then(|hi| hi.checked_add(1)).or_else(|| rg.lo.and_then(|lo| lo.checked_sub(1)))
}

/// One step of the way from a run's input to a value in it: an input or a field, by its name, or
/// the first item of a list.
#[derive(Clone)]
enum Step {
    Name(String),
    First,
}

/// Every place of the inputs that holds a number, an ordered enum's value, a date or a time to a
/// range with room outside it, in the order the inputs and the fields are declared: an input, a
/// field of an input's record at any depth, the items of a list (as its first one), what a `T?`
/// holds. Each is the way to it, and the value just outside its range there: past its high end
/// (the next day, the first second after it), else before its low end.
fn ranged_inputs(m: &Model) -> Vec<(Vec<Step>, Value)> {
    input_places(m, &|t, rg| {
        let v = match t {
            Ty::Int | Ty::Num(_) => rg.and_then(number_outside).map(|n| json!(n)),
            Ty::Enum(e) => rg.and_then(|r| enum_outside(m, *e, r)).map(|v| json!(v)),
            Ty::Date => rg.and_then(number_outside).map(|d| json!(day_text(d))),
            Ty::Timestamp => rg.and_then(number_outside).map(|t| json!(moment_text(t))),
            _ => None,
        };
        v.into_iter().collect()
    })
}

/// Every place of the inputs that holds a date or a time to a range, as `ranged_inputs` finds them,
/// with each end of its range there: the first day and the last, the first second and the last
/// with a fraction of a second after them, which the comparison of the platforms' code keeps in
/// their second. The values in the range a platform's comparison would turn away if it took an
/// end for outside, or read a time's fraction against the end.
fn edge_inputs(m: &Model) -> Vec<(Vec<Step>, Value)> {
    input_places(m, &|t, rg| {
        let Some(r) = rg else { return vec![] };
        let mut ends: Vec<Value> = match t {
            Ty::Date => [r.lo, r.hi].into_iter().flatten().map(|d| json!(day_text(d))).collect(),
            Ty::Timestamp => [r.lo, r.hi].into_iter().flatten().map(|s| json!(format!("{}.5Z", moment_head(s)))).collect(),
            _ => vec![],
        };
        ends.dedup();
        ends
    })
}

/// The places of the inputs, in the order the inputs and the fields are declared, with the values
/// `at` gives for the type and the range there: see `ranged_inputs`.
fn input_places(m: &Model, values: &dyn Fn(&Ty, Option<Range>) -> Vec<Value>) -> Vec<(Vec<Step>, Value)> {
    fn walk(m: &Model, t: &Ty, rg: Option<Range>, values: &dyn Fn(&Ty, Option<Range>) -> Vec<Value>, at: &mut Vec<Step>, out: &mut Vec<(Vec<Step>, Value)>) {
        match t {
            Ty::Opt(x) => walk(m, x, rg, values, at, out),
            Ty::List(x) => {
                at.push(Step::First);
                walk(m, x, rg, values, at, out);
                at.pop();
            }
            Ty::Record(r) => {
                for (f, ft) in &m.records[*r].fields {
                    at.push(Step::Name(f.clone()));
                    walk(m, ft, m.field_range(*r, f), values, at, out);
                    at.pop();
                }
            }
            _ => {
                for v in values(t, rg) {
                    out.push((at.clone(), v));
                }
            }
        }
    }
    let mut out = Vec::new();
    for (n, t) in &m.inputs {
        let mut at = vec![Step::Name(n.clone())];
        walk(m, t, m.input_ranges.get(n).copied(), values, &mut at, &mut out);
    }
    out
}

/// The moment the runs of a workflow read `now` at, which the runners give the platforms' clocks:
/// `SCENARIO_NOW`, the last day of a month at 15:30:00Z, which is the first of the next month east
/// of UTC, so that a platform that reads a moment in the wrong offset shows. A workflow with a
/// period that does not hold it reads the last day of a month at 15:30:00Z nearest to it in the
/// period, else the period's first second (DESIGN 20).
pub fn now_in(m: &Model) -> String {
    let Some(p) = &m.period else { return render::SCENARIO_NOW.into() };
    let r = p.range;
    let t0 = moment_of(render::SCENARIO_NOW).unwrap_or_default();
    if r.contains(t0) {
        return render::SCENARIO_NOW.into();
    }
    // the last day of the month the day `d` is in, at 15:30:00Z
    let end_of = |d: i64| -> Option<i64> {
        let text = day_text(d);
        let (y, mo): (i64, i64) = (text.get(..4)?.parse().ok()?, text.get(5..7)?.parse().ok()?);
        let (ny, nm) = if mo == 12 { (y + 1, 1) } else { (y, mo + 1) };
        let next = ritsu_ports::day_of(&format!("{ny:04}-{nm:02}-01"))?;
        Some((next - 1) * 86_400 + 15 * 3600 + 30 * 60)
    };
    let near = if t0 < r.lo.unwrap_or(i64::MIN) {
        // the first such moment at or after the low end: in its month, else (the low end is after
        // it, on the month's last day) in the next
        let lo = r.lo.unwrap_or_default();
        let d = lo.div_euclid(86_400);
        end_of(d).filter(|t| *t >= lo).or_else(|| end_of(d + 1))
    } else {
        // the last such moment at or before the high end: in its month, else in the month before
        let hi = r.hi.unwrap_or_default();
        end_of(hi.div_euclid(86_400)).filter(|t| *t <= hi).or_else(|| {
            let text = day_text(hi.div_euclid(86_400));
            let first_day = ritsu_ports::day_of(&format!("{}-01", text.get(..7)?))?;
            end_of(first_day - 1)
        })
    };
    match near.filter(|t| r.contains(*t)) {
        Some(t) => moment_text(t),
        None => moment_text(r.lo.or(r.hi).unwrap_or(t0)),
    }
}

/// A moment just outside the period of a workflow: the first second after its end, else the last
/// second before its start. None for a workflow with no period.
pub fn now_outside(m: &Model) -> Option<String> {
    m.period.as_ref().and_then(|p| number_outside(p.range)).map(moment_text)
}

/// The moments a run of `generate_computed` may start at: the one every run starts at (`now_in`);
/// where koyomi computes the dates a run gives `now`, and what the day is decides where the run
/// goes, the middle of its month, where a date that counts from a day of the month (a closing day)
/// falls on its other side, and in a workflow without a period, where `now` may be any moment, the
/// day after the last day a date given `now` takes, which koyomi refuses; and in a period with an
/// end, its last moment, where whatever takes time (a wait, a timeout, a retry) takes the run past
/// the end, and the next statement that reads `now` finds it outside. A moment outside the period
/// is left out. A workflow that does not read `now` starts at the first alone.
fn start_moments(m: &Model, dates: Option<&std::rc::Rc<dyn ritsu_ports::Dates>>) -> Vec<String> {
    let first = now_in(m);
    let mut out = vec![first.clone()];
    if !m.uses_now() {
        return out;
    }
    let mut add = |at: String| {
        if m.period.as_ref().is_none_or(|p| render::moment_in(&at, &p.range)) && !out.contains(&at) {
            out.push(at);
        }
    };
    if m.calls_dates() {
        add(format!("{}-15T15:30:00Z", &first[..7]));
    }
    if let (None, Some(d), true) = (&m.period, dates, m.calls_dates()) {
        for s in m.all_stmts() {
            let TK::Call { callee: Callee::Rule(r), args, .. } = &s.kind else { continue };
            let Some(dc) = m.rules[*r].date() else { continue };
            let Ok(facts) = d.facts(&m.rules[*r].info.path) else { continue };
            for (name, _, _) in dc.params.iter().filter(|(_, _, day)| *day) {
                if args.iter().any(|(n, e)| n == name && matches!(e, TExpr::Now)) {
                    if let Some(i) = facts.inputs.iter().find(|i| i.name == *name) {
                        add(format!("{}T15:30:00Z", day_text(i.max + 1)));
                    }
                }
            }
        }
    }
    if let Some(hi) = m.period.as_ref().and_then(|p| p.range.hi) {
        add(moment_text(hi));
    }
    out
}

/// `v` with `x` put at the end of the way `at`: None when the way goes through what `v` does not
/// have (a record that is absent, a list that is empty). The last name is put whether `v` has it
/// or not, as a request that leaves its zero values out goes without them.
fn put_at(v: &Value, at: &[Step], x: &Value) -> Option<Value> {
    match at.split_first() {
        None => Some(x.clone()),
        Some((Step::Name(n), rest)) => {
            let mut o = v.as_object()?.clone();
            let y = if rest.is_empty() { x.clone() } else { put_at(o.get(n)?, rest, x)? };
            o.insert(n.clone(), y);
            Some(Value::Object(o))
        }
        Some((Step::First, rest)) => {
            let mut items = v.as_array()?.clone();
            items[0] = put_at(items.first()?, rest, x)?;
            Some(Value::Array(items))
        }
    }
}

/// The zero values `z` names (`apis::zeros_of`) that an answer of the type `t` can leave out of its
/// JSON and still be read as a value of `t`: each one that fits its field, as the answer's check
/// reads it (`render::value_fits`). Not an enum's value 0 that says it is not set, which no value of
/// the flow's enum is, nor a 0 that a number's range leaves out: an answer without them fails its
/// check.
fn readable_zeros(m: &Model, z: &Value, t: &Ty) -> Value {
    let r = match t {
        Ty::Opt(x) | Ty::List(x) => return readable_zeros(m, z, x),
        Ty::Record(r) => *r,
        _ => return z.clone(),
    };
    let field = |k: &str| m.records[r].fields.iter().find(|(f, _)| f == k).map(|(_, ft)| ft);
    let mut out = Map::new();
    // the dates are written as protobuf's JSON writes them, and read back as they were
    if let Some(d) = z.get("d") {
        out.insert("d".into(), d.clone());
    }
    if let Some(f) = z["f"].as_object() {
        let kept: Map<String, Value> = f.iter().filter(|(k, zero)| field(k).is_none_or(|ft| render::value_fits(m, zero, ft, m.field_range(r, k)))).map(|(k, v)| (k.clone(), v.clone())).collect();
        out.insert("f".into(), Value::Object(kept));
    }
    for key in ["m", "l"] {
        if let Some(sub) = z[key].as_object() {
            out.insert(key.into(), Value::Object(sub.iter().map(|(k, s)| (k.clone(), field(k).map_or_else(|| s.clone(), |ft| readable_zeros(m, s, ft)))).collect()));
        }
    }
    Value::Object(out)
}

/// Whether the zero values `z` names have one that protobuf's JSON leaves out (`apis::sparse`): a
/// field that is no list or map, here or in a message inside.
fn leaves_some(z: &Value) -> bool {
    z["f"].as_object().is_some_and(|f| f.values().any(|x| !x.is_array() && !x.is_object())) || ["m", "l"].iter().any(|k| z[*k].as_object().is_some_and(|s| s.values().any(leaves_some)))
}

/// A field at the top of an answer of the type `t` that the flow reads as a `date` where the zero
/// values `z` say the message holds a `google.type.Date`: the one a `partial date` answer sends short
/// of its day (DESIGN 44).
fn day_at_top(m: &Model, t: &Ty, z: &Value) -> Option<String> {
    let Ty::Record(r) = t else { return None };
    let days = z["d"].as_array()?;
    m.records[*r].fields.iter().find(|(f, ft)| matches!(ft.inner(), Ty::Date) && days.iter().any(|d| d == f)).map(|(f, _)| f.clone())
}

/// Whether a value of type `t` holds a record with a `json` field, at any depth. (The items of a
/// `list[json]` are values, not fields that may be left out.)
fn has_json_field(m: &Model, t: &Ty) -> bool {
    match t {
        Ty::Opt(x) | Ty::List(x) => has_json_field(m, x),
        Ty::Record(r) => m.records[*r].fields.iter().any(|(_, ft)| *ft == Ty::Json || has_json_field(m, ft)),
        _ => false,
    }
}

/// Whether `v`, a value of type `t`, leaves out a field of a record in it, at any depth, that may be
/// absent: a `json` or a `T?` (what `without_json` leaves out with `nones`).
fn leaves_out(m: &Model, v: &Value, t: &Ty) -> bool {
    match (t, v) {
        (Ty::Opt(x), _) => leaves_out(m, v, x),
        (Ty::List(x), Value::Array(a)) => a.iter().any(|i| leaves_out(m, i, x)),
        (Ty::Record(r), Value::Object(o)) => m.records[*r].fields.iter().any(|(f, ft)| o.get(f).map_or(matches!(ft, Ty::Json | Ty::Opt(_)), |x| leaves_out(m, x, ft))),
        _ => false,
    }
}

/// `v`, a value of type `t`, as protobuf's JSON writes it when no `json` field in it is set: the
/// field is left out, in every record at every depth. With `nones`, so is a `T?` field that is none.
fn without_json(m: &Model, v: &Value, t: &Ty, nones: bool) -> Value {
    match (t, v) {
        (Ty::Opt(x), _) => without_json(m, v, x, nones),
        (Ty::List(x), Value::Array(a)) => Value::Array(a.iter().map(|i| without_json(m, i, x, nones)).collect()),
        (Ty::Record(r), Value::Object(o)) => {
            let mut out = Map::new();
            for (k, x) in o {
                match m.records[*r].fields.iter().find(|(f, _)| f == k).map(|(_, ft)| ft) {
                    Some(Ty::Json) => {}
                    Some(Ty::Opt(_)) if nones && x.is_null() => {}
                    Some(ft) => {
                        out.insert(k.clone(), without_json(m, x, ft, nones));
                    }
                    None => {
                        out.insert(k.clone(), x.clone());
                    }
                }
            }
            Value::Object(out)
        }
        _ => v.clone(),
    }
}

/// The first input, not `T?`, that a field of the request holds which says whether it is set (a
/// message, `optional`, a member of a `oneof`: none that `zeros` fills). A `json` input is passed
/// over: one that is not there is null, and the run goes on (the run with the input left out).
fn needed_presence(m: &Model, zeros: &Value) -> Option<String> {
    m.inputs.iter().find(|(n, t)| !matches!(t, Ty::Opt(_) | Ty::Json) && zeros["f"].get(n.as_str()).is_none()).map(|(n, _)| n.clone())
}

impl<'a> Ex<'a> {
    /// Take the alternative of this choice that the runs so far have taken least.
    fn choose(&mut self, at: &str, n: usize) -> usize {
        let n = n.max(1);
        let pos = self.trail.len();
        if pos < self.prefix.len() {
            let c = self.prefix[pos].min(n - 1);
            self.trail.push((c, n));
            self.made.insert(at.to_string(), c);
            return c;
        }
        if self.sticky {
            if let Some(c) = self.made.get(at).cloned() {
                if c < n {
                    self.again.entry(at.to_string()).or_insert(pos);
                    self.trail.push((c, n));
                    return c;
                }
            }
        }
        if self.firsts {
            *self.counts.entry((at.to_string(), 0)).or_insert(0) += 1;
            self.made.insert(at.to_string(), 0);
            self.trail.push((0, n));
            return 0;
        }
        let mut best = 0;
        let mut best_count = usize::MAX;
        for i in 0..n {
            let c = *self.counts.get(&(at.to_string(), i)).unwrap_or(&0);
            if c < best_count {
                best = i;
                best_count = c;
            }
        }
        *self.counts.entry((at.to_string(), best)).or_insert(0) += 1;
        self.made.insert(at.to_string(), best);
        self.trail.push((best, n));
        best
    }

    fn hole(&mut self, t: &Ty, rg: Option<Range>) -> Value {
        self.holes.push((t.clone(), None, rg));
        json!(format!("{HOLE}{}", self.holes.len() - 1))
    }

    /// A number or a day as it is made: itself, or in a flow that checks a rule's precondition, a
    /// value left open with it in it, which a check may change (`steer`) before the run is written.
    fn steerable(&mut self, t: &Ty, v: Value, rg: Option<Range>) -> Value {
        if !self.steered {
            return v;
        }
        self.holes.push((t.clone(), Some(v), rg));
        json!(format!("{HOLE}{}", self.holes.len() - 1))
    }

    /// The value left open that `e` reads, when it reads one.
    fn open_at(&self, e: &TExpr) -> Option<usize> {
        let TExpr::Var { name, fields, .. } = e else { return None };
        let mut v = self.vars.get(name).cloned().unwrap_or(Value::Null);
        for f in fields {
            if let Some(id) = Ex::hole_id(&v) {
                v = self.holes[id].1.clone().unwrap_or(Value::Null);
            }
            v = v.get(f).cloned().unwrap_or(Value::Null);
        }
        Ex::hole_id(&v)
    }

    /// What `e` reads now, with the values left open in it as they are.
    fn now_of(&self, e: &TExpr) -> Value {
        self.now_with(e, &[])
    }

    /// What `e` would read with the values left open that `over` names changed as it says.
    fn now_with(&self, e: &TExpr, over: &[(usize, Value)]) -> Value {
        let TExpr::Var { name, fields, .. } = e else { return crate::interp::value_of(&self.vars, e) };
        let open = |v: Value| match Ex::hole_id(&v) {
            Some(id) => over.iter().find(|(o, _)| *o == id).map(|(_, x)| x.clone()).or_else(|| self.holes[id].1.clone()).unwrap_or(Value::Null),
            None => v,
        };
        let mut v = open(self.vars.get(name).cloned().unwrap_or(Value::Null));
        for f in fields {
            v = open(v.get(f).cloned().unwrap_or(Value::Null));
        }
        v
    }

    /// Change a value left open that a check reads, so that the check comes out as `holds` says,
    /// where its range has room: the later one made first, and where the check is to hold, one the
    /// rule `rule` takes first. A check none of whose values were left open comes out as it does. A
    /// value not chosen yet (a bool, an enum, a value that may be absent) has no say in the check
    /// until it is: the check is steered, and `settle_check` chooses the rest. A value a check made
    /// before read changes only where that check still comes out as it did (`set_open`).
    fn steer(&mut self, t: &PreTest, rule: usize, holds: bool) {
        if !self.unchosen(t) && crate::prechecks::holds(t, &|e| self.now_of(e)) == holds {
            return;
        }
        match t {
            PreTest::Relation { left, op, right } => {
                let num = |v: Value| v.as_f64().map(|x| x as i64);
                // each side the values it adds up (rulec's §15.216): one, or several
                let vals = |s: &[(String, TExpr)]| s.iter().map(|(_, e)| num(self.now_of(e))).collect::<Option<Vec<i64>>>();
                let (Some(lv), Some(rv)) = (vals(left), vals(right)) else { return };
                let (l, r): (i64, i64) = (lv.iter().sum(), rv.iter().sum());
                // the left side given the right, and the right side given the left
                let lefts = match (op.as_str(), holds) {
                    ("<=", true) | (">=", true) | ("<", false) | (">", false) => r,
                    ("<", true) | (">=", false) => r - 1,
                    _ => r + 1,
                };
                let rights = match (op.as_str(), holds) {
                    ("<=", true) | (">=", true) | ("<", false) | (">", false) => l,
                    ("<", true) | (">=", false) => l + 1,
                    _ => l - 1,
                };
                // one value of a side moved, the others as they are, so that its side comes to what
                // it has to be
                let mut tries: Vec<(Option<usize>, i64, &str)> = Vec::new();
                for (i, (n, e)) in left.iter().enumerate() {
                    tries.push((self.steerable_at(e), lefts - (l - lv[i]), n.as_str()));
                }
                for (i, (n, e)) in right.iter().enumerate() {
                    tries.push((self.steerable_at(e), rights - (r - rv[i]), n.as_str()));
                }
                tries.sort_by_key(|(id, ..)| std::cmp::Reverse(*id));
                // Where the check is to hold, the call goes on to the rule, so a value the rule's
                // own range takes is tried first: with a sum, moving the last value made can take
                // it below zero when the others already come to the most the other side allows.
                let takes = |name: &str, n: i64| -> bool {
                    let Some(c) = self.m.rules.get(rule).and_then(|r| r.info.inputs.iter().find(|c| c.name == name)) else { return true };
                    match &c.ty {
                        crate::rulec::RType::Num { min, max, .. } => min.is_none_or(|lo| n >= lo) && max.is_none_or(|hi| n <= hi),
                        _ => true,
                    }
                };
                let fits = |id: usize, n: i64| self.holes[id].2.is_none_or(|rg| rg.contains(n));
                let open: Vec<(usize, i64, &str)> = tries.iter().filter_map(|(id, n, name)| id.map(|id| (id, *n, *name))).filter(|(id, n, _)| fits(*id, *n)).collect();
                let (first, then): (Vec<_>, Vec<_>) = open.into_iter().partition(|(_, n, name)| !holds || takes(name, *n));
                // the first that leaves every check made before as it came out
                for (id, n, _) in first.into_iter().chain(then) {
                    if self.set_open(vec![(id, json!(n))]) {
                        break;
                    }
                }
            }
            // To break a `never` line every value left open goes where its condition holds; to keep it,
            // the later one made goes where its condition does not.
            PreTest::Never { conds, .. } => {
                let meeting = |t: &ritsu_ports::NeverTest| -> Option<Value> {
                    use ritsu_ports::{NeverTest, Value as V};
                    let lit = |x: &V| match x {
                        V::Bool(b) => Some(json!(b)),
                        V::Int(n) => i64::try_from(*n).ok().map(|m| json!(m)),
                        V::Str(s) | V::Enum(s) | V::Date(s) => Some(json!(s)),
                        _ => None,
                    };
                    match t {
                        NeverTest::Among { values, yes: true } => values.first().and_then(lit),
                        NeverTest::Compare(ops) => {
                            let mut tries: Vec<i128> = ops.iter().flat_map(|(_, k)| [*k, *k + 1, *k - 1]).collect();
                            tries.dedup();
                            tries.into_iter().find(|n| t.holds(&V::Int(*n)) == Some(true)).and_then(|n| i64::try_from(n).ok()).map(|m| json!(m))
                        }
                        _ => None,
                    }
                };
                let missing = |t: &ritsu_ports::NeverTest| -> Option<Value> {
                    use ritsu_ports::{NeverTest, Value as V};
                    match t {
                        NeverTest::Among { values, yes: true } => match values.first() {
                            Some(V::Bool(b)) => Some(json!(!*b)),
                            Some(V::Int(n)) => i64::try_from(*n + 1).ok().map(|m| json!(m)).filter(|_| values.len() == 1),
                            _ => None,
                        },
                        NeverTest::Among { values, yes: false } => values.first().and_then(|x| match x {
                            V::Bool(b) => Some(json!(*b)),
                            V::Int(n) => i64::try_from(*n).ok().map(|m| json!(m)),
                            V::Str(s) | V::Enum(s) | V::Date(s) => Some(json!(s)),
                            _ => None,
                        }),
                        NeverTest::Compare(ops) => {
                            let mut tries: Vec<i128> = ops.iter().flat_map(|(_, k)| [*k, *k + 1, *k - 1]).collect();
                            tries.dedup();
                            tries.into_iter().find(|n| t.holds(&V::Int(*n)) == Some(false)).and_then(|n| i64::try_from(n).ok()).map(|m| json!(m))
                        }
                        _ => None,
                    }
                };
                if holds {
                    // one not chosen yet first, which nothing has read, then the later one made
                    let mut open: Vec<(usize, Value)> = conds.iter().filter_map(|(_, e, t)| Some((self.steerable_at(e)?, missing(t)?))).collect();
                    open.sort_by_key(|(id, _)| (self.holes[*id].1.is_none(), *id));
                    while let Some((id, v)) = open.pop() {
                        if self.set_open(vec![(id, v)]) {
                            break;
                        }
                    }
                } else {
                    let sets: Vec<(usize, Value)> = conds.iter().filter_map(|(_, e, t)| Some((self.steerable_at(e)?, meeting(t)?))).collect();
                    self.set_open(sets);
                }
            }
            PreTest::Days { value, days, .. } => {
                let Some(id) = self.steerable_at(value) else { return };
                let day = if holds { days.first().cloned() } else { ["1999-12-31", "2099-12-31"].iter().find(|d| !days.iter().any(|x| x == *d)).map(|d| d.to_string()) };
                if let Some(d) = day {
                    self.set_open(vec![(id, json!(d))]);
                }
            }
        }
    }

    /// The values a check reads.
    fn check_reads(t: &PreTest) -> Vec<&TExpr> {
        match t {
            PreTest::Relation { left, right, .. } => left.iter().chain(right.iter()).map(|(_, e)| e).collect(),
            PreTest::Days { value, .. } => vec![value],
            PreTest::Never { conds, .. } => conds.iter().map(|(_, e, _)| e).collect(),
        }
    }

    /// Whether a check reads a value left open that is not chosen yet.
    fn unchosen(&self, t: &PreTest) -> bool {
        Ex::check_reads(t).into_iter().any(|e| self.open_at(e).is_some_and(|id| self.holes[id].1.is_none()))
    }

    /// Choose each value the check at `site` reads that is not chosen yet, as reading it chooses it:
    /// the run reads them there, and whether the check holds is what they come to.
    fn settle_check(&mut self, t: &PreTest, site: usize) {
        for (k, e) in Ex::check_reads(t).into_iter().enumerate() {
            if let Some(id) = self.open_at(e).filter(|id| self.holes[*id].1.is_none()) {
                self.read(json!(format!("{HOLE}{id}")), &format!("check {site} {k}"));
            }
        }
    }

    /// A number in `rg` not made before in this run, as far as the range has room.
    fn number(&mut self, rg: Option<Range>) -> Value {
        self.made_values += 1;
        let k = self.made_values as i128;
        let r = rg.unwrap_or_default();
        let n = match (r.lo.map(i128::from), r.hi.map(i128::from)) {
            _ if r.contains(1000 + k as i64) => 1000 + k,
            (Some(lo), Some(hi)) => lo + k % (hi - lo + 1),
            (Some(lo), None) => lo + k,
            (None, Some(hi)) => hi - k,
            (None, None) => 1000 + k,
        };
        json!(n as i64)
    }

    /// A value of type `t` with its enums, bools, optional values and lists left open, to be
    /// chosen when read; its numbers in `rg`, and its records' in theirs.
    fn template(&mut self, t: &Ty, name: &str, rg: Option<Range>) -> Value {
        match t {
            Ty::Str => {
                self.made_values += 1;
                json!(format!("{name}-{}", self.made_values))
            }
            // in its range: when the range does not hold the moment, its first day at 10:00 (as the
            // range writes its days), else its last day at 10:00, else the end it has
            Ty::Timestamp => {
                const AT: &str = "2026-10-01T10:00:00Z";
                let v = match rg {
                    Some(r) if !r.contains(moment_of(AT).unwrap_or_default()) => {
                        let ten = match (r.lo, r.hi) {
                            (Some(lo), _) => lo + 36_000,
                            (None, Some(hi)) => hi + 1 - 86_400 + 36_000,
                            (None, None) => 0,
                        };
                        json!(moment_text(if r.contains(ten) { ten } else { r.lo.or(r.hi).unwrap_or(ten) }))
                    }
                    _ => json!(AT),
                };
                if !self.moments {
                    return v;
                }
                self.holes.push((t.clone(), Some(v), rg));
                json!(format!("{HOLE}{}", self.holes.len() - 1))
            }
            // a day of its own for each value, as a string is, and in its range: when the range does
            // not hold it, a day of the range, from its low end, as a number is made
            Ty::Date => {
                self.made_values += 1;
                let k = self.made_values as i64;
                let text = format!("2026-{:02}-{:02}", 1 + (k / 28) % 12, 1 + k % 28);
                let d = ritsu_ports::day_of(&text).unwrap_or_default();
                let d = match rg {
                    Some(r) if !r.contains(d) => match (r.lo, r.hi) {
                        (Some(lo), Some(hi)) => lo + k % (hi - lo + 1),
                        (Some(lo), None) => lo + k,
                        (None, Some(hi)) => hi - k,
                        (None, None) => d,
                    },
                    _ => d,
                };
                let v = json!(day_text(d));
                self.steerable(t, v, rg)
            }
            Ty::Int | Ty::Num(_) => {
                let v = self.number(rg);
                self.steerable(t, v, rg)
            }
            // an array, so that the targets show they keep a `json` value that is one as one item
            Ty::Json => {
                self.made_values += 1;
                json!([{ "note": format!("{name}-{}", self.made_values) }])
            }
            Ty::Bool | Ty::Enum(_) | Ty::Opt(_) | Ty::List(_) => self.hole(t, rg),
            Ty::Record(r) => {
                let fields = self.m.records[*r].fields.clone();
                let mut o = Map::new();
                for (f, ft) in fields {
                    let v = self.template(&ft, &f, self.m.field_range(*r, &f));
                    o.insert(f, v);
                }
                Value::Object(o)
            }
        }
    }

    /// A value of type `t` with nothing left open: every value that may be absent there, every
    /// list with two items, and the enums' values in turn (those their ranges let in).
    fn full(&mut self, t: &Ty, name: &str, rg: Option<Range>) -> Value {
        match t {
            Ty::Bool => json!(true),
            Ty::Enum(e) => {
                self.made_values += 1;
                let values = self.m.values_in(*e, rg);
                json!(values[self.made_values % values.len()])
            }
            Ty::Opt(inner) => self.full(inner, name, rg),
            Ty::List(inner) => Value::Array((1..=2).map(|i| self.full(inner, &format!("item{i}"), rg)).collect()),
            Ty::Record(r) => {
                let fields = self.m.records[*r].fields.clone();
                let mut o = Map::new();
                for (f, ft) in fields {
                    let v = self.full(&ft, &f, self.m.field_range(*r, &f));
                    o.insert(f, v);
                }
                Value::Object(o)
            }
            _ => self.template(t, name, rg),
        }
    }

    /// A value of type `t` unlike the one a value left open reads as: each enum its last value (of
    /// those its range lets in), a bool false, where an open one reads as the first value and true.
    fn unlike(&mut self, t: &Ty, name: &str, rg: Option<Range>) -> Value {
        match t {
            Ty::Bool => json!(false),
            Ty::Enum(e) => json!(self.m.values_in(*e, rg).last()),
            Ty::Record(r) => {
                let fields = self.m.records[*r].fields.clone();
                let mut o = Map::new();
                for (f, ft) in fields {
                    let v = self.unlike(&ft, &f, self.m.field_range(*r, &f));
                    o.insert(f, v);
                }
                Value::Object(o)
            }
            _ => self.template(t, name, rg),
        }
    }

    /// A value of type `t` with a number, a date or a time outside its range, or a value of an enum
    /// its range does not let in, the first there is: None when nothing in it has a range with room
    /// outside.
    fn outside(&mut self, t: &Ty, rg: Option<Range>) -> Option<Value> {
        match t {
            Ty::Int | Ty::Num(_) => rg?.beyond().map(|n| json!(n)),
            Ty::Date => rg?.beyond().map(|d| json!(day_text(d))),
            Ty::Timestamp => rg?.beyond().map(|t| json!(moment_text(t))),
            Ty::Enum(e) => enum_outside(self.m, *e, rg?).map(|v| json!(v)),
            Ty::Opt(inner) => self.outside(inner, rg),
            Ty::List(inner) => self.outside(inner, rg).map(|v| json!([v])),
            Ty::Record(r) => {
                let fields = self.m.records[*r].fields.clone();
                let at = fields.iter().position(|(f, ft)| self.has_outside(ft, self.m.field_range(*r, f)))?;
                let mut o = Map::new();
                for (i, (f, ft)) in fields.iter().enumerate() {
                    let frg = self.m.field_range(*r, f);
                    let v = if i == at { self.outside(ft, frg)? } else { self.full(ft, f, frg) };
                    o.insert(f.clone(), v);
                }
                Some(Value::Object(o))
            }
            _ => None,
        }
    }

    /// Whether `outside` finds a number, a date, a time or an enum's value with a range in a value
    /// of type `t`.
    fn has_outside(&self, t: &Ty, rg: Option<Range>) -> bool {
        match t {
            Ty::Int | Ty::Num(_) | Ty::Date | Ty::Timestamp => rg.and_then(|r| r.beyond()).is_some(),
            Ty::Enum(e) => rg.and_then(|r| enum_outside(self.m, *e, r)).is_some(),
            Ty::Opt(inner) | Ty::List(inner) => self.has_outside(inner, rg),
            Ty::Record(r) => self.m.records[*r].fields.iter().any(|(f, ft)| self.has_outside(ft, self.m.field_range(*r, f))),
            _ => false,
        }
    }

    /// The values a value left open of type `t` can be chosen as: an enum's, those its range `rg`
    /// lets in.
    fn domain(&self, t: &Ty, rg: Option<Range>) -> Vec<Value> {
        match t {
            Ty::Bool => vec![json!(true), json!(false)],
            Ty::Enum(e) => self.m.values_in(*e, rg).iter().map(|v| json!(v)).collect(),
            // a list left open until the end is empty, an optional value absent
            Ty::List(_) => vec![json!([])],
            _ => vec![Value::Null],
        }
    }

    fn hole_id(v: &Value) -> Option<usize> {
        v.as_str().and_then(|s| s.strip_prefix(HOLE)).and_then(|x| x.parse().ok())
    }

    /// Keep every value left open in `v`, at any depth: the run decided something on it.
    fn keep(&mut self, v: &Value) {
        match v {
            Value::String(_) => {
                if let Some(id) = Ex::hole_id(v) {
                    if self.kept.insert(id) {
                        if let Some(x) = self.holes[id].1.clone() {
                            self.keep(&x);
                        }
                    }
                }
            }
            Value::Object(o) => o.values().for_each(|x| self.keep(x)),
            Value::Array(a) => a.iter().for_each(|x| self.keep(x)),
            _ => {}
        }
    }

    /// The value left open that `e` reads, when it reads one that no `match`, wait or call has
    /// decided on yet.
    fn steerable_at(&self, e: &TExpr) -> Option<usize> {
        self.open_at(e).filter(|id| !self.kept.contains(id))
    }

    /// Whether every check the run has made comes out as it did with the values left open that
    /// `over` names changed as it says: a check that read one of them was made on the value it had.
    fn checks_stay(&self, over: &[(usize, Value)]) -> bool {
        self.checked.iter().all(|(t, held)| crate::prechecks::holds(t, &|e| self.now_with(e, over)) == *held)
    }

    /// Change the values left open that `sets` names, all of them, where every check made before
    /// still comes out as it did; else none. True when they changed.
    fn set_open(&mut self, sets: Vec<(usize, Value)>) -> bool {
        if sets.is_empty() || !self.checks_stay(&sets) {
            return false;
        }
        for (id, v) in sets {
            self.holes[id].1 = Some(v);
        }
        true
    }

    /// Keep the values a call gives a language that answers it: what it answers is decided on them.
    fn keep_args(&mut self, args: &[(String, TExpr)]) {
        for (_, e) in args {
            let v = self.value(e);
            self.keep(&v);
        }
    }

    /// Read a value, choosing an open enum, bool or optional value now.
    fn read(&mut self, v: Value, at: &str) -> Value {
        match Ex::hole_id(&v) {
            Some(id) => {
                if let Some(x) = &self.holes[id].1 {
                    return x.clone();
                }
                let t = self.holes[id].0.clone();
                let rg = self.holes[id].2;
                let picked = match &t {
                    Ty::Opt(inner) => {
                        // absent, or each value of an enum (that its range lets in) or a bool, or some value
                        let mut dom = vec![Value::Null];
                        let listed = matches!(**inner, Ty::Bool | Ty::Enum(_));
                        if listed {
                            dom.extend(self.domain(inner, rg));
                        }
                        let n = if listed { dom.len() } else { 2 };
                        let c = self.choose(at, n);
                        if c == 0 {
                            Value::Null
                        } else if listed {
                            dom[c].clone()
                        } else {
                            self.template(inner, "value", rg)
                        }
                    }
                    _ => {
                        let dom = self.domain(&t, rg);
                        let c = self.choose(at, dom.len());
                        dom[c].clone()
                    }
                };
                self.holes[id].1 = Some(picked.clone());
                picked
            }
            None => v,
        }
    }

    /// The items of a list a `for` goes through, choosing the length of an open one now:
    /// empty, one, two, or one more than the loop takes.
    fn items(&mut self, v: Value, max: u32, at: &str) -> Vec<Value> {
        let id = match Ex::hole_id(&v) {
            Some(id) => id,
            None => return v.as_array().cloned().unwrap_or_default(),
        };
        if let Some(x) = &self.holes[id].1 {
            return x.as_array().cloned().unwrap_or_default();
        }
        let elem = match &self.holes[id].0 {
            Ty::List(t) => (**t).clone(),
            _ => return vec![],
        };
        let mut lengths: Vec<usize> = vec![0, 1, 2];
        lengths.retain(|n| *n <= max as usize);
        lengths.push(max as usize + 1);
        let c = self.choose(at, lengths.len());
        let rg = self.holes[id].2;
        let mut items = Vec::new();
        for i in 0..lengths[c] {
            let v = self.template(&elem, &format!("item{}", i + 1), rg);
            items.push(v);
        }
        self.holes[id].1 = Some(Value::Array(items.clone()));
        items
    }

    /// A deliberate end, and for every loop that went through items on the way, the end after
    /// it: so that a run that fills a list and ends well is kept, not only one with an empty list.
    fn end_label(&mut self, end: &str) {
        self.labels.insert(end.to_string());
        for l in self.done_loops.clone() {
            self.labels.insert(format!("{l} then {end}"));
        }
    }

    /// Stop here as the run fails (an answer the checks refuse, a check that breaks): the run
    /// ends, or inside a parallel round the round does, and the loop fails when every round is done.
    fn fails(&mut self) -> Ctl {
        if self.par_depth == 0 {
            self.ended = Some("fail");
        }
        self.halt(false)
    }

    /// Stop here: the run ends, or inside a parallel round the round does, and the loop
    /// fails when every round is done.
    fn halt(&mut self, task_error: bool) -> Ctl {
        if self.par_depth > 0 {
            if self.pending.is_none() {
                self.pending = Some(task_error);
            }
        } else {
            self.stopped = true;
        }
        Ctl::Stop
    }

    fn fill(&self, v: &Value) -> Value {
        match v {
            Value::String(_) => match Ex::hole_id(v) {
                Some(id) => {
                    let x = self.holes[id].1.clone().unwrap_or_else(|| self.domain(&self.holes[id].0, self.holes[id].2)[0].clone());
                    self.fill(&x)
                }
                None => v.clone(),
            },
            Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), self.fill(x))).collect()),
            Value::Array(a) => Value::Array(a.iter().map(|x| self.fill(x)).collect()),
            other => other.clone(),
        }
    }

    fn finish_input(&self) -> Value {
        let mut o = Map::new();
        for (n, _) in &self.m.inputs {
            o.insert(n.clone(), self.fill(self.vars.get(n).unwrap_or(&Value::Null)));
        }
        match (&self.m.service, self.sparse) {
            // the request as protobuf's JSON writes it: what is at its zero value is left out
            (Some(s), true) => crate::apis::omit_zeros(&Value::Object(o), &s.input_zeros),
            // and in any case a date as `{year, month, day}` (DESIGN 44)
            (Some(s), false) => crate::apis::wire_days(&Value::Object(o), &s.input_zeros),
            _ => Value::Object(o),
        }
    }

    /// Values for `fields` (in `ranges`) at the zero values `zeros` names (`apis::zeros_of`), where
    /// their types take them: each field the zero value its JSON leaves out, a message inside
    /// at its own zero values, a list of messages with one such message in it; the rest as
    /// `template` makes them.
    fn zero_fields(&mut self, fields: &[(String, Ty)], ranges: &dyn Fn(&str) -> Option<Range>, zeros: &Value) -> Map<String, Value> {
        let m = self.m;
        let mut o = Map::new();
        for (f, ft) in fields {
            let rg = ranges(f);
            let record = |t: &Ty| match t {
                Ty::Record(r) => Some(*r),
                _ => None,
            };
            let v = if let (Some(z), Some(r)) = (zeros["m"].get(f), record(ft.inner())) {
                self.zero_record(r, z)
            } else if let (Some(z), Ty::List(elem)) = (zeros["l"].get(f), ft) {
                match record(elem) {
                    Some(r) => Value::Array(vec![self.zero_record(r, z)]),
                    None => self.template(ft, f, rg),
                }
            } else {
                match zeros["f"].get(f) {
                    Some(z) if render::value_fits(m, z, ft, rg) => z.clone(),
                    _ => self.template(ft, f, rg),
                }
            };
            o.insert(f.clone(), v);
        }
        o
    }

    /// A value of the record `r` at the zero values `zeros` names: see `zero_fields`.
    fn zero_record(&mut self, r: RecordId, zeros: &Value) -> Value {
        let m = self.m;
        let fields = m.records[r].fields.clone();
        Value::Object(self.zero_fields(&fields, &|f| m.field_range(r, f), zeros))
    }

    fn value(&mut self, e: &TExpr) -> Value {
        match e {
            TExpr::Var { name, fields, .. } => {
                let mut v = self.vars.get(name).cloned().unwrap_or(Value::Null);
                for f in fields {
                    // a record that was left open and chosen since reads as what was chosen
                    if let Some(id) = Ex::hole_id(&v) {
                        v = self.holes[id].1.clone().unwrap_or(Value::Null);
                    }
                    v = v.get(f).cloned().unwrap_or(Value::Null);
                }
                v
            }
            other => crate::interp::value_of(&self.vars, other),
        }
    }

    fn run(&mut self) {
        // the moment the run starts at, before anything reads the clock
        if self.starts.len() > 1 {
            let c = self.choose("now", self.starts.len());
            if c > 0 {
                let at = self.starts[c].clone();
                self.clock = Clock::of(&json!({ "now": at }), self.computed).expect("a moment a run may start at is a time");
                if let (Some(o), Some(ms)) = (self.oracle.as_mut(), crate::computed::moment_ms(&at)) {
                    o.start_at(ms);
                }
            }
        }
        for (v, _) in &self.m.vars {
            self.vars.insert(v.clone(), Value::Null);
        }
        self.vars.insert(crate::interp::NOW_VAR.into(), json!(self.clock.later()));
        let inputs = self.m.inputs.clone();
        // a workflow that implements a service: an input at its zero values, which its JSON goes without
        if let Some(s) = &self.m.service {
            self.sparse = self.choose("input", 2) == 1;
            if self.sparse {
                self.labels.insert("input:zeros".into());
                let m = self.m;
                let zeros = s.input_zeros.clone();
                let values = self.zero_fields(&inputs, &|n| m.input_ranges.get(n).copied(), &zeros);
                for (n, v) in values {
                    self.vars.insert(n, v);
                }
            }
        }
        for (n, t) in &inputs {
            if self.sparse {
                continue;
            }
            let v = self.template(t, n, self.m.input_ranges.get(n).copied());
            self.vars.insert(n.clone(), v);
        }
        let flow = self.m.flow.clone();
        self.block(&flow);
        if self.cancelled {
            if let Some(block) = self.m.on_cancel.clone() {
                self.stopped = false;
                self.in_on_cancel = true;
                self.labels.insert("on cancel".into());
                self.block(&block);
            }
            if self.ended.is_none() {
                self.ended = Some("cancel");
            }
        }
        // a run that comes to the end of the flow succeeds
        if self.ended.is_none() {
            self.ended = Some("succeed");
        }
    }

    fn block(&mut self, ss: &[TStmt]) -> Ctl {
        for s in ss {
            if self.stopped || self.pending.is_some() {
                return Ctl::Stop;
            }
            self.steps += 1;
            if self.steps > MAX_STEPS {
                self.stopped = true;
                self.cut = true;
                // A run cut here has no end of its own. With the rules, dates and books the
                // languages', a loop goes on as they answer, not as a choice made here sticks, and
                // `ritsu run` would run out of answers where this stopped: such a run is not
                // written (`generate_computed`).
                if self.computed {
                    self.dead = true;
                }
                return Ctl::Stop;
            }
            match self.stmt(s) {
                Ctl::Next => {}
                other => return other,
            }
        }
        Ctl::Next
    }

    fn stmt(&mut self, s: &TStmt) -> Ctl {
        // a statement that reads `now` reads the clock once, before it runs, as the reference
        // interpreter reads it (a check of the period reads it itself, a loop's yield at the end of
        // each round)
        let reads = match &s.kind {
            TK::Period { .. } => false,
            TK::For { list, .. } => list.has_now(),
            k => k.reads_now(),
        };
        if reads && self.read_now().is_none() {
            return Ctl::Stop;
        }
        match &s.kind {
            TK::Pass => Ctl::Next,
            TK::Wait { seconds } => {
                self.go(*seconds as f64, Passing::Wait, None);
                Ctl::Next
            }
            TK::WaitUntil { at } => {
                self.steer_wait(s, at);
                let v = self.value(at);
                self.keep(&v);
                let given = self.fill(&v).as_str().unwrap_or_default().to_string();
                if let Some(seconds) = self.clock.until(&given) {
                    self.go(seconds, Passing::Wait, Some(&given));
                }
                Ctl::Next
            }
            // the period the workflow runs in: the runs walked here start in it, and the run that
            // starts outside it is made after the walk (`now:outside`). The first check after the
            // start chooses whether `now` has left it then, as a run that goes on past the end of its
            // period reads it: the run reads the moment after the end there (on a clock that moves,
            // only a period with an end can be left so, and a clock the run's time took past the end
            // has left it already). Outside, the run goes to `on failure` there, as with a task's
            // error nothing takes
            TK::Period { start: true } => Ctl::Next,
            TK::Period { start: false } => {
                let p = self.m.period.as_ref().expect("a check of the period is in a flow with one");
                if self.later.is_none() {
                    let leaves = match now_outside(self.m) {
                        Some(_) if self.clock.moves() => p.range.hi.is_some() && render::moment_in(&crate::computed::moment_text(self.clock.at_ms()), &p.range),
                        Some(_) => true,
                        None => false,
                    };
                    let stays = !leaves || self.choose(&format!("period {}", s.site), 2) == 0;
                    self.later = Some(stays);
                    if !stays {
                        self.clock.give(now_outside(self.m).unwrap_or_default());
                    }
                }
                let Some(at) = self.read_now() else { return Ctl::Stop };
                let inside = render::moment_in(at.as_str().unwrap_or_default(), &p.range);
                self.vars.insert(crate::interp::now_at(s.site), at);
                if inside {
                    return Ctl::Next;
                }
                self.end_label(&format!("{}:outside", s.site));
                self.unhandled()
            }
            TK::Break => Ctl::Break,
            TK::Assign { name, expr } => {
                let v = self.value(expr);
                self.vars.insert(name.clone(), v);
                Ctl::Next
            }
            TK::Succeed { .. } => {
                self.end_label(&format!("{}:succeed", s.site));
                self.ended = Some("succeed");
                self.halt(false)
            }
            TK::Fail { .. } => {
                self.end_label(&format!("{}:fail", s.site));
                self.ended = Some("fail");
                self.halt(false)
            }
            // a rule's precondition: a run where the values keep it, and one where they break it
            TK::Check(c) => {
                let holds = self.choose(&format!("check {}", s.site), 2) == 0;
                self.steer(&c.test, c.rule, holds);
                self.settle_check(&c.test, s.site);
                let held = crate::prechecks::holds(&c.test, &|e| self.now_of(e));
                self.checked.push((c.test.clone(), held));
                if held {
                    self.labels.insert(format!("{}:holds", s.site));
                    Ctl::Next
                } else {
                    self.end_label(&format!("{}:breaks", s.site));
                    self.fails()
                }
            }
            TK::For { var, list, max, parallel, body, result, locals } => {
                let raw = self.value(list);
                let items = self.items(raw, *max, &format!("for {}", s.site));
                if items.len() > *max as usize {
                    self.labels.insert(format!("{}:too many", s.site));
                    return self.fails();
                }
                if items.is_empty() {
                    self.labels.insert(format!("{}:empty", s.site));
                }
                let mut out = Vec::new();
                match parallel {
                    None => {
                        let mut ran = 0;
                        for it in items.iter() {
                            self.vars.insert(var.clone(), it.clone());
                            let c = self.block(body);
                            match c {
                                Ctl::Break => {
                                    self.labels.insert(format!("{}:break", s.site));
                                    break;
                                }
                                Ctl::Stop => return Ctl::Stop,
                                Ctl::Next => {
                                    ran += 1;
                                    // a round, and more than one: where the rounds' order shows
                                    let l = if ran > 1 { format!("{}:ran again", s.site) } else { format!("{}:ran", s.site) };
                                    self.labels.insert(l.clone());
                                    if !self.done_loops.contains(&l) {
                                        self.done_loops.push(l);
                                    }
                                    if let Some((_, y)) = result {
                                        if y.has_now() && self.read_now().is_none() {
                                            return Ctl::Stop;
                                        }
                                        let v = self.value(y);
                                        out.push(v);
                                    }
                                }
                            }
                        }
                    }
                    Some(_) => {
                        let mut first: Option<bool> = None;
                        let outer = self.pending.take();
                        for it in items.iter() {
                            for l in locals {
                                self.vars.insert(l.clone(), Value::Null);
                            }
                            self.vars.insert(var.clone(), it.clone());
                            self.par_depth += 1;
                            let _ = self.block(body);
                            self.par_depth -= 1;
                            if self.stopped {
                                // a cancellation after a round failed: the cancellation still decides
                                if self.cancelled && first.is_some() {
                                    self.labels.insert(format!("{}:cancelled after a round failed", s.site));
                                }
                                return Ctl::Stop;
                            }
                            match self.pending.take() {
                                Some(k) => {
                                    self.labels.insert(format!("{}:a round fails", s.site));
                                    if first.is_none() {
                                        first = Some(k);
                                    }
                                }
                                None => {
                                    if let Some((_, y)) = result {
                                        if y.has_now() && self.read_now().is_none() {
                                            return Ctl::Stop;
                                        }
                                        let v = self.value(y);
                                        out.push(v);
                                    }
                                }
                            }
                        }
                        self.pending = outer;
                        if let Some(task_error) = first {
                            if task_error {
                                return self.unhandled();
                            }
                            return self.fails();
                        }
                        if !items.is_empty() {
                            // every round, and more than one: where the order of what they yield shows
                            let l = if items.len() > 1 { format!("{}:every one of several rounds done", s.site) } else { format!("{}:every round done", s.site) };
                            self.labels.insert(l.clone());
                            if !self.done_loops.contains(&l) {
                                self.done_loops.push(l);
                            }
                        }
                    }
                }
                if let Some((r, _)) = result {
                    self.vars.insert(r.clone(), Value::Array(out));
                }
                Ctl::Next
            }
            TK::Repeat { times, body } => {
                for n in 0..*times {
                    let c = self.block(body);
                    match c {
                        Ctl::Break => {
                            self.labels.insert(format!("{}:break", s.site));
                            return Ctl::Next;
                        }
                        Ctl::Stop => return Ctl::Stop,
                        Ctl::Next => {}
                    }
                    if n + 1 == *times {
                        self.labels.insert(format!("{}:exhausted", s.site));
                    }
                }
                Ctl::Next
            }
            TK::Match { expr, arms } => {
                let raw = self.value(expr);
                let v = self.read(raw.clone(), &format!("match {}", s.site));
                self.keep(&raw);
                for (i, a) in arms.iter().enumerate() {
                    let hit = (a.none && v.is_null())
                        || (a.some.is_some() && !v.is_null())
                        || a.values.iter().any(|x| match &v {
                            Value::String(s) => s == x,
                            Value::Bool(b) => b.to_string() == *x,
                            _ => false,
                        });
                    if hit {
                        self.labels.insert(format!("{}:arm{}", s.site, i));
                        if let Some(n) = &a.some {
                            self.vars.insert(n.clone(), v.clone());
                        }
                        return self.block(&a.body);
                    }
                }
                self.fails()
            }
            TK::Call { target, callee, handlers, .. } => self.call(s, target.as_ref(), callee, handlers),
        }
    }

    /// The outputs of a rule at its service that its answer can leave out, with the zero value each
    /// is then read as: a bool, which is false; a number whose range has 0 in it (or has none), which
    /// is 0; a string, which is empty; an enum whose value 0 is one of the rule's (a contract's enum
    /// can put a value of its own at 0), which is that value. An enum whose value 0 says that it is
    /// not set is never at it.
    fn zero_outputs(c: &RuleConnect, outputs: &RecordDef) -> Vec<(String, Value)> {
        c.response
            .iter()
            .filter(|f| !f.optional)
            .filter_map(|f| {
                let range = outputs.ranges.get(&f.name);
                match &f.kind {
                    crate::rulec::WireKind::Bool => Some((f.name.clone(), json!(false))),
                    crate::rulec::WireKind::Int if range.is_none_or(|r| r.contains(0)) => Some((f.name.clone(), json!(0))),
                    crate::rulec::WireKind::Str => Some((f.name.clone(), json!(""))),
                    kind @ crate::rulec::WireKind::Enum { .. } => kind.zero_value().map(|v| (f.name.clone(), json!(v))),
                    _ => None,
                }
            })
            .collect()
    }

    /// The first enum output of a rule at its service whose value 0 says that it is not set, which no
    /// value of the rule's is: an answer without it reads as that value, which the check refuses.
    fn unset_output(c: &RuleConnect) -> Option<&crate::rulec::WireField> {
        c.response.iter().find(|f| matches!(f.kind, crate::rulec::WireKind::Enum { .. }) && f.kind.zero_value().is_none())
    }

    fn event_fix(&self, c: usize, event: &str, column: Option<&str>) -> Option<(usize, usize)> {
        let mc = self.m.machine(c);
        let axis = match column {
            Some(col) => mc.axis_of(col)?,
            None => *mc.axes_with_value(event).first()?,
        };
        Some((axis, mc.axes[axis].coords.iter().position(|x| x == event)?))
    }

    fn refused(&self, c: usize, o: &crate::rulec::Outcome) -> bool {
        match &self.m.cases[c].refused_when {
            Some((i, v)) => o.produces.get(*i).cloned().flatten().as_deref() == Some(v.as_str()),
            None => false,
        }
    }

    /// The states the case can really be in now: where it was, and wherever the other side's events take it.
    fn closure(&self, c: usize, st: usize) -> Vec<usize> {
        let case = &self.m.cases[c];
        let mc = self.m.machine(c);
        let mut out = vec![st];
        let mut i = 0;
        while i < out.len() {
            let s = out[i];
            for (axis, coord, _) in &case.external {
                let mut fixed = case.held.clone();
                fixed.push((*axis, *coord));
                for o in mc.outcomes(s, &fixed) {
                    if !self.refused(c, &o) && !out.contains(&o.next) {
                        out.push(o.next);
                    }
                }
            }
            i += 1;
        }
        out
    }

    fn reachable(&self, c: usize) -> Vec<usize> {
        let case = &self.m.cases[c];
        let mc = self.m.machine(c);
        let mut out = vec![mc.initial];
        let mut i = 0;
        while i < out.len() {
            let s = out[i];
            for a in 0..mc.axes.len() {
                if mc.is_state_axis(a) || case.held.iter().any(|(h, _)| *h == a) {
                    continue;
                }
                for ci in 0..mc.axes[a].coords.len() {
                    let mut fixed = case.held.clone();
                    fixed.push((a, ci));
                    for o in mc.outcomes(s, &fixed) {
                        if !self.refused(c, &o) && !out.contains(&o.next) {
                            out.push(o.next);
                        }
                    }
                }
            }
            i += 1;
        }
        out
    }

    fn call(&mut self, s: &TStmt, target: Option<&Target>, callee: &Callee, handlers: &[THandler]) -> Ctl {
        let m = self.m;
        if let (true, Callee::Rule(r)) = (self.computed, callee) {
            if matches!(m.rules[*r].kind, RuleKind::Rule) {
                return self.computed_rule(s, target, *r, handlers);
            }
        }
        // with the ports, koyomi answers a call of a date, and chobo an operation of a book
        if self.oracle.is_some() {
            match callee {
                Callee::Rule(r) if matches!(m.rules[*r].kind, RuleKind::Date(_)) => return self.computed_date(s, target, *r, handlers),
                Callee::Task(t) if m.tasks[*t].book().is_some() => return self.computed_book(s, target, *t, handlers),
                _ => {}
            }
        }
        let result_ty = match callee {
            Callee::Task(t) => m.tasks[*t].result.clone(),
            Callee::Rule(r) => Some(Ty::Record(m.rules[*r].outputs)),
        };
        // the ways this call can come out: (label, state of the case after, answer)
        enum Way {
            Ok(Option<usize>),
            Err(String, Option<usize>),
            RetryThenOk(Option<usize>),
            /// a Claude agent's answer whose enum values differ from the enum's in case
            Recased(Option<usize>),
            /// an answer with a state the generated check does not let through
            Unexpected(usize),
            /// an answer that does not have the declared shape
            Malformed,
            /// an answer with a number outside its range, in a case's state the call may lead to
            OutOfRange(Option<usize>),
            /// a Connect answer whose fields are at their zero values, which its JSON leaves out
            Zeros,
            /// a Connect answer without its `json` fields: a `google.protobuf.Value` that is not set is left
            /// out of its JSON, and reads as null
            JsonLeftOut,
            /// a Connect answer with the `google.type.Date` the flow reads at this field short of its
            /// day, as protobuf's JSON writes a date whose day is 0: the answer's check refuses it
            PartialDay(String),
            /// the value of an event or a callback that a method of the service sends, at its zero values,
            /// which its JSON leaves out
            AnswerZeros,
            /// a rule's service answering with the outputs that are at their zero values (false, 0, an empty
            /// string, an enum's value 0 when it is one of the rule's) left out of its JSON, which every
            /// platform reads back as the zero values
            RuleZeros,
            /// a rule's service answering without one of the enum outputs, which reads as the enum's value
            /// 0, when that says it is not set and no value of the rule's is: the answer's check refuses it
            RuleUnset,
            /// a cancellation that comes while the call is out
            Cancelled,
            /// Jev's answer, less sure than the task asks: the task's error, from an answer
            Low(String),
            /// the Decisions API's answer with a question refused: the task's refusal error
            Refused(String),
        }
        let mut ways: Vec<(String, Way)> = Vec::new();
        let case = match target {
            Some(Target::Case(c)) => Some(*c),
            _ => None,
        };
        match (case, callee) {
            (Some(c), Callee::Task(t)) => {
                let task = &m.tasks[*t];
                let mc = m.machine(c);
                match &task.machine {
                    Some(TaskMachine::Starts { then, .. }) => {
                        let mut states = vec![mc.initial];
                        for ev in then {
                            let mut next = Vec::new();
                            if let Some(fx) = self.event_fix(c, ev, None) {
                                let mut fixed = m.cases[c].held.clone();
                                fixed.push(fx);
                                for st in &states {
                                    for o in mc.outcomes(*st, &fixed) {
                                        if !self.refused(c, &o) && !next.contains(&o.next) {
                                            next.push(o.next);
                                        }
                                    }
                                }
                            }
                            states = next;
                        }
                        for st in states {
                            ways.push((format!("starts:{}", mc.states[st]), Way::Ok(Some(st))));
                        }
                    }
                    Some(TaskMachine::Sends { event, column }) => {
                        let now = self.cases[c].unwrap_or(mc.initial);
                        if let Some(fx) = self.event_fix(c, event, column.as_deref()) {
                            let mut fixed = m.cases[c].held.clone();
                            fixed.push(fx);
                            let mut seen = BTreeSet::new();
                            // a book's hold is refused with the reason its row gives, when the task declares it
                                            let reason_ix = mc.decides.iter().position(|d| d == "reason");
                            for real in self.closure(c, now) {
                                for o in mc.outcomes(real, &fixed) {
                                    if self.refused(c, &o) {
                                        let book_reason = task.book().and(reason_ix).and_then(|i| o.produces.get(i).cloned().flatten()).filter(|r| task.error(r).is_some());
                                        if let Some(err) = task.refused_as.as_ref().or(book_reason.as_ref()) {
                                            if seen.insert(format!("refused@{}", mc.states[real])) {
                                                ways.push((format!("refused:{}", mc.states[real]), Way::Err(err.clone(), Some(real))));
                                            }
                                        }
                                    } else if seen.insert(format!("ok:{}", mc.states[o.next])) {
                                        ways.push((format!("to:{}", mc.states[o.next]), Way::Ok(Some(o.next))));
                                    }
                                }
                            }
                        }
                    }
                    Some(TaskMachine::Observes) => {
                        let seen = match self.cases[c] {
                            Some(now) => self.closure(c, now),
                            // a case this run did not start: any state the machine reaches
                            None => self.reachable(c),
                        };
                        for st in seen {
                            ways.push((format!("sees:{}", mc.states[st]), Way::Ok(Some(st))));
                        }
                    }
                    None => ways.push(("ok".into(), Way::Ok(None))),
                }
            }
            _ => ways.push(("ok".into(), Way::Ok(None))),
        }
        // on a clock that moves (`generate_computed`), a case's state the answer's check does not let
        // through here is no answer the run goes on with: it fails the call, as `unexpected state`
        // below. The walk's own idea of the case can hold more than the check does: after a refusal
        // it took as an error the handler takes, the case is where the event is refused
        if self.computed && !matches!(callee, Callee::Task(t) if m.tasks[*t].book().is_some()) {
            if let (Some(c), Some((_, allowed))) = (case, m.monitors.get(&s.site)) {
                let states = &m.machine(c).states;
                ways.retain(|(_, w)| !matches!(w, Way::Ok(Some(st)) if !allowed.contains(&states[*st])));
            }
        }
        let retriers = retriers(m, callee);
        // a Jev task's error for an answer less sure than it asks
        let floor = match callee {
            Callee::Task(t) => m.tasks[*t].jev().and_then(|j| j.floor.as_ref()).map(|(_, e)| e.clone()),
            Callee::Rule(_) => None,
        };
        // a `jev openai` task's error for a question the model refuses
        let refusal = match callee {
            Callee::Task(t) => m.tasks[*t].jev().and_then(|j| j.refusal_error()).map(String::from),
            Callee::Rule(_) => None,
        };
        for h in handlers {
            let kind = match h.errors.first() {
                Some(HErr::Declared(n)) => n.clone(),
                Some(HErr::Timeout) => "timeout".into(),
                _ => "failure".into(),
            };
            let label = format!("on:{kind}");
            if !ways.iter().any(|(l, _)| *l == label) {
                if floor.as_deref() == Some(kind.as_str()) {
                    ways.push((label, Way::Low(kind)));
                } else if refusal.as_deref() == Some(kind.as_str()) {
                    ways.push((label, Way::Refused(kind)));
                } else {
                    ways.push((label, Way::Err(kind, None)));
                }
            }
        }
        if let Some(e) = &floor {
            // the error nothing at the call takes, or `on failure` takes
            if !ways.iter().any(|(l, _)| *l == format!("on:{e}")) {
                ways.push((format!("less sure:{e}"), Way::Low(e.clone())));
            }
        }
        if let Some(e) = &refusal {
            // the same for a refusal: its own error, or `Dandori.Refused` that `on failure` takes
            if !ways.iter().any(|(l, _)| *l == format!("on:{e}")) {
                ways.push((format!("refused:{e}"), Way::Refused(e.clone())));
            }
        }
        let caught_all = handlers.iter().any(|h| h.errors.contains(&HErr::Failure));
        if !caught_all {
            ways.push(("unhandled".into(), Way::Err("failure".into(), None)));
        }
        let first_ok = ways.iter().find_map(|(_, w)| match w {
            Way::Ok(st) => Some(*st),
            _ => None,
        });
        if let Callee::Task(t) = callee {
            if m.tasks[*t].retry.is_some() {
                if let Some(st) = first_ok {
                    ways.push(("retried".into(), Way::RetryThenOk(st)));
                }
            }
        }
        if let (Callee::Task(t), Some(ty), Some(st), true) = (callee, &result_ty, first_ok, target.is_some()) {
            if matches!(m.tasks[*t].via(Platform::Temporal), Some(Via::Agent { provider: Provider::Claude, .. })) && render::has_cased_enum(m, ty) {
                ways.push(("recased".into(), Way::Recased(st)));
            }
        }
        // a book's operation is answered by chobo's client with done, done before or a refusal, and
        // the code dandori writes makes the hold of it: its state and shape are always what they must be
        let book = matches!(callee, Callee::Task(t) if m.tasks[*t].book().is_some());
        if let (true, Some(st)) = (book, first_ok) {
            ways.push(("done before".into(), Way::Ok(st)));
        }
        if let (Some(c), Some((_, allowed)), false) = (case, m.monitors.get(&s.site), book) {
            let mc = m.machine(c);
            if let Some(st) = (0..mc.states.len()).find(|x| !allowed.contains(&mc.states[*x])) {
                ways.push(("unexpected state".into(), Way::Unexpected(st)));
            }
        }
        // an answer that does not fit is one the workflow reads: not for a task that answers nothing, or anything
        if target.is_some() && !matches!(result_ty, None | Some(Ty::Json)) && !book {
            ways.push(("malformed".into(), Way::Malformed));
        }
        // a number out of its range; not in Jev's answer, whose numbers are how sure it is
        let jev_task = match callee {
            Callee::Task(t) => m.tasks[*t].jev().map(|_| *t),
            Callee::Rule(_) => None,
        };
        if let (Some(ty), Some(st), true, None) = (&result_ty, first_ok, target.is_some(), jev_task) {
            if self.has_outside(ty, m.answer_range(callee)) {
                ways.push(("out of range".into(), Way::OutOfRange(st)));
            }
        }
        let zeros = match callee {
            Callee::Task(t) => m.tasks[*t].connect.clone(),
            Callee::Rule(_) => None,
        };
        if let (Some(z), Some(_), true, None) = (&zeros, first_ok, target.is_some(), case) {
            // on a clock that moves, only when a zero value that reads back as a value is there to leave out
            if !self.computed || result_ty.as_ref().is_some_and(|ty| leaves_some(&readable_zeros(m, z, ty))) {
                ways.push(("zero values".into(), Way::Zeros));
            }
            if result_ty.as_ref().is_some_and(|ty| has_json_field(m, ty)) {
                ways.push(("json left out".into(), Way::JsonLeftOut));
            }
            if let Some(f) = result_ty.as_ref().and_then(|ty| day_at_top(m, ty, z)) {
                ways.push(("partial date".into(), Way::PartialDay(f)));
            }
        }
        // what a method of the service sends a task: an event's value, a callback's answer
        let answer_zeros = match callee {
            Callee::Task(t) => m.tasks[*t].answer_zeros.clone(),
            Callee::Rule(_) => None,
        };
        if let (Some(z), Some(_), true, None, Some(ty @ Ty::Record(_))) = (&answer_zeros, first_ok, target.is_some(), case, &result_ty) {
            ways.push(("zero values".into(), Way::AnswerZeros));
            if let Some(f) = day_at_top(m, ty, z) {
                ways.push(("partial date".into(), Way::PartialDay(f)));
            }
        }
        // a rule called at its service: the outputs at their zero values left out, and an enum left out
        let service = match callee {
            Callee::Rule(r) => m.rules[*r].connect.as_ref().map(|c| (*r, c)),
            Callee::Task(_) => None,
        };
        if let (Some((r, c)), true, Some(_)) = (service, target.is_some(), first_ok) {
            if !Ex::zero_outputs(c, &m.records[m.rules[r].outputs]).is_empty() {
                ways.push(("rule zero values".into(), Way::RuleZeros));
            }
            if Ex::unset_output(c).is_some() {
                ways.push(("rule enum left out".into(), Way::RuleUnset));
            }
        }
        // a workflow that says what to do when it is cancelled can be cancelled at any call, but not in `on cancel`
        if self.m.on_cancel.is_some() && !self.in_on_cancel {
            ways.push(("cancelled".into(), Way::Cancelled));
        }
        let pick = self.choose(&format!("call {}", s.site), ways.len());
        let (label, way) = ways.swap_remove(pick);
        self.labels.insert(format!("{}:{label}", s.site));
        let ok_answer = |ex: &mut Ex, st: Option<usize>| -> Value {
            let mut v = match &result_ty {
                Some(t) => ex.template(t, "value", m.answer_range(callee)),
                None => Value::Null,
            };
            if let (Some(c), Some(st)) = (case, st) {
                let field = m.cases[c].state_field.clone();
                v[field] = json!(m.machine(c).states[st]);
            }
            // how sure Jev is, in the fields that take it, is what the response written for the
            // answer says (`render::jev_wire`), as the targets read it back: a value made here
            // would steer the run where its response does not take it
            if let Some(j) = jev_task.and_then(|t| m.tasks[t].jev()).filter(|j| !j.confidences.is_empty()) {
                let (read, _, _) = render::jev_read(j, &render::jev_wire(j, &v, false, false));
                for (f, _, _) in &j.confidences {
                    v[f.as_str()] = read[f.as_str()].clone();
                }
            }
            v
        };
        // an answer, which for a Jev task becomes Jev's response once its value is chosen, and for a
        // rule at its service the body the service writes
        let done = if label == "done before" { "done_before" } else { "done" };
        let date = match callee {
            Callee::Rule(r) => m.rules[*r].date().map(|_| *r),
            Callee::Task(_) => None,
        };
        // the `google.type.Date`s of a Connect answer, or of the value a method of the service sends,
        // which protobuf's JSON writes as `{year, month, day}` (DESIGN 44)
        let days = zeros.as_ref().or(answer_zeros.as_ref()).filter(|z| crate::apis::has_days(z)).cloned();
        let ok = |v: &Value| match (jev_task, service) {
            (Some(t), _) => json!({ "ok": v, "jev": t }),
            (None, Some((r, _))) => json!({ "ok": v, "rule": r }),
            // what chobo's client answers; the hold is made of it and the arguments
            (None, None) if book => json!({ "ok": { "result": done } }),
            // a date's time is made of its day when the run is written
            (None, None) if date.is_some() => json!({ "ok": v, "date": date }),
            (None, None) if days.is_some() => json!({ "ok": v, "days": days }),
            (None, None) => json!({ "ok": v }),
        };
        match way {
            Way::Ok(st) => {
                let v = ok_answer(self, st);
                self.answers.push(ok(&v));
                self.assign(target, v, st);
                Ctl::Next
            }
            Way::Low(kind) => {
                // not a value the variable may hold already, so that a target that keeps it shows
                let v = self.unlike(result_ty.as_ref().unwrap_or(&Ty::Json), "value", m.answer_range(callee));
                self.answers.push(json!({ "ok": v, "jev": jev_task, "low": true }));
                self.take_error(callee, &kind, &kind, handlers)
            }
            Way::Refused(kind) => {
                // the other answers are there, and not what the variable may hold already
                let v = self.unlike(result_ty.as_ref().unwrap_or(&Ty::Json), "value", m.answer_range(callee));
                self.answers.push(json!({ "ok": v, "jev": jev_task, "refused": true }));
                self.take_error(callee, &kind, &kind, handlers)
            }
            Way::Recased(st) => {
                // every part of the answer there, lists with items, so that every enum value in it is turned
                let t = match callee {
                    Callee::Task(t) => *t,
                    Callee::Rule(_) => unreachable!("only an agent's answer is recased"),
                };
                let v = self.full(result_ty.as_ref().unwrap_or(&Ty::Json), "value", m.answer_range(callee));
                self.answers.push(json!({ "ok": v, "recase": t }));
                self.assign(target, v, st);
                Ctl::Next
            }
            Way::Unexpected(st) => {
                let v = ok_answer(self, Some(st));
                self.answers.push(match &days {
                    Some(z) => json!({ "ok": v, "days": z }),
                    None => json!({ "ok": v }),
                });
                self.fails()
            }
            Way::PartialDay(field) => {
                // every part there, the dates as protobuf's JSON writes them, and the one the flow reads
                // at `field` without its day, as protobuf's JSON writes a date whose day is 0: no date,
                // which the answer's check refuses (DESIGN 44)
                let z = zeros.as_ref().or(answer_zeros.as_ref()).expect("a Connect task or a method of the service");
                let ty = result_ty.as_ref().expect("an answer to read");
                let mut wire = crate::apis::wire_days(&self.full(ty, "value", m.answer_range(callee)), z);
                if let Some(Value::Object(day)) = wire.get_mut(&field) {
                    day.remove("day");
                }
                self.answers.push(json!({ "ok": wire }));
                self.fails()
            }
            Way::Malformed => {
                // protobuf reads {} as a message at its zero values, so a Connect answer that does not fit
                // is a list, and so is a value a method of the service sends
                self.answers.push(json!({ "ok": if zeros.is_some() || service.is_some() || answer_zeros.is_some() { json!([]) } else { json!({}) } }));
                self.fails()
            }
            Way::RuleZeros => {
                // what the service leaves out is there when read: the outputs that can be zero are, and the rest is chosen as it is
                let (r, c) = service.expect("a rule at its service");
                let mut v = ok_answer(self, None);
                for (f, zero) in Ex::zero_outputs(c, &m.records[m.rules[r].outputs]) {
                    v[f] = zero;
                }
                self.answers.push(json!({ "ok": v, "rule": r }));
                self.assign(target, v, None);
                Ctl::Next
            }
            Way::RuleUnset => {
                // an enum output is not in the body: it reads as the enum's value 0, which says it is not set and the check refuses
                let (_, c) = service.expect("a rule at its service");
                let v = ok_answer(self, None);
                let mut wire = render::rule_wire(c, &self.fill(&v));
                let left_out = Ex::unset_output(c).map(|f| f.json.clone()).expect("an enum output that can be unset");
                wire.as_object_mut().expect("a body").remove(&left_out);
                self.answers.push(json!({ "ok": wire }));
                self.fails()
            }
            Way::AnswerZeros => {
                // the value at its zero values, which the method's request goes without; read, they are there
                let z = answer_zeros.as_ref().expect("a value a method of the service sends");
                let Some(Ty::Record(r)) = &result_ty else { unreachable!("a record") };
                let v = self.zero_record(*r, z);
                self.answers.push(json!({ "ok": v, "omit": z }));
                self.assign(target, v, None);
                Ctl::Next
            }
            Way::JsonLeftOut => {
                // every part there, lists with items, but the `json` fields, at every depth: what the answer's
                // JSON goes without, which reads as null (or, for a map, as the empty map)
                let z = zeros.as_ref().expect("a Connect task");
                let ty = result_ty.as_ref().expect("an answer to read");
                let v = self.full(ty, "value", m.answer_range(callee));
                let wire = crate::apis::wire_days(&without_json(m, &v, ty, false), z);
                self.answers.push(json!({ "ok": wire }));
                self.assign(target, crate::apis::fill(&wire, z), None);
                Ctl::Next
            }
            Way::Zeros => {
                // every part there, lists with items, so that fields are left out at every depth
                let z = zeros.as_ref().expect("a Connect task");
                let ty = result_ty.as_ref().expect("an answer to read");
                let v = self.full(ty, "value", m.answer_range(callee));
                // on a clock that moves, the zero values left out are those that read back as values of
                // the answer's type (`readable_zeros`): the run goes on with the answer
                let wire = if self.computed { crate::apis::sparse(&v, &readable_zeros(m, z, ty)) } else { crate::apis::sparse(&v, z) };
                self.answers.push(json!({ "ok": wire }));
                self.assign(target, crate::apis::fill(&wire, z), None);
                Ctl::Next
            }
            Way::OutOfRange(st) => {
                let mut v = self.outside(result_ty.as_ref().expect("an answer with a range has a type"), m.answer_range(callee)).expect("a number with a range is there");
                if let (Some(c), Some(st)) = (case, st) {
                    let field = m.cases[c].state_field.clone();
                    v[field] = json!(m.machine(c).states[st]);
                }
                self.answers.push(ok(&v));
                self.fails()
            }
            Way::Cancelled => {
                // every round stops, not only this one; the run ends as cancelled, unless `on
                // cancel` ends it otherwise
                self.answers.push(json!({ "cancel": true }));
                self.cancelled = true;
                self.ended = None;
                self.stopped = true;
                Ctl::Stop
            }
            Way::RetryThenOk(st) => {
                // one error that the retriers take, then an answer
                let kind = match callee {
                    Callee::Task(t) => match &m.tasks[*t].retry {
                        Some(r) if !r.on.is_empty() && !r.on.iter().any(|x| x == "failure") => r.on[0].clone(),
                        _ => "failure".to_string(),
                    },
                    Callee::Rule(_) => "failure".to_string(),
                };
                self.answers.push(json!({ "error": kind }));
                self.tries_time(s, callee, &[kind.clone(), "ok".into()]);
                let v = ok_answer(self, st);
                self.answers.push(ok(&v));
                self.assign(target, v, st);
                Ctl::Next
            }
            Way::Err(kind, real) => {
                // as many times as the retriers would ask for it again, so that it comes through
                let name = error_name(m, callee, &kind);
                let tries = 1 + retriers.iter().find(|(names, _)| matches_error(names, &name)).map(|(_, max)| *max as usize).unwrap_or(0);
                for _ in 0..tries {
                    self.answers.push(json!({ "error": kind }));
                }
                self.tries_time(s, callee, &vec![kind.clone(); tries]);
                if let (Some(c), Some(r)) = (case, real) {
                    self.cases[c] = Some(r);
                }
                self.take_error(callee, &kind, &name, handlers)
            }
        }
    }

    /// A call of a rule rulec answers (`generate_computed`): the values the call gives, every one
    /// left open chosen here, and the record of outputs rulec computes for them, as `ritsu run`
    /// computes it; an input the generated code refuses at its door fails the call, as there. No
    /// answer is written for it.
    fn computed_rule(&mut self, s: &TStmt, target: Option<&Target>, r: usize, handlers: &[THandler]) -> Ctl {
        let TK::Call { args, .. } = &s.kind else { return Ctl::Next };
        let mut given = Map::new();
        for (name, e) in args {
            let v = self.chosen(e, &format!("call {} {name}", s.site));
            given.insert(name.clone(), v);
        }
        self.steer_rule(s, r, args, &mut given);
        self.keep_args(args);
        self.labels.insert(format!("{}:rulec", s.site));
        match crate::sources::rule_outputs(self.m, r, &given) {
            Ok(Ok(record)) => {
                self.assign(target, record, None);
                Ctl::Next
            }
            Ok(Err(_)) => {
                let callee = Callee::Rule(r);
                let tries = tries_of(self.m, &callee, "failure");
                self.tries_time(s, &callee, &vec!["failure".to_string(); tries]);
                self.take_error(&callee, "failure", "failure", handlers)
            }
            Err(_) => {
                self.dead = true;
                self.stopped = true;
                Ctl::Stop
            }
        }
    }

    /// The values a call of a rule gives that are left open (its numbers and days: `steered`), moved
    /// so that the rule comes to another answer than it comes to with them as they were made
    /// (`generate_computed`, with rulec): each answer of the outputs a `match` can branch on (the
    /// enums and the truth values) that an input of the rule's vectors comes to
    /// (`Rules::vector_inputs`), the values the call gives that are not left open as they are and
    /// the open ones taken from the vector, where their own ranges take them and every check made
    /// before still comes out as it did. The walk chooses one of those answers, or the values as
    /// made. rulec computes the answer from the values, as `ritsu run` does: the walk chooses where
    /// they come from, not what the rule answers. An enum and a truth value are the walk's to
    /// choose where they are read, and a value the run has decided something on is not moved.
    fn steer_rule(&mut self, s: &TStmt, r: usize, args: &[(String, TExpr)], given: &mut Map<String, Value>) {
        let m = self.m;
        // the outputs a `match` reads, of those
        let read: Option<BTreeSet<String>> = self.aims.borrow().aimed.as_ref().map(|a| a.get(&s.site).cloned().unwrap_or_default());
        let outs: Vec<&str> = m.rules[r]
            .info
            .outputs
            .iter()
            .filter(|c| matches!(c.ty, crate::rulec::RType::Enum { .. } | crate::rulec::RType::Bool) && read.as_ref().is_none_or(|w| w.contains(&c.name)))
            .map(|c| c.name.as_str())
            .collect();
        if !self.steered || outs.is_empty() {
            return;
        }
        // each value left open that the call gives, by the input it is given to
        let open: Vec<(&str, usize)> = args
            .iter()
            .filter_map(|(n, e)| Some((n.as_str(), self.steerable_at(e)?)))
            .filter(|(_, id)| matches!(self.holes[*id].0, Ty::Int | Ty::Num(_) | Ty::Date) && self.holes[*id].1.is_some())
            .collect();
        if open.is_empty() {
            return;
        }
        let answer = |given: &Map<String, Value>| -> Option<Vec<Value>> {
            match crate::sources::rule_outputs(m, r, given) {
                Ok(Ok(record)) => Some(outs.iter().map(|o| record.get(*o).cloned().unwrap_or(Value::Null)).collect()),
                _ => None,
            }
        };
        let made = answer(given);
        // what rulec answers with the open values taken from each vector, asked once in a walk
        let fixed: BTreeMap<&String, &Value> = given.iter().filter(|(n, _)| !open.iter().any(|(o, _)| o == n)).collect();
        let key = format!("{r}\u{1}{}\u{1}{}", serde_json::to_string(&fixed).unwrap_or_default(), open.iter().map(|(n, _)| *n).collect::<Vec<_>>().join("\u{1}"));
        let known = self.aims.borrow().answers.get(&key).cloned();
        let aims = match known {
            Some(a) => a,
            None => {
                let vectors = self.aims.borrow_mut().vectors.entry(r).or_insert_with(|| std::rc::Rc::new(crate::sources::rule_vectors(m, r))).clone();
                let mut tried: BTreeSet<String> = BTreeSet::new();
                let mut found: Vec<Aim> = Vec::new();
                for v in vectors.iter() {
                    let taken: Vec<(String, Value)> = open.iter().filter_map(|(n, _)| v.get(*n).map(|x| (n.to_string(), x.clone()))).collect();
                    if taken.is_empty() || !tried.insert(serde_json::to_string(&taken).unwrap_or_default()) {
                        continue;
                    }
                    let mut trial = given.clone();
                    trial.extend(taken.iter().cloned());
                    if let Some(t) = answer(&trial) {
                        found.push((taken, t));
                    }
                }
                let found = std::rc::Rc::new(found);
                self.aims.borrow_mut().answers.insert(key, found.clone());
                found
            }
        };
        // a value its own range takes, of its type
        let fits = |ex: &Ex, id: usize, v: &Value| -> bool {
            let n = match ex.holes[id].0 {
                Ty::Date => v.as_str().and_then(ritsu_ports::day_of),
                _ => v.as_i64(),
            };
            n.is_some_and(|n| ex.holes[id].2.is_none_or(|rg| rg.contains(n)))
        };
        // each other answer, with the first values that come to it
        let mut targets: Vec<Vec<(usize, Value)>> = Vec::new();
        let mut reached: Vec<&Vec<Value>> = made.iter().collect();
        for (taken, t) in aims.iter() {
            if reached.contains(&t) {
                continue;
            }
            let mut sets: Vec<(usize, Value)> = Vec::new();
            let mut ok = true;
            for (n, v) in taken {
                let Some((_, id)) = open.iter().find(|(o, _)| o == n) else { continue };
                match sets.iter().find(|(i, _)| i == id) {
                    // two inputs given one value
                    Some((_, w)) => ok &= w == v,
                    None => {
                        ok &= fits(self, *id, v);
                        sets.push((*id, v.clone()));
                    }
                }
            }
            if ok && self.checks_stay(&sets) {
                reached.push(t);
                targets.push(sets);
            }
        }
        if targets.is_empty() {
            return;
        }
        let k = self.choose(&format!("rule {}", s.site), targets.len() + 1);
        if k == 0 {
            return;
        }
        for (id, v) in &targets[k - 1] {
            self.holes[*id].1 = Some(v.clone());
        }
        for (n, e) in args {
            if open.iter().any(|(o, _)| o == n) {
                given.insert(n.clone(), self.now_of(e));
            }
        }
    }

    /// A call of a date koyomi answers (`generate_computed`, with the ports): the values the call
    /// gives, every one left open chosen here, and the day (and the time) koyomi computes for them,
    /// as `ritsu run` computes it; an input koyomi refuses fails the call, as there, after the
    /// retries. No answer is written for it. A value left open that the call gives an input goes
    /// inside the range the input takes, or outside it, where its own range has room: both are runs
    /// the workflow can make (`ritsu check`'s W205 says which calls cannot be decided).
    fn computed_date(&mut self, s: &TStmt, target: Option<&Target>, r: usize, handlers: &[THandler]) -> Ctl {
        let m = self.m;
        let TK::Call { args, .. } = &s.kind else { return Ctl::Next };
        self.steer_date(s, r, args);
        let mut given = Map::new();
        for (name, e) in args {
            let v = self.chosen(e, &format!("call {} {name}", s.site));
            given.insert(name.clone(), v);
        }
        self.keep_args(args);
        self.labels.insert(format!("{}:koyomi", s.site));
        let callee = Callee::Rule(r);
        let got = self.oracle.as_mut().and_then(|o| o.answer(m, render::View::Asl, &callee, &given));
        match got {
            Some(Ok(v)) => match v.get("ok") {
                Some(day) => {
                    self.labels.insert(format!("{}:ok", s.site));
                    self.assign(target, day.clone(), None);
                    Ctl::Next
                }
                None => {
                    let name = error_name(m, &callee, "failure");
                    let taken = self.taken_by(&callee, "failure", &name, handlers);
                    self.labels.insert(format!("{}:{taken}", s.site));
                    let tries = tries_of(m, &callee, "failure");
                    self.tries_time(s, &callee, &vec!["failure".to_string(); tries]);
                    self.take_error(&callee, "failure", &name, handlers)
                }
            },
            _ => {
                self.dead = true;
                self.stopped = true;
                Ctl::Stop
            }
        }
    }

    /// An operation of a book chobo does (`generate_computed`, with the ports), on the books the run
    /// keeps from its start, as `ritsu run` does it: the values the call gives, every one left open
    /// chosen here, and what chobo answers. What the books hold before the run is chosen where it
    /// changes nothing that came before: an operation that names no account and no key the run has
    /// named yet finds an account it draws from short, as an empty book has it, or holding what the
    /// operation takes (a transfer into it done before the run); and while the run's time has not
    /// gone on, the operation may have been done before the run, which chobo answers as done before,
    /// or another with the same key, which it refuses as a conflict. A post of part of a hold the run
    /// made posts what the hold holds or less, or more, which chobo refuses, where the amount is a
    /// value left open.
    fn computed_book(&mut self, s: &TStmt, target: Option<&Target>, t: usize, handlers: &[THandler]) -> Ctl {
        let m = self.m;
        let TK::Call { args, .. } = &s.kind else { return Ctl::Next };
        let task = &m.tasks[t];
        let Some(b) = task.book() else { return Ctl::Next };
        let mut given = Map::new();
        for (name, e) in args {
            let v = self.chosen(e, &format!("call {} {name}", s.site));
            given.insert(name.clone(), v);
        }
        let Ok(mut call) = crate::computed::book_call_of(m, task, b, &given) else {
            self.dead = true;
            self.stopped = true;
            return Ctl::Stop;
        };
        let bu = &m.books[b.book];
        if b.op == "post" && self.steer_post(s, b, args, &call, &mut given) {
            match crate::computed::book_call_of(m, task, b, &given) {
                Ok(c) => call = c,
                Err(_) => {
                    self.dead = true;
                    self.stopped = true;
                    return Ctl::Stop;
                }
            }
        }
        self.keep_args(args);
        let accounts = book_accounts(bu, &call);
        let key = (b.book, call.transfer.clone(), book_key(bu, &call));
        let fresh = matches!(call.op.as_str(), "do" | "hold") && !self.keys.contains(&key) && accounts.iter().all(|(k, a)| !self.touched.contains(&(b.book, k.clone(), a.clone())));
        if fresh {
            if let Some(short) = self.short_of(b.book, &call).filter(|x| !x.is_empty()) {
                if let Some(fund) = self.funding(b.book, &short) {
                    if self.choose(&format!("book {} holds", s.site), 2) == 1 && !self.before(b.book, fund) {
                        return Ctl::Stop;
                    }
                }
            }
            // done before the run: the same operation, which chobo answers as done before; or one with
            // the same key and another value, which it refuses as a conflict. A hold done before may
            // have been posted or voided since, or have expired, which the case finds it as, and what
            // the run does to it next is answered so
            let fits = self.short_of(b.book, &call).is_some_and(|x| x.is_empty());
            let unmoved = crate::computed::moment_ms(self.clock.start()) == Some(self.clock.at_ms());
            if unmoved {
                let (written, _) = render::book_call(m, task, b, &given);
                let op = json!({ "op": written["op"], "kind": written["transfer"], "args": written["args"] });
                let mut ways: Vec<Vec<Value>> = vec![vec![]];
                if fits {
                    ways.push(vec![op.clone()]);
                }
                if let Some(other) = conflicting(bu, &op) {
                    ways.push(vec![other]);
                }
                if let (true, "hold", Some(t)) = (fits, call.op.as_str(), bu.transfer(&call.transfer)) {
                    let key: Map<String, Value> = written["args"].as_object().into_iter().flatten().filter(|(n, _)| t.key.contains(n)).map(|(n, v)| (n.clone(), v.clone())).collect();
                    for then in ["post", "void"] {
                        ways.push(vec![op.clone(), json!({ "op": then, "kind": written["transfer"], "args": key })]);
                    }
                    if let Some(ritsu_ports::Expiry::After(secs)) = t.pending {
                        ways.push(vec![op.clone(), json!({ "op": "pass", "duration": crate::computed::duration_text(secs) })]);
                    }
                }
                // a transfer done before whose amount was taken since, by another operation
                if fits && call.op == "do" {
                    if let Some(draw) = self.drawing(b.book, &call) {
                        ways.push(vec![op.clone(), draw]);
                    }
                }
                if ways.len() > 1 {
                    let c = self.choose(&format!("book {} before", s.site), ways.len());
                    if !ways[c].is_empty() && !self.before(b.book, ways[c].clone()) {
                        return Ctl::Stop;
                    }
                }
            }
        }
        self.keys.insert(key.clone());
        self.labels.insert(format!("{}:chobo", s.site));
        let callee = Callee::Task(t);
        let got = self.oracle.as_mut().and_then(|o| o.answer(m, render::View::Asl, &callee, &given));
        // an operation done before, which chobo answers as such, changes nothing and reads no
        // balance: what is done before the run to the accounts it names does not change its answer
        if !matches!(&got, Some(Ok(v)) if v["ok"]["result"] == json!("done_before")) {
            for (k, a) in accounts {
                self.touched.insert((b.book, k, a));
            }
        }
        // a case follows the hold as the book has it now
        let state = match target {
            Some(Target::Case(c)) => self.oracle.as_ref().and_then(|o| o.ledger(b.book)).and_then(|l| l.holds().into_iter().find(|(k, v, _)| *k == key.1 && *v == key.2)).and_then(|(_, _, st)| m.machine(*c).states.iter().position(|x| *x == st)),
            _ => None,
        };
        match got {
            Some(Ok(v)) => match v.get("ok") {
                Some(ok) => {
                    // as the walk names an answer it chooses: the state the case starts in or goes to,
                    // or done before
                    let label = match (ok["result"] == json!("done_before"), target, state, &task.machine) {
                        (true, ..) => "done before".to_string(),
                        (false, Some(Target::Case(c)), Some(st), Some(TaskMachine::Starts { .. })) => format!("starts:{}", m.machine(*c).states[st]),
                        (false, Some(Target::Case(c)), Some(st), _) => format!("to:{}", m.machine(*c).states[st]),
                        _ => "ok".to_string(),
                    };
                    self.labels.insert(format!("{}:{label}", s.site));
                    if b.op == "hold" {
                        self.held.insert(key.clone(), given.clone());
                    }
                    let hold = render::book_value(m, b, &given);
                    self.assign(target, hold, state);
                    Ctl::Next
                }
                None => {
                    let kind = v["error"].as_str().unwrap_or("failure").to_string();
                    let reason = v["cause"].as_str().unwrap_or(&kind).to_string();
                    let name = error_name(m, &callee, &kind);
                    let taken = self.taken_by(&callee, &kind, &name, handlers);
                    self.labels.insert(format!("{}:refused:{reason}", s.site));
                    self.labels.insert(format!("{}:{taken}", s.site));
                    if let (Some(Target::Case(c)), Some(st)) = (target, state) {
                        self.cases[*c] = Some(st);
                    }
                    let tries = tries_of(m, &callee, &kind);
                    self.tries_time(s, &callee, &vec![kind.clone(); tries]);
                    self.take_error(&callee, &kind, &name, handlers)
                }
            },
            _ => {
                self.dead = true;
                self.stopped = true;
                Ctl::Stop
            }
        }
    }

    /// A post of part of a hold this run made: each amount the call gives by a value left open is
    /// what the hold holds or less, or one of them is more, which chobo refuses. True when an amount
    /// changed, which `given` then has.
    fn steer_post(&mut self, s: &TStmt, b: &BookOp, args: &[(String, TExpr)], call: &ritsu_ports::BookCall, given: &mut Map<String, Value>) -> bool {
        let m = self.m;
        let bu = &m.books[b.book];
        let Some(t) = bu.transfer(&b.transfer) else { return false };
        let Some(held) = self.held.get(&(b.book, b.transfer.clone(), book_key(bu, call))).cloned() else { return false };
        // each amount given by a value left open, with what the hold holds of it
        let open: Vec<(usize, String, i64)> = t
            .params
            .iter()
            .filter(|p| p.unit.is_some())
            .filter_map(|p| {
                let id = args.iter().find(|(n, _)| *n == p.name).and_then(|(_, e)| self.steerable_at(e))?;
                Some((id, p.name.clone(), held.get(&p.name)?.as_i64()?))
            })
            .collect();
        if open.is_empty() {
            return false;
        }
        // where a check made before read the amount, as it leaves the check
        let fits = |id: usize, n: i64, ex: &Ex| ex.holes[id].2.is_none_or(|rg| rg.contains(n)) && ex.checks_stay(&[(id, json!(n))]);
        let over = open.iter().find(|(id, _, h)| fits(*id, h + 1, self)).map(|(id, name, h)| (*id, name.clone(), h + 1));
        let mut changed = false;
        if let Some((id, name, n)) = over.filter(|_| self.choose(&format!("post {}", s.site), 2) == 1) {
            self.holes[id].1 = Some(json!(n));
            given.insert(name, json!(n));
            return true;
        }
        for (id, name, h) in open {
            let now = self.holes[id].1.as_ref().and_then(|v| v.as_i64());
            if now.is_some_and(|n| n > h) && fits(id, h, self) {
                self.holes[id].1 = Some(json!(h));
                given.insert(name, json!(h));
                changed = true;
            }
        }
        changed
    }

    /// How the walk names a call's error that comes out of a language's answer: by the handler at
    /// the call that takes it, as a way chosen for it is named (`on:<the handler's first error>`),
    /// or `unhandled`.
    fn taken_by(&self, callee: &Callee, kind: &str, name: &str, handlers: &[THandler]) -> String {
        let m = self.m;
        let taker = handlers.iter().find(|h| {
            h.errors.iter().any(|e| match e {
                HErr::Failure => true,
                HErr::Timeout => kind == "timeout",
                HErr::Declared(n) => n == kind || render::asl_error(m, callee, &HErr::Declared(n.clone())).iter().any(|x| x == name),
            })
        });
        match taker.and_then(|h| h.errors.first()) {
            Some(HErr::Declared(n)) => format!("on:{n}"),
            Some(HErr::Timeout) => "on:timeout".into(),
            Some(HErr::Failure) => "on:failure".into(),
            None => "unhandled".into(),
        }
    }

    /// The moment a `wait until` waits for, where it is a value left open (`moments`): one the run's
    /// time stays in the period at, or, where the value's own range has room, one past the period's
    /// end, which the next statement that reads `now` finds outside it. A moment in the period stays
    /// as it is, and so does one past its end; else the moment is a day after the clock, or the
    /// period's last second.
    fn steer_wait(&mut self, s: &TStmt, at: &TExpr) {
        if !self.moments {
            return;
        }
        let Some(hi) = self.m.period.as_ref().and_then(|p| p.range.hi) else { return };
        let Some(id) = self.steerable_at(at) else { return };
        let Some(t) = self.holes[id].1.as_ref().and_then(|v| v.as_str()).and_then(moment_of) else { return };
        let own = self.holes[id].2;
        let now = self.clock.at_ms().div_euclid(1000);
        // where a check made before read the moment, one that leaves the check as it came out
        let fits = |n: i64| own.is_none_or(|rg| rg.contains(n)) && (n == t || self.checks_stay(&[(id, json!(moment_text(n)))]));
        let inside = if t <= hi { Some(t) } else { [now + 86_400, hi].into_iter().find(|n| *n <= hi && *n >= now && fits(*n)) };
        let past = if t > hi { Some(t) } else { Some(hi + 1).filter(|n| fits(*n)) };
        let (Some(inside), Some(past)) = (inside, past) else { return };
        let n = if self.choose(&format!("wait {}", s.site), 2) == 0 { inside } else { past };
        if n != t {
            self.holes[id].1 = Some(json!(moment_text(n)));
        }
    }

    /// The values left open that a call of a date gives its inputs: each where the input's range
    /// takes it, or, where the value's own range has room, one of them just outside, which koyomi
    /// refuses. A value the walk has not left open (`now`, an input of the run, a value it read)
    /// stays as it is.
    fn steer_date(&mut self, s: &TStmt, r: usize, args: &[(String, TExpr)]) {
        let m = self.m;
        let Some(d) = m.rules[r].date() else { return };
        let Some(ranges) = self.oracle.as_mut().and_then(|o| o.date_ranges(m, r)) else { return };
        let number = |v: &Value, day: bool| if day { v.as_str().and_then(ritsu_ports::day_of) } else { v.as_i64() };
        let value = |n: i64, day: bool| if day { json!(day_text(n)) } else { json!(n) };
        // each value left open: the hole, whether it is a day, and the range the input takes
        let mut open = Vec::new();
        for (name, _, day) in &d.params {
            let Some(id) = args.iter().find(|(n, _)| n == name).and_then(|(_, e)| self.steerable_at(e)) else { continue };
            let Some((_, lo, hi)) = ranges.iter().find(|(n, ..)| n == name) else { continue };
            if self.holes[id].1.as_ref().and_then(|v| number(v, *day)).is_some() {
                open.push((id, *day, *lo, *hi));
            }
        }
        if open.is_empty() {
            return;
        }
        // just past the high end, else just before the low end, where the value's own range takes it
        // (and where a check made before read the value, as it leaves the check)
        let outside = open.iter().find_map(|(id, day, lo, hi)| {
            let own = self.holes[*id].2;
            [hi.checked_add(1), lo.checked_sub(1)].into_iter().flatten().find(|n| own.is_none_or(|rg| rg.contains(*n)) && self.checks_stay(&[(*id, value(*n, *day))])).map(|n| (*id, *day, n))
        });
        if let Some((id, day, n)) = outside {
            if self.choose(&format!("date {}", s.site), 2) == 1 {
                self.holes[id].1 = Some(value(n, day));
                return;
            }
        }
        for (id, day, lo, hi) in open {
            let own = self.holes[id].2;
            let now = self.holes[id].1.as_ref().and_then(|v| number(v, day)).unwrap_or(lo);
            // the nearest value both ranges take
            let (a, b) = (own.and_then(|rg| rg.lo).map_or(lo, |x| x.max(lo)), own.and_then(|rg| rg.hi).map_or(hi, |x| x.min(hi)));
            if a <= b && !(lo..=hi).contains(&now) {
                self.set_open(vec![(id, value(now.clamp(a, b), day))]);
            }
        }
    }

    /// Operations done to the book `b` before the run: the books the run keeps have them, and the
    /// scenario writes them. False, and the run is not written, when the book refuses one.
    fn before(&mut self, b: usize, ops: Vec<Value>) -> bool {
        let m = self.m;
        for op in ops {
            if self.oracle.as_mut().is_none_or(|o| o.do_before(m, b, &op).is_err()) {
                self.dead = true;
                self.stopped = true;
                return false;
            }
            self.before_ops[b].push(op);
        }
        true
    }

    /// What an operation of `do` or `hold` finds short on the book `b` as the run has it now: each
    /// account a move draws from past its lower bound, with how much more it takes. None when a
    /// move would take an account past its upper bound, which nothing done before the run mends.
    fn short_of(&self, b: usize, call: &ritsu_ports::BookCall) -> Option<Vec<(String, Vec<String>, i128)>> {
        let bu = &self.m.books[b];
        let t = bu.transfer(&call.transfer)?;
        let ledger = self.oracle.as_ref()?.ledger(b)?;
        let mut out = Vec::new();
        for mv in &t.moves {
            let n = match &mv.amount {
                ritsu_ports::MoveAmount::Param(p) => call.args.iter().find(|(x, _)| x == p).and_then(|(_, v)| v.as_ref().ok().copied()).unwrap_or(0),
                ritsu_ports::MoveAmount::Literal(v) => *v,
            };
            let (from, from_args) = account_of(&mv.from, call);
            if let Some(l) = bu.facts.accounts.iter().find(|a| a.name == from).and_then(|a| a.lower.as_ref()) {
                let bal = ledger.balance(&from, &from_args).ok()?;
                let left = bal.posted - bal.held_out - n;
                if left < l.value {
                    out.push((from, from_args, l.value - left));
                }
            }
            let (to, to_args) = account_of(&mv.to, call);
            if let Some(u) = bu.facts.accounts.iter().find(|a| a.name == to).and_then(|a| a.upper.as_ref()) {
                let bal = ledger.balance(&to, &to_args).ok()?;
                if bal.posted + bal.held_in + n > u.value {
                    return None;
                }
            }
        }
        Some(out)
    }

    /// Operations done before the run that give each short account what it lacks: a transfer of
    /// the book done at once, whose one move goes into the account from one with no lower bound,
    /// with the account's values, the amount it lacks, and the rest made here. None when the book
    /// has no such transfer for one of them.
    fn funding(&mut self, b: usize, short: &[(String, Vec<String>, i128)]) -> Option<Vec<Value>> {
        let bu = &self.m.books[b];
        let mut ops = Vec::new();
        for (kind, args, need) in short {
            let mut found = None;
            for t in &bu.facts.transfers {
                let [mv] = t.moves.as_slice() else { continue };
                if t.pending.is_some() || mv.to.account != *kind {
                    continue;
                }
                if bu.facts.accounts.iter().find(|a| a.name == mv.from.account).is_none_or(|a| a.lower.is_some()) {
                    continue;
                }
                let mut vals = Map::new();
                let mut fits = true;
                for (i, a) in mv.to.args.iter().enumerate() {
                    match a {
                        Ok(p) => {
                            vals.insert(p.clone(), json!(args.get(i).cloned().unwrap_or_default()));
                        }
                        Err(lit) => fits &= args.get(i) == Some(lit),
                    }
                }
                match &mv.amount {
                    ritsu_ports::MoveAmount::Param(p) => {
                        vals.insert(p.clone(), json!(i64::try_from(*need).ok()?));
                    }
                    ritsu_ports::MoveAmount::Literal(v) => fits &= *v >= *need,
                }
                if !fits {
                    continue;
                }
                for p in &t.params {
                    if !vals.contains_key(&p.name) {
                        self.made_values += 1;
                        let v = if p.unit.is_some() { json!(0) } else { json!(format!("{}-{}", p.name, self.made_values)) };
                        vals.insert(p.name.clone(), v);
                    }
                }
                found = Some(json!({ "op": "do", "kind": t.name, "args": vals }));
                break;
            }
            ops.push(found?);
        }
        Some(ops)
    }

    /// An operation that takes what the transfer `call` puts in an account with a lower bound out of
    /// it again: a transfer with one move, from the account to one with no lower bound, its amount
    /// what `call` puts there, and its other values made here, so that its key is its own. Done
    /// before the run after `call`, it leaves the account as it was before `call`. None when the
    /// book has no such transfer.
    fn drawing(&mut self, b: usize, call: &ritsu_ports::BookCall) -> Option<Value> {
        let bu = &self.m.books[b];
        let t = bu.transfer(&call.transfer)?;
        let lower = |name: &str| bu.facts.accounts.iter().find(|a| a.name == name).is_some_and(|a| a.lower.is_some());
        let mv = t.moves.iter().find(|mv| lower(&mv.to.account))?;
        let n = match &mv.amount {
            ritsu_ports::MoveAmount::Param(p) => call.args.iter().find(|(x, _)| x == p).and_then(|(_, v)| v.as_ref().ok().copied())?,
            ritsu_ports::MoveAmount::Literal(v) => *v,
        };
        let (kind, args) = account_of(&mv.to, call);
        for d in &bu.facts.transfers {
            let [dm] = d.moves.as_slice() else { continue };
            if dm.from.account != kind || lower(&dm.to.account) {
                continue;
            }
            let mut vals = Map::new();
            let mut fits = n > 0;
            for (i, a) in dm.from.args.iter().enumerate() {
                match a {
                    Ok(p) => {
                        vals.insert(p.clone(), json!(args.get(i).cloned().unwrap_or_default()));
                    }
                    Err(lit) => fits &= args.get(i) == Some(lit),
                }
            }
            match &dm.amount {
                ritsu_ports::MoveAmount::Param(p) => {
                    vals.insert(p.clone(), json!(i64::try_from(n).ok()?));
                }
                ritsu_ports::MoveAmount::Literal(v) => fits &= *v > 0 && *v <= n,
            }
            if !fits || d.params.iter().any(|p| p.unit.is_some() && !vals.contains_key(&p.name)) {
                continue;
            }
            for p in &d.params {
                if !vals.contains_key(&p.name) {
                    self.made_values += 1;
                    vals.insert(p.name.clone(), json!(format!("{}-{}", p.name, self.made_values)));
                }
            }
            let op = if d.pending.is_some() { "hold" } else { "do" };
            return Some(json!({ "op": op, "kind": d.name, "args": vals }));
        }
        None
    }

    /// A statement reads `now`: what the clock reads, which `now` in it reads too, as the reference
    /// interpreter reads it. None, and the run is not written, when the clock cannot read it.
    fn read_now(&mut self) -> Option<Value> {
        match self.clock.read() {
            Ok((at, moved)) => {
                if moved > 0.0 {
                    let after = self.clock.at_ms();
                    if let Some(o) = self.oracle.as_mut() {
                        o.pass(moved, Passing::Now, None, after);
                    }
                }
                let v = json!(at);
                self.vars.insert(crate::interp::NOW_VAR.into(), v.clone());
                Some(v)
            }
            Err(_) => {
                self.dead = true;
                self.stopped = true;
                None
            }
        }
    }

    /// Time goes by on the run's clock, when it moves, and the books are told.
    fn go(&mut self, seconds: f64, why: Passing, until: Option<&str>) {
        if let Some(after) = self.clock.go(seconds) {
            if let Some(o) = self.oracle.as_mut() {
                o.pass(seconds, why, until, after);
            }
        }
    }

    /// The time the tries of a call take on a clock that moves, as the reference interpreter counts
    /// it: each try that timed out its timeout, and before each try again the wait its retrier asks
    /// for. `kinds` is each try's answer, in order: the kind of an error, or `ok`.
    fn tries_time(&mut self, s: &TStmt, callee: &Callee, kinds: &[String]) {
        if !self.clock.moves() {
            return;
        }
        let m = self.m;
        let rs = retriers_timed(m, callee);
        let mut counts = vec![0u32; rs.len()];
        for k in kinds {
            if k == "ok" {
                break;
            }
            if k == "timeout" {
                match callee {
                    Callee::Task(t) => {
                        let deadline = m.tasks[*t].deadline.as_ref().and_then(|p| match &s.kind {
                            TK::Call { args, .. } => args.iter().find(|(n, _)| n == p).map(|(_, e)| e.clone()),
                            _ => None,
                        });
                        match deadline {
                            Some(e) => {
                                let v = self.value(&e);
                                let given = self.fill(&v).as_str().unwrap_or_default().to_string();
                                if let Some(seconds) = self.clock.timeout_until(&given) {
                                    self.go(seconds, Passing::Timeout, Some(&given));
                                }
                            }
                            None => {
                                if let Some(secs) = m.tasks[*t].timeout {
                                    self.go(secs as f64, Passing::Timeout, None);
                                }
                            }
                        }
                    }
                    Callee::Rule(_) => self.go(crate::check::RULE_SECONDS as f64, Passing::Timeout, None),
                }
            }
            let name = error_name(m, callee, k);
            for (i, (names, max, every, backoff)) in rs.iter().enumerate() {
                if matches_error(names, &name) {
                    if counts[i] < *max {
                        let delay = (*every as f64) * backoff.powi(counts[i] as i32);
                        counts[i] += 1;
                        self.go(delay, Passing::Retry, None);
                    }
                    break;
                }
            }
        }
    }

    /// What `e` comes to now, every value left open in it chosen here.
    fn chosen(&mut self, e: &TExpr, at: &str) -> Value {
        let v = match e {
            TExpr::Var { name, fields, .. } => {
                let mut v = self.vars.get(name).cloned().unwrap_or(Value::Null);
                for (k, f) in fields.iter().enumerate() {
                    v = self.read(v, &format!("{at}.{k}"));
                    v = v.get(f).cloned().unwrap_or(Value::Null);
                }
                v
            }
            _ => crate::interp::value_of(&self.vars, e),
        };
        self.settled(v, at)
    }

    /// A value with every value left open in it chosen: an enum, a bool, a value that may be absent,
    /// the length of a list, at every depth.
    fn settled(&mut self, v: Value, at: &str) -> Value {
        let v = match Ex::hole_id(&v) {
            Some(id) if matches!(self.holes[id].0, Ty::List(_)) => Value::Array(self.items(v, 1, at)),
            Some(_) => self.read(v, at),
            None => v,
        };
        match v {
            Value::Object(o) => {
                let mut out = Map::new();
                for (k, x) in o {
                    let x = self.settled(x, &format!("{at}.{k}"));
                    out.insert(k, x);
                }
                Value::Object(out)
            }
            Value::Array(a) => Value::Array(a.into_iter().enumerate().map(|(i, x)| self.settled(x, &format!("{at}[{i}]"))).collect()),
            other => other,
        }
    }

    /// A call's error, `kind` as the `.flow` names it and `name` as Step Functions does: the first
    /// handler that takes it runs; else the round ends, or the run, after `on failure`.
    fn take_error(&mut self, callee: &Callee, kind: &str, name: &str, handlers: &[THandler]) -> Ctl {
        let m = self.m;
        for h in handlers {
            let hit = h.errors.iter().any(|e| match e {
                HErr::Failure => true,
                HErr::Timeout => kind == "timeout",
                HErr::Declared(n) => n == kind || render::asl_error(m, callee, &HErr::Declared(n.clone())).iter().any(|x| x == name),
            });
            if hit {
                return self.block(&h.body);
            }
        }
        if self.par_depth > 0 {
            return self.halt(true);
        }
        self.unhandled()
    }

    /// A task's error that nothing took: `on failure` runs, and the run ends.
    fn unhandled(&mut self) -> Ctl {
        if self.par_depth > 0 {
            return self.halt(true);
        }
        self.ended = Some("fail");
        if !self.in_on_failure && !self.in_on_cancel {
            if let Some(block) = self.m.on_failure.clone() {
                self.in_on_failure = true;
                self.labels.insert("on failure".into());
                self.block(&block);
            }
        }
        self.stopped = true;
        Ctl::Stop
    }

    fn assign(&mut self, target: Option<&Target>, v: Value, st: Option<usize>) {
        match target {
            Some(Target::Let(x)) => {
                self.vars.insert(x.clone(), v);
            }
            Some(Target::Case(c)) => {
                self.vars.insert(self.m.cases[*c].name.clone(), v);
                if let Some(st) = st {
                    self.cases[*c] = Some(st);
                }
            }
            None => {}
        }
    }
}

fn error_name(m: &Model, callee: &Callee, kind: &str) -> String {
    match kind {
        "timeout" => "States.Timeout".into(),
        "failure" => crate::interp::asl_failure(m, callee).into(),
        other => render::asl_error(m, callee, &HErr::Declared(other.to_string())).into_iter().next().unwrap_or_else(|| other.to_string()),
    }
}

/// How many times a call tries before an error of this kind comes through: once, and again as
/// often as the first retrier that takes it asks.
fn tries_of(m: &Model, callee: &Callee, kind: &str) -> usize {
    let name = error_name(m, callee, kind);
    1 + retriers(m, callee).iter().find(|(names, _)| matches_error(names, &name)).map(|(_, max)| *max as usize).unwrap_or(0)
}

/// The retriers of a call, as the targets have them: the errors each takes, how many times it
/// tries again, the wait before the first, and how much longer each wait is than the one before.
fn retriers_timed(m: &Model, callee: &Callee) -> Vec<(Vec<String>, u32, u64, f64)> {
    let v = match callee {
        Callee::Rule(_) => crate::asl::rule_retriers(),
        Callee::Task(t) => match &m.tasks[*t].retry {
            Some(r) => crate::asl::retriers(m, callee, &m.tasks[*t], r),
            None => json!([]),
        },
    };
    v.as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let names = r["ErrorEquals"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect();
            (names, r["MaxAttempts"].as_u64().unwrap_or(3) as u32, r["IntervalSeconds"].as_u64().unwrap_or(1), r["BackoffRate"].as_f64().unwrap_or(2.0))
        })
        .collect()
}

/// An account a move of an operation names: its kind, and the values of its parameters.
fn account_of(r: &ritsu_ports::MoveRef, call: &ritsu_ports::BookCall) -> (String, Vec<String>) {
    let arg = |p: &str| {
        call.args.iter().find(|(n, _)| n == p).map(|(_, v)| match v {
            Ok(n) => n.to_string(),
            Err(s) => s.clone(),
        })
    };
    (r.account.clone(), r.args.iter().map(|a| a.as_ref().map_or_else(|lit| lit.clone(), |p| arg(p).unwrap_or_default())).collect())
}

/// The accounts an operation of `do` or `hold` names: the two of each move.
fn book_accounts(bu: &BookUse, call: &ritsu_ports::BookCall) -> Vec<(String, Vec<String>)> {
    let Some(t) = bu.transfer(&call.transfer) else { return Vec::new() };
    t.moves.iter().flat_map(|mv| [account_of(&mv.from, call), account_of(&mv.to, call)]).collect()
}

/// An operation with the same key as `op` (written as the scenario's `books` writes one) and another
/// value of the first parameter outside the key: one less, or one more for 1, of an amount, and
/// another string. None for a transfer whose parameters are all its key.
fn conflicting(bu: &BookUse, op: &Value) -> Option<Value> {
    let t = bu.transfer(op["kind"].as_str()?)?;
    let p = t.params.iter().find(|p| !t.key.contains(&p.name))?;
    let mut other = op.clone();
    let v = &op["args"][p.name.as_str()];
    other["args"][p.name.as_str()] = match (v.as_i64(), v.as_str()) {
        (Some(n), _) => json!(if n > 1 { n - 1 } else { n + 1 }),
        (None, Some(s)) => json!(format!("{s}-before")),
        _ => return None,
    };
    Some(other)
}

/// The values of the key of an operation's transfer.
fn book_key(bu: &BookUse, call: &ritsu_ports::BookCall) -> Vec<String> {
    let Some(t) = bu.transfer(&call.transfer) else { return Vec::new() };
    t.key
        .iter()
        .map(|p| {
            call.args.iter().find(|(n, _)| n == p).map_or(String::new(), |(_, v)| match v {
                Ok(n) => n.to_string(),
                Err(s) => s.clone(),
            })
        })
        .collect()
}

fn retriers(m: &Model, callee: &Callee) -> Vec<(Vec<String>, u32)> {
    let v = match callee {
        Callee::Rule(_) => crate::asl::rule_retriers(),
        Callee::Task(t) => match &m.tasks[*t].retry {
            Some(r) => crate::asl::retriers(m, callee, &m.tasks[*t], r),
            None => json!([]),
        },
    };
    v.as_array()
        .unwrap()
        .iter()
        .map(|r| (r["ErrorEquals"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect(), r["MaxAttempts"].as_u64().unwrap_or(3) as u32))
        .collect()
}
