//! A variable read where the check says it has a value (E012, E013), which a flow that passes the
//! check never reads without one (DESIGN 38). Past the check, the run fails with
//! Dandori.UnexpectedValue, `line <n>: <name> has no value`, the line of the statement that reads
//! it: the reference interpreter's run of a model whose statements that set the variables are
//! taken away, the state machine of the same model run by tools/asl-run.mjs, and the code of
//! temporal-typescript, durable, temporal-python, pydantic-graph and temporal-go, whose helpers the
//! tests call as they are. A variable of `T?` holds null as a value and reads as null. tests/unset
//! holds the flow, in English and in Japanese.

use dandori::model::{Model, TStmt, Target, TK};
use dandori::render::View;
use ritsu_testkit::{ready, Need, TempDir};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Duration;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn said(out: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr))
}

/// The checked model of tests/unset/`file`.
fn model(file: &str) -> Model {
    let path = root().join("tests/unset").join(file);
    let src = std::fs::read_to_string(&path).unwrap();
    let c = dandori::check::check_source(&src, &path);
    c.model.unwrap_or_else(|| panic!("{file} passes check: {:?}", c.diags.iter().map(|d| &d.en).collect::<Vec<_>>()))
}

/// The statement that sets `name`, taken out of `ss` at any depth: what the check does not let a
/// flow do.
fn take_away(ss: &mut Vec<TStmt>, name: &str) -> bool {
    if let Some(i) = ss.iter().position(|s| match &s.kind {
        TK::Assign { name: n, .. } => n == name,
        TK::Call { target: Some(Target::Let(n)), .. } => n == name,
        _ => false,
    }) {
        ss.remove(i);
        return true;
    }
    ss.iter_mut().any(|s| match &mut s.kind {
        TK::For { body, .. } => take_away(body, name),
        _ => false,
    })
}

/// The flow of `file` without the statement that sets `name`, or as it is.
fn without(file: &str, name: Option<&str>) -> Model {
    let mut m = model(file);
    if let Some(n) = name {
        assert!(take_away(&mut m.flow, n), "{file} sets {n}");
    }
    m
}

/// The input every run of the flow of `file` is given.
fn input(file: &str) -> Value {
    if file.ends_with(".ja.flow") { json!({ "番号": "a" }) } else { json!({ "id": "a" }) }
}

/// How the reference interpreter ends `m`, the flow of `file`, every call answered with `answers`.
fn reference_end(m: &Model, file: &str, answers: Value, view: View) -> Value {
    let sc = json!({ "input": input(file), "answers": answers });
    dandori::interp::run(m, &sc, view).unwrap()["end"].clone()
}

/// How the reference interpreter ends the flow of `file` without the statement that sets `name`,
/// every call answered with `answer`.
fn ends(file: &str, name: &str, answers: Value) -> Value {
    reference_end(&without(file, Some(name)), file, answers, View::Temporal)
}

fn failed(cause: &str) -> Value {
    json!({ "fail": { "error": "Dandori.UnexpectedValue", "cause": cause } })
}

