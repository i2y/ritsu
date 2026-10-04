//! `yuen trace` (DESIGN 9, PLAN B.11): from the three requirements of the `period` fixture,
//! from its `.cal`, and from the 142nd article, in English, in Japanese and as JSON, against
//! the golden files in `tests/golden/trace/`.

mod common;

use std::path::Path;

#[test]
fn the_traces_of_the_period_fixture() {
    let starts: &[(&str, &[&str])] = &[
        ("起算日", &["--requirement", "起算日"]),
        ("満了日", &["--requirement", "last_day"]),
        ("満了日_142条", &["--requirement", "満了日_142条"]),
        ("cal", &["--artifact", "file \"民法の期間.cal\""]),
        ("142条", &["--source", "@民法 第142条"]),
    ];
    let mut failures = Vec::new();
    for (name, how) in starts {
        for (tag, extra) in [("en", vec![]), ("ja", vec!["--lang", "ja"]), ("json", vec!["--format", "json"])] {
            let mut args = vec!["trace", "tests/fixtures/period", "--root", "tests/fixtures/period"];
            args.extend_from_slice(how);
            args.extend(extra);
            let r = common::yuen(Path::new("."), &args);
            assert_eq!(r.code, 0, "{name}: {}", r.stderr);
            let ext = if tag == "json" { "json" } else { &format!("{tag}.txt") };
            common::golden(&format!("tests/golden/trace/{name}.{ext}"), &r.stdout, &mut failures);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_trace_quotes_the_copy_and_says_the_marks() {
    let t = common::fixture("period");
    common::change_142(&t.path().join("period"));
    let r = common::yuen(t.path(), &["trace", "period", "--root", "period", "--requirement", "満了日_142条"]);
    assert_eq!(r.code, 0);
    assert!(r.stdout.contains("> 第百四十二条　期間の末日が日曜日"), "{}", r.stdout);
    assert!(r.stdout.contains("その翌々日に満了する。"), "the copy as it is now: {}", r.stdout);
    assert!(r.stdout.contains("looked at by 法務 on 2026-10-03; the upper end changed since"), "{}", r.stdout);
}

#[test]
fn what_trace_refuses() {
    let p = Path::new(".");
    let base = ["trace", "tests/fixtures/period", "--root", "tests/fixtures/period"];
    let run = |more: &[&str]| {
        let mut a = base.to_vec();
        a.extend_from_slice(more);
        common::yuen(p, &a).code
    };
    assert_eq!(run(&[]), 2, "one of the three is needed");
    assert_eq!(run(&["--requirement", "起算日", "--source", "@民法 第140条"]), 2, "only one");
    assert_eq!(run(&["--requirement", "無い要件"]), 2);
    assert_eq!(run(&["--artifact", "file \"無い.txt\""]), 2);
    assert_eq!(run(&["--artifact", "excel \"a.xlsx\""]), 2);
    assert_eq!(run(&["--source", "@商法 第1条"]), 2);
}

#[test]
fn a_requirement_read_from_another_is_traced_back() {
    let t = common::TempDir::new("trace-chain");
    t.write(
        "a.req",
        "requirements a v1\nrole 経理\n\nrequirement 方針(policy)\n  text \"締め日ごとにまとめて払う\"\n  owner 経理\n  decided 2026-10-01 by 経理 \"例として決めた\"\n  decided 2026-09-01 by 経理 \"前に決めたこと\"\n  not satisfied \"例\"\n  not verified \"例\"\n\nrequirement 支払日(pay)\n  text \"20 日締め翌月 10 日払い\"\n  owner 経理\n  from 方針\n  not satisfied \"例\"\n  not verified \"例\"\n"
            .as_bytes(),
    );
    let r = common::yuen(t.path(), &["trace", ".", "--root", ".", "--requirement", "pay"]);
    assert_eq!(r.code, 0);
    let out = &r.stdout;
    let at = |s: &str| out.find(s).unwrap_or_else(|| panic!("{s} not in\n{out}"));
    assert!(at("comes from the requirement 方針") < at("方針 (a.req:4)"));
    // The decisions in the order of their dates.
    assert!(at("2026-09-01 経理: 前に決めたこと") < at("2026-10-01 経理: 例として決めた"));
}
