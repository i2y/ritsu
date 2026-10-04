//! Words and lines (DESIGN 1), in English: the twin of syntax.rs (the blocks of DESIGN.md are read by syntax.rs and by the twin of its 1.1 test in english.rs).
//!
//! Written by tests/ twins: the Japanese file keeps its tests; these check the same behavior in English.


use yuen::ast::*;
use yuen::parse::parse;

fn code_of(src: &str) -> Vec<&'static str> {
    parse("t.req", "t.req", src).diags.iter().map(|d| d.code).collect()
}

const HEAD: &str = "requirements trial v1\nrole accounting\n\n";

#[test]
fn what_is_wrong_with_words_and_lines_in_english() {
    let cases: &[(&str, &str)] = &[
        // E001: words.
        ("requirement r1\n  text \"open\n", "E001"),
        ("requirement r1\n  text \"a\\nb\"\n", "E001"),
        ("requirement r1(Pay)\n", "E001"),
        ("requirement 30days\n", "E001"),
        ("requirement r1\n  in force 2026-1-1..\n", "E001"),
        ("requirement r1 v01\n", "E001"),
        ("requirement r1\n  from @cfr \"§1.1\"\n    reviewed 2026-10-03 by accounting sha256:ABC -> sha256:abc\n", "E001"),
        // E002: a word where it does not belong.
        ("requirement r1\n  because \"x\"\n", "E002"),
        ("role text\n", "E002"),
        ("requirement r1\n  owner\n", "E002"),
        ("source cfr = law \"x\"\n", "E002"),
        ("requirement r1\n  in force ..\n", "E002"),
        ("requirement r1\n  satisfied by\n", "E002"),
        ("hello\n", "E002"),
        // E004: order and count.
        ("requirement r1\n  owner accounting\n  text \"x\"\n", "E004"),
        ("requirement r1\n  text \"x\"\n  text \"y\"\n", "E004"),
        ("requirement r1\n  satisfied by file \"a\"\n    reviewed 2026-10-03 by accounting sha256:0000000000000000 -> sha256:0000000000000000\n    reviewed 2026-10-03 by accounting sha256:0000000000000000 -> sha256:0000000000000000\n", "E004"),
        ("description \"x\"\n", "E004"),
        // E005: indentation.
        ("requirement r1\n\ttext \"x\"\n", "E005"),
        ("requirement r1\n  text \"x\"\n   owner accounting\n", "E005"),
        ("  text \"x\"\n", "E005"),
        ("requirement r1\n  text \"x\"\n    reviewed 2026-10-03 by accounting sha256:0000000000000000 -> sha256:0000000000000000\n", "E005"),
        // E006: dates.
        ("requirement r1\n  in force 2026-02-30..\n", "E006"),
        ("requirement r1\n  in force 2027-01-01..2026-01-01\n", "E006"),
    ];
    for (body, code) in cases {
        let src = format!("{HEAD}{body}");
        let got = code_of(&src);
        assert_eq!(got.first(), Some(code), "{src:?} gave {got:?}");
    }
    assert_eq!(code_of("role accounting\n"), ["E003"]);
    assert_eq!(code_of(""), ["E003"]);
    assert_eq!(code_of("requirements trial\n"), ["E002"]);
}

#[test]
fn a_broken_record_is_said_later_and_does_not_stop_the_words_in_english() {
    // The parser keeps what is wrong with a record for stage 6 (E305): the file parses.
    let src = format!("{HEAD}requirement r1\n  text \"x\"\n  satisfied by file \"a\"\n    reviewed 2026-10-03 by accounting sha256:0000000000000000\n  not verified \"y\"\n    reviewed 2026-10-03 by accounting sha256:0000000000000000 -> sha256:0000000000000000\n");
    let p = parse("t.req", "t.req", &src);
    assert!(p.diags.is_empty());
    let r = &p.file.unwrap().requirements[0];
    assert!(r.links[0].record.as_ref().unwrap().parsed.is_err());
    assert!(r.waivers[0].record.as_ref().unwrap().parsed.is_err(), "a waiver's record is `approved`");
}

#[test]
fn periods_aliases_versions_and_comments_in_english() {
    let src = format!(
        "{HEAD}requirement payment_day v2  # the second\n  text \"Closes on the 20th # pays on the 10th of the next month\"\n  in force ..2027-03-31\n  owner accounting\n  replaces old_payment_day v3\n  from policy\n"
    );
    let f = parse("t.req", "t.req", &src).file.unwrap();
    let r = &f.requirements[0];
    assert_eq!(r.version.unwrap().0, 2);
    assert_eq!(r.text.as_ref().unwrap().0, "Closes on the 20th # pays on the 10th of the next month");
    let p = r.in_force.unwrap().0;
    assert_eq!((p.from, p.to.map(|d| d.to_string())), (None, Some("2027-03-31".into())));
    assert_eq!(r.replaces[0].version, Some(3));
    assert!(matches!(&r.from[0].what, FromWhat::Req(rr) if rr.name == "policy" && rr.version.is_none()));
}

#[test]
fn crlf_reads_as_lf_in_english() {
    let src = format!("{HEAD}requirement r1\n  text \"x\"\n  owner accounting\n").replace('\n', "\r\n");
    let p = parse("t.req", "t.req", &src);
    assert!(p.diags.is_empty());
    assert_eq!(p.file.unwrap().requirements[0].text.as_ref().unwrap().0, "x");
}
