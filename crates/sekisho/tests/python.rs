//! The Python sekisho generates (`gen --target python`, DESIGN 5.3, 6.3), as Python's own tools
//! read it and as cedarpy answers it. Every `.gate` of the examples and the tests that passes its
//! check is generated twice — asking cedarpy in the process (`--authorizer cedar`) and Verified
//! Permissions through boto3 (`--authorizer avp`) — into a package laid out as `ritsu gen` lays one
//! out: `rules/` and `dates/` beside `authz/`, the rules' and the dates' modules written by rulec's
//! and koyomi's own generators. Then:
//!
//! - `mypy --strict` reads every package, both authorizers (the code of `avp` is only type-checked:
//!   the tests never call AWS, DESIGN 6.3);
//! - the code of `cedar` is run on the raw values of every combination the check walks, and on the
//!   faults it is to refuse (the store's records, the input, the time; `sekisho::raw`), and each
//!   answer — allowed or not, the policies that decide as a set, the kind of a refusal, the context
//!   Cedar was given — is held to what sekisho's reference evaluation answers for the same values,
//!   through the ports of rulec and koyomi (`Raw::compare`).
//!
//! The Python is tools/runner-py's: cedarpy 4.12.1, boto3 and its stubs, mypy 2.4.0, made with
//! `uv venv --python 3.13 tools/runner-py/.venv` and `uv pip sync --python
//! tools/runner-py/.venv/bin/python --require-hashes tools/runner-py/requirements.txt`. Without it,
//! a test says SKIP and passes.

mod common;

use ritsu_testkit::{Need, TempDir};
use sekisho::r#gen::Authorizer;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The runner's Python, or None with the SKIP line saying why.
fn python() -> Option<PathBuf> {
    if !ritsu_testkit::need(Need::Python) {
        return None;
    }
    // the venv's own python, not the interpreter its link points to: the packages are the venv's
    let py = Some(std::env::current_dir().unwrap().join("tools/runner-py/.venv/bin/python")).filter(|p| p.is_file());
    if py.is_none() {
        ritsu_testkit::skip("tools/runner-py/.venv is missing; make it with `uv venv --python 3.13 tools/runner-py/.venv` and `uv pip sync --python tools/runner-py/.venv/bin/python --require-hashes tools/runner-py/requirements.txt`; the generated Python is not checked");
    }
    py
}

/// Every `.gate` under `examples/` and `tests/`, sorted.
fn gates() -> Vec<String> {
    fn walk(d: &Path, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "gate") {
                out.push(p.to_string_lossy().to_string());
            }
        }
    }
    let mut out = Vec::new();
    walk(Path::new("examples"), &mut out);
    walk(Path::new("tests"), &mut out);
    out.sort();
    out
}

fn write(p: &Path, text: &str) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

/// The package of one gate, as `ritsu gen` lays it out under `dir/<name>/`: the rules' and the
/// dates' modules the gate reads (each written by its own language's generator) and the gate's
/// own, for `authorizer`. None for a gate whose check finds an error.
fn package(gate: &str, dir: &Path, name: &str, authorizer: Authorizer, lang: ritsu_base::text::Lang) -> Option<sekisho::check::Outcome> {
    let suite = common::joined();
    let o = sekisho::check::check_file(gate, &suite, &sekisho::check::Options::default()).unwrap();
    if o.has_errors() {
        return None;
    }
    let base = dir.join(name);
    for sub in ["", "rules", "dates", "authz"] {
        write(&base.join(sub).join("__init__.py"), "");
    }
    write(&base.join("py.typed"), "");
    let scope = o.scope.as_ref().unwrap();
    for read in scope.rules.values() {
        let src = std::fs::read_to_string(&read.path).unwrap();
        let p = read.path.to_string_lossy().to_string();
        let (rel, body) = rulec::codegen::package_module(&src, &p, &p, "python").unwrap();
        write(&base.join("rules").join(rel), &body);
    }
    let checked = o.walked.as_ref().unwrap();
    let mut loader = koyomi::calendar::Loader::default();
    for u in &checked.gate.uses {
        if !matches!(u.kind, sekisho::model::UseKind::Dates | sekisho::model::UseKind::Calendar) {
            continue;
        }
        let p = u.file.to_string_lossy().to_string();
        let k = koyomi::check::check_file(&p, &koyomi::check::Options::default(), &mut loader).unwrap();
        let kc = k.checked.as_ref().unwrap();
        let unit = koyomi::codegen::unit_shown(kc, ritsu_base::text::Lang::En, Some(&p));
        write(&base.join("dates").join(format!("{}.py", unit.alias)), &koyomi::codegen::python::module(&unit));
    }
    let target = sekisho::r#gen::Target { name: "python", authorizer, go_module: String::new() };
    let shown = ritsu_emit::header::file_name(gate);
    for (rel, body) in sekisho::r#gen::code(&o, &suite, &target, &shown, lang).unwrap() {
        let rel = rel.strip_prefix("python/").unwrap_or(&rel).to_string();
        write(&base.join(rel), &body);
    }
    Some(o)
}