/// The reference interpreter: a read with fields, a read of the whole value, a `yield` of a round
/// in parallel and of one after the other, each where the variable has no value, fails the run at
/// the line of the statement; a variable of `T?` with no value reads as null, and the run ends as
/// it would have.
#[test]
fn the_reference_fails_a_read_of_a_variable_with_no_value() {
    for (file, order, names) in [
        ("unset.flow", json!({ "id": "o-1" }), ["order", "text", "n", "k", "maybe", "order.id", "text", "notes", "kept"]),
        ("unset.ja.flow", json!({ "番号": "o-1" }), ["受注", "文", "n", "k", "あれば", "受注.番号", "文", "メモの一覧", "残したもの"]),
    ] {
        let [order_var, text, n, k, maybe, order_field, text_out, notes, kept] = names;
        // `let text = note(id: order.id)`, line 27, reads `order` with a field (the answers are for
        // a run that went on)
        let answers = json!([{ "ok": "t" }, { "ok": "n-1" }, { "ok": "k-1" }]);
        assert_eq!(ends(file, order_var, answers), failed(&format!("line 27: {order_var} has no value")), "{file}: {order_field}");
        // `succeed text = text, …`, line 35, reads `text` as a whole
        let answers = json!([{ "ok": order }, { "ok": "n-1" }, { "ok": "k-1" }]);
        assert_eq!(ends(file, text, answers), failed(&format!("line 35: {text} has no value")), "{file}: {text_out}");
        // the round in parallel fails, and the run with it, at the line of its `for` (29)
        let answers = json!([{ "ok": order }, { "ok": "t" }]);
        assert_eq!(ends(file, n, answers), failed(&format!("line 29: {n} has no value")), "{file}: {notes}");
        // the round of a `for` one after the other fails the run at the line of its `for` (32)
        let answers = json!([{ "ok": order }, { "ok": "t" }, { "ok": "n-1" }]);
        assert_eq!(ends(file, k, answers), failed(&format!("line 32: {k} has no value")), "{file}: {kept}");
        // a variable of `string?` with no value is null, which `keep` takes
        let answers = json!([{ "ok": order }, { "ok": "t" }, { "ok": "n-1" }, { "ok": "k-1" }]);
        let end = ends(file, maybe, answers);
        assert!(end.get("succeed").is_some(), "{file}: {maybe} with no value reads as null: {end}");
    }
}

/// How long tools/asl-run.mjs may take to run a state machine: well under a second now.
const ASL_RUN: Duration = Duration::from_secs(60);

/// How the state machine of `m`, the flow of `file`, ends, run by tools/asl-run.mjs under `dir` as
/// `stem` with every call answered with `answers`.
fn asl_end(m: &Model, file: &str, stem: &str, answers: &Value, dir: &Path) -> Value {
    let files = dandori::asl::build(m).unwrap_or_else(|d| panic!("{file} does not build for Step Functions: {:?}", d.iter().map(|x| &x.en).collect::<Vec<_>>()));
    let def = dir.join(format!("{stem}.asl.json"));
    std::fs::write(&def, &files[0].1).unwrap();
    let run = dir.join(format!("{stem}.run.json"));
    std::fs::write(&run, serde_json::to_string(&json!({ "input": input(file), "answers": answers })).unwrap()).unwrap();
    let mut node = Command::new("node");
    node.arg(root().join("tools/asl-run.mjs")).arg(&def).arg(&run).arg("test");
    let ran = ritsu_testkit::run::finish(&mut node, ASL_RUN, &format!("node tools/asl-run.mjs {stem}.asl.json {stem}.run.json test"));
    assert!(ran.ok, "{stem}: the runner failed:\n{}", ran.both());
    serde_json::from_str::<Value>(&ran.stdout).unwrap()["end"].clone()
}

