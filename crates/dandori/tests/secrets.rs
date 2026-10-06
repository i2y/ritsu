//! What a flow does with secrets (DESIGN 1.18): the check of the fixture that follows a secret from
//! each of its marks, as JSON (its text in English and Japanese is in tests/fixtures, beside the
//! other fixtures); what the port of flows says of the calls that give a secret to a file of the
//! project (`Flows::sends`); the code dandori writes for a workflow whose history is encrypted, which
//! takes a payload codec by its types in TypeScript, Python and Go, and in which a name of the flow or
//! of its service gives way to the names that code adds; and that code, run on Temporal's dev server
//! with a codec, keeping nothing of the workflow's values in the clear in the history.
//! `DANDORI_BLESS=1` (or `RITSU_BLESS=1`) rewrites the golden files.

use ritsu_ports::Flows;
use ritsu_testkit::{need, ready, skip, Need, TempDir};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The ports a flow is read with, as `ritsu dandori` joins them: the rules through rulec's answer,
/// the dates files and the books through koyomi's and chobo's.
fn ports() -> ritsu_ports::Ports {
    let rules: Rc<dyn ritsu_ports::Rules> = Rc::new(rulec::ports::Engine::with_dates(std::sync::Arc::new(koyomi::ports::Engine)));
    ritsu_ports::Ports { rules, dates: Rc::new(koyomi::ports::Engine), books: Rc::new(chobo::ports::Engine) }
}

/// The two fixtures that follow a secret from each of its marks, from the crate's directory.
const FIXTURES: [&str; 2] = ["tests/fixtures/secrets.flow", "tests/fixtures/secrets.ja.flow"];

