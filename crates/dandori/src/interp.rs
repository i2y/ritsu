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

struct Run<'a> {
    m: &'a Model,
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

/// An error a call ended with: the kind as the `.flow` names it, and the name the target uses.
#[derive(Clone)]
struct CallError {
    kind: String,
    target_name: String,
}

pub fn run(m: &Model, sc: &Value, view: View) -> Result<Value, String> {
    run_traced(m, sc, view).map(|(v, _)| v)
}

/// The run, and for every answer the calls took, which task took it and how it came out.
pub fn run_traced(m: &Model, sc: &Value, view: View) -> Result<(Value, Vec<CallInfo>), String> {
    run_all(m, sc, view).map(|(v, calls, _)| (v, calls))
}

/// Each case's state when the run ends, by the case's name: null for a case the run did not
/// start. What Temporal's query `dandori.status` says of the cases then.
pub fn cases_at_end(m: &Model, sc: &Value, view: View) -> Result<Map<String, Value>, String> {
    let (_, _, vars) = run_all(m, sc, view)?;
    Ok(m.cases
        .iter()
        .map(|c| {
            let st = vars.get(&c.name).and_then(|v| v.get(&c.state_field)).cloned().unwrap_or(Value::Null);
            (c.name.clone(), st)
        })
        .collect())
}

fn run_all(m: &Model, sc: &Value, view: View) -> Result<(Value, Vec<CallInfo>, BTreeMap<String, Value>), String> {
    let answers = sc["answers"].as_array().cloned().unwrap_or_default();
    let mut r = Run {
        m,
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
    };
    for (v, _) in &m.vars {
        r.vars.insert(v.clone(), Value::Null);
    }
    let input = &sc["input"];
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
                r.end = Some(json!({ "succeed": Value::Null }));
            }
        }
        if r.cancelled && r.error.is_none() {
            // `on cancel` runs to its end; the run ends as cancelled, unless it fails there
            r.cancelled = false;
            r.in_on_cancel = true;
            if let Some(block) = m.on_cancel.clone() {
                let _ = r.block(&block);
            }
            if r.end.is_none() {
                r.end = Some(json!({ "cancel": Value::Null }));
            }
        }
    }
    if let Some(e) = r.error {
        return Err(e);
    }
    Ok((json!({ "steps": r.steps, "end": r.end }), r.callees, r.vars))
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
        match &s.kind {
            TK::Pass => Ctl::Next,
            TK::Break => Ctl::Break,
            TK::Wait { seconds } => {
                if self.view == View::Asl {
                    self.steps.push(json!({ "wait": seconds }));
                }
                Ctl::Next
            }
            TK::WaitUntil { at } => {
                if self.view == View::Asl {
                    let v = self.value(at);
                    self.steps.push(json!({ "wait_until": v }));
                }
                Ctl::Next
            }
            TK::Succeed { fields } => {
                let out = if fields.is_empty() {
                    Value::Null
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
                        for (i, it) in items.iter().enumerate() {
                            self.rounds.push(i as u64);
                            self.vars.insert(var.clone(), it.clone());
                            let c = self.block(body);
                            if let (Ctl::Next, Some((_, y))) = (&c, result) {
                                if self.end.is_none() && self.error.is_none() && self.pending.is_none() {
                                    out.push(self.value(y));
                                }
                            }
                            self.rounds.pop();
                            match c {
                                Ctl::Break => break,
                                Ctl::Stop => return Ctl::Stop,
                                Ctl::Next => {}
                            }
                        }
                    }
                    Some(_) => {
                        let mut first: Option<Pending> = None;
                        for (i, it) in items.iter().enumerate() {
                            for l in locals {
                                self.vars.insert(l.clone(), Value::Null);
                            }
                            self.rounds.push(i as u64);
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
                    let c = self.block(body);
                    self.rounds.pop();
                    match c {
                        Ctl::Break => break,
                        Ctl::Stop => return Ctl::Stop,
                        Ctl::Next => {}
                    }
                    n += 1;
                }
                Ctl::Next
            }
            TK::Match { expr, arms } => {
                let v = self.value(expr);
                for a in arms {
                    let hit = (a.none && v.is_null())
                        || (a.some.is_some() && !v.is_null())
                        || a.values.iter().any(|x| match &v {
                            Value::String(s) => s == x,
                            Value::Bool(b) => b.to_string() == *x,
                            _ => false,
                        });
                    if hit {
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
            let ans = match self.take_answer(&cname) {
                Some(x) => x,
                None => return Ctl::Stop,
            };
            let (task, callback) = match callee {
                Callee::Task(t) => (Some(*t), m.tasks[*t].callback),
                Callee::Rule(_) => (None, false),
            };
            if ans.get("cancel").is_some() {
                self.callees.push(CallInfo { task, callback, kind: Some("cancel".into()) });
                self.steps.push(json!({ "call": wire, "answer": { "cancel": true } }));
                self.cancelled = true;
                return Ctl::Stop;
            }
            self.callees.push(CallInfo { task, callback, kind: ans.get("error").and_then(|e| e.as_str()).map(String::from) });
            if let Some(v) = ans.get("ok") {
                self.steps.push(json!({ "call": wire, "answer": { "ok": v } }));
                break Ok(v.clone());
            }
            let kind = ans["error"].as_str().unwrap_or("failure").to_string();
            let err = self.error_of(callee, &kind);
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
                for h in handlers {
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
                        return self.block(&h.body);
                    }
                }
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
                self.vars.insert("dd_error".into(), json!({ "Error": err.target_name, "Cause": SCRIPTED_CAUSE }));
                match self.block(&block) {
                    Ctl::Stop => return Ctl::Stop,
                    _ => {
                        if self.end.is_none() {
                            let name = err.target_name.clone();
                            self.fail(&name, SCRIPTED_CAUSE);
                        }
                        return Ctl::Stop;
                    }
                }
            }
        }
        let name = err.target_name.clone();
        self.fail(&name, SCRIPTED_CAUSE);
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
        CallError { kind: kind.to_string(), target_name }
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

pub fn matches_error(names: &[String], err: &str) -> bool {
    names.iter().any(|n| n == err || (n == "States.ALL" && err != "States.Runtime" && err != "States.DataLimitExceeded"))
}

/// The value of an expression over the given variables. A field that is absent reads as null.
pub fn value_of(vars: &BTreeMap<String, Value>, e: &TExpr) -> Value {
    match e {
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