/// The state machine checks the variables a statement reads before the statement, and ends as the
/// reference interpreter does: a read with fields, a read of the whole value, a `yield` of a round
/// in parallel and of one after the other, each where the variable has no value, fail the run with
/// Dandori.UnexpectedValue at the line of the statement, and `on failure` does not run. A variable
/// of `T?` with no value reads as null, and the flow as it is runs as it did.
#[test]
fn the_state_machine_ends_a_read_of_a_variable_with_no_value_as_the_reference_does() {
    let has = || root().join("tools/node_modules/jsonata").exists() && Command::new("node").arg("--version").output().is_ok_and(|o| o.status.success());
    if !ready(Need::Node, has, "node, or tools/node_modules/jsonata, is missing; the state machines are not run") {
        return;
    }
    let scratch = TempDir::new("dandori-unset-asl");
    for (file, order, names) in [
        ("unset.flow", json!({ "id": "o-1" }), ["order", "text", "n", "k", "maybe"]),
        ("unset.ja.flow", json!({ "番号": "o-1" }), ["受注", "文", "n", "k", "あれば"]),
    ] {
        let [order_var, text, n, k, maybe] = names;
        let all = json!([{ "ok": order }, { "ok": "t" }, { "ok": "n-1" }, { "ok": "k-1" }]);
        for (name, answers, cause) in [
            (Some(order_var), json!([{ "ok": "t" }, { "ok": "n-1" }, { "ok": "k-1" }]), Some(format!("line 27: {order_var} has no value"))),
            (Some(text), json!([{ "ok": order }, { "ok": "n-1" }, { "ok": "k-1" }]), Some(format!("line 35: {text} has no value"))),
            (Some(n), json!([{ "ok": order }, { "ok": "t" }]), Some(format!("line 29: {n} has no value"))),
            (Some(k), json!([{ "ok": order }, { "ok": "t" }, { "ok": "n-1" }]), Some(format!("line 32: {k} has no value"))),
            (Some(maybe), all.clone(), None),
            (None, all.clone(), None),
        ] {
            let m = without(file, name);
            let stem = format!("{}-{}", file.trim_end_matches(".flow"), name.unwrap_or("as-it-is"));
            let reference = reference_end(&m, file, answers.clone(), View::Asl);
            match &cause {
                Some(c) => assert_eq!(reference, failed(c), "{stem}: the reference"),
                None => assert!(reference.get("succeed").is_some(), "{stem}: the reference: {reference}"),
            }
            assert_eq!(asl_end(&m, file, &stem, &answers, scratch.path()), reference, "{stem}");
        }
    }
}

