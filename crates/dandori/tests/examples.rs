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
/// path has it, to look at one flow on a slow platform. A flow that needs a rulec the one at hand
/// is not is left out, with a SKIP line (`NAMED_ENUMS`).
fn runnable() -> Vec<PathBuf> {
    let mut out = flows(&root().join("examples"));
    out.extend(flows(&root().join("tests/flows")));
    if let Ok(part) = std::env::var("DANDORI_FLOW") {
        out.retain(|f| rel(f).contains(&part));
    }
    if !rulec_names_enums() {
        out.retain(|f| {
            let keep = !NAMED_ENUMS.contains(&rel(f).as_str());
            if !keep {
                eprintln!("SKIP: {}: it calls at its service a rule whose enum is a contract's, and this rulec's `rulec api` does not say what the service calls the values (`connect.enums`)", rel(f));
            }
            keep
        });
    }
    out
}

/// The flows that call, at its Connect service, a rule whose enum is a contract's (`import proto`):
/// with a rulec whose `rulec api` does not say what the service calls the enum's values
/// (`connect.enums`), `check` refuses them (E005, DESIGN 1.13), and the tests leave them out.
const NAMED_ENUMS: [&str; 1] = ["tests/flows/connect_rules_contract.flow"];

/// Whether the rulec at hand says, in `rulec api`, what a rule's service calls the values of its
/// enums (`connect.enums`): rulec 0.22.0 does, and 0.21.2 and before do not. The field is asked
/// for, not the version. Asked once.
fn rulec_names_enums() -> bool {
    static NAMES: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *NAMES.get_or_init(|| {
        let bin = std::env::var("DANDORI_RULEC").unwrap_or_else(|_| "rulec".into());
        let out = Command::new(&bin).arg("api").arg(root().join("examples/order/rules/urgency.rule")).output();
        out.ok().and_then(|o| serde_json::from_slice::<Value>(&o.stdout).ok()).is_some_and(|v| v["connect"]["enums"].is_array())
    })
}

/// The platforms a version of an example is written for, from its directory: `temporal/`,
/// `pydantic-graph/` or `argo/`, which only that platform's runners play. None for a version
/// written for AWS (`aws/`), which every platform plays, since the code dandori writes for each
/// makes its Lambda, HTTP and AWS calls; and for a flow beside the versions, which every
/// platform runs as it is (and for the flows of tests/flows). The Japanese twin of a version for
/// AWS (`.ja.flow`) is played by the two AWS platforms alone: the English one already tries those
/// calls everywhere, and every platform has its own Japanese version besides.
fn written_for(f: &Path) -> Option<Vec<Platform>> {
    let r = rel(f);
    if r.contains("/temporal/") {
        Some(vec![Platform::Temporal])
    } else if r.contains("/pydantic-graph/") {
        Some(vec![Platform::Graph])
    } else if r.contains("/argo/") {
        Some(vec![Platform::Argo])
    } else if r.contains("/aws/") && r.ends_with(".ja.flow") {
        Some(vec![Platform::StepFunctions, Platform::Durable])
    } else {
        None
    }
}

/// The flows a platform's runner plays: every flow but the versions written for other platforms.
fn runnable_on(p: Platform) -> Vec<PathBuf> {
    runnable().into_iter().filter(|f| written_for(f).is_none_or(|w| w.contains(&p))).collect()
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
                ("temporal-go", dandori::temporal_go::build(m)),
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

/// What `check` says of a flow that calls at its service a rule whose enum is a contract's, when rulec
/// does not say what the service calls the enum's values (`connect.enums`): E005 (DESIGN 1.13). What
/// such a rulec printed for the rule is kept in tests/fixtures/unnamed_enums/rulec.json, as `check`
/// asked for it from the repository's root (`rulec schema|certificate|api` on the rule, by rulec
/// 0.21.2, which did not name them), so the diagnostics are the same whatever rulec is at hand, and
/// need none. `DANDORI_BLESS=1` rewrites the golden files beside it.
#[test]
fn a_rulec_that_does_not_name_the_enums_has_a_contracts_refused() {
    let bless = std::env::var("DANDORI_BLESS").is_ok();
    let dir = root().join("tests/fixtures/unnamed_enums");
    let recorded: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("rulec.json")).unwrap()).unwrap();
    let bundle = std::rc::Rc::new(dandori::sources::Bundle::from_json(&recorded).unwrap());
    let path = "tests/flows/connect_rules_contract.flow";
    let text = std::fs::read_to_string(root().join(path)).unwrap();
    let mut failures = Vec::new();
    for (lang, tag) in [(Lang::En, "en"), (Lang::Ja, "ja")] {
        let sources = std::rc::Rc::new(dandori::sources::Playground { bundle: bundle.clone(), path: path.into(), text: text.clone(), lang });
        let checked = dandori::sources::with(sources, || dandori::check::check_source(&text, Path::new(path)));
        assert!(checked.model.is_none() && checked.diags.iter().all(|d| d.code == "E005"), "{path} is refused with E005 alone: {:?}", checked.diags.iter().map(|d| (&d.code, &d.en)).collect::<Vec<_>>());
        let said: String = checked.diags.iter().map(|d| d.render(path, &text, lang)).collect();
        let golden = dir.join(format!("connect_rules_contract.{tag}.txt"));
        if bless {
            std::fs::write(&golden, &said).unwrap();
            continue;
        }
        let want = std::fs::read_to_string(&golden).unwrap_or_default();
        if want != said {
            failures.push(format!("{path} ({tag}) differs from {}:\n--- want\n{want}\n--- got\n{said}", rel(&golden)));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// Every HTTP Task of a state machine, in a Map's or a Parallel's states too.
fn http_tasks(v: &Value, each: &mut impl FnMut(&Value)) {
    match v {
        Value::Object(o) => {
            if o.get("Resource").and_then(|r| r.as_str()) == Some("arn:aws:states:::http:invoke") {
                each(v);
            }
            o.values().for_each(|x| http_tasks(x, each));
        }
        Value::Array(a) => a.iter().for_each(|x| http_tasks(x, each)),
        _ => {}
    }
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

        // an HTTP Task reaches its API through the EventBridge connection the flow names, whichever call it is: the runners play HTTP Tasks without it
        let mut connections = std::collections::BTreeSet::new();
        let mut tasks = 0;
        http_tasks(&serde_json::from_str(&files[0].1).unwrap(), &mut |t| {
            tasks += 1;
            let arn = t["Arguments"]["InvocationConfig"]["ConnectionArn"].as_str().unwrap_or_default();
            assert!(arn.starts_with("arn:aws:events:"), "{}: an HTTP Task has no connection:\n{t}", rel(&f));
            connections.insert(arn.to_string());
        });
        let declared: std::collections::BTreeSet<String> = m.rules.iter().filter_map(|r| r.connection.clone()).chain(m.tasks.iter().filter_map(|t| t.connection.clone())).collect();
        assert_eq!(connections, declared, "{}: the connections of the HTTP Tasks ({tasks}) are not the ones the flow names", rel(&f));

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
            // every Jev task sends to one URL; an error's status is the task's that declares it
            Some(Via::Jev(_)) => {
                let errors: serde_json::Map<String, Value> = t.errors.iter().filter_map(|e| e.status.map(|s| (e.name.clone(), json!(s)))).collect();
                http.push(json!({ "method": "POST", "url": dandori::model::JEV_URL, "errors": errors }));
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
    temporal_all(Sdk::Ts);
}

/// The Python of tools/temporal-python/.venv, where the Temporal SDK for Python is.
fn temporal_python() -> Option<PathBuf> {
    let py = root().join("tools/temporal-python/.venv/bin/python");
    py.exists().then_some(py)
}

#[test]
fn temporal_python_runs_as_the_reference_says() {
    need_rulec!();
    if temporal_python().is_none() {
        eprintln!("SKIP: tools/temporal-python/.venv is missing; make it as tools/temporal-python/requirements.txt says");
        return;
    }
    temporal_all(Sdk::Py);
}

/// The languages of the Temporal SDKs that dandori writes for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sdk {
    Ts,
    Py,
    Go,
}

impl Sdk {
    fn name(self) -> &'static str {
        match self {
            Sdk::Ts => "TypeScript",
            Sdk::Py => "Python",
            Sdk::Go => "Go",
        }
    }

    fn short(self) -> &'static str {
        match self {
            Sdk::Ts => "ts",
            Sdk::Py => "py",
            Sdk::Go => "go",
        }
    }

    fn build(self, m: &Model) -> Result<Vec<(String, String)>, Vec<dandori::diag::Diag>> {
        match self {
            Sdk::Ts => dandori::temporal::build(m),
            Sdk::Py => dandori::temporal_py::build(m),
            Sdk::Go => dandori::temporal_go::build(m),
        }
    }

    /// The directory of the build's code: the workflow's package.
    fn package(self, m: &Model) -> String {
        match self {
            Sdk::Ts => dandori::render::ident(&m.name),
            Sdk::Py => dandori::temporal_py::package(m),
            Sdk::Go => dandori::temporal_go::package(m),
        }
    }

    /// The method of the stand-in that runs a task the user writes, in the language that serves the activities.
    fn method(self, task: &str) -> String {
        match self {
            Sdk::Ts => dandori::render::ident(task),
            Sdk::Py => dandori::temporal_py::method(task),
            Sdk::Go => task.to_string(),
        }
    }

    /// The function of the workflow's code that calls a task's activity.
    fn proxy(self, task: &str) -> String {
        match self {
            Sdk::Ts => dandori::render::ident(&format!("task_{task}")),
            Sdk::Py => format!("dd_task_{}", dandori::render::ident(task)),
            Sdk::Go => dandori::temporal_go::task_function(task),
        }
    }
}

/// The Go runner: one binary, built by tools/temporal-go from the Go of every flow the Temporal
/// tests run (building one a flow costs too much time and memory), and the key of each package in
/// it, by what the package is: a flow (`rel(f)`), the two of tests/children (`children/<name>`),
/// and the two builds of tests/versions (`versions/a`, `versions/b`).
struct GoRunner {
    bin: PathBuf,
    keys: std::collections::BTreeMap<String, String>,
}

