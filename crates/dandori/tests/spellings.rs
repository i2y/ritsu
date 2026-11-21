//! The names a target's code cannot keep (src/spellings.rs, DESIGN 22). Each form under
//! tests/spellings, in English and in Japanese, is said by `check` (W105) in both languages, stops
//! `gen` for the targets it names and no other (E051), and, renamed as the note says, is said
//! nothing and builds everywhere. The code of the targets a form does not name, and of its renamed
//! flow everywhere, passes `tsc --strict`, `mypy --strict` and `go vet`; and a generator made to write
//! what `gen` stops at writes Go or Python that does not pass. A name the code has for itself alone
//! stops nothing: the code gives its own another name, and the flows that take such names
//! (tests/flows/*_names.flow, python_tasks.flow, which also run on the platforms' runners) pass the
//! same checks. What the tables keep is what the code of every flow of the tree reads, no more and
//! no less; a name it does not read keeps its name and stops nothing (tests/flows/plain_*.flow). A
//! variable of a state machine named as a function of JSONata the machines call goes by another
//! name, which no caller reads (tests/flows/jsonata_names.flow, which runs on them too).
//! `DANDORI_BLESS=1` rewrites the golden files.

use ritsu_testkit::{need, ready, skip, Need, TempDir};
use std::collections::BTreeSet;
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
/// a service's methods and messages are in client.ts, which temporal-typescript alone writes
const CLIENT_TS: &[&str] = &["temporal-typescript"];
/// what two types of one name stop: TypeScript, Python and Go, whose callers read both of them; the
/// caller of Argo keeps them apart, and the state machine has no types
const TWO_TYPES: &[&str] = &["temporal-typescript", "durable", "temporal-python", "pydantic-graph", "temporal-go"];

const FORMS: &[Form] = &[
    Form { en: "go_own", ja: "Goの自分の名前", targets: GO, broken: &["temporal-go"] },
    Form { en: "go_two_types", ja: "Goの二つの型", targets: GO, broken: &["temporal-go"] },
    Form { en: "go_values", ja: "GoのValues", targets: GO, broken: &["temporal-go"] },
    Form { en: "go_is", ja: "GoのIs", targets: GO, broken: &["temporal-go"] },
    Form { en: "go_fields", ja: "Goのフィールド", targets: GO, broken: &["temporal-go"] },
    Form { en: "go_inputs", ja: "Goの入力", targets: GO, broken: &["temporal-go"] },
    Form { en: "go_methods", ja: "Goのメソッド", targets: GO, broken: &["temporal-go"] },
    Form { en: "go_encrypted", ja: "Goの暗号化", targets: GO, broken: &["temporal-go"] },
    Form { en: "go_book_names", ja: "Goの帳簿の名前", targets: GO, broken: &["temporal-go"] },
    Form { en: "ts_word", ja: "TypeScriptの語", targets: TS_TYPE, broken: &[] },
    Form { en: "ts_keyword", ja: "TypeScriptのキーワード", targets: TS_TYPE, broken: &[] },
    Form { en: "workflow_input", ja: "自分のWorkflowInput", targets: EVERY_LANGUAGE, broken: &["temporal-go"] },
    Form { en: "leading_underscore", ja: "始めが_の名前", targets: TS_PY, broken: &[] },
    Form { en: "python_keyword", ja: "Pythonのキーワード", targets: PY_TYPE, broken: &[] },
    Form { en: "proto_type_name", ja: "protoの型の名前", targets: TWO_TYPES, broken: &["temporal-python", "temporal-go"] },
    Form { en: "python_values", ja: "Pythonのvalues", targets: PY_GO, broken: &["temporal-python", "temporal-go"] },
    Form { en: "python_is", ja: "Pythonのis", targets: PY_GO, broken: &["temporal-python", "temporal-go"] },
    Form { en: "client_send", ja: "クライアントのsend", targets: CLIENTS, broken: &[] },
    Form { en: "client_send_starts", ja: "クライアントのsendで始める", targets: CLIENTS, broken: &[] },
    Form { en: "client_send_once", ja: "クライアントのSendOnce", targets: CLIENTS, broken: &[] },
    Form { en: "client_histories_status", ja: "クライアントのhistoriesで状態", targets: CLIENTS, broken: &[] },
    Form { en: "client_two_types", ja: "クライアントの二つの型", targets: CLIENTS, broken: &[] },
    Form { en: "client_sent_error", ja: "クライアントのSentWithAnotherValueError", targets: CLIENTS, broken: &[] },
    Form { en: "client_answer", ja: "クライアントのanswer", targets: CLIENT_TS_PY, broken: &[] },
    Form { en: "client_answer_callback", ja: "クライアントのAnswerCallback", targets: GO, broken: &[] },
    Form { en: "client_run_status_starts", ja: "クライアントのRunStatusで始める", targets: GO, broken: &[] },
    Form { en: "client_status_type", ja: "クライアントのStatusのレコード", targets: GO, broken: &[] },
    Form { en: "client_keyword", ja: "クライアントのキーワード", targets: CLIENT_TS, broken: &[] },
];

/// The flows no name stops, in English and in Japanese: what a form above would be, but for a name
/// the code does not keep, or keeps for itself alone and gives another name where the flow takes it
/// (DESIGN 22). The flows of tests/flows run on the platforms' runners too.
const FREE: &[&str] = &[
    "tests/spellings/date_name.flow",
    "tests/spellings/自分のDATE.flow",
    "tests/spellings/ts_global.flow",
    "tests/spellings/TypeScriptのグローバル.flow",
    "tests/spellings/python_builtin.flow",
    "tests/spellings/Pythonの組み込み.flow",
    "tests/spellings/python_own.flow",
    "tests/spellings/Pythonの自分の名前.flow",
    "tests/spellings/python_local.flow",
    "tests/spellings/Pythonのローカル.flow",
    "tests/spellings/client_message.flow",
    "tests/spellings/クライアントのメッセージ.flow",
    "tests/spellings/client_global.flow",
    "tests/spellings/クライアントのグローバル.flow",
    "tests/spellings/client_module.flow",
    "tests/spellings/クライアントのモジュール.flow",
    "tests/spellings/client_shared.flow",
    "tests/spellings/クライアントの共有.flow",
    "tests/spellings/client_status.flow",
    "tests/spellings/クライアントのstatus.flow",
    "tests/spellings/client_run_status.flow",
    "tests/spellings/クライアントのRunStatus.flow",
    "tests/spellings/client_status_beside.flow",
    "tests/spellings/クライアントのStatusと並ぶレコード.flow",
    "tests/spellings/client_same.flow",
    "tests/spellings/クライアントの同じ関数.flow",
    "tests/spellings/go_no_date_no_book.flow",
    "tests/spellings/Goの日付も帳簿も無いフロー.flow",
    "tests/flows/plain_names.flow",
    "tests/flows/plain_names.ja.flow",
    "tests/flows/plain_tasks.flow",
    "tests/flows/plain_tasks.ja.flow",
    "tests/flows/jsonata_names.flow",
    "tests/flows/jsonata_names.ja.flow",
    "tests/flows/python_names.flow",
    "tests/flows/python_names.ja.flow",
    "tests/flows/python_tasks.flow",
    "tests/flows/python_tasks.ja.flow",
    "tests/flows/ts_names.flow",
    "tests/flows/ts_names.ja.flow",
    "tests/flows/go_names.flow",
    "tests/flows/go_names.ja.flow",
    "tests/flows/service_date_names.flow",
    "tests/flows/service_date_names.ja.flow",
    "tests/flows/variable_names.flow",
    "tests/flows/variable_names.ja.flow",
];

