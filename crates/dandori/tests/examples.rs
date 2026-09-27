//! The examples pass `check`; the fixtures print exactly the diagnostics in their golden
//! files; and on every scenario `scenarios` finds, the generated state machine, run by
//! tools/asl-run.mjs with JSONata 2.0.6, does exactly what the reference interpreter does.
//!
//! The rules are read with rulec (`DANDORI_RULEC`, else `rulec` on the PATH), and the
//! state machines are run with Node and the packages in tools/ (`npm install --prefix
//! tools`). A test that cannot find them says so and skips; read the skip lines.
//! `DANDORI_BLESS=1` rewrites the golden files.

use dandori::diag::Lang;
use dandori::interp::CallInfo;
use dandori::model::{Callee, Model, Platform, Via, TK};
use dandori::render::View;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rulec_available() -> bool {
    let bin = std::env::var("DANDORI_RULEC").unwrap_or_else(|_| "rulec".into());
    Command::new(&bin).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

fn node_available() -> bool {
    root().join("tools/node_modules/jsonata").exists() && Command::new("node").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

macro_rules! need_rulec {
    () => {
        if !rulec_available() {
            eprintln!("SKIP: rulec is not on the PATH; set DANDORI_RULEC to run this test");
            return;
        }
    };
}

macro_rules! need_node {
    () => {
        if !node_available() {
            eprintln!("SKIP: node or tools/node_modules is missing; run `npm install --prefix tools`");
            return;
        }
    };
}

fn flows(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        if let Ok(rd) = std::fs::read_dir(&d) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().map(|x| x == "flow").unwrap_or(false) {
                    out.push(p);
                }
            }
        }
    }
    out.sort();
    out
}

/// The flows every platform runs: the examples, and the ones in tests/flows that exercise
/// the corners of the language. `DANDORI_FLOW=<part of a path>` runs only the flows whose
/// path has it, to look at one flow on a slow platform.
fn runnable() -> Vec<PathBuf> {
    let mut out = flows(&root().join("examples"));
    out.extend(flows(&root().join("tests/flows")));
    if let Ok(part) = std::env::var("DANDORI_FLOW") {
        out.retain(|f| rel(f).contains(&part));
    }
    out
}

/// The platform a version of an example is written for, from its directory: `temporal/`,
/// `pydantic-graph/` or `argo/`, which only that platform's runners play. None for a version
/// written for AWS (`aws/`), which every platform plays, since the code dandori writes for each
/// makes its Lambda, HTTP and AWS calls; and for a flow beside the versions, which every
/// platform runs as it is (and for the flows of tests/flows).
fn written_for(f: &Path) -> Option<Platform> {
    let r = rel(f);
    if r.contains("/temporal/") {
        Some(Platform::Temporal)
    } else if r.contains("/pydantic-graph/") {
        Some(Platform::Graph)
    } else if r.contains("/argo/") {
        Some(Platform::Argo)
    } else {
        None
    }
}

/// The flows a platform's runner plays: every flow but the versions written for another platform.
fn runnable_on(p: Platform) -> Vec<PathBuf> {
    runnable().into_iter().filter(|f| written_for(f).is_none_or(|w| w == p)).collect()
}

/// The tests that start a process for every flow at once — the Temporal runners with their dev
/// servers, and the durable functions runners — take turns: run together, they slow each other
/// down, and the Argo test beside them, more than they gain. The Argo test, the longest, does not
/// wait for them.
static HEAVY: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn heavy() -> std::sync::MutexGuard<'static, ()> {
    HEAVY.lock().unwrap_or_else(|e| e.into_inner())
}

fn rel(p: &Path) -> String {
    p.strip_prefix(root()).unwrap_or(p).display().to_string()
}