#[test]
fn the_secrets_fixture_says_each_secret_as_json() {
    let rules: Rc<dyn ritsu_ports::Rules> = Rc::new(rulec::ports::Engine::with_dates(std::sync::Arc::new(koyomi::ports::Engine)));
    let mut failures = Vec::new();
    for f in FIXTURES {
        for (lang, tag) in [("en", "check"), ("ja", "check.ja")] {
            let args: Vec<String> = ["check", f, "--format", "json", "--lang", lang].iter().map(|s| s.to_string()).collect();
            let (mut out, mut err) = (Vec::new(), Vec::new());
            // run from the crate's directory, as the paths are written
            let code = dandori::cli::run(&args, rules.clone(), &mut out, &mut err);
            assert_eq!(code, 0, "{f} does not pass check: {}", String::from_utf8_lossy(&err));
            let golden = root().join(f).with_extension(format!("{tag}.json"));
            if let Err(e) = ritsu_testkit::golden::check(&golden, &String::from_utf8(out).unwrap()) {
                failures.push(format!("{f} ({lang}): {e}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// A call the port says gives secrets, as JSON, the paths from the crate's directory.
fn shown(sends: &[ritsu_ports::Send]) -> Value {
    let path = |p: &Path| p.to_string_lossy().replace('\\', "/");
    Value::Array(
        sends
            .iter()
            .map(|s| {
                let ritsu_ports::Destination::File(to) = &s.to;
                json!({
                    "line": s.line,
                    "task": s.task,
                    "to": path(to),
                    "secrets": s.secrets.iter().map(|(p, x)| json!({ "param": p, "shown": x.shown, "marked_in": path(&x.marked_in), "line": x.line, "mark": x.mark })).collect::<Vec<_>>(),
                    "disclosed": s.disclosed.iter().map(|(p, why)| json!([p, why])).collect::<Vec<_>>(),
                })
            })
            .collect(),
    )
}

#[test]
fn the_port_says_which_calls_give_a_secret_to_a_file_of_the_project() {
    let ports = ports();
    // the fixtures: the calls of the bank's `.proto` and of the CRM's OpenAPI document that give one
    let mut failures = Vec::new();
    for f in FIXTURES {
        let sends = dandori::ports::Engine.sends(Path::new(f), &ports).unwrap_or_else(|e| panic!("{f}: {e:?}"));
        let text = serde_json::to_string_pretty(&shown(&sends)).unwrap() + "\n";
        let golden = root().join(f).with_extension("sends.json");
        if let Err(e) = ritsu_testkit::golden::check(&golden, &text) {
            failures.push(format!("{f}: {e}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
    // no example and no flow of tests/flows gives a secret anywhere
    let mut all = Vec::new();
    for dir in ["examples", "tests/flows"] {
        ritsu_base::paths::walk(&root(), dir, &[], &mut all);
    }
    all.retain(|f| f.ends_with(".flow"));
    assert!(all.len() > 40, "{all:?}");
    for f in &all {
        let sends = dandori::ports::Engine.sends(&root().join(f), &ports).unwrap_or_else(|e| panic!("{f}: {e:?}"));
        assert!(sends.is_empty(), "{f} gives a secret: {sends:?}");
    }
    // a flow that does not pass the check is said by its errors
    let failing = dandori::ports::Engine.sends(Path::new("tests/fixtures/security/E906_agent.flow"), &ports).unwrap_err();
    assert!(failing.iter().any(|s| s.code == "E906"), "{failing:?}");
}

/// A flow built for a platform, written under `dir`: the directory of its code.
fn build_into(flow: &Path, target: &str, dir: &Path) -> PathBuf {
    let (_, c) = dandori::check::check_file(flow).unwrap();
    let m = c.model.unwrap_or_else(|| panic!("{} passes check", flow.display()));
    let files = dandori::commands::build(&m, target).expect("a target dandori builds for").unwrap_or_else(|d| panic!("{} does not build for {target}: {}", flow.display(), d[0].en));
    for (name, text) in &files {
        let p = dir.join(name);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, text).unwrap();
    }
    dir.join(files[0].0.split('/').next().unwrap())
}

/// The flow of tests/encrypted, built for a platform, written under `dir`.
fn built(target: &str, dir: &Path) -> PathBuf {
    build_into(&root().join("tests/encrypted/pay.flow"), target, dir)
}

/// TypeScript that uses the generated client and worker: with the codec (`good`), or without it,
/// a client the SDK makes and a worker without `codec`.
const TS_GOOD: &str = r#"import { Connection } from "@temporalio/client";
import type { Payload, PayloadCodec } from "@temporalio/common";
import { encryptedClient, start } from "./client";
import { makeWorker } from "./worker";

const codec: PayloadCodec = {
  async encode(payloads: Payload[]): Promise<Payload[]> {
    return payloads;
  },
  async decode(payloads: Payload[]): Promise<Payload[]> {
    return payloads;
  },
};

export async function main(): Promise<void> {
  const client = encryptedClient(await Connection.connect({}), codec);
  await makeWorker({ pay: async () => "receipt" }, { codec });
  await start(client, "pay-1", { account: { id: "acct-1", number: "4111" } });
}
"#;

const TS_BAD: &str = r#"import { Client, Connection } from "@temporalio/client";
import { start } from "./client";
import { makeWorker } from "./worker";

export async function main(): Promise<void> {
  const client = new Client({ connection: await Connection.connect({}) });
  await makeWorker({ pay: async () => "receipt" }, {});
  await start(client, "pay-1", { account: { id: "acct-1", number: "4111" } });
}
"#;

const PY_GOOD: &str = r#"from typing import Any, Sequence

from temporalio.api.common.v1 import Payload
from temporalio.converter import PayloadCodec

from pay.client import connect, start
from pay.worker import make_worker


class Identity(PayloadCodec):
    async def encode(self, payloads: Sequence[Payload]) -> list[Payload]:
        return list(payloads)

    async def decode(self, payloads: Sequence[Payload]) -> list[Payload]:
        return list(payloads)


class Own:
    async def pay(self, args: dict[str, Any]) -> str:
        return "receipt"


async def main() -> None:
    client = await connect("localhost:7233", Identity())
    make_worker(client, Own())
    await start(client, "pay-1", {"account": {"id": "acct-1", "number": "4111"}})
"#;

const PY_BAD: &str = r#"from typing import Any

from temporalio.client import Client

from pay.client import start
from pay.worker import make_worker


class Own:
    async def pay(self, args: dict[str, Any]) -> str:
        return "receipt"


async def main() -> None:
    client = await Client.connect("localhost:7233")
    make_worker(client, Own())
    await start(client, "pay-1", {"account": {"id": "acct-1", "number": "4111"}})
"#;

const GO_GOOD: &str = r#"package main

import (
	"context"

	commonpb "go.temporal.io/api/common/v1"
	"go.temporal.io/sdk/client"

	"codeccheck/flows/pay"
)

type identity struct{}

func (identity) Encode(ps []*commonpb.Payload) ([]*commonpb.Payload, error) { return ps, nil }
func (identity) Decode(ps []*commonpb.Payload) ([]*commonpb.Payload, error) { return ps, nil }

type own struct{}

func (own) Pay(ctx context.Context, args map[string]any) (any, error) { return "receipt", nil }

func main() {
	c, err := pay.Dial(client.Options{}, identity{})
	if err != nil {
		panic(err)
	}
	_ = pay.NewWorker(c, own{}, nil, "")
	_, _ = pay.Start(context.Background(), c, "pay-1", map[string]any{"account": map[string]any{"id": "acct-1", "number": "4111"}}, false)
}
"#;

/// Go that gives the worker, or `Start`, a client the SDK makes, without the codec: go vet says the
/// first type error of a package, so each is a program of its own.
const GO_BAD_WORKER: &str = r#"package main

import (
	"context"

	"go.temporal.io/sdk/client"

	"codeccheck/flows/pay"
)

type own struct{}

func (own) Pay(ctx context.Context, args map[string]any) (any, error) { return "receipt", nil }

func main() {
	c, err := client.Dial(client.Options{})
	if err != nil {
		panic(err)
	}
	_ = pay.NewWorker(c, own{}, nil, "")
}
"#;

const GO_BAD_CLIENT: &str = r#"package main

import (
	"context"

	"go.temporal.io/sdk/client"

	"codeccheck/flows/pay"
)

func main() {
	c, err := client.Dial(client.Options{})
	if err != nil {
		panic(err)
	}
	_, _ = pay.Start(context.Background(), c, "pay-1", map[string]any{"account": map[string]any{"id": "acct-1", "number": "4111"}}, false)
}
"#;

/// A Go module of tools/temporal-go's go.mod and go.sum, named `codeccheck`, with the generated
/// package at flows/pay and each program under cmd/.
fn go_module(dir: &Path, programs: &[(&str, &str)]) {
    go_module_of(dir, &["tests/encrypted/pay.flow"], programs);
}

/// `go_module`, with the package of each of `flows` under flows/.
fn go_module_of(dir: &Path, flows: &[&str], programs: &[(&str, &str)]) {
    let tools = root().join("tools/temporal-go");
    let gomod = std::fs::read_to_string(tools.join("go.mod")).unwrap();
    let gomod = gomod.lines().map(|l| if l.starts_with("module ") { "module codeccheck".to_string() } else { l.to_string() }).collect::<Vec<_>>().join("\n") + "\n";
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("go.mod"), gomod).unwrap();
    std::fs::copy(tools.join("go.sum"), dir.join("go.sum")).unwrap();
    for f in flows {
        build_into(&root().join(f), "temporal-go", &dir.join("flows"));
    }
    for (name, text) in programs {
        let p = dir.join("cmd").join(name).join("main.go");
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
}

fn go_in(dir: &Path) -> Command {
    let mut go = Command::new("go");
    go.current_dir(dir).env("GOWORK", "off").env("GOFLAGS", "-trimpath");
    go
}

fn said(out: &std::process::Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr))
}

/// `tsc --strict` on the TypeScript in `code`, with tools/temporal's packages, and the packages the
/// default Transport loads when it needs them of any type (as generated_typescript_type_checks has
/// them).
fn tsc_strict(tsc: &Path, code: &Path) -> std::process::Output {
    std::fs::write(code.join("shims.d.ts"), "declare module \"@anthropic-ai/sdk\";\ndeclare module \"openai\";\ndeclare module \"@openai/agents\";\ndeclare module \"@aws-sdk/*\";\n").unwrap();
    std::os::unix::fs::symlink(root().join("tools/temporal/node_modules"), code.join("node_modules")).unwrap();
    let tsconfig = json!({
        "compilerOptions": { "strict": true, "noEmit": true, "module": "nodenext", "moduleResolution": "nodenext", "target": "es2022", "skipLibCheck": true, "types": ["node"], "typeRoots": [root().join("tools/temporal/node_modules/@types")] },
        "include": ["*.ts"]
    });
    std::fs::write(code.join("tsconfig.json"), serde_json::to_string_pretty(&tsconfig).unwrap()).unwrap();
    Command::new(tsc).arg("-p").arg(code.join("tsconfig.json")).output().unwrap()
}

/// The code dandori writes for a workflow whose history is encrypted takes the payload codec by
/// its types: `tsc --strict`, `mypy --strict` and `go vet` pass a program that gives the worker and
/// the client the codec, and refuse one that does not, at the worker and at the client.
#[test]
fn the_code_of_an_encrypted_history_takes_the_codec() {
    let scratch = TempDir::new("dandori-encrypted-types");
    // TypeScript
    let tsc = root().join("tools/temporal/node_modules/.bin/tsc");
    if ready(Need::Node, || tsc.exists(), "TypeScript is not in tools/temporal/node_modules; run `npm install --prefix tools/temporal`") {
        for (name, uses, passes) in [("good", TS_GOOD, true), ("bad", TS_BAD, false)] {
            let code = built("temporal", &scratch.path().join(format!("ts-{name}")));
            std::fs::write(code.join("uses.ts"), uses).unwrap();
            let out = tsc_strict(&tsc, &code);
            if passes {
                assert!(out.status.success(), "tsc --strict refuses the TypeScript that gives the codec:\n{}", said(&out));
            } else {
                let text = said(&out);
                assert!(!out.status.success(), "tsc --strict passes the TypeScript that gives no codec");
                assert!(text.contains("uses.ts(7,") && text.contains("'codec'"), "tsc does not say the worker needs the codec:\n{text}");
                assert!(text.contains("uses.ts(8,") && text.contains("EncryptedClient"), "tsc does not say the client needs the codec:\n{text}");
            }
        }
        eprintln!("tsc --strict: the TypeScript takes the codec");
    }
    // Python
    let python = root().join("tools/temporal-python/.venv/bin/python");
    let mypy = std::env::var_os("DANDORI_MYPY").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("mypy"));
    let has_mypy = || python.exists() && Command::new(&mypy).arg("--version").output().is_ok_and(|o| o.status.success());
    if ready(Need::Python, has_mypy, "mypy (on PATH, or DANDORI_MYPY) or tools/temporal-python/.venv is missing") {
        for (name, uses, passes) in [("good", PY_GOOD, true), ("bad", PY_BAD, false)] {
            let dir = scratch.path().join(format!("py-{name}"));
            built("temporal-python", &dir);
            std::fs::write(dir.join("uses.py"), uses).unwrap();
            let out = Command::new(&mypy).current_dir(&dir).args(["--strict", "--cache-dir", "cache", "--python-executable"]).arg(&python).args(["-p", "pay", "-m", "uses"]).output().unwrap();
            let text = said(&out);
            if passes {
                assert!(out.status.success(), "mypy --strict refuses the Python that gives the codec:\n{text}");
            } else {
                assert!(!out.status.success(), "mypy --strict passes the Python that gives no codec");
                assert!(text.contains("uses.py:16:") && text.contains("\"EncryptedClient\""), "mypy does not say the worker needs the client with the codec:\n{text}");
                assert!(text.contains("uses.py:17:") && text.contains("\"EncryptedClient\""), "mypy does not say start needs the client with the codec:\n{text}");
            }
        }
        eprintln!("mypy --strict: the Python takes the codec");
    }
    // Go
    let has_go = || Command::new("go").arg("version").output().is_ok_and(|o| o.status.success()) && root().join("tools/temporal-go/go.mod").exists();
    if ready(Need::Go, has_go, "go or tools/temporal-go is missing") {
        let dir = scratch.path().join("go");
        go_module(&dir, &[("good", GO_GOOD), ("bad-worker", GO_BAD_WORKER), ("bad-client", GO_BAD_CLIENT)]);
        let out = go_in(&dir).args(["vet", "./flows/...", "./cmd/good"]).output().unwrap();
        assert!(out.status.success(), "go vet refuses the Go that gives the codec:\n{}", said(&out));
        let fmt = Command::new("gofmt").arg("-l").arg("flows").current_dir(&dir).output().unwrap();
        assert!(fmt.status.success() && fmt.stdout.is_empty(), "gofmt would write the Go of an encrypted history otherwise:\n{}", said(&fmt));
        for (program, line, what) in [("bad-worker", "main.go:20:", "the worker"), ("bad-client", "main.go:16:", "Start")] {
            let out = go_in(&dir).args(["vet", &format!("./cmd/{program}")]).output().unwrap();
            let text = said(&out);
            assert!(!out.status.success(), "go vet passes {program}, which gives {what} no codec");
            assert!(text.contains(line) && text.contains("EncryptedClient"), "go vet does not say {what} needs the client with the codec:\n{text}");
        }
        eprintln!("go vet: the Go takes the codec");
    }
}

/// The flows of tests/encrypted, English and Japanese, that give their records, an enum, and the
/// methods and messages of the service they implement the names the client of an encrypted history
/// declares.
const NAMES: [&str; 2] = ["tests/encrypted/names.flow", "tests/encrypted/names.ja.flow"];

/// A name the client of an encrypted history declares, given to a record, an enum, or a method or a
/// message of the service the workflow implements, gives way in the code dandori writes as one of
/// the client's own names does (DESIGN 1.14, 1.18): a function or a type of the service takes `Rpc`,
/// `_rpc` or `RPC`, and a Go type `_`. The check says nothing of them, the code passes `tsc --strict`
/// and `go vet`, and `mypy --strict` finds no name defined twice in the Python.
#[test]
fn the_names_of_an_encrypted_client_give_way() {
    let scratch = TempDir::new("dandori-encrypted-names");
    for f in NAMES {
        let (_, c) = dandori::check::check_file(&root().join(f)).unwrap();
        assert!(c.diags.is_empty(), "{f} is said something: {:?}", c.diags.iter().map(|d| format!("{} {}", d.code, d.en)).collect::<Vec<_>>());
    }
    // what the code of both flows names them (the names are the .proto's and the types', the same in both)
    let code_of = |f: &str, target: &str| build_into(&root().join(f), target, &scratch.path().join(target).join(f.rsplit('/').next().unwrap()));
    let has = |target: &str, file: &str, names: &[&str]| {
        for f in NAMES {
            let text = std::fs::read_to_string(code_of(f, target).join(file)).unwrap();
            for n in names {
                assert!(text.contains(n), "{f}: the {file} dandori writes for {target} has no `{n}`:\n{text}");
            }
        }
    };
    has("temporal", "client.ts", &["export type ConnectionRpc =", "export type PayloadCodecRpc =", "export type ClientOptionsRpc =", "export async function encryptedClientRpc(", "export async function encryptedRpc("]);
    has("temporal-python", "client.py", &["\nNewType_rpc = ", "\nPayloadCodec_rpc = ", "\nDataConverter_rpc = ", "async def connect_rpc("]);
    has("temporal-go", "client.go", &["func EncryptedClientRPC(", "func DialRPC(", "func CodecFailureConverterRPC(", "type CodecDataConverterRPC ="]);
    has("temporal-go", "types.go", &["type Dial_ struct", "type EncryptedClient_ struct", "type CodecFailureConverter_ struct", "type CodecDataConverter_ string"]);
    // TypeScript
    let tsc = root().join("tools/temporal/node_modules/.bin/tsc");
    if ready(Need::Node, || tsc.exists(), "TypeScript is not in tools/temporal/node_modules; run `npm install --prefix tools/temporal`") {
        for f in NAMES {
            let out = tsc_strict(&tsc, &code_of(f, "temporal"));
            assert!(out.status.success(), "tsc --strict refuses the TypeScript of {f}:\n{}", said(&out));
        }
        eprintln!("tsc --strict: the TypeScript of the names of an encrypted client");
    }
    // Python
    let python = root().join("tools/temporal-python/.venv/bin/python");
    let mypy = std::env::var_os("DANDORI_MYPY").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("mypy"));
    let has_mypy = || python.exists() && Command::new(&mypy).arg("--version").output().is_ok_and(|o| o.status.success());
    if ready(Need::Python, has_mypy, "mypy (on PATH, or DANDORI_MYPY) or tools/temporal-python/.venv is missing") {
        for f in NAMES {
            let code = code_of(f, "temporal-python");
            let package = code.file_name().unwrap().to_string_lossy().to_string();
            let out = Command::new(&mypy).current_dir(code.parent().unwrap()).args(["--strict", "--no-color-output", "--cache-dir", "cache", "--python-executable"]).arg(&python).args(["-p", &package]).output().unwrap();
            let text = said(&out);
            assert!(text.contains("source file"), "mypy did not check the Python of {f}:\n{text}");
            // The Python of a workflow that implements a service does not pass `mypy --strict` as it
            // is, with an encrypted history or not (tests/flows/service.flow): the method that starts
            // a run gives `start` its request, a TypedDict, where `start` takes a dict. Any other
            // error, a name defined twice (`no-redef`) or a type given another value (`misc`) among
            // them, is one this test is for.
            let errors: Vec<&str> = text.lines().filter(|l| l.contains(": error:")).collect();
            assert!(errors.iter().all(|l| l.contains("Argument 3 to \"start\"") && l.ends_with("[arg-type]")), "mypy --strict finds fault with the Python of {f}:\n{text}");
        }
        eprintln!("mypy --strict: the Python of the names of an encrypted client");
    }
    // Go
    let has_go = || Command::new("go").arg("version").output().is_ok_and(|o| o.status.success()) && root().join("tools/temporal-go/go.mod").exists();
    if ready(Need::Go, has_go, "go or tools/temporal-go is missing") {
        let dir = scratch.path().join("go");
        go_module_of(&dir, &NAMES, &[]);
        let out = go_in(&dir).args(["vet", "./flows/..."]).output().unwrap();
        assert!(out.status.success(), "go vet refuses the Go of the names of an encrypted client:\n{}", said(&out));
        let fmt = Command::new("gofmt").arg("-l").arg("flows").current_dir(&dir).output().unwrap();
        assert!(fmt.status.success() && fmt.stdout.is_empty(), "gofmt would write the Go of the names of an encrypted client otherwise:\n{}", said(&fmt));
        eprintln!("go vet: the Go of the names of an encrypted client");
    }
}

/// The names TypeScript, Python or Go code declares and imports at its top level (`ts`, `py`, `go`).
/// Go's imports are its files' own, and not the package's: only what the package declares.
fn top_names(lang: &str, text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let word = |s: &str| s.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect::<String>();
    let mut block = false;
    for line in text.lines() {
        match lang {
            "ts" => {
                if let Some(rest) = line.strip_prefix("import ") {
                    let rest = rest.strip_prefix("type ").unwrap_or(rest);
                    if let Some(ns) = rest.strip_prefix("* as ") {
                        out.insert(word(ns));
                    } else if let (Some(a), Some(b)) = (rest.find('{'), rest.find('}')) {
                        for part in rest[a + 1..b].split(',') {
                            let part = part.trim();
                            let part = part.strip_prefix("type ").unwrap_or(part);
                            out.insert(word(part.rsplit(" as ").next().unwrap_or(part)));
                        }
                    }
                    continue;
                }
                let rest = line.strip_prefix("export ").unwrap_or(line);
                let rest = rest.strip_prefix("declare ").unwrap_or(rest);
                let rest = rest.strip_prefix("async ").unwrap_or(rest);
                for kw in ["function* ", "function ", "const ", "let ", "type ", "interface ", "class "] {
                    if let Some(name) = rest.strip_prefix(kw) {
                        out.insert(word(name));
                        break;
                    }
                }
            }
            "py" => {
                if let Some(m) = line.strip_prefix("import ") {
                    out.insert(word(m));
                } else if let Some(rest) = line.strip_prefix("from ") {
                    if let Some((_, names)) = rest.split_once(" import ") {
                        for part in names.split(',') {
                            let part = part.trim();
                            out.insert(word(part.rsplit(" as ").next().unwrap_or(part)));
                        }
                    }
                } else if let Some(name) = line.strip_prefix("async def ").or_else(|| line.strip_prefix("def ")).or_else(|| line.strip_prefix("class ")) {
                    out.insert(word(name));
                } else if line.starts_with(|c: char| c.is_alphabetic() || c == '_') {
                    let name = word(line);
                    let after = line[name.len()..].trim_start();
                    if after.starts_with('=') || after.starts_with(':') {
                        out.insert(name);
                    }
                }
            }
            _ => {
                if block {
                    if line == ")" {
                        block = false;
                    } else if let Some(rest) = line.strip_prefix('\t') {
                        if rest.starts_with(|c: char| c.is_alphabetic() || c == '_') {
                            out.insert(word(rest));
                        }
                    }
                } else if line == "const (" || line == "var (" {
                    block = true;
                } else if let Some(rest) = line.strip_prefix("func ") {
                    if !rest.starts_with('(') {
                        out.insert(word(rest));
                    }
                } else if let Some(rest) = line.strip_prefix("type ").or_else(|| line.strip_prefix("const ")).or_else(|| line.strip_prefix("var ")) {
                    out.insert(word(rest));
                }
            }
        }
    }
    out.remove("");
    out
}

/// What the code dandori writes declares and imports when the history is encrypted, and not
/// otherwise, where a name of the flow or of its service could be written beside it, is what the
/// generators' lists hold, neither more nor less (DESIGN 1.18): client.ts and client.py (the types
/// of the flow are in types.ts and types.py, its rules in rules.ts and rules.py, and the worker
/// holds no name of the flow), and the whole package in Go.
#[test]
fn the_names_an_encrypted_history_adds_are_the_ones_given_way_to() {
    let scratch = TempDir::new("dandori-encrypted-added");
    let plain = scratch.path().join("plain/pay.flow");
    std::fs::create_dir_all(plain.parent().unwrap()).unwrap();
    let text = std::fs::read_to_string(root().join("tests/encrypted/pay.flow")).unwrap();
    assert!(text.contains("\n  history encrypted\n"));
    std::fs::write(&plain, text.replace("\n  history encrypted\n", "\n")).unwrap();
    let lists: [(&str, &str, &[&str]); 3] = [("temporal", "ts", dandori::temporal::ENCRYPTED_CLIENT), ("temporal-python", "py", dandori::temporal_py::ENCRYPTED_CLIENT), ("temporal-go", "go", dandori::temporal_go::ENCRYPTED)];
    for (target, lang, list) in lists {
        let names = |flow: &Path, side: &str| -> BTreeSet<String> {
            let code = build_into(flow, target, &scratch.path().join(side).join(target));
            let mut out = BTreeSet::new();
            for e in std::fs::read_dir(&code).unwrap() {
                let p = e.unwrap().path();
                let file = p.file_name().unwrap().to_string_lossy().to_string();
                if file == format!("client.{lang}") || (lang == "go" && file.ends_with(".go")) {
                    out.extend(top_names(lang, &std::fs::read_to_string(&p).unwrap()));
                }
            }
            out
        };
        let encrypted = names(&root().join("tests/encrypted/pay.flow"), "encrypted");
        let added: BTreeSet<String> = encrypted.difference(&names(&plain, "plain")).cloned().collect();
        let listed: BTreeSet<String> = list.iter().map(|s| s.to_string()).collect();
        assert_eq!(added, listed, "the names the {target} code of an encrypted history adds, and the ones its generator's list gives way to");
        assert!(listed.iter().all(|n| encrypted.contains(n)));
    }
}

/// What every SDK's run of tests/encrypted/pay.flow comes to: the run that pays and the one the bank
/// refuses end as the reference says, the input in the history is the codec's, the account's number
/// is nowhere in the history in the clear, and the failure's message is encoded with the rest.
fn encrypted_runs() -> Value {
    json!({
        "paid": {
            "end": { "succeed": { "receipt": "receipt for acct-paid" } },
            "input_encoding": "binary/dandori-test-base64",
            "number_in_the_clear": false,
            "failure_message": null,
            "failure_encoded": false
        },
        "refused": {
            "end": { "fail": { "error": "Refused", "cause": "the bank refused to pay 5500-0000-0000-0004" } },
            "input_encoding": "binary/dandori-test-base64",
            "number_in_the_clear": false,
            "failure_message": "Encoded failure",
            "failure_encoded": true
        }
    })
}

/// The Go program that runs tests/encrypted/pay.flow as tools/temporal/codec.mjs does.
const GO_RUN: &str = r#"package main

import (
	"context"
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"strings"

	commonpb "go.temporal.io/api/common/v1"
	enumspb "go.temporal.io/api/enums/v1"
	"go.temporal.io/sdk/client"
	"go.temporal.io/sdk/temporal"
	"go.temporal.io/sdk/testsuite"
	"google.golang.org/protobuf/encoding/protojson"

	"codeccheck/flows/pay"
)

const encoding = "binary/dandori-test-base64"

type b64 struct{}

func (b64) Encode(ps []*commonpb.Payload) ([]*commonpb.Payload, error) {
	out := make([]*commonpb.Payload, len(ps))
	for i, p := range ps {
		out[i] = &commonpb.Payload{Metadata: map[string][]byte{"encoding": []byte(encoding), "dandori-encoding": p.Metadata["encoding"]}, Data: []byte(base64.StdEncoding.EncodeToString(p.Data))}
	}
	return out, nil
}

func (b64) Decode(ps []*commonpb.Payload) ([]*commonpb.Payload, error) {
	out := make([]*commonpb.Payload, len(ps))
	for i, p := range ps {
		if string(p.Metadata["encoding"]) != encoding {
			out[i] = p
			continue
		}
		data, err := base64.StdEncoding.DecodeString(string(p.Data))
		if err != nil {
			return nil, err
		}
		out[i] = &commonpb.Payload{Metadata: map[string][]byte{"encoding": p.Metadata["dandori-encoding"]}, Data: data}
	}
	return out, nil
}

type own struct{}

func (own) Pay(ctx context.Context, args map[string]any) (any, error) {
	account, _ := args["account"].(map[string]any)
	if n, _ := account["number"].(string); strings.HasPrefix(n, "5500") {
		return nil, temporal.NewNonRetryableApplicationError("the account is closed", "refused", nil)
	}
	return fmt.Sprintf("receipt for %v", account["id"]), nil
}

func run() (map[string]any, error) {
	server, err := testsuite.StartDevServer(context.Background(), testsuite.DevServerOptions{LogLevel: "error", Stdout: os.Stderr, Stderr: os.Stderr})
	if err != nil {
		return nil, err
	}
	defer server.Stop()
	c, err := pay.Dial(client.Options{HostPort: server.FrontendHostPort()}, b64{})
	if err != nil {
		return nil, err
	}
	defer c.Close()
	plain, err := client.Dial(client.Options{HostPort: server.FrontendHostPort()})
	if err != nil {
		return nil, err
	}
	defer plain.Close()
	w := pay.NewWorker(c, own{}, nil, "")
	if err := w.Start(); err != nil {
		return nil, err
	}
	defer w.Stop()
	results := map[string]any{}
	for _, r := range [][2]string{{"paid", "4111-1111-1111-1111"}, {"refused", "5500-0000-0000-0004"}} {
		id, number := r[0], r[1]
		run, err := pay.Start(context.Background(), c, id, map[string]any{"account": map[string]any{"id": "acct-" + id, "number": number}}, false)
		if err != nil {
			return nil, err
		}
		var end map[string]any
		var out map[string]any
		if err := run.Get(context.Background(), &out); err != nil {
			var app *temporal.ApplicationError
			if !errors.As(err, &app) {
				return nil, err
			}
			end = map[string]any{"fail": map[string]any{"error": app.Type(), "cause": app.Message()}}
		} else {
			end = map[string]any{"succeed": out}
		}
		// the history as the server keeps it, read with no codec
		events := plain.GetWorkflowHistory(context.Background(), id, "", false, enumspb.HISTORY_EVENT_FILTER_TYPE_ALL_EVENT)
		var kept strings.Builder
		var inputEncoding string
		var failureMessage any
		failureEncoded := false
		for events.HasNext() {
			ev, err := events.Next()
			if err != nil {
				return nil, err
			}
			text, _ := protojson.Marshal(ev)
			kept.Write(text)
			if a := ev.GetWorkflowExecutionStartedEventAttributes(); a != nil {
				inputEncoding = string(a.Input.Payloads[0].Metadata["encoding"])
			}
			if a := ev.GetWorkflowExecutionFailedEventAttributes(); a != nil {
				failureMessage = a.Failure.Message
				failureEncoded = a.Failure.EncodedAttributes != nil
			}
		}
		// protojson writes bytes as base64: the number would show as the base64 of itself
		clear := strings.Contains(kept.String(), number) || strings.Contains(kept.String(), base64.StdEncoding.EncodeToString([]byte(number)))
		results[id] = map[string]any{"end": end, "input_encoding": inputEncoding, "number_in_the_clear": clear, "failure_message": failureMessage, "failure_encoded": failureEncoded}
	}
	return results, nil
}

func main() {
	results, err := run()
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	text, _ := json.MarshalIndent(results, "", "  ")
	if err := os.WriteFile(os.Args[1], append(text, '\n'), 0o644); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
"#;

/// tests/encrypted/pay.flow, whose history is encrypted, run on Temporal's dev server through the
/// worker and the client dandori writes in TypeScript, Python and Go, with a codec that writes every
/// payload as base64 under an encoding of its own: the runs end as they should, and the history the
/// server keeps, read without the codec, holds the input as the codec wrote it, nothing of the
/// account's number in the clear, and the failure's message encoded too.
#[test]
fn temporal_keeps_an_encrypted_history_with_the_codec() {
    if !need(Need::Temporal) {
        return;
    }
    let scratch = TempDir::new("dandori-encrypted-runs");
    let want = encrypted_runs();
    let mut ran = Vec::new();
    // TypeScript
    if root().join("tools/temporal/node_modules/@temporalio/testing").exists() {
        let code = built("temporal", &scratch.path().join("ts"));
        let out_file = scratch.path().join("ts.json");
        let out = Command::new("node").arg(root().join("tools/temporal/codec.mjs")).arg(&code).arg(&out_file).env("TMPDIR", scratch.path()).output().unwrap();
        assert!(out.status.success(), "tools/temporal/codec.mjs failed:\n{}", said(&out));
        let got: Value = serde_json::from_str(&std::fs::read_to_string(&out_file).unwrap()).unwrap();
        assert_eq!(got, want, "TypeScript");
        ran.push("TypeScript");
    } else {
        skip("tools/temporal/node_modules is missing; the TypeScript is not run with a codec");
    }
    // Python
    let python = root().join("tools/temporal-python/.venv/bin/python");
    if python.exists() {
        let code = built("temporal-python", &scratch.path().join("py"));
        let out_file = scratch.path().join("py.json");
        let out = Command::new(&python).arg("-B").arg(root().join("tools/temporal-python/codec.py")).arg(&code).arg(&out_file).env("TMPDIR", scratch.path()).output().unwrap();
        assert!(out.status.success(), "tools/temporal-python/codec.py failed:\n{}", said(&out));
        let got: Value = serde_json::from_str(&std::fs::read_to_string(&out_file).unwrap()).unwrap();
        assert_eq!(got, want, "Python");
        ran.push("Python");
    } else {
        skip("tools/temporal-python/.venv is missing; the Python is not run with a codec");
    }
    // Go
    if Command::new("go").arg("version").output().is_ok_and(|o| o.status.success()) && root().join("tools/temporal-go/go.mod").exists() {
        let dir = scratch.path().join("go");
        go_module(&dir, &[("run", GO_RUN)]);
        let out_file = scratch.path().join("go.json");
        let out = go_in(&dir).args(["run", "./cmd/run"]).arg(&out_file).env("TMPDIR", scratch.path()).output().unwrap();
        assert!(out.status.success(), "the Go program failed:\n{}", said(&out));
        let got: Value = serde_json::from_str(&std::fs::read_to_string(&out_file).unwrap()).unwrap();
        assert_eq!(got, want, "Go");
        ran.push("Go");
    } else {
        skip("go or tools/temporal-go is missing; the Go is not run with a codec");
    }
    eprintln!("ran tests/encrypted/pay.flow with a codec in {}", ran.join(", "));
}