impl GoRunner {
    fn key(&self, what: &str) -> &str {
        self.keys.get(what).unwrap_or_else(|| panic!("the Go runner has no package for {what}"))
    }
}

/// The Go runner, built once (by the first test that asks); None, with a SKIP line, when Go or the
/// module in tools/temporal-go is missing. The runner of a test process that has ended is removed
/// first: it is a hundred megabytes, and the temporary directory keeps it.
fn go_runner() -> Option<&'static GoRunner> {
    static GO: std::sync::OnceLock<Option<GoRunner>> = std::sync::OnceLock::new();
    let got = GO.get_or_init(|| {
        let tools = root().join("tools/temporal-go");
        if !tools.join("go.mod").exists() || !Command::new("go").arg("version").output().map(|o| o.status.success()).unwrap_or(false) {
            return None;
        }
        for e in std::fs::read_dir(std::env::temp_dir()).into_iter().flatten().flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let Some(pid) = name.strip_prefix("dandori-test-").and_then(|n| n.strip_suffix("-go-runner")) else { continue };
            let ended = pid != std::process::id().to_string() && Command::new("kill").args(["-0", pid]).stderr(std::process::Stdio::null()).status().is_ok_and(|s| !s.success());
            if ended {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
        let dir = scratch("go-runner");
        let mut entries = Vec::new();
        let mut keys = std::collections::BTreeMap::new();
        let mut add = |what: String, m: &Model, patch: &str, entries: &mut Vec<Value>| {
            let key = format!("p{}", keys.len());
            let out = dir.join(&key);
            let files = dandori::temporal_go::build(m).unwrap_or_else(|d| panic!("{what} does not build for Go: {}", d[0].en));
            for (name, text) in &files {
                let p = out.join(name);
                std::fs::create_dir_all(p.parent().unwrap()).unwrap();
                std::fs::write(&p, text).unwrap();
            }
            let package = out.join(dandori::temporal_go::package(m));
            let callbacks: Vec<String> = m.tasks.iter().filter(|t| t.callback && !t.is_child(Platform::Temporal)).map(|t| dandori::temporal_go::task_function(&t.name)).collect();
            entries.push(json!({ "key": key, "dir": package, "patch": patch, "callbacks": callbacks }));
            keys.insert(what, key);
        };
        // every flow, the versions for the other platforms too, whose default Transport the checks send through
        for f in runnable() {
            let (_, checked) = dandori::check::check_file(&f).unwrap();
            add(rel(&f), &checked.model.expect("the examples pass check"), "full", &mut entries);
        }
        // the parent and the child of tests/children, as they are
        let (_, checked) = dandori::check::check_file(&root().join("tests/children/受付.flow")).unwrap();
        let parent = checked.model.expect("the flow passes check");
        let child = parent.tasks.iter().find_map(|t| t.flow.as_ref()).expect("a task runs a .flow").model.clone();
        add("children/parent".into(), &parent, "none", &mut entries);
        add("children/child".into(), &child, "none", &mut entries);
        // the two builds of tests/versions, which go on in a new run at every round
        for (name, flow) in versions_texts() {
            if name == "again" {
                continue;
            }
            let d = dir.join(format!("versions-{name}"));
            std::fs::create_dir_all(&d).unwrap();
            let f = d.join("approvals.flow");
            std::fs::write(&f, flow).unwrap();
            let (_, checked) = dandori::check::check_file(&f).unwrap();
            add(format!("versions/{name}"), &checked.model.unwrap(), "continue", &mut entries);
        }
        let manifest = dir.join("manifest.json");
        std::fs::write(&manifest, serde_json::to_string_pretty(&entries).unwrap()).unwrap();
        let bin = dir.join("run");
        let started = std::time::Instant::now();
        let out = Command::new("go").arg("run").arg("./build").arg(&bin).arg(&manifest).current_dir(&tools).env("GOWORK", "off").output().unwrap();
        assert!(out.status.success(), "tools/temporal-go could not build the Go runner:\n{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        eprintln!("built the Go runner of {} package(s) in {:.0} s", entries.len(), started.elapsed().as_secs_f64());
        Some(GoRunner { bin, keys })
    });
    if got.is_none() {
        eprintln!("SKIP: go or tools/temporal-go is missing; the Go that dandori writes for Temporal is not run");
    }
    got.as_ref()
}

/// The flow of tests/versions as it is (a, and again), and with one text changed (b).
fn versions_texts() -> Vec<(&'static str, String)> {
    let text = std::fs::read_to_string(root().join("tests/versions/approvals.flow")).unwrap();
    let changed = text.replace("さんが申込 {申込.id} を承認しました\")", "さんが申込 {申込.id} を承認しました（新しいビルド）\")");
    assert_ne!(text, changed, "tests/versions/approvals.flow no longer has the text the test changes");
    vec![("a", text.clone()), ("b", changed), ("again", text)]
}

#[test]
fn temporal_go_runs_as_the_reference_says() {
    need_rulec!();
    if go_runner().is_none() {
        return;
    }
    temporal_all(Sdk::Go);
}

/// Every flow on Temporal at once, each with a runner and a dev server of its own, in one language.
fn temporal_all(lang: Sdk) {
    let _turn = heavy();
    std::thread::scope(|scope| {
        for f in runnable_on(Platform::Temporal) {
            scope.spawn(move || temporal_one(&f, lang, None));
        }
    });
}

/// The workflow in one language, and its activities in another: the workers of the build that
/// runs the workflow run nothing else, and the other build's runner serves the activities on the
/// same server (`--serve`): TypeScript and Python each way, and then Go each way with TypeScript.
/// The two rounds take turns: the four pairs of every flow at once start twice the processes of
/// one, and beside the rest of the tests, runs then time out. The flows whose rules say `local`
/// are left out: a local activity runs in the worker of the workflow, so it is in the workflow's
/// language.
#[test]
fn temporal_activities_run_in_the_other_language() {
    need_rulec!();
    if !temporal_available() || temporal_python().is_none() {
        eprintln!("SKIP: tools/temporal/node_modules or tools/temporal-python/.venv is missing");
        return;
    }
    let go = go_runner().is_some();
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
    let mut rounds = vec![[(Sdk::Ts, Sdk::Py), (Sdk::Py, Sdk::Ts)]];
    if go {
        rounds.push([(Sdk::Go, Sdk::Ts), (Sdk::Ts, Sdk::Go)]);
    }
    for pairs in &rounds {
        std::thread::scope(|scope| {
            for f in &flows {
                for (wf, acts) in pairs {
                    scope.spawn(move || temporal_one(f, *wf, Some(*acts)));
                }
            }
        });
    }
}

/// One flow on Temporal, its workflow in `wf`; with `acts`, its activities in that other language,
/// served by that language's runner.
fn temporal_one(f: &Path, wf: Sdk, acts: Option<Sdk>) {
    let (_, checked) = dandori::check::check_file(f).unwrap();
    let m = checked.model.expect("the examples pass check");
    let lang = match acts {
        None => format!("{}-", wf.short()),
        Some(a) => format!("{}-{}-", wf.short(), a.short()),
    };
    let dir = scratch(&format!("temporal-{lang}{}", key(f)));
    // the code of the languages whose runner is not the Go one, which has every flow's built in
    for l in std::iter::once(wf).chain(acts) {
        if l == Sdk::Go {
            continue;
        }
        let files = l.build(&m).unwrap_or_else(|d| panic!("{} does not build: {}", rel(f), d[0].en));
        for (name, text) in &files {
            let p = dir.join(name);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, text).unwrap();
        }
    }
    let go = if wf == Sdk::Go || acts == Some(Sdk::Go) { go_runner() } else { None };
    let python = temporal_python();
    let p = Platform::Temporal;
    // the stand-ins of the tasks the user writes are methods of the language that serves the activities
    let serving = acts.unwrap_or(wf);
    let own: Vec<Value> = m.tasks.iter().filter(|t| t.via(p) == Some(Via::Own)).map(|t| json!({ "name": t.name, "method": serving.method(&t.name), "callback": t.callback })).collect();
    // the rules whose code goes with the workflow are stand-ins; one called at its service is an activity the generated code writes, which sends through the stand-in Transport
    let rules: Vec<String> = m.rules.iter().filter(|r| r.connect.is_none()).map(|r| dandori::render::rule_activity(&r.name)).collect();
    let children: Vec<Value> = m.tasks.iter().filter_map(|t| t.workflow.as_ref().map(|w| json!({ "type": w, "queue": t.queue }))).collect();
    let mut queues: Vec<String> = m.tasks.iter().filter_map(|t| t.queue.clone()).collect();
    queues.sort();
    queues.dedup();
    let (http, aws) = transport_spec(&m, p);
    // on a real server an activity times out as the scenario says, and a callback that gets no answer
    let (runs, references, _) = plays(&m, View::Temporal, true, |_| true);
    let runs_file = dir.join("runs.json");
    let results_file = dir.join("results.json");
    // the tasks that hand on a callback's id, by the name of their function in the workflow's code
    let callbacks: Vec<String> = m.tasks.iter().filter(|t| t.callback && !t.is_child(p)).map(|t| wf.proxy(&t.name)).collect();
    let spec = json!({ "workflow": dandori::render::ident(&m.name), "own": own, "rules": rules, "children": children, "queues": queues, "http": http, "aws": aws, "callbacks": callbacks, "runs": runs });
    std::fs::write(&runs_file, serde_json::to_string(&spec).unwrap()).unwrap();
    let histories = dir.join("histories");
    let _ = std::fs::remove_dir_all(&histories);
    let mut run = match wf {
        Sdk::Py => {
            let mut c = Command::new(python.as_ref().expect("the Python of tools/temporal-python"));
            c.arg(root().join("tools/temporal-python/run.py")).arg(dir.join(wf.package(&m)));
            c
        }
        Sdk::Ts => {
            let mut c = Command::new("node");
            c.arg(root().join("tools/temporal/run.mjs")).arg(dir.join(wf.package(&m)));
            c
        }
        Sdk::Go => {
            let go = go.expect("the Go runner");
            let mut c = Command::new(&go.bin);
            c.arg(go.key(&rel(f)));
            c
        }
    };
    run.arg(&runs_file).arg(&results_file).arg(&histories);
    if let Some(by) = acts {
        let mut serve: Vec<String> = match by {
            Sdk::Ts => vec!["node".into(), root().join("tools/temporal/run.mjs").display().to_string(), "--serve".into(), dir.join(by.package(&m)).display().to_string()],
            Sdk::Py => vec![python.as_ref().unwrap().display().to_string(), root().join("tools/temporal-python/run.py").display().to_string(), "--serve".into(), dir.join(by.package(&m)).display().to_string()],
            Sdk::Go => {
                let go = go.expect("the Go runner");
                vec![go.bin.display().to_string(), "--serve".into(), go.key(&rel(f)).to_string()]
            }
        };
        serve.push(runs_file.display().to_string());
        run.env("DANDORI_ACTIVITIES_BY", serde_json::to_string(&serve).unwrap());
    }
    let out = run.output().unwrap();
    let what = match acts {
        None if wf == Sdk::Ts => "the Temporal workflow".to_string(),
        None => format!("the Temporal workflow in {}", wf.name()),
        Some(a) if wf == Sdk::Ts => format!("the Temporal workflow with its activities in {}", a.name()),
        Some(a) => format!("the Temporal workflow in {} with its activities in {}", wf.name(), a.name()),
    };
    assert!(out.status.success(), "{}: the runner of {what} failed:\n{}", rel(f), String::from_utf8_lossy(&out.stderr));
    let mut got: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(&results_file).unwrap()).unwrap();
    // what the query dandori.status and the search attribute DandoriCases say of the cases at the end,
    // looked at once the runs themselves are: a run that went another way says why its cases differ
    let said: Vec<(Value, Value, Vec<Value>)> = got
        .iter_mut()
        .map(|g| {
            let o = g.as_object_mut().unwrap();
            let (cases, shown) = (o.remove("cases").unwrap_or(Value::Null), o.remove("shown").unwrap_or(Value::Null));
            (cases, shown, o.remove("asked").and_then(|a| a.as_array().cloned()).unwrap_or_default())
        })
        .collect();
    compare(&what, f, &references, &got);
    for (i, ((cases, shown, asked), (sc, _))) in said.iter().zip(&references).enumerate() {
        // the query that said otherwise than the search attribute, and was asked again (see the runners)
        for again in asked {
            eprintln!("{} run {}: asked again, since the query said {again} where the search attribute said {shown}", rel(f), i + 1);
        }
        let want = Value::Object(dandori::interp::cases_at_end(&m, sc, View::Temporal).unwrap());
        assert_eq!(*cases, want, "{} run {}: the query to {what} says the cases are {cases}, and the reference {want}", rel(f), i + 1);
        // the workflow writes the search attribute when a case moves; before that, it has none
        let listed: Vec<Value> = want.as_object().unwrap().iter().filter(|(_, s)| !s.is_null()).map(|(c, s)| json!(format!("{c}={}", s.as_str().unwrap_or_default()))).collect();
        let listed = if listed.is_empty() { Value::Null } else { Value::Array(listed) };
        assert_eq!(*shown, listed, "{} run {}: the search attribute of {what} says {shown}, and the reference's cases {listed}", rel(f), i + 1);
    }
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
                if ev["markerRecordedEventAttributes"]["markerName"] == "core_local_activity" || ev["markerRecordedEventAttributes"]["markerName"] == "LocalActivity" {
                    markers += 1;
                }
            }
        }
        assert!(markers > 0, "{}: no run called a rule that says `local` as a local activity", rel(f));
    }
    let how = match acts {
        None if wf == Sdk::Ts => String::new(),
        None => format!(" in {}", wf.name()),
        Some(a) if wf == Sdk::Ts => format!(", the activities in {},", a.name()),
        Some(a) => format!(" in {}, the activities in {},", wf.name(), a.name()),
    };
    eprintln!("{}: compared {} run(s) on Temporal{how} (the dev server), with the query and the search attribute, and replayed each", rel(f), references.len());
    // with DANDORI_BLESS, the history of the run with the most calls is kept for the replay test
    if acts.is_none() && std::env::var("DANDORI_BLESS").is_ok() && RECORDED.iter().any(|r| rel(f) == *r) {
        let longest = references.iter().enumerate().max_by_key(|(_, (_, r))| r["steps"].as_array().map(|a| a.len()).unwrap_or(0)).map(|(i, _)| i).unwrap();
        let keep = recorded_dir(&m, wf);
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
            // the Go SDK names a sticky task queue <host>:<uuid>
            t = t.replace(&format!("@{host}"), "@localhost").replace(&format!("\"{host}:"), "\"localhost:");
        }
    }
    t
}

