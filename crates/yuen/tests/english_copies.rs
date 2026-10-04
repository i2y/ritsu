//! The copies of the sources (DESIGN 1.4), in English: the twin of copies.rs, on the sections of
//! 37 CFR part 1 the fixture `period_of_months` copies. SHA-256 and base64 are copies.rs's alone;
//! what only e-Gov's articles show (kanji numerals, paragraph and item files, the revision id)
//! stays in copies.rs.

mod common;

use ritsu_base::sha256;
use yuen::ast::LawDb;
use yuen::copies;

const LAW: &str = "tests/fixtures/period_of_months/sources/law/37-CFR-1@2026-01-01";

#[test]
fn the_pins_of_the_copies_of_the_cfr() {
    for (section, pin) in [("1.6", "6bcdc27c3428886c"), ("1.7", "01de176ebe4740d7"), ("1.8", "fa698e3ea7cb1e49"), ("1.10", "5adcbc193371fd26")] {
        let b = std::fs::read(format!("{LAW}/{section}.xml")).unwrap();
        assert_eq!(sha256::short(&b), pin, "§{section}");
        assert!(copies::readable(LawDb::Ecfr, &format!("{section}.xml"), &b).is_ok());
    }
    assert_eq!(copies::revision(std::path::Path::new(LAW)), None, "the eCFR has no revision id");
}

#[test]
fn the_text_of_a_cfr_copy() {
    let xml = std::fs::read_to_string(format!("{LAW}/1.7.xml")).unwrap();
    let text = copies::xml_text(&xml);
    assert!(text.contains("the action may be taken, or the fee paid, on the next succeeding business day which is not a Saturday, Sunday, or a Federal holiday."), "{text}");
    assert!(text.lines().next().unwrap().starts_with("§ 1.7 Times for taking action"), "{text}");
    let lines = copies::article_lines(&xml);
    assert!(lines[0].starts_with("§ 1.7 Times for taking action"), "{}", lines[0]);
    // the eCFR gives a section as one run of text, where e-Gov's article is a line for each paragraph
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains("(b) If the day that is twelve months after the filing date"), "{lines:?}");
}

#[test]
fn the_files_sections_are_copied_into() {
    let e = |s: &str| copies::fragment_file(LawDb::Ecfr, s);
    assert_eq!(e("§1.7").as_deref(), Some("1.7.xml"));
    assert_eq!(e("1.7").as_deref(), Some("1.7.xml"));
    assert_eq!(e("§1.10").as_deref(), Some("1.10.xml"));
    assert_eq!(e("§1910.157").as_deref(), Some("1910.157.xml"));
    assert_eq!(e("1"), None);
    assert_eq!(e("section 1.7"), None);
    assert_eq!(copies::copy_dir("37 CFR 1", "2026-01-01"), "sources/law/37-CFR-1@2026-01-01");
}

/// A change to a copy of the fixture: what it is, what it does, the code it gives.
type Case<'a> = (&'a str, &'a dyn Fn(&std::path::Path), &'a str);

