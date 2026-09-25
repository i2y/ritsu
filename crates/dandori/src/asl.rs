//! AWS Step Functions: a state machine in ASL with JSONata and variables, and for every
//! rule it calls, the Lambda function's handler around the Python rulec generates.
//!
//! Every variable of the workflow is a variable of the state machine, set to null at the
//! start. Every call is a Task followed by a Choice that checks the answer against its
//! declared type, and for a case, against the states the checker said it can be in; an
//! answer that fails the check ends the execution with `Dandori.BadResponse` or
//! `Dandori.UnexpectedState`, instead of being taken down a branch it does not belong to.

use crate::diag::Diag;
use crate::model::*;
use crate::render::{self, asl_var, jsonata_check, jsonata_expr, jsonata_list, jsonata_path, jsonata_string};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

pub const RULE_RETRY_INTERVAL: u64 = 1;
pub const RULE_RETRY_BACKOFF: f64 = 2.0;

struct Gen<'a> {
    m: &'a Model,
    states: Map<String, Value>,
    used: BTreeSet<String>,
    /// (site of the loop, the state after the loop)
    loops: Vec<(usize, String)>,
    failure_entry: Option<String>,
    in_on_failure: bool,
}

pub fn build(m: &Model) -> Result<Vec<(String, String)>, Vec<Diag>> {
    let mut errs = Vec::new();
    for t in &m.tasks {
        match &t.binding {
            None => errs.push(Diag::error("E050", t.line, 1, format!("`{}` needs `lambda` or `http` to run on Step Functions", t.name), format!("`{}` を Step Functions で動かすには `lambda` か `http` が要ります", t.name))),
            Some(Binding::Http { .. }) if t.connection.is_none() => errs.push(Diag::error("E050", t.line, 1, format!("`{}` needs `connection \"<EventBridge connection ARN>\"`", t.name), format!("`{}` には `connection \"<EventBridge の接続の ARN>\"` が要ります", t.name))),
            _ => {}
        }
    }
    let called: BTreeSet<usize> = m.all_stmts().iter().filter_map(|s| match &s.kind { TK::Call { callee: Callee::Rule(r), .. } => Some(*r), _ => None }).collect();
    for r in &called {
        if m.rules[*r].lambda.is_none() {
            let ru = &m.rules[*r];
            errs.push(Diag::error("E050", ru.line, 1, format!("the rule `{}` is called, so it needs `lambda \"<function>\"` under `use rule`", ru.name), format!("規則 `{}` は呼ばれているので、`use rule` の下に `lambda \"<関数>\"` が要ります", ru.name)));
        }
    }
    if !errs.is_empty() {
        return Err(errs);
    }

    let mut g = Gen { m, states: Map::new(), used: BTreeSet::new(), loops: vec![], failure_entry: None, in_on_failure: false };
    // the end states first, so that everything can point at them
    let done = g.name("done");
    g.states.insert(done.clone(), json!({ "Type": "Succeed" }));
    if let Some(block) = &m.on_failure {
        let rethrow = g.name("on failure end");
        g.states.insert(
            rethrow.clone(),
            json!({ "Type": "Fail", "Comment": "fail with the error that sent the run to on failure", "Error": "{% $dd_error.Error %}", "Cause": "{% $dd_error.Cause %}" }),
        );
        g.in_on_failure = true;
        let entry = g.block(block, &rethrow);
        g.in_on_failure = false;
        g.failure_entry = Some(entry);
    }
    let first = g.block(&m.flow, &done);

    // Start: every variable, the inputs from the execution's input
    let mut assign = Map::new();
    for (v, _) in &m.vars {
        if m.inputs.iter().any(|(i, _)| i == v) {
            assign.insert(asl_var(v), json!(format!("{{% $states.input.{} %}}", render::jsonata_field(v))));
        } else {
            assign.insert(asl_var(v), Value::Null);
        }
    }
    if m.on_failure.is_some() {
        assign.insert("dd_error".into(), Value::Null);
    }
    let start = g.name("start");
    let check_in = g.name("check input");
    let bad_in = g.name("bad input");
    g.states.insert(start.clone(), json!({ "Type": "Pass", "Comment": "set every variable; the inputs come from the execution's input", "Assign": assign, "Next": check_in }));
    let conds: Vec<String> = m.inputs.iter().map(|(n, t)| jsonata_check(m, &format!("$states.input.{}", render::jsonata_field(n)), t, 0)).collect();
    if conds.is_empty() {
        g.states.insert(check_in.clone(), json!({ "Type": "Pass", "Next": first }));
    } else {
        g.states.insert(
            check_in.clone(),
            json!({ "Type": "Choice", "Choices": [ { "Condition": format!("{{% {} %}}", conds.join(" and ")), "Next": first } ], "Default": bad_in }),
        );
        g.states.insert(bad_in, json!({ "Type": "Fail", "Error": "Dandori.BadInput", "Cause": "the execution's input does not have the declared shape" }));
    }

    let ordered = order(&g.states, &start);
    let mut top = Map::new();
    let comment = if m.description.is_empty() { format!("{} v{} (dandori)", m.name, m.version) } else { format!("{} v{}: {}", m.name, m.version, m.description) };
    top.insert("Comment".into(), json!(comment));
    top.insert("QueryLanguage".into(), json!("JSONata"));
    top.insert("StartAt".into(), json!(start));
    top.insert("States".into(), Value::Object(ordered));
    let mut files = vec![(format!("{}.asl.json", render::ident(&m.name)), serde_json::to_string_pretty(&Value::Object(top)).unwrap() + "\n")];
    for r in called {
        files.push(lambda_handler(m, r));
    }
    Ok(files)
}