/// Worker Deployment Versioning, through the generated worker, on the dev server: two builds of
/// tests/versions/approvals.flow whose code differs in one text, A and B, as versions of one
/// deployment. A run that starts on A and waits for its approval while B becomes the current
/// version ends on A's code, and so does its second round, which starts in a new run
/// (Continue-As-New); a run that starts after runs on B's. The build id is a hash of the code:
/// the same for the same code, and another for B. In TypeScript, in Python and in Go.
#[test]
fn temporal_worker_versioning_keeps_a_run_on_its_build() {
    need_rulec!();
    if !temporal_available() {
        eprintln!("SKIP: tools/temporal/node_modules is missing; run `npm install --prefix tools/temporal`");
        return;
    }
    let _turn = heavy();
    let dir = scratch("versions");
    let python = temporal_python();
    let go = go_runner();
    for lang in [Sdk::Ts, Sdk::Py, Sdk::Go] {
        if lang == Sdk::Py && python.is_none() {
            eprintln!("SKIP: tools/temporal-python/.venv is missing; Worker Deployment Versioning is not tried in Python");
            continue;
        }
        if lang == Sdk::Go && go.is_none() {
            continue;
        }
        let mut built = Vec::new();
        for (name, flow) in versions_texts() {
            let d = dir.join(format!("{}-{name}", lang.short()));
            std::fs::create_dir_all(&d).unwrap();
            let f = d.join("approvals.flow");
            std::fs::write(&f, flow).unwrap();
            let (_, checked) = dandori::check::check_file(&f).unwrap();
            let m = checked.model.unwrap();
            let files = lang.build(&m).unwrap();
            for (n, t) in &files {
                let p = d.join(n);
                std::fs::create_dir_all(p.parent().unwrap()).unwrap();
                std::fs::write(&p, t).unwrap();
            }
            let worker = files.iter().find(|(n, _)| n.ends_with("/worker.ts") || n.ends_with("/worker.py") || n.ends_with("/worker.go")).unwrap();
            let id = worker.1.lines().find(|l| l.contains("BUILD_ID =") || l.contains("BuildID =")).unwrap().split('"').nth(1).unwrap().to_string();
            built.push((d.join(lang.package(&m)), id));
        }
        assert_eq!(built[0].1, built[2].1, "the same code has two build ids");
        assert_ne!(built[0].1, built[1].1, "two builds of different code have one build id");
        let results = dir.join(format!("{}-results.json", lang.short()));
        let out = match lang {
            Sdk::Py => Command::new(python.as_ref().unwrap()).arg(root().join("tools/temporal-python/versions.py")).arg(&built[0].0).arg(&built[1].0).arg(&results).output().unwrap(),
            Sdk::Ts => Command::new("node").arg(root().join("tools/temporal/versions.mjs")).arg(&built[0].0).arg(&built[1].0).arg(&results).output().unwrap(),
            Sdk::Go => {
                let go = go.unwrap();
                Command::new(&go.bin).arg("--versions").arg(go.key("versions/a")).arg(go.key("versions/b")).arg(&results).output().unwrap()
            }
        };
        let lang = lang.name();
        assert!(out.status.success(), "the Worker Deployment Versioning run in {lang} failed:\n{}", String::from_utf8_lossy(&out.stderr));
        let got: Value = serde_json::from_str(&std::fs::read_to_string(&results).unwrap()).unwrap();
        let a_says = json!(["a さんが申込 申込-1 を承認しました", "a さんが申込 申込-1 を承認しました"]);
        let b_says = json!(["b さんが申込 申込-1 を承認しました（新しいビルド）", "b さんが申込 申込-1 を承認しました（新しいビルド）"]);
        assert_eq!(got["runs"], json!({ "run-a": 2, "run-b": 2 }), "{lang}: the second round of each did not start in a new run: {}", got["runs"]);
        assert_eq!(got["notified"]["run-a"], a_says, "{lang}: the run that started on A did not end on A's code: {}", got["notified"]);
        assert_eq!(got["notified"]["run-b"], b_says, "{lang}: the run that started on B did not run B's code: {}", got["notified"]);
        if lang == "Go" {
            assert_eq!(got["buildIds"], json!([built[0].1, built[1].1]), "Go: the runner's builds have other build ids than dandori wrote: {}", got["buildIds"]);
        }
        eprintln!("{lang}: a run pinned to build {} ended on it, in the run it went on in too, after {} became the current version", built[0].1, built[1].1);
    }
}

