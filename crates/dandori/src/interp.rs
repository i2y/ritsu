//! The reference interpreter: the meaning of a `.flow`, run against a scenario — the
//! execution's input and the answers the calls get, in order. It reports every call as the
//! chosen target would send it (see `render`), with the answer it got, every wait, and how
//! the run ended. The runners of the generated ASL and TypeScript report the same, so a
//! difference between them and this is a defect of a generator.
//!
//! Errors and retries follow the ASL the generator writes: the retriers are the same
//! list (`asl::retriers`), matched the way Step Functions matches them.
//!
//! A scenario's answer `{"cancel": true}` is a cancellation that comes while that call is out:
//! the run stops there, `on cancel` runs if there is one, and the run ends as cancelled.
//!
//! The rounds of a `for … in parallel` run here one after another, in the list's order.
//! Every round runs to its end; then, if any failed, the loop fails as the first of them
//! (by place in the list) did. A round does not see or set another round's variables, so
//! no order in which the platforms interleave them can come out differently.

use crate::model::*;
use crate::render::{self, View};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

pub const SCRIPTED_CAUSE: &str = "scripted";
pub const TEST_FAILURE: &str = "Dandori.Test.Failure";
/// An agent's failure as Step Functions sees it when the model refuses: the answer has nothing
/// to read, and reading it fails the Task. The ASL runner plays an agent's failure this way.
pub const AGENT_REFUSED: &str = "States.QueryEvaluationError";

/// The name Step Functions gives a failure of this callee in a scenario.
pub fn asl_failure(m: &Model, callee: &Callee) -> &'static str {
    match callee {
        Callee::Task(t) if matches!(m.tasks[*t].via(Platform::StepFunctions), Some(Via::Agent { .. })) => AGENT_REFUSED,
        _ => TEST_FAILURE,
    }
}

enum Ctl {
    Next,
    Break,
    Stop,
}

/// Where a run takes the answers of some of its calls from, in place of the scenario: `ritsu run`
/// asks rulec for a rule's outputs, koyomi for a date, and chobo for what an operation of a book
/// comes to (crate::computed; ritsu's DESIGN 7.9). Each answer is written as a scenario writes one
/// (`{"ok": …}`, `{"error": …}`, with the cause of an error under `cause`), so the run goes on as it
/// would on a scenario that held it: the same retries, the same names for an error, the same checks
/// of what comes back. Time is told as it goes by, for a book whose holds expire.
pub trait Answers {
    /// The answer to one try of this call, when this side takes the call; None for one the scenario
    /// answers. Err stops the run: the call cannot be answered at all, and why, in both languages.
    fn answer(&mut self, m: &Model, view: View, callee: &Callee, args: &Map<String, Value>) -> Option<Result<Value, ritsu_base::text::Text>>;

    /// The scenario answered a try of a call this side does not take.
    fn scripted(&mut self, _m: &Model, _callee: &Callee, _args: &Map<String, Value>, _answer: &Value) {}

    /// Time went by on the run's clock: so many seconds, why, the moment a `wait until` or a `timeout
    /// until` was given, and the moment after, in milliseconds since 1970-01-01 in UTC.
    fn pass(&mut self, _seconds: f64, _why: Passing, _until: Option<&str>, _after_ms: i64) {}

    /// The run stops because the scenario cannot be run as it is written (a moment of its `now` that
    /// is not a time, or that comes before the run's clock): what to say of it.
    fn cannot_run(&mut self, _why: &ritsu_base::text::Text) {}
}

/// Why time goes by in a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Passing {
    Wait,
    Retry,
    Timeout,
    /// a statement read `now` at a later moment the scenario gives it (its `now`, a list)
    Now,
}

/// The clock a run reads `now` from (DESIGN 5 and 20). A run starts at the first moment of the
/// scenario's `now`; each statement that reads `now` reads it once, and the k-th one after the start
/// reads the moment k+1 of the list when there is one, else the clock. The clock starts at the first
/// moment, and is set to each moment of the list that is read. A clock that moves also goes on by
/// each `wait` (to the moment of a `wait until`), each wait before a retry and each timeout, as time
/// goes on where the workflow runs; a moment of the list before it is one the run cannot read, for
/// time does not go back. A clock that does not move is the one the runners give the platforms
/// (their waits made short): the start reads the first moment, and every `now` after it the second,
/// or the first when there is one.
#[derive(Clone, Debug)]
pub struct Clock {
    /// the moments the scenario's `now` gives, the start's first
    given: Vec<String>,
    moves: bool,
    /// how many times `now` was read after the start
    reads: usize,
    /// what `now` reads off the clock: a moment of the scenario's as it is written, until the
    /// clock moves on from it, and then the moment it shows, to the second
    shown: String,
    /// the moment the clock shows, in milliseconds since 1970-01-01 in UTC (a clock that moves)
    at_ms: i64,
}

impl Clock {
    /// The clock of a run of the scenario: one that moves, or one that does not. A clock that moves
    /// reads moments only: a `now` that is not a time is said.
    pub fn of(sc: &Value, moves: bool) -> Result<Clock, ritsu_base::text::Text> {
        let read = |x: &Value| x.as_str().unwrap_or(render::SCENARIO_NOW).to_string();
        let given: Vec<String> = match &sc["now"] {
            Value::Array(v) if !v.is_empty() => v.iter().map(read).collect(),
            Value::Array(_) => vec![render::SCENARIO_NOW.to_string()],
            v => vec![read(v)],
        };
        let mut at_ms = 0;
        if moves {
            for g in &given {
                if crate::computed::moment_ms(g).is_none() {
                    return Err(ritsu_base::tr!(
                        "シナリオの `now` の `{g}` は時刻（`2026-03-31T15:30:00Z` の形）ではありません",
                        "the scenario's `now`, `{g}`, is not a time (`2026-03-31T15:30:00Z`)"
                    ));
                }
            }
            at_ms = crate::computed::moment_ms(&given[0]).unwrap_or_default();
        }
        let shown = given[0].clone();
        Ok(Clock { given, moves, reads: 0, shown, at_ms })
    }

    /// The moment the run starts at.
    pub fn start(&self) -> &str {
        &self.given[0]
    }

    /// What `now` reads after the start before any statement reads it: the moment every `now` after
    /// the start reads on a clock that does not move.
    pub fn later(&self) -> &str {
        self.given.get(1).unwrap_or(&self.given[0])
    }

