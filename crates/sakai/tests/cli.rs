//! The command line (PLAN B.11, DESIGN 6): the table, `--help`, exit codes, and what is refused
//! with exit 2 rather than ignored.

mod common;

use common::sakai;
use std::process::{Command, Output};

fn code(o: &Output) -> i32 {
    o.status.code().unwrap()
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).to_string()
}

fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).to_string()
}

const MAP: &str = "tests/maps/基本/基本.ctx";
const ROOT: &str = "tests/maps/基本";

#[test]
fn no_arguments_help_and_version() {
    let o = sakai(&[]);
    assert_eq!(code(&o), 2);
    assert!(err(&o).contains("sakai check"), "{}", err(&o));
    let o = sakai(&["--help"]);
    assert_eq!(code(&o), 0);
    for c in ["check", "gen", "export", "api", "explain"] {
        assert!(out(&o).contains(&format!("sakai {c} ")), "{c} is in --help");
    }
    let o = sakai(&["--version"]);
    assert_eq!(out(&o).trim(), format!("sakai {}", env!("CARGO_PKG_VERSION")));
    assert_eq!(code(&sakai(&["-V"])), 0);
}

#[test]
fn every_command_has_its_help() {
    for c in ["check", "gen", "export", "api", "explain"] {
        let a = sakai(&[c, "--help"]);
        let b = sakai(&["help", c]);
        let h = sakai(&[c, "-h"]);
        assert_eq!(code(&a), 0);
        assert_eq!(out(&a), out(&b));
        assert_eq!(out(&a), out(&h));
        assert!(out(&a).contains("Usage:") && out(&a).contains("Exit codes:") && out(&a).contains("Examples:"), "{c}");
        let ja = sakai(&[c, "--help", "--lang", "ja"]);
        assert!(out(&ja).contains("使い方:"), "{c}");
    }
    let check = out(&sakai(&["check", "--help"]));
    assert!(check.contains("E401") && check.contains("W103"), "{check}");
    assert!(!check.contains("E501"), "gen's codes are not check's");
    let help = out(&sakai(&["gen", "--help"]));
    assert!(help.contains("E501 E502") && help.contains("--target <tool>..."), "{help}");
}

#[test]
fn what_is_refused_with_exit_2() {
    for args in [
        vec!["check", MAP, "--frmat", "json"],
        vec!["check", MAP, "--format", "yaml"],
        vec!["check", MAP, "--format"],
        vec!["check", MAP, "--format", "json", "--format", "json"],
        vec!["check", MAP, "--root"],
        vec!["check", MAP, "--root", "tests/maps/none"],
        vec!["check", MAP, "--root", "tests/maps/パターン"],
        vec!["check", MAP, "--lang", "fr"],
        vec!["check", "tests/maps/none.ctx"],
        vec!["check", "tests/maps/基本/ctx/受注.ctx"],
        vec!["check", "src"],
        vec!["check"],
        vec!["nonsense"],
        vec!["help", "nonsense"],
        vec!["explain", "E999"],
        vec!["explain"],
        vec!["explain", "E401", "E402"],
        vec!["explain", "--all", "E401"],
        vec!["explain", "--all=yes"],
        vec!["explain", "E401", "--format", "html"],
        vec!["api"],
        vec!["api", MAP, MAP],
        vec!["api", ROOT],
        vec!["api", MAP, "--format", "json"],
        vec!["gen", MAP],
        vec!["gen", MAP, "--target", "depguard"],
        vec!["gen", MAP, MAP, "--target", "import-linter"],
        vec!["gen", ROOT, "--target", "import-linter"],
        vec!["gen", MAP, "--target", "import-linter", "--target", "import-linter"],
        vec!["build", MAP, "--target", "import-linter"],
        vec!["help", "build"],
        vec!["export", MAP],
        vec!["export", "plantuml", MAP],
        vec!["export", "cml"],
    ] {
        let o = sakai(&args);
        assert_eq!(code(&o), 2, "{args:?}: {}{}", out(&o), err(&o));
        assert!(!err(&o).is_empty(), "{args:?} says why");
    }
}