/// A name for a flow's work directory, from its path: the versions of an example, written for
/// different platforms, share the workflow's name.
fn key(f: &Path) -> String {
    rel(f).trim_end_matches(".flow").replace(['/', '.'], "-")
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("dandori-test-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Numbers compared as numbers: 2 and 2.0 are the same wait.
fn norm(v: &Value) -> Value {
    match v {
        Value::Number(n) => json!(n.as_f64().unwrap()),
        Value::Array(a) => Value::Array(a.iter().map(norm).collect()),
        Value::Object(o) => Value::Object(o.iter().map(|(k, x)| (k.clone(), norm(x))).collect()),
        other => other.clone(),
    }
}

#[test]
fn examples_pass_check() {
    need_rulec!();
    for f in flows(&root().join("examples")) {
        let (src, checked) = dandori::check::check_file(&f).unwrap();
        let text: String = checked.diags.iter().map(|d| d.render(&rel(&f), &src, Lang::En)).collect();
        assert!(checked.model.is_some(), "{} does not pass check:\n{text}", rel(&f));
        assert!(checked.diags.is_empty(), "{} has warnings:\n{text}", rel(&f));
    }
}

#[test]
fn diagnostics_match_the_golden_files() {
    need_rulec!();
    let bless = std::env::var("DANDORI_BLESS").is_ok();
    let mut failures = Vec::new();
    for f in flows(&root().join("tests/fixtures")) {
        let (src, checked) = dandori::check::check_file(&f).unwrap();
        // a fixture that passes check has its builds' refusals (E050) in the golden file too
        let refusals: Vec<(&str, Vec<dandori::diag::Diag>)> = match &checked.model {
            Some(m) => [
                ("asl", dandori::asl::build(m)),
                ("temporal", dandori::temporal::build(m)),
                ("temporal-python", dandori::temporal_py::build(m)),
                ("durable", dandori::temporal::build_flavor(m, dandori::temporal::Flavor::Durable)),
                ("argo", dandori::argo::build(m)),
                ("pydantic-graph", dandori::pydantic_graph::build(m)),
            ]
            .into_iter()
            .filter_map(|(target, built)| built.err().map(|d| (target, d)))
            .collect(),
            None => vec![],
        };
        for (lang, tag) in [(Lang::En, "en"), (Lang::Ja, "ja")] {
            let mut text: String = checked.diags.iter().map(|d| d.render(&rel(&f), &src, lang)).collect();
            for (target, diags) in &refusals {
                text.push_str(&format!("build --target {target}:\n"));
                text.extend(diags.iter().map(|d| d.render(&rel(&f), &src, lang)));
            }
            let golden = f.with_extension(format!("{tag}.txt"));
            if bless {
                std::fs::write(&golden, &text).unwrap();
                continue;
            }
            let want = std::fs::read_to_string(&golden).unwrap_or_default();
            if want != text {
                failures.push(format!("{} ({tag}) differs from {}:\n--- want\n{want}\n--- got\n{text}", rel(&f), rel(&golden)));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

#[test]
fn asl_runs_as_the_reference_says() {
    need_rulec!();
    need_node!();
    let mut compared = 0;
    for f in runnable_on(Platform::StepFunctions) {
        let (_, checked) = dandori::check::check_file(&f).unwrap();
        let m = checked.model.expect("the examples pass check");
        let files = match dandori::asl::build(&m) {
            Ok(files) => files,
            Err(d) if d.iter().all(|x| x.code == "E050") => {
                // a workflow for the platforms that run the user's own code, as it says
                eprintln!("{}: not for Step Functions ({} task(s) with no way to call them there: E050)", rel(&f), d.len());
                continue;
            }
            Err(d) => panic!("{} does not build: {}", rel(&f), d[0].en),
        };
        let dir = scratch(&key(&f));
        let def = dir.join(&files[0].0);
        std::fs::write(&def, &files[0].1).unwrap();

        let validator = root().join("tools/node_modules/.bin/asl-validator");
        if validator.exists() {
            let out = Command::new(&validator).arg("--json-path").arg(&def).output().unwrap();
            assert!(out.status.success(), "asl-validator rejects {}:\n{}{}", rel(&f), String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        } else {
            eprintln!("SKIP: asl-validator is not in tools/node_modules; the definition is not validated");
        }

        let scenarios = dandori::scenarios::generate(&m);
        assert!(!scenarios.is_empty(), "no scenarios for {}", rel(&f));
        for (i, sc) in scenarios.iter().enumerate() {
            let reference = dandori::interp::run(&m, sc, View::Asl).unwrap_or_else(|e| panic!("{} scenario {}: {e}", rel(&f), i + 1));
            let answers: Vec<Value> = reference["steps"].as_array().unwrap().iter().filter(|s| s.get("call").is_some()).map(|s| s["answer"].clone()).collect();
            let run_file = dir.join(format!("run-{:03}.json", i + 1));
            std::fs::write(&run_file, serde_json::to_string(&json!({ "input": sc["input"], "answers": answers })).unwrap()).unwrap();
            let out = Command::new("node").arg(root().join("tools/asl-run.mjs")).arg(&def).arg(&run_file).arg("test").output().unwrap();
            assert!(out.status.success(), "{} scenario {}: the runner failed:\n{}", rel(&f), i + 1, String::from_utf8_lossy(&out.stderr));
            let got: Value = serde_json::from_slice(&out.stdout).unwrap();
            if norm(&got) != norm(&reference) {
                panic!(
                    "{} scenario {}: the state machine and the reference interpreter differ\n--- scenario\n{}\n--- reference\n{}\n--- state machine\n{}",
                    rel(&f),
                    i + 1,
                    serde_json::to_string_pretty(sc).unwrap(),
                    serde_json::to_string_pretty(&reference).unwrap(),
                    serde_json::to_string_pretty(&got).unwrap()
                );
            }
            compared += 1;
        }
    }
    eprintln!("compared {compared} scenario(s) between the state machines and the reference interpreter");
}

/// The last image of LocalStack's community edition, which starts without an account; the
/// images after it ask for an auth token.
const LOCALSTACK_IMAGE: &str = "localstack/localstack:4.14.0";

fn localstack_ready() -> Result<(), String> {
    let out = Command::new("docker").args(["image", "inspect", "--format", "{{.Id}}", LOCALSTACK_IMAGE]).output().map_err(|_| "docker is missing".to_string())?;
    if out.status.success() {
        return Ok(());
    }
    let err = String::from_utf8_lossy(&out.stderr);
    if err.contains("No such image") {
        Err(format!("{LOCALSTACK_IMAGE} is not pulled; run `docker pull {LOCALSTACK_IMAGE}`"))
    } else {
        Err(format!("docker does not answer: {}", err.trim()))
    }
}

/// A retry wait on LocalStack is timed from the history (it really waits), from the end of one
/// try to the start of the next. It counts as the reference's wait when it is no shorter, give
/// or take the 50 ms of the history's clock, and longer by less than 0.9 s or a quarter of it,
/// whichever is more, which a loaded machine adds, and a wait of a backoff one step off (twice
/// or half as long) does not. The intervals themselves are the definition's, which
/// tools/asl-run.mjs reads to the second. Gives how much longer the longest was.
fn timed(reference: &Value, got: &mut Value) -> f64 {
    let mut over: f64 = 0.0;
    let (Some(want), Some(steps)) = (reference["steps"].as_array(), got["steps"].as_array_mut()) else {
        return over;
    };
    for (w, g) in want.iter().zip(steps.iter_mut()) {
        if let (Some(a), Some(b)) = (w.get("retry_wait").and_then(|x| x.as_f64()), g.get("retry_wait").and_then(|x| x.as_f64())) {
            if b >= a - 0.05 && b < a + (a / 4.0).max(0.9) {
                over = over.max(b - a);
                g["retry_wait"] = w["retry_wait"].clone();
            }
        }
    }
    over
}

/// Whether a run differs from the reference in its retry waits alone, which came out longer.
fn late(reference: &Value, got: &Value) -> bool {
    let (Some(want), Some(steps)) = (reference["steps"].as_array(), got["steps"].as_array()) else {
        return false;
    };
    if want.len() != steps.len() || norm(&reference["end"]) != norm(&got["end"]) {
        return false;
    }
    let mut late = false;
    for (w, g) in want.iter().zip(steps) {
        if norm(w) == norm(g) {
            continue;
        }
        match (w.get("retry_wait").and_then(|x| x.as_f64()), g.get("retry_wait").and_then(|x| x.as_f64())) {
            (Some(a), Some(b)) if b > a => late = true,
            _ => return false,
        }
    }
    late
}

/// The flows' runs on LocalStack, by tools/localstack/run.mjs: each flow's `runs` and `http`.
fn on_localstack(dir: &Path, name: &str, flows: &[Value]) -> Vec<Value> {
    let spec = dir.join(format!("{name}.json"));
    let results = dir.join(format!("{name}-results.json"));
    std::fs::write(&spec, serde_json::to_string(&json!({ "image": LOCALSTACK_IMAGE, "flows": flows })).unwrap()).unwrap();
    let out = Command::new("node").arg(root().join("tools/localstack/run.mjs")).arg(&spec).arg(&results).output().unwrap();
    assert!(out.status.success(), "the LocalStack runner failed:\n{}", String::from_utf8_lossy(&out.stderr));
    let got: Value = serde_json::from_str(&std::fs::read_to_string(&results).unwrap()).unwrap();
    got["flows"].as_array().unwrap().clone()
}

/// The same state machines on LocalStack's Step Functions (its JSONata is the Java one), with
/// every call's answer mocked; tools/localstack/run.mjs says how. A run whose calls and end are
/// the reference's, but a retry wait of which came out longer than `timed` takes, is played once
/// more in a new container: at the start of the whole test suite, the machine is loaded enough to
/// wake LocalStack's threads a second late.
#[test]
fn localstack_runs_as_the_reference_says() {
    need_rulec!();
    need_node!();
    if let Err(why) = localstack_ready() {
        eprintln!("SKIP: {why}");
        return;
    }
    let mut flows = Vec::new();
    let mut compared = Vec::new();
    for f in runnable_on(Platform::StepFunctions) {
        let (_, checked) = dandori::check::check_file(&f).unwrap();
        let m = checked.model.expect("the flows pass check");
        let files = match dandori::asl::build(&m) {
            Ok(files) => files,
            Err(d) if d.iter().all(|x| x.code == "E050") => continue,
            Err(d) => panic!("{} does not build: {}", rel(&f), d[0].en),
        };
        let definition: Value = serde_json::from_str(&files[0].1).unwrap();
        // every run is an execution named run-<n>, which the idempotency keys carry
        let mut runs = Vec::new();
        let mut references = Vec::new();
        for (i, mut sc) in dandori::scenarios::generate(&m).into_iter().enumerate() {
            let id = format!("run-{}", i + 1);
            sc["execution"] = json!(id);
            let reference = dandori::interp::run(&m, &sc, View::Asl).unwrap_or_else(|e| panic!("{} scenario {}: {e}", rel(&f), i + 1));
            let answers: Vec<Value> = reference["steps"].as_array().unwrap().iter().filter(|s| s.get("call").is_some()).map(|s| s["answer"].clone()).collect();
            runs.push(json!({ "id": id, "input": sc["input"], "answers": answers }));
            references.push((sc, reference));
        }
        flows.push(json!({ "name": key(&f), "definition": definition, "runs": runs }));
        compared.push((f, references));
    }
    let dir = scratch("localstack");
    let mut got = on_localstack(&dir, "spec", &flows);
    let mut over = vec![0.0f64; flows.len()];
    let mut again: Vec<Vec<usize>> = vec![Vec::new(); flows.len()];
    for (fi, (_, references)) in compared.iter().enumerate() {
        for (ri, (_, reference)) in references.iter().enumerate() {
            let run = &mut got[fi]["runs"][ri];
            over[fi] = over[fi].max(timed(reference, run));
            if late(reference, run) {
                again[fi].push(ri);
            }
        }
    }
    if again.iter().any(|a| !a.is_empty()) {
        let replay: Vec<Value> = flows
            .iter()
            .zip(&again)
            .filter(|(_, a)| !a.is_empty())
            .map(|(flow, a)| {
                let mut flow = flow.clone();
                flow["runs"] = json!(a.iter().map(|ri| flow["runs"][*ri].clone()).collect::<Vec<_>>());
                flow
            })
            .collect();
        let replayed = on_localstack(&dir, "again", &replay);
        let mut next = replayed.iter();
        for (fi, a) in again.iter().enumerate().filter(|(_, a)| !a.is_empty()) {
            let played = next.next().unwrap();
            for (k, ri) in a.iter().enumerate() {
                let reference = &compared[fi].1[*ri].1;
                let mut run = played["runs"][k].clone();
                over[fi] = over[fi].max(timed(reference, &mut run));
                eprintln!("{} run {}: played again, since one of its retry waits came out longer than the reference's by more than the allowance", rel(&compared[fi].0), ri + 1);
                got[fi]["runs"][*ri] = run;
            }
        }
    }
    let mut total = 0;
    for (fi, ((f, references), g)) in compared.iter().zip(&got).enumerate() {
        compare("the state machine on LocalStack", f, references, g["runs"].as_array().unwrap());
        let http = g["http"].as_u64().unwrap_or(0);
        eprintln!(
            "{}: compared {} run(s) on LocalStack's Step Functions{}; the retry waits took at most {:.3} s longer than the reference's",
            rel(f),
            references.len(),
            if http > 0 { format!(" ({http} HTTP Task(s) with another resource)") } else { String::new() },
            over[fi]
        );
        total += references.len();
    }
    eprintln!("compared {total} scenario(s) between the state machines on LocalStack and the reference interpreter");
}

fn temporal_available() -> bool {
    root().join("tools/temporal/node_modules/@temporalio/testing").exists() && Command::new("node").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// What the stand-in transport needs to know to answer as the other side would: the HTTP
/// status of each declared error, and the exception of an AWS API's.
fn transport_spec(m: &Model, p: Platform) -> (Vec<Value>, Vec<Value>) {
    let mut http = Vec::new();
    let mut aws = Vec::new();
    for t in &m.tasks {
        match t.via(p) {
            Some(Via::Http { method, url, .. }) => {
                let errors: serde_json::Map<String, Value> = t.errors.iter().filter_map(|e| e.status.map(|s| (e.name.clone(), json!(s)))).collect();
                http.push(json!({ "method": method, "url": url, "errors": errors }));
            }
            Some(Via::Aws { service, action }) => {
                let errors: serde_json::Map<String, Value> = t.errors.iter().map(|e| (e.name.clone(), json!(e.exception.clone().unwrap_or_else(|| e.name.clone())))).collect();
                aws.push(json!({ "api": format!("{service}:{action}"), "errors": errors, "keyParam": t.key_param }));
            }
            _ => {}
        }
    }
    (http, aws)
}

/// The scenarios' runs for a platform: the input and the answers, and the reference's
/// trace; `keep` says which runs the platform's test environment can play. With `named`, the
/// runs go at once, each as an execution named `run-<n>`, which the keys carry; else each is
/// named `test`, one after another.
fn plays(m: &Model, view: View, named: bool, keep: impl Fn(&[CallInfo]) -> bool) -> (Vec<Value>, Vec<(Value, Value)>, usize) {
    let mut runs = Vec::new();
    let mut references = Vec::new();
    let mut left_out = 0;
    for mut sc in dandori::scenarios::generate(m) {
        let id = format!("run-{}", runs.len() + 1);
        if named {
            sc["execution"] = json!(id);
        }
        let (reference, calls) = dandori::interp::run_traced(m, &sc, view).unwrap();
        if !keep(&calls) {
            left_out += 1;
            continue;
        }
        let answers: Vec<Value> = reference["steps"].as_array().unwrap().iter().filter(|s| s.get("call").is_some()).map(|s| s["answer"].clone()).collect();
        let script: Vec<Value> = answers
            .iter()
            .map(|a| {
                if a.get("ok").is_some() {
                    json!({ "ok": a["ok"] })
                } else if a.get("cancel").is_some() {
                    json!({ "cancel": true })
                } else {
                    json!({ "error": a["error"] })
                }
            })
            .collect();
        // the answers that are an event's (Temporal), by their place among the answers
        let events: serde_json::Map<String, Value> = reference["steps"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|s| s.get("call").is_some())
            .enumerate()
            .filter_map(|(i, s)| s["call"].get("event").map(|e| (i.to_string(), e.clone())))
            .collect();
        runs.push(json!({ "id": id, "input": sc["input"], "answers": script, "events": events }));
        references.push((sc, reference));
    }
    (runs, references, left_out)
}

fn compare(what: &str, f: &Path, references: &[(Value, Value)], got: &[Value]) {
    assert_eq!(got.len(), references.len());
    for (i, ((sc, reference), g)) in references.iter().zip(got).enumerate() {
        if norm(g) != norm(reference) {
            panic!(
                "{} run {}: {what} and the reference interpreter differ\n--- scenario\n{}\n--- reference\n{}\n--- {what}\n{}",
                rel(f),
                i + 1,
                serde_json::to_string_pretty(sc).unwrap(),
                serde_json::to_string_pretty(reference).unwrap(),
                serde_json::to_string_pretty(g).unwrap()
            );
        }
    }
}

#[test]
fn temporal_runs_as_the_reference_says() {
    need_rulec!();
    if !temporal_available() {
        eprintln!("SKIP: tools/temporal/node_modules is missing; run `npm install --prefix tools/temporal`");
        return;
    }
    temporal_all(None);
}

/// The Python of tools/temporal-python/.venv, where the Temporal SDK for Python is.
fn temporal_python() -> Option<PathBuf> {
    let py = root().join("tools/temporal-python/.venv/bin/python");
    py.exists().then_some(py)
}

#[test]
fn temporal_python_runs_as_the_reference_says() {
    need_rulec!();
    let python = match temporal_python() {
        Some(p) => p,
        None => {
            eprintln!("SKIP: tools/temporal-python/.venv is missing; make it as tools/temporal-python/requirements.txt says");
            return;
        }
    };
    temporal_all(Some(python));
}

/// Every flow on Temporal at once, each with a runner and a dev server of its own: the
/// TypeScript build, or with `python`, the Python build.
fn temporal_all(python: Option<PathBuf>) {
    let _turn = heavy();
    std::thread::scope(|scope| {
        for f in runnable_on(Platform::Temporal) {
            let python = python.clone();
            scope.spawn(move || temporal_one(&f, python.as_deref(), None));
        }
    });
}

/// The workflow in one language, and its activities in the other: the workers of the build that
/// runs the workflow run nothing else, and the other build's runner serves the activities on the
/// same server (`--serve`). The flows whose rules say `local` are left out: a local activity runs
/// in the worker of the workflow, so it is in the workflow's language.
#[test]
fn temporal_activities_run_in_the_other_language() {
    need_rulec!();
    let python = match (temporal_available(), temporal_python()) {
        (true, Some(p)) => p,
        _ => {
            eprintln!("SKIP: tools/temporal/node_modules or tools/temporal-python/.venv is missing");
            return;
        }
    };
    let _turn = heavy();
    let flows: Vec<PathBuf> = runnable_on(Platform::Temporal)
        .into_iter()
        .filter(|f| {
            let (_, checked) = dandori::check::check_file(f).unwrap();
            let m = checked.model.expect("the examples pass check");
            if m.rules.iter().any(|r| r.local) {
                eprintln!("{}: not with the other language's activities (a rule that says `local` runs in the workflow's worker)", rel(f));
                return false;
            }
            if m.tasks.iter().any(|t| t.event) {
                eprintln!("{}: not with the other language's activities (an event goes to the workflow, not to an activity)", rel(f));
                return false;
            }
            true
        })
        .collect();
    std::thread::scope(|scope| {
        for f in &flows {
            let python = &python;
            scope.spawn(move || temporal_one(f, None, Some(python)));
            scope.spawn(move || temporal_one(f, Some(python), Some(Path::new("node"))));
        }
    });
}

/// One flow on Temporal: the TypeScript build, or with `python`, the Python build. With
/// `activities_by` (the other language's program: Python, or Node), the activities are that
/// language's, served by its runner.
fn temporal_one(f: &Path, python: Option<&Path>, activities_by: Option<&Path>) {
    let (_, checked) = dandori::check::check_file(f).unwrap();
    let m = checked.model.expect("the examples pass check");
    let lang = match (python.is_some(), activities_by.is_some()) {
        (false, false) => "",
        (true, false) => "python-",
        (false, true) => "ts-py-",
        (true, true) => "py-ts-",
    };
    let dir = scratch(&format!("temporal-{lang}{}", key(f)));
    let mut files = match python {
        Some(_) => dandori::temporal_py::build(&m),
        None => dandori::temporal::build(&m),
    }
    .unwrap_or_else(|d| panic!("{} does not build: {}", rel(f), d[0].en));
    if activities_by.is_some() {
        // the other language's build, whose runner serves the activities
        files.extend(match python {
            Some(_) => dandori::temporal::build(&m),
            None => dandori::temporal_py::build(&m),
        }.unwrap());
    }
    for (name, text) in &files {
        let p = dir.join(name);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, text).unwrap();
    }
    let p = Platform::Temporal;
    // the stand-ins of the tasks the user writes are methods of the language that serves the activities
    let activities_in_python = python.is_some() != activities_by.is_some();
    let method = |t: &str| if activities_in_python { dandori::temporal_py::method(t) } else { dandori::render::ident(t) };
    let own: Vec<Value> = m.tasks.iter().filter(|t| t.via(p) == Some(Via::Own)).map(|t| json!({ "name": t.name, "method": method(&t.name), "callback": t.callback })).collect();
    let rules: Vec<String> = m.rules.iter().map(|r| dandori::render::rule_activity(&r.name)).collect();
    let children: Vec<Value> = m.tasks.iter().filter_map(|t| t.workflow.as_ref().map(|w| json!({ "type": w, "queue": t.queue }))).collect();
    let mut queues: Vec<String> = m.tasks.iter().filter_map(|t| t.queue.clone()).collect();
    queues.sort();
    queues.dedup();
    let (http, aws) = transport_spec(&m, p);
    // on a real server an activity times out as the scenario says, and a callback that gets no answer
    let (runs, references, _) = plays(&m, View::Temporal, true, |_| true);
    let runs_file = dir.join("runs.json");
    let results_file = dir.join("results.json");
    // the tasks that hand on a callback's id, by the name of their proxy in the workflow's code
    let callbacks: Vec<String> = m
        .tasks
        .iter()
        .filter(|t| t.callback && !t.is_child(p))
        .map(|t| if python.is_some() { format!("dd_task_{}", dandori::render::ident(&t.name)) } else { dandori::render::ident(&format!("task_{}", t.name)) })
        .collect();
    let spec = json!({ "workflow": dandori::render::ident(&m.name), "own": own, "rules": rules, "children": children, "queues": queues, "http": http, "aws": aws, "callbacks": callbacks, "runs": runs });
    std::fs::write(&runs_file, serde_json::to_string(&spec).unwrap()).unwrap();
    let histories = dir.join("histories");
    let _ = std::fs::remove_dir_all(&histories);
    let mut run = match python {
        Some(py) => {
            let mut c = Command::new(py);
            c.arg(root().join("tools/temporal-python/run.py")).arg(dir.join(dandori::temporal_py::package(&m)));
            c
        }
        None => {
            let mut c = Command::new("node");
            c.arg(root().join("tools/temporal/run.mjs")).arg(dir.join(dandori::render::ident(&m.name)));
            c
        }
    };
    run.arg(&runs_file).arg(&results_file).arg(&histories);
    if let Some(by) = activities_by {
        let serve: Vec<String> = match python {
            Some(_) => vec![by.display().to_string(), root().join("tools/temporal/run.mjs").display().to_string(), "--serve".into(), dir.join(dandori::render::ident(&m.name)).display().to_string()],
            None => vec![by.display().to_string(), root().join("tools/temporal-python/run.py").display().to_string(), "--serve".into(), dir.join(dandori::temporal_py::package(&m)).display().to_string()],
        };
        let mut serve = serve;
        serve.push(runs_file.display().to_string());
        run.env("DANDORI_ACTIVITIES_BY", serde_json::to_string(&serve).unwrap());
    }
    let out = run.output().unwrap();
    let what = match (python.is_some(), activities_by.is_some()) {
        (false, false) => "the Temporal workflow",
        (true, false) => "the Temporal workflow in Python",
        (false, true) => "the Temporal workflow with its activities in Python",
        (true, true) => "the Temporal workflow in Python with its activities in TypeScript",
    };
    assert!(out.status.success(), "{}: the runner of {what} failed:\n{}", rel(f), String::from_utf8_lossy(&out.stderr));
    let mut got: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(&results_file).unwrap()).unwrap();
    // what the query dandori.status and the search attribute DandoriCases say of the cases at the end
    for (i, (g, (sc, _))) in got.iter_mut().zip(&references).enumerate() {
        let o = g.as_object_mut().unwrap();
        let (cases, shown) = (o.remove("cases").unwrap_or(Value::Null), o.remove("shown").unwrap_or(Value::Null));
        let want = dandori::interp::cases_at_end(&m, sc, View::Temporal).unwrap();
        assert_eq!(cases, Value::Object(want.clone()), "{} run {}: the query says the cases are {cases}, and the reference {}", rel(f), i + 1, Value::Object(want.clone()));
        // the workflow writes the search attribute when a case moves; before that, it has none
        let listed: Vec<Value> = want.iter().filter(|(_, s)| !s.is_null()).map(|(c, s)| json!(format!("{c}={}", s.as_str().unwrap_or_default()))).collect();
        let listed = if listed.is_empty() { Value::Null } else { Value::Array(listed) };
        assert_eq!(shown, listed, "{} run {}: the search attribute says {shown}, and the reference's cases {listed}", rel(f), i + 1);
    }
    compare(what, f, &references, &got);
    // a rule that says `local` runs as a local activity: a marker in the history, never an activity task
    let locals: Vec<String> = m.rules.iter().filter(|r| r.local).map(|r| dandori::render::rule_activity(&r.name)).collect();
    if !locals.is_empty() {
        let mut markers = 0;
        for e in std::fs::read_dir(&histories).unwrap() {
            let path = e.unwrap().path();
            if path.file_name().unwrap() == "spec.json" {
                continue;
            }
            let h: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            for ev in h["events"].as_array().unwrap() {
                if let Some(name) = ev["activityTaskScheduledEventAttributes"]["activityType"]["name"].as_str() {
                    assert!(!locals.iter().any(|l| l == name), "{}: the rule activity {name} says `local`, and ran as an activity task ({})", rel(f), path.display());
                }
                if ev["markerRecordedEventAttributes"]["markerName"] == "core_local_activity" {
                    markers += 1;
                }
            }
        }
        assert!(markers > 0, "{}: no run called a rule that says `local` as a local activity", rel(f));
    }
    let how = match (python.is_some(), activities_by.is_some()) {
        (false, false) => "",
        (true, false) => " in Python",
        (false, true) => ", the activities in Python,",
        (true, true) => " in Python, the activities in TypeScript,",
    };
    eprintln!("{}: compared {} run(s) on Temporal{how} (the dev server), with the query and the search attribute, and replayed each", rel(f), references.len());
    // with DANDORI_BLESS, the history of the run with the most calls is kept for the replay test
    if activities_by.is_none() && std::env::var("DANDORI_BLESS").is_ok() && RECORDED.iter().any(|r| rel(f) == *r) {
        let longest = references.iter().enumerate().max_by_key(|(_, (_, r))| r["steps"].as_array().map(|a| a.len()).unwrap_or(0)).map(|(i, _)| i).unwrap();
        let keep = recorded_dir(&m, python.is_some());
        let _ = std::fs::remove_dir_all(&keep);
        std::fs::create_dir_all(&keep).unwrap();
        // the run's history, and the histories of the runs that went on from it (Continue-As-New)
        let id = format!("run-{}", longest + 1);
        for e in std::fs::read_dir(&histories).unwrap() {
            let name = e.unwrap().file_name().into_string().unwrap();
            if name == format!("{id}.json") || (name.starts_with(&format!("{id}.")) && name[id.len() + 1..].trim_end_matches(".json").parse::<u32>().is_ok()) {
                std::fs::write(keep.join(&name), neutral(&std::fs::read_to_string(histories.join(&name)).unwrap())).unwrap();
            }
        }
        std::fs::copy(histories.join("spec.json"), keep.join("spec.json")).unwrap();
    }
}

/// A kept history is made neutral of the machine it was recorded on: the workers' identities and
/// sticky task queues (`<pid>@<host>`) and the paths in stack traces (this tree, the temporary
/// directory) name it. The replay reads none of them.
fn neutral(text: &str) -> String {
    let mut t = text.to_string();
    let tmp = std::env::temp_dir().display().to_string();
    let tmp = tmp.trim_end_matches('/');
    t = t.replace(&format!("/private{tmp}/"), "/tmp/").replace(&format!("{tmp}/"), "/tmp/");
    t = t.replace(&format!("{}/", root().display()), "/work/dandori/");
    if let Ok(out) = Command::new("hostname").output() {
        let host = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !host.is_empty() {
            t = t.replace(&format!("@{host}"), "@localhost");
        }
    }
    t
}

/// Worker Deployment Versioning, through the generated worker, on the dev server: two builds of
/// tests/versions/approvals.flow whose code differs in one text, A and B, as versions of one
/// deployment. A run that starts on A and waits for its approval while B becomes the current
/// version ends on A's code, and so does its second round, which starts in a new run
/// (Continue-As-New); a run that starts after runs on B's. The build id is a hash of the code:
/// the same for the same code, and another for B.
#[test]
fn temporal_worker_versioning_keeps_a_run_on_its_build() {
    need_rulec!();
    if !temporal_available() {
        eprintln!("SKIP: tools/temporal/node_modules is missing; run `npm install --prefix tools/temporal`");
        return;
    }
    let _turn = heavy();
    let text = std::fs::read_to_string(root().join("tests/versions/approvals.flow")).unwrap();
    let changed = text.replace("さんが申込 {申込.id} を承認しました\")", "さんが申込 {申込.id} を承認しました（新しいビルド）\")");
    assert_ne!(text, changed, "tests/versions/approvals.flow no longer has the text the test changes");
    let dir = scratch("versions");
    let python = temporal_python();
    for (lang, py) in [("TypeScript", None), ("Python", python.as_ref())] {
        if lang == "Python" && py.is_none() {
            eprintln!("SKIP: tools/temporal-python/.venv is missing; Worker Deployment Versioning is not tried in Python");
            continue;
        }
        let mut built = Vec::new();
        for (name, flow) in [("a", &text), ("b", &changed), ("again", &text)] {
            let d = dir.join(format!("{}-{name}", lang.to_lowercase()));
            std::fs::create_dir_all(&d).unwrap();
            let f = d.join("approvals.flow");
            std::fs::write(&f, flow).unwrap();
            let (_, checked) = dandori::check::check_file(&f).unwrap();
            let m = checked.model.unwrap();
            let files = if py.is_some() { dandori::temporal_py::build(&m) } else { dandori::temporal::build(&m) }.unwrap();
            for (n, t) in &files {
                let p = d.join(n);
                std::fs::create_dir_all(p.parent().unwrap()).unwrap();
                std::fs::write(&p, t).unwrap();
            }
            let worker = files.iter().find(|(n, _)| n.ends_with("/worker.ts") || n.ends_with("/worker.py")).unwrap();
            let id = worker.1.lines().find(|l| l.contains("BUILD_ID =")).unwrap().split('"').nth(1).unwrap().to_string();
            built.push((d.join(dandori::render::ident(&m.name)), id));
        }
        assert_eq!(built[0].1, built[2].1, "the same code has two build ids");
        assert_ne!(built[0].1, built[1].1, "two builds of different code have one build id");
        let results = dir.join(format!("{}-results.json", lang.to_lowercase()));
        let out = match py {
            Some(py) => Command::new(py).arg(root().join("tools/temporal-python/versions.py")).arg(&built[0].0).arg(&built[1].0).arg(&results).output().unwrap(),
            None => Command::new("node").arg(root().join("tools/temporal/versions.mjs")).arg(&built[0].0).arg(&built[1].0).arg(&results).output().unwrap(),
        };
        assert!(out.status.success(), "the Worker Deployment Versioning run in {lang} failed:\n{}", String::from_utf8_lossy(&out.stderr));
        let got: Value = serde_json::from_str(&std::fs::read_to_string(&results).unwrap()).unwrap();
        let a_says = json!(["a さんが申込 申込-1 を承認しました", "a さんが申込 申込-1 を承認しました"]);
        let b_says = json!(["b さんが申込 申込-1 を承認しました（新しいビルド）", "b さんが申込 申込-1 を承認しました（新しいビルド）"]);
        assert_eq!(got["runs"], json!({ "run-a": 2, "run-b": 2 }), "{lang}: the second round of each did not start in a new run: {}", got["runs"]);
        assert_eq!(got["notified"]["run-a"], a_says, "{lang}: the run that started on A did not end on A's code: {}", got["notified"]);
        assert_eq!(got["notified"]["run-b"], b_says, "{lang}: the run that started on B did not run B's code: {}", got["notified"]);
        eprintln!("{lang}: a run pinned to build {} ended on it, in the run it went on in too, after {} became the current version", built[0].1, built[1].1);
    }
}

/// A workflow that runs another `.flow` as its child (`flow "<path>"`, tests/children), both as
/// dandori writes them, on one Temporal server: the parent's worker and the child's, in one
/// language and in the two crossed. Nothing stands in for the child. Each run must end as the
/// reference interpreter says, given the child's end, which the reference interpreter also
/// decides, from the input the parent passes it, as the answer of the parent's call.
#[test]
fn temporal_runs_a_flow_as_its_child() {
    need_rulec!();
    let python = temporal_python();
    if !temporal_available() || python.is_none() {
        eprintln!("SKIP: tools/temporal/node_modules or tools/temporal-python/.venv is missing");
        return;
    }
    let python = python.unwrap();
    let f = root().join("tests/children/受付.flow");
    let (_, checked) = dandori::check::check_file(&f).unwrap();
    let pm = checked.model.expect("the flow passes check");
    let task = pm.tasks.iter().find(|t| t.flow.is_some()).expect("a task runs a .flow");
    let cm = task.flow.as_ref().unwrap().model.clone();
    // a run for each kind of application, and what the reference says each ends with
    let kinds = match &pm.inputs[0].1 {
        dandori::model::Ty::Record(r) => match pm.field_ty(*r, "区分") {
            Some(dandori::model::Ty::Enum(e)) => pm.enums[*e].values.clone(),
            _ => panic!("the application has a kind"),
        },
        _ => panic!("the input is an application"),
    };
    let mut runs = Vec::new();
    let mut references = Vec::new();
    for (i, kind) in kinds.iter().enumerate() {
        let input = json!({ "申込": { "id": format!("申込-{}", i + 1), "額": 1000 * (i + 1), "区分": kind } });
        // what the parent passes the child: the arguments of its call, whatever the call answers
        let probe = dandori::interp::run(&pm, &json!({ "input": input, "answers": [{ "error": "failure" }] }), View::Temporal).unwrap();
        let args = probe["steps"][0]["call"]["args"].clone();
        let child_end = dandori::interp::run(&cm, &json!({ "input": args, "answers": [] }), View::Temporal).unwrap()["end"].clone();
        let answer = match (&child_end["succeed"], child_end["fail"]["error"].as_str()) {
            (out, None) if !out.is_null() => json!({ "ok": out }),
            (_, Some(e)) if task.error(e).is_some() => json!({ "error": e }),
            _ => json!({ "error": "failure" }),
        };
        let reference = dandori::interp::run(&pm, &json!({ "input": input, "answers": [answer] }), View::Temporal).unwrap();
        runs.push(json!({ "id": format!("run-{}", i + 1), "input": input }));
        references.push(json!({ "end": reference["end"] }));
    }
    let dir = scratch("children");
    let runs_file = dir.join("runs.json");
    std::fs::write(&runs_file, serde_json::to_string(&runs).unwrap()).unwrap();
    // both flows, in both languages
    let mut built = std::collections::BTreeMap::new();
    for (lang, m) in [("ts", &pm), ("ts", &cm), ("py", &pm), ("py", &cm)] {
        let out = dir.join(lang).join(dandori::render::ident(&m.name));
        let files = if lang == "py" { dandori::temporal_py::build(m) } else { dandori::temporal::build(m) }.unwrap();
        for (name, text) in &files {
            let p = dir.join(lang).join(name);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, text).unwrap();
        }
        let pkg = if lang == "py" { dir.join(lang).join(dandori::temporal_py::package(m)) } else { out };
        built.insert((lang, m.name.clone()), pkg);
    }
    let node_runner = root().join("tools/temporal/children.mjs");
    let py_runner = root().join("tools/temporal-python/children.py");
    let lang = |l: &str| if l == "py" { "Python" } else { "TypeScript" };
    let _turn = heavy();
    for (parent, child) in [("ts", "ts"), ("py", "py"), ("ts", "py"), ("py", "ts")] {
        let (p_dir, c_dir) = (&built[&(parent, pm.name.clone())], &built[&(child, cm.name.clone())]);
        let results = dir.join(format!("results-{parent}-{child}.json"));
        let mut cmd = if parent == "py" {
            let mut c = Command::new(&python);
            c.arg(&py_runner);
            c
        } else {
            let mut c = Command::new("node");
            c.arg(&node_runner);
            c
        };
        cmd.arg(p_dir).arg(c_dir).arg(&runs_file).arg(&results);
        if parent != child {
            let serve: Vec<String> = if child == "py" {
                vec![python.display().to_string(), py_runner.display().to_string(), "--serve".into(), c_dir.display().to_string()]
            } else {
                vec!["node".into(), node_runner.display().to_string(), "--serve".into(), c_dir.display().to_string()]
            };
            cmd.env("DANDORI_CHILD_BY", serde_json::to_string(&serve).unwrap());
        }
        let out = cmd.output().unwrap();
        assert!(out.status.success(), "the parent in {} with the child in {} failed:\n{}", lang(parent), lang(child), String::from_utf8_lossy(&out.stderr));
        let got: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(&results).unwrap()).unwrap();
        assert_eq!(got.len(), references.len());
        for (i, (g, r)) in got.iter().zip(&references).enumerate() {
            assert_eq!(norm(g), norm(r), "run {} (the parent in {}, the child in {}) ends otherwise than the reference says", i + 1, lang(parent), lang(child));
        }
        eprintln!("{}: {} run(s) of the parent in {} with its child {} in {} on Temporal (the dev server) ended as the reference says", rel(&f), got.len(), lang(parent), task.flow.as_ref().unwrap().path, lang(child));
    }
}

/// The flows whose histories are kept in tests/histories: one run of each, the one with the most
/// calls, recorded by the Temporal runners with DANDORI_BLESS=1. The examples' are their versions
/// for Temporal. The run of the order goes on in new runs (Continue-As-New), and each of them is kept.
const RECORDED: [&str; 5] = [
    "examples/hotel/temporal/hotel.flow",
    "examples/fulfillment/temporal/fulfillment.flow",
    "examples/review/temporal/review.flow",
    "tests/flows/cancel.flow",
    "examples/order/temporal/order.flow",
];

fn recorded_dir(m: &Model, python: bool) -> PathBuf {
    root().join("tests/histories").join(if python { "python" } else { "typescript" }).join(dandori::render::ident(&m.name))
}

/// Whether an HTTP task's URL, with its `{placeholders}`, is the URL of a call.
fn url_matches(pattern: &str, url: &str) -> bool {
    // the literal parts between the placeholders
    let mut lits = Vec::new();
    let mut rest = pattern;
    while let Some(i) = rest.find('{') {
        lits.push(&rest[..i]);
        rest = &rest[rest[i..].find('}').map(|j| i + j + 1).unwrap_or(rest.len())..];
    }
    lits.push(rest);
    let Some(mut u) = url.strip_prefix(lits[0]) else { return false };
    for lit in &lits[1..] {
        // a placeholder's value is one segment of the path
        let seg = u.find(['/', '?']).unwrap_or(u.len());
        if lit.is_empty() {
            u = &u[seg..];
            continue;
        }
        match u.find(lit) {
            Some(i) if i <= seg => u = &u[i + lit.len()..],
            _ => return false,
        }
    }
    u.is_empty()
}

/// A value's pairs in a URL-encoded body or query, as the Transports write them: `a[b]=c`, and
/// nothing for none.
fn form_pairs(v: &Value, prefix: &str, out: &mut Vec<Value>) {
    match v {
        Value::Object(o) => {
            for (k, x) in o {
                let key = if prefix.is_empty() { k.clone() } else { format!("{prefix}[{k}]") };
                form_pairs(x, &key, out);
            }
        }
        Value::Null => out.push(json!([prefix, ""])),
        Value::String(s) => out.push(json!([prefix, s])),
        other => out.push(json!([prefix, other.to_string()])),
    }
}

/// The calls of the scenarios that go through a Transport — Lambda, HTTP, the AWS APIs — as
/// cases for tools/wire: the request, and what the stand-in on this machine answers, which
/// makes the Transport give back what the stand-in Transport of the runners gives for the
/// scenario's answer. A call the scenario times out or cancels during gets no answer, and is
/// left out; so is an AWS error that moto cannot be made to give.
fn wire_cases(m: &Model) -> Vec<Value> {
    let p = Platform::Temporal;
    let mut cases = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for sc in dandori::scenarios::generate(m) {
        let reference = dandori::interp::run(m, &sc, View::Temporal).unwrap();
        for st in reference["steps"].as_array().unwrap() {
            let (Some(call), Some(ans)) = (st.get("call"), st.get("answer")) else { continue };
            if ans.get("cancel").is_some() || ans.get("error") == Some(&json!("timeout")) {
                continue;
            }
            let kind = ans.get("error").and_then(|e| e.as_str());
            let case = if let Some(method) = call.get("http").and_then(|x| x.as_str()) {
                let url = call["url"].as_str().unwrap();
                let task = m.tasks.iter().find(|t| matches!(t.via(p), Some(Via::Http { method: tm, url: tu, .. }) if tm == method && url_matches(tu, url))).unwrap_or_else(|| panic!("no task sends {method} {url}"));
                let Some(Via::Http { form, .. }) = task.via(p) else { unreachable!() };
                let mut request = call.clone();
                request["form"] = json!(form);
                let reply = match kind {
                    None => json!({ "status": 200, "body": ans["ok"] }),
                    Some(k) => json!({ "status": task.errors.iter().find(|e| e.name == k).and_then(|e| e.status).unwrap_or(500), "body": "scripted" }),
                };
                json!({ "kind": "http", "request": request, "reply": reply })
            } else if let Some(fn_) = call.get("lambda").and_then(|x| x.as_str()) {
                let task = m.tasks.iter().find(|t| t.via(p) == Some(Via::Lambda(fn_)));
                let mut payload = call["payload"].clone();
                let reply = if task.map(|t| t.callback).unwrap_or(false) {
                    // a callback's submit hands on its id, and its answer comes later
                    payload["callback_id"] = json!("wire-callback");
                    json!({ "ok": null })
                } else {
                    match kind {
                        None => json!({ "ok": ans["ok"] }),
                        Some(k) => json!({ "error": if k == "failure" { "Dandori.Test.Failure" } else { k }, "message": "scripted" }),
                    }
                };
                json!({ "kind": "lambda", "fn": fn_, "payload": payload, "reply": reply })
            } else if let Some(api) = call.get("aws").and_then(|x| x.as_str()) {
                let (service, action) = api.split_once(':').unwrap();
                let task = m.tasks.iter().find(|t| t.via(p) == Some(Via::Aws { service, action })).unwrap();
                let mut input = call["args"].clone();
                let name_of = |arn_or_url: &str| arn_or_url.rsplit([':', '/']).next().unwrap().to_string();
                let (target, is_topic) = match api {
                    "sns:publish" => (name_of(input["TopicArn"].as_str().unwrap()), true),
                    "sqs:sendMessage" => (name_of(input["QueueUrl"].as_str().unwrap()), false),
                    _ => continue,
                };
                let listen = if is_topic { json!({ "topic": target, "queue": "wire-listen" }) } else { json!({ "queue": target }) };
                if task.callback {
                    input["MessageBody"]["callback_id"] = json!("wire-callback");
                }
                match kind {
                    // the topic or the queue is there, and a queue listens
                    _ if task.callback => json!({ "kind": "aws", "service": service, "action": action, "input": input, "topics": if is_topic { vec![target.clone()] } else { vec![] }, "queues": if is_topic { vec![] } else { vec![target.clone()] }, "listen": listen }),
                    None => json!({ "kind": "aws", "service": service, "action": action, "input": input, "topics": if is_topic { vec![target.clone()] } else { vec![] }, "queues": if is_topic { vec![] } else { vec![target.clone()] }, "listen": listen }),
                    // an error moto gives: the topic or the queue is not there
                    Some(k) => {
                        let exception = task.errors.iter().find(|e| e.name == k).and_then(|e| e.exception.clone());
                        match exception.as_deref() {
                            Some("NotFoundException") if is_topic => json!({ "kind": "aws", "service": service, "action": action, "input": input, "topics": [], "queues": [], "listen": null, "error": "NotFoundException" }),
                            Some("QueueDoesNotExist") if !is_topic => json!({ "kind": "aws", "service": service, "action": action, "input": input, "topics": [], "queues": [], "listen": null, "error": "QueueDoesNotExist" }),
                            _ => continue,
                        }
                    }
                }
            } else {
                continue;
            };
            if seen.insert(case.to_string()) {
                cases.push(case);
            }
        }
    }
    cases
}

/// The default Transports — TypeScript's (fetch, the AWS SDK for JavaScript) and Python's (the
/// standard library, boto3) — send the calls of every scenario to stand-ins on this machine
/// (tools/wire): an HTTP request arrives with the method, the path, the query, the headers and
/// the body the call has, JSON or URL-encoded; a Lambda invoke with the function and the
/// payload; an SNS message or an SQS message on moto, the AWS APIs' stand-in; and each gives
/// back what the runners' stand-in Transport gives for the scenario's answer, an AWS error by
/// the name the task declares. Nothing leaves the machine.
#[test]
fn default_transports_send_what_the_calls_say() {
    need_rulec!();
    need_node!();
    let wire = root().join("tools/wire");
    let python = wire.join(".venv/bin/python");
    if !wire.join("node_modules").exists() || !python.exists() {
        eprintln!("SKIP: tools/wire/node_modules or tools/wire/.venv is missing; see tools/wire/package.json and requirements.txt");
        return;
    }
    // moto, on a port nothing else holds
    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let mut moto = Command::new(wire.join(".venv/bin/moto_server")).arg("-p").arg(port.to_string()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().unwrap();
    let address = format!("http://127.0.0.1:{port}");
    for _ in 0..100 {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let mut checked = 0;
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for f in runnable() {
            let (_, got) = dandori::check::check_file(&f).unwrap();
            let m = got.model.expect("the flows pass check");
            let cases = wire_cases(&m);
            if cases.is_empty() {
                continue;
            }
            let dir = scratch(&format!("wire-{}", key(&f)));
            let (Ok(ts), Ok(py)) = (dandori::temporal::build(&m), dandori::temporal_py::build(&m)) else { continue };
            for (name, text) in ts.iter().chain(py.iter()) {
                let p = dir.join(name);
                std::fs::create_dir_all(p.parent().unwrap()).unwrap();
                std::fs::write(&p, text).unwrap();
            }
            let cases_file = dir.join("cases.json");
            std::fs::write(&cases_file, serde_json::to_string(&cases).unwrap()).unwrap();
            let mut by_lang = Vec::new();
            for (lang, out) in [
                ("TypeScript", Command::new("node").arg(wire.join("check.mjs")).arg(dir.join(dandori::render::ident(&m.name))).arg(&cases_file).arg(dir.join("ts.json")).arg(&address).output().unwrap()),
                ("Python", Command::new(&python).arg(wire.join("check.py")).arg(dir.join(dandori::temporal_py::package(&m))).arg(&cases_file).arg(dir.join("py.json")).arg(&address).output().unwrap()),
            ] {
                assert!(out.status.success(), "{}: the {lang} check failed:\n{}", rel(&f), String::from_utf8_lossy(&out.stderr));
                let results: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(dir.join(if lang == "Python" { "py.json" } else { "ts.json" })).unwrap()).unwrap();
                for (c, r) in cases.iter().zip(&results) {
                    let at = || format!("{} ({lang}): {}", rel(&f), serde_json::to_string(c).unwrap());
                    match c["kind"].as_str().unwrap() {
                        "http" => {
                            let req = &c["request"];
                            let got = &r["received"];
                            assert_eq!(got["method"], req["http"], "{}: the method", at());
                            let url = req["url"].as_str().unwrap();
                            let path = url.splitn(4, '/').nth(3).map(|p| format!("/{}", p.split('?').next().unwrap())).unwrap();
                            // the path arrives as the URL has it, percent-encoded where it is not ASCII
                            let arrived = got["path"].as_str().unwrap();
                            let decoded = String::from_utf8(arrived.split('%').enumerate().fold(Vec::new(), |mut b, (i, s)| {
                                if i == 0 {
                                    b.extend(s.bytes());
                                } else {
                                    b.push(u8::from_str_radix(&s[..2], 16).unwrap());
                                    b.extend(s[2..].bytes());
                                }
                                b
                            }))
                            .unwrap();
                            assert_eq!(decoded, path, "{}: the path", at());
                            let mut want_query = Vec::new();
                            if !req["query"].is_null() {
                                form_pairs(&req["query"], "", &mut want_query);
                            }
                            assert_eq!(got["query"], Value::Array(want_query), "{}: the query", at());
                            for (k, v) in req["headers"].as_object().into_iter().flatten() {
                                assert_eq!(got["headers"][k.to_lowercase()], *v, "{}: the header {k}", at());
                            }
                            assert_eq!(got["headers"]["x-dandori-check"], json!("wire"), "{}: the headers the options add", at());
                            let want_body = if req["body"].is_null() {
                                Value::Null
                            } else if req["form"] == json!(true) {
                                let mut p = Vec::new();
                                form_pairs(&req["body"], "", &mut p);
                                Value::Array(p)
                            } else {
                                assert_eq!(got["headers"]["content-type"], json!("application/json"), "{}: the body's type", at());
                                req["body"].clone()
                            };
                            assert_eq!(got["body"], want_body, "{}: the body", at());
                            assert_eq!(r["returned"], c["reply"], "{}: what the Transport gave back", at());
                        }
                        "lambda" => {
                            assert_eq!(r["received"]["fn"], c["fn"], "{}: the function", at());
                            assert_eq!(r["received"]["payload"], c["payload"], "{}: the payload", at());
                            assert!(r["received"]["invocationType"].is_null() || r["received"]["invocationType"] == json!("RequestResponse"), "{}: the invocation's type", at());
                            assert_eq!(r["returned"], c["reply"], "{}: what the Transport gave back", at());
                        }
                        _ => match c.get("error") {
                            Some(e) => assert_eq!(r["returned"]["error"], *e, "{}: the error's name ({})", at(), r["returned"]),
                            None => {
                                assert!(r["returned"]["ok"]["MessageId"].is_string(), "{}: no message id in {}", at(), r["returned"]);
                                let messages = r["messages"].as_array().unwrap();
                                assert_eq!(messages.len(), 1, "{}: the listening queue got {messages:?}", at());
                                let body = messages[0].as_str().unwrap();
                                let input = &c["input"];
                                if c["service"] == "sns" {
                                    assert_eq!(body, input["Message"].as_str().unwrap(), "{}: the message", at());
                                } else {
                                    // SQS takes the message as text: a JSON body goes as JSON text
                                    let want = &input["MessageBody"];
                                    let got: Value = if want.is_string() { json!(body) } else { serde_json::from_str(body).unwrap() };
                                    assert_eq!(&got, want, "{}: the message", at());
                                }
                            }
                        },
                    }
                }
                by_lang.push(results);
            }
            // the two languages send the same text, and what came back from moto has the same fields
            for (i, c) in cases.iter().enumerate() {
                let (ts, py) = (&by_lang[0][i], &by_lang[1][i]);
                let case = serde_json::to_string(c).unwrap();
                assert_eq!(ts["received"]["raw"], py["received"]["raw"], "{}: the two languages send different text: {case}", rel(&f));
                assert_eq!(ts["received"]["rawQuery"], py["received"]["rawQuery"], "{}: the two languages send different queries: {case}", rel(&f));
                assert_eq!(ts["messages"], py["messages"], "{}: the two languages send different messages: {case}", rel(&f));
                if c["kind"] == "aws" {
                    let keys = |r: &Value| r["returned"]["ok"].as_object().map(|o| o.keys().cloned().collect::<Vec<_>>());
                    assert_eq!(keys(ts), keys(py), "{}: the fields of moto's answer differ between the languages: {case}", rel(&f));
                }
            }
            eprintln!("{}: sent {} call(s) through the default Transport of TypeScript and of Python", rel(&f), cases.len());
            checked += cases.len();
        }
    }));
    let _ = moto.kill();
    if let Err(e) = outcome {
        std::panic::resume_unwind(e);
    }
    assert!(checked > 0, "no call went through a Transport");
}

/// The kept histories replay with the code dandori writes now: a change of the generator that
/// would make a running workflow of an unchanged `.flow` nondeterministic shows here. A change
/// that has to do so is made with DANDORI_BLESS=1, which records the histories anew; runs that
/// are going on need the version of the `.flow` raised, or Worker Deployment Versioning.
#[test]
fn temporal_replays_the_recorded_histories() {
    need_rulec!();
    let python = temporal_python();
    if !temporal_available() || python.is_none() {
        eprintln!("SKIP: tools/temporal/node_modules or tools/temporal-python/.venv is missing");
        return;
    }
    let mut replayed = 0;
    for r in RECORDED {
        let f = root().join(r);
        let (_, checked) = dandori::check::check_file(&f).unwrap();
        let m = checked.model.expect("the flows pass check");
        for py in [false, true] {
            let kept = recorded_dir(&m, py);
            if !kept.exists() {
                panic!("{r}: no histories kept in {}; record them with DANDORI_BLESS=1", rel(&kept));
            }
            let dir = scratch(&format!("replay-{}-{}", if py { "python" } else { "typescript" }, key(&f)));
            let files = if py { dandori::temporal_py::build(&m) } else { dandori::temporal::build(&m) }.unwrap();
            for (name, text) in &files {
                let p = dir.join(name);
                std::fs::create_dir_all(p.parent().unwrap()).unwrap();
                std::fs::write(&p, text).unwrap();
            }
            let results = dir.join("replayed.json");
            let out = if py {
                Command::new(python.as_ref().unwrap()).arg(root().join("tools/temporal-python/run.py")).arg("--replay").arg(dir.join(dandori::temporal_py::package(&m))).arg(&kept).arg(&results).output().unwrap()
            } else {
                Command::new("node").arg(root().join("tools/temporal/run.mjs")).arg("--replay").arg(dir.join(dandori::render::ident(&m.name))).arg(&kept).arg(&results).output().unwrap()
            };
            assert!(out.status.success(), "{r}: the replay failed:\n{}", String::from_utf8_lossy(&out.stderr));
            let got: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(&results).unwrap()).unwrap();
            for g in &got {
                assert!(g["error"].is_null(), "{r}: the kept history {} does not replay with the code dandori writes now ({}): {}", g["file"], if py { "Python" } else { "TypeScript" }, g["error"]);
                replayed += 1;
            }
        }
    }
    eprintln!("replayed {replayed} kept history(ies) with the code dandori writes now");
}

/// The TypeScript dandori writes — for Temporal, for durable functions, and Argo's caller —
/// passes `tsc --strict` (TypeScript 7 in tools/temporal), with the modules rulec generates for
/// the rules. The packages the default Transport loads when it needs them (the AWS SDK's clients,
/// the agents' SDKs) are declared as modules of any type.
#[test]
fn generated_typescript_type_checks() {
    need_rulec!();
    let tsc = root().join("tools/temporal/node_modules/.bin/tsc");
    if !tsc.exists() {
        eprintln!("SKIP: TypeScript is not in tools/temporal/node_modules; run `npm install --prefix tools/temporal`");
        return;
    }
    let types = root().join("tools/temporal/node_modules/@types");
    let shims = "declare module \"@anthropic-ai/sdk\";\ndeclare module \"openai\";\ndeclare module \"@openai/agents\";\ndeclare module \"@aws-sdk/*\";\n";
    let mut checked = 0;
    for f in runnable() {
        let (_, c) = dandori::check::check_file(&f).unwrap();
        let m = c.model.expect("the flows pass check");
        let builds = [
            ("temporal", dandori::temporal::build(&m), "tools/temporal/node_modules", false),
            ("durable", dandori::temporal::build_flavor(&m, dandori::temporal::Flavor::Durable), "tools/durable/node_modules", false),
            ("argo", dandori::argo::build(&m), "tools/temporal/node_modules", true),
        ];
        for (target, built, modules, argo) in builds {
            let files = match built {
                Ok(files) => files,
                Err(d) if d.iter().all(|x| x.code == "E050") => continue,
                Err(d) => panic!("{} does not build for {target}: {}", rel(&f), d[0].en),
            };
            let dir = scratch(&format!("tsc-{target}-{}", key(&f)));
            for (name, text) in &files {
                let p = dir.join(name);
                std::fs::create_dir_all(p.parent().unwrap()).unwrap();
                std::fs::write(&p, text).unwrap();
            }
            // the directory of the TypeScript: the workflow's package, or Argo's caller
            let code = if argo { dir.join("caller") } else { dir.join(dandori::render::ident(&m.name)) };
            let called: std::collections::BTreeSet<usize> = m.all_stmts().iter().filter_map(|s| match &s.kind { TK::Call { callee: Callee::Rule(r), .. } => Some(*r), _ => None }).collect();
            if target != "durable" {
                for r in &called {
                    let gen = Command::new(rulec_bin()).arg("gen").arg(&m.rules[*r].info.path).arg("--out").arg(code.join("rulec")).output().unwrap();
                    assert!(gen.status.success(), "rulec gen failed: {}", String::from_utf8_lossy(&gen.stderr));
                }
            }
            let link = code.join("node_modules");
            let _ = std::fs::remove_file(&link);
            std::os::unix::fs::symlink(root().join(modules), &link).unwrap();
            std::fs::write(code.join("shims.d.ts"), shims).unwrap();
            let tsconfig = json!({
                "compilerOptions": {
                    "strict": true, "noEmit": true, "module": "nodenext", "moduleResolution": "nodenext", "target": "es2022",
                    "skipLibCheck": true, "types": ["node"], "typeRoots": [types], "allowImportingTsExtensions": argo
                },
                "include": ["*.ts"]
            });
            std::fs::write(code.join("tsconfig.json"), serde_json::to_string_pretty(&tsconfig).unwrap()).unwrap();
            let out = Command::new(&tsc).arg("-p").arg(code.join("tsconfig.json")).output().unwrap();
            assert!(out.status.success(), "{}: the TypeScript for {target} does not type-check:\n{}{}", rel(&f), String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
            checked += 1;
        }
    }
    eprintln!("type-checked the TypeScript of {checked} build(s) with tsc --strict");
}

fn rulec_bin() -> String {
    std::env::var("DANDORI_RULEC").unwrap_or_else(|_| "rulec".into())
}

/// The code between a platform and a rule — the Lambda handler for Step Functions and the
/// activity for Temporal — answers every vector rulec generates for the rule as rulec says.
#[test]
fn rule_glue_answers_the_rulec_vectors() {
    need_rulec!();
    let python = Command::new("python3").arg("--version").output().map(|o| o.status.success()).unwrap_or(false);
    let node = Command::new("node").arg("--version").output().map(|o| o.status.success()).unwrap_or(false);
    for f in flows(&root().join("examples")) {
        let (_, checked) = dandori::check::check_file(&f).unwrap();
        let m = checked.model.expect("the examples pass check");
        let asl_files = dandori::asl::build(&m).unwrap_or_default();
        let ts_files = dandori::temporal::build(&m).unwrap();
        for r in &m.rules {
            let handler = asl_files.iter().find(|(n, _)| n.starts_with("lambda/") && n.contains(r.info.api["python"]["module"].as_str().unwrap_or("?")));
            let (hname, htext) = match handler {
                Some(h) => h,
                None => continue, // the rule is only read for its machine
            };
            let dir = scratch(&format!("glue-{}", dandori::render::ident(&r.name)));
            let gen = Command::new(rulec_bin()).arg("gen").arg(&r.info.path).arg("--out").arg(dir.join("rulec")).output().unwrap();
            assert!(gen.status.success(), "rulec gen failed: {}", String::from_utf8_lossy(&gen.stderr));
            let vectors = Command::new(rulec_bin()).arg("vectors").arg(&r.info.path).output().unwrap();
            std::fs::write(dir.join("vectors.jsonl"), &vectors.stdout).unwrap();
            let expected: Vec<Value> = String::from_utf8_lossy(&vectors.stdout).lines().map(|l| serde_json::from_str::<Value>(l).unwrap()["out"].clone()).collect();

            if python {
                let hfile = dir.join(Path::new(hname).file_name().unwrap());
                std::fs::write(&hfile, htext).unwrap();
                let module = hfile.file_stem().unwrap().to_string_lossy().to_string();
                let script = format!(
                    "import json, sys\nsys.path.insert(0, 'rulec/python'); sys.path.insert(0, '.')\nfrom {module} import handler\nprint(json.dumps([handler(json.loads(l)['in'], None) for l in open('vectors.jsonl')], ensure_ascii=False))\n"
                );
                let out = Command::new("python3").arg("-c").arg(script).current_dir(&dir).output().unwrap();
                assert!(out.status.success(), "the Lambda handler failed: {}", String::from_utf8_lossy(&out.stderr));
                let got: Vec<Value> = serde_json::from_slice(&out.stdout).unwrap();
                assert_eq!(norm(&json!(got)), norm(&json!(expected)), "the Lambda handler of {} differs from rulec's vectors", r.name);
            } else {
                eprintln!("SKIP: python3 is missing; the Lambda handler of {} is not run", r.name);
            }

            if node {
                let rules_ts = ts_files.iter().find(|(n, _)| n.ends_with("/rules.ts")).map(|(_, t)| t.clone()).unwrap();
                // Node runs TypeScript by stripping the types, and wants the extension on an import
                let text = rules_ts.replace("\";\n", "\";\n").replace("/typescript/", "/typescript/").lines().map(|l| {
                    if l.starts_with("import ") && l.contains("./rulec/typescript/") {
                        l.replacen("\";", ".ts\";", 1)
                    } else {
                        l.to_string()
                    }
                }).collect::<Vec<_>>().join("\n");
                std::fs::write(dir.join("rules.ts"), text).unwrap();
                let activity = dandori::render::rule_activity(&r.name);
                let script = format!(
                    "import fs from 'node:fs';\nconst {{ rules }} = await import('./rules.ts');\nconst out = [];\nfor (const l of fs.readFileSync('vectors.jsonl', 'utf8').trim().split('\\n')) out.push(await rules[{}](JSON.parse(l).in));\nconsole.log(JSON.stringify(out));\n",
                    serde_json::to_string(&activity).unwrap()
                );
                std::fs::write(dir.join("check.mjs"), script).unwrap();
                let out = Command::new("node").arg("--no-warnings").arg("check.mjs").current_dir(&dir).output().unwrap();
                assert!(out.status.success(), "rules.ts failed: {}", String::from_utf8_lossy(&out.stderr));
                let got: Value = serde_json::from_slice(&out.stdout).unwrap();
                assert_eq!(norm(&got), norm(&json!(expected)), "the Temporal activity of {} differs from rulec's vectors", r.name);
            } else {
                eprintln!("SKIP: node is missing; rules.ts is not run");
            }
        }
    }
}

/// The Python of tools/pydantic-graph/.venv, where pydantic-graph is.
fn pydantic_graph_python() -> Option<PathBuf> {
    let py = root().join("tools/pydantic-graph/.venv/bin/python");
    py.exists().then_some(py)
}

#[test]
fn pydantic_graph_runs_as_the_reference_says() {
    need_rulec!();
    let python = match pydantic_graph_python() {
        Some(p) => p,
        None => {
            eprintln!("SKIP: tools/pydantic-graph/.venv is missing; make it as tools/pydantic-graph/requirements.txt says");
            return;
        }
    };
    for f in runnable_on(Platform::Graph) {
        let (_, checked) = dandori::check::check_file(&f).unwrap();
        let m = checked.model.expect("the examples pass check");
        let dir = scratch(&format!("pydantic-graph-{}", key(&f)));
        let files = match dandori::pydantic_graph::build(&m) {
            Ok(files) => files,
            Err(d) if d.iter().all(|x| x.code == "E050") => {
                eprintln!("{}: not for pydantic-graph ({}: E050)", rel(&f), d[0].en);
                continue;
            }
            Err(d) => panic!("{} does not build: {}", rel(&f), d[0].en),
        };
        for (name, text) in &files {
            let p = dir.join(name);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, text).unwrap();
        }
        let p = Platform::Graph;
        let own: Vec<Value> = m.tasks.iter().filter(|t| t.via(p) == Some(Via::Own)).map(|t| json!({ "name": t.name, "method": dandori::temporal_py::method(&t.name), "callback": t.callback })).collect();
        let rules: Vec<Value> = m.rules.iter().map(|r| json!({ "fn": dandori::render::rule_activity(&r.name), "name": r.name })).collect();
        let (http, aws) = transport_spec(&m, p);
        // a task's own timeout cannot be scripted; a callback's can, by not answering it
        let (runs, references, left_out) = plays(&m, View::Graph, false, |calls| calls.iter().all(|c| c.kind.as_deref() != Some("timeout") || c.callback));
        let runs_file = dir.join("runs.json");
        let results_file = dir.join("results.json");
        let spec = json!({ "own": own, "rules": rules, "http": http, "aws": aws, "runs": runs });
        std::fs::write(&runs_file, serde_json::to_string(&spec).unwrap()).unwrap();
        let out = Command::new(&python)
            .arg(root().join("tools/pydantic-graph/run.py"))
            .arg(dir.join(dandori::pydantic_graph::package(&m)))
            .arg(&runs_file)
            .arg(&results_file)
            .output()
            .unwrap();
        assert!(out.status.success(), "{}: the pydantic-graph runner failed:\n{}", rel(&f), String::from_utf8_lossy(&out.stderr));
        let got: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(&results_file).unwrap()).unwrap();
        compare("the pydantic-graph graph", &f, &references, &got);
        eprintln!("{}: compared {} run(s) on pydantic-graph; left out {left_out} with a task's timeout", rel(&f), references.len());
    }
}

