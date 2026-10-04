//! Coverage and scope (DESIGN 5.2, 5.3), in English: the twin of coverage.rs.
//!
//! Written by tests/ twins: the Japanese file keeps its tests; these check the same behavior in English.


mod common;

use common::{TempDir, yuen};
use yuen::names::parse_one;

const HEAD: &str = "requirements payment v1\nrole accounting\n\n";

fn run(scope: &str, body: &str, files: &[&str]) -> (Vec<String>, String) {
    let t = TempDir::new("coverage");
    t.write("payment.req", format!("{HEAD}{scope}{body}").as_bytes());
    for f in files {
        t.write(f, b"code\n");
    }
    // Write the records, then check: what is left is coverage and scope.
    yuen(t.path(), &["review", ".", "--root", ".", "--all", "--by", "accounting", "--date", "2026-10-03"]);
    let r = yuen(t.path(), &["check", ".", "--root", "."]);
    (common::codes(&r.stdout), r.stdout)
}

const REQ: &str = "requirement pay\n  text \"x\"\n  owner accounting\n  decided 2026-10-03 by accounting \"example\"\n";

#[test]
fn what_meets_and_what_checks_in_english() {
    let (codes, _) = run("", &format!("{REQ}  not verified \"example\"\n"), &[]);
    assert_eq!(codes, ["E401"]);
    let (codes, _) = run("", &format!("{REQ}  satisfied by file \"pay.py\"\n"), &["pay.py"]);
    assert_eq!(codes, ["E402"]);
    let (codes, out) = run("", &format!("{REQ}  satisfied by file \"pay.py\"\n  not satisfied \"example\"\n  not verified \"example\"\n"), &["pay.py"]);
    assert_eq!(codes, ["W401"], "{out}");
    let (codes, out) = run("", &format!("{REQ}  satisfied by file \"pay.py\"\n  verified by file \"test_pay.py\"\n"), &["pay.py", "test_pay.py"]);
    assert!(codes.is_empty(), "{out}");
    // A waiver counts before it is approved: E304 says so, not E401.
    let t = TempDir::new("coverage-waiver");
    t.write("payment.req", format!("{HEAD}{REQ}  not satisfied \"example\"\n  not verified \"example\"\n").as_bytes());
    let r = yuen(t.path(), &["check", ".", "--root", "."]);
    assert_eq!(common::codes(&r.stdout), ["E304", "E304"]);
}

#[test]
fn every_version_is_counted_in_english() {
    let body = "requirement pay v1\n  text \"x\"\n  in force ..2026-12-31\n  owner accounting\n  decided 2026-10-03 by accounting \"example\"\n  satisfied by file \"pay.py\"\n  not verified \"example\"\n\nrequirement pay v2\n  text \"y\"\n  in force 2027-01-01..\n  owner accounting\n  decided 2026-10-03 by accounting \"example\"\n  not verified \"example\"\n";
    let (codes, out) = run("", body, &["pay.py"]);
    assert_eq!(codes, ["E401"], "{out}");
    assert!(out.contains("pay v2 has nothing that meets it"), "{out}");
}

#[test]
fn a_scope_traces_to_requirements_in_english() {
    let link = format!("{REQ}  satisfied by file \"src/pay.py\"\n  not verified \"example\"\n");
    // The file a link names.
    let (codes, _) = run("scope file \"src/pay.py\"\n\n", &link, &["src/pay.py"]);
    assert!(codes.is_empty());
    // A directory with one file no link names.
    let (codes, out) = run("scope file \"src\"\n\n", &link, &["src/pay.py", "src/report.py", "src/.cache/x", "src/target/y"]);
    assert_eq!(codes, ["E404"], "{out}");
    assert!(out.contains("file \"src/report.py\" is in scope, and no requirement leads to it"), "{out}");
    assert!(out.contains("The rest of the scope is reached: file \"src/pay.py\""), "{out}");
    // A file in scope no link names at all.
    let (codes, _) = run("scope file \"README.md\"\n\n", &link, &["src/pay.py", "README.md"]);
    assert_eq!(codes, ["E404"]);
    // A path of the scope that is not there.
    let (codes, _) = run("scope file \"lib\"\n\n", &link, &["src/pay.py"]);
    assert_eq!(codes, ["E201"]);
    // The summary says how many files trace.
    let (_, out) = run("scope file \"src\"\n\n", &format!("{REQ}  satisfied by file \"src/pay.py\"\n  verified by file \"src/report.py\"\n"), &["src/pay.py", "src/report.py"]);
    assert!(out.ends_with("the 2 files in scope all trace to a requirement\n"), "{out}");
}

#[test]
fn what_traces_by_containing_in_english() {
    // A link that names a file reaches what is in it; one that names something in a file
    // reaches the file (DESIGN 5.3, 2 and 3).
    let file = parse_one("rulec \"x.rule\"").unwrap();
    let output = parse_one("rulec \"x.rule\" output postage").unwrap();
    let other = parse_one("rulec \"y.rule\" output postage").unwrap();
    assert!(yuen::coverage::traced(&output, &[&file]));
    assert!(yuen::coverage::traced(&file, &[&output]));
    assert!(!yuen::coverage::traced(&file, &[&other]));
    let value = parse_one("proto \"o.proto\" enum E value V").unwrap();
    let parent = parse_one("proto \"o.proto\" enum E").unwrap();
    assert!(yuen::coverage::traced(&value, &[&parent]));
}