/// The states reachable from the start, in the order a reader follows them.
fn order(states: &Map<String, Value>, start: &str) -> Map<String, Value> {
    let mut out = Map::new();
    let mut stack = vec![start.to_string()];
    while let Some(n) = stack.pop() {
        if out.contains_key(&n) {
            continue;
        }
        let st = match states.get(&n) {
            Some(s) => s.clone(),
            None => continue,
        };
        let mut next: Vec<String> = Vec::new();
        if let Some(x) = st["Next"].as_str() {
            next.push(x.to_string());
        }
        if let Some(cs) = st["Choices"].as_array() {
            for c in cs {
                if let Some(x) = c["Next"].as_str() {
                    next.push(x.to_string());
                }
            }
        }
        if let Some(x) = st["Default"].as_str() {
            next.push(x.to_string());
        }
        if let Some(cs) = st["Catch"].as_array() {
            for c in cs {
                if let Some(x) = c["Next"].as_str() {
                    next.push(x.to_string());
                }
            }
        }
        out.insert(n, st);
        for x in next.into_iter().rev() {
            stack.push(x);
        }
    }
    out
}

fn literal(e: &TExpr) -> Option<Value> {
    match e {
        TExpr::Str(s) => Some(json!(s)),
        TExpr::Int(n) => Some(json!(n)),
        TExpr::Bool(b) => Some(json!(b)),
        TExpr::Enum(v, _) => Some(json!(v)),
        TExpr::Var { .. } => None,
    }
}

fn arg_value(e: &TExpr) -> Value {
    literal(e).unwrap_or_else(|| json!(format!("{{% {} %}}", jsonata_expr(e))))
}

impl<'a> Gen<'a> {
    /// A state name that is unique, at most 80 characters, and plain.
    fn name(&mut self, base: &str) -> String {
        let clean: String = base.chars().map(|c| if c.is_alphanumeric() || " ._-()".contains(c) { c } else { ' ' }).collect();
        let clean = clean.split_whitespace().collect::<Vec<_>>().join(" ");
        let mut stem: String = clean.chars().take(72).collect();
        if stem.is_empty() {
            stem = "state".into();
        }
        let mut n = stem.clone();
        let mut k = 2;
        while self.used.contains(&n) {
            n = format!("{stem} {k}");
            k += 1;
        }
        self.used.insert(n.clone());
        n
    }

    fn block(&mut self, ss: &[TStmt], cont: &str) -> String {
        let mut end = ss.len();
        for (i, s) in ss.iter().enumerate() {
            if matches!(s.kind, TK::Succeed { .. } | TK::Fail { .. } | TK::Break) {
                end = i + 1;
                break;
            }
        }
        let mut next = cont.to_string();
        for s in ss[..end].iter().rev() {
            next = self.stmt(s, &next);
        }
        next
    }

    fn key_expr(&self, site: usize) -> String {
        let mut e = format!("$states.context.Execution.Name & \"/{site}\"");
        for (lsite, _) in &self.loops {
            e.push_str(&format!(" & \"/\" & $string($dd_loop_{lsite})"));
        }
        format!("{{% {e} %}}")
    }

