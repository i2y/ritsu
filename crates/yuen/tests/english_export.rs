//! `yuen export` (DESIGN 12, 13), in English: the twin of the last test of export.rs. The
//! projects of the English fixtures are in export.rs's list, which every export test walks.

mod common;

use common::TempDir;

/// The command with every language joined, as `ritsu yuen` runs it.
fn run(args: &[&str]) -> common::Ran {
    common::run(args)
}

fn export(what: &str, dir: &str, more: &[&str]) -> String {
    let mut args = vec!["export", what, dir, "--root", dir];
    args.extend_from_slice(more);
    let r = run(&args);
    assert_eq!(r.code, 0, "yuen {}: {}", args.join(" "), r.stderr);
    r.stdout
}

#[test]
fn what_stops_an_export_and_what_does_not_in_english() {
    // An error of the first four stages: nothing is written.
    let m = "tests/mutants/E101_copy_missing";
    for what in ["reqif", "prov"] {
        let r = run(&["export", what, m, "--root", m]);
        assert_eq!(r.code, 1, "{what}: {}", r.stderr);
        assert!(r.stdout.is_empty(), "{what}");
        assert!(r.stderr.contains("error[E101]"), "{what}: {}", r.stderr);
    }
    // A mark is written as the state of the relation.
    let m = "tests/mutants/E302_article_changed";
    let xml = export("reqif", m, &[]);
    assert!(xml.contains("THE-VALUE=\"up_changed\""), "the mark is in the ReqIF");
    let n = export("prov", m, &[]);
    assert!(n.contains("yuen:status=\"up_changed\""), "the mark is in PROV");
    // A project that writes down no day: ReqIF needs --time; PROV does not.
    let t = TempDir::new("notime");
    t.write("t.req", b"requirements t v1\nrole accounting\n\nrequirement r1\n  text \"x\"\n  owner accounting\n  from r0\n  not satisfied \"y\"\n  not verified \"z\"\n\nrequirement r0\n  text \"w\"\n  owner accounting\n  from r1\n  not satisfied \"y\"\n  not verified \"z\"\n");
    let r = common::yuen(t.path(), &["export", "reqif", ".", "--root", "."]);
    assert_eq!(r.code, 2, "{}", r.stdout);
    assert!(r.stderr.contains("--time 2026-10-03T00:00:00Z"), "{}", r.stderr);
    let r = common::yuen(t.path(), &["export", "reqif", ".", "--root", ".", "--time", "2026-10-03T09:00:00+09:00"]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(r.stdout.contains("<CREATION-TIME>2026-10-03T09:00:00+09:00</CREATION-TIME>"));
    assert!(r.stdout.contains("THE-VALUE=\"unreadable\""), "the links of a cycle (E405), whose ends cannot be made, are written as states");
    assert_eq!(common::yuen(t.path(), &["export", "prov", ".", "--root", "."]).code, 0);
    // The flags of one form are refused with the other.
    let p = "tests/fixtures/period_of_months";
    assert_eq!(run(&["export", "reqif", p, "--format", "json"]).code, 2);
    assert_eq!(run(&["export", "prov", p, "--time", "2026-10-03T00:00:00Z"]).code, 2);
    assert_eq!(run(&["export", "reqif", p, "--time", "2026-10-03"]).code, 2);
    // A value the string type cannot hold, and a character XML cannot hold.
    let t = TempDir::new("long");
    let long = "a".repeat(100_001);
    t.write("t.req", format!("requirements t v1\ndescription \"{long}\"\nrole accounting\n\nrequirement r1\n  text \"x\"\n  owner accounting\n  decided 2026-10-03 by accounting \"y\"\n  not satisfied \"y\"\n  not verified \"z\"\n").as_bytes());
    let r = common::yuen(t.path(), &["export", "reqif", ".", "--root", "."]);
    assert_eq!(r.code, 2, "{}", r.stderr);
    assert!(r.stderr.contains("100,001 characters, more than the 100,000"), "{}", r.stderr);
    let t = TempDir::new("ctrl");
    t.write("t.req", "requirements t v1\nrole accounting\n\nrequirement r1\n  text \"x\u{1}y\"\n  owner accounting\n  decided 2026-10-03 by accounting \"y\"\n  not satisfied \"y\"\n  not verified \"z\"\n".as_bytes());
    let r = common::yuen(t.path(), &["export", "reqif", ".", "--root", "."]);
    assert_eq!(r.code, 2, "{}", r.stderr);
    assert!(r.stderr.contains("U+0001"), "{}", r.stderr);
    // --out writes the file and says so.
    let t = TempDir::new("out");
    let out = t.path().join("period_of_months.reqif");
    let r = run(&["export", "reqif", p, "--root", p, "--out", out.to_str().unwrap()]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert_eq!(r.stdout, format!("wrote: {}\n", out.display()));
    assert_eq!(std::fs::read_to_string(&out).unwrap(), export("reqif", p, &[]));
}
