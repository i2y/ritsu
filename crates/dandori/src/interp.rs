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
    /// answers. Err stops the run: the call cannot be answered at all.
    fn answer(&mut self, m: &Model, view: View, callee: &Callee, args: &Map<String, Value>) -> Option<Result<Value, String>>;

    /// The scenario answered a try of a call this side does not take.
    fn scripted(&mut self, _m: &Model, _callee: &Callee, _args: &Map<String, Value>, _answer: &Value) {}

    /// Time goes by: so many seconds of a `wait`, of the wait before a retry, or of a call that timed
    /// out (its timeout).
    fn pass(&mut self, _seconds: f64, _why: Passing) {}

    /// The run waits until this moment, as `wait until` is given it.
    fn wait_until(&mut self, _at: &Value) {}
}

/// Why time goes by in a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Passing {
    Wait,
    Retry,
    Timeout,
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
    error: Option<String>,
    /// how many `for … in parallel` rounds are running around the current statement
    par_depth: usize,
    /// how the current parallel round failed, kept until every round is done
    pending: Option<Pending>,
    /// for every answer taken: the task that took it, and whether it waits for a callback
    callees: Vec<CallInfo>,
    /// what the run went through, in order
    visits: Vec<Visit>,
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

pub fn run(m: &Model, sc: &Value, view: View) -> Result<Value, String> {
    run_traced(m, sc, view).map(|(v, _)| v)
}

/// The run, and for every answer the calls took, which task took it and how it came out.
pub fn run_traced(m: &Model, sc: &Value, view: View) -> Result<(Value, Vec<CallInfo>), String> {
    run_all(m, sc, view, None).map(|r| (r.trace, r.callees))
}

/// The run, with the calls `answers` takes answered by it and the others by the scenario, in order;
/// and for every answer the calls took, which task took it and how it came out.
pub fn run_answered(m: &Model, sc: &Value, view: View, answers: &mut dyn Answers) -> Result<(Value, Vec<CallInfo>), String> {
    run_all(m, sc, view, Some(answers)).map(|r| (r.trace, r.callees))
}

/// The run, and everything it went through, in order.
pub fn run_visits(m: &Model, sc: &Value, view: View) -> Result<(Value, Vec<Visit>), String> {
    run_all(m, sc, view, None).map(|r| (r.trace, r.visits))
}

/// Each case's state when the run ends, by the case's name: null for a case the run did not
/// start. What Temporal's query `dandori.status` says of the cases then.
pub fn cases_at_end(m: &Model, sc: &Value, view: View) -> Result<Map<String, Value>, String> {
    let vars = run_all(m, sc, view, None)?.vars;
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
}

fn run_all<'a>(m: &'a Model, sc: &Value, view: View, hook: Option<&'a mut dyn Answers>) -> Result<Ran, String> {
    let answers = sc["answers"].as_array().cloned().unwrap_or_default();
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
    };
    for (v, _) in &m.vars {
        r.vars.insert(v.clone(), Value::Null);
    }
    // what `now` reads: the scenario's moment, which the runners give the platforms' clocks too
    r.vars.insert(NOW_VAR.into(), json!(sc["now"].as_str().unwrap_or(render::SCENARIO_NOW)));
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
    Ok(Ran { trace: json!({ "steps": r.steps, "end": r.end }), callees: r.callees, vars: r.vars, visits: r.visits })
}

impl<'a> Run<'a> {
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
            self.error = Some("the run took more than 20000 steps".into());
            return Ctl::Stop;
        }
        self.visits.push(Visit::Stmt(s.site));
        match &s.kind {
            TK::Pass => Ctl::Next,
            TK::Break => Ctl::Break,
            TK::Wait { seconds } => {
                if self.view == View::Asl {
                    self.steps.push(json!({ "wait": seconds }));
                }
                if let Some(h) = self.hook.as_deref_mut() {
                    h.pass(*seconds as f64, Passing::Wait);
                }
                Ctl::Next
            }
            TK::WaitUntil { at } => {
                let v = self.value(at);
                if self.view == View::Asl {
                    self.steps.push(json!({ "wait_until": v }));
                }
                if let Some(h) = self.hook.as_deref_mut() {
                    h.wait_until(&v);
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
            // a rule's precondition ritsu could not decide: the values the call will give it, as soon as they are made
            TK::Check(c) => {
                if crate::prechecks::holds(&c.test, &|e| self.value(e)) {
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
                                    out.push(self.value(y));
                                }
                            }
                            self.rounds.pop();
                            match c {
                                Ctl::Break => {
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
                                    if first.is_none() {
                                        first = Some(p);
                                    }
                                }
                                None => {
                                    if let Some((_, y)) = result {
                                        out.push(self.value(y));
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
                        Ctl::Break => return Ctl::Next,
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
                self.error = Some(format!("the scenario has no answer for call {} ({callee_name})", self.next_answer + 1));
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
            // a try that timed out took its timeout
            if let (true, Callee::Task(t), Some(h)) = (kind == "timeout", callee, self.hook.as_deref_mut()) {
                if let Some(secs) = m.tasks[*t].timeout {
                    h.pass(secs as f64, Passing::Timeout);
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
                        if let Some(h) = self.hook.as_deref_mut() {
                            h.pass(delay, Passing::Retry);
                        }
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
                        self.fail("Dandori.BadResponse", &format!("line {}: the answer from {} does not have the declared shape", s.line, cname));
                        return Ctl::Stop;
                    }
                    if let (Some(Target::Case(c)), Some((_, allowed))) = (target, m.monitors.get(&s.site)) {
                        let st = v.get(&m.cases[*c].state_field).and_then(|x| x.as_str()).unwrap_or("");
                        if !allowed.iter().any(|x| x == st) {
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
                self.vars.insert("dd_error".into(), json!({ "Error": err.target_name, "Cause": err.cause }));
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
        let cause = if refused && self.view != View::Asl { kind.to_string() } else { SCRIPTED_CAUSE.into() };
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

/// The value of an expression over the given variables. A field that is absent reads as null.
pub fn value_of(vars: &BTreeMap<String, Value>, e: &TExpr) -> Value {
    match e {
        TExpr::Now => vars.get(NOW_VAR).cloned().unwrap_or_else(|| json!(render::SCENARIO_NOW)),
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
