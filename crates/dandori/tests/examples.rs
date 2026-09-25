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
/// the corners of the language.
fn runnable() -> Vec<PathBuf> {
    let mut out = flows(&root().join("examples"));
    out.extend(flows(&root().join("tests/flows")));
    out
}

fn rel(p: &Path) -> String {
    p.strip_prefix(root()).unwrap_or(p).display().to_string()
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
        for (lang, tag) in [(Lang::En, "en"), (Lang::Ja, "ja")] {
            let text: String = checked.diags.iter().map(|d| d.render(&rel(&f), &src, lang)).collect();
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
    for f in runnable() {
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
        let dir = scratch(&dandori::render::ident(&m.name));
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
/// trace; `keep` says which runs the platform's test environment can play.
fn plays(m: &Model, view: View, keep: impl Fn(&[CallInfo]) -> bool) -> (Vec<Value>, Vec<(Value, Value)>, usize) {
    let mut runs = Vec::new();
    let mut references = Vec::new();
    let mut left_out = 0;
    for sc in dandori::scenarios::generate(m) {
        let (reference, calls) = dandori::interp::run_traced(m, &sc, view).unwrap();
        if !keep(&calls) {
            left_out += 1;
            continue;
        }
        let answers: Vec<Value> = reference["steps"].as_array().unwrap().iter().filter(|s| s.get("call").is_some()).map(|s| s["answer"].clone()).collect();
        let script: Vec<Value> = answers.iter().map(|a| if a.get("ok").is_some() { json!({ "ok": a["ok"] }) } else { json!({ "error": a["error"] }) }).collect();
        runs.push(json!({ "input": sc["input"], "answers": script }));
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
    for f in runnable() {
        let (_, checked) = dandori::check::check_file(&f).unwrap();
        let m = checked.model.expect("the examples pass check");
        let dir = scratch(&format!("temporal-{}", dandori::render::ident(&m.name)));
        let files = dandori::temporal::build(&m).unwrap_or_else(|d| panic!("{} does not build: {}", rel(&f), d[0].en));
        for (name, text) in &files {
            let p = dir.join(name);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, text).unwrap();
        }
        let p = Platform::Temporal;
        let own: Vec<Value> = m.tasks.iter().filter(|t| t.via(p) == Some(Via::Own)).map(|t| json!({ "name": t.name, "method": dandori::render::ident(&t.name), "callback": t.callback })).collect();
        let rules: Vec<String> = m.rules.iter().map(|r| dandori::render::rule_activity(&r.name)).collect();
        let children: Vec<Value> = m.tasks.iter().filter_map(|t| t.workflow.as_ref().map(|w| json!({ "type": w, "queue": t.queue }))).collect();
        let mut queues: Vec<String> = m.tasks.iter().filter_map(|t| t.queue.clone()).collect();
        queues.sort();
        queues.dedup();
        let (http, aws) = transport_spec(&m, p);
        // an activity's timeout cannot be scripted in the test environment; a callback's can, by not answering
        let (runs, references, left_out) = plays(&m, View::Temporal, |calls| calls.iter().all(|c| c.kind.as_deref() != Some("timeout") || c.callback));
        let runs_file = dir.join("runs.json");
        let results_file = dir.join("results.json");
        let spec = json!({ "workflow": dandori::render::ident(&m.name), "own": own, "rules": rules, "children": children, "queues": queues, "http": http, "aws": aws, "runs": runs });
        std::fs::write(&runs_file, serde_json::to_string(&spec).unwrap()).unwrap();
        let out = Command::new("node")
            .arg(root().join("tools/temporal/run.mjs"))
            .arg(dir.join(dandori::render::ident(&m.name)))
            .arg(&runs_file)
            .arg(&results_file)
            .output()
            .unwrap();
        assert!(out.status.success(), "{}: the Temporal runner failed:\n{}", rel(&f), String::from_utf8_lossy(&out.stderr));
        let got: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(&results_file).unwrap()).unwrap();
        compare("the Temporal workflow", &f, &references, &got);
        eprintln!("{}: compared {} run(s) on Temporal; left out {left_out} with an activity's timeout", rel(&f), references.len());
    }
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
    for f in runnable() {
        let (_, checked) = dandori::check::check_file(&f).unwrap();
        let m = checked.model.expect("the examples pass check");
        let dir = scratch(&format!("durable-{}", dandori::render::ident(&m.name)));
        let files = dandori::temporal::build_flavor(&m, dandori::temporal::Flavor::Durable).unwrap_or_else(|d| panic!("{} does not build: {}", rel(&f), d[0].en));
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
        let (runs, references, left_out) = plays(&m, View::Durable, |calls| calls.iter().all(|c| c.kind.as_deref() != Some("timeout")));
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