    fn stmt(&mut self, s: &TStmt, cont: &str) -> String {
        match &s.kind {
            TK::Pass => cont.to_string(),
            TK::Break => self.loops.last().map(|(_, exit)| exit.clone()).unwrap_or_else(|| cont.to_string()),
            TK::Wait { seconds } => {
                let n = self.name(&format!("{} wait {}", s.line, crate::flow::show_dur(*seconds)));
                self.states.insert(n.clone(), json!({ "Type": "Wait", "Comment": format!("line {}", s.line), "Seconds": seconds, "Next": cont }));
                n
            }
            TK::WaitUntil { at } => {
                let n = self.name(&format!("{} wait until {}", s.line, show_expr(at)));
                self.states.insert(n.clone(), json!({ "Type": "Wait", "Comment": format!("line {}", s.line), "Timestamp": format!("{{% {} %}}", jsonata_expr(at)), "Next": cont }));
                n
            }
            TK::Succeed { fields } => {
                let n = self.name(&format!("{} succeed", s.line));
                let mut st = Map::new();
                st.insert("Type".into(), json!("Succeed"));
                st.insert("Comment".into(), json!(format!("line {}", s.line)));
                if !fields.is_empty() {
                    let mut out = Map::new();
                    for (f, e) in fields {
                        out.insert(f.clone(), arg_value(e));
                    }
                    st.insert("Output".into(), Value::Object(out));
                }
                self.states.insert(n.clone(), Value::Object(st));
                n
            }
            TK::Fail { error, cause, .. } => {
                let n = self.name(&format!("{} fail {error}", s.line));
                let mut st = Map::new();
                st.insert("Type".into(), json!("Fail"));
                st.insert("Comment".into(), json!(format!("line {}", s.line)));
                st.insert("Error".into(), json!(error));
                if let Some(c) = cause {
                    st.insert("Cause".into(), json!(c));
                }
                self.states.insert(n.clone(), Value::Object(st));
                n
            }
            TK::Repeat { times, body } => {
                let counter = format!("dd_loop_{}", s.site);
                let init = self.name(&format!("{} repeat", s.line));
                let head = self.name(&format!("{} repeat check", s.line));
                let next = self.name(&format!("{} repeat next", s.line));
                self.loops.push((s.site, cont.to_string()));
                let entry = self.block(body, &next);
                self.loops.pop();
                self.states.insert(init.clone(), json!({ "Type": "Pass", "Comment": format!("line {}: at most {times} rounds", s.line), "Assign": { counter.clone(): 0 }, "Next": head }));
                self.states.insert(
                    head.clone(),
                    json!({ "Type": "Choice", "Choices": [ { "Condition": format!("{{% ${counter} < {times} %}}"), "Next": entry } ], "Default": cont }),
                );
                self.states.insert(next.clone(), json!({ "Type": "Pass", "Assign": { counter.clone(): format!("{{% ${counter} + 1 %}}") }, "Next": head }));
                init
            }
            TK::Match { expr, arms } => {
                let n = self.name(&format!("{} match {}", s.line, show_expr(expr)));
                let bad = self.name(&format!("{} no arm", s.line));
                let x = jsonata_expr(expr);
                let mut choices = Vec::new();
                for a in arms {
                    let entry = self.block(&a.body, cont);
                    let mut parts = Vec::new();
                    if a.none {
                        if let TExpr::Var { name, .. } = expr {
                            parts.push(format!("${} = null", asl_var(name)));
                        }
                    }
                    if !a.values.is_empty() {
                        if expr.ty() == Ty::Bool {
                            for v in &a.values {
                                parts.push(format!("{x} = {v}"));
                            }
                        } else {
                            parts.push(format!("{x} in {}", jsonata_list(&a.values)));
                        }
                    }
                    let cond = parts.join(" or ");
                    choices.push(json!({ "Condition": format!("{{% {cond} %}}"), "Next": entry }));
                }
                self.states.insert(n.clone(), json!({ "Type": "Choice", "Comment": format!("line {}", s.line), "Choices": choices, "Default": bad }));
                self.states.insert(bad, json!({ "Type": "Fail", "Error": "Dandori.UnexpectedValue", "Cause": format!("line {}: {} took a value that no arm names", s.line, show_expr(expr)) }));
                n
            }
            TK::Call { target, callee, args, handlers } => self.call(s, target.as_ref(), callee, args, handlers, cont),
        }
    }