/// What is wrong with a copy or a citation, each in a copy of the fixture changed one way.
#[test]
fn what_is_wrong_with_copies_and_citations_in_english() {
    let copy = "sources/law/37-CFR-1@2026-01-01/1.7.xml";
    let cases: &[Case] = &[
        ("the copy is gone", &|d| std::fs::remove_file(d.join(copy)).unwrap(), "E101"),
        ("no hash on the pin", &|d| common::edit(d, "period_of_months.req", "\"§1.7\" sha256:01de176ebe4740d7", "\"§1.7\""), "E102"),
        ("no pin line", &|d| common::edit(d, "period_of_months.req", "  \"§1.7\" sha256:01de176ebe4740d7\n", ""), "E102"),
        ("the copy changed", &|d| common::edit(d, copy, "next succeeding business day which is not a Saturday", "second succeeding business day which is not a Saturday"), "E103"),
        ("not XML", &|d| std::fs::write(d.join(copy), "x").unwrap(), "E104"),
        ("not the section", &|d| std::fs::write(d.join(copy), std::fs::read_to_string(d.join(copy)).unwrap().replace("DIV8", "DIV5")).unwrap(), "E104"),
        ("a source not declared", &|d| common::edit(d, "period_of_months.req", "from @cfr \"§1.7\"", "from @usc \"§1.7\""), "E105"),
        ("a law cited whole", &|d| common::edit(d, "period_of_months.req", "from @cfr \"§1.7\"", "from @cfr"), "E105"),
        ("a section of no form", &|d| common::edit(d, "period_of_months.req", "from @cfr \"§1.7\"", "from @cfr \"section seven\""), "E105"),
        ("pinned and cited nowhere", &|d| common::edit(d, "period_of_months.req", "  from @cfr \"§1.7\"\n    reviewed 2026-10-03 by legal sha256:01de176ebe4740d7 -> sha256:8d9c6066ea40430f\n", ""), "W101"),
    ];
    for (what, change, code) in cases {
        let t = common::fixture("period_of_months");
        let d = t.path().join("period_of_months");
        change(&d);
        let r = common::yuen(t.path(), &["check", "period_of_months", "--root", "period_of_months"]);
        let got = common::codes(&r.stdout);
        assert!(got.iter().any(|c| c == code), "{what}: want {code}, got {got:?}\n{}", r.stdout);
    }
}

/// A `file` source: the copy pinned whole, cited whole.
#[test]
fn a_file_source_in_english() {
    let t = common::TempDir::new("filesource");
    let terms = "Section 1. A refund is requested within 30 days of purchase.\n";
    t.write("docs/terms.md", terms.as_bytes());
    let pin = sha256::short(terms.as_bytes());
    let src = format!(
        "requirements refund v1\nrole accounting\n\nsource terms = file \"docs/terms.md\" url \"https://example.org/terms.md\" sha256:{pin}\n\nrequirement refund_window\n  text \"A refund is accepted only when requested within 30 days of purchase\"\n  owner accounting\n  from @terms\n  not satisfied \"example\"\n  not verified \"example\"\n"
    );
    t.write("refund.req", src.as_bytes());
    let r = common::yuen(t.path(), &["review", ".", "--root", ".", "--all", "--by", "accounting", "--date", "2026-10-03"]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    let r = common::yuen(t.path(), &["check", ".", "--root", "."]);
    assert_eq!(r.code, 0, "{}", r.stdout);
    let api: serde_json::Value = serde_json::from_str(&common::yuen(t.path(), &["api", ".", "--root", "."]).stdout).unwrap();
    assert_eq!(api["sources"][0]["kind"], "file");
    assert_eq!(api["sources"][0]["path"], "docs/terms.md");
    assert_eq!(api["requirements"][0]["from"][0]["sources"][0]["sha256"], pin.as_str());
    // The requirement's end writes the file's path from the root.
    let end = t.read(&format!("reviewed/{}", api["requirements"][0]["sha256"].as_str().unwrap()));
    assert!(end.contains(&format!("from file docs/terms.md sha256:{pin}\n")), "{end}");
    // Changing the file: E103, and with the pin written again, E302 on the `from`.
    t.write("docs/terms.md", "Section 1. A refund is requested within 14 days of purchase.\n".as_bytes());
    let r = common::yuen(t.path(), &["check", ".", "--root", "."]);
    assert_eq!(common::codes(&r.stdout), ["E103"]);
    // A file source cited with a section is E105.
    t.write("refund.req", src.replace("from @terms", "from @terms \"§1\"").as_bytes());
    let r = common::yuen(t.path(), &["check", ".", "--root", "."]);
    assert!(common::codes(&r.stdout).contains(&"E105".to_string()), "{}", r.stdout);
}