    /// Whether the clock moves on as time goes by.
    pub fn moves(&self) -> bool {
        self.moves
    }

    /// The moment the clock shows, in milliseconds since 1970-01-01 in UTC (a clock that moves).
    pub fn at_ms(&self) -> i64 {
        self.at_ms
    }

    /// The moments the scenario gives `now`, the start's first.
    pub fn given(&self) -> &[String] {
        &self.given
    }

    /// One more moment for the scenario's `now`, read by the next statement that reads it after the
    /// ones given (what a scenario generator gives a run that leaves the period it runs in).
    pub fn give(&mut self, moment: String) {
        self.given.push(moment);
    }

    /// The next statement that reads `now` reads it: the moment, and how long the clock went on to
    /// it, for a moment of the scenario's later than a clock that moves. Err for one before it.
    pub fn read(&mut self) -> Result<(String, f64), ritsu_base::text::Text> {
        self.reads += 1;
        match self.given.get(self.reads).cloned() {
            Some(g) if self.moves => {
                let ms = crate::computed::moment_ms(&g).unwrap_or_default();
                if ms < self.at_ms {
                    let (k, clock) = (self.reads + 1, crate::computed::moment_text(self.at_ms));
                    return Err(ritsu_base::tr!(
                        "シナリオの `now` の {k} 番目の時刻 `{g}` は、それを読む文が走る時刻 {clock} より前です。時間は戻らないので、この実行はその時刻を読めません",
                        "moment {k} of the scenario's `now`, `{g}`, comes before {clock}, when the statement that reads it runs: time does not go back, so the run cannot read it"
                    ));
                }
                let moved = (ms - self.at_ms) as f64 / 1000.0;
                self.at_ms = ms;
                self.shown = g.clone();
                Ok((g, moved))
            }
            Some(g) => {
                self.shown = g.clone();
                Ok((g, 0.0))
            }
            None => Ok((self.shown.clone(), 0.0)),
        }
    }

    /// Let so many seconds go by: the moment after, in milliseconds, on a clock that moves; None on
    /// one that does not.
    pub fn go(&mut self, seconds: f64) -> Option<i64> {
        if !self.moves {
            return None;
        }
        let ms = (seconds * 1000.0).round().max(0.0) as i64;
        if ms > 0 {
            self.at_ms += ms;
            self.shown = crate::computed::moment_text(self.at_ms - self.at_ms.rem_euclid(1000));
        }
        Some(self.at_ms)
    }

    /// How long a `wait until` the moment `at` waits: none for a moment that has come already, or for
    /// a value that is not a time (the check holds `wait until` to timestamps).
    pub fn until(&self, at: &str) -> Option<f64> {
        crate::computed::moment_ms(at).map(|t| (t - self.at_ms).max(0) as f64 / 1000.0)
    }

    /// How long the wait of a `timeout until` the moment `at` lasts before it times out: to the
    /// moment, rounded up to the second, and a second at the least (DESIGN 16).
    pub fn timeout_until(&self, at: &str) -> Option<f64> {
        crate::computed::moment_ms(at).map(|t| ((t - self.at_ms) as f64 / 1000.0).ceil().max(1.0))
    }
}

struct Run<'a> {
    m: &'a Model,
    /// where the answers come from, besides the scenario
    hook: Option<&'a mut dyn Answers>,
    view: View,
    execution: String,
    answers: Vec<Value>,
    next_answer: usize,
    vars: BTreeMap<String, Value>,
    rounds: Vec<u64>,
    steps: Vec<Value>,
    end: Option<Value>,
    in_on_failure: bool,
    /// a cancellation stopped the run; `on cancel` has not run yet
    cancelled: bool,
    in_on_cancel: bool,
    /// why the run stopped before its end
    error: Option<ritsu_base::text::Text>,
    /// how many `for … in parallel` rounds are running around the current statement
    par_depth: usize,
    /// how the current parallel round failed, kept until every round is done
    pending: Option<Pending>,
    /// for every answer taken: the task that took it, and whether it waits for a callback
    callees: Vec<CallInfo>,
    /// what the run went through, in order
    visits: Vec<Visit>,
    /// what `now` reads
    clock: Clock,
}

/// One thing a run went through, for `doc` to draw the run on the flow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Visit {
    /// a statement starts, by its site
    Stmt(usize),
    /// a match takes its arm, by the arm's place in the match
    Arm(usize, usize),
    /// a try of a call comes back: `ok`, the kind of the error, or `cancel`
    Answer(usize, String),
    /// a call's error goes to its handler, by the handler's place under the call
    Handler(usize, usize),
    /// a call's error goes on with nothing at the call to take it
    Unhandled(usize),
    /// a round of a loop starts: the loop's site, and the round, from 0
    Round(usize, u64),
    /// a loop runs out of rounds or of items, without a `break`
    Done(usize),
    /// a loop is left by a `break` in a round of its own
    Broke(usize),
    /// a loop's list has more items than it takes: the run fails there
    TooMany(usize),
    /// a round of a parallel loop fails (a task's error nothing in it took, or a deliberate end), by
    /// the loop's site and the round's place
    RoundFailed(usize, u64),
    /// a check of a rule's precondition: whether the values keep it
    Check(usize, bool),
    /// an answer that came back fails its checks: it does not have the declared shape
    /// (`Dandori.BadResponse`), or a case's state the machine does not lead to there
    /// (`Dandori.UnexpectedState`)
    Bad(usize),
    OnFailure,
    OnCancel,
    /// the flow runs to its end, and so do `on failure` and `on cancel`
    FlowEnd,
    OnFailureEnd,
    OnCancelEnd,
}

/// Which task a call was, for a test that has to leave some answers out.
#[derive(Clone, Debug)]
pub struct CallInfo {
    pub task: Option<usize>,
    pub callback: bool,
    pub kind: Option<String>,
}

/// A parallel round's failure: a task's error nothing handled, or a deliberate end.
#[derive(Clone)]
enum Pending {
    Task(CallError),
    Fail { error: String, cause: Value },
}

/// An error a call ended with: the kind as the `.flow` names it, the name the target uses, and
/// its cause: the stand-in's for an error the other side gives, dandori's own for one the code it
/// writes raises after the call (Jev's answer less sure than the task asks).
#[derive(Clone)]
struct CallError {
    kind: String,
    target_name: String,
    cause: String,
}