/// `dandori` with koyomi's and chobo's answers, as a program that joins them runs it: a form may
/// have a book.
fn run(args: &[&str]) -> (u8, String, String) {
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = dandori::cli::run_with_ports(&args, Rc::new(dandori::sources::NoRules), Rc::new(koyomi::ports::Engine), Rc::new(chobo::ports::Engine), &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

/// What `check` makes of `flow`, read as `run` reads it.
fn checked(flow: &str) -> dandori::check::Checked {
    dandori::sources::with_ports(Rc::new(dandori::sources::NoRules), Rc::new(koyomi::ports::Engine), Rc::new(chobo::ports::Engine), || dandori::check::check_file(Path::new(flow))).unwrap().1
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

/// `dandori` with rulec's, koyomi's and chobo's answers, as `ritsu dandori` runs it.
fn run_ruled(args: &[&str]) -> (u8, String, String) {
    let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let rules = rulec::ports::Engine::with_dates(std::sync::Arc::new(koyomi::ports::Engine));
    let code = dandori::cli::run_with_ports(&args, Rc::new(rules), Rc::new(koyomi::ports::Engine), Rc::new(chobo::ports::Engine), &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

/// The file under `dir` whose path ends with `end`.
fn file_under(dir: &Path, end: &str) -> Option<PathBuf> {
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        let p = e.path();
        if p.is_dir() {
            if let Some(f) = file_under(&p, end) {
                return Some(f);
            }
        } else if p.to_string_lossy().ends_with(end) {
            return Some(p);
        }
    }
    None
}

/// Two types that `types.ts`, `types.py` and `types.go` write by one name (a record `shop_Line`
/// beside the message `shop.Line` of a `.proto`, the form proto_type_name; a record
/// `urgency_carrier` beside the enum `urgency.carrier` of a rule) stop TypeScript, Python and Go,
/// whose callers read both. The caller of Argo, whose types no caller reads, keeps them apart: the
/// flow's own goes by its name with `_` after it, each name is declared once, and Node runs it. The
/// state machine has no types. A run that names a target that stops writes no target, though it
/// names one that does not stop. Renamed as the note says, the flow is said nothing, and every
/// target writes it.
#[test]
fn argo_keeps_two_types_of_one_name_apart() {
    let scratch = TempDir::new("dandori-spellings-apart");
    let has_node = || Command::new("node").arg("--version").output().is_ok_and(|o| o.status.success());
    let node = ready(Need::Node, has_node, "node is missing");
    let gen = |flow: &str, targets: &[&str], out: &Path| {
        let _ = std::fs::remove_dir_all(out);
        let out_s = out.to_str().unwrap().to_string();
        let mut args = vec!["gen", flow];
        for t in targets {
            args.extend(["--target", *t]);
        }
        args.extend(["--out", out_s.as_str()]);
        let (code, o, e) = run_ruled(&args);
        (code, format!("{o}{e}"), files_under(out))
    };
    for (stem, own) in [("proto_type_name", "shop_Line"), ("protoの型の名前", "店_Item"), ("rule_type_name", "urgency_carrier"), ("規則の型の名前", "急ぎ_便")] {
        let flow = format!("tests/spellings/{stem}.flow");
        let (code, o, e) = run_ruled(&["check", &flow]);
        assert!(code == 0 && format!("{o}{e}").contains("W105"), "{flow}: exit {code}:\n{o}{e}");
        let out = scratch.path().join(stem);
        for target in dandori::commands::TARGETS {
            let (code, printed, files) = gen(&flow, &[target], &out);
            if TWO_TYPES.contains(&target) {
                assert!(code == 1 && printed.contains("E051") && files == 0, "{flow} --target {target}: exit {code}, {files} file(s):\n{printed}");
            } else {
                assert!(code == 0 && files > 0 && !printed.contains("E051"), "{flow} --target {target}: exit {code}, {files} file(s):\n{printed}");
            }
        }
        let (code, printed, files) = gen(&flow, &["asl", "temporal-python"], &out);
        assert!(code == 1 && printed.contains("E051") && files == 0, "{flow} --target asl --target temporal-python: exit {code}, {files} file(s):\n{printed}");
        let renamed = format!("tests/spellings/renamed/{stem}.flow");
        let (code, o, e) = run_ruled(&["check", &renamed]);
        assert!(code == 0 && !format!("{o}{e}").contains("warning["), "{renamed}: exit {code}:\n{o}{e}");
        for target in dandori::commands::TARGETS {
            let (code, printed, files) = gen(&renamed, &[target], &out);
            assert!(code == 0 && files > 0 && !printed.contains("E051"), "{renamed} --target {target}: exit {code}, {files} file(s):\n{printed}");
        }
        let (code, printed, _) = gen(&flow, &["argo"], &out);
        assert_eq!(code, 0, "{flow} --target argo:\n{printed}");
        let types = file_under(&out, "caller/types.ts").unwrap_or_else(|| panic!("{flow} --target argo: no caller/types.ts"));
        let text = std::fs::read_to_string(&types).unwrap();
        let mut declared = BTreeSet::new();
        for line in text.lines() {
            let Some(rest) = line.strip_prefix("export function ").or_else(|| line.strip_prefix("export const ")) else { continue };
            let name: String = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$').collect();
            assert!(declared.insert(name.clone()), "{flow}: `{name}` is declared twice in Argo's caller/types.ts:\n{text}");
        }
        assert!(declared.contains(&format!("is_{own}_")), "{flow}: the record `{own}` does not go by `{own}_` in Argo's caller/types.ts:\n{text}");
        if node {
            let ran = Command::new("node").arg(&types).output().unwrap();
            assert!(ran.status.success(), "{flow}: Node does not run Argo's caller/types.ts:\n{}", said(&ran));
        }
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

/// The variables are the flow's, and the code of every target keeps them apart (DESIGN 22): a
/// variable named as the code's own, and two that a target's code would write as one name, pass
/// `check` in both languages, and each target writes each by an identifier of its own. The one the
/// target takes as it is keeps it; the other takes `_` after it. tests/flows/variable_names.flow
/// runs such variables on every platform.
#[test]
fn the_code_keeps_the_variables_apart() {
    let scratch = TempDir::new("dandori-spellings-variables");
    let write = |name: &str, inputs: &str, lets: &str| {
        let path = scratch.path().join(format!("{name}.flow"));
        let text = format!("workflow names v1\n\ninputs\n  a : int\n{inputs}\noutputs\n  n : int\n\nflow\n{lets}  succeed n = a\n");
        std::fs::write(&path, text).unwrap();
        path.to_str().unwrap().to_string()
    };
    // each flow, with the identifiers of its two variables in TypeScript, in Python, in Go and in the state machine
    for (name, inputs, lets, ids) in [
        ("two_lets", "", "  let _x = 1\n  let v_x = 2\n", [["v_x_", "v_x"]; 4]),
        ("an_input", "  _x : int\n", "  let v_x = 2\n", [["v_x_", "v_x"]; 4]),
        ("python", "", "  let class = 1\n  let class_ = 2\n", [["_class", "class_"], ["class__", "class_"], ["class", "class_"], ["class", "class_"]]),
        ("go", "", "  let type = 1\n  let type_ = 2\n", [["type", "type_"], ["type__", "type_"], ["type__", "type_"], ["v_type", "type_"]]),
        ("its_own", "", "  let dd_error = 1\n  let dd_v = 2\n", [["dd_error", "dd_v"], ["dd_error_", "dd_v_"], ["dd_error_", "dd_v_"], ["dd_error_", "dd_v_"]]),
    ] {
        let flow = write(name, inputs, lets);
        for lang in ["en", "ja"] {
            let (code, out, err) = run(&["check", &flow, "--lang", lang]);
            assert!(code == 0 && !format!("{out}{err}").contains("E006"), "{name} {lang}: {out}{err}");
        }
        let m = checked(&flow).model.unwrap();
        let declared: [(&str, &str, fn(&str) -> String, usize); 6] = [
            ("temporal-typescript", "workflow.ts", |id| format!(" {id}: number"), 0),
            ("durable", "workflow.ts", |id| format!(" {id}: number"), 0),
            ("temporal-python", "workflow.py", |id| format!("        {id}: int"), 1),
            ("pydantic-graph", "graph.py", |id| format!("    {id}: int"), 1),
            ("temporal-go", "workflow.go", |id| format!("var {id} any"), 2),
            ("asl", ".asl.json", |id| format!("\"{id}\": "), 3),
        ];
        for (target, file, decl, at) in declared {
            let files = dandori::commands::build(&m, target).unwrap().unwrap_or_else(|d| panic!("{name} --target {target}: {d:?}"));
            let text = &files.iter().find(|(n, _)| n.ends_with(file)).unwrap_or_else(|| panic!("{name} --target {target}: no {file}")).1;
            for id in ids[at] {
                assert!(text.contains(&decl(id)), "{name} --target {target}: no variable `{id}` in {file}:\n{text}");
            }
        }
    }
    let flow = write("apart", "", "  let x = 1\n  let v_y = 2\n  let _z = 3\n  let class = 4\n  let type = 5\n  let count = 6\n  let v_count = 7\n  let String = 8\n");
    let (code, out, err) = run(&["check", &flow]);
    assert!(code == 0 && !format!("{out}{err}").contains("E006"), "{out}{err}");
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
        for (target, file, head, times) in [("temporal-typescript", "client.ts", "export type Sent = ", 1), ("temporal-python", "client.py", "\nSent = ", 1), ("temporal-go", "client.go", "type Sent ", 0)] {
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
        let c = checked(flow);
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
    let m = checked(flow).model.unwrap_or_else(|| panic!("{flow} passes check"));
    let files = dandori::commands::build(&m, target).unwrap().unwrap_or_else(|d| panic!("{flow} --target {target}: {d:?}"));
    for (name, text) in files {
        let p = dir.join(name);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
}

/// Every flow whose code `target` writes: each form's renamed flow, the form itself when it does not
/// stop `target`, and the flows no name stops.
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
    out.extend(FREE.iter().map(|f| f.to_string()));
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
    if let Some(refused) = tsc_in(dir, &["*.ts", "f*/**/*.ts"]) {
        panic!("tsc --strict refuses the TypeScript of what is not stopped:\n{refused}");
    }
    eprintln!("tsc --strict: compared {} flows", buildable("temporal-typescript").len());
}

/// `mypy --strict` in the directory of each package at once: its errors, each after the package's name.
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
        errors.extend(text.lines().filter(|l| l.contains(": error:")).map(|l| format!("{package}: {l}")));
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
        let m = checked(flow).model.unwrap_or_else(|| panic!("{flow} passes check"));
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
            assert!(errors.iter().any(|e| e.starts_with(&format!("{}: ", form.en)) && (e.contains("already defined") || e.contains("[no-redef]") || e.contains("[misc]") || e.contains("[assignment]"))), "{}: mypy passes the code past the stop:\n{}", form.en, errors.join("\n"));
        }
        eprintln!("mypy --strict: refused the code past the stop");
    }
}

/// The file `name` of the code `flow` writes for `target`.
fn generated(flow: &str, target: &str, name: &str) -> String {
    let m = checked(flow).model.unwrap_or_else(|| panic!("{flow} passes check"));
    let files = dandori::commands::build(&m, target).unwrap().unwrap_or_else(|d| panic!("{flow} --target {target}: {d:?}"));
    files.into_iter().find(|(n, _)| n.ends_with(&format!("/{name}"))).unwrap_or_else(|| panic!("{flow} --target {target}: no {name}")).1
}

/// The flows no name stops are said nothing, and every target that can write them does.
#[test]
fn what_no_code_keeps_is_said_nothing() {
    let scratch = TempDir::new("dandori-spellings-free");
    for flow in FREE {
        let (code, o, e) = run(&["check", flow]);
        assert!(code == 0 && !format!("{o}{e}").contains("warning["), "{flow}: {o}{e}");
        for target in dandori::commands::TARGETS {
            let (_, said, _) = gen_for(flow, target, scratch.path());
            assert!(!said.contains("E051"), "{flow} --target {target}:\n{said}");
        }
    }
}

/// A type, a task and a variable named as what no code reads have the names you wrote in the code:
/// the nouns in Go, the builtins in Python, the globals in TypeScript.
#[test]
fn what_no_code_reads_keeps_its_name() {
    for flow in ["tests/flows/plain_names.flow", "tests/flows/plain_names.ja.flow"] {
        let go = generated(flow, "temporal-go", "types.go");
        for head in ["type Outcome string", "type Status string", "type Answer struct", "type History struct"] {
            assert!(go.contains(head), "{flow}: no `{head}`:\n{go}");
        }
        let py = generated(flow, "temporal-python", "types.py");
        for head in ["\nformat = TypedDict(", "\nhash = TypedDict(", "\nfilter = Literal[", "\nopen = Literal[", "\nprint = Literal[", "\nTypeError = Literal[", "\nTypeGuard = TypedDict("] {
            assert!(py.contains(head), "{flow}: no `{head}`:\n{py}");
        }
        let workflow = generated(flow, "temporal-python", "workflow.py");
        assert!(workflow.contains("\n        format = dd_r\n") && !workflow.contains("format_"), "{flow}: a variable named as a builtin the code does not read:\n{workflow}");
        for variable in ["filter", "map", "TypeError", "TypeGuard"] {
            assert!(workflow.contains(&format!("\n        {variable} = dd_r\n")), "{flow}: the variable `{variable}`:\n{workflow}");
        }
        for target in ["temporal-typescript", "durable"] {
            let ts = generated(flow, target, "types.ts");
            for head in ["export interface Date ", "export type Map = ", "export interface Number ", "export interface Object ", "export type Promise = "] {
                assert!(ts.contains(head), "{flow}: no `{head}` for {target}:\n{ts}");
            }
        }
    }
    for flow in ["tests/flows/plain_tasks.flow", "tests/flows/plain_tasks.ja.flow"] {
        for target in ["temporal-python", "pydantic-graph"] {
            let file = if target == "temporal-python" { "activities.py" } else { "tasks.py" };
            let py = generated(flow, target, file);
            for head in ["def print(", "def open(", "def format(", "def TypeError(", "def TypeGuard("] {
                assert!(py.contains(head), "{flow}: no `{head}` for {target}:\n{py}");
            }
        }
    }
}

/// In Go a method `Status` is a function of its own, over `RunStatus`, and reads the type `Where`,
/// since the function has the message's name; a method of another name has the alias
/// `Status`; a method `RunStatus` is the function there is; and `Answer` is a function over
/// `AnswerCallback`, which `go vet` passes (`buildable`).
#[test]
fn the_status_and_the_answer_of_a_service_in_go() {
    for flow in ["tests/spellings/client_status.flow", "tests/spellings/クライアントのstatus.flow"] {
        let c = generated(flow, "temporal-go", "client.go");
        assert!(c.contains("func Status(ctx context.Context, c client.Client, id string) (Where, error) {\n\treturn RunStatus(ctx, c, id)\n}"), "{flow}:\n{c}");
        assert!(!c.contains("type Status"), "{flow}:\n{c}");
    }
    for flow in ["tests/spellings/client_run_status.flow", "tests/spellings/クライアントのRunStatus.flow"] {
        let c = generated(flow, "temporal-go", "client.go");
        assert_eq!(c.matches("func RunStatus(").count(), 1, "{flow}:\n{c}");
    }
    for flow in ["tests/spellings/client_shared.flow", "tests/spellings/クライアントの共有.flow"] {
        let c = generated(flow, "temporal-go", "client.go");
        assert!(c.contains("\ntype Status = Where\n") && c.contains("func GetStatus(ctx context.Context, c client.Client, id string) (Status, error) {\n\treturn RunStatus(ctx, c, id)\n}"), "{flow}:\n{c}");
    }
    for flow in ["tests/spellings/client_status_beside.flow", "tests/spellings/クライアントのStatusと並ぶレコード.flow"] {
        let c = generated(flow, "temporal-go", "client.go");
        assert!(!c.contains("type Status") && c.contains("func GetStatus(ctx context.Context, c client.Client, id string) (Where, error) {"), "{flow}:\n{c}");
    }
    for flow in ["tests/spellings/client_answer.flow", "tests/spellings/クライアントのanswer.flow"] {
        let c = generated(flow, "temporal-go", "client.go");
        assert!(c.contains("func Answer(ctx context.Context, c client.Client, callbackID string, value Sent) error {\n\treturn AnswerCallback(ctx, c, callbackID, CallbackAnswer{OK: value})\n}"), "{flow}:\n{c}");
    }
}

/// A method that does what a function of the client does, and has its name, is that function, in
/// each language: the client writes no other, and nothing is said (`client_same`). A method of a
/// name of the client's that does something else is said, and written nowhere (`FORMS`): a caller
/// would read the client's `send` as the function that starts the workflow.
#[test]
fn a_method_that_does_what_a_function_does_is_that_function() {
    for flow in ["tests/spellings/client_same.flow", "tests/spellings/クライアントの同じ関数.flow"] {
        for (target, file, heads) in [
            ("temporal-typescript", "client.ts", ["export async function start(", "export async function status("]),
            ("temporal-python", "client.py", ["async def start(", "async def status("]),
        ] {
            let c = generated(flow, target, file);
            for head in heads {
                assert_eq!(c.matches(head).count(), 1, "{flow} --target {target}: `{head}`:\n{c}");
            }
        }
        // in Go the function of `rpc Status` is `Status`, over `RunStatus` (`the_status_and_the_answer_of_a_service_in_go`)
        let c = generated(flow, "temporal-go", "client.go");
        for head in ["func Start(", "func RunStatus(", "func Status("] {
            assert_eq!(c.matches(head).count(), 1, "{flow}: `{head}`:\n{c}");
        }
    }
}

/// Every `.flow` of the tree.
fn every_flow(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            if e.file_name() != "node_modules" && e.file_name() != ".venv" {
                every_flow(&p, out);
            }
        } else if p.extension().is_some_and(|x| x == "flow") {
            out.push(p);
        }
    }
}

/// Each flow of the tree that passes `check`, read as `ritsu dandori` reads it, with the code of
/// each of `targets` it can write: the flows of a project's module, whose rules are read by the
/// project, and a fixture of a check, have none.
fn code_of_every_flow(targets: &[&'static str]) -> Vec<(String, &'static str, Vec<(String, String)>)> {
    let mut flows = Vec::new();
    every_flow(&root().join(".."), &mut flows);
    flows.sort();
    let mut out = Vec::new();
    for f in flows {
        let rules = rulec::ports::Engine::with_dates(std::sync::Arc::new(koyomi::ports::Engine));
        let checked = dandori::sources::with_ports(Rc::new(rules), Rc::new(koyomi::ports::Engine), Rc::new(chobo::ports::Engine), || dandori::check::built(&f));
        let Ok((_, c)) = checked else { continue };
        let Some(m) = c.model else { continue };
        for target in targets {
            if let Some(Ok(files)) = dandori::commands::build(&m, target) {
                out.push((f.display().to_string(), *target, files));
            }
        }
    }
    out
}

/// What the Python of `dir` reads of `builtins` where the file does not bind the name itself, and
/// every name of `builtins` it reads or binds.
const PY_READS: &str = r#"
import ast, builtins, json, os, sys
names = {n for n in dir(builtins) if not n.startswith("__")}
read, seen = set(), set()
for r, _, fs in os.walk(sys.argv[1]):
    for x in fs:
        if not x.endswith(".py"):
            continue
        loads, binds = set(), set()
        for n in ast.walk(ast.parse(open(os.path.join(r, x), encoding="utf-8").read())):
            if isinstance(n, ast.Name):
                (loads if isinstance(n.ctx, ast.Load) else binds).add(n.id)
            elif isinstance(n, ast.arg):
                binds.add(n.arg)
            elif isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef)):
                binds.add(n.name)
            elif isinstance(n, ast.ExceptHandler) and n.name:
                binds.add(n.name)
            elif isinstance(n, ast.alias):
                binds.add((n.asname or n.name).split(".")[0])
        read |= (loads - binds) & names
        seen |= (loads | binds) & names
print(json.dumps({"names": sorted(names), "read": sorted(read), "seen": sorted(seen)}))
"#;

/// `PY_MORE` keeps every builtin the Python of a flow of the tree reads, and no other: a builtin
/// the code starts to read has to come into the table (a variable of the name would hide it),
/// and one it stops reading has to leave, so that no variable takes another name for it. A name
/// of the flow that is a builtin, which the file binds, is the flow's (`print` in
/// tests/flows/plain_tasks.flow).
#[test]
fn the_table_of_python_is_the_builtins_the_code_reads() {
    if !need(Need::Python) {
        return;
    }
    let scratch = TempDir::new("dandori-spellings-builtins");
    let code = code_of_every_flow(&["temporal-python", "pydantic-graph"]);
    let mut written = BTreeSet::new();
    for (i, (flow, target, files)) in code.iter().enumerate() {
        written.insert(flow.clone());
        for (name, text) in files {
            let p = scratch.path().join(format!("{target}-{i}")).join(name);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        }
    }
    let out = Command::new("python3").args(["-I", "-c", PY_READS]).arg(scratch.path()).output().unwrap();
    assert!(out.status.success(), "{}", said(&out));
    let got: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let names = |k: &str| -> BTreeSet<String> { got[k].as_array().unwrap().iter().map(|n| n.as_str().unwrap().to_string()).collect() };
    use ritsu_emit::copies::dandori::PY_RESERVED;
    use ritsu_emit::words::python::NAMES;
    let short: Vec<String> = names("read").into_iter().filter(|n| !PY_RESERVED.contains(n)).collect();
    assert!(short.is_empty(), "the Python reads {short:?}, which are not in `PY_MORE` (ritsu-emit's copies.rs): a variable of the flow would hide them");
    let (seen, builtins) = (names("seen"), names("names"));
    let idle: Vec<&str> = PY_RESERVED.iter().filter(|n| !NAMES.contains(n) && builtins.contains(*n) && !seen.contains(*n)).collect();
    assert!(idle.is_empty(), "no code of a flow reads {idle:?}: take them out of `PY_MORE` (ritsu-emit's copies.rs), so that they stop nobody");
    assert!(written.len() > 200, "the code of only {} flows", written.len());
    eprintln!("the builtins of Python: compared the code of {} flows ({} packages)", written.len(), code.len());
}

/// The identifiers of TypeScript that read a type, in `text`: comments and strings taken out, and
/// the values among the names (`Number.isInteger`, what follows a dot) left out.
fn ts_type_reads(text: &str) -> BTreeSet<String> {
    let cs: Vec<char> = text.chars().collect();
    let mut out = BTreeSet::new();
    let mut i = 0;
    let mut before = ' ';
    while i < cs.len() {
        let c = cs[i];
        if c == '/' && cs.get(i + 1) == Some(&'/') {
            while i < cs.len() && cs[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && cs.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < cs.len() && !(cs[i] == '*' && cs[i + 1] == '/') {
                i += 1;
            }
            i += 2;
        } else if c == '"' || c == '\'' || c == '`' {
            i += 1;
            while i < cs.len() && cs[i] != c {
                i += if cs[i] == '\\' { 2 } else { 1 };
            }
            i += 1;
            before = c;
        } else if c.is_alphabetic() || c == '_' || c == '$' {
            let start = i;
            while i < cs.len() && (cs[i].is_alphanumeric() || cs[i] == '_' || cs[i] == '$') {
                i += 1;
            }
            let word: String = cs[start..i].iter().collect();
            let next = cs[i..].iter().find(|x| !x.is_whitespace()).copied().unwrap_or(' ');
            if before != '.' && next != '.' {
                out.insert(word);
            }
            before = 'a';
        } else {
            if !c.is_whitespace() {
                before = c;
            }
            i += 1;
        }
    }
    out
}

/// The global types of JavaScript and of TypeScript's own library: what a type of the name would
/// hide, if the file reads it.
const LIB_TYPES: &[&str] = &[
    "Array", "ArrayBuffer", "AsyncIterable", "AsyncIterator", "Awaited", "BigInt", "Boolean", "DataView", "Date", "Error", "Exclude", "Extract", "Float32Array", "Float64Array", "Function", "Int16Array", "Int32Array", "Int8Array", "Iterable", "Iterator", "JSON", "Map", "Math", "NonNullable", "Number", "Object", "Omit", "Parameters", "Partial", "Pick", "Promise",
    "PromiseLike", "Readonly", "ReadonlyArray", "ReadonlyMap", "ReadonlySet", "Record", "RegExp", "Required", "ReturnType", "Set", "String", "Symbol", "TextDecoder", "TextEncoder", "Uint16Array", "Uint32Array", "Uint8Array", "URL", "WeakMap", "WeakSet",
];

/// `TYPES_READ` keeps every global type `types.ts` reads and no other, and `CLIENT_GLOBALS_TS` every
/// one `client.ts` reads, for every flow of the tree: a type of the flow of one of those names has
/// the file read the global through `globalThis`. A type hides a type, no value, and the other files
/// read a flow's type as `T.Date`.
#[test]
fn the_table_of_typescript_is_the_global_types_the_code_reads() {
    use dandori::temporal::{CLIENT_GLOBALS_TS, TYPES_READ};
    let mut in_types: BTreeSet<String> = BTreeSet::new();
    let mut in_clients: BTreeSet<String> = BTreeSet::new();
    let code = code_of_every_flow(&["temporal-typescript", "durable"]);
    for (_, _, files) in &code {
        for (name, text) in files {
            let declared = |word: &str| [format!("export type {word} "), format!("export interface {word} ")].iter().any(|h| text.contains(h.as_str()));
            let reads = || ts_type_reads(text).into_iter().filter(|w| LIB_TYPES.contains(&w.as_str()) && !declared(w));
            if name.ends_with("/types.ts") {
                in_types.extend(reads());
            } else if name.ends_with("/client.ts") {
                in_clients.extend(reads());
            }
        }
    }
    let read: BTreeSet<String> = TYPES_READ.iter().map(|n| n.to_string()).collect();
    assert_eq!(in_types, read, "the global types `types.ts` reads are not `TYPES_READ` (src/temporal.rs)");
    let read: BTreeSet<String> = CLIENT_GLOBALS_TS.iter().map(|n| n.to_string()).collect();
    assert_eq!(in_clients, read, "the global types `client.ts` reads are not `CLIENT_GLOBALS_TS` (src/temporal.rs)");
    assert!(code.len() > 300, "the code of only {} packages", code.len());
    eprintln!("the globals of TypeScript: compared {} packages, {} of types and {} of clients", code.len(), in_types.len(), in_clients.len());
}

/// The names of JavaScript in `text`, each not after a dot: outside comments and strings, but inside
/// the `${…}` of a template.
fn js_names(text: &str, out: &mut BTreeSet<String>) {
    let cs: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut before = ' ';
    while i < cs.len() {
        let c = cs[i];
        if c == '/' && cs.get(i + 1) == Some(&'/') {
            while i < cs.len() && cs[i] != '\n' {
                i += 1;
            }
        } else if c == '/' && cs.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < cs.len() && !(cs[i] == '*' && cs[i + 1] == '/') {
                i += 1;
            }
            i += 2;
        } else if c == '"' || c == '\'' {
            i += 1;
            while i < cs.len() && cs[i] != c {
                i += if cs[i] == '\\' { 2 } else { 1 };
            }
            i += 1;
            before = c;
        } else if c == '`' {
            i += 1;
            while i < cs.len() && cs[i] != '`' {
                if cs[i] == '\\' {
                    i += 2;
                } else if cs[i] == '$' && cs.get(i + 1) == Some(&'{') {
                    let (from, mut depth) = (i + 2, 1);
                    i += 2;
                    while i < cs.len() && depth > 0 {
                        depth += match cs[i] {
                            '{' => 1,
                            '}' => -1,
                            _ => 0,
                        };
                        i += 1;
                    }
                    js_names(&cs[from..i - 1].iter().collect::<String>(), out);
                } else {
                    i += 1;
                }
            }
            i += 1;
            before = c;
        } else if c == '.' && cs.get(i + 1) == Some(&'.') && cs.get(i + 2) == Some(&'.') {
            // a spread reads the name after it
            i += 3;
            before = ' ';
        } else if c.is_alphabetic() || c == '_' || c == '$' {
            let start = i;
            while i < cs.len() && (cs[i].is_alphanumeric() || cs[i] == '_' || cs[i] == '$') {
                i += 1;
            }
            if before != '.' {
                out.insert(cs[start..i].iter().collect());
            }
            before = 'a';
        } else {
            if !c.is_whitespace() {
                before = c;
            }
            i += 1;
        }
    }
}

/// The global values of JavaScript, and of the hosts the code runs on: what a variable of the name
/// would hide, if `run` reads it.
const JS_VALUES: &[&str] = &[
    "AbortController", "Array", "ArrayBuffer", "Atomics", "BigInt", "Boolean", "DataView", "Date", "Error", "EvalError", "Float32Array", "Float64Array", "Function", "Infinity", "Int16Array", "Int32Array", "Int8Array", "Intl", "JSON", "Map", "Math", "NaN", "Number", "Object", "Promise", "Proxy", "RangeError", "ReferenceError", "Reflect", "RegExp", "Set",
    "String", "Symbol", "SyntaxError", "TextDecoder", "TextEncoder", "TypeError", "URIError", "URL", "Uint16Array", "Uint32Array", "Uint8Array", "WeakMap", "WeakSet", "atob", "btoa", "clearTimeout", "console", "crypto", "decodeURI", "decodeURIComponent", "encodeURI", "encodeURIComponent", "eval", "fetch", "globalThis", "isFinite", "isNaN", "parseFloat",
    "parseInt", "process", "queueMicrotask", "setTimeout", "structuredClone", "undefined",
];

/// The values workflow.ts has at its top: what it imports (not as a type) and what it declares.
fn top_values(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let word = |s: &str| s.trim().split(|c: char| !(c.is_alphanumeric() || c == '_' || c == '$')).next().unwrap_or("").to_string();
    for line in text.lines() {
        let line = line.strip_prefix("export ").unwrap_or(line);
        if line.starts_with("import type ") {
            continue;
        } else if let Some(rest) = line.strip_prefix("import * as ") {
            out.insert(word(rest));
        } else if let Some(rest) = line.strip_prefix("import {") {
            let names = rest.split('}').next().unwrap_or("");
            for n in names.split(',').map(str::trim).filter(|n| !n.is_empty() && !n.starts_with("type ")) {
                out.insert(word(n.rsplit(" as ").next().unwrap_or(n)));
            }
        } else {
            for head in ["const ", "let ", "function ", "async function ", "class "] {
                if let Some(rest) = line.strip_prefix(head) {
                    out.insert(word(rest));
                }
            }
        }
    }
    out.remove("");
    out
}

/// `run`, the function of workflow.ts that runs the flow: its parameters, and what its body reads
/// but does not declare (the flow's variables, its own `dd_`).
fn run_of(text: &str) -> (BTreeSet<String>, BTreeSet<String>) {
    let start = text.find("async function run(").expect("workflow.ts has run");
    let end = text[start..].find("\n}\n").map_or(text.len(), |e| start + e);
    let open = start + "async function run(".len();
    let close = open + text[open..].find(')').unwrap();
    let params = text[open..close].split(',').map(|p| p.split(':').next().unwrap().trim().to_string()).filter(|p| !p.is_empty()).collect();
    let mut reads = BTreeSet::new();
    js_names(&text[open..end], &mut reads);
    let body = &text[close..end];
    for head in ["let ", "const "] {
        for (i, _) in body.match_indices(head) {
            let name: String = body[i + head.len()..].chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '$').collect();
            reads.remove(&name);
        }
    }
    (params, reads)
}

