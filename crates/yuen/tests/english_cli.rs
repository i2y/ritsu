//! The command line (DESIGN 7), in English: the twin of cli.rs, on the fixture
//! `period_of_months`, the mutants of English names, and requirements written in English. What
//! does not read a material (the help pages, the unknown command) is cli.rs's alone.

mod common;

use std::path::Path;

fn run(args: &[&str]) -> common::Ran {
    common::yuen(Path::new("."), args)
}

#[test]
fn exit_codes_in_english() {
    assert_eq!(run(&["check", "tests/fixtures/period_of_months"]).code, 0);
    assert_eq!(run(&["check", "tests/mutants/E302_article_changed"]).code, 1);
    assert_eq!(run(&["check", "tests/mutants/W101_pin_not_cited"]).code, 0, "a warning is not an error");
    assert_eq!(run(&["check", "--root", "no/such/dir", "tests/fixtures/period_of_months"]).code, 2);
    assert_eq!(run(&["explain", "E302"]).code, 0);
}

#[test]
fn flags_are_read_against_the_table_in_english() {
    let r = run(&["check", "tests/fixtures/period_of_months", "--fromat", "json"]);
    assert_eq!(r.code, 2);
    assert!(r.stderr.contains("unknown flag `--fromat`"), "{}", r.stderr);
    assert_eq!(run(&["check", "tests/fixtures/period_of_months", "--format", "yaml"]).code, 2);
    assert_eq!(run(&["check", "tests/fixtures/period_of_months", "--format"]).code, 2);
    assert_eq!(run(&["check", "tests/fixtures/period_of_months", "--lang", "ja", "--lang", "en"]).code, 2);
    assert_eq!(run(&["check", "tests/fixtures/period_of_months", "--format=json"]).code, 0);
    // `--requirement` may repeat for review.
    let t = common::fixture("period_of_months");
    let r = common::yuen(t.path(), &["review", "period_of_months", "--root", "period_of_months", "--requirement", "date_of_receipt", "--requirement", "timely_filing", "--by", "legal"]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert_eq!(common::yuen(t.path(), &["review", "period_of_months", "--by", "legal", "--by", "development", "--all"]).code, 2);
}

#[test]
fn the_language_in_english() {
    let en = run(&["check", "tests/mutants/E302_article_changed"]);
    assert!(en.stdout.starts_with("error[E302]"));
    let ja = run(&["check", "tests/mutants/E302_article_changed", "--lang", "ja"]);
    assert!(ja.stdout.starts_with("エラー[E302]"));
    let o = std::process::Command::new(env!("CARGO_BIN_EXE_yuen")).args(["check", "tests/mutants/E302_article_changed"]).env("YUEN_LANG", "ja").output().unwrap();
    assert!(String::from_utf8_lossy(&o.stdout).starts_with("エラー[E302]"));
    let o = std::process::Command::new(env!("CARGO_BIN_EXE_yuen")).args(["check", "tests/mutants/E302_article_changed", "--lang", "en"]).env("YUEN_LANG", "ja").output().unwrap();
    assert!(String::from_utf8_lossy(&o.stdout).starts_with("error[E302]"), "--lang wins");
}

#[test]
fn the_json_of_check_in_english() {
    let r = run(&["check", "tests/fixtures/period_of_months", "--root", ".", "--format", "json"]);
    assert_eq!(r.code, 0);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["root"], ".");
    assert_eq!(r.stdout.lines().count(), 1, "one line for the project");
}

