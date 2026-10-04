//! The command line (DESIGN 7, PLAN B.13): one table for the commands and flags, the exit
//! codes, and the language.

mod common;

use std::path::Path;

fn run(args: &[&str]) -> common::Ran {
    common::yuen(Path::new("."), args)
}

const COMMANDS: &[&str] = &["check", "review", "trace", "affected", "api", "export", "source", "explain"];

#[test]
fn every_command_has_its_help_page() {
    for c in COMMANDS {
        let r = run(&[c, "--help"]);
        assert_eq!(r.code, 0, "{c}");
        assert!(r.stdout.starts_with(&format!("yuen {c} — ")), "{c}: {}", r.stdout);
        assert!(r.stdout.contains("--lang ja|en"), "{c}");
        assert!(r.stdout.contains("--root <dir>"), "{c}");
        assert_eq!(run(&["help", c]).stdout, r.stdout, "{c}");
        let ja = run(&[c, "--help", "--lang", "ja"]);
        assert!(ja.stdout.contains("使い方:"), "{c}");
    }
    let r = run(&["--help"]);
    assert_eq!(r.code, 0);
    for c in COMMANDS {
        assert!(r.stdout.contains(&format!("  yuen {c} ")), "{c} is listed");
    }
    // The command of the next stage is not in the table yet.
    for c in ["doc"] {
        assert!(!r.stdout.contains(&format!("  yuen {c} ")), "{c} is not built yet");
        assert_eq!(run(&[c]).code, 2, "{c}");
    }
    assert!(run(&["check", "--help"]).stdout.contains("E302"), "check lists the codes it can print");
}

#[test]
fn exit_codes() {
    assert_eq!(run(&[]).code, 2);
    assert_eq!(run(&["--version"]).stdout, format!("yuen {}\n", env!("CARGO_PKG_VERSION")));
    assert_eq!(run(&["frobnicate"]).code, 2);
    assert_eq!(run(&["check", "tests/fixtures/period"]).code, 0);
    assert_eq!(run(&["check", "tests/mutants/E302_条が変わった"]).code, 1);
    assert_eq!(run(&["check", "tests/mutants/W101_引かれていない固定"]).code, 0, "a warning is not an error");
    assert_eq!(run(&["check", "no/such/path"]).code, 2);
    assert_eq!(run(&["check"]).code, 2);
    assert_eq!(run(&["check", "--root", "no/such/dir", "tests/fixtures/period"]).code, 2);
    assert_eq!(run(&["explain", "E302"]).code, 0);
    assert_eq!(run(&["explain", "e302"]).code, 0);
    assert_eq!(run(&["explain", "E999"]).code, 2);
    assert_eq!(run(&["explain"]).code, 2);
}

#[test]
fn flags_are_read_against_the_table() {
    let r = run(&["check", "tests/fixtures/period", "--fromat", "json"]);
    assert_eq!(r.code, 2);
    assert!(r.stderr.contains("unknown flag `--fromat`"), "{}", r.stderr);
    assert_eq!(run(&["check", "tests/fixtures/period", "--format", "yaml"]).code, 2);
    assert_eq!(run(&["check", "tests/fixtures/period", "--format"]).code, 2);
    assert_eq!(run(&["check", "tests/fixtures/period", "--lang", "ja", "--lang", "en"]).code, 2);
    assert_eq!(run(&["explain", "--all=yes"]).code, 2);
    assert_eq!(run(&["check", "tests/fixtures/period", "--format=json"]).code, 0);
    // `--at` and `--requirement` may repeat for review.
    let t = common::fixture("period");
    let r = common::yuen(t.path(), &["review", "period", "--root", "period", "--requirement", "起算日", "--requirement", "満了日", "--by", "法務"]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert_eq!(common::yuen(t.path(), &["review", "period", "--by", "法務", "--by", "開発", "--all"]).code, 2);
}

#[test]
fn the_language() {
    let en = run(&["check", "tests/mutants/E302_条が変わった"]);
    assert!(en.stdout.starts_with("error[E302]"));
    let ja = run(&["check", "tests/mutants/E302_条が変わった", "--lang", "ja"]);
    assert!(ja.stdout.starts_with("エラー[E302]"));
    let o = std::process::Command::new(env!("CARGO_BIN_EXE_yuen")).args(["check", "tests/mutants/E302_条が変わった"]).env("YUEN_LANG", "ja").output().unwrap();
    assert!(String::from_utf8_lossy(&o.stdout).starts_with("エラー[E302]"));
    let o = std::process::Command::new(env!("CARGO_BIN_EXE_yuen")).args(["check", "tests/mutants/E302_条が変わった", "--lang", "en"]).env("YUEN_LANG", "ja").output().unwrap();
    assert!(String::from_utf8_lossy(&o.stdout).starts_with("error[E302]"), "--lang wins");
}

#[test]
fn the_json_of_check() {
    let r = run(&["check", "tests/fixtures/period", "--root", ".", "--format", "json"]);
    assert_eq!(r.code, 0);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["root"], ".");
    assert_eq!(r.stdout.lines().count(), 1, "one line for the project");
}