/// The agent call of the default Transport — io.ts, which Temporal, durable functions and
/// Argo's caller share, and io.py, which Temporal's Python SDK and pydantic-graph share — run
/// without sending anything anywhere (tools/agents). OpenAI's agents run with OpenAI's Agents
/// SDK and a scripted model in place of OpenAI's; for every agent call of the scenarios, the
/// model must be asked what Step Functions asks for the same call: the model, the instructions,
/// the arguments as the same JSON text, and the answer's schema, with no settings of the SDK's
/// own and no tools. Claude's agents run with Anthropic's SDK against a stand-in of the Messages
/// API on this machine, which must get the very request Step Functions sends, once. For either,
/// an error status must fail the call at once: neither SDK's client may retry by itself, since
/// the workflow retries as the task's `retry` says. The Transport must give back what the model
/// answered, and throw when the model refuses.
#[test]
fn agents_sdk_is_asked_what_step_functions_asks() {
    need_rulec!();
    let node = root().join("tools/agents/node_modules/@openai/agents").exists()
        && root().join("tools/agents/node_modules/@anthropic-ai/sdk").exists()
        && Command::new("node").arg("--version").output().map(|o| o.status.success()).unwrap_or(false);
    let python = root().join("tools/agents/.venv/bin/python");
    if !node {
        eprintln!("SKIP: tools/agents/node_modules is missing; run `npm install --prefix tools/agents`");
    }
    if !python.exists() {
        eprintln!("SKIP: tools/agents/.venv is missing; make it as tools/agents/requirements.txt says");
    }
    for f in runnable() {
        let (_, checked) = dandori::check::check_file(&f).unwrap();
        let m = checked.model.expect("the examples pass check");
        if !m.tasks.iter().any(|t| matches!(t.via(Platform::Temporal), Some(Via::Agent { .. }))) {
            continue;
        }
        // every agent call the scenarios make, as the Transport gets it and as Step Functions sends it
        let mut cases = Vec::new();
        let mut expected = Vec::new();
        let mut refused: Vec<String> = Vec::new();
        for sc in dandori::scenarios::generate(&m) {
            // every call, in order, as each view has it: the Transport's, and Step Functions' HTTP Task
            let calls = |view: View| -> Vec<(Value, Value)> {
                let r = dandori::interp::run(&m, &sc, view).unwrap();
                r["steps"].as_array().unwrap().iter().filter(|s| s.get("call").is_some()).map(|s| (s["call"].clone(), s["answer"].clone())).collect()
            };
            for ((call, answer), (sent, _)) in calls(View::Temporal).into_iter().zip(calls(View::Asl)) {
                if call.get("agent").is_none() {
                    continue;
                }
                let name = call["agent"].as_str().unwrap().to_string();
                let first = !refused.contains(&name);
                if first {
                    refused.push(name);
                }
                if let Some(url) = call["url"].as_str() {
                    // a server of Open Responses gets what Step Functions sends it, once
                    let path = format!("{}/responses", url.split("://").nth(1).and_then(|r| r.find('/').map(|i| &r[i..])).unwrap_or("").trim_end_matches('/'));
                    let asked = json!([{ "method": "POST", "path": path, "body": sent["body"] }]);
                    if let Some(ok) = answer.get("ok") {
                        cases.push(json!({ "call": call, "text": json!({ "answer": ok }).to_string() }));
                        expected.push(json!({ "answer": { "answer": ok }, "asked": asked }));
                    }
                    if first {
                        cases.push(json!({ "call": call, "refusal": "I can't help with that." }));
                        expected.push(json!({ "error": "AgentStopped", "asked": asked }));
                        cases.push(json!({ "call": call, "status": 500 }));
                        expected.push(json!({ "error": "AgentHttpError", "asked": asked }));
                    }
                    continue;
                }
                if call["provider"] == json!("claude") {
                    // the one request the stand-in of the Messages API must get
                    let asked = json!([{ "method": "POST", "path": "/v1/messages", "version": sent["headers"]["anthropic-version"], "body": sent["body"] }]);
                    if let Some(ok) = answer.get("ok") {
                        cases.push(json!({ "call": call, "text": json!({ "answer": ok }).to_string() }));
                        expected.push(json!({ "answer": { "answer": ok }, "asked": asked }));
                    }
                    if first {
                        cases.push(json!({ "call": call, "refusal": "I can't help with that." }));
                        expected.push(json!({ "error": "AgentStopped", "asked": asked }));
                        cases.push(json!({ "call": call, "status": 500 }));
                        expected.push(json!({ "error": "InternalServerError", "asked": asked }));
                    }
                    continue;
                }
                let asked = json!({
                    "models": [call["model"]],
                    "calls": 1,
                    "instructions": call["instructions"],
                    "input": sent["body"]["input"],
                    "outputType": { "type": "json_schema", "name": "answer", "strict": true, "schema": call["schema"] },
                    "modelSettings": {},
                    "tools": []
                });
                if let Some(ok) = answer.get("ok") {
                    cases.push(json!({ "call": call, "text": json!({ "answer": ok }).to_string() }));
                    expected.push(json!({ "answer": { "answer": ok }, "asked": asked }));
                }
                if first {
                    cases.push(json!({ "call": call, "refusal": "I can't help with that." }));
                    expected.push(json!({ "error": "ModelRefusalError", "asked": asked }));
                    // the client the Transport makes sends the request once
                    cases.push(json!({ "call": call, "status": 500 }));
                    expected.push(json!({ "error": "InternalServerError", "asked": [{ "method": "POST", "path": "/v1/responses" }] }));
                }
            }
        }
        let dir = scratch(&format!("agents-{}", key(&f)));
        let cases_file = dir.join("cases.json");
        std::fs::write(&cases_file, serde_json::to_string(&cases).unwrap()).unwrap();
        // the Agents SDKs hand the model the input as a user message; its text is what is compared
        let read = |file: &Path| -> Vec<Value> {
            let mut got: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
            for g in got.iter_mut() {
                if let Some(text) = g["asked"]["input"][0].get("content").cloned() {
                    g["asked"]["input"] = text;
                }
            }
            got
        };
        let check = |what: &str, got: Vec<Value>| {
            assert_eq!(got.len(), expected.len(), "{}: {what} answered {} case(s) of {}", rel(&f), got.len(), expected.len());
            for (i, (g, e)) in got.iter().zip(&expected).enumerate() {
                assert!(
                    norm(g) == norm(e),
                    "{} case {}: {what} differs\n--- case\n{}\n--- expected\n{}\n--- got\n{}",
                    rel(&f),
                    i + 1,
                    serde_json::to_string_pretty(&cases[i]).unwrap(),
                    serde_json::to_string_pretty(e).unwrap(),
                    serde_json::to_string_pretty(g).unwrap()
                );
            }
        };
        if node {
            let files = dandori::temporal::build(&m).unwrap();
            let io = files.iter().find(|(n, _)| n.ends_with("/io.ts")).unwrap();
            let io_file = dir.join("io.ts");
            std::fs::write(&io_file, &io.1).unwrap();
            let results = dir.join("results-ts.json");
            let out = Command::new("node").arg("--no-warnings").arg(root().join("tools/agents/check.mjs")).arg(&io_file).arg(&cases_file).arg(&results).output().unwrap();
            assert!(out.status.success(), "{}: tools/agents/check.mjs failed:\n{}", rel(&f), String::from_utf8_lossy(&out.stderr));
            check("the default Transport in TypeScript", read(&results));
            eprintln!("{}: the default Transport in TypeScript asked what Step Functions asks in {} case(s)", rel(&f), cases.len());
        }
        if python.exists() {
            let files = dandori::temporal_py::build(&m).unwrap();
            let io = files.iter().find(|(n, _)| n.ends_with("/io.py")).unwrap();
            let io_file = dir.join("io.py");
            std::fs::write(&io_file, &io.1).unwrap();
            let results = dir.join("results-py.json");
            let out = Command::new(&python).arg(root().join("tools/agents/check.py")).arg(&io_file).arg(&cases_file).arg(&results).output().unwrap();
            assert!(out.status.success(), "{}: tools/agents/check.py failed:\n{}", rel(&f), String::from_utf8_lossy(&out.stderr));
            check("the default Transport in Python", read(&results));
            eprintln!("{}: the default Transport in Python asked what Step Functions asks in {} case(s)", rel(&f), cases.len());
        }
    }
}