/// The run on a clock that does not move, as the runners run the platforms: what the platforms'
/// runs are held to (`run_traced`, `run_visits` and `cases_at_end` too). `dandori run` runs a
/// scenario on a clock that moves (`run_moving`).
pub fn run(m: &Model, sc: &Value, view: View) -> Result<Value, String> {
    run_traced(m, sc, view).map(|(v, _)| v)
}

// The runs on a clock that does not move are what the platforms' runs are held to, and say why
// they stop in English; the runs on a clock that moves are the commands' (`dandori run`, `ritsu
// run`, the walk of `dandori scenarios`), and say it in both languages.

/// The run, and for every answer the calls took, which task took it and how it came out.
pub fn run_traced(m: &Model, sc: &Value, view: View) -> Result<(Value, Vec<CallInfo>), String> {
    run_all(m, sc, view, None, false).map(|r| (r.trace, r.callees)).map_err(|e| e.en)
}

/// The run on a clock that moves, as time goes on where the workflow runs: what `dandori run`
/// prints, and the answers of the scenario it did not take.
pub fn run_moving(m: &Model, sc: &Value, view: View) -> Result<(Value, Option<LeftOver>), ritsu_base::text::Text> {
    run_all(m, sc, view, None, true).map(|r| (r.trace, LeftOver::of(sc, r.used)))
}

/// A run on a clock that moves, with the calls a side answers answered by it (`run_answered`).
pub struct Answered {
    pub trace: Value,
    /// for every answer the calls took, which task took it and how it came out
    pub callees: Vec<CallInfo>,
    /// everything the run went through, in order
    pub visits: Vec<Visit>,
    /// the answers of the scenario it did not take
    pub unused: Option<LeftOver>,
}

/// The run on a clock that moves, with the calls `answers` takes answered by it and the others by
/// the scenario, in order.
pub fn run_answered(m: &Model, sc: &Value, view: View, answers: &mut dyn Answers) -> Result<Answered, ritsu_base::text::Text> {
    run_all(m, sc, view, Some(answers), true).map(|r| Answered { unused: LeftOver::of(sc, r.used), trace: r.trace, callees: r.callees, visits: r.visits })
}

/// The run on a clock that moves, with every call answered by the scenario, as `dandori run` runs it
/// without the ports, and everything it went through.
pub fn run_moving_visits(m: &Model, sc: &Value, view: View) -> Result<Answered, ritsu_base::text::Text> {
    run_all(m, sc, view, None, true).map(|r| Answered { unused: LeftOver::of(sc, r.used), trace: r.trace, callees: r.callees, visits: r.visits })
}

/// The answers of a scenario that a run did not take: the run came to its end with them still in
/// the list. `ritsu run` and `dandori run` say them, and end with 1 as for a scenario that runs out
/// of answers (DESIGN 36): an answer left over is a count gone wrong, or a scenario written for
/// another way through the flow, or for the flow before it changed.
#[derive(Clone, Debug, PartialEq)]
pub struct LeftOver {
    /// the place of the first in the scenario's `answers`, counted from 1
    pub from: usize,
    pub answers: Vec<Value>,
}

impl LeftOver {
    /// The answers of `sc` after the first `used`; None when the run took every one.
    pub fn of(sc: &Value, used: usize) -> Option<LeftOver> {
        let all = sc.get("answers")?.as_array()?;
        (all.len() > used).then(|| LeftOver { from: used + 1, answers: all[used..].to_vec() })
    }

    /// What `--format json` writes of them.
    pub fn json(&self) -> Value {
        json!({ "from": self.from, "count": self.answers.len(), "answers": self.answers })
    }

    /// What is said of them, a line each: which answers they are, the first three of them cut
    /// short, and why a run leaves answers; with `computes`, the run computed calls of rules, dates
    /// or books, which take no answer from the scenario.
    pub fn said(&self, computes: bool) -> Vec<ritsu_base::text::Text> {
        use ritsu_base::tr;
        let n = self.answers.len();
        let mut shown: Vec<String> = self.answers.iter().take(3).map(|a| short(&serde_json::to_string(a).unwrap_or_default(), 60)).collect();
        if n > 3 {
            shown.push("…".into());
        }
        let shown = shown.join(", ");
        let (from, to) = (self.from, self.from + n - 1);
        let mut lines = vec![if n == 1 {
            tr!("シナリオの {from} 番目の結果は使われませんでした: {shown}", "the run did not take answer {from} of the scenario: {shown}")
        } else {
            tr!("シナリオの {from} 番目から {to} 番目までの {n} 個の結果は使われませんでした: {shown}", "the run did not take answers {from} to {to} of the scenario ({n} of them): {shown}")
        }];
        lines.push(tr!(
            "実行は、その結果を受け取る呼び出しの前に終わりました。シナリオが呼び出しの数を数え違えているか、フローの別の分岐や、変わる前のフローに合わせて書かれているのかもしれません。",
            "The run came to its end before a call took it: the scenario may count the calls wrong, or be written for another way through the flow, or for the flow before it changed."
        ));
        if computes {
            lines.push(tr!(
                "実行が計算する規則・日付・帳簿の呼び出しは、シナリオの結果を受け取りません。",
                "A call of a rule, a date or a book that the run computes takes no answer from the scenario."
            ));
        }
        lines
    }
}

/// `s`, cut to `max` characters at the most, with `…` where it is cut.
fn short(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut t: String = s.chars().take(max - 1).collect();
    t.push('…');
    t
}

/// The run, and everything it went through, in order.
pub fn run_visits(m: &Model, sc: &Value, view: View) -> Result<(Value, Vec<Visit>), String> {
    run_all(m, sc, view, None, false).map(|r| (r.trace, r.visits)).map_err(|e| e.en)
}

/// Each case's state when the run ends, by the case's name: null for a case the run did not
/// start. What Temporal's query `dandori.status` says of the cases then.
pub fn cases_at_end(m: &Model, sc: &Value, view: View) -> Result<Map<String, Value>, String> {
    let vars = run_all(m, sc, view, None, false).map_err(|e| e.en)?.vars;
    Ok(m.cases
        .iter()
        .map(|c| {
            let st = vars.get(&c.name).and_then(|v| v.get(&c.state_field)).cloned().unwrap_or(Value::Null);
            (c.name.clone(), st)
        })
        .collect())
}