/// A workflow that runs another `.flow` as its child (`flow "<path>"`, tests/children), both as
/// dandori writes them, on one Temporal server: the parent's worker and the child's, in each
/// language, and crossed: TypeScript and Python each way, Go and TypeScript each way. Nothing stands in for the child. Each run must end as the
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
    let go = go_runner();
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
    let lang = |l: &str| match l {
        "py" => "Python",
        "go" => "Go",
        _ => "TypeScript",
    };
    let _turn = heavy();
    let mut pairs = vec![("ts", "ts"), ("py", "py"), ("ts", "py"), ("py", "ts")];
    if go.is_some() {
        pairs.extend([("go", "go"), ("go", "ts"), ("ts", "go")]);
    }
    for (parent, child) in pairs {
        let results = dir.join(format!("results-{parent}-{child}.json"));
        let mut cmd = match parent {
            "py" => {
                let mut c = Command::new(&python);
                c.arg(&py_runner);
                c
            }
            "go" => {
                let mut c = Command::new(&go.unwrap().bin);
                c.arg("--children");
                c
            }
            _ => {
                let mut c = Command::new("node");
                c.arg(&node_runner);
                c
            }
        };
        // the Go runner has its packages in it, by their keys
        let at = |l: &str, what: &str, name: &String| -> String {
            if l == "go" {
                go.unwrap().key(what).to_string()
            } else {
                built[&(l, name.clone())].display().to_string()
            }
        };
        // the Go runner names both by their keys, also a child that another language's runner serves
        let child_arg = if parent == "go" { go.unwrap().key("children/child").to_string() } else { at(child, "children/child", &cm.name) };
        cmd.arg(at(parent, "children/parent", &pm.name)).arg(child_arg).arg(&runs_file).arg(&results);
        if parent != child {
            let c_dir = at(child, "children/child", &cm.name);
            let serve: Vec<String> = match child {
                "py" => vec![python.display().to_string(), py_runner.display().to_string(), "--serve".into(), c_dir],
                "go" => vec![go.unwrap().bin.display().to_string(), "--children-serve".into(), c_dir],
                _ => vec!["node".into(), node_runner.display().to_string(), "--serve".into(), c_dir],
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
/// for Temporal, in English and in Japanese. The run of the order goes on in new runs
/// (Continue-As-New), and each of them is kept.
const RECORDED: [&str; 9] = [
    "examples/hotel/temporal/hotel.flow",
    "examples/fulfillment/temporal/fulfillment.flow",
    "examples/review/temporal/review.flow",
    "tests/flows/cancel.flow",
    "examples/order/temporal/order.flow",
    "examples/hotel/temporal/hotel.ja.flow",
    "examples/fulfillment/temporal/fulfillment.ja.flow",
    "examples/review/temporal/review.ja.flow",
    "examples/order/temporal/order.ja.flow",
];

fn recorded_dir(m: &Model, lang: Sdk) -> PathBuf {
    let dir = match lang {
        Sdk::Ts => "typescript",
        Sdk::Py => "python",
        Sdk::Go => "go",
    };
    root().join("tests/histories").join(dir).join(dandori::render::ident(&m.name))
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
                // a Jev task's call: every one goes to TypeSafe's URL, and the one that declares the error names its status
                let jev = url == dandori::model::JEV_URL;
                // a rule called at its Connect service sends a POST of its own: no task declares an error for it, so any failure is a 500
                let service = m.rules.iter().any(|r| r.connect.as_ref().is_some_and(|c| method == "POST" && c.url == url));
                let task = if jev {
                    let jevs: Vec<&dandori::model::TaskDef> = m.tasks.iter().filter(|t| matches!(t.via(p), Some(Via::Jev(_)))).collect();
                    jevs.iter().find(|t| kind.is_some_and(|k| t.errors.iter().any(|e| e.name == k))).or(jevs.first()).copied()
                } else if service {
                    None
                } else {
                    Some(m.tasks.iter().find(|t| matches!(t.via(p), Some(Via::Http { method: tm, url: tu, .. }) if tm == method && url_matches(tu, url))).unwrap_or_else(|| panic!("no task sends {method} {url}")))
                };
                let form = task.is_some_and(|t| matches!(t.via(p), Some(Via::Http { form: true, .. })));
                let mut request = call.clone();
                request["form"] = json!(form);
                if jev {
                    request["typesafe"] = json!(true);
                }
                let reply = match kind {
                    None => json!({ "status": 200, "body": ans["ok"] }),
                    Some(k) => json!({ "status": task.and_then(|t| t.errors.iter().find(|e| e.name == k)).and_then(|e| e.status).unwrap_or(500), "body": "scripted" }),
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

/// The default Transports — TypeScript's (fetch, the AWS SDK for JavaScript), Python's (the
/// standard library, boto3) and Go's (net/http, the AWS SDK for Go v2, through the Go runner's
/// --wire) — send the calls of every scenario to stand-ins on this machine
/// (tools/wire): an HTTP request arrives with the method, the path, the query, the headers and
/// the body the call has, JSON or URL-encoded; a Lambda invoke with the function and the
/// payload; an SNS message or an SQS message on moto, the AWS APIs' stand-in; and each gives
/// back what the runners' stand-in Transport gives for the scenario's answer, an AWS error by
/// the name the task declares. TypeScript and Python send the same text; Go's maps keep no order,
/// so Go's JSON, query and form are held to theirs as values and as pairs. Nothing leaves the machine.
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
    // the calls by what they are (a rule's service, the other HTTP calls, Lambda, the AWS APIs), and how many of each the scenario fails
    let mut tally: std::collections::BTreeMap<String, (usize, usize)> = std::collections::BTreeMap::new();
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
                ("TypeScript", Command::new("node").arg(wire.join("check.mjs")).arg(dir.join(dandori::render::ident(&m.name))).arg(&cases_file).arg(dir.join("ts.json")).arg(&address).env("TYPESAFE_API_KEY", "wire-typesafe-key").output().unwrap()),
                ("Python", Command::new(&python).arg(wire.join("check.py")).arg(dir.join(dandori::temporal_py::package(&m))).arg(&cases_file).arg(dir.join("py.json")).arg(&address).env("TYPESAFE_API_KEY", "wire-typesafe-key").output().unwrap()),
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
                            // Jev's call carries TypeSafe's key, from TYPESAFE_API_KEY
                            if req["typesafe"] == json!(true) {
                                assert_eq!(got["headers"]["authorization"], json!("Bearer wire-typesafe-key"), "{}: TypeSafe's key", at());
                            }
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
            // the default Transport in Go: the same calls, and the same JSON; Go's maps keep no order, so it
            // writes an object's keys, and a query's and a form's pairs, in the order of their names
            if let Some(go) = go_runner() {
                let results_file = dir.join("go.json");
                let out = Command::new(&go.bin).arg("--wire").arg(go.key(&rel(&f))).arg(&cases_file).arg(&results_file).arg(&address).env("TYPESAFE_API_KEY", "wire-typesafe-key").output().unwrap();
                assert!(out.status.success(), "{}: the Go check failed:\n{}", rel(&f), String::from_utf8_lossy(&out.stderr));
                let results: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(&results_file).unwrap()).unwrap();
                let sorted = |v: &Value| -> Value {
                    let mut a = v.as_array().cloned().unwrap_or_default();
                    a.sort_by_key(|x| x.to_string());
                    Value::Array(a)
                };
                for (i, (c, r)) in cases.iter().zip(&results).enumerate() {
                    let at = || format!("{} (Go): {}", rel(&f), serde_json::to_string(c).unwrap());
                    let ts = &by_lang[0][i];
                    match c["kind"].as_str().unwrap() {
                        "http" => {
                            let (got, want) = (&r["received"], &ts["received"]);
                            assert_eq!(got["method"], want["method"], "{}: the method", at());
                            assert_eq!(got["path"], want["path"], "{}: the path", at());
                            assert_eq!(sorted(&got["query"]), sorted(&want["query"]), "{}: the query", at());
                            for (k, v) in c["request"]["headers"].as_object().into_iter().flatten() {
                                assert_eq!(got["headers"][k.to_lowercase()], *v, "{}: the header {k}", at());
                            }
                            assert_eq!(got["headers"]["x-dandori-check"], json!("wire"), "{}: the headers the options add", at());
                            if c["request"]["typesafe"] == json!(true) {
                                assert_eq!(got["headers"]["authorization"], json!("Bearer wire-typesafe-key"), "{}: TypeSafe's key", at());
                            }
                            if c["request"]["form"] == json!(true) {
                                assert_eq!(sorted(&got["body"]), sorted(&want["body"]), "{}: the form", at());
                            } else {
                                assert_eq!(got["body"], want["body"], "{}: the body", at());
                                if !want["body"].is_null() {
                                    assert_eq!(got["headers"]["content-type"], json!("application/json"), "{}: the body's type", at());
                                }
                            }
                            assert_eq!(r["returned"], c["reply"], "{}: what the Transport gave back", at());
                        }
                        "lambda" => {
                            assert_eq!(r["received"]["fn"], c["fn"], "{}: the function", at());
                            assert_eq!(r["received"]["payload"], c["payload"], "{}: the payload", at());
                            assert!(r["received"]["invocationType"].is_null() || r["received"]["invocationType"] == json!("RequestResponse"), "{}: the invocation's type", at());
                            assert_eq!(r["returned"], c["reply"], "{}: what the Transport gave back", at());
                        }
                        _ => {
                            match c.get("error") {
                                Some(e) => assert_eq!(r["returned"]["error"], *e, "{}: the error's name ({})", at(), r["returned"]),
                                None => assert!(r["returned"]["ok"]["MessageId"].is_string(), "{}: no message id in {}", at(), r["returned"]),
                            }
                            // a message that is JSON text is compared as the JSON it is: Go writes its keys in the order of their names
                            let read = |v: &Value| -> Value { Value::Array(v.as_array().into_iter().flatten().map(|m| m.as_str().and_then(|t| serde_json::from_str::<Value>(t).ok()).unwrap_or_else(|| m.clone())).collect()) };
                            assert_eq!(read(&r["messages"]), read(&ts["messages"]), "{}: the messages differ from TypeScript's", at());
                            let keys = |r: &Value| r["returned"]["ok"].as_object().map(|o| o.keys().cloned().collect::<std::collections::BTreeSet<_>>());
                            assert_eq!(keys(r), keys(ts), "{}: the fields of moto's answer differ from TypeScript's", at());
                        }
                    }
                }
                eprintln!("{}: sent {} call(s) through the default Transport of Go", rel(&f), cases.len());
            }
            eprintln!("{}: sent {} call(s) through the default Transport of TypeScript and of Python", rel(&f), cases.len());
            checked += cases.len();
            for c in &cases {
                let kind = c["kind"].as_str().unwrap_or_default();
                let failed = match kind {
                    "http" => c["reply"]["status"] != json!(200),
                    "lambda" => c["reply"].get("error").is_some(),
                    _ => c.get("error").is_some(),
                };
                let service = kind == "http" && m.rules.iter().any(|r| r.connect.as_ref().is_some_and(|x| c["request"]["http"] == "POST" && c["request"]["url"] == json!(x.url)));
                let class = if service { "a rule's service" } else { kind };
                let t = tally.entry(class.to_string()).or_insert((0, 0));
                t.0 += 1;
                t.1 += usize::from(failed);
            }
        }
        eprintln!("{} call(s) through the default Transport in all: {}", tally.values().map(|t| t.0).sum::<usize>(), tally.iter().map(|(k, t)| format!("{} {k} ({} failed)", t.0, t.1)).collect::<Vec<_>>().join(", "));
    }));
    let _ = moto.kill();
    if let Err(e) = outcome {
        std::panic::resume_unwind(e);
    }
    assert!(checked > 0, "no call went through a Transport");
}

