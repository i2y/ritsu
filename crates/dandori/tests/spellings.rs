//! The names a target's code cannot keep (src/spellings.rs, DESIGN 22). Each form under
//! tests/spellings, in English and in Japanese, is said by `check` (W105) in both languages, stops
//! `gen` for the targets it names and no other (E051), and, renamed as the note says, is said
//! nothing and builds everywhere. The code of the targets a form does not name, and of its renamed
//! flow everywhere, passes `tsc --strict`, `mypy --strict` and `go vet`; and a generator made to write
//! what `gen` stops at writes Go or Python that does not pass.
//! `DANDORI_BLESS=1` rewrites the golden files.

use ritsu_testkit::{ready, skip, Need, TempDir};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::rc::Rc;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

struct Form {
    en: &'static str,
    ja: &'static str,
    /// the targets it stops
    targets: &'static [&'static str],
    /// what a generator made to write it writes that does not pass: Go, and Python
    broken: &'static [&'static str],
}

const GO: &[&str] = &["temporal-go"];
const TS_TYPE: &[&str] = &["temporal-typescript", "durable"];
const PY_TYPE: &[&str] = &["temporal-python", "pydantic-graph"];
const TS_PY: &[&str] = &["temporal-typescript", "durable", "temporal-python", "pydantic-graph"];
const PY_GO: &[&str] = &["temporal-python", "pydantic-graph", "temporal-go"];
const EVERY_LANGUAGE: &[&str] = &["temporal-typescript", "durable", "temporal-python", "pydantic-graph", "temporal-go"];
const CLIENTS: &[&str] = &["temporal-typescript", "temporal-python", "temporal-go"];
const CLIENT_TS_PY: &[&str] = &["temporal-typescript", "temporal-python"];

const FORMS: &[Form] = &[
    Form { en: "go_own", ja: "Goの自分の名前", targets: GO, broken: &["temporal-go"] },
    Form { en: "go_two_types", ja: "Goの二つの型", targets: GO, broken: &["temporal-go"] },
    Form { en: "go_values", ja: "GoのValues", targets: GO, broken: &["temporal-go"] },
    Form { en: "go_is", ja: "GoのIs", targets: GO, broken: &["temporal-go"] },
    Form { en: "go_fields", ja: "Goのフィールド", targets: GO, broken: &["temporal-go"] },
    Form { en: "go_inputs", ja: "Goの入力", targets: GO, broken: &["temporal-go"] },
    Form { en: "go_methods", ja: "Goのメソッド", targets: GO, broken: &["temporal-go"] },
    Form { en: "go_encrypted", ja: "Goの暗号化", targets: GO, broken: &["temporal-go"] },
    Form { en: "ts_global", ja: "TypeScriptのグローバル", targets: TS_TYPE, broken: &[] },
    Form { en: "workflow_input", ja: "自分のWorkflowInput", targets: EVERY_LANGUAGE, broken: &["temporal-go"] },
    Form { en: "leading_underscore", ja: "始めが_の名前", targets: TS_PY, broken: &[] },
    Form { en: "python_keyword", ja: "Pythonのキーワード", targets: PY_TYPE, broken: &[] },
    Form { en: "python_builtin", ja: "Pythonの組み込み", targets: PY_TYPE, broken: &[] },
    Form { en: "python_own", ja: "Pythonの自分の名前", targets: PY_TYPE, broken: &[] },
    Form { en: "python_values", ja: "Pythonのvalues", targets: PY_GO, broken: &["temporal-python", "temporal-go"] },
    Form { en: "python_is", ja: "Pythonのis", targets: PY_GO, broken: &["temporal-python", "temporal-go"] },
    Form { en: "client_send", ja: "クライアントのsend", targets: CLIENTS, broken: &[] },
    Form { en: "client_message", ja: "クライアントのメッセージ", targets: CLIENT_TS_PY, broken: &[] },
    Form { en: "client_two_types", ja: "クライアントの二つの型", targets: CLIENTS, broken: &[] },
];