    fn call(&mut self, s: &TStmt, target: Option<&Target>, callee: &Callee, args: &[(String, TExpr)], handlers: &[THandler], cont: &str) -> String {
        let m = self.m;
        let cname = match callee {
            Callee::Task(t) => m.tasks[*t].name.clone(),
            Callee::Rule(r) => m.rules[*r].name.clone(),
        };
        let var = match target {
            Some(Target::Case(c)) => m.cases[*c].name.clone(),
            Some(Target::Let(v)) => v.clone(),
            None => String::new(),
        };
        let task_name = self.name(&format!("{} {} {}", s.line, cname, var));
        let check = self.name(&format!("{} check {}", s.line, cname));
        let bad = self.name(&format!("{} bad answer from {}", s.line, cname));

        // the request
        let mut payload = Map::new();
        for (a, e) in args {
            payload.insert(a.clone(), arg_value(e));
        }
        let (resource, arguments, result, timeout, retry) = match callee {
            Callee::Rule(r) => {
                let f = m.rules[*r].lambda.clone().unwrap_or_default();
                let retry = json!([
                    { "ErrorEquals": ["States.Timeout"], "MaxAttempts": 0 },
                    { "ErrorEquals": ["States.ALL"], "IntervalSeconds": RULE_RETRY_INTERVAL, "MaxAttempts": crate::check::RULE_RETRIES, "BackoffRate": RULE_RETRY_BACKOFF }
                ]);
                ("arn:aws:states:::lambda:invoke".to_string(), json!({ "FunctionName": f, "Payload": payload }), "$states.result.Payload".to_string(), None, Some(retry))
            }
            Callee::Task(t) => {
                let task = &m.tasks[*t];
                let retry = task.retry.as_ref().map(|r| retriers(m, callee, task, r));
                match task.binding.as_ref().unwrap() {
                    Binding::Lambda(f) => {
                        if task.key {
                            payload.insert("idempotency_key".into(), json!(self.key_expr(s.site)));
                        }
                        if task.callback {
                            payload.insert("task_token".into(), json!("{% $states.context.Task.Token %}"));
                            ("arn:aws:states:::lambda:invoke.waitForTaskToken".to_string(), json!({ "FunctionName": f, "Payload": payload }), "$states.result".to_string(), task.timeout, retry)
                        } else {
                            ("arn:aws:states:::lambda:invoke".to_string(), json!({ "FunctionName": f, "Payload": payload }), "$states.result.Payload".to_string(), task.timeout, retry)
                        }
                    }
                    Binding::Http { method, url, form } => {
                        let used = crate::lower::placeholders(url);
                        let mut rest = Map::new();
                        for (a, e) in args {
                            if !used.contains(a) {
                                rest.insert(a.clone(), arg_value(e));
                            }
                        }
                        let mut w = Map::new();
                        w.insert("ApiEndpoint".into(), url_value(url, args));
                        w.insert("Method".into(), json!(method));
                        w.insert("InvocationConfig".into(), json!({ "ConnectionArn": task.connection.clone().unwrap_or_default() }));
                        let mut headers = Map::new();
                        if *form {
                            headers.insert("Content-Type".into(), json!("application/x-www-form-urlencoded"));
                        }
                        if task.key {
                            headers.insert("Idempotency-Key".into(), json!(self.key_expr(s.site)));
                        }
                        if !headers.is_empty() {
                            w.insert("Headers".into(), Value::Object(headers));
                        }
                        if !rest.is_empty() {
                            let place = if method == "GET" || method == "DELETE" { "QueryParameters" } else { "RequestBody" };
                            w.insert(place.into(), Value::Object(rest));
                        }
                        if *form {
                            w.insert("Transform".into(), json!({ "RequestBodyEncoding": "URL_ENCODED" }));
                        }
                        ("arn:aws:states:::http:invoke".to_string(), Value::Object(w), "$states.result.ResponseBody".to_string(), task.timeout, retry)
                    }
                }
            }
        };

        let mut st = Map::new();
        st.insert("Type".into(), json!("Task"));
        st.insert("Comment".into(), json!(format!("line {}: {}", s.line, cname)));
        st.insert("Resource".into(), json!(resource));
        st.insert("Arguments".into(), arguments);
        if !var.is_empty() {
            st.insert("Assign".into(), json!({ asl_var(&var): format!("{{% {result} %}}") }));
        }
        if let Some(t) = timeout {
            st.insert("TimeoutSeconds".into(), json!(t));
        }
        if let Some(r) = retry {
            st.insert("Retry".into(), r);
        }
        let mut catches = Vec::new();
        let mut catches_all = false;
        for h in handlers {
            let entry = self.block(&h.body, cont);
            let mut errs: Vec<String> = Vec::new();
            if h.errors.contains(&HErr::Failure) {
                errs.push("States.ALL".into());
                catches_all = true;
            } else {
                for e in &h.errors {
                    for x in render::asl_error(m, callee, e) {
                        if !errs.contains(&x) {
                            errs.push(x);
                        }
                    }
                }
            }
            catches.push(json!({ "ErrorEquals": errs, "Next": entry }));
        }
        if !catches_all && !self.in_on_failure {
            if let Some(f) = &self.failure_entry {
                catches.push(json!({ "ErrorEquals": ["States.ALL"], "Assign": { "dd_error": "{% $states.errorOutput %}" }, "Next": f }));
            }
        }
        if !catches.is_empty() {
            st.insert("Catch".into(), Value::Array(catches));
        }

        // the answer: its declared type, and for a case the states it may carry
        if var.is_empty() {
            st.insert("Next".into(), json!(cont));
            self.states.insert(task_name.clone(), Value::Object(st));
            return task_name;
        }
        st.insert("Next".into(), json!(check.clone()));
        self.states.insert(task_name.clone(), Value::Object(st));
        let ty = match callee {
            Callee::Task(t) => m.tasks[*t].result.clone(),
            Callee::Rule(r) => Ty::Record(m.rules[*r].outputs),
        };
        let x = format!("${}", asl_var(&var));
        let typed = jsonata_check(m, &x, &ty, 0);
        let mut choices = Vec::new();
        match (target, m.monitors.get(&s.site)) {
            (Some(Target::Case(c)), Some((_, allowed))) => {
                let field = jsonata_path(&var, &[m.cases[*c].state_field.clone()]);
                choices.push(json!({ "Condition": format!("{{% {typed} and {field} in {} %}}", jsonata_list(allowed)), "Next": cont }));
                let unexpected = self.name(&format!("{} unexpected state from {}", s.line, cname));
                choices.push(json!({ "Condition": format!("{{% {typed} %}}"), "Next": unexpected.clone() }));
                self.states.insert(
                    unexpected,
                    json!({ "Type": "Fail", "Error": "Dandori.UnexpectedState", "Cause": format!("line {}: {} answered with a state the machine does not lead to here (expected one of {})", s.line, cname, allowed.join(", ")) }),
                );
            }
            _ => choices.push(json!({ "Condition": format!("{{% {typed} %}}"), "Next": cont })),
        }
        self.states.insert(check.clone(), json!({ "Type": "Choice", "Choices": choices, "Default": bad.clone() }));
        self.states.insert(bad, json!({ "Type": "Fail", "Error": "Dandori.BadResponse", "Cause": format!("line {}: the answer from {} does not have the declared shape", s.line, cname) }));
        task_name
    }
}