/// The agents on a server of Open Responses (`url`), sent for real to Ollama on this machine by
/// the default Transport of both languages: for each such agent, the arguments of the first call
/// the scenarios answer, with the model swapped for one Ollama has. Each answer must fit the
/// task's type. Skipped when no Ollama answers at DANDORI_OLLAMA (http://127.0.0.1:11434) or it
/// has no model; DANDORI_OLLAMA_MODEL picks the model, else the smallest there is.
#[test]
fn open_responses_agents_answer_on_ollama() {
    need_rulec!();
    let base = std::env::var("DANDORI_OLLAMA").unwrap_or_else(|_| "http://127.0.0.1:11434".into());
    let get = |path: &str| -> Option<Value> {
        let out = Command::new("curl").args(["-s", "-m", "3", &format!("{base}{path}")]).output().ok()?;
        serde_json::from_slice(&out.stdout).ok()
    };
    let (Some(version), Some(tags)) = (get("/api/version"), get("/api/tags")) else {
        eprintln!("SKIP: no Ollama answers at {base}; the agents on a server of Open Responses are not sent to a real one");
        return;
    };
    let mut models: Vec<(u64, String)> = tags["models"].as_array().into_iter().flatten().filter_map(|m| Some((m["size"].as_u64().unwrap_or(u64::MAX), m["name"].as_str()?.to_string()))).collect();
    models.sort();
    let model = match std::env::var("DANDORI_OLLAMA_MODEL").ok().or(models.first().map(|m| m.1.clone())) {
        Some(m) => m,
        None => {
            eprintln!("SKIP: Ollama at {base} has no model; pull one, or name one with DANDORI_OLLAMA_MODEL");
            return;
        }
    };
    // Ollama keeps a model it loads for five minutes, more than 8 GB for one of 8B parameters: the
    // test lets go of the model when it is done, unless the model was loaded before
    struct LetGo(Option<(String, String)>);
    impl Drop for LetGo {
        fn drop(&mut self) {
            if let Some((base, model)) = &self.0 {
                let _ = Command::new("curl").args(["-s", "-m", "10", &format!("{base}/api/generate"), "-d", &json!({ "model": model, "keep_alive": 0 }).to_string()]).output();
            }
        }
    }
    let loaded = get("/api/ps").is_some_and(|ps| ps["models"].as_array().into_iter().flatten().any(|x| x["name"].as_str() == Some(model.as_str())));
    let _let_go = LetGo((!loaded).then(|| (base.clone(), model.clone())));
    let node = root().join("tools/agents/node_modules/@openai/agents").exists();
    let python = root().join("tools/agents/.venv/bin/python");
    let mut sent = 0;
    for f in runnable() {
        let (_, checked) = dandori::check::check_file(&f).unwrap();
        let m = checked.model.expect("the flows pass check");
        // the first answered call of each agent on a server of Open Responses, sent to Ollama
        let mut cases = Vec::new();
        let mut tasks = Vec::new();
        for sc in dandori::scenarios::generate(&m) {
            let r = dandori::interp::run(&m, &sc, View::Temporal).unwrap();
            for s in r["steps"].as_array().unwrap() {
                let (call, answer) = (&s["call"], &s["answer"]);
                let Some(name) = call["agent"].as_str() else { continue };
                if call.get("url").is_none() || answer.get("ok").is_none() || tasks.iter().any(|t: &&dandori::model::TaskDef| t.name == name) {
                    continue;
                }
                let mut live = call.clone();
                live["url"] = json!(format!("{base}/v1"));
                live["model"] = json!(model);
                cases.push(json!({ "call": live, "live": true }));
                tasks.push(m.tasks.iter().find(|t| t.name == name).unwrap());
            }
        }
        if cases.is_empty() {
            continue;
        }
        let dir = scratch(&format!("ollama-{}", key(&f)));
        let cases_file = dir.join("cases.json");
        std::fs::write(&cases_file, serde_json::to_string(&cases).unwrap()).unwrap();
        let check = |lang: &str, results: &Path| {
            let got: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(results).unwrap()).unwrap();
            for (g, t) in got.iter().zip(&tasks) {
                let answer = &g["answer"]["answer"];
                assert!(
                    g.get("error").is_none() && dandori::render::value_fits(&m, answer, t.result.as_ref().unwrap(), t.result_range),
                    "{}: `{}` on Ollama ({model}), from the default Transport in {lang}, answered what its type does not take: {}",
                    rel(&f),
                    t.name,
                    serde_json::to_string(g).unwrap()
                );
            }
        };
        if node {
            let files = dandori::temporal::build(&m).unwrap();
            let io_file = dir.join("io.ts");
            std::fs::write(&io_file, &files.iter().find(|(n, _)| n.ends_with("/io.ts")).unwrap().1).unwrap();
            let results = dir.join("results-ts.json");
            let out = Command::new("node").arg("--no-warnings").arg(root().join("tools/agents/check.mjs")).arg(&io_file).arg(&cases_file).arg(&results).output().unwrap();
            assert!(out.status.success(), "{}: tools/agents/check.mjs failed:\n{}", rel(&f), String::from_utf8_lossy(&out.stderr));
            check("TypeScript", &results);
            sent += cases.len();
        }
        if python.exists() {
            let files = dandori::temporal_py::build(&m).unwrap();
            let io_file = dir.join("io.py");
            std::fs::write(&io_file, &files.iter().find(|(n, _)| n.ends_with("/io.py")).unwrap().1).unwrap();
            let results = dir.join("results-py.json");
            let out = Command::new(&python).arg(root().join("tools/agents/check.py")).arg(&io_file).arg(&cases_file).arg(&results).output().unwrap();
            assert!(out.status.success(), "{}: tools/agents/check.py failed:\n{}", rel(&f), String::from_utf8_lossy(&out.stderr));
            check("Python", &results);
            sent += cases.len();
        }
        eprintln!("{}: {} agent(s) on a server of Open Responses answered from Ollama {} ({model}) as their types say, from the default Transport of TypeScript and of Python", rel(&f), cases.len(), version["version"].as_str().unwrap_or("?"));
    }
    assert!(sent > 0, "no agent on a server of Open Responses was sent to Ollama");
}

