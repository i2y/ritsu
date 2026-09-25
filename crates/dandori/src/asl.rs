//! AWS Step Functions: a state machine in ASL with JSONata and variables, and for every
//! rule it calls, the Lambda function's handler around the Python rulec generates.
//!
//! Every variable of the workflow is a variable of the state machine, set to null at the
//! start. Every call is a Task followed by a Choice that checks the answer against its
//! declared type, and for a case, against the states the checker said it can be in; an
//! answer that fails the check ends the execution with `Dandori.BadResponse` or
//! `Dandori.UnexpectedState`, instead of being taken down a branch it does not belong to.
//!
//! A `for … in parallel` is a Map. Each round has its own variables, set at the round's
//! start, and ends with what it yields or with how it failed; a failure does not stop the
//! other rounds. When all are done, the first failure by place in the list decides.

use crate::diag::Diag;
use crate::model::*;
use crate::render::{self, asl_var, jsonata_check, jsonata_expr, jsonata_list, jsonata_path, jsonata_string};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

pub const RULE_RETRY_INTERVAL: u64 = 1;
pub const RULE_RETRY_BACKOFF: f64 = 2.0;

struct Gen<'a> {
    m: &'a Model,
    /// the states of the scope being written: the state machine's, or a Map round's
    states: Map<String, Value>,
    used: BTreeSet<String>,
    /// (site of the loop, the state after the loop)
    loops: Vec<(usize, String)>,
    failure_entry: Option<String>,
    in_on_failure: bool,
    /// inside a parallel round: the state that ends the round with the failure it is given
    round_failed: Option<String>,
}