struct Ran {
    trace: Value,
    callees: Vec<CallInfo>,
    vars: BTreeMap<String, Value>,
    visits: Vec<Visit>,
    /// how many of the scenario's answers the run took
    used: usize,
}

fn run_all<'a>(m: &'a Model, sc: &Value, view: View, mut hook: Option<&'a mut dyn Answers>, moves: bool) -> Result<Ran, ritsu_base::text::Text> {
    let answers = sc["answers"].as_array().cloned().unwrap_or_default();
    let clock = match Clock::of(sc, moves) {
        Ok(c) => c,
        Err(why) => {
            if let Some(h) = hook.as_deref_mut() {
                h.cannot_run(&why);
            }
            return Err(why);
        }
    };
    let mut r = Run {
        m,
        hook,
        view,
        execution: sc["execution"].as_str().unwrap_or("test").to_string(),
        answers,
        next_answer: 0,
        vars: BTreeMap::new(),
        rounds: vec![],
        steps: vec![],
        end: None,
        in_on_failure: false,
        cancelled: false,
        in_on_cancel: false,
        error: None,
        par_depth: 0,
        pending: None,
        callees: vec![],
        visits: vec![],
        clock,
    };
    for (v, _) in &m.vars {
        r.vars.insert(v.clone(), Value::Null);
    }
    // what `now` reads: the scenario's moments, which the runners give the platforms' clocks too
    // (`Clock`); each statement that reads it reads the clock first (`Run::read_now`)
    let (start, later) = (r.clock.start().to_string(), r.clock.later().to_string());
    r.vars.insert(START_VAR.into(), json!(start));
    r.vars.insert(NOW_VAR.into(), json!(later));
    // a workflow that implements a service reads its input as protobuf reads the request: the zero
    // values its JSON leaves out are there
    let input = match &m.service {
        Some(s) => crate::apis::fill(&sc["input"], &s.input_zeros),
        None => sc["input"].clone(),
    };
    let mut ok = true;
    for (n, t) in &m.inputs {
        let v = input.get(n).cloned().unwrap_or(Value::Null);
        if !render::value_fits(m, &v, t, m.input_ranges.get(n).copied()) {
            ok = false;
        }
        r.vars.insert(n.clone(), v);
    }
    if !ok {
        r.fail("Dandori.BadInput", "the execution's input does not have the declared shape");
    } else {
        let flow = m.flow.clone();
        if let Ctl::Next = r.block(&flow) {
            if r.end.is_none() {
                r.visits.push(Visit::FlowEnd);
                r.end = Some(json!({ "succeed": no_output(m) }));
            }
        }
        if r.cancelled && r.error.is_none() {
            // `on cancel` runs to its end; the run ends as cancelled, unless it fails there
            r.cancelled = false;
            r.in_on_cancel = true;
            if let Some(block) = m.on_cancel.clone() {
                r.visits.push(Visit::OnCancel);
                let _ = r.block(&block);
                if r.end.is_none() {
                    r.visits.push(Visit::OnCancelEnd);
                }
            }
            if r.end.is_none() {
                r.end = Some(json!({ "cancel": Value::Null }));
            }
        }
    }
    if let Some(e) = r.error {
        return Err(e);
    }
    Ok(Ran { trace: json!({ "steps": r.steps, "end": r.end }), callees: r.callees, vars: r.vars, visits: r.visits, used: r.next_answer })
}

impl<'a> Run<'a> {
    /// A statement reads `now`: what the clock reads, which `now` in the statement reads too. None
    /// when the scenario gives it a moment the run cannot read, which stops the run.
    fn read_now(&mut self) -> Option<Value> {
        match self.clock.read() {
            Ok((at, moved)) => {
                if moved > 0.0 {
                    let after = self.clock.at_ms();
                    if let Some(h) = self.hook.as_deref_mut() {
                        h.pass(moved, Passing::Now, None, after);
                    }
                }
                let v = json!(at);
                self.vars.insert(NOW_VAR.into(), v.clone());
                Some(v)
            }
            Err(why) => {
                if let Some(h) = self.hook.as_deref_mut() {
                    h.cannot_run(&why);
                }
                self.error = Some(why);
                None
            }
        }
    }

    /// Time goes by: the clock moves on, when it moves, and the side the answers come from is told.
    fn pass(&mut self, seconds: f64, why: Passing, until: Option<&str>) {
        if let Some(after) = self.clock.go(seconds) {
            if let Some(h) = self.hook.as_deref_mut() {
                h.pass(seconds, why, until, after);
            }
        }
    }

    fn fail(&mut self, error: &str, cause: &str) {
        self.fail_with(error, json!(cause));
    }

    /// End the run as failed; inside a parallel round, end the round and keep the failure.
    fn fail_with(&mut self, error: &str, cause: Value) {
        if self.par_depth > 0 {
            self.pending = Some(Pending::Fail { error: error.to_string(), cause });
        } else {
            self.end = Some(json!({ "fail": { "error": error, "cause": cause } }));
        }
    }

    fn value(&self, e: &TExpr) -> Value {
        value_of(&self.vars, e)
    }

    /// Whether `es` read a variable where it has no value (`TExpr::must_have`), in the order the
    /// code reads them: the run fails then with Dandori.UnexpectedValue, at the statement's line.
    /// The check says it does not happen (E012, E013); it comes about only past it (DESIGN 38).
    fn unset(&mut self, es: &[&TExpr], line: usize) -> bool {
        let found = crate::model::must_haves(self.m, es).into_iter().find(|n| self.vars.get(*n).is_none_or(Value::is_null)).map(str::to_string);
        match found {
            Some(name) => {
                self.fail("Dandori.UnexpectedValue", &format!("line {line}: {name} has no value"));
                true
            }
            None => false,
        }
    }

    fn block(&mut self, ss: &[TStmt]) -> Ctl {
        for s in ss {
            match self.stmt(s) {
                Ctl::Next => {}
                other => return other,
            }
            if self.end.is_some() || self.error.is_some() || self.pending.is_some() || self.cancelled {
                return Ctl::Stop;
            }
        }
        Ctl::Next
    }