/// rules.py of the Python builds — the activities for Temporal and the functions for
/// pydantic-graph — answers every vector rulec generates for each rule the flow calls.
#[test]
fn python_rules_answer_the_rulec_vectors() {
    need_rulec!();
    let python = match temporal_python() {
        Some(p) => p,
        None => {
            eprintln!("SKIP: tools/temporal-python/.venv is missing; rules.py is not run");
            return;
        }
    };
    for f in flows(&root().join("examples")) {
        let (_, checked) = dandori::check::check_file(&f).unwrap();
        let m = checked.model.expect("the examples pass check");
        // a version of an example written for one platform may not build for the other
        for (label, built) in [("Temporal", dandori::temporal_py::build(&m)), ("pydantic-graph", dandori::pydantic_graph::build(&m))] {
            let Ok(files) = built else { continue };
            let rules_py = match files.iter().find(|(n, _)| n.ends_with("/rules.py")) {
                Some((_, t)) => t.clone(),
                None => continue,
            };
            let dir = scratch(&format!("glue-python-{}-{}", label, key(&f)));
            let pkg = dir.join("glue");
            std::fs::create_dir_all(&pkg).unwrap();
            std::fs::write(pkg.join("__init__.py"), "").unwrap();
            std::fs::write(pkg.join("rules.py"), &rules_py).unwrap();
            let mut called: Vec<usize> = m.all_stmts().iter().filter_map(|s| match &s.kind { TK::Call { callee: Callee::Rule(r), .. } => Some(*r), _ => None }).collect();
            called.sort();
            called.dedup();
            for r in &called {
                let gen = Command::new(rulec_bin()).arg("gen").arg(&m.rules[*r].info.path).arg("--out").arg(pkg.join("rulec")).output().unwrap();
                assert!(gen.status.success(), "rulec gen failed: {}", String::from_utf8_lossy(&gen.stderr));
            }
            for r in &called {
                let ru = &m.rules[*r];
                let vectors = Command::new(rulec_bin()).arg("vectors").arg(&ru.info.path).output().unwrap();
                std::fs::write(dir.join("vectors.jsonl"), &vectors.stdout).unwrap();
                let expected: Vec<Value> = String::from_utf8_lossy(&vectors.stdout).lines().map(|l| serde_json::from_str::<Value>(l).unwrap()["out"].clone()).collect();
                let script = format!(
                    "import asyncio, json, sys\nsys.path.insert(0, '.')\nimport glue.rules as R\nf = getattr(R, {})\nasync def main():\n    return [await f(json.loads(l)['in']) for l in open('vectors.jsonl', encoding='utf-8')]\nprint(json.dumps(asyncio.run(main()), ensure_ascii=False))\n",
                    serde_json::to_string(&dandori::render::rule_activity(&ru.name)).unwrap()
                );
                let out = Command::new(&python).arg("-c").arg(script).current_dir(&dir).output().unwrap();
                assert!(out.status.success(), "rules.py failed: {}", String::from_utf8_lossy(&out.stderr));
                let got: Value = serde_json::from_slice(&out.stdout).unwrap();
                assert_eq!(norm(&got), norm(&json!(expected)), "rules.py for {label} of {} differs from rulec's vectors", ru.name);
                eprintln!("{}: rules.py for {label} answers the {} vector(s) of {} as rulec does", rel(&f), expected.len(), ru.name);
            }
        }
    }
}

