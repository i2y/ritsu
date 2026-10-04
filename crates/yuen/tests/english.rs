//! The English twins of the tests that read the Japanese materials: the fixture
//! `period_of_months` beside `period` (37 CFR 1.6, 1.7, 1.8 and 1.10 of the eCFR in the place of
//! articles 140 to 143 of the Civil Code, and koyomi's `period_of_months.cal` in the place of
//! `民法の期間.cal`), the fixtures `payment_policy`, `calendar_sources`, `refunds_book`,
//! `delivery_flow`, `warehouse_proto` and `ordering_terms` beside `payment`, `koyomi`, `chobo`,
//! `dandori`, `proto` and `sakai`, and the requirements the tests write themselves in English.
//! Each test checks what its Japanese twin does (the same marks, codes and exit codes) with the
//! names and the hashes of the English material; the Japanese tests stay as they are, in the
//! files of the same names (DESIGN 16.2).

mod common;

use common::{TempDir, edit};
use std::path::Path;

const REQ: &str = "period_of_months.req";
const CAL: &str = "period_of_months.cal";
const FIXTURE: &str = "tests/fixtures/period_of_months";

fn fixture() -> TempDir {
    common::fixture("period_of_months")
}

use common::{change_cal_en as change_cal, change_section, change_text_en as change_text};

// ── api ──

#[test]
fn the_api_of_the_period_of_months_fixture() {
    let r = common::yuen(Path::new("."), &["api", FIXTURE, "--root", FIXTURE]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    let mut failures = Vec::new();
    common::golden("tests/golden/api/period_of_months.json", &r.stdout, &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["yuen", "root", "files", "roles", "sources", "requirements", "artifacts", "scopes", "check"]);
    let req = &v["requirements"][2];
    let keys: Vec<&String> = req.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["name", "alias", "version", "file", "line", "text", "in_force", "owner", "replaces", "sha256", "from", "decided", "links", "waivers"]);
    assert_eq!(req["name"], "next_business_day");
    assert_eq!(req["sha256"], "8d9c6066ea40430f");
    assert_eq!(req["from"][0]["status"], "ok");
    assert_eq!(req["links"][0]["artifact"]["text"], "file \"period_of_months.cal\"");
    assert_eq!(req["links"][0]["sha256"], "0f1a06d9b71a39f6");
    assert_eq!(v["scopes"][0]["artifacts"], 1);
    assert_eq!(v["check"]["ok"], true);
}

#[test]
fn the_api_carries_the_marks_as_states_in_english() {
    let t = fixture();
    change_section(&t.path().join("period_of_months"));
    let r = common::yuen(t.path(), &["api", "period_of_months", "--root", "period_of_months"]);
    assert_eq!(r.code, 0, "marks do not stop the api");
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    let req = &v["requirements"][2];
    assert_eq!(req["from"][0]["status"], "up_changed");
    assert_eq!(req["links"][0]["status"], "up_changed");
    assert_eq!(req["waivers"][0]["status"], "up_changed");
    assert_eq!(v["requirements"][0]["from"][0]["status"], "ok");
    assert_eq!(v["check"]["ok"], false);
    assert_eq!(v["check"]["diagnostics"][0]["code"], "E302");
}

#[test]
fn no_api_for_a_project_whose_names_are_wrong_in_english() {
    let r = common::yuen(Path::new("."), &["api", "tests/mutants/E008_undeclared_role"]);
    assert_eq!(r.code, 1);
    assert!(r.stdout.is_empty());
    assert!(r.stderr.contains("error[E008]"), "{}", r.stderr);
}