/// The kept histories, of TypeScript, Python and Go, replay with the code dandori writes now: a change of the generator that
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
    let go = go_runner();
    let mut replayed = 0;
    for r in RECORDED {
        let f = root().join(r);
        let (_, checked) = dandori::check::check_file(&f).unwrap();
        let m = checked.model.expect("the flows pass check");
        for lang in [Sdk::Ts, Sdk::Py, Sdk::Go] {
            if lang == Sdk::Go && go.is_none() {
                continue;
            }
            let kept = recorded_dir(&m, lang);
            if !kept.exists() {
                panic!("{r}: no histories kept in {}; record them with DANDORI_BLESS=1", rel(&kept));
            }
            let dir = scratch(&format!("replay-{}-{}", lang.short(), key(&f)));
            if lang != Sdk::Go {
                for (name, text) in &lang.build(&m).unwrap() {
                    let p = dir.join(name);
                    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
                    std::fs::write(&p, text).unwrap();
                }
            }
            let results = dir.join("replayed.json");
            let out = match lang {
                Sdk::Py => Command::new(python.as_ref().unwrap()).arg(root().join("tools/temporal-python/run.py")).arg("--replay").arg(dir.join(lang.package(&m))).arg(&kept).arg(&results).output().unwrap(),
                Sdk::Ts => Command::new("node").arg(root().join("tools/temporal/run.mjs")).arg("--replay").arg(dir.join(lang.package(&m))).arg(&kept).arg(&results).output().unwrap(),
                Sdk::Go => Command::new(&go.unwrap().bin).arg("--replay").arg(go.unwrap().key(r)).arg(&kept).arg(&results).output().unwrap(),
            };
            assert!(out.status.success(), "{r}: the replay failed:\n{}", String::from_utf8_lossy(&out.stderr));
            let got: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(&results).unwrap()).unwrap();
            for g in &got {
                assert!(g["error"].is_null(), "{r}: the kept history {} does not replay with the code dandori writes now ({}): {}", g["file"], lang.name(), g["error"]);
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
            // the rules whose code goes with the workflow need rulec's modules; one called at its service needs none
            let called: std::collections::BTreeSet<usize> = m.all_stmts().iter().filter_map(|s| match &s.kind { TK::Call { callee: Callee::Rule(r), .. } if m.rules[*r].connect.is_none() => Some(*r), _ => None }).collect();
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

/// A Go module of the Go that dandori writes for flows, with the rules as rulec generates them: the
/// module of tools/temporal-go's go.mod and go.sum, each flow's package under flows/, and each
/// rule's package under rulec/go/, which go.mod requires and replaces, as rules.go says to. Two
/// rules whose Go packages have one name (a rule and its Japanese twin) go to two modules.
struct GoModule {
    dir: PathBuf,
    packages: Vec<GoPackage>,
}

/// A flow's package in a Go module.
struct GoPackage {
    flow: String,
    import: String,
    /// the rules it bundles: each one's activity, and the file of its vectors
    vectors: Vec<(String, PathBuf)>,
}

/// The modules of these flows, made once a name.
fn go_modules(name: &str, flows: &[PathBuf]) -> Vec<GoModule> {
    let tools = root().join("tools/temporal-go");
    let mut modules: Vec<(GoModule, std::collections::BTreeMap<String, PathBuf>)> = Vec::new();
    for f in flows {
        let (_, checked) = dandori::check::check_file(f).unwrap();
        let m = checked.model.expect("the flows pass check");
        let Ok(files) = dandori::temporal_go::build(&m) else { continue };
        let called: std::collections::BTreeSet<usize> = m.all_stmts().iter().filter_map(|s| match &s.kind { TK::Call { callee: Callee::Rule(r), .. } if m.rules[*r].connect.is_none() => Some(*r), _ => None }).collect();
        let rules: Vec<(String, PathBuf)> = called.iter().map(|r| (m.rules[*r].info.api["go"]["package"].as_str().unwrap().to_string(), m.rules[*r].info.path.clone())).collect();
        // the first module whose rules of these names are these rules
        let at = modules.iter().position(|(_, by)| rules.iter().all(|(p, path)| by.get(p).is_none_or(|x| x == path))).unwrap_or_else(|| {
            let dir = scratch(&format!("go-{name}-{}", modules.len()));
            let gomod = std::fs::read_to_string(tools.join("go.mod")).unwrap();
            let gomod = gomod.lines().map(|l| if l.starts_with("module ") { "module gocheck".to_string() } else { l.to_string() }).collect::<Vec<_>>().join("\n") + "\n";
            std::fs::write(dir.join("go.mod"), gomod).unwrap();
            std::fs::copy(tools.join("go.sum"), dir.join("go.sum")).unwrap();
            modules.push((GoModule { dir, packages: vec![] }, std::collections::BTreeMap::new()));
            modules.len() - 1
        });
        let (module, by) = &mut modules[at];
        let k = format!("f{}", module.packages.len());
        for (fname, text) in &files {
            let p = module.dir.join("flows").join(&k).join(fname);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, text).unwrap();
        }
        let mut vectors = Vec::new();
        for r in &called {
            let ru = &m.rules[*r];
            let p = ru.info.api["go"]["package"].as_str().unwrap().to_string();
            if !by.contains_key(&p) {
                let gen = Command::new(rulec_bin()).arg("gen").arg(&ru.info.path).arg("--out").arg(module.dir.join("rulec")).output().unwrap();
                assert!(gen.status.success(), "rulec gen failed: {}", String::from_utf8_lossy(&gen.stderr));
                let mut gomod = std::fs::read_to_string(module.dir.join("go.mod")).unwrap();
                gomod.push_str(&format!("\nrequire {p} v0.0.0\n\nreplace {p} => ./rulec/go/{p}\n"));
                std::fs::write(module.dir.join("go.mod"), gomod).unwrap();
                by.insert(p.clone(), ru.info.path.clone());
            }
            let file = module.dir.join(format!("vectors-{}.jsonl", dandori::render::ident(&ru.name)));
            let out = Command::new(rulec_bin()).arg("vectors").arg(&ru.info.path).output().unwrap();
            std::fs::write(&file, &out.stdout).unwrap();
            vectors.push((dandori::render::rule_activity(&ru.name), file));
        }
        let import = format!("gocheck/flows/{k}/{}", dandori::temporal_go::package(&m));
        module.packages.push(GoPackage { flow: rel(f), import, vectors });
    }
    // rulec's own runners are modules of their own, which nothing here needs
    for (module, _) in &modules {
        if let Ok(rd) = std::fs::read_dir(module.dir.join("rulec/go")) {
            for e in rd.flatten() {
                if e.file_name().to_string_lossy().ends_with("runner") {
                    let _ = std::fs::remove_dir_all(e.path());
                }
            }
        }
    }
    modules.into_iter().map(|(m, _)| m).collect()
}

/// The Go dandori writes for every flow, with the rules as rulec generates them, passes `go vet`,
/// and is as gofmt writes it.
#[test]
fn generated_go_vets() {
    need_rulec!();
    if !Command::new("go").arg("version").output().map(|o| o.status.success()).unwrap_or(false) || !root().join("tools/temporal-go/go.mod").exists() {
        eprintln!("SKIP: go or tools/temporal-go is missing; the Go that dandori writes is not vetted");
        return;
    }
    let mut vetted = 0;
    for module in go_modules("vet", &runnable()) {
        let out = Command::new("go").args(["vet", "./..."]).current_dir(&module.dir).env("GOWORK", "off").output().unwrap();
        assert!(out.status.success(), "go vet finds fault with the Go dandori writes ({}):\n{}{}", module.dir.display(), String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        let fmt = Command::new("gofmt").arg("-l").arg("flows").current_dir(&module.dir).output().unwrap();
        assert!(fmt.status.success() && fmt.stdout.is_empty(), "gofmt would write these otherwise ({}):\n{}{}", module.dir.display(), String::from_utf8_lossy(&fmt.stdout), String::from_utf8_lossy(&fmt.stderr));
        vetted += module.packages.len();
    }
    eprintln!("vetted the Go of {vetted} flow(s), with their rules, with go vet and gofmt");
}

/// The code between a platform and a rule — the Lambda handler for Step Functions and the
/// activity for Temporal, in TypeScript and in Go — answers every vector rulec generates for the
/// rule as rulec says.
#[test]
fn rule_glue_answers_the_rulec_vectors() {
    need_rulec!();
    let python = Command::new("python3").arg("--version").output().map(|o| o.status.success()).unwrap_or(false);
    let node = Command::new("node").arg("--version").output().map(|o| o.status.success()).unwrap_or(false);
    for f in flows(&root().join("examples")) {
        let (_, checked) = dandori::check::check_file(&f).unwrap();
        let m = checked.model.expect("the examples pass check");
        let ts_files = dandori::temporal::build(&m).unwrap();
        // every rule the flow calls, whether or not Step Functions can run the flow; a rule that is
        // only read for its machine has no glue
        let called: std::collections::BTreeSet<usize> = m.all_stmts().iter().filter_map(|s| match &s.kind { TK::Call { callee: Callee::Rule(r), .. } => Some(*r), _ => None }).collect();
        for (ri, r) in m.rules.iter().enumerate() {
            // a rule called at its service has no code that goes with the workflow
            if !called.contains(&ri) || r.connect.is_some() {
                continue;
            }
            let (hname, htext) = &dandori::asl::lambda_handler(&m, ri);
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
    // rules.go, with the Go rulec generates: every rule of every example, in a program of each module
    if !Command::new("go").arg("version").output().map(|o| o.status.success()).unwrap_or(false) || !root().join("tools/temporal-go/go.mod").exists() {
        eprintln!("SKIP: go or tools/temporal-go is missing; rules.go is not run");
        return;
    }
    let mut answered = 0;
    for module in go_modules("glue", &flows(&root().join("examples"))) {
        let mut imports = Vec::new();
        let mut calls = Vec::new();
        let mut expected = serde_json::Map::new();
        for (i, GoPackage { flow: f, import, vectors }) in module.packages.iter().enumerate() {
            if vectors.is_empty() {
                continue;
            }
            // the rules of the package, which are its own, for the program to call
            let dir = module.dir.join(import.trim_start_matches("gocheck/"));
            let pkg = import.rsplit('/').next().unwrap();
            std::fs::write(dir.join("dd_rules_export.go"), format!("package {pkg}\n\n// DDRulesForTheTest are the rules' activities, for the test of their glue.\nfunc DDRulesForTheTest() map[string]any {{\n\treturn ddRules\n}}\n")).unwrap();
            imports.push(format!("\tp{i} \"{import}\""));
            for (activity, file) in vectors {
                let k = format!("{f} {activity}");
                calls.push(format!("\trun({}, p{i}.DDRulesForTheTest(), {}, {})", json!(k), json!(activity), json!(file.display().to_string())));
                let lines = std::fs::read_to_string(file).unwrap();
                expected.insert(k, Value::Array(lines.lines().map(|l| serde_json::from_str::<Value>(l).unwrap()["out"].clone()).collect()));
            }
        }
        if calls.is_empty() {
            continue;
        }
        let main = format!(
            "package main\n\nimport (\n\t\"bufio\"\n\t\"context\"\n\t\"encoding/json\"\n\t\"os\"\n\n{}\n)\n\nfunc main() {{\n\tout := map[string][]any{{}}\n\trun := func(key string, rules map[string]any, activity string, file string) {{\n\t\tcall := rules[activity].(func(context.Context, map[string]any) (any, error))\n\t\tf, err := os.Open(file)\n\t\tif err != nil {{\n\t\t\tpanic(err)\n\t\t}}\n\t\tdefer f.Close()\n\t\tlines := bufio.NewScanner(f)\n\t\tlines.Buffer(make([]byte, 1<<20), 1<<20)\n\t\tfor lines.Scan() {{\n\t\t\tvar v struct {{\n\t\t\t\tIn map[string]any `json:\"in\"`\n\t\t\t}}\n\t\t\tif err := json.Unmarshal(lines.Bytes(), &v); err != nil {{\n\t\t\t\tpanic(err)\n\t\t\t}}\n\t\t\tgot, err := call(context.Background(), v.In)\n\t\t\tif err != nil {{\n\t\t\t\tgot = map[string]any{{\"error\": err.Error()}}\n\t\t\t}}\n\t\t\tout[key] = append(out[key], got)\n\t\t}}\n\t}}\n{}\n\tif err := json.NewEncoder(os.Stdout).Encode(out); err != nil {{\n\t\tpanic(err)\n\t}}\n}}\n",
            imports.join("\n"),
            calls.join("\n")
        );
        std::fs::create_dir_all(module.dir.join("cmd/vectors")).unwrap();
        std::fs::write(module.dir.join("cmd/vectors/main.go"), main).unwrap();
        let out = Command::new("go").args(["run", "./cmd/vectors"]).current_dir(&module.dir).env("GOWORK", "off").output().unwrap();
        assert!(out.status.success(), "rules.go failed ({}):\n{}", module.dir.display(), String::from_utf8_lossy(&out.stderr));
        let got: Value = serde_json::from_slice(&out.stdout).unwrap();
        for (k, want) in &expected {
            assert_eq!(norm(&got[k]), norm(want), "the Temporal activity in Go of {k} differs from rulec's vectors");
            answered += want.as_array().map(|a| a.len()).unwrap_or(0);
        }
    }
    eprintln!("rules.go answered {answered} vector(s) of the examples' rules as rulec says");
}

/// What a rule's service answers is read as the rule's own record the same way on every platform,
/// also when it is not what the service would write: a number that is not a decimal, a name no enum
/// has (`constructor`, `__proto__`), a field of another kind, a null, a body that is not an object. The
/// reference (`render::rule_read`), the TypeScript of io.ts, the Python of io.py, the Go of io.go and the JSONata a
/// state machine reads the answer with give the same record, and none of them raises: what is wrong
/// is left for the check of the answer to refuse (DESIGN 1.13). The rules are those of
/// connect_rules.flow, whose enums are their own, and of connect_rules_contract.flow, whose enum is a
/// contract's with a value of its own at 0, which an answer that leaves it out has.
#[test]
fn a_rules_answer_is_read_alike_by_the_reference_typescript_python_go_and_jsonata() {
    need_rulec!();
    need_node!();
    if !Command::new("python3").arg("--version").output().map(|o| o.status.success()).unwrap_or(false) {
        eprintln!("SKIP: python3 is missing; io.py's reading of a rule's answer is not run");
        return;
    }
    use dandori::rulec::WireKind;
    let mut models = Vec::new();
    for f in ["tests/flows/connect_rules.flow", "tests/flows/connect_rules_contract.flow"] {
        if !rulec_names_enums() && NAMED_ENUMS.contains(&f) {
            eprintln!("SKIP: {f}: this rulec's `rulec api` does not say what the service of its rule calls the values (`connect.enums`)");
            continue;
        }
        let (_, checked) = dandori::check::check_file(&root().join(f)).unwrap();
        models.push(checked.model.expect("the flows pass check"));
    }
    // io.ts and io.py are the same for every flow
    let ts = dandori::temporal::build(&models[0]).unwrap();
    let py = dandori::temporal_py::build(&models[0]).unwrap();
    let dir = scratch("rule-read");
    let io_ts = ts.iter().find(|(n, _)| n.ends_with("/io.ts")).map(|(_, t)| t.clone()).unwrap();
    let io_py = py.iter().find(|(n, _)| n.ends_with("/io.py")).map(|(_, t)| t.clone()).unwrap();
    std::fs::write(dir.join("io.ts"), io_ts).unwrap();
    std::fs::create_dir_all(dir.join("py")).unwrap();
    std::fs::write(dir.join("py/io.py"), io_py).unwrap();

    // the answers: what the service writes, and each field left out, null, of another kind, or a value it has not
    let rules: Vec<(&String, &dandori::model::RuleConnect)> = models.iter().flat_map(|m| m.rules.iter().filter_map(|r| Some((&r.name, r.connect.as_ref()?)))).collect();
    let mut cases: Vec<(usize, Value)> = Vec::new();
    for (ri, (_, c)) in rules.iter().enumerate() {
        let good = |f: &dandori::rulec::WireField| match &f.kind {
            WireKind::Bool => json!(true),
            WireKind::Int => json!("12000"),
            WireKind::Str => json!("x"),
            WireKind::Enum { values, .. } => json!(values[0].1),
        };
        let whole: serde_json::Map<String, Value> = c.response.iter().map(|f| (f.json.clone(), good(f))).collect();
        cases.push((ri, Value::Object(whole.clone())));
        for f in &c.response {
            let variants: Vec<Value> = match &f.kind {
                WireKind::Int => ["12000", "-3", "007", "+5", "1.5", "1e3", "", "-", " 5", "5 ", "5\n", "\n5", "\u{663}", "9007199254740991", "-9007199254740991", "9007199254740992", "0000000000000012", "00000000000000012", "99999999999999999999", "0x10", "1_0", "12.", "--1", "NaN"]
                    .iter()
                    .map(|x| json!(x))
                    .chain([json!(12000), json!(-3), json!(0), json!(1.5), json!(1e300), json!(true), json!([]), json!({}), json!(["12000"])])
                    .collect(),
                WireKind::Enum { zero, values } => [zero.as_str(), "constructor", "__proto__", "toString", "hasOwnProperty", "", "next_day", "carrier_standard"]
                    .iter()
                    .map(|x| json!(x))
                    .chain(values.iter().map(|(rule, _)| json!(rule)))
                    .chain([json!(0), json!(1), json!(true), json!([]), json!({}), json!(["x"])])
                    .collect(),
                WireKind::Bool => vec![json!(false), json!(0), json!(1), json!("true"), json!("false"), json!([]), json!({})],
                WireKind::Str => vec![json!(""), json!(5), json!(true), json!([]), json!({})],
            };
            for v in variants {
                let mut one = whole.clone();
                one.insert(f.json.clone(), v);
                cases.push((ri, Value::Object(one)));
            }
            // left out, and null
            let mut left = whole.clone();
            left.remove(&f.json);
            cases.push((ri, Value::Object(left)));
            let mut null = whole.clone();
            null.insert(f.json.clone(), Value::Null);
            cases.push((ri, Value::Object(null)));
        }
        let mut extra = whole.clone();
        extra.insert("zzz".into(), json!(1));
        cases.push((ri, Value::Object(extra)));
        for body in [json!({}), json!([]), json!(["x"]), json!([{ "a": 1 }]), json!([[]]), json!(null), json!(5), json!("x"), json!(true), json!("{}")] {
            cases.push((ri, body));
        }
    }
    let spec: Vec<Value> = cases
        .iter()
        .map(|(ri, body)| {
            let c = rules[*ri].1;
            json!({ "wire": dandori::render::rule_wire_spec(c), "body": body, "jsonata": dandori::render::jsonata_rule_read(c, "$body") })
        })
        .collect();
    std::fs::write(dir.join("cases.json"), serde_json::to_string(&spec).unwrap()).unwrap();
    let node = Command::new("node").arg(root().join("tools/connect/read.mjs")).arg(dir.join("io.ts")).arg(dir.join("cases.json")).arg(dir.join("ts-jsonata.json")).output().unwrap();
    assert!(node.status.success(), "tools/connect/read.mjs failed:\n{}", String::from_utf8_lossy(&node.stderr));
    let python = Command::new("python3").arg(root().join("tools/connect/read.py")).arg(dir.join("py")).arg(dir.join("cases.json")).arg(dir.join("python.json")).output().unwrap();
    assert!(python.status.success(), "tools/connect/read.py failed:\n{}", String::from_utf8_lossy(&python.stderr));
    let both: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(dir.join("ts-jsonata.json")).unwrap()).unwrap();
    let in_python: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(dir.join("python.json")).unwrap()).unwrap();
    assert_eq!((both.len(), in_python.len()), (cases.len(), cases.len()));
    // the Go of io.go, which is the same for every flow: that of connect_rules.flow
    let in_go: Option<Vec<Value>> = go_runner().map(|go| {
        let out = Command::new(&go.bin).arg("--connect-read").arg(go.key("tests/flows/connect_rules.flow")).arg(dir.join("cases.json")).arg(dir.join("go.json")).output().unwrap();
        assert!(out.status.success(), "the Go runner's --connect-read failed:\n{}", String::from_utf8_lossy(&out.stderr));
        serde_json::from_str(&std::fs::read_to_string(dir.join("go.json")).unwrap()).unwrap()
    });
    let mut wrong = Vec::new();
    for (i, (ri, body)) in cases.iter().enumerate() {
        let want = norm(&dandori::render::rule_read(rules[*ri].1, body));
        let mut readers = vec![("TypeScript", &both[i]["ts"]), ("JSONata", &both[i]["jsonata"]), ("Python", &in_python[i])];
        if let Some(go) = &in_go {
            readers.push(("Go", &go[i]));
        }
        for (what, got) in readers {
            if norm(got) != want {
                wrong.push(format!("the rule {}, the answer {body}\n    the reference reads {want}\n    {what} reads {got}", rules[*ri].0));
            }
        }
    }
    let readers = if in_go.is_some() { 4 } else { 3 };
    assert!(wrong.is_empty(), "{} of {} reading(s) differ:\n{}", wrong.len(), cases.len() * readers, wrong.iter().take(12).cloned().collect::<Vec<_>>().join("\n"));
    eprintln!("{} answer(s) of {} rule(s) were read alike by the reference, TypeScript, Python{} and JSONata", cases.len(), rules.len(), if in_go.is_some() { ", Go" } else { "" });
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
        // the graph has the rules only when it calls one: a rule a flow only follows the machine of is no function
        let called: std::collections::BTreeSet<usize> = m.all_stmts().iter().filter_map(|s| match &s.kind { TK::Call { callee: Callee::Rule(r), .. } => Some(*r), _ => None }).collect();
        // a rule called at its service is a method of the tasks, which send through the stand-in Transport
        let rules: Vec<Value> = m.rules.iter().enumerate().filter(|(i, r)| called.contains(i) && r.connect.is_none()).map(|(_, r)| json!({ "fn": dandori::render::rule_activity(&r.name), "name": r.name })).collect();
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
/// answered, and throw when the model refuses. io.go has no Agents SDK to run, since OpenAI has
/// none for Go: through the Go runner's --agents, an OpenAI agent's call goes to a stand-in of the
/// Responses API with OpenAI's Go client, a Claude agent's to one of the Messages API with
/// Anthropic's Go SDK, and each must get the very request Step Functions sends, once.
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
        // what the default Transport in Go is to give back and send: it has no Agents SDK, and sends an
        // OpenAI agent's call to the Responses API as Step Functions does; an error by its Go type
        let mut expected_go = Vec::new();
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
                        expected_go.push(json!({ "answer": { "answer": ok }, "asked": asked }));
                    }
                    if first {
                        cases.push(json!({ "call": call, "refusal": "I can't help with that." }));
                        expected.push(json!({ "error": "AgentStopped", "asked": asked }));
                        expected_go.push(json!({ "error": "AgentStopped", "asked": asked }));
                        cases.push(json!({ "call": call, "status": 500 }));
                        expected.push(json!({ "error": "AgentHttpError", "asked": asked }));
                        expected_go.push(json!({ "error": "AgentHTTPError", "asked": asked }));
                    }
                    continue;
                }
                if call["provider"] == json!("claude") {
                    // the one request the stand-in of the Messages API must get
                    let asked = json!([{ "method": "POST", "path": "/v1/messages", "version": sent["headers"]["anthropic-version"], "body": sent["body"] }]);
                    if let Some(ok) = answer.get("ok") {
                        cases.push(json!({ "call": call, "text": json!({ "answer": ok }).to_string() }));
                        expected.push(json!({ "answer": { "answer": ok }, "asked": asked }));
                        expected_go.push(json!({ "answer": { "answer": ok }, "asked": asked }));
                    }
                    if first {
                        cases.push(json!({ "call": call, "refusal": "I can't help with that." }));
                        expected.push(json!({ "error": "AgentStopped", "asked": asked }));
                        expected_go.push(json!({ "error": "AgentStopped", "asked": asked }));
                        cases.push(json!({ "call": call, "status": 500 }));
                        expected.push(json!({ "error": "InternalServerError", "asked": asked }));
                        expected_go.push(json!({ "error": "Error", "asked": asked }));
                    }
                    continue;
                }
                let asked = json!({
                    "models": [call["model"]],
                    "calls": 1,
                    "instructions": call["instructions"],
                    "input": sent["body"]["input"],
                    "outputType": { "type": "json_schema", "name": "answer", "strict": true, "schema": call["schema"] },
                    // nothing but the effort the task asks for, as Step Functions sends it
                    "modelSettings": match sent["body"].get("reasoning") {
                        Some(r) => json!({ "reasoning": r }),
                        None => json!({}),
                    },
                    "tools": []
                });
                // what Go sends: Step Functions' request, to OpenAI's path
                let asked_go = json!([{ "method": "POST", "path": "/v1/responses", "body": sent["body"] }]);
                if let Some(ok) = answer.get("ok") {
                    cases.push(json!({ "call": call, "text": json!({ "answer": ok }).to_string() }));
                    expected.push(json!({ "answer": { "answer": ok }, "asked": asked }));
                    expected_go.push(json!({ "answer": { "answer": ok }, "asked": asked_go }));
                }
                if first {
                    cases.push(json!({ "call": call, "refusal": "I can't help with that." }));
                    expected.push(json!({ "error": "ModelRefusalError", "asked": asked }));
                    expected_go.push(json!({ "error": "AgentStopped", "asked": asked_go }));
                    // the client the Transport makes sends the request once
                    cases.push(json!({ "call": call, "status": 500 }));
                    expected.push(json!({ "error": "InternalServerError", "asked": [{ "method": "POST", "path": "/v1/responses" }] }));
                    expected_go.push(json!({ "error": "Error", "asked": asked_go }));
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
        let check = |what: &str, got: Vec<Value>, expected: &Vec<Value>| {
            assert_eq!(got.len(), expected.len(), "{}: {what} answered {} case(s) of {}", rel(&f), got.len(), expected.len());
            for (i, (g, e)) in got.iter().zip(expected).enumerate() {
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
            check("the default Transport in TypeScript", read(&results), &expected);
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
            check("the default Transport in Python", read(&results), &expected);
            eprintln!("{}: the default Transport in Python asked what Step Functions asks in {} case(s)", rel(&f), cases.len());
        }
        if let Some(go) = go_runner() {
            let results = dir.join("results-go.json");
            let out = Command::new(&go.bin).arg("--agents").arg(go.key(&rel(&f))).arg(&cases_file).arg(&results).output().unwrap();
            assert!(out.status.success(), "{}: the Go check of the agents failed:\n{}", rel(&f), String::from_utf8_lossy(&out.stderr));
            // Go's maps keep no order, so Go writes the input's JSON text with its keys in the order of their
            // names: the text the model reads is compared as the JSON it is
            fn as_json(v: &Value) -> Value {
                let mut v = v.clone();
                for asked in v["asked"].as_array_mut().into_iter().flatten() {
                    if let Some(text) = asked["body"]["input"].as_str().map(str::to_string) {
                        asked["body"]["input"] = serde_json::from_str(&text).unwrap_or(json!(text));
                    }
                    for m in asked["body"]["messages"].as_array_mut().into_iter().flatten() {
                        if let Some(text) = m["content"].as_str().map(str::to_string) {
                            m["content"] = serde_json::from_str(&text).unwrap_or(json!(text));
                        }
                    }
                }
                v
            }
            let got: Vec<Value> = serde_json::from_str::<Vec<Value>>(&std::fs::read_to_string(&results).unwrap()).unwrap().iter().map(as_json).collect();
            let expected_go: Vec<Value> = expected_go.iter().map(as_json).collect();
            check("the default Transport in Go", got, &expected_go);
            eprintln!("{}: the default Transport in Go asked what Step Functions asks in {} case(s)", rel(&f), cases.len());
        }
    }
}

/// The agents on a server of Open Responses (`url`), sent for real to Ollama on this machine by
/// the default Transport of TypeScript, Python and Go: for each such agent, the arguments of the first call
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
                // the local model may not reason at all, and Ollama refuses an effort for such a model
                // ("does not support thinking"); the request's effort is held to Step Functions' on the
                // stand-in server instead
                if let Some(o) = live.as_object_mut() {
                    o.remove("effort");
                }
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
        if let Some(go) = go_runner() {
            let results = dir.join("results-go.json");
            let out = Command::new(&go.bin).arg("--agents").arg(go.key(&rel(&f))).arg(&cases_file).arg(&results).output().unwrap();
            assert!(out.status.success(), "{}: the Go check of the agents failed:\n{}", rel(&f), String::from_utf8_lossy(&out.stderr));
            check("Go", &results);
            sent += cases.len();
        }
        eprintln!("{}: {} agent(s) on a server of Open Responses answered from Ollama {} ({model}) as their types say, from the default Transport of TypeScript, of Python and of Go", rel(&f), cases.len(), version["version"].as_str().unwrap_or("?"));
    }
    assert!(sent > 0, "no agent on a server of Open Responses was sent to Ollama");
}