/// A root above the directory yuen runs in, as for a crate of a workspace whose `.git` is two
/// levels up: the text writes a path the shortest way from where yuen runs, not up to the root
/// and down again; the JSON keeps the path from the root and says where the root is. From above
/// the root, a path goes down into it as before.
#[test]
fn a_root_above_where_yuen_runs_in_english() {
    let t = common::TempDir::new("above");
    std::fs::create_dir_all(t.path().join(".git")).unwrap();
    let here = t.path().join("crates/a");
    common::copy_dir(Path::new("tests/fixtures/period_of_months"), &here.join("period_of_months"));
    common::change_section(&here.join("period_of_months"));
    let copy = "period_of_months/sources/law/37-CFR-1@2026-01-01/1.7.xml";

    let r = common::yuen(&here, &["check", "period_of_months"]);
    assert_eq!(r.code, 1, "{}", r.stdout);
    assert!(r.stdout.contains(&format!("(the copy {copy})")), "{}", r.stdout);
    assert!(r.stdout.contains("yuen review period_of_months --at period_of_months/period_of_months.req:41 --by <role>"), "{}", r.stdout);
    assert!(!r.stdout.contains("../"), "no path goes up to the root and down again:\n{}", r.stdout);

    let r = common::yuen(&here, &["check", "period_of_months", "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["root"], "../..");
    assert_eq!(v["diagnostics"][0]["file"], "crates/a/period_of_months/period_of_months.req");

    let r = common::yuen(t.path(), &["check", "crates/a/period_of_months"]);
    assert!(r.stdout.contains(&format!("(the copy crates/a/{copy})")), "{}", r.stdout);
    let r = common::yuen(&here.join("period_of_months"), &["check", "."]);
    assert!(r.stdout.contains("(the copy sources/law/37-CFR-1@2026-01-01/1.7.xml)"), "{}", r.stdout);
}

/// The binary of this crate holds no other language (ritsu's DESIGN 2.3): a project that names
/// what only another language reads is told so (E206) with exit 2, and to run through `ritsu
/// yuen`, rather than passed unchecked. The same project, with every language joined, is checked.
#[test]
fn the_binary_of_this_crate_reads_no_other_language_in_english() {
    let t = common::TempDir::new("noport");
    t.write(
        "t.req",
        b"requirements t v1\nrole accounting\n\nrequirement r1\n  text \"x\"\n  owner accounting\n  decided 2026-10-03 by accounting \"y\"\n  satisfied by koyomi \"a.cal\" date next_day\n  not verified \"z\"\n",
    );
    t.write("a.cal", b"dates example v1\n\ninputs\n  origin : date  range >=2026-01-01 <=2026-12-31\n\ndate next_day = origin\n  + 1 day\n");
    let r = common::yuen(t.path(), &["check", ".", "--root", "."]);
    assert_eq!(r.code, 2);
    assert_eq!(
        r.stdout,
        "error[E206]: t.req:8:16: this yuen cannot read koyomi artifacts: koyomi \"a.cal\" date next_day\n     8 |   satisfied by koyomi \"a.cal\" date next_day\n  = The binary of yuen's own crate holds no other language; run it with every language joined, through ritsu: `ritsu yuen check . --root .`.\n.: 1 error\n"
    );
    let ja = common::yuen(t.path(), &["check", ".", "--root", ".", "--lang", "ja"]);
    assert!(ja.stdout.contains("すべての言語をつないだ ritsu で、`ritsu yuen check . --root . --lang ja`"), "{}", ja.stdout);
    // the commands built on the check say the same, on standard error, with exit 2
    let api = common::yuen(t.path(), &["api", ".", "--root", "."]);
    assert_eq!(api.code, 2);
    assert!(api.stderr.starts_with("error[E206]: t.req:8:16: "), "{}", api.stderr);
    let path = t.path().to_string_lossy().to_string();
    let joined = common::run(&["check", &path, "--root", &path]);
    assert_eq!(joined.code, 1, "{}{}", joined.stdout, joined.stderr);
    assert_eq!(common::codes(&joined.stdout), ["E301", "E304"], "{}", joined.stdout);
    // a `.proto` and a file are read by yuen itself
    assert_eq!(run(&["check", "tests/fixtures/warehouse_proto", "--root", "tests/fixtures/warehouse_proto"]).code, 0);
}

/// `check` and the commands that read what it found never read the network (DESIGN 7, P4):
/// with no `curl` to be found, they all run as before.
#[test]
fn the_commands_that_do_not_read_the_network_run_without_curl_in_english() {
    let t = common::TempDir::new("nopath");
    let empty = t.path().to_str().unwrap();
    let p = "tests/fixtures/period_of_months";
    for args in [
        vec!["check", p],
        vec!["trace", p, "--requirement", "timely_filing"],
        vec!["api", p],
        vec!["export", "reqif", p],
        vec!["export", "prov", p, "--format", "json"],
    ] {
        let r = common::yuen_env(Path::new("."), &args, &[("PATH", empty)]);
        assert_eq!(r.code, 0, "{args:?}: {}", r.stderr);
    }
}