/// `TS_RESERVED` keeps, but for the words of JavaScript, every name the `run` of a flow of the tree
/// reads that a variable could hide — a global of JavaScript, a parameter of `run`, a value the file
/// imports or declares — and no other; what else it reads of them is a task's activity or the
/// workflow's function, which `ts_var` knows by their shape. So no variable hides what `run` reads,
/// and a name `run` stops reading leaves the table, so that no variable is renamed for it.
#[test]
fn the_table_of_typescript_is_what_run_reads() {
    use dandori::temporal::ts_var;
    use ritsu_emit::copies::dandori::TS_RESERVED;
    use ritsu_emit::words::typescript::NAMES;
    let code = code_of_every_flow(&["temporal-typescript", "durable"]);
    let (mut read, mut shapes, mut runs) = (BTreeSet::new(), BTreeSet::new(), 0);
    for (flow, target, files) in &code {
        let Some((_, text)) = files.iter().find(|(n, _)| n.ends_with("/workflow.ts")) else { continue };
        runs += 1;
        let (params, reads) = run_of(text);
        let mut values: BTreeSet<String> = JS_VALUES.iter().map(|w| w.to_string()).collect();
        values.extend(top_values(text));
        values.extend(params);
        for w in reads.intersection(&values).filter(|w| !w.starts_with("dd_")) {
            assert_ne!(&ts_var(w), w, "{flow} --target {target}: `run` reads `{w}`, and a variable of the name would hide it");
            if TS_RESERVED.contains(w) {
                if !NAMES.contains(w) {
                    read.insert(w.clone());
                }
            } else {
                shapes.insert(if w.starts_with("task_") { "task_<name>" } else { "<name>_v<version>" });
            }
        }
    }
    let table: BTreeSet<String> = TS_RESERVED.iter().filter(|w| !NAMES.contains(w)).map(String::from).collect();
    assert_eq!(read, table, "what `run` reads is not `TS_RESERVED` (ritsu-emit's copies.rs), but for the words of JavaScript");
    assert_eq!(shapes.len(), 2, "`run` reads {shapes:?} by their shape");
    assert!(runs > 500, "only {runs} workflow.ts");
    eprintln!("the variables of TypeScript: compared {runs} workflow.ts, {} names of the table", read.len());
}