    fn stmt(&mut self, s: &TStmt) -> Ctl {
        if self.steps.len() > 20_000 {
            self.error = Some(ritsu_base::tr!("実行が 20000 ステップを超えました", "the run took more than 20000 steps"));
            return Ctl::Stop;
        }
        self.visits.push(Visit::Stmt(s.site));
        // a statement that reads `now` reads the clock once, before it runs (a check of the period
        // reads it itself, and a loop's yield at the end of each round)
        let reads = match &s.kind {
            TK::Period { .. } => false,
            TK::For { list, .. } => list.has_now(),
            k => k.reads_now(),
        };
        if reads && self.read_now().is_none() {
            return Ctl::Stop;
        }
        // a variable the statement reads that has no value, which the check says does not happen;
        // the run fails as the code of the targets fails (a `for`'s `yield` is read at each round's
        // end, below)
        let read = s.kind.reads_first(self.m);
        if self.unset(&read, s.line) {
            return Ctl::Stop;
        }
        match &s.kind {
            TK::Pass => Ctl::Next,
            TK::Break => Ctl::Break,
            TK::Wait { seconds } => {
                if self.view == View::Asl {
                    self.steps.push(json!({ "wait": seconds }));
                }
                self.pass(*seconds as f64, Passing::Wait, None);
                Ctl::Next
            }
            TK::WaitUntil { at } => {
                let v = self.value(at);
                if self.view == View::Asl {
                    self.steps.push(json!({ "wait_until": v }));
                }
                // a value that is not a time does not come here: the check holds `wait until` to timestamps
                let given = v.as_str().unwrap_or_default().to_string();
                if let Some(seconds) = self.clock.until(&given) {
                    self.pass(seconds, Passing::Wait, Some(&given));
                }
                Ctl::Next
            }
            TK::Succeed { fields } => {
                let out = if fields.is_empty() {
                    no_output(self.m)
                } else {
                    let mut o = Map::new();
                    for (f, e) in fields {
                        o.insert(f.clone(), self.value(e));
                    }
                    Value::Object(o)
                };
                self.end = Some(json!({ "succeed": out }));
                Ctl::Stop
            }
            TK::Fail { error, cause, .. } => {
                let c = cause.as_ref().map(|c| self.value(c)).unwrap_or(Value::Null);
                self.fail_with(error, c);
                Ctl::Stop
            }
            // the period the workflow runs in (DESIGN 20): at the start, the moment the run starts at, which
            // fails it as an input outside its range does; before a statement that reads `now`, the
            // moment it reads, which goes to `on failure` as a task's error nothing handles does
            TK::Period { start } => {
                let p = self.m.period.as_ref().expect("a check of the period is in a flow with one");
                let v = if *start {
                    self.vars.get(START_VAR).cloned().unwrap_or(Value::Null)
                } else {
                    match self.read_now() {
                        Some(v) => v,
                        None => return Ctl::Stop,
                    }
                };
                let t = v.as_str().unwrap_or_default().to_string();
                if !*start {
                    self.vars.insert(now_at(s.site), v);
                }
                if render::moment_in(&t, &p.range) {
                    Ctl::Next
                } else if *start {
                    let cause = p.cause(None, &t);
                    self.fail(OUT_OF_PERIOD, &cause);
                    Ctl::Stop
                } else {
                    let cause = p.cause(Some(s.line), &t);
                    self.visits.push(Visit::Unhandled(s.site));
                    self.raise(Pending::Task(CallError { kind: OUT_OF_PERIOD.into(), target_name: OUT_OF_PERIOD.into(), cause }))
                }
            }
            // a rule's precondition ritsu could not decide: the values the call will give it, as soon as they are made
            TK::Check(c) => {
                let held = crate::prechecks::holds(&c.test, &|e| self.value(e));
                self.visits.push(Visit::Check(s.site, held));
                if held {
                    Ctl::Next
                } else {
                    let cause = c.cause(self.m);
                    self.fail(crate::prechecks::ERROR, &cause);
                    Ctl::Stop
                }
            }
            TK::Assign { name, expr } => {
                let v = self.value(expr);
                self.vars.insert(name.clone(), v);
                Ctl::Next
            }
            TK::For { var, list, max, parallel, body, result, locals } => {
                let items = self.value(list).as_array().cloned().unwrap_or_default();
                if items.len() > *max as usize {
                    self.visits.push(Visit::TooMany(s.site));
                    self.fail("Dandori.TooManyItems", &format!("line {}: the list has more than {max} items", s.line));
                    return Ctl::Stop;
                }
                let mut out = Vec::new();
                match parallel {
                    None => {
                        let mut broke = false;
                        for (i, it) in items.iter().enumerate() {
                            self.rounds.push(i as u64);
                            self.visits.push(Visit::Round(s.site, i as u64));
                            self.vars.insert(var.clone(), it.clone());
                            let c = self.block(body);
                            if let (Ctl::Next, Some((_, y))) = (&c, result) {
                                if self.end.is_none() && self.error.is_none() && self.pending.is_none() {
                                    // a yield that reads `now` reads the clock at the end of each round
                                    if y.has_now() && self.read_now().is_none() {
                                        return Ctl::Stop;
                                    }
                                    if self.unset(&[y], s.line) {
                                        return Ctl::Stop;
                                    }
                                    out.push(self.value(y));
                                }
                            }
                            self.rounds.pop();
                            match c {
                                Ctl::Break => {
                                    self.visits.push(Visit::Broke(s.site));
                                    broke = true;
                                    break;
                                }
                                Ctl::Stop => return Ctl::Stop,
                                Ctl::Next => {}
                            }
                        }
                        if !broke {
                            self.visits.push(Visit::Done(s.site));
                        }
                    }
                    Some(_) => {
                        let mut first: Option<Pending> = None;
                        for (i, it) in items.iter().enumerate() {
                            for l in locals {
                                self.vars.insert(l.clone(), Value::Null);
                            }
                            self.rounds.push(i as u64);
                            self.visits.push(Visit::Round(s.site, i as u64));
                            self.vars.insert(var.clone(), it.clone());
                            self.par_depth += 1;
                            let _ = self.block(body);
                            self.par_depth -= 1;
                            self.rounds.pop();
                            // a cancellation stops every round, not only its own
                            if self.error.is_some() || self.cancelled {
                                return Ctl::Stop;
                            }
                            match self.pending.take() {
                                Some(p) => {
                                    self.visits.push(Visit::RoundFailed(s.site, i as u64));
                                    if first.is_none() {
                                        first = Some(p);
                                    }
                                }
                                None => {
                                    if let Some((_, y)) = result {
                                        if y.has_now() && self.read_now().is_none() {
                                            return Ctl::Stop;
                                        }
                                        // the yield is the round's own: a variable it reads that
                                        // has no value fails the round, as a failure in its body does
                                        self.par_depth += 1;
                                        let unset = self.unset(&[y], s.line);
                                        self.par_depth -= 1;
                                        match self.pending.take() {
                                            Some(p) if unset => {
                                                self.visits.push(Visit::RoundFailed(s.site, i as u64));
                                                if first.is_none() {
                                                    first = Some(p);
                                                }
                                            }
                                            _ => out.push(self.value(y)),
                                        }
                                    }
                                }
                            }
                        }
                        for l in locals {
                            self.vars.insert(l.clone(), Value::Null);
                        }
                        if let Some(p) = first {
                            return self.raise(p);
                        }
                        self.visits.push(Visit::Done(s.site));
                    }
                }
                if let Some((r, _)) = result {
                    self.vars.insert(r.clone(), Value::Array(out));
                }
                Ctl::Next
            }
            TK::Repeat { times, body } => {
                let mut n: u64 = 0;
                while n < *times as u64 {
                    self.rounds.push(n);
                    self.visits.push(Visit::Round(s.site, n));
                    let c = self.block(body);
                    self.rounds.pop();
                    match c {
                        Ctl::Break => {
                            self.visits.push(Visit::Broke(s.site));
                            return Ctl::Next;
                        }
                        Ctl::Stop => return Ctl::Stop,
                        Ctl::Next => {}
                    }
                    n += 1;
                }
                self.visits.push(Visit::Done(s.site));
                Ctl::Next
            }
            TK::Match { expr, arms } => {
                let v = self.value(expr);
                for (i, a) in arms.iter().enumerate() {
                    let hit = (a.none && v.is_null())
                        || (a.some.is_some() && !v.is_null())
                        || a.values.iter().any(|x| match &v {
                            Value::String(s) => s == x,
                            Value::Bool(b) => b.to_string() == *x,
                            _ => false,
                        });
                    if hit {
                        self.visits.push(Visit::Arm(s.site, i));
                        if let Some(n) = &a.some {
                            self.vars.insert(n.clone(), v.clone());
                        }
                        return self.block(&a.body);
                    }
                }
                let shown = expr.show();
                self.fail("Dandori.UnexpectedValue", &format!("line {}: {} took a value that no arm names", s.line, shown));
                Ctl::Stop
            }
            TK::Call { target, callee, args, handlers } => self.call(s, target.as_ref(), callee, args, handlers),
        }
    }