/// Every Jev task of the examples and of tests/flows, asked of the real Jev when TYPESAFE_API_KEY is
/// set: the first call of each that the scenarios make, sent to TypeSafe's API by the default
/// Transport of TypeScript, of Python and of Go, which add the key, and read with their io.jev
/// (Go's ddJev). The
/// response must be the API's shape, with an answer to every question, and it must read into the
/// task's type, or fail the call with the task's own error when Jev is less sure than the task
/// asks. What Jev answered, and how sure it was, is printed. It costs next to nothing: the API
/// charges $0.042 a million input tokens, and a call here takes a few hundred.
#[test]
fn jev_tasks_answer_on_typesafe() {
    need_rulec!();
    if std::env::var("TYPESAFE_API_KEY").map_or(true, |k| k.is_empty()) {
        eprintln!("SKIP: TYPESAFE_API_KEY is not set; the Jev tasks are not sent to TypeSafe");
        return;
    }
    let python = Command::new("python3").arg("--version").output().is_ok_and(|o| o.status.success());
    let mut sent = 0;
    for f in runnable() {
        let (_, checked) = dandori::check::check_file(&f).unwrap();
        let m = checked.model.expect("the flows pass check");
        // the first call of each Jev task, as the scenarios make it
        let mut cases = Vec::new();
        let mut tasks: Vec<&dandori::model::TaskDef> = Vec::new();
        for sc in dandori::scenarios::generate(&m) {
            let r = dandori::interp::run(&m, &sc, View::Temporal).unwrap();
            for s in r["steps"].as_array().unwrap() {
                let call = &s["call"];
                if call["url"].as_str() != Some(dandori::model::JEV_URL) {
                    continue;
                }
                let Some(task) = m.tasks.iter().find(|t| t.jev().is_some_and(|j| dandori::render::jev_questions(j) == call["body"]["questions"])) else { continue };
                if tasks.iter().any(|t| t.name == task.name) {
                    continue;
                }
                cases.push(json!({ "request": call, "spec": dandori::render::jev_spec(task, task.jev().unwrap()) }));
                tasks.push(task);
            }
        }
        if cases.is_empty() {
            continue;
        }
        let dir = scratch(&format!("typesafe-{}", key(&f)));
        let cases_file = dir.join("cases.json");
        std::fs::write(&cases_file, serde_json::to_string(&cases).unwrap()).unwrap();
        let check = |lang: &str, results: &Path| {
            let got: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(results).unwrap()).unwrap();
            for (g, t) in got.iter().zip(&tasks) {
                let j = t.jev().unwrap();
                let at = || format!("{}: `{}` at TypeSafe, from the default Transport in {lang}", rel(&f), t.name);
                assert_eq!(g["status"], json!(200), "{}: {}", at(), serde_json::to_string(&g["body"]).unwrap());
                assert!(g["body"]["model"].is_string(), "{}: the response names no model: {}", at(), g["body"]);
                for q in &j.questions {
                    assert!(g["body"]["answers"][&q.id].is_object(), "{}: no answer to `{}`: {}", at(), q.id, g["body"]);
                }
                match (&g["error"], j.floor.as_ref()) {
                    (Value::Null, _) => assert!(
                        dandori::render::value_fits(&m, &g["value"], t.result.as_ref().unwrap(), t.result_range),
                        "{}: Jev's answer does not read into the task's type: {}",
                        at(),
                        serde_json::to_string(g).unwrap()
                    ),
                    (e, Some((_, error))) if e["kind"] == json!(error) => {}
                    (e, _) => panic!("{}: reading the answer failed with {e}: {}", at(), g["body"]),
                }
                // what Jev answered, for a person to look at
                let sure: Vec<String> = j
                    .questions
                    .iter()
                    .map(|q| {
                        let a = &g["body"]["answers"][&q.id];
                        let said = a.get("choice").or(a.get("score")).or(a.get("noul")).cloned().unwrap_or(Value::Null);
                        match a.get("confidence") {
                            Some(c) => format!("{} {said} (confidence {c})", q.id),
                            None => format!("{} {said}", q.id),
                        }
                    })
                    .collect();
                let outcome = if g["error"].is_null() { format!("reads as {}", g["value"]) } else { format!("fails with {}", g["error"]["kind"]) };
                eprintln!("{}: `{}` ({lang}, {} ms, {}): {}; {outcome}", rel(&f), t.name, g["ms"], g["body"]["model"].as_str().unwrap_or("?"), sure.join(", "));
            }
        };
        let ts = dandori::temporal::build(&m).unwrap();
        let io_ts = dir.join("io.ts");
        std::fs::write(&io_ts, &ts.iter().find(|(n, _)| n.ends_with("/io.ts")).unwrap().1).unwrap();
        let results = dir.join("results-ts.json");
        let out = Command::new("node").arg("--no-warnings").arg(root().join("tools/jev/check.mjs")).arg(&io_ts).arg(&cases_file).arg(&results).output().unwrap();
        assert!(out.status.success(), "{}: tools/jev/check.mjs failed:\n{}", rel(&f), String::from_utf8_lossy(&out.stderr));
        check("TypeScript", &results);
        sent += cases.len();
        if python {
            let py = dandori::temporal_py::build(&m).unwrap();
            let io_py = dir.join("io.py");
            std::fs::write(&io_py, &py.iter().find(|(n, _)| n.ends_with("/io.py")).unwrap().1).unwrap();
            let results = dir.join("results-py.json");
            let out = Command::new("python3").arg(root().join("tools/jev/check.py")).arg(&io_py).arg(&cases_file).arg(&results).output().unwrap();
            assert!(out.status.success(), "{}: tools/jev/check.py failed:\n{}", rel(&f), String::from_utf8_lossy(&out.stderr));
            check("Python", &results);
            sent += cases.len();
        } else {
            eprintln!("SKIP: python3 is missing; the Jev tasks are not sent from Python");
        }
        if let Some(go) = go_runner() {
            let results = dir.join("results-go.json");
            let out = Command::new(&go.bin).arg("--jev").arg(go.key(&rel(&f))).arg(&cases_file).arg(&results).output().unwrap();
            assert!(out.status.success(), "{}: the Go check of Jev failed:\n{}", rel(&f), String::from_utf8_lossy(&out.stderr));
            check("Go", &results);
            sent += cases.len();
        }
    }
    assert!(sent > 0, "no Jev task was sent to TypeSafe");
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
            // only the rules whose code goes with the workflow are in rules.py
            let mut called: Vec<usize> = m.all_stmts().iter().filter_map(|s| match &s.kind { TK::Call { callee: Callee::Rule(r), .. } if m.rules[*r].connect.is_none() => Some(*r), _ => None }).collect();
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
                // a rule called at its service is no Lambda function: the generated step sends through the stand-in Transport
                if m.rules[*r].connect.is_some() {
                    continue;
                }
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
    /// what the runner said when it ended
    runner: std::process::Output,
}