/// `tsc --strict` over `include` under `dir`, with the SDKs of tools/temporal and stand-ins for those
/// of the agents' calls, which it does not have: what it says, when it refuses, with the paths
/// under `dir`.
fn tsc_in(dir: &Path, include: &[&str]) -> Option<String> {
    std::fs::write(dir.join("shims.d.ts"), "declare module \"@anthropic-ai/sdk\";\ndeclare module \"openai\";\ndeclare module \"@openai/agents\";\ndeclare module \"@aws-sdk/*\";\n").unwrap();
    let _ = std::os::unix::fs::symlink(root().join("tools/temporal/node_modules"), dir.join("node_modules"));
    let tsconfig = serde_json::json!({
        "compilerOptions": { "strict": true, "noEmit": true, "module": "nodenext", "moduleResolution": "nodenext", "target": "es2022", "skipLibCheck": true, "types": ["node"], "typeRoots": [root().join("tools/temporal/node_modules/@types")] },
        "include": include
    });
    std::fs::write(dir.join("tsconfig.json"), serde_json::to_string_pretty(&tsconfig).unwrap()).unwrap();
    let out = Command::new(root().join("tools/temporal/node_modules/.bin/tsc")).current_dir(dir).arg("-p").arg("tsconfig.json").output().unwrap();
    (!out.status.success()).then(|| said(&out))
}