    fn take_answer(&mut self, callee_name: &str) -> Option<Value> {
        match self.answers.get(self.next_answer) {
            Some(a) => {
                self.next_answer += 1;
                Some(a.clone())
            }
            None => {
                let n = self.next_answer + 1;
                self.error = Some(ritsu_base::tr!("シナリオに、{n} 番目の呼び出し（{callee_name}）の結果がありません", "the scenario has no answer for call {n} ({callee_name})"));
                None
            }
        }
    }

    /// The answer to one try of a call: from the side the run takes answers from besides the
    /// scenario, when it takes the call; else the scenario's next.
    fn answer_for(&mut self, callee: &Callee, args: &Map<String, Value>, callee_name: &str) -> Option<Value> {
        let (m, view) = (self.m, self.view);
        if let Some(h) = self.hook.as_deref_mut() {
            match h.answer(m, view, callee, args) {
                Some(Ok(v)) => return Some(v),
                Some(Err(e)) => {
                    self.error = Some(e);
                    return None;
                }
                None => {}
            }
        }
        let v = self.take_answer(callee_name)?;
        if let Some(h) = self.hook.as_deref_mut() {
            h.scripted(m, callee, args, &v);
        }
        Some(v)
    }

    fn call(&mut self, s: &TStmt, target: Option<&Target>, callee: &Callee, args: &[(String, TExpr)], handlers: &[THandler]) -> Ctl {
        let m = self.m;
        let cname = match callee {
            Callee::Task(t) => m.tasks[*t].name.clone(),
            Callee::Rule(r) => m.rules[*r].name.clone(),
        };
        let mut a = Map::new();
        for (n, e) in args {
            a.insert(n.clone(), self.value(e));
        }
        let key = match callee {
            Callee::Task(t) if m.tasks[*t].key => Some(render::key(&self.execution, s.site, &self.rounds)),
            _ => None,
        };
        let wire = render::call(m, self.view, callee, &a, key.as_deref());
        let retry = self.retriers(callee);
        let mut counts = vec![0u32; retry.len()];
        let outcome: Result<Value, CallError> = loop {
            let ans = match self.answer_for(callee, &a, &cname) {
                Some(x) => x,
                None => return Ctl::Stop,
            };
            let (task, callback) = match callee {
                Callee::Task(t) => (Some(*t), m.tasks[*t].callback),
                Callee::Rule(_) => (None, false),
            };
            let kind = if ans.get("cancel").is_some() {
                "cancel"
            } else if ans.get("ok").is_some() {
                "ok"
            } else {
                ans["error"].as_str().unwrap_or("failure")
            };
            self.visits.push(Visit::Answer(s.site, kind.to_string()));
            if ans.get("cancel").is_some() {
                self.callees.push(CallInfo { task, callback, kind: Some("cancel".into()) });
                self.steps.push(json!({ "call": wire, "answer": { "cancel": true } }));
                self.cancelled = true;
                return Ctl::Stop;
            }
            self.callees.push(CallInfo { task, callback, kind: ans.get("error").and_then(|e| e.as_str()).map(String::from) });
            if let Some(v) = ans.get("ok") {
                self.steps.push(json!({ "call": wire, "answer": { "ok": v } }));
                // a decision task's answer is read into the task's type; a refused question, and then
                // an answer less sure than the task asks, fail the call with the task's error, which
                // no retry takes
                if let Some(j) = match callee {
                    Callee::Task(t) => m.tasks[*t].jev(),
                    Callee::Rule(_) => None,
                } {
                    let (read, low, refused) = render::jev_read(j, v);
                    if let (true, Some(error), Callee::Task(t)) = (refused, j.refusal_error(), callee) {
                        break Err(CallError { kind: error.to_string(), target_name: error.to_string(), cause: render::jev_refused_cause(&m.tasks[*t]) });
                    }
                    if let (true, Some((_, error)), Callee::Task(t)) = (low, &j.floor, callee) {
                        break Err(CallError { kind: error.clone(), target_name: error.clone(), cause: render::jev_low_cause(&m.tasks[*t]) });
                    }
                    break Ok(read);
                }
                // a book's operation done, now or before: the hold the code dandori writes makes of the
                // arguments, in the state the operation leaves it in
                if let Some(b) = match callee {
                    Callee::Task(t) => m.tasks[*t].book(),
                    Callee::Rule(_) => None,
                } {
                    break Ok(render::book_value(m, b, &a));
                }
                // a Connect answer is read as protobuf reads it: the zero values its JSON leaves out are
                // there; so is the value of an event or a callback that a method of the service sends
                break Ok(match callee {
                    Callee::Task(t) => m.tasks[*t].connect.as_ref().or(m.tasks[*t].answer_zeros.as_ref()).map(|z| crate::apis::fill(v, z)).unwrap_or_else(|| v.clone()),
                    // a rule called at its service answers as the service writes it: read back as the rule's record
                    Callee::Rule(r) => m.rules[*r].connect.as_ref().map(|c| render::rule_read(c, v)).unwrap_or_else(|| v.clone()),
                });
            }
            let kind = ans["error"].as_str().unwrap_or("failure").to_string();
            let mut err = self.error_of(callee, &kind);
            // an answer that says its cause (one rulec, koyomi or chobo gave) keeps it
            if let Some(c) = ans.get("cause").and_then(|c| c.as_str()) {
                err.cause = c.to_string();
            }
            // a try that timed out took its timeout: the task's, the wait until the time `timeout until`
            // names, or the limit of a call of a rule or of a date (DESIGN 16)
            if kind == "timeout" {
                match callee {
                    Callee::Task(t) => match m.tasks[*t].deadline.as_ref().and_then(|p| a.get(p)) {
                        Some(at) => {
                            // a value that is not a time does not come here: the check holds `timeout until` to timestamps
                            let given = at.as_str().unwrap_or_default().to_string();
                            if let Some(seconds) = self.clock.timeout_until(&given) {
                                self.pass(seconds, Passing::Timeout, Some(&given));
                            }
                        }
                        None => {
                            if let Some(secs) = m.tasks[*t].timeout {
                                self.pass(secs as f64, Passing::Timeout, None);
                            }
                        }
                    },
                    Callee::Rule(_) => self.pass(crate::check::RULE_SECONDS as f64, Passing::Timeout, None),
                }
            }
            self.steps.push(json!({ "call": wire, "answer": { "error": kind, "as": err.target_name } }));
            // the retriers, in order: the first whose errors match decides
            let mut again = false;
            for (i, (names, max, every, backoff)) in retry.iter().enumerate() {
                if matches_error(names, &err.target_name) {
                    if counts[i] < *max {
                        let delay = (*every as f64) * backoff.powi(counts[i] as i32);
                        counts[i] += 1;
                        if self.view == View::Asl {
                            let d = if delay.fract() == 0.0 { json!(delay as u64) } else { json!(delay) };
                            self.steps.push(json!({ "retry_wait": d }));
                        }
                        self.pass(delay, Passing::Retry, None);
                        again = true;
                    }
                    break;
                }
            }
            if !again {
                break Err(err);
            }
        };
        match outcome {
            Ok(v) => {
                // a Claude agent's enum values are taken without regard to case
                let v = match callee {
                    Callee::Task(t) => match (m.tasks[*t].via(self.view.platform()), &m.tasks[*t].result) {
                        (Some(Via::Agent { provider: Provider::Claude, .. }), Some(ty)) => render::fold_enums(m, &v, ty),
                        _ => v,
                    },
                    Callee::Rule(_) => v,
                };
                let var = match target {
                    Some(Target::Let(x)) => Some(x.clone()),
                    Some(Target::Case(c)) => Some(m.cases[*c].name.clone()),
                    None => None,
                };
                if let Some(var) = var {
                    let ty = match callee {
                        Callee::Task(t) => m.tasks[*t].result.clone().unwrap_or(Ty::Json),
                        Callee::Rule(r) => Ty::Record(m.rules[*r].outputs),
                    };
                    if !render::value_fits(m, &v, &ty, m.answer_range(callee)) {
                        self.visits.push(Visit::Bad(s.site));
                        self.fail("Dandori.BadResponse", &format!("line {}: the answer from {} does not have the declared shape", s.line, cname));
                        return Ctl::Stop;
                    }
                    if let (Some(Target::Case(c)), Some((_, allowed))) = (target, m.monitors.get(&s.site)) {
                        let st = v.get(&m.cases[*c].state_field).and_then(|x| x.as_str()).unwrap_or("");
                        if !allowed.iter().any(|x| x == st) {
                            self.visits.push(Visit::Bad(s.site));
                            self.fail(
                                "Dandori.UnexpectedState",
                                &format!("line {}: {} answered with a state the machine does not lead to here (expected one of {})", s.line, cname, allowed.join(", ")),
                            );
                            return Ctl::Stop;
                        }
                    }
                    // the answer is kept only when it passes its checks, as in the generated code
                    self.vars.insert(var.clone(), v.clone());
                }
                Ctl::Next
            }
            Err(err) => {
                for (j, h) in handlers.iter().enumerate() {
                    let hit = h.errors.iter().any(|e| match e {
                        HErr::Failure => true,
                        HErr::Timeout => err.kind == "timeout",
                        HErr::Declared(n) => {
                            // the target sees the error by its own name, so two declared errors with the same
                            // status cannot be told apart there; the interpreter matches the same way
                            render::asl_error(m, callee, &HErr::Declared(n.clone())).contains(&err.target_name) || *n == err.kind
                        }
                    });
                    if hit {
                        self.visits.push(Visit::Handler(s.site, j));
                        return self.block(&h.body);
                    }
                }
                self.visits.push(Visit::Unhandled(s.site));
                self.raise(Pending::Task(err))
            }
        }
    }