/// How many flows' runners go at once on Argo. Every run of a flow goes at the same time, and the
/// kind cluster's API server began to time requests out when the runners of more than twenty flows
/// did; fourteen went through together.
const ARGO_AT_ONCE: usize = 14;
static ARGO_SLOTS: (std::sync::Mutex<usize>, std::sync::Condvar) = (std::sync::Mutex::new(0), std::sync::Condvar::new());

/// Wait for a slot among the flows going at once on Argo, and hold it while `f` runs.
fn with_argo_slot<T>(f: impl FnOnce() -> T) -> T {
    let (lock, freed) = &ARGO_SLOTS;
    let mut going = freed.wait_while(lock.lock().unwrap_or_else(|e| e.into_inner()), |n| *n >= ARGO_AT_ONCE).unwrap_or_else(|e| e.into_inner());
    *going += 1;
    drop(going);
    let out = f();
    *lock.lock().unwrap_or_else(|e| e.into_inner()) -= 1;
    freed.notify_one();
    out
}

/// Make a flow ready for Argo (its scenarios, the reference's runs, the build) and run the runner
/// on it, when a slot is free, to its end; None for a flow that is not for Argo.
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
    // the rules called at their services are run by the caller as they are generated, and send through the stand-in Transport
    let connect_rules: Vec<String> = m.rules.iter().filter(|r| r.connect.is_some()).map(|r| dandori::render::rule_activity(&r.name)).collect();
    let spec = json!({ "template": doc, "own": own, "children": children, "connectRules": connect_rules, "http": http, "aws": aws, "runs": runs, "real": [real] });
    std::fs::write(&runs_file, serde_json::to_string(&spec).unwrap()).unwrap();
    let runner = with_argo_slot(|| {
        Command::new("node")
            .arg(root().join("tools/argo/run.mjs"))
            .arg(&dir)
            .arg(&runs_file)
            .arg(&results)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .output()
            .unwrap()
    });
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
    // runs one of them again with real pods; each flow is made ready and run on a thread of its own,
    // ARGO_AT_ONCE of them at a time, and every runner ends, and takes its workflows away, before
    // any is looked at
    let going: Vec<ArgoFlow> = std::thread::scope(|scope| {
        let ready: Vec<_> = runnable_on(Platform::Argo).into_iter().map(|f| scope.spawn(move || ready_on_argo(f))).collect();
        ready.into_iter().filter_map(|h| h.join().unwrap()).collect()
    });
    for flow in going {
        let f = &flow.f;
        let out = &flow.runner;
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