pub fn build(m: &Model) -> Result<Vec<(String, String)>, Vec<Diag>> {
    let mut errs = Vec::new();
    for t in &m.tasks {
        match t.via(Platform::StepFunctions) {
            None => errs.push(Diag::error(
                "E050",
                t.line,
                1,
                format!("`{}` needs `lambda`, `http`, `aws` or `state machine` to run on Step Functions", t.name),
                format!("`{}` を Step Functions で動かすには `lambda`・`http`・`aws`・`state machine` のどれかが要ります", t.name),
            )),
            Some(Via::Http { .. }) if t.connection.is_none() => errs.push(Diag::error("E050", t.line, 1, format!("`{}` needs `connection \"<EventBridge connection ARN>\"`", t.name), format!("`{}` には `connection \"<EventBridge の接続の ARN>\"` が要ります", t.name))),
            Some(Via::StateMachine(_)) if !t.errors.is_empty() => errs.push(Diag::error(
                "E050",
                t.line,
                1,
                format!("Step Functions reports a nested execution's failure as States.TaskFailed, so the errors of `{}` cannot be told apart there; leave out `errors` and handle `failure`", t.name),
                format!("Step Functions は入れ子の実行の失敗を States.TaskFailed として伝えるので、`{}` のエラーを見分けられません。`errors` を外し、`failure` で受けてください", t.name),
            )),
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

    let mut g = Gen { m, states: Map::new(), used: BTreeSet::new(), loops: vec![], failure_entry: None, in_on_failure: false, round_failed: None };
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

    // Start: every variable, the inputs from the execution's input. A parallel round's own
    // variables belong to the round and are set at its start.
    let locals = parallel_locals(m);
    let mut assign = Map::new();
    for (v, _) in &m.vars {
        if locals.contains(v) {
            continue;
        }
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

/// Every variable some parallel round keeps for itself.
pub fn parallel_locals(m: &Model) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for s in m.all_stmts() {
        if let TK::For { locals, parallel: Some(_), .. } = &s.kind {
            out.extend(locals.iter().cloned());
        }
    }
    out
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

fn arg_value(e: &TExpr) -> Value {
    render::literal(e).unwrap_or_else(|| json!(format!("{{% {} %}}", jsonata_expr(e))))
}

/// A value of the ASL (a literal, or a `{% … %}` string) as a JSONata expression.
fn as_jsonata(v: &Value) -> String {
    match v.as_str() {
        Some(s) if s.starts_with("{%") && s.ends_with("%}") => s[2..s.len() - 2].trim().to_string(),
        _ => v.to_string(),
    }
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

    /// End as failed: a Fail state, or inside a parallel round, the round's end with the
    /// failure as its answer. `error` and `cause` are ASL values: literals or `{% … %}`.
    fn failing(&mut self, base: &str, comment: Option<String>, error: Value, cause: Option<Value>, task: bool) -> String {
        let n = self.name(base);
        match self.round_failed.clone() {
            None => {
                let mut st = Map::new();
                st.insert("Type".into(), json!("Fail"));
                if let Some(c) = comment {
                    st.insert("Comment".into(), json!(c));
                }
                st.insert("Error".into(), error);
                if let Some(c) = cause {
                    st.insert("Cause".into(), c);
                }
                self.states.insert(n.clone(), Value::Object(st));
            }
            Some(end) => {
                let c = cause.as_ref().map(as_jsonata).unwrap_or_else(|| "null".into());
                let out = format!("{{% {{\"fail\": {{\"Error\": {}, \"Cause\": {c}, \"task\": {task}}}}} %}}", as_jsonata(&error));
                let mut st = Map::new();
                st.insert("Type".into(), json!("Pass"));
                if let Some(c) = comment {
                    st.insert("Comment".into(), json!(c));
                }
                st.insert("Output".into(), json!(out));
                st.insert("Next".into(), json!(end));
                self.states.insert(n.clone(), Value::Object(st));
            }
        }
        n
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
                let n = self.name(&format!("{} wait until {}", s.line, at.show()));
                self.states.insert(n.clone(), json!({ "Type": "Wait", "Comment": format!("line {}", s.line), "Timestamp": format!("{{% {} %}}", jsonata_expr(at)), "Next": cont }));
                n
            }
            TK::Assign { name, expr } => {
                let n = self.name(&format!("{} let {name}", s.line));
                self.states.insert(n.clone(), json!({ "Type": "Pass", "Comment": format!("line {}", s.line), "Assign": { asl_var(name): arg_value(expr) }, "Next": cont }));
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
                let c = cause.as_ref().map(arg_value);
                self.failing(&format!("{} fail {error}", s.line), Some(format!("line {}", s.line)), json!(error), c, false)
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
            TK::For { var, list, max, parallel: None, body, result, .. } => {
                let site = s.site;
                let counter = format!("dd_loop_{site}");
                let items = format!("dd_items_{site}");
                let out = format!("dd_out_{site}");
                let init = self.name(&format!("{} for {var}", s.line));
                let bound = self.name(&format!("{} for {var} bound", s.line));
                let head = self.name(&format!("{} for {var} check", s.line));
                let take = self.name(&format!("{} for {var} take", s.line));
                let next = self.name(&format!("{} for {var} next", s.line));
                let after = match result {
                    Some((r, _)) => {
                        let a = self.name(&format!("{} for {var} done", s.line));
                        self.states.insert(a.clone(), json!({ "Type": "Pass", "Assign": { asl_var(r): format!("{{% ${out} %}}") }, "Next": cont }));
                        a
                    }
                    None => cont.to_string(),
                };
                self.loops.push((site, after.clone()));
                let entry = self.block(body, &next);
                self.loops.pop();
                let mut start = Map::new();
                start.insert(items.clone(), arg_value(list));
                start.insert(counter.clone(), json!(0));
                if result.is_some() {
                    start.insert(out.clone(), json!([]));
                }
                self.states.insert(init.clone(), json!({ "Type": "Pass", "Comment": format!("line {}: the list, at most {max} items", s.line), "Assign": start, "Next": bound }));
                let too_many = self.failing(
                    &format!("{} too many items", s.line),
                    None,
                    json!("Dandori.TooManyItems"),
                    Some(json!(format!("line {}: the list has more than {max} items", s.line))),
                    false,
                );
                self.states.insert(bound.clone(), json!({ "Type": "Choice", "Choices": [ { "Condition": format!("{{% $count(${items}) > {max} %}}"), "Next": too_many } ], "Default": head }));
                self.states.insert(head.clone(), json!({ "Type": "Choice", "Choices": [ { "Condition": format!("{{% ${counter} < $count(${items}) %}}"), "Next": take } ], "Default": after }));
                self.states.insert(take.clone(), json!({ "Type": "Pass", "Assign": { asl_var(var): format!("{{% ${items}[${counter}] %}}") }, "Next": entry }));
                let mut step = Map::new();
                step.insert(counter.clone(), json!(format!("{{% ${counter} + 1 %}}")));
                if let Some((_, y)) = result {
                    step.insert(out.clone(), json!(format!("{{% $append(${out}, {}) %}}", render::jsonata_one(&jsonata_expr(y), &y.ty()))));
                }
                self.states.insert(next.clone(), json!({ "Type": "Pass", "Assign": step, "Next": head }));
                init
            }
            TK::For { var, list, max, parallel: Some(k), body, result, locals } => self.map(s, var, list, *max, *k, body, result.as_ref(), locals, cont),
            TK::Match { expr, arms } => {
                let n = self.name(&format!("{} match {}", s.line, expr.show()));
                let x = jsonata_expr(expr);
                let mut choices = Vec::new();
                for a in arms {
                    let mut entry = self.block(&a.body, cont);
                    let mut parts = Vec::new();
                    if a.none {
                        match expr {
                            TExpr::Var { name, .. } if m_is_case(self.m, name) => parts.push(format!("${} = null", asl_var(name))),
                            _ => parts.push(format!("({x}) = null")),
                        }
                    }
                    if let Some(v) = &a.some {
                        parts.push(format!("({x}) != null"));
                        let bind = self.name(&format!("{} some {v}", a.line));
                        self.states.insert(bind.clone(), json!({ "Type": "Pass", "Assign": { asl_var(v): format!("{{% {x} %}}") }, "Next": entry }));
                        entry = bind;
                    }
                    if !a.values.is_empty() {
                        if expr.ty().inner() == &Ty::Bool {
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
                let bad = self.failing(
                    &format!("{} no arm", s.line),
                    None,
                    json!("Dandori.UnexpectedValue"),
                    Some(json!(format!("line {}: {} took a value that no arm names", s.line, expr.show()))),
                    false,
                );
                self.states.insert(n.clone(), json!({ "Type": "Choice", "Comment": format!("line {}", s.line), "Choices": choices, "Default": bad }));
                n
            }
            TK::Call { target, callee, args, handlers } => self.call(s, target.as_ref(), callee, args, handlers, cont),
        }
    }

    /// `for … in parallel`: a Map whose rounds end with `{"ok": <what they yield>}` or
    /// `{"fail": {"Error", "Cause", "task"}}`; after it, the first failure decides.
    #[allow(clippy::too_many_arguments)]
    fn map(&mut self, s: &TStmt, var: &str, list: &TExpr, max: u32, k: u32, body: &[TStmt], result: Option<&(String, TExpr)>, locals: &[String], cont: &str) -> String {
        let site = s.site;
        let rounds = format!("dd_par_{site}");
        let first = format!("dd_first_{site}");
        let name = self.name(&format!("{} for {var} in parallel", s.line));
        let bound = self.name(&format!("{} for {var} bound", s.line));

        // the rounds, in a scope of their own
        let outer_states = std::mem::take(&mut self.states);
        let outer_round = self.round_failed.take();
        let outer_loops = std::mem::take(&mut self.loops);
        let failed = self.name(&format!("{} round failed", s.line));
        let finished = self.name(&format!("{} round done", s.line));
        self.states.insert(failed.clone(), json!({ "Type": "Succeed", "Comment": "the round ends with its failure" }));
        let ok = match result {
            Some((_, y)) => format!("{{% {{\"ok\": {}}} %}}", jsonata_expr(y)),
            None => "{% {\"ok\": null} %}".to_string(),
        };
        self.states.insert(finished.clone(), json!({ "Type": "Succeed", "Output": ok }));
        self.round_failed = Some(failed);
        self.loops = vec![(site, finished.clone())];
        let entry = self.block(body, &finished);
        let begin = self.name(&format!("{} round of {var}", s.line));
        let mut start = Map::new();
        start.insert(asl_var(var), json!("{% $states.input.item %}"));
        start.insert(format!("dd_loop_{site}"), json!("{% $states.input.index %}"));
        for l in locals {
            if l != var {
                start.insert(asl_var(l), Value::Null);
            }
        }
        self.states.insert(begin.clone(), json!({ "Type": "Pass", "Comment": "the round's own variables", "Assign": start, "Next": entry }));
        let inner = order(&self.states, &begin);
        self.states = outer_states;
        self.round_failed = outer_round;
        self.loops = outer_loops;

        // after the rounds
        let check = self.name(&format!("{} for {var} rounds", s.line));
        let raise = self.name(&format!("{} for {var} failed", s.line));
        let how = self.name(&format!("{} for {var} how it failed", s.line));
        let collect = match result {
            Some((r, y)) => {
                let c = self.name(&format!("{} for {var} done", s.line));
                // what the rounds yielded, in order; a `json` value that is an array stays one item
                let all = if y.ty().inner() == &Ty::Json {
                    format!("{{% $reduce(${rounds}, function($acc, $r) {{ $append($acc, {}) }}, []) %}}", render::jsonata_one("$r.ok", &y.ty()))
                } else {
                    format!("{{% $append([], ${rounds}.ok) %}}")
                };
                self.states.insert(c.clone(), json!({ "Type": "Pass", "Assign": { asl_var(r): all }, "Next": cont }));
                c
            }
            None => cont.to_string(),
        };
        self.states.insert(check.clone(), json!({ "Type": "Choice", "Choices": [ { "Condition": format!("{{% $count(${rounds}[$exists(fail)]) > 0 %}}"), "Next": raise } ], "Default": collect }));
        self.states.insert(raise.clone(), json!({ "Type": "Pass", "Comment": "the first round that failed", "Assign": { first.clone(): format!("{{% (${rounds}[$exists(fail)])[0].fail %}}") }, "Next": how }));
        let as_task = self.rethrow(&format!("{} for {var} task error", s.line), &first, true);
        let as_fail = self.rethrow(&format!("{} for {var} fail", s.line), &first, false);
        self.states.insert(how.clone(), json!({ "Type": "Choice", "Choices": [ { "Condition": format!("{{% ${first}.task = true %}}"), "Next": as_task } ], "Default": as_fail }));

        let mut st = Map::new();
        st.insert("Type".into(), json!("Map"));
        st.insert("Comment".into(), json!(format!("line {}: every round runs to its end", s.line)));
        st.insert("Items".into(), arg_value(list));
        st.insert("ItemSelector".into(), json!({ "item": "{% $states.context.Map.Item.Value %}", "index": "{% $states.context.Map.Item.Index %}" }));
        st.insert("MaxConcurrency".into(), json!(k));
        st.insert("ItemProcessor".into(), json!({ "ProcessorConfig": { "Mode": "INLINE" }, "StartAt": begin, "States": Value::Object(inner) }));
        st.insert("Assign".into(), json!({ rounds.clone(): "{% $states.result %}" }));
        st.insert("Next".into(), json!(check));
        self.states.insert(name.clone(), Value::Object(st));
        let too_many = self.failing(&format!("{} too many items", s.line), None, json!("Dandori.TooManyItems"), Some(json!(format!("line {}: the list has more than {max} items", s.line))), false);
        self.states.insert(bound.clone(), json!({ "Type": "Choice", "Choices": [ { "Condition": format!("{{% $count({}) > {max} %}}", jsonata_expr(list)) , "Next": too_many } ], "Default": name }));
        bound
    }

    /// Fail again as a parallel round did (`first` holds how): a task's error goes to
    /// `on failure`; inside another round, the round ends with the same failure.
    fn rethrow(&mut self, base: &str, first: &str, task: bool) -> String {
        if let Some(end) = self.round_failed.clone() {
            let n = self.name(base);
            self.states.insert(n.clone(), json!({ "Type": "Pass", "Output": format!("{{% {{\"fail\": ${first}}} %}}"), "Next": end }));
            return n;
        }
        if task && !self.in_on_failure {
            if let Some(f) = self.failure_entry.clone() {
                let n = self.name(base);
                self.states.insert(n.clone(), json!({ "Type": "Pass", "Assign": { "dd_error": format!("{{% {{\"Error\": ${first}.Error, \"Cause\": ${first}.Cause}} %}}") }, "Next": f }));
                return n;
            }
        }
        // a Fail's cause is a string or nothing
        let n = self.name(base);
        let with = self.name(&format!("{base} with cause"));
        let without = self.name(&format!("{base} without cause"));
        self.states.insert(with.clone(), json!({ "Type": "Fail", "Error": format!("{{% ${first}.Error %}}"), "Cause": format!("{{% ${first}.Cause %}}") }));
        self.states.insert(without.clone(), json!({ "Type": "Fail", "Error": format!("{{% ${first}.Error %}}") }));
        self.states.insert(n.clone(), json!({ "Type": "Choice", "Choices": [ { "Condition": format!("{{% $type(${first}.Cause) = \"string\" %}}"), "Next": with } ], "Default": without }));
        n
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
                match task.via(Platform::StepFunctions).expect("build refuses a task Step Functions cannot call") {
                    Via::Lambda(f) => {
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
                    Via::Aws { service, action } => {
                        if let (true, Some(p)) = (task.key, &task.key_param) {
                            payload.insert(p.clone(), json!(self.key_expr(s.site)));
                        }
                        if task.callback {
                            // the token goes in the message; whoever reads it answers with SendTaskSuccess
                            let body = args.iter().find(|(a, _)| a == "MessageBody").map(|(_, e)| jsonata_expr(e)).unwrap_or_else(|| "{}".into());
                            payload.insert("MessageBody".into(), json!(format!("{{% $merge([{body}, {{\"task_token\": $states.context.Task.Token}}]) %}}")));
                            (format!("arn:aws:states:::{service}:{action}.waitForTaskToken"), Value::Object(payload), "$states.result".to_string(), task.timeout, retry)
                        } else {
                            (format!("arn:aws:states:::aws-sdk:{service}:{action}"), Value::Object(payload), "$states.result".to_string(), task.timeout, retry)
                        }
                    }
                    Via::StateMachine(arn) => {
                        ("arn:aws:states:::states:startExecution.sync:2".to_string(), json!({ "StateMachineArn": arn, "Input": payload }), "$states.result.Output".to_string(), task.timeout, retry)
                    }
                    Via::Http { method, url, form } => {
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
                        if form {
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
                        if form {
                            w.insert("Transform".into(), json!({ "RequestBodyEncoding": "URL_ENCODED" }));
                        }
                        ("arn:aws:states:::http:invoke".to_string(), Value::Object(w), "$states.result.ResponseBody".to_string(), task.timeout, retry)
                    }
                    Via::Workflow(_) | Via::DurableFunction(_) | Via::Own | Via::Image(_) | Via::ArgoTemplate(_) => unreachable!("not a way Step Functions calls"),
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
        if !catches_all {
            if let Some(end) = self.round_failed.clone() {
                catches.push(json!({
                    "ErrorEquals": ["States.ALL"],
                    "Output": "{% {\"fail\": {\"Error\": $states.errorOutput.Error, \"Cause\": $states.errorOutput.Cause, \"task\": true}} %}",
                    "Next": end
                }));
            } else if !self.in_on_failure {
                if let Some(f) = &self.failure_entry {
                    catches.push(json!({ "ErrorEquals": ["States.ALL"], "Assign": { "dd_error": "{% $states.errorOutput %}" }, "Next": f }));
                }
            }
        }
        if !catches.is_empty() {
            st.insert("Catch".into(), Value::Array(catches));
        }

        // the answer: its declared type, and for a case the states it may carry
        let ty = match callee {
            Callee::Task(t) => m.tasks[*t].result.clone(),
            Callee::Rule(r) => Some(Ty::Record(m.rules[*r].outputs)),
        };
        let ty = match (var.is_empty(), ty) {
            (false, Some(t)) => t,
            _ => {
                st.insert("Next".into(), json!(cont));
                self.states.insert(task_name.clone(), Value::Object(st));
                return task_name;
            }
        };
        st.insert("Next".into(), json!(check.clone()));
        self.states.insert(task_name.clone(), Value::Object(st));
        let x = format!("${}", asl_var(&var));
        let typed = jsonata_check(m, &x, &ty, 0);
        let mut choices = Vec::new();
        match (target, m.monitors.get(&s.site)) {
            (Some(Target::Case(c)), Some((_, allowed))) => {
                let field = jsonata_path(&var, &[m.cases[*c].state_field.clone()]);
                choices.push(json!({ "Condition": format!("{{% {typed} and {field} in {} %}}", jsonata_list(allowed)), "Next": cont }));
                let unexpected = self.failing(
                    &format!("{} unexpected state from {}", s.line, cname),
                    None,
                    json!("Dandori.UnexpectedState"),
                    Some(json!(format!("line {}: {} answered with a state the machine does not lead to here (expected one of {})", s.line, cname, allowed.join(", ")))),
                    false,
                );
                choices.push(json!({ "Condition": format!("{{% {typed} %}}"), "Next": unexpected }));
            }
            _ => choices.push(json!({ "Condition": format!("{{% {typed} %}}"), "Next": cont })),
        }
        let bad = self.failing(
            &format!("{} bad answer from {}", s.line, cname),
            None,
            json!("Dandori.BadResponse"),
            Some(json!(format!("line {}: the answer from {} does not have the declared shape", s.line, cname))),
            false,
        );
        self.states.insert(check.clone(), json!({ "Type": "Choice", "Choices": choices, "Default": bad }));
        task_name
    }
}

fn m_is_case(m: &Model, name: &str) -> bool {
    m.case_index(name).is_some()
}

/// `retry` as ASL retriers. Without `on`, a task is retried on failures and timeouts but
/// never on its own declared errors: a retrier that stops them comes first.
pub fn retriers(m: &Model, callee: &Callee, task: &TaskDef, r: &Retry) -> Value {
    let every = r.every;
    let on: Vec<String> = if r.on.is_empty() { vec!["failure".into(), "timeout".into()] } else { r.on.clone() };
    let mut out = Vec::new();
    if on.iter().any(|x| x == "failure") {
        let mut stop: Vec<String> = Vec::new();
        for e in &task.errors {
            if !on.contains(&e.name) {
                stop.extend(render::asl_error(m, callee, &HErr::Declared(e.name.clone())));
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
            Some((_, e)) => parts.push(match render::literal(e) {
                Some(v) => jsonata_string(&render::value_text(&v)),
                None => jsonata_expr(e),
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