/// A variable named as a word of JavaScript or as what `run` reads (`TS_RESERVED`), as a task's
/// activity (`task_note`) or as the workflow's function (`names_v1`), and an input named so, go by
/// `_` and the name in the TypeScript of both targets, which tsc passes; an input keeps its key.
#[test]
fn a_variable_of_typescript_hides_nothing() {
    use ritsu_emit::copies::dandori::TS_RESERVED;
    let tsc = root().join("tools/temporal/node_modules/.bin/tsc");
    if !ready(Need::Node, || tsc.exists() && root().join("tools/durable/node_modules").exists(), "TypeScript is not in tools/temporal/node_modules, or the durable SDK in tools/durable/node_modules") {
        return;
    }
    let scratch = TempDir::new("dandori-spellings-ts-variables");
    let inputs = ["input", "class", "String", "context"];
    let mut lets: Vec<String> = TS_RESERVED.iter().filter(|w| !inputs.contains(w)).map(String::from).collect();
    lets.extend(["task_note".to_string(), "names_v1".to_string()]);
    lets.sort();
    lets.dedup();
    let text = format!(
        "workflow names v1\n\ninputs\n{}\noutputs\n  text : string\n\ntask note(id: string) -> string\n  lambda \"arn:aws:lambda:eu-west-2:123456789012:function:note\"\n\nflow\n{}  succeed text = \"{}\"\n",
        inputs.iter().map(|i| format!("  {i} : string\n")).collect::<String>(),
        lets.iter().map(|l| format!("  let {l} = note(id: input)\n")).collect::<String>(),
        inputs.iter().map(|i| i.to_string()).chain(lets.iter().cloned()).map(|n| format!("{{{n}}}")).collect::<String>(),
    );
    let flow = scratch.path().join("names.flow");
    std::fs::write(&flow, text).unwrap();
    let flow = flow.to_str().unwrap();
    let (code, o, e) = run(&["check", flow]);
    assert_eq!(code, 0, "{o}{e}");
    for target in ["temporal-typescript", "durable"] {
        let dir = scratch.path().join(target);
        write_files(flow, target, &dir);
        let workflow = std::fs::read_to_string(dir.join("names/workflow.ts")).unwrap();
        for l in &lets {
            assert!(workflow.contains(&format!("  let _{l}: string | null = null;\n")), "{target}: no `_{l}`:\n{workflow}");
        }
        for i in inputs {
            assert!(workflow.contains(&format!("  const _{i}: string = input[\"{i}\"];\n")), "{target}: no `_{i}`:\n{workflow}");
        }
        if target == "durable" {
            let _ = std::os::unix::fs::symlink(root().join("tools/durable/node_modules"), dir.join("names/node_modules"));
        }
        if let Some(refused) = tsc_in(&dir, &["*.ts", "names/**/*.ts"]) {
            panic!("{target}: tsc --strict refuses the variables:\n{refused}");
        }
    }
}

