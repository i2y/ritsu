//! dandori's reference interpreter, held to `DandoriCore` (DESIGN 11.3).
//!
//! Every flow of dandori's examples and tests, the ones that use koyomi's dates and chobo's books
//! too (`use dates`, `use book`, `now`): each scenario `dandori scenarios` writes for it is run by
//! the reference interpreter (as Temporal sees a run: an error known by its kind) and by the Lean
//! model, and the two are compared — everything the run went through (each statement, arm, answer,
//! handler, round and loop's end, `on failure` and `on cancel`), how it ended (the outputs, or the
//! error and its cause), the state each case's record says at the end, and the arguments each call
//! is given, where the call the platform sees carries them as they are. How some answers are read
//! before the flow sees them is not the core (DESIGN 11.2), and the model is handed what dandori's
//! own functions read each one as (`read_as`); a book's operation, whose answer is the hold its
//! arguments make, is the model's own (`DandoriCore.bookValue`). `-- --nocapture` shows a
//! `compared dandori <flow>: <n> lines` line for each flow.

use dandori::interp::{self, Visit};
use dandori::model::*;
use dandori::render::{self, View};
use ritsu_model::{compare, program};
use ritsu_testkit::{Need, TempDir, ready};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn dandori_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../dandori")
}

/// Run `f` reading the rules, the dates files and the books through rulec's, koyomi's and chobo's
/// own answers to the ports, as a program that joins them all reads them.
fn with_rules<R>(f: impl FnOnce() -> R) -> R {
    thread_local! {
        static RULEC: std::rc::Rc<rulec::ports::Engine> = std::rc::Rc::new(rulec::ports::Engine::new());
    }
    let rules: std::rc::Rc<dyn ritsu_ports::Rules> = RULEC.with(|r| r.clone());
    dandori::sources::with_ports(rules, std::rc::Rc::new(koyomi::ports::Engine), std::rc::Rc::new(chobo::ports::Engine), f)
}

/// Every `.flow` under a directory, in order, the English ones before their Japanese twins.
fn flows_in(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).collect();
    entries.sort_by_key(|p| (p.to_string_lossy().ends_with(".ja.flow"), p.clone()));
    for p in entries {
        if p.is_dir() {
            flows_in(&p, out);
        } else if p.extension().is_some_and(|e| e == "flow") {
            out.push(p);
        }
    }
}

/// What dandori reads an answer as before the flow sees it, for a callee that reads one: Jev's
/// answer (or the error of one less sure than the task asks), protobuf's zero values put back, a
/// Claude agent's enum values spelled as the enum spells them, a rule's answer from its Connect
/// service. None for a callee that takes an answer as it is. These are dandori's own functions; the
/// model is handed what they give and holds what the flow does with it.
fn read_as(m: &Model, callee: &Callee, v: &Value) -> Option<Value> {
    match callee {
        Callee::Rule(r) => m.rules[*r].connect.as_ref().map(|c| json!({"value": render::rule_read(c, v)})),
        Callee::Task(t) => {
            let task = &m.tasks[*t];
            let claude = matches!(task.via(Platform::Temporal), Some(Via::Agent { provider: Provider::Claude, .. })) && task.result.is_some();
            let fold = |x: Value| if claude { render::fold_enums(m, &x, task.result.as_ref().unwrap()) } else { x };
            if let Some(j) = task.jev() {
                let (read, low) = render::jev_read(j, v);
                if let (true, Some((_, error))) = (low, &j.floor) {
                    return Some(json!({"error": error, "cause": render::jev_low_cause(task)}));
                }
                return Some(json!({"value": fold(read)}));
            }
            let filled = task.connect.as_ref().or(task.answer_zeros.as_ref()).map(|z| dandori::apis::fill(v, z));
            if filled.is_none() && !claude {
                return None;
            }
            Some(json!({"value": fold(filled.unwrap_or_else(|| v.clone()))}))
        }
    }
}