    /// A failure that nothing inside took: inside a parallel round it waits for the other
    /// rounds; outside, a task's error runs `on failure` and the run fails.
    fn raise(&mut self, p: Pending) -> Ctl {
        if self.par_depth > 0 {
            self.pending = Some(p);
            return Ctl::Stop;
        }
        let err = match p {
            Pending::Fail { error, cause } => {
                self.end = Some(json!({ "fail": { "error": error, "cause": cause } }));
                return Ctl::Stop;
            }
            Pending::Task(err) => err,
        };
        let m = self.m;
        // `on failure` does not run for an error in itself, nor in `on cancel`
        if !self.in_on_failure && !self.in_on_cancel {
            if let Some(block) = m.on_failure.clone() {
                self.in_on_failure = true;
                self.vars.insert(ERROR_VAR.into(), json!({ "Error": err.target_name, "Cause": err.cause }));
                self.visits.push(Visit::OnFailure);
                match self.block(&block) {
                    Ctl::Stop => return Ctl::Stop,
                    _ => {
                        if self.end.is_none() {
                            self.visits.push(Visit::OnFailureEnd);
                            let name = err.target_name.clone();
                            self.fail(&name, &err.cause);
                        }
                        return Ctl::Stop;
                    }
                }
            }
        }
        let name = err.target_name.clone();
        self.fail(&name, &err.cause);
        Ctl::Stop
    }