/// `mypy --strict` over `dir`, as the runner's mypy reads it.
fn mypy(py: &Path, dir: &Path) -> Result<String, String> {
    let out = Command::new(py).args(["-m", "mypy", "--strict", "--no-incremental", "--cache-dir", "/dev/null", "--no-color-output", "--no-error-summary", "."]).current_dir(dir).output().unwrap();
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    if out.status.success() { Ok(text) } else { Err(text) }
}

/// Every `.gate` that passes its check, generated for both authorizers, and with its comments in
/// Japanese: `mypy --strict` reads each package without an error.
#[test]
fn every_package_type_checks() {
    let Some(py) = python() else { return };
    let t = TempDir::new("sekisho-py");
    let mut made = Vec::new();
    let (en, ja) = (ritsu_base::text::Lang::En, ritsu_base::text::Lang::Ja);
    for (i, g) in gates().iter().enumerate() {
        for (k, (authorizer, lang, said)) in [(Authorizer::Cedar, en, "cedar"), (Authorizer::Avp, en, "avp"), (Authorizer::Cedar, ja, "cedar, --lang ja")].into_iter().enumerate() {
            let name = format!("g{i}_{k}");
            if package(g, t.path(), &name, authorizer, lang).is_some() {
                made.push(format!("{g} ({said})"));
            }
        }
    }
    if std::env::var_os("SEKISHO_KEEP").is_some() {
        let keep = std::env::temp_dir().join("sekisho-py-kept");
        let _ = std::fs::remove_dir_all(&keep);
        let _ = Command::new("cp").arg("-R").arg(t.path()).arg(&keep).status();
    }
    match mypy(&py, t.path()) {
        Ok(_) => println!("mypy --strict passes {} packages: {}", made.len(), made.join(", ")),
        Err(said) => panic!("mypy --strict:\n{said}"),
    }
    assert!(made.iter().any(|m| m.starts_with("examples/refunds/refunds.gate")));
}

/// The harness: the cases of the raw values (`Raw::json`) run through the generated module, an
/// answer a line, as `Raw::compare` reads them. The store holds the records the cases give, each
/// made as the module's dataclass of its type; a type with no record (one the code knows by its id
/// alone) is not held.
const HARNESS: &str = r#"import dataclasses
import importlib
import json
import sys
from datetime import date, datetime

m = importlib.import_module(sys.argv[1])
with open(sys.argv[2], encoding="utf-8") as f:
    raw = json.load(f)
types, actions = raw["types"], raw["actions"]


def snake(alias):
    out = ""
    for i, c in enumerate(alias):
        if c.isupper():
            if i > 0 and not out.endswith("_"):
                out += "_"
            out += c.lower()
        else:
            out += c
    return out


def value(kind, v):
    if v is None:
        return None
    return date.fromisoformat(v) if kind == "date" else v


class Store:
    def __init__(self, held):
        self.held = held

    def __getattr__(self, method):
        return lambda id: self.held.get((method, id))


def record(e):
    cls = getattr(m, e["type"], None)
    if cls is None or not dataclasses.is_dataclass(cls):
        return None
    kinds = types[e["type"]]["attrs"]
    args = {}
    for f in dataclasses.fields(cls):
        if f.name == "roles":
            args[f.name] = list(e["roles"])
        elif f.name.startswith("member_of_"):
            args[f.name] = [x["id"] for x in e["member_of"] if "member_of_" + snake(x["type"]) == f.name]
        else:
            args[f.name] = value(kinds.get(f.name), e["attrs"].get(f.name))
    return cls(**args)


for case in raw["cases"]:
    held = {}
    for e in case["store"]:
        r = record(e)
        if r is not None:
            held[(snake(e["type"]), e["id"])] = r
    kinds = actions[case["action"]]["inputs"]
    given = {k: value(kinds.get(k), v) for k, v in case["input"].items()}
    principal = m.Principal(case["principal"]["type"], case["principal"]["id"])
    now = datetime.fromisoformat(case["now"])
    a = getattr(m, "authorize_" + case["action"])(Store(held), principal, given, now)
    print(json.dumps({"name": case["name"], "allowed": a.allowed, "policies": a.policies, "error": a.error.kind if a.error else None, "context": a.context}, ensure_ascii=False))
"#;