/// The code of `flow` for `target`, written under `dir`: the directory of its package.
fn written(flow: &str, target: &str, dir: &Path) -> PathBuf {
    let m = model(flow);
    let files = dandori::commands::build(&m, target).unwrap().unwrap_or_else(|d| panic!("{flow} --target {target}: {d:?}"));
    for (name, text) in &files {
        let p = dir.join(name);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    let package = files[0].0.split('/').next().unwrap().to_string();
    dir.join(package)
}

/// What the helpers say, line by line, as each language's own code calls them: `have` of a value,
/// `have` of nothing, and `attempt` (with a retrier for every kind of error) of a call that reads
/// nothing.
const SAID: &str = "a\nDandori.UnexpectedValue true line 12: order has no value\nattempt Dandori.UnexpectedValue line 12: order has no value\n";

const PY_TEMPORAL: &str = r#"
import asyncio
from temporalio.exceptions import ApplicationError
import unset.runtime as dd

print(dd.have("a", "order", 12))
try:
    dd.have(None, "order", 12)
    print("no error")
except ApplicationError as e:
    print(e.type, str(e.non_retryable).lower(), e.message)


async def call() -> object:
    return dd.have(None, "order", 12)

try:
    asyncio.run(dd.attempt(call, [{"on": ["*"], "max": 3, "every": 1, "backoff": 2}], []))
    print("no error")
except dd.TaskError as e:
    print("task error", e.kind, e.message)
except ApplicationError as e:
    print("attempt", e.type, e.message)
"#;

const PY_GRAPH: &str = r#"
import asyncio
import unset.runtime as dd

print(dd.have("a", "order", 12))
try:
    dd.have(None, "order", 12)
    print("no error")
except dd.Failure as e:
    print(e.error, "true", e.cause)


async def call() -> object:
    return dd.have(None, "order", 12)


class Clock:
    async def sleep(self, seconds: float) -> None:
        raise RuntimeError("waited")

try:
    asyncio.run(dd.attempt(call, [{"on": ["*"], "max": 3, "every": 1, "backoff": 2}], [], Clock()))
    print("no error")
except dd.TaskError as e:
    print("task error", e.kind, e.message)
except dd.Failure as e:
    print("attempt", e.error, e.cause)
"#;

/// The Python of temporal-python and pydantic-graph: `have` raises Dandori.UnexpectedValue (not
/// retried, on Temporal), and `attempt` lets it through.
#[test]
fn the_python_helpers_fail_a_read_of_nothing() {
    for (target, venv, script) in [("temporal-python", "temporal-python", PY_TEMPORAL), ("pydantic-graph", "pydantic-graph", PY_GRAPH)] {
        let python = root().join("tools").join(venv).join(".venv/bin/python");
        if !ready(Need::Python, || python.exists(), &format!("tools/{venv}/.venv is missing")) {
            return;
        }
        let scratch = TempDir::new("dandori-unset-py");
        let package = written("unset.flow", target, scratch.path());
        let out = Command::new(&python).current_dir(package.parent().unwrap()).arg("-I").arg("-c").arg(format!("import sys; sys.path.insert(0, '.')\n{script}")).output().unwrap();
        assert!(out.status.success(), "{target}: {}", said(&out));
        assert_eq!(String::from_utf8_lossy(&out.stdout), SAID, "{target}: {}", said(&out));
    }
}

const TS_TEMPORAL: &str = r#"
const dd = require("./js/runtime.js");
const { ApplicationFailure } = require("@temporalio/common");
const out = [];
out.push(String(dd.have("a", "order", 12)));
try {
  dd.have(null, "order", 12);
  out.push("no error");
} catch (e) {
  out.push(`${e instanceof ApplicationFailure ? e.type : "not a failure"} ${e.nonRetryable} ${e.message}`);
}
(async () => {
  try {
    await dd.attempt(async () => dd.have(null, "order", 12), [{ on: ["*"], max: 3, every: 1, backoff: 2 }], []);
    out.push("no error");
  } catch (e) {
    out.push(e instanceof dd.TaskError ? `task error ${e.kind} ${e.message}` : `attempt ${e.type} ${e.message}`);
  }
  console.log(out.join("\n"));
})();
"#;

const TS_DURABLE: &str = r#"
const dd = require("./js/runtime.js");
const { DurableOperationError } = require("@aws/durable-execution-sdk-js");
const out = [];
out.push(String(dd.have("a", "order", 12)));
try {
  dd.have(null, "order", 12);
  out.push("no error");
} catch (e) {
  out.push(`${e instanceof dd.Failure ? e.errorType : "not a failure"} ${e instanceof dd.Unexpected} ${e.message}`);
}
const context = { wait: async () => { throw new Error("waited"); } };
(async () => {
  // as it comes, and as a step that threw it fails (and a replay reads it again)
  const step = DurableOperationError.fromErrorObject({ ErrorType: "Dandori.UnexpectedValue", ErrorMessage: "line 12: order has no value" });
  const seen = [];
  for (const call of [async () => dd.have(null, "order", 12), async () => { throw step; }]) {
    try {
      await dd.attempt(context, "12 note", call, [{ on: ["*"], max: 3, every: 1, backoff: 2 }], []);
      seen.push("no error");
    } catch (e) {
      seen.push(e instanceof dd.TaskError ? `task error ${e.kind} ${e.message}` : `attempt ${e.errorType} ${e.message}`);
    }
  }
  if (seen[0] !== seen[1]) seen[0] += ` / from a step: ${seen[1]}`;
  out.push(seen[0]);
  console.log(out.join("\n"));
})();
"#;

/// The TypeScript of temporal-typescript and durable: `have` throws Dandori.UnexpectedValue (an
/// ApplicationFailure, not retried, on Temporal), and `attempt` lets it through; on durable, also
/// as a step that threw it fails.
#[test]
fn the_typescript_helpers_fail_a_read_of_nothing() {
    for (target, tools) in [("temporal-typescript", "temporal"), ("durable", "durable")] {
        let modules = root().join("tools").join(tools).join("node_modules");
        let tsc = root().join("tools/temporal/node_modules/.bin/tsc");
        let has = || modules.exists() && tsc.exists() && Command::new("node").arg("--version").output().is_ok_and(|o| o.status.success());
        if !ready(Need::Node, has, &format!("node, or tools/{tools}/node_modules, is missing")) {
            return;
        }
        let scratch = TempDir::new("dandori-unset-ts");
        let package = written("unset.flow", target, scratch.path());
        std::os::unix::fs::symlink(&modules, package.join("node_modules")).unwrap();
        let out = Command::new(&tsc)
            .current_dir(&package)
            .args(["--strict", "--skipLibCheck", "--target", "es2022", "--module", "nodenext", "--moduleResolution", "nodenext", "--types", "node", "--typeRoots"])
            .arg(root().join("tools/temporal/node_modules/@types"))
            .args(["--outDir", "js", "runtime.ts"])
            .output()
            .unwrap();
        assert!(out.status.success(), "{target}: tsc: {}", said(&out));
        std::fs::write(package.join("check.cjs"), if target == "durable" { TS_DURABLE } else { TS_TEMPORAL }).unwrap();
        let out = Command::new("node").current_dir(&package).arg("check.cjs").output().unwrap();
        assert!(out.status.success(), "{target}: {}", said(&out));
        assert_eq!(String::from_utf8_lossy(&out.stdout), SAID, "{target}: {}", said(&out));
    }
}

const GO_TEST: &str = r#"package unset

import (
	"errors"
	"testing"

	"go.temporal.io/sdk/temporal"
	"go.temporal.io/sdk/workflow"
)

// said is how err ends a workflow: its type, whether it is retried, and its message.
func said(t *testing.T, err error) string {
	var app *temporal.ApplicationError
	if !errors.As(err, &app) {
		t.Fatalf("not an application error: %v", err)
	}
	r := "true"
	if !app.NonRetryable() {
		r = "false"
	}
	return app.Type() + " " + r + " " + app.Message()
}

func TestHave(t *testing.T) {
	if v := ddHave("a", "order", 12); v != "a" {
		t.Fatalf("ddHave of a value: %v", v)
	}
	want := "Dandori.UnexpectedValue true line 12: order has no value"
	err := func() (err error) {
		defer ddRecover(&err)
		ddHave(nil, "order", 12)
		return errors.New("no error")
	}()
	if got := said(t, err); got != want {
		t.Fatalf("ddHave of nil, in ddFlow: %s", got)
	}
	_, err = ddRound(nil, func(ctx workflow.Context, item any, index int) (any, error) {
		return ddHave(nil, "order", 12), nil
	}, "a", 0)
	if err == nil {
		t.Fatalf("ddHave of nil, in a round: no error")
	}
	if got := said(t, err); got != want {
		t.Fatalf("ddHave of nil, in a round: %s", got)
	}
}
"#;

/// The Go of temporal-go: `ddHave` panics with the fault, which ddFlow and a round in parallel end
/// with as Dandori.UnexpectedValue, not retried.
#[test]
fn the_go_helpers_fail_a_read_of_nothing() {
    let has = || Command::new("go").arg("version").output().is_ok_and(|o| o.status.success()) && root().join("tools/temporal-go/go.mod").exists();
    if !ready(Need::Go, has, "go or tools/temporal-go is missing") {
        return;
    }
    let scratch = TempDir::new("dandori-unset-go");
    let dir = scratch.path().join("m");
    let tools = root().join("tools/temporal-go");
    let gomod = std::fs::read_to_string(tools.join("go.mod")).unwrap();
    let gomod = gomod.lines().map(|l| if l.starts_with("module ") { "module unsetcheck".to_string() } else { l.to_string() }).collect::<Vec<_>>().join("\n") + "\n";
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("go.mod"), gomod).unwrap();
    std::fs::copy(tools.join("go.sum"), dir.join("go.sum")).unwrap();
    let package = written("unset.flow", "temporal-go", &dir.join("flows"));
    std::fs::write(package.join("have_test.go"), GO_TEST).unwrap();
    let out = Command::new("go").current_dir(&dir).env("GOWORK", "off").env("GOFLAGS", "-trimpath").args(["test", "-count=1", "./flows/unset/"]).output().unwrap();
    assert!(out.status.success(), "go test: {}", said(&out));
}
