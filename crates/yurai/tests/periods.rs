//! Cycles and periods (DESIGN 5.4, 5.5, PLAN B.6): a gap of one day, an overlap of three, an
//! open end that is not the last, versions out of the order of their periods, what replaces
//! starting a day late; splitting and merging pass; cycles of two and three.

mod common;

use common::{TempDir, yurai};

const HEAD: &str = "requirements 支払 v1\nrole 経理\n\n";

fn version(name: &str, v: u32, period: Option<&str>, extra: &str) -> String {
    let p = period.map(|p| format!("  in force {p}\n")).unwrap_or_default();
    format!("requirement {name} v{v}\n  text \"{name} の v{v}\"\n{p}  owner 経理\n{extra}  decided 2026-10-03 by 経理 \"例\"\n  not satisfied \"例\"\n  not verified \"例\"\n\n")
}

fn single(name: &str, period: &str, extra: &str) -> String {
    format!("requirement {name}\n  text \"{name}\"\n  in force {period}\n  owner 経理\n{extra}  decided 2026-10-03 by 経理 \"例\"\n  not satisfied \"例\"\n  not verified \"例\"\n\n")
}

/// The diagnostics other than the waivers not approved yet (E304), as `code: message` lines.
fn run(body: &str) -> (Vec<String>, String) {
    let t = TempDir::new("periods");
    t.write("支払.req", format!("{HEAD}{body}").as_bytes());
    let r = yurai(t.path(), &["check", ".", "--root", "."]);
    let codes: Vec<String> = common::codes(&r.stdout).into_iter().filter(|c| c != "E304" && c != "E301").collect();
    (codes, r.stdout)
}

#[test]
fn a_gap_of_one_day() {
    let (codes, out) = run(&(version("pay", 1, Some("2026-01-01..2027-03-31"), "") + &version("pay", 2, Some("2027-04-02.."), "")));
    assert_eq!(codes, ["E406"], "{out}");
    assert!(out.contains("2027-04-01, between pay v1 and v2, falls in no version's period"), "{out}");
    assert!(out.contains("The line, fixed: in force 2027-04-01.."), "{out}");
}

#[test]
fn an_overlap_of_three_days() {
    let (codes, out) = run(&(version("pay", 1, Some("2026-01-01..2027-03-31"), "") + &version("pay", 2, Some("2027-03-29.."), "")));
    assert_eq!(codes, ["E407"], "{out}");
    assert!(out.contains("pay v1 and v2 are both in force on 2027-03-29..2027-03-31"), "{out}");
}

#[test]
fn an_open_end_that_is_not_the_last() {
    let (codes, out) = run(&(version("pay", 1, Some("2026-01-01.."), "") + &version("pay", 2, Some("2027-04-01.."), "")));
    assert_eq!(codes, ["E408"], "{out}");
    assert!(out.contains("pay v1 leaves its end open, and it is not the last version"), "{out}");
}

#[test]
fn versions_out_of_the_order_of_their_periods() {
    let (codes, out) = run(&(version("pay", 1, Some("2027-04-01.."), "") + &version("pay", 2, Some("2026-01-01..2027-03-31"), "")));
    assert_eq!(codes, ["E408"], "{out}");
    assert!(out.contains("(v2 comes before v1)"), "{out}");
}

#[test]
fn a_version_without_a_period() {
    let (codes, _) = run(&(version("pay", 1, Some("2026-01-01..2027-03-31"), "") + &version("pay", 2, None, "")));
    assert_eq!(codes, ["E408"]);
}

#[test]
fn what_replaces_starts_a_day_late() {
    let (codes, out) = run(&(single("old", "2026-01-01..2027-03-31", "") + &single("new", "2027-04-02..", "  replaces old\n")));
    assert_eq!(codes, ["E409"], "{out}");
    assert!(out.contains("new replaces old, and does not start on 2027-04-01, the day after old ends (2027-03-31)"), "{out}");
    // Replacing what has no end.
    let (codes, _) = run(&(single("old", "2026-01-01..", "") + &single("new", "2027-04-01..", "  replaces old\n")));
    assert_eq!(codes, ["E409"]);
}

#[test]
fn splitting_and_merging_pass() {
    // One into two.
    let (codes, out) = run(&(single("old", "2026-01-01..2027-03-31", "") + &single("a", "2027-04-01..", "  replaces old\n") + &single("b", "2027-04-01..", "  replaces old\n")));
    assert!(codes.is_empty(), "{out}");
    // Two into one, and the last version of what has versions.
    let (codes, out) = run(&(single("a", "2026-01-01..2027-03-31", "")
        + &version("b", 1, Some("2025-01-01..2025-12-31"), "")
        + &version("b", 2, Some("2026-01-01..2027-03-31"), "")
        + &single("c", "2027-04-01..", "  replaces a\n  replaces b\n")));
    assert!(codes.is_empty(), "{out}");
    // Versions that follow one another pass.
    let (codes, out) = run(&(version("pay", 1, Some("..2027-03-31"), "") + &version("pay", 2, Some("2027-04-01.."), "")));
    assert!(codes.is_empty(), "{out}");
}

#[test]
fn cycles_of_two_and_three() {
    let req = |n: &str, from: &str| format!("requirement {n}\n  text \"{n}\"\n  owner 経理\n  from {from}\n  not satisfied \"例\"\n  not verified \"例\"\n\n");
    let (codes, out) = run(&(req("a", "b") + &req("b", "a")));
    assert_eq!(codes, ["E405"], "{out}");
    assert!(out.contains("The requirements make a cycle: a → b → a"), "{out}");
    let (codes, out) = run(&(req("a", "c") + &req("b", "a") + &req("c", "b")));
    assert_eq!(codes, ["E405"], "{out}");
    assert!(out.contains("a → c → b → a"), "{out}");
    // A requirement read from one in a cycle has no end either: its links are not compared,
    // and nothing else is said about it.
    let (codes, out) = run(&(req("a", "b") + &req("b", "a") + &req("d", "a")));
    assert_eq!(codes, ["E405"], "{out}");
    // A cycle through `replaces` is one too.
    let (codes, out) = run(&(single("a", "2026-01-01..2026-12-31", "  replaces b\n") + &single("b", "2027-01-01..2027-12-31", "  replaces a\n")));
    assert!(codes.contains(&"E405".to_string()), "{out}");
}