fn durable_available() -> bool {
    root().join("tools/durable/node_modules/@aws/durable-execution-sdk-js-testing").exists() && Command::new("node").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

#[test]
fn durable_runs_as_the_reference_says() {
    need_rulec!();
    if !durable_available() {
        eprintln!("SKIP: tools/durable/node_modules is missing; run `npm install --prefix tools/durable`");
        return;
    }
    // every flow at once, taking its turn with the other tests that start many processes
    let _turn = heavy();
    std::thread::scope(|scope| {
        for f in runnable_on(Platform::Durable) {
            scope.spawn(move || durable_one(&f));
        }
    });
}

fn durable_one(f: &Path) {
    {
        let f = f.to_path_buf();
        let (_, checked) = dandori::check::check_file(&f).unwrap();
        let m = checked.model.expect("the examples pass check");
        let dir = scratch(&format!("durable-{}", key(&f)));
        let files = match dandori::temporal::build_flavor(&m, dandori::temporal::Flavor::Durable) {
            Ok(files) => files,
            Err(d) if d.iter().all(|x| x.code == "E050") => {
                eprintln!("{}: not for Lambda durable functions ({}: E050)", rel(&f), d[0].en);
                return;
            }
            Err(d) => panic!("{} does not build: {}", rel(&f), d[0].en),
        };
        for (name, text) in &files {
            let p = dir.join(name);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, text).unwrap();
        }
        // the Lambda functions the handler invokes for the rules are the ones Step Functions calls
        if let Ok(asl) = dandori::asl::build(&m) {
            for (name, text) in asl.iter().filter(|(n, _)| n.starts_with("lambda/")) {
                let same = files.iter().find(|(n, _)| n.ends_with(&format!("/{name}"))).map(|(_, t)| t);
                assert_eq!(same, Some(text), "{}: the durable build's {name} differs from the Step Functions build's", rel(&f));
            }
        }
        let p = Platform::Durable;
        let own: Vec<Value> = m.tasks.iter().filter(|t| t.via(p) == Some(Via::Own)).map(|t| json!({ "name": t.name, "method": dandori::render::ident(&t.name), "callback": t.callback })).collect();
        let mut rules: Vec<String> = Vec::new();
        for s in m.all_stmts() {
            if let TK::Call { callee: Callee::Rule(r), .. } = &s.kind {
                let arn = m.rules[*r].lambda.clone().unwrap_or_default();
                if !rules.contains(&arn) {
                    rules.push(arn);
                }
            }
        }
        let children: Vec<String> = m.tasks.iter().filter_map(|t| t.durable_function.clone()).collect();
        let (http, aws) = transport_spec(&m, p);
        // the local runner does not time a callback out, so the runs with a timeout are left out
        let (runs, references, left_out) = plays(&m, View::Durable, false, |calls| calls.iter().all(|c| c.kind.as_deref() != Some("timeout")));
        let runs_file = dir.join("runs.json");
        let results_file = dir.join("results.json");
        let spec = json!({ "own": own, "rules": rules, "children": children, "http": http, "aws": aws, "runs": runs });
        std::fs::write(&runs_file, serde_json::to_string(&spec).unwrap()).unwrap();
        let out = Command::new("node")
            .arg(root().join("tools/durable/run.mjs"))
            .arg(dir.join(dandori::render::ident(&m.name)))
            .arg(&runs_file)
            .arg(&results_file)
            .output()
            .unwrap();
        assert!(out.status.success(), "{}: the durable runner failed:\n{}", rel(&f), String::from_utf8_lossy(&out.stderr));
        let got: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(&results_file).unwrap()).unwrap();
        compare("the durable handler", &f, &references, &got);
        eprintln!("{}: compared {} run(s) on Lambda durable functions; left out {left_out} with a timeout", rel(&f), references.len());
    }
}