#[test]
fn check_exit_codes_and_formats() {
    // The map is written as it was given, from where sakai runs, whatever the root.
    let o = sakai(&["check", MAP, "--root", ROOT]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert_eq!(out(&o), "tests/maps/基本/基本.ctx: ok — 3 contexts, 3 relationships; 9 artifacts, each in one context; 3 crossings checked (proto 3)\n");
    // Without --root the root is the nearest directory above with a .git: here a copy of the map
    // two levels below one. The text writes the map as it was given; `api` writes it from the
    // root, which shows the root that was taken.
    let dir = common::TempDir::new("root");
    std::fs::create_dir_all(dir.path().join(".git")).unwrap();
    let here = dir.path().join("a/b");
    common::copy_dir(std::path::Path::new(ROOT), &here.join("基本"));
    let o = common::sakai_in(&here, &["check", "基本/基本.ctx"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert_eq!(out(&o), "基本/基本.ctx: ok — 3 contexts, 3 relationships; 9 artifacts, each in one context; 3 crossings checked (proto 3)\n");
    let o = common::sakai_in(&here, &["api", "基本/基本.ctx"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    let v: serde_json::Value = serde_json::from_str(&out(&o)).unwrap();
    assert_eq!(v["map"]["file"], "a/b/基本/基本.ctx");
    // With no .git above, the root is the directory of the map given.
    std::fs::remove_dir(dir.path().join(".git")).unwrap();
    let o = common::sakai_in(&here, &["api", "基本/基本.ctx"]);
    let v: serde_json::Value = serde_json::from_str(&out(&o)).unwrap();
    assert_eq!(v["map"]["file"], "基本.ctx");
    // One JSON object a map; a directory stands for every map under it. The map is written from
    // the root, and the root from where sakai runs.
    let o = sakai(&["check", "tests/maps", "--format", "json", "--root", "tests/maps"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    let lines: Vec<serde_json::Value> = out(&o).lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    let files: Vec<&str> = lines.iter().map(|v| v["file"].as_str().unwrap()).collect();
    assert_eq!(files, ["basic/basic.ctx", "nested/nested.ctx", "patterns/patterns.ctx", "rust/shop.ctx", "パターン/パターン.ctx", "入れ子/入れ子.ctx", "基本/基本.ctx"]);
    assert!(lines.iter().all(|v| v["root"] == "tests/maps"));
    assert!(lines.iter().all(|v| v["ok"] == true && v["diagnostics"].as_array().unwrap().is_empty()));
}

/// A diagnostic writes the place of a file as the suite's tools do: from where sakai runs, the way
/// the path given was written (DESIGN 2.4, 5.1). JSON writes it from the root, with the root
/// beside it. A name keeps its path from the root, in the text as in JSON, so that read again it
/// is the same name.
#[test]
fn the_paths_of_a_diagnostic_are_written_from_where_sakai_runs() {
    let dir = common::mutant("E401_注文の状態に値が増えた");
    let parent = dir.path().parent().unwrap();
    let base = dir.path().file_name().unwrap().to_string_lossy().to_string();
    // From the directory above the root.
    let o = common::sakai_in(parent, &["check", &format!("{base}/基本.ctx"), "--root", &base]);
    assert_eq!(code(&o), 1);
    let t = out(&o);
    assert!(t.starts_with(&format!("error[E401]: {base}/ctx/請求.ctx:17:3:")), "{t}");
    assert!(t.contains(&format!("= ORDER_STATUS_RETURNED is the value at {base}/proto/shop/ordering/v1/order.proto:16.")), "{t}");
    assert!(t.contains("受注  proto \"proto/shop/ordering/v1/order.proto\" enum OrderStatus"), "{t}");
    // From a directory under the root.
    let o = common::sakai_in(&dir.path().join("ctx"), &["check", "../基本.ctx", "--root", ".."]);
    let t = out(&o);
    assert!(t.starts_with("error[E401]: 請求.ctx:17:3:"), "{t}");
    assert!(t.contains("請求  請求.ctx:14 "), "{t}");
    assert!(t.contains("受注  proto \"proto/shop/ordering/v1/order.proto\" enum OrderStatus"), "{t}");
    assert!(t.contains("= ORDER_STATUS_RETURNED is the value at ../proto/shop/ordering/v1/order.proto:16."), "{t}");
    // An absolute path given, absolute paths written.
    let abs = dir.path().join("基本.ctx");
    let o = common::sakai_in(parent, &["check", abs.to_str().unwrap()]);
    let t = out(&o);
    let want = dir.path().join("ctx/請求.ctx");
    assert!(t.starts_with(&format!("error[E401]: {}:17:3:", want.display())), "{t}");
    // In JSON the places of files are written from the root, and the root from where sakai runs;
    // a name keeps its path from the root.
    let o = common::sakai_in(parent, &["check", &format!("{base}/基本.ctx"), "--root", &base, "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(out(&o).trim()).unwrap();
    assert_eq!(v["root"], base);
    assert_eq!(v["file"], "基本.ctx");
    let d = &v["diagnostics"][0];
    assert_eq!(d["file"], "ctx/請求.ctx");
    assert_eq!(d["references"][0]["file"], "ctx/請求.ctx");
    assert_eq!(d["references"][1]["name"]["text"], "proto \"proto/shop/ordering/v1/order.proto\" enum OrderStatus");
    assert_eq!(d["references"][1]["name"]["path"], "proto/shop/ordering/v1/order.proto");
}

#[test]
fn a_map_with_errors_exits_1() {
    let dir = common::mutant("E401_注文の状態に値が増えた");
    let o = common::sakai_in(dir.path(), &["check", "."]);
    assert_eq!(code(&o), 1);
    assert!(out(&o).starts_with("error[E401]: ctx/請求.ctx:17:3:"), "{}", out(&o));
    let o = common::sakai_in(dir.path(), &["api", "基本.ctx"]);
    assert_eq!(code(&o), 1);
    assert!(out(&o).is_empty());
    assert!(err(&o).contains("error[E401]"), "{}", err(&o));
}

/// `gen` writes nothing from a map with errors, and `--check` says it when the file is not there.
#[test]
fn gen_on_the_command_line() {
    let dir = common::mutant("E401_注文の状態に値が増えた");
    let o = common::sakai_in(dir.path(), &["gen", "基本.ctx", "--target", "import-linter"]);
    assert_eq!(code(&o), 1);
    assert!(out(&o).starts_with("error[E401]: ctx/請求.ctx:17:3:"), "{}", out(&o));
    assert!(!dir.path().join("py/.importlinter").exists());
    let dir = common::variant("基本", &[]);
    let o = common::sakai_in(dir.path(), &["gen", "基本.ctx", "--target", "import-linter", "--check"]);
    assert_eq!(code(&o), 1);
    assert!(out(&o).starts_with("error[E502]: py/.importlinter: The settings file py/.importlinter is not there"), "{}", out(&o));
    let o = common::sakai_in(dir.path(), &["gen", "基本.ctx", "--target", "import-linter"]);
    assert_eq!(code(&o), 0);
    assert_eq!(out(&o), "py/.importlinter: Written (3 contracts)\n");
    let o = common::sakai_in(dir.path(), &["gen", "基本.ctx", "--target", "import-linter", "--check", "--lang", "ja"]);
    assert_eq!(code(&o), 1, "the file was written in English");
    let o = common::sakai_in(dir.path(), &["gen", "基本.ctx", "--target", "import-linter", "--check"]);
    assert_eq!(out(&o), "py/.importlinter: Up to date (3 contracts)\n");
    // --out puts it elsewhere; the header names the map from there.
    let o = common::sakai_in(dir.path(), &["gen", "基本.ctx", "--target", "import-linter", "--out", "lint"]);
    assert_eq!(out(&o), "lint/.importlinter: Written (3 contracts)\n");
    let written = std::fs::read_to_string(dir.path().join("lint/.importlinter")).unwrap();
    assert!(written.starts_with("# Written by `sakai gen --target import-linter` from ../基本.ctx.\n# Edit the .ctx files and write it again; `sakai gen --target import-linter --check` says whether it is up to date.\n"), "{written}");
}

#[test]
fn the_language_from_the_flag_or_the_environment() {
    let ja = Command::new(env!("CARGO_BIN_EXE_sakai")).args(["check", MAP, "--root", ROOT]).env("SAKAI_LANG", "ja").output().unwrap();
    assert!(out(&ja).contains("コンテキスト 3、関係 3。"), "{}", out(&ja));
    let en = Command::new(env!("CARGO_BIN_EXE_sakai")).args(["check", MAP, "--root", ROOT, "--lang", "en"]).env("SAKAI_LANG", "ja").output().unwrap();
    assert!(out(&en).contains("3 contexts"), "--lang wins over SAKAI_LANG: {}", out(&en));
}

#[test]
fn explain_gives_the_reproduction() {
    let o = sakai(&["explain", "e401"]);
    assert_eq!(code(&o), 0);
    let t = out(&o);
    assert!(t.starts_with("E401 (error) — "), "{t}");
    assert!(t.contains("  map.ctx:") && t.contains("enum Kind -> alpha_kind"), "{t}");
    // the Japanese output shows the example with Japanese names
    let ja = out(&sakai(&["explain", "e401", "--lang", "ja"]));
    assert!(ja.contains("  地図.ctx:") && ja.contains("enum Kind -> 甲の種類"), "{ja}");
    let o = sakai(&["explain", "E104", "--lang", "ja"]);
    assert!(out(&o).contains("`sakai check .` を走らせます"), "{}", out(&o));
    // a code that reads another language comes out with every language joined
    let o = sakai(&["explain", "E105"]);
    assert!(out(&o).contains("run `ritsu sakai check .` there"), "{}", out(&o));
    let o = sakai(&["explain", "N101"]);
    assert!(out(&o).contains("Retired in ritsu 0.23.0"), "{}", out(&o));
    let all = out(&sakai(&["explain", "--all"]));
    assert!(all.contains("E001 (error)") && all.contains("W402 (warning)") && all.contains("N101 (note)") && all.contains("E209 (error)"));
    let md = out(&sakai(&["explain", "--all", "--format", "markdown", "--lang", "ja"]));
    assert!(md.starts_with("# 診断のコード\n"), "{md}");
    assert!(md.contains("<a id=\"e401\"></a>") && md.contains("```ctx\n"));
}

#[test]
fn api_prints_the_map() {
    let o = sakai(&["api", MAP, "--root", ROOT]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    let v: serde_json::Value = serde_json::from_str(&out(&o)).unwrap();
    assert_eq!(v["map"]["file"], "基本.ctx");
    assert_eq!(v["contexts"].as_array().unwrap().len(), 3);
}

// ── The English twins: the same checks on the English map ──

const MAP_EN: &str = "tests/maps/basic/basic.ctx";
const ROOT_EN: &str = "tests/maps/basic";

#[test]
fn what_is_refused_with_exit_2_in_english() {
    for args in [
        vec!["check", MAP_EN, "--frmat", "json"],
        vec!["check", MAP_EN, "--format", "yaml"],
        vec!["check", MAP_EN, "--format"],
        vec!["check", MAP_EN, "--format", "json", "--format", "json"],
        vec!["check", MAP_EN, "--root"],
        vec!["check", MAP_EN, "--root", "tests/maps/none"],
        vec!["check", MAP_EN, "--root", "tests/maps/patterns"],
        vec!["check", MAP_EN, "--lang", "fr"],
        vec!["check", "tests/maps/none.ctx"],
        vec!["check", "tests/maps/basic/ctx/ordering.ctx"],
        vec!["api", ROOT_EN],
        vec!["api", MAP_EN, MAP_EN],
        vec!["api", MAP_EN, "--format", "json"],
        vec!["gen", MAP_EN],
        vec!["gen", MAP_EN, "--target", "depguard"],
        vec!["gen", MAP_EN, MAP_EN, "--target", "import-linter"],
        vec!["gen", ROOT_EN, "--target", "import-linter"],
        vec!["export", MAP_EN],
        vec!["export", "plantuml", MAP_EN],
    ] {
        let o = sakai(&args);
        assert_eq!(code(&o), 2, "{args:?}: {}{}", out(&o), err(&o));
        assert!(!err(&o).is_empty(), "{args:?} says why");
    }
}

#[test]
fn check_exit_codes_and_formats_in_english() {
    let o = sakai(&["check", MAP_EN, "--root", ROOT_EN]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert_eq!(out(&o), "tests/maps/basic/basic.ctx: ok — 3 contexts, 3 relationships; 9 artifacts, each in one context; 3 crossings checked (proto 3)\n");
    let dir = common::TempDir::new("root-en");
    std::fs::create_dir_all(dir.path().join(".git")).unwrap();
    let here = dir.path().join("a/b");
    common::copy_dir(std::path::Path::new(ROOT_EN), &here.join("basic"));
    let o = common::sakai_in(&here, &["check", "basic/basic.ctx"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    assert_eq!(out(&o), "basic/basic.ctx: ok — 3 contexts, 3 relationships; 9 artifacts, each in one context; 3 crossings checked (proto 3)\n");
    let o = common::sakai_in(&here, &["api", "basic/basic.ctx"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    let v: serde_json::Value = serde_json::from_str(&out(&o)).unwrap();
    assert_eq!(v["map"]["file"], "a/b/basic/basic.ctx");
    std::fs::remove_dir(dir.path().join(".git")).unwrap();
    let o = common::sakai_in(&here, &["api", "basic/basic.ctx"]);
    let v: serde_json::Value = serde_json::from_str(&out(&o)).unwrap();
    assert_eq!(v["map"]["file"], "basic.ctx");
}

#[test]
fn the_paths_of_a_diagnostic_are_written_from_where_sakai_runs_in_english() {
    let dir = common::mutant("E401_value_added_to_the_order_status");
    let parent = dir.path().parent().unwrap();
    let base = dir.path().file_name().unwrap().to_string_lossy().to_string();
    let o = common::sakai_in(parent, &["check", &format!("{base}/basic.ctx"), "--root", &base]);
    assert_eq!(code(&o), 1);
    let t = out(&o);
    assert!(t.starts_with(&format!("error[E401]: {base}/ctx/billing.ctx:17:3:")), "{t}");
    assert!(t.contains(&format!("= ORDER_STATUS_RETURNED is the value at {base}/proto/shop/ordering/v1/order.proto:16.")), "{t}");
    assert!(t.contains("Ordering  proto \"proto/shop/ordering/v1/order.proto\" enum OrderStatus"), "{t}");
    let o = common::sakai_in(&dir.path().join("ctx"), &["check", "../basic.ctx", "--root", ".."]);
    let t = out(&o);
    assert!(t.starts_with("error[E401]: billing.ctx:17:3:"), "{t}");
    assert!(t.contains("Billing   billing.ctx:14 "), "{t}");
    assert!(t.contains("Ordering  proto \"proto/shop/ordering/v1/order.proto\" enum OrderStatus"), "{t}");
    assert!(t.contains("= ORDER_STATUS_RETURNED is the value at ../proto/shop/ordering/v1/order.proto:16."), "{t}");
    let abs = dir.path().join("basic.ctx");
    let o = common::sakai_in(parent, &["check", abs.to_str().unwrap()]);
    let t = out(&o);
    let want = dir.path().join("ctx/billing.ctx");
    assert!(t.starts_with(&format!("error[E401]: {}:17:3:", want.display())), "{t}");
    let o = common::sakai_in(parent, &["check", &format!("{base}/basic.ctx"), "--root", &base, "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(out(&o).trim()).unwrap();
    assert_eq!(v["root"], base);
    assert_eq!(v["file"], "basic.ctx");
    let d = &v["diagnostics"][0];
    assert_eq!(d["file"], "ctx/billing.ctx");
    assert_eq!(d["references"][0]["file"], "ctx/billing.ctx");
    assert_eq!(d["references"][1]["name"]["text"], "proto \"proto/shop/ordering/v1/order.proto\" enum OrderStatus");
    assert_eq!(d["references"][1]["name"]["path"], "proto/shop/ordering/v1/order.proto");
}

#[test]
fn a_map_with_errors_exits_1_in_english() {
    let dir = common::mutant("E401_value_added_to_the_order_status");
    let o = common::sakai_in(dir.path(), &["check", "."]);
    assert_eq!(code(&o), 1);
    assert!(out(&o).starts_with("error[E401]: ctx/billing.ctx:17:3:"), "{}", out(&o));
    let o = common::sakai_in(dir.path(), &["api", "basic.ctx"]);
    assert_eq!(code(&o), 1);
    assert!(out(&o).is_empty());
    assert!(err(&o).contains("error[E401]"), "{}", err(&o));
}

#[test]
fn gen_on_the_command_line_in_english() {
    let dir = common::mutant("E401_value_added_to_the_order_status");
    let o = common::sakai_in(dir.path(), &["gen", "basic.ctx", "--target", "import-linter"]);
    assert_eq!(code(&o), 1);
    assert!(out(&o).starts_with("error[E401]: ctx/billing.ctx:17:3:"), "{}", out(&o));
    assert!(!dir.path().join("py/.importlinter").exists());
    let dir = common::variant("basic", &[]);
    let o = common::sakai_in(dir.path(), &["gen", "basic.ctx", "--target", "import-linter", "--check"]);
    assert_eq!(code(&o), 1);
    assert!(out(&o).starts_with("error[E502]: py/.importlinter: The settings file py/.importlinter is not there"), "{}", out(&o));
    let o = common::sakai_in(dir.path(), &["gen", "basic.ctx", "--target", "import-linter"]);
    assert_eq!(code(&o), 0);
    assert_eq!(out(&o), "py/.importlinter: Written (3 contracts)\n");
    let o = common::sakai_in(dir.path(), &["gen", "basic.ctx", "--target", "import-linter", "--check", "--lang", "ja"]);
    assert_eq!(code(&o), 1, "the file was written in English");
    let o = common::sakai_in(dir.path(), &["gen", "basic.ctx", "--target", "import-linter", "--check"]);
    assert_eq!(out(&o), "py/.importlinter: Up to date (3 contracts)\n");
    let o = common::sakai_in(dir.path(), &["gen", "basic.ctx", "--target", "import-linter", "--out", "lint"]);
    assert_eq!(out(&o), "lint/.importlinter: Written (3 contracts)\n");
    let written = std::fs::read_to_string(dir.path().join("lint/.importlinter")).unwrap();
    assert!(written.starts_with("# Written by `sakai gen --target import-linter` from ../basic.ctx."), "{written}");
}

/// `--target` given more than once: every tool's settings, in the order of the tools whatever the
/// order of the flags, each where its language's code is, or all in `--out`; `--check` holds each
/// to the map; and a tool whose settings cannot be written (E501) stops the others too.
#[test]
fn gen_writes_every_tool_it_is_given() {
    let all = ["--target", "go-arch-lint", "--target", "archunit", "--target", "dependency-cruiser", "--target", "import-linter"];
    let dir = common::variant("nested", &[]);
    let run = |rest: &[&str]| common::sakai_in(dir.path(), &[&["gen", "nested.ctx"][..], rest].concat());
    let o = run(&all);
    assert_eq!((code(&o), out(&o).as_str(), err(&o).as_str()), (0, "py/.importlinter: Written (3 contracts)\nts/.dependency-cruiser.cjs: Written (3 rules)\njava/src/test/java/SakaiContextsTest.java: Written (2 rules)\ngo/.go-arch-lint.yml: Written (3 components)\n", ""));
    // each is what the one-tool run writes
    for (t, f) in [("import-linter", "py/.importlinter"), ("dependency-cruiser", "ts/.dependency-cruiser.cjs"), ("archunit", "java/src/test/java/SakaiContextsTest.java"), ("go-arch-lint", "go/.go-arch-lint.yml")] {
        let many = std::fs::read_to_string(dir.path().join(f)).unwrap();
        assert_eq!(code(&run(&["--target", t])), 0);
        assert_eq!(std::fs::read_to_string(dir.path().join(f)).unwrap(), many, "{t}");
    }
    let o = run(&[&all[..], &["--check"]].concat());
    assert_eq!((code(&o), out(&o).as_str()), (0, "py/.importlinter: Up to date (3 contracts)\nts/.dependency-cruiser.cjs: Up to date (3 rules)\njava/src/test/java/SakaiContextsTest.java: Up to date (2 rules)\ngo/.go-arch-lint.yml: Up to date (3 components)\n"));
    // one stale and one missing: each said, the rest up to date
    std::fs::write(dir.path().join("py/.importlinter"), "# written by hand\n").unwrap();
    std::fs::remove_file(dir.path().join("ts/.dependency-cruiser.cjs")).unwrap();
    let o = run(&[&all[..], &["--check"]].concat());
    assert_eq!(code(&o), 1);
    assert!(out(&o).starts_with("error[E502]: py/.importlinter:1:1: The settings file py/.importlinter differs from what the map writes now\n"), "{}", out(&o));
    assert!(out(&o).contains("error[E502]: ts/.dependency-cruiser.cjs: The settings file ts/.dependency-cruiser.cjs is not there\n  = Write it with `sakai gen --target dependency-cruiser`.\n"), "{}", out(&o));
    assert!(out(&o).ends_with("java/src/test/java/SakaiContextsTest.java: Up to date (2 rules)\ngo/.go-arch-lint.yml: Up to date (3 components)\n"), "{}", out(&o));
    // --out puts every file in it
    let o = run(&["--target", "import-linter", "--target", "go-arch-lint", "--out", "lint"]);
    assert_eq!(out(&o), "lint/.importlinter: Written (3 contracts)\nlint/.go-arch-lint.yml: Written (3 components)\n");
    // go-arch-lint cannot be written with no go.mod: nothing is
    std::fs::remove_file(dir.path().join("go/go.mod")).unwrap();
    std::fs::remove_file(dir.path().join("py/.importlinter")).unwrap();
    let o = run(&["--target", "import-linter", "--target", "go-arch-lint"]);
    assert_eq!(code(&o), 1);
    assert!(out(&o).starts_with("error[E501]: nested.ctx:16:1: There is no go.mod in go\n"), "{}", out(&o));
    assert!(!out(&o).contains("Written") && !dir.path().join("py/.importlinter").exists(), "{}", out(&o));
}

/// The verb of 0.40.0 stops with the one of now, as `help` does; no tool is a mistake of its own.
#[test]
fn the_verb_of_0_40_stops_with_the_verb_now() {
    let said = |args: &[&str]| {
        let o = sakai(args);
        (code(&o), out(&o), err(&o))
    };
    let renamed = (2, String::new(), "error: `sakai build` is `sakai gen` from 0.41.0\n".to_string());
    assert_eq!(said(&["build", MAP_EN, "--target", "import-linter"]), renamed);
    assert_eq!(said(&["help", "build"]), renamed);
    assert_eq!(said(&["build", MAP_EN, "--lang", "ja"]), (2, String::new(), "エラー: `sakai build` は 0.41.0 から `sakai gen` です\n".to_string()));
    assert_eq!(said(&["gen", MAP_EN]), (2, String::new(), "error: `sakai gen` needs one or more `--target import-linter|dependency-cruiser|archunit|go-arch-lint`\n".to_string()));
    assert_eq!(said(&["gen", MAP_EN, "--lang", "ja"]), (2, String::new(), "エラー: `sakai gen` には `--target import-linter|dependency-cruiser|archunit|go-arch-lint` が一つ以上要ります\n".to_string()));
    assert_eq!(said(&["gen", MAP_EN, "--target", "archunit", "--target", "archunit"]), (2, String::new(), "error: `--target archunit` is given twice\n".to_string()));
}

#[test]
fn the_language_from_the_flag_or_the_environment_in_english() {
    let ja = Command::new(env!("CARGO_BIN_EXE_sakai")).args(["check", MAP_EN, "--root", ROOT_EN]).env("SAKAI_LANG", "ja").output().unwrap();
    assert!(out(&ja).contains("コンテキスト 3、関係 3。"), "{}", out(&ja));
    let en = Command::new(env!("CARGO_BIN_EXE_sakai")).args(["check", MAP_EN, "--root", ROOT_EN, "--lang", "en"]).env("SAKAI_LANG", "ja").output().unwrap();
    assert!(out(&en).contains("3 contexts"), "--lang wins over SAKAI_LANG: {}", out(&en));
}

#[test]
fn api_prints_the_map_in_english() {
    let o = sakai(&["api", MAP_EN, "--root", ROOT_EN]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    let v: serde_json::Value = serde_json::from_str(&out(&o)).unwrap();
    assert_eq!(v["map"]["file"], "basic.ctx");
    assert_eq!(v["contexts"].as_array().unwrap().len(), 3);
}