/// `retry` as ASL retriers. Without `on`, a task is retried on failures and timeouts but
/// never on its own declared errors: a retrier that stops them comes first.
pub fn retriers(m: &Model, callee: &Callee, task: &TaskDef, r: &Retry) -> Value {
    let every = r.every;
    let on: Vec<String> = if r.on.is_empty() { vec!["failure".into(), "timeout".into()] } else { r.on.clone() };
    let mut out = Vec::new();
    if on.iter().any(|x| x == "failure") {
        let mut stop: Vec<String> = Vec::new();
        for (e, _) in &task.errors {
            if !on.contains(e) {
                stop.extend(render::asl_error(m, callee, &HErr::Declared(e.clone())));
            }
        }
        if !on.iter().any(|x| x == "timeout") {
            stop.push("States.Timeout".into());
        }
        if !stop.is_empty() {
            out.push(json!({ "ErrorEquals": stop, "MaxAttempts": 0 }));
        }
        out.push(json!({ "ErrorEquals": ["States.ALL"], "IntervalSeconds": every, "MaxAttempts": r.times, "BackoffRate": r.backoff }));
    } else {
        let mut errs: Vec<String> = Vec::new();
        for e in &on {
            let he = if e == "timeout" { HErr::Timeout } else { HErr::Declared(e.clone()) };
            errs.extend(render::asl_error(m, callee, &he));
        }
        out.push(json!({ "ErrorEquals": errs, "IntervalSeconds": every, "MaxAttempts": r.times, "BackoffRate": r.backoff }));
    }
    Value::Array(out)
}

