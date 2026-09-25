//! The reference interpreter: the meaning of a `.flow`, run against a scenario — the
//! execution's input and the answers the calls get, in order. It reports every call as the
//! chosen target would send it (see `render`), with the answer it got, every wait, and how
//! the run ended. The runners of the generated ASL and TypeScript report the same, so a
//! difference between them and this is a defect of a generator.
//!
//! Errors and retries follow the ASL the generator writes: the retriers are the same
//! list (`asl::retriers`), matched the way Step Functions matches them.

use crate::model::*;
use crate::render::{self, View};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

pub const SCRIPTED_CAUSE: &str = "scripted";
pub const TEST_FAILURE: &str = "Dandori.Test.Failure";

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
    error: Option<String>,
}

/// An error a call ended with: the kind as the `.flow` names it, and the name the target uses.
#[derive(Clone)]
struct CallError {
    kind: String,
    target_name: String,
}

pub fn run(m: &Model, sc: &Value, view: View) -> Result<Value, String> {
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
        error: None,
    };
    for (v, _) in &m.vars {
        r.vars.insert(v.clone(), Value::Null);
    }
    let input = &sc["input"];
    let mut ok = true;
    for (n, t) in &m.inputs {
        let v = input.get(n).cloned().unwrap_or(Value::Null);
        if !render::value_fits(m, &v, t) {
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
    }
    if let Some(e) = r.error {
        return Err(e);
    }
    Ok(json!({ "steps": r.steps, "end": r.end }))
}

impl<'a> Run<'a> {
    fn fail(&mut self, error: &str, cause: &str) {
        self.end = Some(json!({ "fail": { "error": error, "cause": cause } }));
    }

    fn value(&self, e: &TExpr) -> Value {
        match e {
            TExpr::Str(s) => json!(s),
            TExpr::Int(n) => json!(n),
            TExpr::Bool(b) => json!(b),
            TExpr::Enum(v, _) => json!(v),
            TExpr::Var { name, fields, .. } => {
                let mut v = self.vars.get(name).cloned().unwrap_or(Value::Null);
                for f in fields {
                    v = v.get(f).cloned().unwrap_or(Value::Null);
                }
                v
            }
        }
    }

    fn block(&mut self, ss: &[TStmt]) -> Ctl {
        for s in ss {
            match self.stmt(s) {
                Ctl::Next => {}
                other => return other,
            }
            if self.end.is_some() || self.error.is_some() {
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
                self.end = Some(json!({ "fail": { "error": error, "cause": cause.clone().map(Value::String).unwrap_or(Value::Null) } }));
                Ctl::Stop
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
                    let hit = (a.none && v.is_null()) || a.values.iter().any(|x| match &v {
                        Value::String(s) => s == x,
                        Value::Bool(b) => b.to_string() == *x,
                        _ => false,
                    });
                    if hit {
                        return self.block(&a.body);
                    }
                }
                let shown = show(expr);
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
                let var = match target {
                    Some(Target::Let(x)) => Some(x.clone()),
                    Some(Target::Case(c)) => Some(m.cases[*c].name.clone()),
                    None => None,
                };
                if let Some(var) = var {
                    self.vars.insert(var.clone(), v.clone());
                    let ty = match callee {
                        Callee::Task(t) => m.tasks[*t].result.clone(),
                        Callee::Rule(r) => Ty::Record(m.rules[*r].outputs),
                    };
                    if !render::value_fits(m, &v, &ty) {
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
                if !self.in_on_failure {
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
        }
    }

    /// The name the target gives an error of this kind from this callee.
    fn error_of(&self, callee: &Callee, kind: &str) -> CallError {
        let target_name = match self.view {
            View::Temporal | View::Durable => kind.to_string(),
            View::Asl => match kind {
                "timeout" => "States.Timeout".to_string(),
                "failure" => TEST_FAILURE.to_string(),
                other => render::asl_error(self.m, callee, &HErr::Declared(other.to_string())).into_iter().next().unwrap_or_else(|| other.to_string()),
            },
        };
        CallError { kind: kind.to_string(), target_name }
    }

    /// (errors, max attempts, interval, backoff) for each retrier, as the targets have them.
    fn retriers(&self, callee: &Callee) -> Vec<(Vec<String>, u32, u64, f64)> {
        let v = match callee {
            Callee::Rule(_) => json!([
                { "ErrorEquals": ["States.Timeout"], "MaxAttempts": 0 },
                { "ErrorEquals": ["States.ALL"], "IntervalSeconds": crate::asl::RULE_RETRY_INTERVAL, "MaxAttempts": crate::check::RULE_RETRIES, "BackoffRate": crate::asl::RULE_RETRY_BACKOFF }
            ]),
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
                                    .find(|(e, _)| render::asl_error(self.m, callee, &HErr::Declared(e.clone())).contains(&n))
                                    .map(|(e, _)| e.clone())
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

fn show(e: &TExpr) -> String {
    match e {
        TExpr::Var { name, fields, .. } => std::iter::once(name.clone()).chain(fields.iter().cloned()).collect::<Vec<_>>().join("."),
        TExpr::Str(s) => s.clone(),
        TExpr::Int(n) => n.to_string(),
        TExpr::Bool(b) => b.to_string(),
        TExpr::Enum(v, _) => v.clone(),
    }
}