/// The twin of `the_api_of_the_fixtures_that_read_another_language`: the English fixtures that
/// name the things of another language, read through ritsu's ports.
#[test]
fn the_api_of_the_english_fixtures_that_read_another_language() {
    let mut failures = Vec::new();
    for name in ["calendar_sources", "refunds_book", "delivery_flow", "warehouse_proto", "ordering_terms", "payment_policy"] {
        let dir = format!("tests/fixtures/{name}");
        let r = common::run(&["api", &dir, "--root", &dir]);
        assert_eq!(r.code, 0, "{name}: {}", r.stderr);
        common::golden(&format!("tests/golden/api/{name}.json"), &r.stdout, &mut failures);
        let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
        assert_eq!(v["check"]["ok"], true, "{name}");
        for a in v["artifacts"].as_array().unwrap() {
            assert_eq!(a["end"], if a["items"].as_array().unwrap().is_empty() { "file" } else { "item" }, "{name}: {a}");
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// ── ends ──

fn period() -> yuen::check::Checked {
    let p = FIXTURE.to_string();
    yuen::check::check(std::slice::from_ref(&p), Some(&p)).unwrap()
}

#[test]
fn the_ends_of_the_period_of_months_fixture() {
    let c = period();
    let p = c.project.as_ref().unwrap();
    let m = c.model.as_ref().unwrap();
    let hash = |name: &str| {
        let r = p.find_req(name).unwrap()[0];
        m.req_ends[r].as_ref().unwrap().hash.clone()
    };
    assert_eq!(hash("date_of_receipt"), "c6a57f069e4638f2");
    assert_eq!(hash("timely_filing"), "8d02c6a1f0ad3364");
    assert_eq!(hash("next_business_day"), "8d9c6066ea40430f");
    let cal = m.artifacts.iter().find(|(n, _)| n.path == CAL).unwrap().1.as_ref().unwrap();
    assert_eq!(cal.hash, "0f1a06d9b71a39f6");
}

#[test]
fn the_end_of_a_requirement_is_what_design_4_1_shows_in_english() {
    let c = period();
    let p = c.project.as_ref().unwrap();
    let m = c.model.as_ref().unwrap();
    let r = p.find_req("date_of_receipt").unwrap()[0];
    let end = String::from_utf8(m.req_ends[r].as_ref().unwrap().bytes.clone()).unwrap();
    let design = std::fs::read_to_string("DESIGN.md").unwrap();
    let a = design.find("`date_of_receipt` の端の中身は次の 3 行").unwrap();
    let block = &design[a..];
    let start = block.find("```\n").unwrap() + 4;
    let stop = start + block[start..].find("```").unwrap();
    assert_eq!(end, &block[start..stop]);
    assert_eq!(end.lines().count(), 3);
}

#[test]
fn the_end_carries_a_requirement_it_is_read_from_in_english() {
    let t = TempDir::new("ends-en");
    t.write(
        "a.req",
        "requirements a v1\nrole accounting\n\nrequirement policy\n  text \"x\"\n  owner accounting\n  decided 2026-10-03 by accounting \"example\"\n\nrequirement pay\n  text \"y\"\n  in force 2026-10-01..\n  owner accounting\n  from policy\n",
    );
    let path = t.path().to_string_lossy().to_string();
    let c = yuen::check::check(std::slice::from_ref(&path), Some(&path)).unwrap();
    let p = c.project.as_ref().unwrap();
    let m = c.model.as_ref().unwrap();
    let policy = m.req_ends[p.find_req("policy").unwrap()[0]].as_ref().unwrap();
    assert_eq!(String::from_utf8(policy.bytes.clone()).unwrap(), "text x\n");
    let pay = m.req_ends[p.find_req("pay").unwrap()[0]].as_ref().unwrap();
    assert_eq!(String::from_utf8(pay.bytes.clone()).unwrap(), format!("text y\nfrom requirement policy v1 sha256:{}\nin force 2026-10-01..\n", policy.hash));
}

// ── marks ──

fn drop_records(d: &Path) {
    edit(d, REQ, "    reviewed 2026-10-03 by development sha256:c6a57f069e4638f2 -> sha256:0f1a06d9b71a39f6\n", "");
    edit(d, REQ, "    approved 2026-10-03 by legal sha256:8d02c6a1f0ad3364\n", "");
}

fn without_reviewed(d: &Path) {
    std::fs::remove_dir_all(d.join("reviewed")).unwrap();
    change_section(d);
}

#[allow(clippy::type_complexity)]
fn cases() -> Vec<(&'static str, fn(&Path), Vec<&'static str>)> {
    vec![
        ("1_section_text", change_section, vec!["E302", "E302", "E304"]),
        ("2_requirement_text", change_text, vec!["E303", "E302", "E304"]),
        ("3_a_line_of_the_cal", change_cal, vec!["E303", "E303", "E303"]),
        ("4_drop_a_record_and_an_approval", drop_records, vec!["E301", "E304"]),
        ("5_without_reviewed", without_reviewed, vec!["E302", "W301", "E302", "W301", "E304"]),
    ]
}

#[test]
fn the_five_changes_in_english() {
    let mut failures = Vec::new();
    for (name, change, want) in cases() {
        let t = fixture();
        change(&t.path().join("period_of_months"));
        let en = common::yuen(t.path(), &["check", "period_of_months", "--root", "period_of_months"]);
        let ja = common::yuen(t.path(), &["check", "period_of_months", "--root", "period_of_months", "--lang", "ja"]);
        assert_eq!(en.code, 1, "{name}");
        let got = common::codes(&en.stdout);
        if got != want {
            failures.push(format!("{name}: want {want:?}, got {got:?}\n{}", en.stdout));
        }
        common::golden(&format!("tests/golden/marks/{name}.en.txt"), &en.stdout, &mut failures);
        common::golden(&format!("tests/golden/marks/{name}.ja.txt"), &ja.stdout, &mut failures);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_diff_is_shown_once_in_english() {
    let t = fixture();
    change_section(&t.path().join("period_of_months"));
    let r = common::yuen(t.path(), &["check", "period_of_months", "--root", "period_of_months"]);
    let blocks: Vec<&str> = r.stdout.split("\nerror[").collect();
    assert!(blocks[0].contains("+ (a) Whenever periods of time are specified") && blocks[0].contains("second succeeding"), "{}", blocks[0]);
    assert!(!blocks[1].contains("@@"), "{}", blocks[1]);
    let t = fixture();
    change_cal(&t.path().join("period_of_months"));
    let r = common::yuen(t.path(), &["check", "period_of_months", "--root", "period_of_months"]);
    assert_eq!(r.stdout.matches("@@ -").count(), 1, "{}", r.stdout);
    assert_eq!(r.stdout.matches("The same change as at period_of_months/period_of_months.req:21").count(), 2, "{}", r.stdout);
    let t = fixture();
    without_reviewed(&t.path().join("period_of_months"));
    let r = common::yuen(t.path(), &["check", "period_of_months", "--root", "period_of_months"]);
    assert!(!r.stdout.contains("@@"), "{}", r.stdout);
}

#[test]
fn what_is_not_text_in_english() {
    let t = TempDir::new("binary-en");
    t.write("a.bin", &[0u8, 159, 146, 150]);
    t.write("t.req", b"requirements t v1\nrole accounting\n\nrequirement r1\n  text \"x\"\n  owner accounting\n  decided 2026-10-03 by accounting \"y\"\n  satisfied by file \"a.bin\"\n  not verified \"z\"\n");
    let r = common::yuen(t.path(), &["review", ".", "--root", ".", "--all", "--by", "accounting", "--date", "2026-10-03"]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    let kept: Vec<String> = std::fs::read_dir(t.path().join("reviewed")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
    assert_eq!(kept.len(), 1, "{kept:?}");
    t.write("a.bin", &[0u8, 159, 146, 151]);
    let r = common::yuen(t.path(), &["check", ".", "--root", "."]);
    assert_eq!(common::codes(&r.stdout), ["E303", "W301"], "{}", r.stdout);
}

// ── trace ──

#[test]
fn the_traces_of_the_period_of_months_fixture() {
    let starts: &[(&str, &[&str])] = &[
        ("date_of_receipt", &["--requirement", "date_of_receipt"]),
        ("timely_filing", &["--requirement", "timely_filing"]),
        ("next_business_day", &["--requirement", "next_business_day"]),
        ("period_cal", &["--artifact", "file \"period_of_months.cal\""]),
        ("section_1_7", &["--source", "@cfr \"§1.7\""]),
    ];
    let mut failures = Vec::new();
    for (name, how) in starts {
        for (tag, extra) in [("en", vec![]), ("ja", vec!["--lang", "ja"]), ("json", vec!["--format", "json"])] {
            let mut args = vec!["trace", FIXTURE, "--root", FIXTURE];
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
fn a_trace_quotes_the_copy_and_says_the_marks_in_english() {
    let t = fixture();
    change_section(&t.path().join("period_of_months"));
    let r = common::yuen(t.path(), &["trace", "period_of_months", "--root", "period_of_months", "--requirement", "next_business_day"]);
    assert_eq!(r.code, 0);
    assert!(r.stdout.contains("> § 1.7 Times for taking action; Expiration on Saturday, Sunday or Federal holiday."), "{}", r.stdout);
    assert!(r.stdout.contains("on the second succeeding business day which is not a Saturday"), "the copy as it is now: {}", r.stdout);
    assert!(r.stdout.contains("looked at by legal on 2026-10-03; the upper end changed since"), "{}", r.stdout);
}

#[test]
fn what_trace_refuses_in_english() {
    let p = Path::new(".");
    let base = ["trace", FIXTURE, "--root", FIXTURE];
    let run = |more: &[&str]| {
        let mut a = base.to_vec();
        a.extend_from_slice(more);
        common::yuen(p, &a).code
    };
    assert_eq!(run(&[]), 2, "one of the three is needed");
    assert_eq!(run(&["--requirement", "date_of_receipt", "--source", "@cfr \"§1.6\""]), 2, "only one");
    assert_eq!(run(&["--requirement", "no_such_requirement"]), 2);
    assert_eq!(run(&["--artifact", "file \"no_such.txt\""]), 2);
    assert_eq!(run(&["--artifact", "excel \"a.xlsx\""]), 2);
    assert_eq!(run(&["--source", "@ucc \"§1.1\""]), 2);
}

#[test]
fn a_requirement_read_from_another_is_traced_back_in_english() {
    let t = TempDir::new("trace-chain-en");
    t.write(
        "a.req",
        "requirements a v1\nrole accounting\n\nrequirement policy\n  text \"Pays together at each closing day\"\n  owner accounting\n  decided 2026-10-01 by accounting \"decided for the example\"\n  decided 2026-09-01 by accounting \"decided earlier\"\n  not satisfied \"example\"\n  not verified \"example\"\n\nrequirement pay\n  text \"Closes on the 20th, pays on the 10th of the next month\"\n  owner accounting\n  from policy\n  not satisfied \"example\"\n  not verified \"example\"\n"
            .as_bytes(),
    );
    let r = common::yuen(t.path(), &["trace", ".", "--root", ".", "--requirement", "pay"]);
    assert_eq!(r.code, 0);
    let out = &r.stdout;
    let at = |s: &str| out.find(s).unwrap_or_else(|| panic!("{s} not in\n{out}"));
    assert!(at("comes from the requirement policy") < at("policy (a.req:4)"));
    assert!(at("2026-09-01 accounting: decided earlier") < at("2026-10-01 accounting: decided for the example"));
}

// ── DESIGN 1.1 ──

/// The twin of `the_file_of_design_1_1_reads_as_it_says`: the English file of DESIGN 1.1.
#[test]
fn the_file_of_design_1_1_reads_as_it_says_in_english() {
    use yuen::ast::*;
    let design = std::fs::read_to_string("DESIGN.md").unwrap();
    let a = design.find("### 1.1 ファイルの形").unwrap();
    let b = a + design[a..].find("### 1.2").unwrap();
    let mut blocks = Vec::new();
    let mut cur: Option<String> = None;
    for l in design[a..b].lines() {
        if l.starts_with("```") {
            match cur.take() {
                Some(x) => blocks.push(x),
                None => cur = Some(String::new()),
            }
        } else if let Some(x) = cur.as_mut() {
            x.push_str(l);
            x.push('\n');
        }
    }
    let src = blocks.into_iter().find(|b| b.starts_with("requirements period_of_months v1")).unwrap();
    let f = yuen::parse::parse("x.req", "x.req", &src).file.unwrap();
    assert_eq!((f.header.name.as_str(), f.header.version), ("period_of_months", 1));
    assert_eq!(f.roles.len(), 2);
    assert!(matches!(&f.sources[0].kind, SourceKind::Law { db: LawDb::Ecfr, .. }));
    assert_eq!(f.scopes.len(), 1);
    assert_eq!(f.requirements.len(), 2);
    let r = &f.requirements[1];
    assert_eq!(r.name, "timely_filing");
    let FromWhat::Cite { source, fragments, .. } = &r.from[0].what else { panic!() };
    assert_eq!(source, "cfr");
    assert_eq!(fragments.iter().map(|x| x.0.as_str()).collect::<Vec<_>>(), ["§1.8", "§1.10"]);
    let rec = r.from[0].record.as_ref().unwrap().parsed.as_ref().unwrap();
    assert_eq!(rec.up, ["fa698e3ea7cb1e49", "5adcbc193371fd26"]);
    assert_eq!(rec.down.as_deref(), Some("8d02c6a1f0ad3364"));
    assert_eq!(r.waivers.len(), 1);
    assert_eq!(r.waivers[0].side, Side::Verified);
}