/// The words of TypeScript a name of `types.ts` could be: those of strict-mode JavaScript, the types
/// of TypeScript's own, and the words it reads in a type or a declaration.
const TS_WORDS: &[&str] = &[
    "abstract", "accessor", "any", "as", "asserts", "assert", "async", "bigint", "boolean", "constructor", "declare", "from", "get", "global", "infer", "intrinsic", "is", "keyof", "module", "namespace", "never", "number", "object", "of", "out", "override", "readonly", "require", "satisfies", "set", "string", "symbol", "type", "unique", "unknown", "using",
];

/// What tsc refuses as the name of a type of `types.ts` is what `check` stops for TypeScript (W105),
/// and no other: each word, as a record (an interface, with its check `is_<name>`) and as an enum
/// (a type alias, with its `<name>_values`), each read by an interface, in a file of its own.
#[test]
fn the_words_a_type_of_typescript_cannot_be_are_what_tsc_refuses() {
    use dandori::temporal::ts_type_word;
    use ritsu_emit::words::typescript::NAMES;
    let tsc = root().join("tools/temporal/node_modules/.bin/tsc");
    if !ready(Need::Node, || tsc.exists(), "TypeScript is not in tools/temporal/node_modules; run `npm install --prefix tools/temporal`") {
        return;
    }
    let scratch = TempDir::new("dandori-spellings-ts-words");
    let words: BTreeSet<&str> = NAMES.iter().chain(TS_WORDS.iter().copied()).collect();
    let words: Vec<&str> = words.into_iter().collect();
    for (i, w) in words.iter().enumerate() {
        std::fs::write(scratch.path().join(format!("record{i}.ts")), format!("export interface {w} {{\n  \"day\": string;\n}}\n\nexport function is_{w}(v: any): v is {w} {{\n  return typeof v === \"object\" && v !== null && typeof v[\"day\"] === \"string\";\n}}\n\nexport interface WorkflowInput {{\n  \"x\": {w};\n}}\n")).unwrap();
        std::fs::write(scratch.path().join(format!("enum{i}.ts")), format!("export const {w}_values: readonly unknown[] = [\"a\", \"b\"];\nexport type {w} = \"a\" | \"b\";\n\nexport interface WorkflowInput {{\n  \"x\": {w};\n}}\n")).unwrap();
    }
    // tsc checks the types once no file has an error of its syntax: what it refuses at once, and
    // then, without those files, what it refuses of the rest, until it refuses nothing
    let mut said = String::new();
    let mut files: Vec<String> = (0..words.len()).flat_map(|i| [format!("record{i}.ts"), format!("enum{i}.ts")]).collect();
    while let Some(refused) = tsc_in(scratch.path(), &files.iter().map(String::as_str).collect::<Vec<_>>()) {
        let before = files.len();
        files.retain(|f| !refused.lines().any(|l| l.starts_with(&format!("{f}("))));
        assert!(files.len() < before, "tsc refuses what is not a type's name:\n{refused}");
        said.push_str(&refused);
    }
    for (i, w) in words.iter().enumerate() {
        for (file, alias) in [(format!("record{i}.ts("), false), (format!("enum{i}.ts("), true)] {
            let refused = said.lines().any(|l| l.starts_with(&file));
            assert_eq!(refused, ts_type_word(w, alias), "`{w}` as {}: tsc {}refuses it:\n{}", if alias { "an enum" } else { "a record" }, if refused { "" } else { "never " }, said.lines().filter(|l| l.starts_with(&file)).collect::<Vec<_>>().join("\n"));
        }
    }
    eprintln!("the words of TypeScript: compared {} words", words.len());
}