fn run(args: &[&str]) -> (u8, String, String) {
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = dandori::cli::run(&args, Rc::new(dandori::sources::NoRules), &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

fn files_under(dir: &Path) -> usize {
    let Ok(rd) = std::fs::read_dir(dir) else { return 0 };
    rd.flatten().map(|e| if e.path().is_dir() { files_under(&e.path()) } else { 1 }).sum()
}

/// What `gen` says for a flow and a target: its code, what it prints, the files it wrote.
fn gen_for(flow: &str, target: &str, scratch: &Path) -> (u8, String, usize) {
    let out = scratch.join("gen");
    let _ = std::fs::remove_dir_all(&out);
    let (code, o, e) = run(&["gen", flow, "--target", target, "--out", out.to_str().unwrap()]);
    let files = files_under(&out);
    let _ = std::fs::remove_dir_all(&out);
    (code, format!("{o}{e}"), files)
}

#[test]
fn every_form_is_said_and_stops_the_targets_it_names() {
    let bless = std::env::var_os("DANDORI_BLESS").is_some() || std::env::var_os("RITSU_BLESS").is_some();
    let scratch = TempDir::new("dandori-spellings");
    let mut wrong = Vec::new();
    for form in FORMS {
        for stem in [form.en, form.ja] {
            let flow = format!("tests/spellings/{stem}.flow");
            // what check says, in both languages
            for lang in ["en", "ja"] {
                let (code, o, e) = run(&["check", &flow, "--lang", lang]);
                assert_eq!(code, 0, "{flow}: {o}{e}");
                let text = format!("{o}{e}");
                assert!(text.contains("W105"), "{flow}: {text}");
                let golden = root().join(format!("tests/spellings/{stem}.{lang}.txt"));
                if bless {
                    std::fs::write(&golden, &text).unwrap();
                } else if std::fs::read_to_string(&golden).unwrap_or_default() != text {
                    wrong.push(format!("{} is not what check says now:\n{text}", golden.display()));
                }
            }
            let (_, json, _) = run(&["check", &flow, "--format", "json"]);
            let v: serde_json::Value = serde_json::from_str(&json).unwrap_or_else(|e| panic!("{flow}: {e}: {json}"));
            assert!(json.contains("\"W105\""), "{flow}: {v}");
            for target in dandori::commands::TARGETS {
                let (code, said, files) = gen_for(&flow, target, scratch.path());
                if form.targets.contains(&target) {
                    assert!(code == 1 && said.contains("E051") && files == 0, "{flow} --target {target}: exit {code}, {files} file(s):\n{said}");
                } else {
                    assert!(!said.contains("E051"), "{flow} --target {target}:\n{said}");
                }
            }
            // renamed as the note says: nothing is said, and no target stops at a name
            let renamed = format!("tests/spellings/renamed/{stem}.flow");
            let (code, o, e) = run(&["check", &renamed]);
            assert!(code == 0 && !format!("{o}{e}").contains("warning["), "{renamed}: {o}{e}");
            for target in dandori::commands::TARGETS {
                let (code, said, files) = gen_for(&renamed, target, scratch.path());
                assert!(!said.contains("E051"), "{renamed} --target {target}:\n{said}");
                if form.targets.contains(&target) {
                    assert!(code == 0 && files > 0, "{renamed} --target {target}: exit {code}:\n{said}");
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    if bless {
        skip("the golden files are being rewritten; run this test again without DANDORI_BLESS");
    }
}

/// `gen`'s stop has the names of the code, and how to put it right, in the words of the message.
#[test]
fn gen_says_what_stopped_it() {
    let scratch = TempDir::new("dandori-spellings-gen");
    let (code, said, files) = gen_for("tests/spellings/go_methods.flow", "temporal-go", scratch.path());
    assert!(code == 1 && files == 0, "{said}");
    assert!(said.contains("error[E051]") && said.contains("`send_mail`") && said.contains("`sendMail`") && said.contains("`SendMail`") && said.contains("rename it in the `.flow`"), "{said}");
}

/// A message two methods read as one type is one alias: nothing is said of it, and no client writes
/// it twice. A record of the flow with the message's name is the type itself, in Go, where an alias
/// could not have the name.
#[test]
fn a_message_two_methods_share_is_one_type() {
    let scratch = TempDir::new("dandori-spellings-shared");
    for stem in ["client_shared", "クライアントの共有"] {
        let flow = format!("tests/spellings/{stem}.flow");
        let (code, o, e) = run(&["check", &flow]);
        assert!(code == 0 && !format!("{o}{e}").contains("warning["), "{flow}: {o}{e}");
        let out = scratch.path().join(stem);
        for (target, file, head, times) in [("temporal-typescript", "client.ts", "export type Sent = ", 1), ("temporal-python", "client.py", "\nSent = ", 1), ("temporal-go", "client.go", "type Sent", 0)] {
            let (code, _, e) = run(&["gen", &flow, "--target", target, "--out", out.to_str().unwrap()]);
            assert_eq!(code, 0, "{flow} --target {target}: {e}");
            let dir = std::fs::read_dir(out.join(target)).unwrap().flatten().next().unwrap().path();
            let text = std::fs::read_to_string(dir.join(file)).unwrap();
            assert_eq!(text.matches(head).count(), times, "{flow} --target {target}:\n{text}");
        }
    }
}

/// A record `Books` in a flow that runs a book's operations: `io_books.go` declares `Books` in a
/// package of `ritsu gen`, so `check` says so, and only `gen` in a package stops.
#[test]
fn the_books_of_a_package_stop_only_in_a_package() {
    for flow in ["tests/spellings/go_books.flow", "tests/spellings/Goの帳簿.flow"] {
        let (_, c) = dandori::sources::with_ports(Rc::new(dandori::sources::NoRules), Rc::new(koyomi::ports::Engine), Rc::new(chobo::ports::Engine), || dandori::check::check_file(Path::new(flow))).unwrap();
        assert_eq!(c.diags.iter().map(|d| d.code).collect::<Vec<_>>(), ["W105"], "{flow}");
        assert!(c.diags[0].en.contains("`Books`") && c.diags[0].notes.iter().any(|n| n.en.contains("`ritsu gen`")), "{:?}", c.diags[0]);
        let mut m = c.model.expect("the flow passes");
        assert!(dandori::commands::build(&m, "temporal-go").unwrap().is_ok(), "{flow}: alone, a flow's package has no `Books`");
        m.package = Some(dandori::model::InPackage { go_module: "shop".to_string(), place: ritsu_emit::package::Place::here() });
        let stopped = dandori::commands::build(&m, "temporal-go").unwrap().err().expect("in a package it stops");
        assert_eq!(stopped.iter().map(|d| d.code).collect::<Vec<_>>(), ["E051"], "{flow}");
        assert!(dandori::commands::build(&m, "temporal-typescript").unwrap().is_ok(), "{flow}");
    }
}

fn said(out: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr))
}

/// Each file of `flow`'s build for `target` under `dir`.
fn write_files(flow: &str, target: &str, dir: &Path) {
    let (_, c) = dandori::check::check_file(Path::new(flow)).unwrap();
    let m = c.model.unwrap_or_else(|| panic!("{flow} passes check"));
    let files = dandori::commands::build(&m, target).unwrap().unwrap_or_else(|d| panic!("{flow} --target {target}: {d:?}"));
    for (name, text) in files {
        let p = dir.join(name);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
}

/// Every flow whose code `target` writes: each form's renamed flow, the form itself when it does not
/// stop `target`, and the flow of the shared message.
fn buildable(target: &'static str) -> Vec<String> {
    let mut out = Vec::new();
    for form in FORMS {
        for stem in [form.en, form.ja] {
            out.push(format!("tests/spellings/renamed/{stem}.flow"));
            if !form.targets.contains(&target) {
                out.push(format!("tests/spellings/{stem}.flow"));
            }
        }
    }
    out.push("tests/spellings/client_shared.flow".to_string());
    out.push("tests/spellings/クライアントの共有.flow".to_string());
    out
}

/// The code of each of them, in a directory of its own: a form and its renamed flow have one
/// workflow's name.
fn build_each(target: &'static str, dir: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for (i, flow) in buildable(target).iter().enumerate() {
        let d = dir.join(format!("f{i}"));
        write_files(flow, target, &d);
        dirs.push(d);
    }
    dirs
}

fn go_in(dir: &Path) -> Command {
    let mut go = Command::new("go");
    go.current_dir(dir).env("GOWORK", "off").env("GOFLAGS", "-trimpath");
    go
}

fn go_module(dir: &Path) {
    let tools = root().join("tools/temporal-go");
    let gomod = std::fs::read_to_string(tools.join("go.mod")).unwrap();
    let gomod = gomod.lines().map(|l| if l.starts_with("module ") { "module spellingcheck".to_string() } else { l.to_string() }).collect::<Vec<_>>().join("\n") + "\n";
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join("go.mod"), gomod).unwrap();
    std::fs::copy(tools.join("go.sum"), dir.join("go.sum")).unwrap();
}

#[test]
fn the_code_of_what_is_not_stopped_passes_go_vet() {
    let has_go = || Command::new("go").arg("version").output().is_ok_and(|o| o.status.success()) && root().join("tools/temporal-go/go.mod").exists();
    if !ready(Need::Go, has_go, "go or tools/temporal-go is missing") {
        return;
    }
    let scratch = TempDir::new("dandori-spellings-go");
    let dir = scratch.path().join("m");
    go_module(&dir);
    build_each("temporal-go", &dir.join("flows"));
    let out = go_in(&dir).args(["vet", "./flows/..."]).output().unwrap();
    assert!(out.status.success(), "go vet refuses the Go of what is not stopped:\n{}", said(&out));
    eprintln!("go vet: compared {} flows", buildable("temporal-go").len());
}

#[test]
fn the_code_of_what_is_not_stopped_passes_tsc() {
    let tsc = root().join("tools/temporal/node_modules/.bin/tsc");
    if !ready(Need::Node, || tsc.exists(), "TypeScript is not in tools/temporal/node_modules; run `npm install --prefix tools/temporal`") {
        return;
    }
    let scratch = TempDir::new("dandori-spellings-ts");
    let dir = scratch.path();
    build_each("temporal-typescript", dir);
    std::fs::write(dir.join("shims.d.ts"), "declare module \"@anthropic-ai/sdk\";\ndeclare module \"openai\";\ndeclare module \"@openai/agents\";\ndeclare module \"@aws-sdk/*\";\n").unwrap();
    std::os::unix::fs::symlink(root().join("tools/temporal/node_modules"), dir.join("node_modules")).unwrap();
    let tsconfig = serde_json::json!({
        "compilerOptions": { "strict": true, "noEmit": true, "module": "nodenext", "moduleResolution": "nodenext", "target": "es2022", "skipLibCheck": true, "types": ["node"], "typeRoots": [root().join("tools/temporal/node_modules/@types")] },
        "include": ["*.ts", "f*/**/*.ts"]
    });
    std::fs::write(dir.join("tsconfig.json"), serde_json::to_string_pretty(&tsconfig).unwrap()).unwrap();
    let out = Command::new(&tsc).arg("-p").arg(dir.join("tsconfig.json")).output().unwrap();
    assert!(out.status.success(), "tsc --strict refuses the TypeScript of what is not stopped:\n{}", said(&out));
    eprintln!("tsc --strict: compared {} flows", buildable("temporal-typescript").len());
}

/// `mypy --strict` in the directory of each package at once: the errors of a workflow that implements
/// a service, which `start` taking a dict has always given (tests/flows/service.flow), are not this
/// test's.
fn mypy_errors(dirs: &[PathBuf]) -> Option<Vec<String>> {
    let python = root().join("tools/temporal-python/.venv/bin/python");
    let mypy = std::env::var_os("DANDORI_MYPY").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("mypy"));
    let has = || python.exists() && Command::new(&mypy).arg("--version").output().is_ok_and(|o| o.status.success());
    if !ready(Need::Python, has, "mypy (on PATH, or DANDORI_MYPY) or tools/temporal-python/.venv is missing") {
        return None;
    }
    let mut errors = Vec::new();
    for d in dirs {
        let Some(package) = std::fs::read_dir(d).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).find(|n| n != "cache") else { continue };
        let out = Command::new(&mypy).current_dir(d).args(["--strict", "--no-color-output", "--cache-dir", "../cache", "--python-executable"]).arg(&python).args(["-p", &package]).output().unwrap();
        let text = said(&out);
        assert!(text.contains("source file"), "mypy did not check {}:\n{text}", d.display());
        errors.extend(text.lines().filter(|l| l.contains(": error:") && !(l.contains("Argument 3 to \"start\"") && l.ends_with("[arg-type]"))).map(|l| format!("{package}: {l}")));
    }
    Some(errors)
}

#[test]
fn the_code_of_what_is_not_stopped_passes_mypy() {
    let scratch = TempDir::new("dandori-spellings-py");
    let dirs = build_each("temporal-python", scratch.path());
    let Some(errors) = mypy_errors(&dirs) else { return };
    assert!(errors.is_empty(), "mypy --strict finds fault with the Python of what is not stopped:\n{}", errors.join("\n"));
    eprintln!("mypy --strict: compared {} flows", dirs.len());
}

/// What `gen` stops at is what would not work: the generators, called past the stop, write Go that
/// `go vet` refuses and Python that `mypy --strict` finds a name defined twice in.
#[test]
fn what_stops_would_not_work() {
    let write = |flow: &str, target: &str, dir: &Path| {
        let (_, c) = dandori::check::check_file(Path::new(flow)).unwrap();
        let m = c.model.unwrap_or_else(|| panic!("{flow} passes check"));
        let files = match target {
            "temporal-go" => dandori::temporal_go::build(&m),
            _ => dandori::temporal_py::build(&m),
        };
        for (name, text) in files.unwrap_or_else(|d| panic!("{flow}: {d:?}")) {
            let p = dir.join(name);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        }
    };
    // Go
    let has_go = || Command::new("go").arg("version").output().is_ok_and(|o| o.status.success()) && root().join("tools/temporal-go/go.mod").exists();
    if ready(Need::Go, has_go, "go or tools/temporal-go is missing") {
        for form in FORMS.iter().filter(|f| f.broken.contains(&"temporal-go")) {
            let scratch = TempDir::new("dandori-spellings-broken-go");
            let dir = scratch.path().join("m");
            go_module(&dir);
            write(&format!("tests/spellings/{}.flow", form.en), "temporal-go", &dir.join("flows"));
            let out = go_in(&dir).args(["vet", "./flows/..."]).output().unwrap();
            let text = said(&out);
            assert!(!out.status.success() && (text.contains("redeclared") || text.contains("duplicate") || text.contains("already declared")), "{}: go vet passes the code past the stop:\n{text}", form.en);
        }
        eprintln!("go vet: refused the code past the stop");
    }
    // Python
    let scratch = TempDir::new("dandori-spellings-broken-py");
    let mut dirs = Vec::new();
    for form in FORMS.iter().filter(|f| f.broken.contains(&"temporal-python")) {
        let d = scratch.path().join(form.en);
        write(&format!("tests/spellings/{}.flow", form.en), "temporal-python", &d);
        dirs.push(d);
    }
    if let Some(errors) = mypy_errors(&dirs) {
        for form in FORMS.iter().filter(|f| f.broken.contains(&"temporal-python")) {
            assert!(errors.iter().any(|e| e.starts_with(&format!("{}: ", form.en)) && (e.contains("already defined") || e.contains("[no-redef]") || e.contains("[misc]"))), "{}: mypy passes the code past the stop:\n{}", form.en, errors.join("\n"));
        }
        eprintln!("mypy --strict: refused the code past the stop");
    }
}

/// Every flow of the directory is a form above.
#[test]
fn the_form_list_is_every_flow_of_the_directory() {
    let mut names: Vec<String> = FORMS.iter().flat_map(|f| [f.en.to_string(), f.ja.to_string()]).collect();
    names.extend(["client_shared", "クライアントの共有", "go_books", "Goの帳簿"].map(String::from));
    names.sort();
    let mut files: Vec<String> = std::fs::read_dir(root().join("tests/spellings")).unwrap().flatten().filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(".flow")).map(String::from)).collect();
    files.sort();
    assert_eq!(files, names);
}
