//! The names of a gate that the code of a target writes as one identifier (W402 when the file is
//! checked, E401 when `gen` writes the target; DESIGN 24).
//!
//! Every gate of tests/spellings gives W402 once, naming the targets [`FORMS`] gives it, and what
//! `check` prints of it, in English and in Japanese, is the golden file beside it
//! (`SEKISHO_BLESS=1 cargo test` writes them again; read the diff). `gen` of each of those targets
//! stops with E401 and writes nothing, and every other target is written. The same gate with the
//! alias its note points to changed (tests/spellings/renamed) gives nothing, and every target is
//! written; that it answers as the reference does, in each language, is tests/typescript.rs's,
//! tests/python.rs's and tests/go.rs's, which walk the files under tests/.
//!
//! With each language's own tools, the code a target would write of each gate breaks for the
//! targets W402 names and for no other, and the code of every renamed gate passes them all: tsc,
//! `mypy --strict`, and `go vet`.

mod common;

use ritsu_base::text::Lang;
use ritsu_testkit::{Need, TempDir, golden};
use sekisho::r#gen::{Authorizer, Target, typescript};
use std::path::{Path, PathBuf};
use std::process::Command;

const EVERY: &[&str] = &["typescript", "python", "go"];
const TS_AND_GO: &[&str] = &["typescript", "go"];
const TS_AND_PY: &[&str] = &["typescript", "python"];

/// Each gate of tests/spellings by its file's name, the English one first and the Japanese one
/// after it, with the targets its W402 names, in the order of the languages.
const FORMS: &[(&str, &[&str])] = &[
    // a type named as the type of what reads the entities
    ("own_store", EVERY),
    ("自分のStore", EVERY),
    // a type named as TypeScript's global and the Go package's type of a date
    ("global_date", TS_AND_GO),
    ("グローバルのDate", TS_AND_GO),
    // an enum TypeScript writes as a type of its module
    ("own_value", &["typescript"]),
    ("自分のValue", &["typescript"]),
    // an enum Python writes as what its module imports
    ("python_literal", &["python"]),
    ("PythonのLiteral", &["python"]),
    // an enum and a type that are one name, where enums are declared
    ("enum_and_type", TS_AND_PY),
    ("列挙と型", TS_AND_PY),
    // a type named as the input of an action
    ("input_type", EVERY),
    ("入力の型", EVERY),
    // two actions with one input
    ("two_actions", EVERY),
    ("二つのaction", EVERY),
    // two attributes that are one field of Go's
    ("go_fields", &["go"]),
    ("Goのフィールド", &["go"]),
    // an attribute that is the field of the roles of Go's
    ("go_roles", &["go"]),
    ("Goの役割", &["go"]),
];

fn every(dir: &str) -> Vec<(String, &'static [&'static str])> {
    FORMS.iter().map(|(stem, named)| (format!("{dir}{stem}.gate"), *named)).collect()
}

fn check(path: &str) -> sekisho::check::Outcome {
    let suite = common::joined();
    sekisho::check::check_file(path, &suite, &sekisho::check::Options::default()).unwrap()
}

