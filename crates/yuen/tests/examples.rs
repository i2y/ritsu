//! The examples (DESIGN 15, PLAN D.2), checked with every language joined, as `ritsu yuen`
//! checks them. Every example passes but `civil_code_periods_reread`, which stops on purpose on
//! the one link whose date was read another way after it was looked at, and
//! `openspec_greeter_archived`, which stops on the requirement of the OpenSpec spec a change
//! modified when it was archived; no example has a link left to look at; and the records of geas
//! in `greeter` are what `geas map` writes.

mod common;

use std::path::Path;

const REREAD: &str = "civil_code_periods_reread";
const ARCHIVED: &str = "openspec_greeter_archived";

#[test]
fn every_example_passes_but_the_one_that_stops_on_purpose() {
    let mut failures = Vec::new();
    for (ex, reqs, _) in common::EXAMPLES {
        let dir = format!("examples/{ex}");
        for req in *reqs {
            let path = format!("{dir}/{req}");
            let r = common::run(&["check", &path, "--root", &dir]);
            let codes = common::codes(&r.stdout);
            if *ex == REREAD {
                if r.code != 1 || codes != ["E303"] {
                    failures.push(format!("{path}: exit {}, codes {codes:?} (one E303 expected)\n{}", r.code, r.stdout));
                }
                // the diff is the line that was read another way
                for want in ["- if closed + 1 day", "+ roll following", "date 満了日_142条"] {
                    if !r.stdout.contains(want) {
                        failures.push(format!("{path}: no {want:?} in\n{}", r.stdout));
                    }
                }
            } else if *ex == ARCHIVED {
                // the pin of the requirement the change modified, the requirement it added, and the
                // scenario it added, which no claim answers yet
                if r.code != 1 || codes != ["W102", "E103", "W402"] {
                    failures.push(format!("{path}: exit {}, codes {codes:?} (W102, E103 and W402 expected)\n{}", r.code, r.stdout));
                }
                for want in ["#### Scenario: rejects a name of spaces", "#### Scenario: 空白だけの名前は受け付けない"] {
                    if !r.stdout.contains(want) && path.contains(if want.is_ascii() { "greeter.req" } else { "greeter.ja.req" }) {
                        failures.push(format!("{path}: no {want:?} in\n{}", r.stdout));
                    }
                }
            } else if r.code != 0 || !codes.is_empty() {
                failures.push(format!("{path}: exit {}, codes {codes:?}\n{}{}", r.code, r.stdout, r.stderr));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The two copies of the civil code example are the same `.req` and the same records: only the
/// `.cal` differs, by one line.
#[test]
fn the_reread_example_differs_by_one_line_of_the_calendar() {
    let a = std::fs::read("examples/civil_code_periods/civil_code_periods.ja.req").unwrap();
    let b = std::fs::read("examples/civil_code_periods_reread/civil_code_periods_reread.ja.req").unwrap();
    assert_eq!(a, b, "the .req of the two examples differ");
    let names = |d: &str| -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(d).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
        v.sort();
        v
    };
    let (ra, rb) = (names("examples/civil_code_periods/reviewed"), names("examples/civil_code_periods_reread/reviewed"));
    assert_eq!(ra, rb);
    for n in &ra {
        assert_eq!(std::fs::read(format!("examples/civil_code_periods/reviewed/{n}")).unwrap(), std::fs::read(format!("examples/civil_code_periods_reread/reviewed/{n}")).unwrap(), "reviewed/{n}");
    }
    let ca = std::fs::read_to_string("examples/civil_code_periods/civil_code_period_end.ja.cal").unwrap();
    let cb = std::fs::read_to_string("examples/civil_code_periods_reread/civil_code_period_end.ja.cal").unwrap();
    let differ: Vec<(&str, &str)> = ca.lines().zip(cb.lines()).filter(|(x, y)| x != y).collect();
    assert_eq!(ca.lines().count(), cb.lines().count());
    assert_eq!(differ.len(), 1, "{differ:?}");
    assert!(differ[0].0.contains("if closed + 1 day") && differ[0].1.contains("roll following"), "{differ:?}");
}

/// `review --all` writes nothing: every link and waiver of the examples has been looked at (the
/// one that stops on purpose is left out: its mark is the point of it).
#[test]
fn no_example_has_anything_left_to_look_at() {
    let mut failures = Vec::new();
    for (ex, reqs, role) in common::EXAMPLES {
        if *ex == REREAD || *ex == ARCHIVED {
            continue;
        }
        let t = common::TempDir::new("examples-review");
        let dir = t.path().join(ex);
        common::copy_dir(&Path::new("examples").join(ex), &dir);
        for req in *reqs {
            let file = dir.join(req);
            let before = std::fs::read(&file).unwrap();
            // the role that looks at the artifacts, as this file declares it
            let text = String::from_utf8_lossy(&before).to_string();
            let role = if text.lines().any(|l| l.starts_with(&format!("role {role} "))) { role.to_string() } else { "開発".to_string() };
            let r = common::run(&["review", file.to_str().unwrap(), "--all", "--by", &role, "--date", "2026-10-05", "--root", dir.to_str().unwrap()]);
            let after = std::fs::read(&file).unwrap();
            if r.code != 0 || before != after {
                failures.push(format!("{ex}/{req}: exit {}, changed: {}\n{}{}", r.code, before != after, r.stdout, r.stderr));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The records of geas in `greeter` (before the change and after it) are what `geas map` writes
/// over a copy of the example, and the change is the diff the example keeps.
#[test]
fn the_records_of_greeter_are_what_geas_map_writes() {
    if !ritsu_testkit::need(ritsu_testkit::Need::Python) {
        return;
    }
    if ritsu_testkit::tools::on_path("python3").is_none() {
        ritsu_testkit::skip("python3 is not on PATH; geas runs the greeter's server with it");
        return;
    }
    let t = common::TempDir::new("examples-geas");
    let dir = t.path().join("greeter");
    common::copy_dir(Path::new("examples/greeter"), &dir);
    std::fs::remove_dir_all(dir.join(".geas")).unwrap();
    let d = dir.to_str().unwrap().to_string();
    for spec in ["greeter.geas", "greeter.ja.geas"] {
        let code = geas::cli::run(&["map".to_string(), format!("{d}/{spec}"), "--root".to_string(), d.clone()]);
        assert_eq!(code, 0, "geas map {spec}");
        let stem = spec.trim_end_matches(".geas");
        let got = std::fs::read_to_string(dir.join(format!(".geas/{stem}.map.jsonl"))).unwrap();
        let want = std::fs::read_to_string(format!("examples/greeter/.geas/{stem}.map.jsonl")).unwrap();
        assert_eq!(got, want, "the record of {spec}");
    }
    // the change, applied to the copy, and the record after it
    let diff = std::fs::read_to_string("examples/greeter/changes/change.diff").unwrap();
    let minus: Vec<&str> = diff.lines().filter(|l| l.starts_with('-') && !l.starts_with("---")).map(|l| &l[1..]).collect();
    let plus: Vec<&str> = diff.lines().filter(|l| l.starts_with('+') && !l.starts_with("+++")).map(|l| &l[1..]).collect();
    assert_eq!((minus.len(), plus.len()), (1, 1), "the change is one line");
    let server = std::fs::read_to_string(dir.join("server.py")).unwrap();
    assert!(server.contains(minus[0]));
    std::fs::write(dir.join("server.py"), server.replacen(minus[0], plus[0], 1)).unwrap();
    let out = dir.join("after.map.jsonl");
    let code = geas::cli::run(&["map".to_string(), format!("{d}/greeter.geas"), "--root".to_string(), d.clone(), "--out".to_string(), out.to_string_lossy().to_string()]);
    assert_eq!(code, 0, "geas map after the change");
    assert_eq!(std::fs::read_to_string(&out).unwrap(), std::fs::read_to_string("examples/greeter/changes/after.map.jsonl").unwrap(), "the record after the change");
}

/// `affected` over the example's change: the requirements of the line the change touches.
#[test]
fn the_change_of_greeter_reaches_the_requirement_of_an_empty_name() {
    let r = common::run(&[
        "affected",
        "examples/greeter/greeter.req",
        "--root",
        "examples/greeter",
        "--diff",
        "examples/greeter/changes/change.diff",
        "--map",
        "examples/greeter/greeter.geas=examples/greeter/.geas/greeter.map.jsonl",
        "--map",
        "examples/greeter/greeter.geas=examples/greeter/changes/after.map.jsonl",
    ]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    assert!(r.stdout.contains("rejects_an_empty_name"), "{}", r.stdout);
}
