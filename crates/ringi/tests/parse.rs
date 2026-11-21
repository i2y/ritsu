//! The syntax: every `.ringi` of `examples/` reads, and the lines that do not read say E001 where
//! they stop, with what the line takes there.

use ritsu_base::text::Lang;

fn example_files() -> Vec<String> {
    let mut v = Vec::new();
    for dir in ["examples", "examples/shared"] {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.extension().is_some_and(|x| x == "ringi") {
                v.push(p.display().to_string());
            }
        }
    }
    v.sort();
    v
}

#[test]
fn every_file_of_the_examples_reads() {
    let files = example_files();
    assert_eq!(files.len(), 13, "{files:?}");
    let mut failures = Vec::new();
    for f in files {
        let src = std::fs::read_to_string(&f).unwrap();
        let p = ringi::parse::parse(&f, &src);
        if p.file.is_none() || !p.diags.is_empty() {
            let text: String = p.diags.iter().map(|d| d.render(Lang::En)).collect();
            failures.push(format!("{f}:\n{text}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A request whose `every stage` is the one line given.
fn with_deadline(line: &str) -> String {
    format!(
        "request example v1 \"Example\"\n\nform\n  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY\n\nroute\n  check  manager\n\nwho\n  manager = manager of requester\n\nevery stage\n  {line}\n\nticket\n  self approval  not allowed\n"
    )
}

/// The `when empty` line of a request whose `every stage` is the one line given.
fn when_empty_of(line: &str) -> ringi::ast::WhenEmpty {
    use ringi::ast::Deadline;
    let p = ringi::parse::parse("example.ringi", &with_deadline(line));
    let text: String = p.diags.iter().map(|d| d.render(Lang::En)).collect();
    assert!(p.diags.is_empty(), "{line}: {text}");
    let every = p.file.and_then(|f| f.every_stage).unwrap();
    let got: Vec<_> = every.lines.iter().filter_map(|d| if let Deadline::Empty { what, .. } = d { Some(what.clone()) } else { None }).collect();
    assert_eq!(got.len(), 1, "{line}: {got:?}");
    got.into_iter().next().unwrap()
}

/// `when empty escalate` says after `then` what the stage does where nobody is above: `skip`,
/// `stay`, or a way to choose declared in `who` that it goes to; written alone, it is `then stay`
/// (DESIGN 5.4). Each form is kept as it is written, so that what is said of it quotes the line.
#[test]
fn when_empty_escalate_says_what_it_does_at_the_top() {
    use ringi::ast::{AtTop, WhenEmpty};
    for (line, want) in [
        ("when empty escalate", WhenEmpty::Escalate(None)),
        ("when empty escalate, then skip", WhenEmpty::Escalate(Some(AtTop::Skip))),
        ("when empty escalate, then stay", WhenEmpty::Escalate(Some(AtTop::Stay))),
        ("when empty stay", WhenEmpty::Stay),
        ("when empty skip", WhenEmpty::Skip),
    ] {
        assert_eq!(when_empty_of(line), want, "{line}");
        assert_eq!(want.written(), line);
    }
    // `then board`: the way is kept with where it is written, for what is said of it
    let line = "when empty escalate, then board";
    let got = when_empty_of(line);
    assert_eq!(got.written(), line);
    let way = got.at_top_way().unwrap();
    assert_eq!((way.text.as_str(), way.span.line, way.span.col), ("board", 13, 29));
}

/// `when empty` goes up once: after `then` it takes `skip`, `stay` or a way to choose, and `then`
/// follows nothing but `escalate`, in `when empty` and in `when late` alike. Each says E001 at the
/// word it stops at.
#[test]
fn then_follows_escalate_alone() {
    for (line, col, says) in [
        ("when empty escalate, then escalate", 29, "`when empty` does not go up twice: after `then`, write `skip`, `stay` or a way to choose an approver"),
        ("when empty escalate, then 3", 29, "Expected `skip`, `stay` or a way to choose an approver"),
        ("when empty escalate, then", 28, "The line ends where `skip`, `stay` or a way to choose an approver"),
        ("when empty stay, then skip", 18, "In `when empty`, `, then …` follows `escalate` only"),
        ("when empty skip, then stay", 18, "In `when empty`, `, then …` follows `escalate` only"),
        ("when late expire, then stay", 19, "In `when late`, `, then …` follows `escalate` only"),
        ("when late stay, then expire", 17, "In `when late`, `, then …` follows `escalate` only"),
    ] {
        let p = ringi::parse::parse("example.ringi", &with_deadline(line));
        let text: String = p.diags.iter().map(|d| d.render(Lang::En)).collect();
        assert_eq!(p.diags.iter().map(|d| d.code).collect::<Vec<_>>(), ["E001"], "{line}: {text}");
        assert!(text.contains(says), "{line}: {text}");
        assert!(text.starts_with(&format!("error[E001]: example.ringi:13:{col}: ")), "{line}: {text}");
    }
}

/// A request whose `ticket` is `self approval not allowed` and the line given.
fn with_ticket_line(line: &str) -> String {
    format!(
        "request example v1 \"Example\"\n\nform\n  amount : money[JPY] \"Amount\"  range >=1JPY <=100_000JPY\n\nroute\n  check  manager\n\nwho\n  manager = manager of requester\n\nticket\n  self approval  not allowed\n  {line}\n"
    )
}

/// `same approver approves once` is a line of the ticket (DESIGN 5.12), its words lined up as the
/// other lines are; a line that starts so and says something else is E001 at the word it stops at.
#[test]
fn same_approver_approves_once_is_a_line_of_the_ticket() {
    let p = ringi::parse::parse("example.ringi", &with_ticket_line("same approver  approves once"));
    let text: String = p.diags.iter().map(|d| d.render(Lang::En)).collect();
    assert!(p.diags.is_empty(), "{text}");
    let t = p.file.and_then(|f| f.ticket).unwrap();
    assert!(t.lines.iter().any(|l| matches!(l, ringi::ast::TicketLine::SameApprover { last: false, line: 14 })), "{:?}", t.lines);
    for (line, col, says) in [("same approver approves twice", 26, "Expected `once`"), ("same approval approves once", 8, "Expected `approver`"), ("same approver once", 17, "Expected `approves`")] {
        let p = ringi::parse::parse("example.ringi", &with_ticket_line(line));
        let text: String = p.diags.iter().map(|d| d.render(Lang::En)).collect();
        assert_eq!(p.diags.iter().map(|d| d.code).collect::<Vec<_>>(), ["E001"], "{line}: {text}");
        assert!(text.starts_with(&format!("error[E001]: example.ringi:14:{col}: {says}")), "{line}: {text}");
    }
}

/// `same approver approves once, at the last stage` is the same line with what comes after its comma
/// (DESIGN 5.12); what follows the comma is those four words and nothing else.
#[test]
fn at_the_last_stage_follows_the_line_after_a_comma() {
    let p = ringi::parse::parse("example.ringi", &with_ticket_line("same approver  approves once, at the last stage"));
    let text: String = p.diags.iter().map(|d| d.render(Lang::En)).collect();
    assert!(p.diags.is_empty(), "{text}");
    let t = p.file.and_then(|f| f.ticket).unwrap();
    assert!(t.lines.iter().any(|l| matches!(l, ringi::ast::TicketLine::SameApprover { last: true, line: 14 })), "{:?}", t.lines);
    for (line, col, says) in [
        ("same approver approves once, at the first stage", 39, "Expected `last`"),
        ("same approver approves once, at last stage", 35, "Expected `the`"),
        ("same approver approves once, at the last", 43, "The line ends where `stage`"),
        ("same approver approves once,", 31, "The line ends where `at`"),
        ("same approver approves once at the last stage", 31, "Expected `,`, found `at`"),
        ("same approver approves once, at the last stage of all", 50, "Nothing can follow here: `of` is left over"),
    ] {
        let p = ringi::parse::parse("example.ringi", &with_ticket_line(line));
        let text: String = p.diags.iter().map(|d| d.render(Lang::En)).collect();
        assert_eq!(p.diags.iter().map(|d| d.code).collect::<Vec<_>>(), ["E001"], "{line}: {text}");
        assert!(text.starts_with(&format!("error[E001]: example.ringi:14:{col}: {says}")), "{line}: {text}");
    }
}

/// `submitted by` is a line of the ticket (DESIGN 5.13): groups with commas, and the ones `except`
/// leaves out; each group is written as `who` writes the same people (`team`, `person`) or with the
/// organization's positions. A line that stops early, or names no group, is E001 where it stops.
#[test]
fn submitted_by_is_a_line_of_the_ticket() {
    use ringi::ast::{Group, TicketLine};
    let line = "submitted by   everyone in sales_div, team board, position manager or higher in legal, person \"e9001\" except position staff, team interns";
    let p = ringi::parse::parse("example.ringi", &with_ticket_line(line));
    let text: String = p.diags.iter().map(|d| d.render(Lang::En)).collect();
    assert!(p.diags.is_empty(), "{text}");
    let t = p.file.and_then(|f| f.ticket).unwrap();
    let Some(TicketLine::SubmittedBy { groups, except, line: 14, col: 3 }) = t.lines.iter().find(|l| matches!(l, TicketLine::SubmittedBy { .. })) else { panic!("{:?}", t.lines) };
    assert_eq!(Group::line_shown(groups, except), "everyone in sales_div, team board, position manager or higher in legal, person \"e9001\" except position staff, team interns");
    assert!(matches!(&groups[2], Group::Position { higher: true, within: Some(w), .. } if w.text == "legal" && w.span.col == 83), "{:?}", groups[2]);
    assert!(matches!(&groups[3], Group::Person(id, span) if id == "e9001" && span.col == 97), "{:?}", groups[3]);
    for (line, col, says) in [
        ("submitted by", 15, "The line ends where who may submit"),
        ("submitted by   board", 18, "Expected who may submit"),
        ("submitted by   team", 22, "The line ends where the team's ID"),
        ("submitted by   position manager or above", 38, "Expected `higher`"),
        ("submitted by   everyone except", 33, "The line ends where who may submit"),
        ("submitted by   person e9001", 25, "Expected the person's ID"),
        ("submitted for  everyone", 13, "Expected `by`"),
    ] {
        let p = ringi::parse::parse("example.ringi", &with_ticket_line(line));
        let text: String = p.diags.iter().map(|d| d.render(Lang::En)).collect();
        assert_eq!(p.diags.iter().map(|d| d.code).collect::<Vec<_>>(), ["E001"], "{line}: {text}");
        assert!(text.starts_with(&format!("error[E001]: example.ringi:14:{col}: {says}")), "{line}: {text}");
    }
}

/// A rule a `ticket` takes once, written twice, is E006 at the second line, which is left out; `on`
/// lines are as many as there are.
#[test]
fn a_ticket_takes_each_rule_once() {
    for (line, word) in [("self approval  allowed", "self approval"), ("same approver  approves once\n  same approver  approves once", "same approver approves once"), ("same approver  approves once\n  same approver  approves once, at the last stage", "same approver approves once"), ("submitted by   everyone\n  submitted by   team board", "submitted by")] {
        let p = ringi::parse::parse("example.ringi", &with_ticket_line(line));
        let text: String = p.diags.iter().map(|d| d.render(Lang::En)).collect();
        assert_eq!(p.diags.iter().map(|d| d.code).collect::<Vec<_>>(), ["E006"], "{line}: {text}");
        assert!(text.contains(&format!(": A `ticket` takes `{word}` once\n")), "{line}: {text}");
    }
    let p = ringi::parse::parse("example.ringi", &with_ticket_line("on approved    post budget.reserve(request: ticket.id)\n  on approved    post budget.reserve(request: ticket.id)"));
    assert!(p.diags.is_empty(), "{:?}", p.diags.iter().map(|d| d.render(Lang::En)).collect::<String>());
}

/// A scenario's table has a `comment` column after `by`, or none (DESIGN 3.2): a cell of it is one
/// text in quotes, or empty, and a header that puts the column anywhere else does not read (E001).
#[test]
fn a_scenario_table_takes_a_comment_column_after_by() {
    let read = |table: &str| {
        let src = format!("{}\nscenario a_request\n  requester e1201\n  form amount = 1_000JPY\n{table}", with_ticket_line("send back      to the requester, at most 2 times"));
        let p = ringi::parse::parse("example.ringi", &src);
        let text: String = p.diags.iter().map(|d| d.render(Lang::En)).collect();
        (p, text)
    };
    let (p, text) = read("| at | event | by | comment | -> stage | status |\n| 2026-11-16 10:00 | submit | e1201 | \"For the new office\" | check | in progress |\n| 2026-11-16 11:00 | send back | e1101 | | | with requester |\n");
    assert!(p.diags.is_empty(), "{text}");
    let events = &p.file.as_ref().unwrap().scenarios[0].events;
    assert_eq!(events.iter().map(|e| e.comment.as_ref().map(|w| w.text.as_str())).collect::<Vec<_>>(), [Some("For the new office"), None]);
    // a table with no such column reads as it did
    let (p, text) = read("| at | event | by | -> stage | status |\n| 2026-11-16 10:00 | submit | e1201 | check | in progress |\n");
    assert!(p.diags.is_empty(), "{text}");
    assert!(p.file.unwrap().scenarios[0].events[0].comment.is_none());
    // the column elsewhere, and a cell that is not a text in quotes
    let (p, text) = read("| at | event | comment | by | -> stage | status |\n| 2026-11-16 10:00 | submit | \"For the new office\" | e1201 | check | in progress |\n");
    assert_eq!(p.diags.iter().map(|d| d.code).collect::<Vec<_>>(), ["E001"], "{text}");
    assert!(text.contains(", and a `comment` column after `by` where a row writes what a person wrote\n"), "{text}");
    let (p, text) = read("| at | event | by | comment | -> stage | status |\n| 2026-11-16 10:00 | submit | e1201 | For the new office | check | in progress |\n");
    assert_eq!(p.diags.iter().map(|d| d.code).collect::<Vec<_>>(), ["E001"], "{text}");
    assert!(text.contains(": A `comment` cell is one text in quotes, or empty\n"), "{text}");
}