/// The answers of the generated Python for every case of the raw values of every `.gate` that
/// passes its check, held to the reference evaluation.
#[test]
fn every_case_is_answered_as_sekisho_answers_it() {
    let Some(py) = python() else { return };
    let suite = common::joined();
    let t = TempDir::new("sekisho-py-cases");
    std::fs::write(t.path().join("harness.py"), HARNESS).unwrap();
    let (mut wrong, mut lines) = (Vec::new(), Vec::new());
    let mut total = 0usize;
    for (i, g) in gates().iter().enumerate() {
        let name = format!("g{i}");
        let Some(o) = package(g, t.path(), &name, Authorizer::Cedar, ritsu_base::text::Lang::En) else { continue };
        let (scope, checked) = (o.scope.as_ref().unwrap(), o.walked.as_ref().unwrap());
        let r = sekisho::raw::raw(scope, checked, &suite);
        let data = t.path().join(format!("{name}.json"));
        std::fs::write(&data, r.json(&checked.gate)).unwrap();
        let module = format!("{name}.authz.{}", checked.gate.named.alias);
        let out = Command::new(&py).arg(t.path().join("harness.py")).arg(&module).arg(&data).current_dir(t.path()).env("PYTHONPATH", t.path()).env("PYTHON_COLORS", "0").output().unwrap();
        if !out.status.success() {
            wrong.push(format!("{g}: the harness fails:\n{}", String::from_utf8_lossy(&out.stderr)));
            continue;
        }
        match r.compare(&String::from_utf8_lossy(&out.stdout)) {
            Ok(0) => lines.push(format!("{g}: no action, so no case")),
            Ok(n) => {
                total += n;
                let (refused, allowed) = r.counts();
                lines.push(format!("{g}: {n} cases ({allowed} allowed, {refused} refused) answered as sekisho answers them"));
            }
            Err(e) => wrong.push(format!("{g}: {e}")),
        }
    }
    for l in &lines {
        println!("{l}");
    }
    println!("{} gates, {total} cases, through cedarpy 4.12.1", lines.len());
    assert!(wrong.is_empty(), "{}", wrong.join("\n\n"));
    assert!(lines.iter().any(|l| l.starts_with("examples/refunds/refunds.gate: ")), "the example is run");
}

/// The command run as a function, every language joined: its exit code, and what it printed on
/// both outputs.
fn sekisho(args: &[&str]) -> (u8, String, String) {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = sekisho::run::run(&args, common::joined(), &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

/// `gen --target python` (DESIGN 5, 11): the module of each gate under `<out>/python/authz/`, its
/// head naming the gate; `--check` finds it current; `--authorizer avp` writes another module,
/// which asks Verified Permissions; `--lang ja` writes its comments in Japanese.
#[test]
fn gen_writes_the_python_of_each_gate() {
    let t = TempDir::new("sekisho-gen-py");
    let out = t.path().join("out").to_string_lossy().to_string();
    let (code, said, err) = sekisho(&["gen", "examples/refunds/refunds.gate", "examples/refunds/refunds.ja.gate", "--target", "python", "--out", &out]);
    assert_eq!(code, 0, "{said}{err}");
    for alias in ["refunds", "refunds_ja"] {
        assert!(said.contains(&format!("generated: {out}/python/authz/{alias}.py\n")), "{said}");
    }
    let text = std::fs::read_to_string(format!("{out}/python/authz/refunds.py")).unwrap();
    let head = format!("# Code generated by sekisho {}. DO NOT EDIT.\n# Source: refunds.gate (gate refunds v1, sha256:", env!("CARGO_PKG_VERSION"));
    assert!(text.starts_with(&head), "{}", &text[..200.min(text.len())]);
    assert!(text.contains("import cedarpy") && text.contains("def authorize_refund_order(store: Store, principal: Principal, input: RefundOrderInput, now: datetime | None = None) -> Answer:"));
    let (code, said, _) = sekisho(&["gen", "examples/refunds/refunds.gate", "examples/refunds/refunds.ja.gate", "--target", "python", "--out", &out, "--check"]);
    assert_eq!((code, said.as_str()), (0, ""));
    // the code of Verified Permissions, in place of cedarpy's
    let (code, said, _) = sekisho(&["gen", "examples/refunds/refunds.gate", "--target", "python", "--authorizer", "avp", "--out", &out, "--check"]);
    assert_eq!(code, 1);
    assert_eq!(said, format!("differs from what gen writes: {out}/python/authz/refunds.py\n"));
    let (code, _, _) = sekisho(&["gen", "examples/refunds/refunds.gate", "--target", "python", "--authorizer", "avp", "--out", &out]);
    assert_eq!(code, 0);
    let avp = std::fs::read_to_string(format!("{out}/python/authz/refunds.py")).unwrap();
    assert!(!avp.contains("import cedarpy") && avp.contains("def authorize_refund_order(avp: VerifiedPermissions, store: Store,"), "{avp}");
    // the comments in Japanese
    let (code, _, _) = sekisho(&["gen", "examples/refunds/refunds.gate", "--target", "python", "--out", &out, "--lang", "ja"]);
    assert_eq!(code, 0);
    let ja = std::fs::read_to_string(format!("{out}/python/authz/refunds.py")).unwrap();
    assert!(ja.contains("# もと: refunds.gate（gate refunds v1、sha256:") && ja.contains("組み立てられなければ SekishoError を投げます"), "{}", &ja[..400.min(ja.len())]);
    // an authorizer it does not know
    let (code, _, err) = sekisho(&["gen", "examples/refunds/refunds.gate", "--target", "python", "--authorizer", "opa", "--out", &out]);
    assert_eq!(code, 2, "{err}");
}