/// The scenario as the model reads it: the input as the flow reads it (a flow that implements a
/// service reads the zero values its request leaves out), the answers, and what dandori reads each
/// answer as for every callee that reads one.
fn scenario_line(m: &Model, sc: &Value) -> String {
    let input = match &m.service {
        Some(s) => dandori::apis::fill(&sc["input"], &s.input_zeros),
        None => sc["input"].clone(),
    };
    let callees: Vec<Callee> = (0..m.tasks.len()).map(Callee::Task).chain((0..m.rules.len()).map(Callee::Rule)).collect();
    let mut reads = Vec::new();
    for (i, a) in sc["answers"].as_array().into_iter().flatten().enumerate() {
        let Some(v) = a.get("ok") else { continue };
        for c in &callees {
            if let Some(r) = read_as(m, c, v) {
                let who = match c { Callee::Task(t) => json!({"task": t}), Callee::Rule(r) => json!({"rule": r}) };
                reads.push(json!([i, who, r]));
            }
        }
    }
    json!({"input": input, "answers": sc["answers"], "reads": reads, "now": sc.get("now").cloned().unwrap_or(Value::Null)}).to_string()
}

fn range(r: Option<Range>) -> Value {
    r.map(|r| json!({"lo": r.lo, "hi": r.hi})).unwrap_or(Value::Null)
}

fn ty(m: &Model, t: &Ty) -> Value {
    match t {
        Ty::Int | Ty::Num(_) => json!("int"),
        Ty::Str => json!("string"),
        Ty::Bool => json!("bool"),
        Ty::Timestamp => json!("timestamp"),
        Ty::Date => json!("date"),
        Ty::Json => json!("json"),
        Ty::Enum(e) => json!({"enum": m.enums[*e].values}),
        Ty::Record(r) => json!({"record": r}),
        Ty::List(t) => json!({"list": ty(m, t)}),
        Ty::Opt(t) => json!({"opt": ty(m, t)}),
    }
}

fn expr(e: &TExpr) -> Value {
    match e {
        TExpr::Var { name, fields, .. } => json!({"var": name, "fields": fields}),
        TExpr::Str(s) => json!({"lit": s}),
        TExpr::Int(n) => json!({"lit": n}),
        TExpr::Bool(b) => json!({"lit": b}),
        TExpr::Enum(v, _) => json!({"lit": v}),
        TExpr::None(_) => json!({"lit": null}),
        TExpr::Now => json!({"now": null}),
        TExpr::Record { fields, .. } => json!({"record": fields.iter().map(|(f, x)| json!([f, expr(x)])).collect::<Vec<_>>()}),
        TExpr::List { items, .. } => json!({"list": items.iter().map(expr).collect::<Vec<_>>()}),
        TExpr::Interp(parts) => json!({"interp": parts.iter().map(|p| match p {
            IPart::Lit(s) => json!({"lit": s}),
            IPart::Hole(x) => expr(x),
        }).collect::<Vec<_>>()}),
    }
}

/// The retriers of a callee as the reference interpreter has them for Temporal: the ASL's, with
/// the errors named by their kind (`interp.rs`'s `retriers`).
fn retriers(m: &Model, callee: &Callee) -> Value {
    let v = match callee {
        Callee::Rule(_) => dandori::asl::rule_retriers(),
        Callee::Task(t) => match &m.tasks[*t].retry {
            Some(r) => dandori::asl::retriers(m, callee, &m.tasks[*t], r),
            None => json!([]),
        },
    };
    let mut out = Vec::new();
    for r in v.as_array().unwrap() {
        let names: Vec<String> = r["ErrorEquals"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().to_string())
            .map(|n| {
                if n == "States.Timeout" {
                    "timeout".to_string()
                } else if n == "States.ALL" {
                    n
                } else {
                    match callee {
                        Callee::Task(t) => m.tasks[*t]
                            .errors
                            .iter()
                            .find(|e| render::asl_error(m, callee, &HErr::Declared(e.name.clone())).contains(&n))
                            .map(|e| e.name.clone())
                            .unwrap_or(n),
                        Callee::Rule(_) => n,
                    }
                }
            })
            .collect();
        out.push(json!({"names": names, "max": r["MaxAttempts"].as_u64().unwrap_or(3)}));
    }
    Value::Array(out)
}