/// A root above the directory yuen runs in, as for a crate of a workspace whose `.git` is two
/// levels up: the text writes a path the shortest way from where yuen runs, not up to the root
/// and down again (`../../crates/a/period/…`); the JSON keeps the path from the root and says
/// where the root is. From above the root, a path goes down into it as before.
#[test]
fn a_root_above_where_yuen_runs() {
    let t = common::TempDir::new("above");
    std::fs::create_dir_all(t.path().join(".git")).unwrap();
    let here = t.path().join("crates/a");
    common::copy_dir(Path::new("tests/fixtures/period"), &here.join("period"));
    common::change_142(&here.join("period"));
    let copy = "period/sources/law/129AC0000000089@2026-10-01/MainProvision-Article_142.xml";

    let r = common::yuen(&here, &["check", "period"]);
    assert_eq!(r.code, 1, "{}", r.stdout);
    assert!(r.stdout.contains(&format!("(the copy {copy})")), "{}", r.stdout);
    assert!(r.stdout.contains("yuen review period --at period/民法の期間.req:41 --by <role>"), "{}", r.stdout);
    assert!(!r.stdout.contains("../"), "no path goes up to the root and down again:\n{}", r.stdout);

    let r = common::yuen(&here, &["check", "period", "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["root"], "../..");
    assert_eq!(v["diagnostics"][0]["file"], "crates/a/period/民法の期間.req");

    let r = common::yuen(t.path(), &["check", "crates/a/period"]);
    assert!(r.stdout.contains(&format!("(the copy crates/a/{copy})")), "{}", r.stdout);
    let r = common::yuen(&here.join("period"), &["check", "."]);
    assert!(r.stdout.contains("(the copy sources/law/129AC0000000089@2026-10-01/MainProvision-Article_142.xml)"), "{}", r.stdout);
}

/// The binary of this crate holds no other language (ritsu's DESIGN 2.3): a project that names
/// what only another language reads is told so (E206) with exit 2, and to run through `ritsu
/// yuen`, rather than passed unchecked. The same project, with every language joined, is checked.
#[test]
fn the_binary_of_this_crate_reads_no_other_language() {
    let t = common::TempDir::new("noport");
    t.write(
        "t.req",
        "requirements t v1\nrole 経理\n\nrequirement r1\n  text \"x\"\n  owner 経理\n  decided 2026-10-03 by 経理 \"y\"\n  satisfied by koyomi \"a.cal\" date 翌日\n  not verified \"z\"\n".as_bytes(),
    );
    t.write("a.cal", "dates 例(example) v1\n\ninputs\n  起点(origin) : date  range >=2026-01-01 <=2026-12-31\n\ndate 翌日(next_day) = 起点\n  + 1 day\n".as_bytes());
    let r = common::yuen(t.path(), &["check", ".", "--root", "."]);
    assert_eq!(r.code, 2);
    assert_eq!(
        r.stdout,
        "error[E206]: ./t.req:8:16: this yuen cannot read koyomi artifacts: koyomi \"a.cal\" date 翌日\n     8 |   satisfied by koyomi \"a.cal\" date 翌日\n  = The binary of yuen's own crate holds no other language; run it with every language joined, through ritsu: `ritsu yuen check . --root .`.\n.: 1 error\n"
    );
    let ja = common::yuen(t.path(), &["check", ".", "--root", ".", "--lang", "ja"]);
    assert!(ja.stdout.contains("すべての言語をつないだ `ritsu yuen check . --root . --lang ja`"), "{}", ja.stdout);
    // the commands built on the check say the same, on standard error, with exit 2
    let api = common::yuen(t.path(), &["api", ".", "--root", "."]);
    assert_eq!(api.code, 2);
    assert!(api.stderr.starts_with("error[E206]: ./t.req:8:16: "), "{}", api.stderr);
    let path = t.path().to_string_lossy().to_string();
    let joined = common::run(&["check", &path, "--root", &path]);
    assert_eq!(joined.code, 1, "{}{}", joined.stdout, joined.stderr);
    assert_eq!(common::codes(&joined.stdout), ["E301", "E304"], "{}", joined.stdout);
    // a `.proto` and a file are read by yuen itself
    assert_eq!(run(&["check", "tests/fixtures/proto", "--root", "tests/fixtures/proto"]).code, 0);
}

#[test]
fn explain_prints_every_code() {
    let r = run(&["explain", "--all"]);
    assert_eq!(r.code, 0);
    assert!(r.stdout.matches(" — ").count() >= 44);
    let md = run(&["explain", "--all", "--format", "markdown", "--lang", "ja"]);
    assert!(md.stdout.starts_with("# 診断のコード\n"));
    assert_eq!(md.stdout.matches("<a id=\"").count(), 44);
    let one = run(&["explain", "E302"]);
    assert!(one.stdout.starts_with("E302 (error) — The upper end changed after the link was looked at\n"), "{}", one.stdout);
    let e107 = run(&["explain", "E107"]);
    assert!(e107.stdout.contains("Example:\n    requirements 例 v1\n"), "{}", e107.stdout);
    assert!(e107.stdout.contains("beside it: cal/a.cal"), "{}", e107.stdout);
    let retired = run(&["explain", "E204"]);
    assert!(retired.stdout.contains("Example:\n    Retired in ritsu 0.23.0"), "{}", retired.stdout);
}

/// `check` and the commands that read what it found never read the network (DESIGN 7, P4):
/// with no `curl` to be found, they all run as before.
#[test]
fn the_commands_that_do_not_read_the_network_run_without_curl() {
    let t = common::TempDir::new("nopath");
    let empty = t.path().to_str().unwrap();
    let p = "tests/fixtures/period";
    for args in [
        vec!["check", p],
        vec!["trace", p, "--requirement", "満了日_142条"],
        vec!["api", p],
        vec!["export", "reqif", p],
        vec!["export", "prov", p, "--format", "json"],
    ] {
        let r = common::yuen_env(Path::new("."), &args, &[("PATH", empty)]);
        assert_eq!(r.code, 0, "{args:?}: {}", r.stderr);
    }
}