/// The URL with its `{placeholders}` put in, as a JSONata string when it has any.
fn url_value(url: &str, args: &[(String, TExpr)]) -> Value {
    let used = crate::lower::placeholders(url);
    if used.is_empty() {
        return json!(url);
    }
    let mut parts: Vec<String> = Vec::new();
    let mut rest = url;
    while let Some(i) = rest.find('{') {
        let j = match rest[i..].find('}') {
            Some(j) => i + j,
            None => break,
        };
        if i > 0 {
            parts.push(jsonata_string(&rest[..i]));
        }
        let p = &rest[i + 1..j];
        match args.iter().find(|(a, _)| a == p) {
            Some((_, e)) => parts.push(match e {
                TExpr::Var { .. } => jsonata_expr(e),
                other => jsonata_string(&render::value_text(&literal(other).unwrap())),
            }),
            None => parts.push(jsonata_string("")),
        }
        rest = &rest[j + 1..];
    }
    if !rest.is_empty() {
        parts.push(jsonata_string(rest));
    }
    json!(format!("{{% {} %}}", parts.join(" & ")))
}

fn show_expr(e: &TExpr) -> String {
    match e {
        TExpr::Var { name, fields, .. } => std::iter::once(name.clone()).chain(fields.iter().cloned()).collect::<Vec<_>>().join("."),
        TExpr::Str(s) => s.clone(),
        TExpr::Int(n) => n.to_string(),
        TExpr::Bool(b) => b.to_string(),
        TExpr::Enum(v, _) => v.clone(),
    }
}

/// The handler of the Lambda function that answers for a rule: JSON in, the call of the
/// Python rulec generated, JSON out.
pub(crate) fn lambda_handler(m: &Model, r: usize) -> (String, String) {
    let ru = &m.rules[r];
    let py = &ru.info.api["python"];
    let module = py["module"].as_str().unwrap_or("rule");
    let function = py["function"].as_str().unwrap_or("rule");
    let enums: Vec<String> = py["enums"].as_array().map(|a| a.iter().map(|e| e["alias"].as_str().unwrap_or("").to_string()).collect()).unwrap_or_default();
    let is_enum = |ty: &str| enums.iter().any(|a| a == ty);
    let mut imports: BTreeSet<String> = BTreeSet::new();
    imports.insert(function.to_string());
    let mut call_args = Vec::new();
    for p in py["params"].as_array().unwrap_or(&vec![]) {
        let name = p["name"].as_str().unwrap_or("");
        let ty = p["type"].as_str().unwrap_or("");
        let get = format!("event[{}]", py_str(name));
        if is_enum(ty) {
            imports.insert(ty.to_string());
            call_args.push(format!("{ty}({get})"));
        } else if ty == "bool" {
            call_args.push(format!("bool({get})"));
        } else if ty == "str" {
            call_args.push(format!("str({get})"));
        } else {
            call_args.push(format!("int({get})"));
        }
    }
    let mut outs = Vec::new();
    for o in py["outputs"].as_array().unwrap_or(&vec![]) {
        let name = o["name"].as_str().unwrap_or("");
        let alias = o["alias"].as_str().unwrap_or("");
        let ty = o["type"].as_str().unwrap_or("");
        let v = if is_enum(ty) {
            format!("out.{alias}.value")
        } else if ty == "bool" || ty == "str" {
            format!("out.{alias}")
        } else {
            format!("int(out.{alias})")
        };
        outs.push(format!("        {}: {v},", py_str(name)));
    }
    let sha = &ru.info.sha256[..12.min(ru.info.sha256.len())];
    let mut text = String::new();
    text.push_str(&format!("# Code generated by dandori from {}. DO NOT EDIT.\n", m.source_file));
    text.push_str(&format!("# The Lambda function the state machine calls for the rule {} v{} (sha256:{sha}).\n", ru.info.rule, ru.info.version));
    text.push_str(&format!("# Deploy it together with {module}.py, which `rulec gen` writes for the rule.\n\n"));
    text.push_str(&format!("from {module} import {}\n\n\n", imports.into_iter().collect::<Vec<_>>().join(", ")));
    text.push_str("def handler(event, context):\n");
    text.push_str(&format!("    out = {function}({})\n", call_args.join(", ")));
    text.push_str("    return {\n");
    text.push_str(&outs.join("\n"));
    text.push_str("\n    }\n");
    (format!("lambda/{module}_handler.py"), text)
}

fn py_str(s: &str) -> String {
    serde_json::to_string(s).unwrap()
}
