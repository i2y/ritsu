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
    for c in ["check", "build", "export", "api", "explain"] {
        assert!(out(&o).contains(&format!("sakai {c} ")), "{c} is in --help");
    }
    let o = sakai(&["--version"]);
    assert_eq!(out(&o).trim(), format!("sakai {}", env!("CARGO_PKG_VERSION")));
    assert_eq!(code(&sakai(&["-V"])), 0);
}

#[test]
fn every_command_has_its_help() {
    for c in ["check", "build", "export", "api", "explain"] {
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
    assert!(!check.contains("E501"), "build's codes are not check's");
    let build = out(&sakai(&["build", "--help"]));
    assert!(build.contains("E501 E502") && build.contains("--target <tool>"), "{build}");
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
        vec!["build", MAP],
        vec!["build", MAP, "--target", "depguard"],
        vec!["build", MAP, MAP, "--target", "import-linter"],
        vec!["build", ROOT, "--target", "import-linter"],
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
    // Without --root the root is the repository's, the nearest directory with .git.
    if std::path::Path::new(".git").exists() {
        let o = sakai(&["check", MAP]);
        assert!(out(&o).starts_with("tests/maps/基本/基本.ctx: ok"), "{}", out(&o));
    }
    // One JSON object a map; a directory stands for every map under it.
    let o = sakai(&["check", "tests/maps", "--format", "json", "--root", "tests/maps"]);
    assert_eq!(code(&o), 0, "{}", err(&o));
    let lines: Vec<serde_json::Value> = out(&o).lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    let files: Vec<&str> = lines.iter().map(|v| v["file"].as_str().unwrap()).collect();
    assert_eq!(files, ["tests/maps/パターン/パターン.ctx", "tests/maps/入れ子/入れ子.ctx", "tests/maps/基本/基本.ctx"]);
    assert!(lines.iter().all(|v| v["ok"] == true && v["diagnostics"].as_array().unwrap().is_empty()));
}

/// A diagnostic writes the place of a file as the suite's tools do: from where sakai runs, the way
/// the path given was written (DESIGN 2.4, 5.1). A name keeps its path from the root, in the text
/// as in JSON, so that read again it is the same name.
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
    // In JSON the places of files are written the same way, and a name keeps its path from the root.
    let o = common::sakai_in(parent, &["check", &format!("{base}/基本.ctx"), "--root", &base, "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(out(&o).trim()).unwrap();
    assert_eq!(v["file"], format!("{base}/基本.ctx"));
    let d = &v["diagnostics"][0];
    assert_eq!(d["file"], format!("{base}/ctx/請求.ctx"));
    assert_eq!(d["references"][0]["file"], format!("{base}/ctx/請求.ctx"));
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

/// `build` writes nothing from a map with errors, and `--check` says it when the file is not there.
#[test]
fn build_on_the_command_line() {
    let dir = common::mutant("E401_注文の状態に値が増えた");
    let o = common::sakai_in(dir.path(), &["build", "基本.ctx", "--target", "import-linter"]);
    assert_eq!(code(&o), 1);
    assert!(out(&o).starts_with("error[E401]: ctx/請求.ctx:17:3:"), "{}", out(&o));
    assert!(!dir.path().join("py/.importlinter").exists());
    let dir = common::variant("基本", &[]);
    let o = common::sakai_in(dir.path(), &["build", "基本.ctx", "--target", "import-linter", "--check"]);
    assert_eq!(code(&o), 1);
    assert!(out(&o).starts_with("error[E502]: py/.importlinter: The settings file py/.importlinter is not there"), "{}", out(&o));
    let o = common::sakai_in(dir.path(), &["build", "基本.ctx", "--target", "import-linter"]);
    assert_eq!(code(&o), 0);
    assert_eq!(out(&o), "py/.importlinter: Written (3 contracts)\n");
    let o = common::sakai_in(dir.path(), &["build", "基本.ctx", "--target", "import-linter", "--check", "--lang", "ja"]);
    assert_eq!(code(&o), 1, "the file was written in English");
    let o = common::sakai_in(dir.path(), &["build", "基本.ctx", "--target", "import-linter", "--check"]);
    assert_eq!(out(&o), "py/.importlinter: Up to date (3 contracts)\n");
    // --out puts it elsewhere; the header names the map from there.
    let o = common::sakai_in(dir.path(), &["build", "基本.ctx", "--target", "import-linter", "--out", "lint"]);
    assert_eq!(out(&o), "lint/.importlinter: Written (3 contracts)\n");
    let written = std::fs::read_to_string(dir.path().join("lint/.importlinter")).unwrap();
    assert!(written.starts_with("# Written by `sakai build --target import-linter` from ../基本.ctx."), "{written}");
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
    assert!(t.contains("  地図.ctx:") && t.contains("enum Kind -> 甲の種類"), "{t}");
    let o = sakai(&["explain", "E104", "--lang", "ja"]);
    assert!(out(&o).contains("まだ出さない"), "{}", out(&o));
    let all = out(&sakai(&["explain", "--all"]));
    assert!(all.contains("E001 (error)") && all.contains("W402 (warning)") && all.contains("N101 (note)"));
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