/// The functions a JSONata expression calls: `$name(`, outside its strings and its names in backquotes.
fn jsonata_calls(text: &str, out: &mut BTreeSet<String>) {
    let cs: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c == '"' || c == '\'' || c == '`' {
            i += 1;
            while i < cs.len() && cs[i] != c {
                i += if cs[i] == '\\' { 2 } else { 1 };
            }
            i += 1;
        } else if c == '$' {
            let start = i + 1;
            i = start;
            while i < cs.len() && (cs[i].is_alphanumeric() || cs[i] == '_') {
                i += 1;
            }
            if i > start && cs.get(i) == Some(&'(') {
                out.insert(cs[start..i].iter().collect());
            }
        } else {
            i += 1;
        }
    }
}

/// What a state machine calls, in every expression (`{% … %}`), and the variables it binds (`Assign`).
fn read_machine(v: &serde_json::Value, calls: &mut BTreeSet<String>, bound: &mut BTreeSet<String>) {
    match v {
        serde_json::Value::Object(o) => {
            for (k, x) in o {
                if let (true, Some(a)) = (k == "Assign", x.as_object()) {
                    bound.extend(a.keys().cloned());
                }
                read_machine(x, calls, bound);
            }
        }
        serde_json::Value::Array(a) => a.iter().for_each(|x| read_machine(x, calls, bound)),
        serde_json::Value::String(s) if s.starts_with("{%") => jsonata_calls(s, calls),
        _ => {}
    }
}