    /// The name the target gives an error of this kind from this callee.
    fn error_of(&self, callee: &Callee, kind: &str) -> CallError {
        let target_name = match self.view {
            View::Temporal | View::Durable | View::Argo | View::Graph => kind.to_string(),
            View::Asl => match kind {
                "timeout" => "States.Timeout".to_string(),
                "failure" => asl_failure(self.m, callee).to_string(),
                other => render::asl_error(self.m, callee, &HErr::Declared(other.to_string())).into_iter().next().unwrap_or_else(|| other.to_string()),
            },
        };
        // a book's refusal comes back with its reason, which the code dandori writes gives as the
        // cause; on Step Functions it is the book's Lambda function that fails, as the stand-in says
        let refused = matches!(callee, Callee::Task(t) if self.m.tasks[*t].book().is_some() && self.m.tasks[*t].error(kind).is_some());
        // on Argo, a child Workflow that fails leaves its parent nothing to read a cause from: the
        // step that makes it has no error file (DESIGN 7)
        let child_on_argo = self.view == View::Argo && matches!(callee, Callee::Task(t) if matches!(self.m.tasks[*t].via(Platform::Argo), Some(Via::ArgoTemplate(_))));
        let cause = if refused && self.view != View::Asl {
            kind.to_string()
        } else if child_on_argo {
            String::new()
        } else {
            SCRIPTED_CAUSE.into()
        };
        CallError { kind: kind.to_string(), target_name, cause }
    }

    /// (errors, max attempts, interval, backoff) for each retrier, as the targets have them.
    fn retriers(&self, callee: &Callee) -> Vec<(Vec<String>, u32, u64, f64)> {
        let v = match callee {
            Callee::Rule(_) => crate::asl::rule_retriers(),
            Callee::Task(t) => match &self.m.tasks[*t].retry {
                Some(r) => crate::asl::retriers(self.m, callee, &self.m.tasks[*t], r),
                None => json!([]),
            },
        };
        let mut out = Vec::new();
        for r in v.as_array().unwrap() {
            let mut names: Vec<String> = r["ErrorEquals"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect();
            if self.view != View::Asl {
                // the TypeScript names errors by kind
                names = names
                    .into_iter()
                    .map(|n| {
                        if n == "States.Timeout" {
                            "timeout".to_string()
                        } else if n == "States.ALL" {
                            n
                        } else {
                            match callee {
                                Callee::Task(t) => self.m.tasks[*t]
                                    .errors
                                    .iter()
                                    .find(|e| render::asl_error(self.m, callee, &HErr::Declared(e.name.clone())).contains(&n))
                                    .map(|e| e.name.clone())
                                    .unwrap_or(n),
                                Callee::Rule(_) => n,
                            }
                        }
                    })
                    .collect();
            }
            out.push((names, r["MaxAttempts"].as_u64().unwrap_or(3) as u32, r["IntervalSeconds"].as_u64().unwrap_or(1), r["BackoffRate"].as_f64().unwrap_or(2.0)));
        }
        out
    }
}

/// What a run that ends with no outputs ends with: null, but `{}` for a workflow that implements a
/// service, whose response protobuf's JSON reads from an object and not from null.
pub fn no_output(m: &Model) -> Value {
    if m.service.is_some() {
        json!({})
    } else {
        Value::Null
    }
}

pub fn matches_error(names: &[String], err: &str) -> bool {
    names.iter().any(|n| n == err || (n == "States.ALL" && err != "States.Runtime" && err != "States.DataLimitExceeded"))
}

/// Where the run keeps what `now` reads: a name no variable can have.
pub const NOW_VAR: &str = "(now)";

/// Where the run keeps what the check of the period at `site` read (`TExpr::NowAt`).
pub fn now_at(site: usize) -> String {
    format!("(now@{site})")
}

/// Where the run keeps the moment it starts at: what the check of the period at the start reads.
pub const START_VAR: &str = "(start)";

/// Where the run keeps the error `on failure` runs for: a name no variable can have.
pub const ERROR_VAR: &str = "(error)";

/// The value of an expression over the given variables. A field that is absent reads as null.
pub fn value_of(vars: &BTreeMap<String, Value>, e: &TExpr) -> Value {
    match e {
        TExpr::Now => vars.get(NOW_VAR).cloned().unwrap_or_else(|| json!(render::SCENARIO_NOW)),
        TExpr::NowAt(site) => vars.get(&now_at(*site)).cloned().unwrap_or(Value::Null),
        TExpr::Str(s) => json!(s),
        TExpr::Int(n) => json!(n),
        TExpr::Bool(b) => json!(b),
        TExpr::Enum(v, _) => json!(v),
        TExpr::None(_) => Value::Null,
        TExpr::Var { name, fields, .. } => {
            let mut v = vars.get(name).cloned().unwrap_or(Value::Null);
            for f in fields {
                v = v.get(f).cloned().unwrap_or(Value::Null);
            }
            v
        }
        TExpr::Record { fields, .. } => {
            let mut o = Map::new();
            for (f, x) in fields {
                o.insert(f.clone(), value_of(vars, x));
            }
            Value::Object(o)
        }
        TExpr::List { items, .. } => Value::Array(items.iter().map(|x| value_of(vars, x)).collect()),
        TExpr::Interp(parts) => {
            let mut out = String::new();
            for p in parts {
                match p {
                    IPart::Lit(s) => out.push_str(s),
                    IPart::Hole(x) => out.push_str(&render::value_text(&value_of(vars, x))),
                }
            }
            json!(out)
        }
    }
}