fn argo_ready() -> Result<(), String> {
    let argo = std::env::var("DANDORI_ARGO").unwrap_or_else(|_| "argo".into());
    if !Command::new(&argo).arg("version").output().map(|o| o.status.success()).unwrap_or(false) {
        return Err("the argo command is missing; put it on the PATH or in DANDORI_ARGO".into());
    }
    let mock = Command::new("kubectl").args(["--context", "kind-dandori", "-n", "argo", "get", "deploy", "dandori-mock"]).output();
    if !mock.map(|o| o.status.success()).unwrap_or(false) {
        return Err("the kind cluster dandori is not set up; run tools/argo/setup.sh".into());
    }
    Ok(())
}

/// On Argo Workflows the workflows run on a real controller, in the kind cluster that
/// tools/argo/setup.sh sets up; this test says SKIP when it is not there.
/// A flow on its way through Argo: the reference's runs, and the runner playing them.
struct ArgoFlow {
    f: PathBuf,
    references: Vec<(Value, Value)>,
    left_out: usize,
    /// the run played again with real pods
    real: usize,
    bound: u64,
    results: PathBuf,
    runner: std::process::Child,
}

/// Make a flow ready for Argo (its scenarios, the reference's runs, the build) and start the
/// runner on it; None for a flow that is not for Argo.
fn ready_on_argo(f: PathBuf) -> Option<ArgoFlow> {
    let (_, checked) = dandori::check::check_file(&f).unwrap();
    let m = checked.model.expect("the flows pass check");
    let files = match dandori::argo::build(&m) {
        Ok(files) => files,
        Err(d) if d.iter().all(|x| x.code == "E050") => {
            eprintln!("{}: not for Argo Workflows ({}: E050)", rel(&f), d[0].en);
            return None;
        }
        Err(d) => panic!("{} does not build for Argo: {}", rel(&f), d[0].en),
    };
    let dir = scratch(&format!("argo-{}", key(&f)));
    for (name, text) in &files {
        let p = dir.join(name);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, text).unwrap();
    }
    let p = Platform::Argo;
    let own: Vec<Value> = m
        .tasks
        .iter()
        .filter(|t| matches!(t.via(p), Some(Via::Image(_))))
        .map(|t| json!({ "name": t.name, "callback": t.callback, "declared": t.errors.iter().map(|e| e.name.clone()).collect::<Vec<_>>() }))
        .collect();
    let children: Vec<String> = m.tasks.iter().filter_map(|t| t.argo_template.clone()).collect();
    let (http, aws) = transport_spec(&m, p);
    // a task's own timeout cannot be scripted there; a callback's can, by the answer the runner sets
    let (runs, references, left_out) = plays(&m, View::Argo, false, |calls| calls.iter().all(|c| c.kind.as_deref() != Some("timeout") || c.callback));
    // the run played again with real pods: one through a declared error if there is one, else an error, else the first
    let errors = |r: &Value, declared: bool| r["answers"].as_array().unwrap().iter().any(|a| a.get("error").and_then(|e| e.as_str()).is_some_and(|e| !declared || (e != "failure" && e != "timeout")));
    let real = runs.iter().position(|r| errors(r, true)).or_else(|| runs.iter().position(|r| errors(r, false))).unwrap_or(0);
    let doc = dandori::argo::document(&m).unwrap();
    // the YAML the build writes says what the document the runner applies says
    let yaml = files.iter().find(|(n, _)| n.ends_with(".argo.yaml")).map(|(n, _)| dir.join(n)).expect("the build writes the WorkflowTemplate");
    let read = Command::new("kubectl").args(["--context", "kind-dandori", "-n", "argo", "create", "--dry-run=client", "-o", "json", "-f"]).arg(&yaml).output().unwrap();
    assert!(read.status.success(), "{}: kubectl does not read the YAML:\n{}", rel(&f), String::from_utf8_lossy(&read.stderr));
    let read: Value = serde_json::from_slice(&read.stdout).unwrap();
    assert!(read["spec"] == doc["spec"] && read["metadata"]["name"] == doc["metadata"]["name"], "{}: the YAML does not say what the document says", rel(&f));
    let runs_file = dir.join("runs.json");
    let results = dir.join("results.json");
    let spec = json!({ "template": doc, "own": own, "children": children, "http": http, "aws": aws, "runs": runs, "real": [real] });
    std::fs::write(&runs_file, serde_json::to_string(&spec).unwrap()).unwrap();
    let runner = Command::new("node")
        .arg(root().join("tools/argo/run.mjs"))
        .arg(&dir)
        .arg(&runs_file)
        .arg(&results)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    Some(ArgoFlow { f, references, left_out, real, bound: dandori::check::bound(&m, &dandori::check::ARGO_COST), results, runner })
}