fn block(m: &Model, ss: &[TStmt]) -> Value {
    Value::Array(ss.iter().map(|s| stmt(m, s)).collect())
}

fn stmt(m: &Model, s: &TStmt) -> Value {
    let k = match &s.kind {
        TK::Call { target, callee, args, handlers } => json!({"call": {
            "target": match target {
                None => Value::Null,
                Some(Target::Let(x)) => json!({"let": x}),
                Some(Target::Case(c)) => json!({"case": c}),
            },
            "callee": match callee { Callee::Task(t) => json!({"task": t}), Callee::Rule(r) => json!({"rule": r}) },
            "args": args.iter().map(|(n, e)| json!([n, expr(e)])).collect::<Vec<_>>(),
            "handlers": handlers.iter().map(|h| json!({
                "errors": h.errors.iter().map(|e| match e {
                    HErr::Failure => json!("failure"),
                    HErr::Timeout => json!("timeout"),
                    HErr::Declared(n) => {
                        let mut names = render::asl_error(m, callee, &HErr::Declared(n.clone()));
                        names.push(n.clone());
                        json!({"declared": names})
                    }
                }).collect::<Vec<_>>(),
                "body": block(m, &h.body),
            })).collect::<Vec<_>>(),
        }}),
        TK::Assign { name, expr: e } => json!({"assign": {"name": name, "expr": expr(e)}}),
        TK::Match { expr: e, arms } => json!({"match": {
            "expr": expr(e),
            "shown": e.show(),
            "arms": arms.iter().map(|a| json!({"values": a.values, "none": a.none, "some": a.some, "body": block(m, &a.body)})).collect::<Vec<_>>(),
        }}),
        TK::Wait { .. } | TK::WaitUntil { .. } => json!({"wait": true}),
        TK::Repeat { times, body } => json!({"repeat": {"times": times, "body": block(m, body)}}),
        TK::For { var, list, max, body, result, parallel, locals } => json!({"for": {
            "var": var, "list": expr(list), "max": max, "body": block(m, body),
            "result": result.as_ref().map(|(r, y)| json!({"name": r, "yield": expr(y)})),
            "parallel": parallel.map(|_| locals.clone()),
        }}),
        TK::Break => json!({"break": true}),
        TK::Pass => json!({"pass": true}),
        TK::Succeed { fields } => json!({"succeed": {"fields": fields.iter().map(|(f, e)| json!([f, expr(e)])).collect::<Vec<_>>()}}),
        TK::Fail { error, cause, leaving } => json!({"fail": {"error": error, "cause": cause.as_ref().map(expr), "leaving": leaving}}),
    };
    let mut o = k.as_object().unwrap().clone();
    o.insert("site".into(), json!(s.site));
    o.insert("line".into(), json!(s.line));
    Value::Object(o)
}

