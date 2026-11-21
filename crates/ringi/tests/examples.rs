//! The examples of `examples/` (PLAN B.7, C2.5, F1): the ten requests, in English and in Japanese names,
//! and the three shared files. With every language joined, as `ritsu ringi check` checks them, and
//! the approvers left unlisted, they pass with nothing to say; listed against the organization's
//! two versions (`examples/org/`), what is said is the organization's data alone (E502–E504, W502,
//! W506, W507, W510), the same in both names, and the golden files hold it with what the shape of
//! the organization does, counted under `ok` (DESIGN 8.6); the contract
//! passes in the version in effect now and fails in the one laid ahead. With no language
//! joined, as the binary of ringi's own crate checks them, the organization and the masters still
//! pass, and a request or a shared route says which languages it needs (E008), once each.

mod common;

use ringi::check::{Options, Run};
use ringi::suite::Suite;
use ritsu_base::text::Lang;

#[test]
fn every_example_passes_with_every_language_joined() {
    let files = common::example_files();
    assert_eq!(files.len(), 13, "{files:?}");
    // one run, as `ritsu ringi check examples/*.ringi examples/shared/*.ringi` makes it: a shared file
    // several requests read is checked once
    let run = Run::with(common::joined(), Options { unlisted: true, ..Options::default() });
    let mut failures = Vec::new();
    for f in &files {
        let o = run.file(f).unwrap();
        if !o.diags.is_empty() || o.summary.is_none() {
            failures.push(format!("{f}:\n{}", common::shown(&o, Lang::En)));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Listed against the organization, every finding is of its data, and an example in Japanese names
/// says what its English version says, at the same places.
#[test]
fn listed_the_examples_say_only_the_organizations_data() {
    let run = Run::new(common::joined());
    let listing = ["E502", "E503", "E504", "W502", "W506", "W507", "W510"];
    for f in common::example_files() {
        let o = run.file(&f).unwrap();
        let other: Vec<&str> = common::codes(&o).into_iter().filter(|c| !listing.contains(c)).collect();
        assert!(other.is_empty(), "{f}: {other:?}");
        let Some(en) = f.strip_suffix(".ja.ringi").map(|s| format!("{s}.ringi")) else { continue };
        let e = run.file(&en).unwrap();
        let at = |o: &ringi::check::Outcome| o.diags.iter().map(|d| (d.code, d.line, d.message.en.split(", ").next().unwrap_or_default().to_string())).collect::<Vec<_>>();
        assert_eq!(at(&o), at(&e), "{f} and {en}");
    }
}

/// What `ringi check` prints for the examples, in both languages: the findings of the organization's
/// data, and the lines that say each example passes, with what its data holds and the days a request
/// can be used on.
#[test]
fn the_examples_say_what_they_hold() {
    let run = Run::new(common::joined());
    for lang in [Lang::En, Lang::Ja] {
        let said: String = common::example_files().iter().map(|f| common::shown(&run.file(f).unwrap(), lang)).collect();
        ritsu_testkit::golden(format!("tests/golden/examples/check.{}.txt", lang.code()), &said);
    }
}

/// The three findings DESIGN 8.6 counted by hand for the version of 2027-04-01, where the first
/// section of sales is gone and its three people are in the first sales department, and the
/// general counsel e9001 has left: the stage `counsel` goes up when it is empty, and cannot go up
/// from someone the version does not have (E503, at the way to choose). The expense request says
/// `same approver approves once`, so e3101 answering both `first` and `decide` is said on the row of
/// a purchase alone, where accounting answers between them (DESIGN 5.12).
#[test]
fn the_findings_the_design_counted() {
    let run = Run::with(common::joined(), Options { at: ritsu_ports::day_of("2027-04-01"), ..Options::default() });
    let notes = |d: &ringi::diag::Diag| d.notes.iter().map(|n| n.en.clone()).collect::<Vec<_>>();
    let contract = run.file("examples/contract.ringi").unwrap();
    let counsel = contract.diags.iter().find(|d| d.code == "E503" && d.line == Some(41)).unwrap_or_else(|| panic!("{}", common::shown(&contract, Lang::En)));
    assert_eq!(counsel.message.en, "In the organization from 2027-04-01, `counsel` names someone who is not there");
    assert_eq!(notes(counsel)[0], "`counsel` names e9001, who is not in the organization from 2027-04-01.");
    let second = contract.diags.iter().find(|d| d.code == "E504").unwrap();
    assert_eq!((second.line, second.message.en.as_str()), (Some(31), "In the organization from 2027-04-01, the stage `second` cannot get 2 approvals for some requesters"));
    assert_eq!(notes(second)[..2], ["`legal` has only 1 member besides the requester.", "For 2 requesters: e9002 (Lawyer 2) and e9003 (Lawyer 3)."]);
    let expense = run.file("examples/expense.ringi").unwrap();
    let w506: Vec<&ringi::diag::Diag> = expense.diags.iter().filter(|d| d.code == "W506" && notes(d)[0].contains("`decide` is `dept_head`")).collect();
    assert_eq!(w506.iter().map(|d| d.line).collect::<Vec<_>>(), [Some(37)], "{}", common::shown(&expense, Lang::En));
    let said = notes(w506[0]);
    assert!(said.contains(&"For 3 requesters, e3101 (Head of Sales Department 1) answers both: e1101 (Manager of Section 1A), e1201 (Sales staff 1) and e1203 (Sales staff 3).".to_string()), "{said:?}");
    assert!(said.contains(&"`same approver approves once` does not do `decide` with the approval of `first`: `second` comes between them.".to_string()), "{said:?}");
}

/// The contract passes in the version in effect now, with warnings alone, and fails in the version
/// laid ahead: what a check before the switch shows (DESIGN 8.6).
#[test]
fn the_contract_passes_now_and_fails_after_the_switch() {
    for (day, exit, errors) in [("2026-11-16", 0, Vec::new()), ("2027-04-01", 1, vec!["E504", "E503"])] {
        let run = Run::with(common::joined(), Options { at: ritsu_ports::day_of(day), ..Options::default() });
        for f in ["examples/contract.ringi", "examples/contract.ja.ringi"] {
            let o = run.file(f).unwrap();
            let e: Vec<&str> = o.diags.iter().filter(|d| d.code.starts_with('E')).map(|d| d.code).collect();
            assert_eq!((o.exit(), e), (exit, errors.clone()), "{f} on {day}:\n{}", common::shown(&o, Lang::En));
        }
    }
}

#[test]
fn alone_a_request_says_the_languages_it_needs() {
    let run = Run::new(Suite::default());
    for f in common::example_files() {
        let o = run.file(&f).unwrap();
        let src = std::fs::read_to_string(&f).unwrap();
        let first = ringi::names::first_word(&src).unwrap();
        let codes = common::codes(&o);
        match first.as_str() {
            "organization" | "masters" => assert!(codes.is_empty(), "{f}: {codes:?}"),
            _ => {
                assert!(!codes.is_empty() && codes.iter().all(|c| *c == "E008"), "{f}: {codes:?}");
                assert_eq!(o.exit(), 2, "{f}");
                // once for each language
                let mut tools: Vec<String> = o.diags.iter().map(|d| d.message.en.split_whitespace().nth(3).unwrap_or_default().to_string()).collect();
                let n = tools.len();
                tools.sort();
                tools.dedup();
                assert_eq!(tools.len(), n, "{f}: {tools:?}");
            }
        }
    }
}

/// expense.ringi needs every language it reads and is lowered to: kaname, rulec, koyomi, chobo,
/// dandori and sekisho, each said at the first line that needs it.
#[test]
fn alone_the_expense_request_says_six_languages() {
    let o = Run::new(Suite::default()).file("examples/expense.ringi").unwrap();
    let said: Vec<(usize, String)> = o.diags.iter().map(|d| (d.line.unwrap_or(0), d.message.en.split_whitespace().nth(3).unwrap_or_default().trim_end_matches(',').to_string())).collect();
    let want: Vec<(usize, String)> = [(1, "kaname"), (7, "koyomi"), (8, "chobo"), (9, "sekisho"), (10, "dandori"), (14, "rulec")].iter().map(|(l, t)| (*l, t.to_string())).collect();
    assert_eq!(said, want, "{}", common::shown(&o, Lang::En));
}