#[test]
fn argo_runs_as_the_reference_says() {
    need_rulec!();
    if let Err(why) = argo_ready() {
        eprintln!("SKIP: {why}");
        return;
    }
    // every flow's runs go at the same time: the runner plays the pods (tools/argo/run.mjs), and
    // runs one of them again with real pods; each flow is made ready on a thread of its own
    let going: Vec<ArgoFlow> = std::thread::scope(|scope| {
        let ready: Vec<_> = runnable_on(Platform::Argo).into_iter().map(|f| scope.spawn(move || ready_on_argo(f))).collect();
        ready.into_iter().filter_map(|h| h.join().unwrap()).collect()
    });
    for flow in going {
        let f = &flow.f;
        let out = flow.runner.wait_with_output().unwrap();
        assert!(out.status.success(), "{}: the Argo runner failed:\n{}", rel(f), String::from_utf8_lossy(&out.stderr));
        let got: Value = serde_json::from_str(&std::fs::read_to_string(&flow.results).unwrap()).unwrap();
        // the nodes each run made, against what E040 reckons with
        let mut most = 0;
        let mut take = |g: &Value, what: &str| -> Value {
            let mut g = g.clone();
            let o = g.as_object_mut().unwrap();
            let nodes = o.remove("nodes").and_then(|n| n.as_u64()).expect("the runner counts the nodes");
            most = most.max(nodes);
            // a pod the platform could not run: the runner played the run again
            for e in o.remove("platform").and_then(|p| p.as_array().cloned()).unwrap_or_default() {
                eprintln!("{} {what}: played again, since the platform could not run a pod ({})", rel(f), e.as_str().unwrap_or_default());
            }
            g
        };
        let runs: Vec<Value> = got["runs"].as_array().unwrap().iter().enumerate().map(|(i, g)| take(g, &format!("run {}", i + 1))).collect();
        let real: Vec<Value> = got["real"].as_array().unwrap().iter().map(|g| take(g, &format!("run {} with real pods", flow.real + 1))).collect();
        assert!(most <= flow.bound, "{}: a run on Argo made {most} nodes, more than the {} E040 reckons with", rel(f), flow.bound);
        compare("the Argo workflow", f, &flow.references, &runs);
        compare(&format!("the Argo workflow with real pods (run {})", flow.real + 1), f, &flow.references[flow.real..flow.real + 1], &real);
        eprintln!(
            "{}: compared {} run(s) on Argo Workflows, and run {} again with real pods; left out {} with a task's timeout; at most {most} nodes in a run, where E040 reckons with {}",
            rel(f),
            flow.references.len(),
            flow.real + 1,
            flow.left_out,
            flow.bound
        );
    }
}