/// `JSONATA_FUNCTIONS` is every function the state machine of a flow of the tree calls, and no other
/// (`$dd_fill` is the machine's own), and no machine has a variable of one of the names, which would
/// hide the function: `render::asl_var` is the way every variable goes.
#[test]
fn the_table_of_jsonata_is_the_functions_the_machines_call() {
    use ritsu_emit::copies::dandori::JSONATA_FUNCTIONS;
    let code = code_of_every_flow(&["asl"]);
    let (mut calls, mut bound) = (BTreeSet::new(), BTreeSet::new());
    for (_, _, files) in &code {
        for (name, text) in files {
            if name.ends_with(".asl.json") {
                read_machine(&serde_json::from_str(text).unwrap(), &mut calls, &mut bound);
            }
        }
    }
    calls.retain(|c: &String| !c.starts_with("dd_"));
    let table: BTreeSet<String> = JSONATA_FUNCTIONS.iter().map(|n| n.to_string()).collect();
    assert_eq!(calls, table, "the functions the state machines call are not `JSONATA_FUNCTIONS` (ritsu-emit's copies.rs)");
    let hiding: Vec<&String> = bound.intersection(&table).collect();
    assert!(hiding.is_empty(), "a state machine has the variables {hiding:?}, which hide the functions");
    assert!(code.len() > 80, "the machines of only {} flows", code.len());
    eprintln!("the functions of JSONata: compared the machines of {} flows, {} functions", code.len(), table.len());
}

/// In the state machine a variable named as a function is `v_<name>`, and one the flow names `v_count`
/// beside `count` is a variable of its own. The input and the outputs keep their keys in JSON, which
/// is what a caller reads; any other variable keeps its name. tests/examples.rs runs these machines.
#[test]
fn a_variable_of_the_machine_hides_no_function() {
    let machine = |flow: &str| -> serde_json::Value {
        let m = checked(flow).model.unwrap_or_else(|| panic!("{flow} passes check"));
        let files = dandori::commands::build(&m, "asl").unwrap().unwrap_or_else(|d| panic!("{flow}: {d:?}"));
        serde_json::from_str(&files.iter().find(|(n, _)| n.ends_with(".asl.json")).unwrap().1).unwrap()
    };
    for flow in ["tests/flows/jsonata_names.flow", "tests/flows/jsonata_names.ja.flow"] {
        let v = machine(flow);
        let start = &v["States"][v["StartAt"].as_str().unwrap()]["Assign"];
        assert_eq!(start["v_type"], "{% $exists($states.input.type) ? $states.input.type : null %}", "{flow}: {start}");
        for name in ["v_count", "v_v_count", "v_states", "v_v_states", "v_filter", "v_map", "v_string"] {
            assert!(start.get(name).is_some(), "{flow}: no variable `{name}`: {start}");
        }
        let succeed = v["States"].as_object().unwrap().values().find(|s| s["Type"] == "Succeed").unwrap();
        for (key, read) in [("type", "$v_type"), ("count", "$v_count"), ("v_count", "$v_v_count"), ("states", "$v_states"), ("v_states", "$v_v_states")] {
            assert_eq!(succeed["Output"][key], format!("{{% {read} %}}"), "{flow}: {succeed}");
        }
    }
    for flow in ["tests/flows/plain_names.flow", "tests/flows/plain_names.ja.flow"] {
        let v = machine(flow);
        let start = &v["States"][v["StartAt"].as_str().unwrap()]["Assign"];
        for name in ["v_filter", "v_map", "bytes", "format", "object"] {
            assert!(start.get(name).is_some(), "{flow}: no variable `{name}`: {start}");
        }
    }
}

/// Every flow of the directory is a form above.
#[test]
fn the_form_list_is_every_flow_of_the_directory() {
    let mut names: Vec<String> = FORMS.iter().flat_map(|f| [f.en.to_string(), f.ja.to_string()]).collect();
    names.extend(["go_books", "Goの帳簿", "rule_type_name", "規則の型の名前"].map(String::from));
    names.extend(FREE.iter().filter_map(|f| f.strip_prefix("tests/spellings/")).map(|f| f.trim_end_matches(".flow").to_string()));
    names.sort();
    let mut files: Vec<String> = std::fs::read_dir(root().join("tests/spellings")).unwrap().flatten().filter_map(|e| e.file_name().to_str().and_then(|n| n.strip_suffix(".flow")).map(String::from)).collect();
    files.sort();
    assert_eq!(files, names);
}