/// The flow as `DandoriCore.readFlow` reads it.
fn flow_json(m: &Model) -> Value {
    json!({
        "records": m.records.iter().enumerate().map(|(r, rec)| rec.fields.iter().map(|(f, t)| json!({"name": f, "ty": ty(m, t), "range": range(m.field_range(r, f))})).collect::<Vec<_>>()).collect::<Vec<_>>(),
        "inputs": m.inputs.iter().map(|(n, t)| json!({"name": n, "ty": ty(m, t), "range": range(m.input_ranges.get(n).copied())})).collect::<Vec<_>>(),
        "tasks": m.tasks.iter().enumerate().map(|(i, t)| json!({
            "name": t.name,
            "result": t.result.as_ref().map(|r| ty(m, r)),
            "range": range(t.result_range),
            "retriers": retriers(m, &Callee::Task(i)),
            "book": t.book().map(|b| json!({"book": b.book, "transfer": b.transfer, "op": b.op})),
        })).collect::<Vec<_>>(),
        "rules": m.rules.iter().enumerate().map(|(i, r)| json!({
            "name": r.name,
            "outputs": r.outputs,
            "retriers": retriers(m, &Callee::Rule(i)),
            "kind": match &r.kind {
                RuleKind::Rule => json!("rule"),
                RuleKind::Date(d) => json!({"date": [d.file, d.date]}),
                RuleKind::Hold { book, transfer } => json!({"hold": [book, transfer]}),
            },
        })).collect::<Vec<_>>(),
        // the parameters of each transfer's key in the order the transfer declares them: what a hold's
        // record carries (`render::book_value`)
        "books": m.books.iter().map(|b| json!({
            "name": b.name,
            "transfers": b.facts.transfers.iter().map(|t| json!({
                "name": t.name,
                "key": t.params.iter().filter(|p| t.key.contains(&p.name)).map(|p| p.name.clone()).collect::<Vec<_>>(),
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "cases": m.cases.iter().map(|c| json!({"name": c.name, "state_field": c.state_field, "rule": c.rule})).collect::<Vec<_>>(),
        "monitors": m.monitors.iter().map(|(site, (_, allowed))| json!([site, allowed])).collect::<Vec<_>>(),
        "flow": block(m, &m.flow),
        "on_failure": m.on_failure.as_ref().map(|b| block(m, b)),
        "on_cancel": m.on_cancel.as_ref().map(|b| block(m, b)),
        "service": m.service.is_some(),
    })
}

fn visit(v: &Visit) -> String {
    match v {
        Visit::Stmt(s) => format!("s{s}"),
        Visit::Arm(s, i) => format!("a{s}.{i}"),
        Visit::Answer(s, k) => format!("r{s}:{k}"),
        Visit::Handler(s, j) => format!("h{s}.{j}"),
        Visit::Unhandled(s) => format!("u{s}"),
        Visit::Round(s, n) => format!("o{s}.{n}"),
        Visit::Done(s) => format!("d{s}"),
        Visit::OnFailure => "of".into(),
        Visit::OnCancel => "oc".into(),
        Visit::FlowEnd => "fe".into(),
        Visit::OnFailureEnd => "ofe".into(),
        Visit::OnCancelEnd => "oce".into(),
    }
}

/// The callee of every call, by the call's site.
fn callees(m: &Model) -> std::collections::BTreeMap<usize, Callee> {
    m.all_stmts().into_iter().filter_map(|s| match &s.kind {
        TK::Call { callee, .. } => Some((s.site, callee.clone())),
        _ => None,
    }).collect()
}

/// The sites of the calls whose wire carries their arguments as they are (`carried`): a rule's or a
/// date's (not one called at its Connect service), and a task that Temporal runs as its own
/// activity or a Lambda function. The binding decides it, whatever the run.
fn carrying(m: &Model) -> std::collections::BTreeSet<usize> {
    callees(m)
        .into_iter()
        .filter(|(_, c)| match c {
            Callee::Rule(r) => m.rules[*r].connect.is_none(),
            Callee::Task(t) => matches!(m.tasks[*t].via(Platform::Temporal), Some(Via::Own) | Some(Via::Image(_)) | Some(Via::Lambda(_))),
        })
        .map(|(site, _)| site)
        .collect()
}

/// The arguments a call was given, where the call as Temporal sees it carries them as they are: a
/// rule's or a date's activity, a task's own activity or its Lambda function (the idempotency key
/// beside them taken out). None for the other ways a call goes out (HTTP, an agent, a book, …),
/// which make something else of the arguments.
fn carried(m: &Model, callee: &Callee, wire: &Value) -> Option<Value> {
    let mut a = if wire.get("activity").is_some() {
        wire.get("args")?.clone()
    } else if wire.get("lambda").is_some() {
        wire.get("payload")?.clone()
    } else {
        return None;
    };
    if let Callee::Task(t) = callee {
        if m.tasks[*t].key {
            a.as_object_mut()?.remove("idempotency_key");
        }
    }
    Some(a)
}

/// What the reference interpreter says of one scenario, as the model prints it, with the arguments
/// of the calls whose wire carries them (the sites in `shown`, which `same` holds the model to).
fn ours(m: &Model, sc: &Value, shown: &std::collections::BTreeSet<usize>) -> String {
    let run = interp::run_visits(m, sc, View::Temporal).and_then(|(trace, visits)| interp::cases_at_end(m, sc, View::Temporal).map(|cases| (trace, visits, cases)));
    match run {
        Ok((trace, visits, cases)) => {
            // every answer a call takes is one step of the trace, in the order of the visits' answers
            let by_site = callees(m);
            let steps = trace["steps"].as_array().cloned().unwrap_or_default();
            let answered = visits.iter().filter_map(|v| match v {
                Visit::Answer(site, _) => Some(*site),
                _ => None,
            });
            let mut args = Vec::new();
            for (site, step) in answered.zip(&steps) {
                if !shown.contains(&site) {
                    continue;
                }
                // a call `carrying` names whose wire does not carry its arguments goes in as null, and
                // differs from the model's
                let a = by_site.get(&site).and_then(|c| carried(m, c, &step["call"])).unwrap_or(Value::Null);
                args.push(json!([site, a]));
            }
            json!({
                "visits": visits.iter().map(visit).collect::<Vec<_>>(),
                "args": args,
                "end": trace["end"],
                "cases": cases,
            })
            .to_string()
        }
        Err(e) => json!({"error": e}).to_string(),
    }
}

/// Whether the model's run is ours: the same in everything, the arguments compared for the calls
/// whose wire carries them (the model prints them for every call).
fn same(ours: &str, model: &str, shown: &std::collections::BTreeSet<usize>) -> bool {
    let (Ok(a), Ok(mut b)) = (serde_json::from_str::<Value>(ours), serde_json::from_str::<Value>(model)) else { return false };
    if a.get("error").is_some() || b.get("error").is_some() {
        return a.get("error").is_some() && b.get("error").is_some();
    }
    if let Some(args) = b.get_mut("args").and_then(|x| x.as_array_mut()) {
        args.retain(|e| e[0].as_u64().is_some_and(|site| shown.contains(&(site as usize))));
    }
    a == b
}

#[test]
fn every_scenario_of_every_flow_runs_as_the_model_says() {
    let bin = program();
    if !ready(Need::Lean, || bin.exists(), "proofs/ is not built (lake build makes ritsu-model)") {
        return;
    }
    let tmp = TempDir::new("model-dandori");
    let root = dandori_dir();
    let mut files = Vec::new();
    for d in ["examples", "tests/flows", "tests/children"] {
        flows_in(&root.join(d), &mut files);
    }
    let (mut flows, mut lines, mut outside, mut arg_calls) = (0, 0usize, Vec::new(), 0usize);
    for (i, p) in files.iter().enumerate() {
        let shown = p.strip_prefix(&root).unwrap().display().to_string();
        let m = with_rules(|| dandori::check::check_file(p)).unwrap_or_else(|e| panic!("{shown}: {e}")).1.model;
        let Some(m) = m else {
            outside.push(format!("{shown}: does not pass check"));
            continue;
        };
        let scenarios = dandori::scenarios::generate(&m);
        let file = tmp.write(&format!("{i}.json"), flow_json(&m).to_string());
        let sites = carrying(&m);
        let rows = scenarios.iter().map(|sc| (scenario_line(&m, sc), ours(&m, sc, &sites)));
        let c = with_rules(|| compare(&bin, "dandori", &file, rows, &|a: &str, b: &str| same(a, b, &sites))).unwrap_or_else(|e| panic!("{shown}: {e}"));
        assert_eq!(c.differ, 0, "{}", c.report(&shown));
        println!("compared dandori {shown}: {} lines, the arguments of {} call(s)", c.lines, sites.len());
        flows += 1;
        lines += c.lines;
        arg_calls += sites.len();
    }
    for o in &outside {
        println!("not compared: {o}");
    }
    assert!(outside.is_empty(), "every flow of the examples and tests passes check");
    assert!(flows >= 52, "the flows are fewer than they were: {flows}");
    assert!(lines >= 1150, "the scenarios are fewer than they were: {lines}");
    assert!(arg_calls >= 40, "the calls whose arguments are compared are fewer than they were: {arg_calls}");
}