#[test]
fn every_gate_gives_w402_once_for_the_targets_it_names_and_says_what_its_golden_files_say() {
    let mut failures = Vec::new();
    for (path, named) in every("tests/spellings/") {
        let o = check(&path);
        let codes: Vec<&str> = o.diags.iter().map(|d| d.code).collect();
        if codes != ["W402"] {
            failures.push(format!("{path} gives {codes:?}, not W402 once"));
            continue;
        }
        let scope = o.scope.as_ref().unwrap();
        let checked = o.walked.as_ref().unwrap();
        let found = sekisho::spellings::find(&checked.gate, &sekisho::cedar::shape(&checked.gate, scope, checked));
        assert_eq!(found.len(), 1, "{path}");
        if found[0].targets() != named {
            failures.push(format!("{path} names {:?}, not {named:?}", found[0].targets()));
        }
        for (lang, ext) in [(Lang::En, "en"), (Lang::Ja, "ja")] {
            golden(&Path::new(&path).with_extension(format!("{ext}.txt")), &sekisho::check::render(&o, lang));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn sekisho_gen(path: &str, target: &str, out: &Path) -> (bool, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_sekisho"));
    let r = c.args(["gen", path, "--target", target, "--out"]).arg(out).output().unwrap();
    (r.status.success(), format!("{}{}", String::from_utf8_lossy(&r.stdout), String::from_utf8_lossy(&r.stderr)))
}

/// `gen` of a target W402 names stops with E401, at the same place, and writes nothing, even of a
/// target given with it; every other target is written.
#[test]
fn gen_of_each_target_it_names_stops_with_e401_and_every_other_is_written() {
    let t = TempDir::new("spellings-gen");
    let mut failures = Vec::new();
    for (k, (path, named)) in every("tests/spellings/").into_iter().enumerate() {
        let o = check(&path);
        let w = &o.diags[0];
        for (i, target) in ["cedar", "typescript", "python", "go"].into_iter().enumerate() {
            let out = t.path().join(format!("{k}-{i}"));
            let (ok, said) = sekisho_gen(&path, target, &out);
            if named.contains(&target) {
                let e = &sekisho::spellings::stops(&o, target)[0];
                if ok || !said.contains("E401") || out.exists() || (e.line, e.col) != (w.line, w.col) {
                    failures.push(format!("{path} {target}: {ok}, written: {}\n{said}", out.exists()));
                }
            } else if !ok || said.contains("E401") {
                failures.push(format!("{path} {target}:\n{said}"));
            }
        }
        // the first of them with one it stops at: nothing of either
        let out = t.path().join(format!("{k}-both"));
        let mut c = Command::new(env!("CARGO_BIN_EXE_sekisho"));
        let r = c.args(["gen", &path, "--target", "cedar", "--target", named[0], "--out"]).arg(&out).output().unwrap();
        if r.status.success() || out.exists() {
            failures.push(format!("{path}: cedar and {} leave {} written", named[0], out.exists()));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_renamed_gates_give_nothing_and_every_target_is_written() {
    let t = TempDir::new("spellings-renamed");
    let mut failures = Vec::new();
    for (k, (path, _)) in every("tests/spellings/renamed/").into_iter().enumerate() {
        let o = check(&path);
        if !o.diags.is_empty() {
            failures.push(format!("{path} gives {:?}", o.diags.iter().map(|d| d.code).collect::<Vec<_>>()));
        }
        for (i, target) in ["cedar", "typescript", "python", "go"].into_iter().enumerate() {
            let (ok, said) = sekisho_gen(&path, target, &t.path().join(format!("{k}-{i}")));
            if !ok {
                failures.push(format!("{path} {target}:\n{said}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// What the tools of `lang` said of each gate: why it breaks, if it does.
struct Broken {
    path: String,
    named: bool,
    why: Option<String>,
}

fn hold(what: &str, seen: Vec<Broken>) {
    let wrong: Vec<String> = seen
        .iter()
        .filter(|b| b.named != b.why.is_some())
        .map(|b| match &b.why {
            Some(w) => format!("{} breaks, and W402 does not name it:\n{w}", b.path),
            None => format!("{}: W402 names it, and it does not break", b.path),
        })
        .collect();
    assert!(wrong.is_empty(), "{what}:\n{}", wrong.join("\n"));
    eprintln!("compared: {what}: {} gates, {} broken as W402 says", seen.len(), seen.iter().filter(|b| b.named).count());
}

fn gates() -> Vec<(String, bool, &'static str)> {
    let mut out = Vec::new();
    for (dir, named) in [("tests/spellings/", true), ("tests/spellings/renamed/", false)] {
        for (path, langs) in every(dir) {
            out.extend(EVERY.iter().map(|l| (path.clone(), named && langs.contains(l), *l)));
        }
    }
    out
}

/// A name of a directory or a package for the `k`th gate.
fn dir_of(k: usize) -> String {
    format!("g{k}")
}

fn code_of(path: &str, target: &str, module: &str) -> Vec<(String, String)> {
    let o = check(path);
    let suite = common::joined();
    let shown = ritsu_emit::header::file_name(path);
    let to = Target { name: target, authorizer: Authorizer::Cedar, go_module: module.to_string(), place: ritsu_emit::package::Place::here() };
    sekisho::r#gen::code(&o, &suite, &to, &shown, Lang::En).unwrap()
}

#[test]
fn typescript_breaks_where_w402_says() {
    if !ritsu_testkit::need(Need::Node) {
        return;
    }
    let runner = std::env::current_dir().unwrap().join("tools/runner-ts");
    let tsc = runner.join("node_modules/.bin/tsc");
    if !tsc.exists() {
        ritsu_testkit::skip("the runner of the TypeScript is not installed; the code of tests/spellings is not type-checked");
        return;
    }
    let t = TempDir::new("spellings-ts");
    let root = t.path();
    std::os::unix::fs::symlink(runner.join("node_modules"), root.join("node_modules")).unwrap();
    std::fs::write(root.join("package.json"), "{ \"private\": true }\n").unwrap();
    std::fs::write(
        root.join("tsconfig.json"),
        format!(
            "{{\"compilerOptions\": {{\"strict\": true, \"noEmit\": true, \"module\": \"nodenext\", \"moduleResolution\": \"nodenext\", \"target\": \"es2022\", \"skipLibCheck\": true, \"typeRoots\": [\"{}\"], \"types\": [\"node\"]}}, \"include\": [\"g*/**/*.ts\"]}}",
            runner.join("node_modules/@types").display()
        ),
    )
    .unwrap();
    let list: Vec<(String, bool, &str)> = gates().into_iter().filter(|(_, _, l)| *l == "typescript").collect();
    for (k, (path, _, _)) in list.iter().enumerate() {
        let o = check(path);
        let suite = common::joined();
        let text = typescript::module(o.scope.as_ref().unwrap(), o.walked.as_ref().unwrap(), &suite, &ritsu_emit::header::file_name(path), Lang::En, Authorizer::Cedar, &ritsu_emit::package::Place::here()).unwrap();
        let p = root.join(dir_of(k)).join("m.ts");
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    let r = ritsu_testkit::run(Command::new(&tsc).args(["-p", ".", "--pretty", "false"]).current_dir(root), std::time::Duration::from_secs(300));
    let said = r.both();
    let seen = list
        .iter()
        .enumerate()
        .map(|(k, (path, named, _))| {
            let key = format!("{}/", dir_of(k));
            let mut errors: Vec<String> = said.lines().filter(|l| l.starts_with(&key)).map(String::from).collect();
            // two interfaces of one name are one to tsc, which asks of a caller what both ask
            let text = std::fs::read_to_string(root.join(dir_of(k)).join("m.ts")).unwrap();
            let heads: Vec<&str> = text.lines().filter(|l| l.starts_with("export interface ")).collect();
            errors.extend(heads.iter().enumerate().filter(|(i, h)| heads[..*i].contains(h)).map(|(_, h)| format!("two of `{h}`")));
            Broken { path: path.clone(), named: *named, why: (!errors.is_empty()).then(|| errors.join("\n")) }
        })
        .collect();
    hold("tsc", seen);
}

fn runner_python() -> Option<PathBuf> {
    if !ritsu_testkit::need(Need::Python) {
        return None;
    }
    let py = Some(std::env::current_dir().unwrap().join("tools/runner-py/.venv/bin/python")).filter(|p| p.is_file());
    if py.is_none() {
        ritsu_testkit::skip("tools/runner-py/.venv is missing; the code of tests/spellings is not type-checked");
    }
    py
}

#[test]
fn python_breaks_where_w402_says() {
    let Some(py) = runner_python() else { return };
    let t = TempDir::new("spellings-py");
    let list: Vec<(String, bool, &str)> = gates().into_iter().filter(|(_, _, l)| *l == "python").collect();
    let mut seen = Vec::new();
    for (k, (path, named, _)) in list.iter().enumerate() {
        let dir = t.path().join(dir_of(k));
        for (rel, text) in code_of(path, "python", "") {
            let p = dir.join(rel.strip_prefix("python/").unwrap_or(&rel));
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        }
        std::fs::write(dir.join("authz/__init__.py"), "").unwrap();
        let out = Command::new(&py).args(["-m", "mypy", "--strict", "--no-incremental", "--cache-dir", "/dev/null", "--no-color-output", "--no-error-summary", "."]).current_dir(&dir).output().unwrap();
        let said = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
        seen.push(Broken { path: path.clone(), named: *named, why: (!out.status.success()).then_some(said) });
    }
    hold("mypy --strict", seen);
}

#[test]
fn go_breaks_where_w402_says() {
    if !ritsu_testkit::need(Need::Go) {
        return;
    }
    let runner = std::env::current_dir().unwrap().join("tools/runner-go");
    if !runner.join("go.mod").is_file() || !ritsu_testkit::tools::runs("go", &["version"]) {
        ritsu_testkit::skip("go or tools/runner-go is missing; the code of tests/spellings is not vetted");
        return;
    }
    let t = TempDir::new("spellings-go");
    let module = t.path();
    for f in ["go.mod", "go.sum"] {
        std::fs::copy(runner.join(f), module.join(f)).unwrap();
    }
    let path_of = std::fs::read_to_string(runner.join("go.mod")).unwrap().lines().find_map(|l| l.strip_prefix("module ").map(|m| m.trim().to_string())).unwrap();
    let list: Vec<(String, bool, &str)> = gates().into_iter().filter(|(_, _, l)| *l == "go").collect();
    for (k, (path, _, _)) in list.iter().enumerate() {
        for (rel, text) in code_of(path, "go", &format!("{path_of}/{}", dir_of(k))) {
            let p = module.join(dir_of(k)).join(rel.strip_prefix("go/").unwrap_or(&rel));
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        }
    }
    let out = Command::new("go").args(["vet", "./..."]).current_dir(module).env("GOWORK", "off").env("GOFLAGS", "-trimpath").env("GOPROXY", "off").env("GOTOOLCHAIN", "local").output().unwrap();
    let said = String::from_utf8_lossy(&out.stderr).to_string();
    let seen = list
        .iter()
        .enumerate()
        .map(|(k, (path, named, _))| {
            let key = format!("{}/", dir_of(k));
            let errors: Vec<&str> = said.lines().filter(|l| l.contains(&key)).collect();
            Broken { path: path.clone(), named: *named, why: (!errors.is_empty()).then(|| errors.join("\n")) }
        })
        .collect();
    hold("go vet", seen);
}
