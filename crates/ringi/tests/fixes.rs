//! What a diagnostic or the ledger says to write, written as it says, passes the check (DESIGN
//! 9.5): for each way to write a code away that a note, a message or the fix of the ledger shows,
//! a file that gives the code, and the same file with the lines written in as they are shown,
//! which the check passes with nothing to say. A way shown is held to the text of the diagnostic
//! (or of the ledger) in English and in Japanese, word for word, `…` and a `<…>` standing for what
//! the file fills in. The files are checked as files of `examples/`, whose organization has a board,
//! in its version of 2026-11-16, or of `tests/mutants/`, whose small one has none; nothing is
//! written to the disk. Correcting the data, the other way the notes show, is checked with the data
//! of the small organization corrected in the cell the note names.

mod common;

use ringi::check::{Options, Outcome, Run};
use ritsu_base::text::Lang;

/// One way to write a code away.
struct Fix {
    /// What it is, for the failures.
    what: &'static str,
    code: &'static str,
    /// What else the file gives, which the same lines write away.
    also: &'static [&'static str],
    /// The file it is checked as.
    path: &'static str,
    before: &'static str,
    /// What the diagnostic shows to write, each in backquotes in its message or a note.
    said: &'static [&'static str],
    /// What the fix of the code's entry in the ledger (`ringi explain`) shows, the same way.
    ledger: &'static [&'static str],
    /// The edits that write it: each first text is in the file once, and is written over with the
    /// second.
    edits: &'static [(&'static str, &'static str)],
}

/// A file of `examples/`: the organization of the examples, which has a board.
const DESIGN: &str = "examples/fix.ringi";
/// A file of `tests/mutants/`: the small organization, with no board.
const SMALL: &str = "tests/mutants/fix.ringi";

/// The check of `src` as the file `path`: a file of `examples/` in the version of 2026-11-16.
fn check(path: &str, src: &str) -> Outcome {
    let at = if path.starts_with("examples/") { ritsu_ports::day_of("2026-11-16") } else { None };
    Run::with(common::joined(), Options { at, ..Options::default() }).text(path, src)
}

/// What the code's diagnostics say, in `lang`: the messages and the notes.
fn said(o: &Outcome, code: &str, lang: Lang) -> String {
    o.diags.iter().filter(|d| d.code == code).map(|d| d.render(lang)).collect()
}

/// The fix of the code's entry in the ledger, in `lang`.
fn ledger_fix(code: &str, lang: Lang) -> String {
    let l = ringi::codes::ledger();
    let e = l.entries.iter().find(|e| e.code == code).unwrap_or_else(|| panic!("{code} is not in the ledger"));
    match lang {
        Lang::Ja => e.fix.ja.clone(),
        _ => e.fix.en.clone(),
    }
}

/// A way shown, as the Japanese text shows it.
fn in_japanese(shown: &str) -> String {
    shown.replace("<table>", "<表>").replace("<way>", "<決め方>")
}

/// The pieces a file writes of a way shown: the text between `…` and `<…>`, trimmed.
fn pieces(shown: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let cs: Vec<char> = shown.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        // `<table>`: a word in angle brackets, which `when margin < 0JPY` is not
        let close = (c == '<' && cs.get(i + 1).is_some_and(|n| !n.is_whitespace())).then(|| cs[i + 1..].iter().position(|&x| x == '>' || x.is_whitespace())).flatten();
        match close {
            Some(j) if cs[i + 1 + j] == '>' => {
                out.push(std::mem::take(&mut cur));
                i += j + 2;
                continue;
            }
            _ if c == '…' => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
        i += 1;
    }
    out.push(cur);
    out.into_iter().map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect()
}

/// Whether `text` writes the way shown: its pieces, in order, the spaces that line up a block
/// being one space.
fn writes(text: &str, shown: &str) -> bool {
    let one = |s: &str| s.split(' ').filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ");
    let text = text.lines().map(one).collect::<Vec<_>>().join("\n");
    let mut rest = text.as_str();
    for p in pieces(shown) {
        let p = one(&p);
        match rest.find(&p) {
            Some(i) => rest = &rest[i + p.len()..],
            None => return false,
        }
    }
    true
}

fn fixes() -> Vec<Fix> {
    vec![
        // ── E309: what the route cannot read, by what it is ──
        Fix {
            what: "the requester, and a stage for the person at the top",
            code: "E309",
            also: &["E502"],
            path: DESIGN,
            before: r#"request fix v1 "Fix"

use organization org from "shared/company.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  manager
  audit  board    when requester.id is "e0001"

who
  manager = manager of requester
  board   = team board

ticket
  self approval  not allowed
"#,
            said: &["when empty escalate, then board", "board = team board"],
            ledger: &["when empty escalate, then board", "board = team board"],
            edits: &[("  audit  board    when requester.id is \"e0001\"\n", ""), ("ticket\n", "every stage\n  when empty escalate, then board\n\nticket\n")],
        },
        Fix {
            what: "the requester, and who may submit",
            code: "E309",
            also: &["E502"],
            path: DESIGN,
            before: r#"request fix v1 "Fix"

use organization org from "shared/company.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  manager
  audit  board    when requester.id is "e0001"

who
  manager = manager of requester
  board   = team board

ticket
  self approval  not allowed
"#,
            said: &["submitted by", "ticket"],
            ledger: &["submitted by", "ticket"],
            edits: &[("  audit  board    when requester.id is \"e0001\"\n", ""), ("ticket\n", "ticket\n  submitted by   everyone except person \"e0001\"\n")],
        },
        Fix {
            what: "two fields compared",
            code: "E309",
            also: &[],
            path: DESIGN,
            before: r#"request fix v1 "Fix"

use organization org from "shared/company.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY
  limit  : money[JPY] "Limit"  range >=1JPY <=100_000JPY

route
  check  manager
  audit  board    when amount > limit

who
  manager = manager of requester
  board   = team board

every stage
  when empty escalate, then board

ticket
  self approval  not allowed
"#,
            said: &["difference : money[JPY] = amount - limit", "when difference > 0JPY"],
            ledger: &["difference : money[JPY] = amount - limit", "when difference > 0JPY"],
            edits: &[
                ("  limit  : money[JPY] \"Limit\"  range >=1JPY <=100_000JPY\n", "  limit  : money[JPY] \"Limit\"  range >=1JPY <=100_000JPY\n  difference : money[JPY] = amount - limit\n"),
                ("when amount > limit", "when difference > 0JPY"),
            ],
        },
        Fix {
            what: "two fields compared, in Japanese names",
            code: "E309",
            also: &[],
            path: SMALL,
            before: r#"request 例(example) v1 "例"

form
  金額(amount) : money[円] "金額"  range >=1円 <=100_000円
  上限(limit)  : money[円] "上限"  range >=1円 <=100_000円

route
  確かめ(check)  経理
  監査(audit)    取締役会  when 金額 is 上限

who
  経理(finance)   = team finance
  取締役会(board) = team board

ticket
  self approval  not allowed
"#,
            said: &["difference : money[円] = 金額 - 上限", "when difference is 0円"],
            ledger: &[],
            edits: &[("  上限(limit)  : money[円] \"上限\"  range >=1円 <=100_000円\n", "  上限(limit)  : money[円] \"上限\"  range >=1円 <=100_000円\n  difference : money[円] = 金額 - 上限\n"), ("when 金額 is 上限", "when difference is 0円")],
        },
        Fix {
            what: "a computed field the form does not compute",
            code: "E309",
            also: &[],
            path: DESIGN,
            before: r#"request fix v1 "Fix"

use organization org from "shared/company.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY
  limit  : money[JPY] "Limit"  range >=1JPY <=100_000JPY
  over   : bool = amount > limit

route
  check  manager
  audit  board    when over

who
  manager = manager of requester
  board   = team board

every stage
  when empty escalate, then board

ticket
  self approval  not allowed
"#,
            said: &["margin : money[JPY] = limit - amount", "when margin < 0JPY"],
            ledger: &[],
            edits: &[("  over   : bool = amount > limit\n", "  margin : money[JPY] = limit - amount\n"), ("when over", "when margin < 0JPY")],
        },
        Fix {
            what: "a computation in the condition",
            code: "E309",
            also: &[],
            path: DESIGN,
            before: r#"request fix v1 "Fix"

use organization org from "shared/company.ringi"

form
  items : rows "Items"  count >=1 <=10
    what  : text "What"  count <=40
    price : money[JPY] "Price"  range >=1JPY <=1_000_000JPY

route
  check  manager
  audit  board    when sum(items.price) > 100_000JPY

who
  manager = manager of requester
  board   = team board

every stage
  when empty escalate, then board

ticket
  self approval  not allowed
"#,
            said: &["value : money[JPY] = sum(items.price)", "when value > 100_000JPY"],
            ledger: &[],
            edits: &[
                ("    price : money[JPY] \"Price\"  range >=1JPY <=1_000_000JPY\n", "    price : money[JPY] \"Price\"  range >=1JPY <=1_000_000JPY\n  value : money[JPY] = sum(items.price)\n"),
                ("when sum(items.price) > 100_000JPY", "when value > 100_000JPY"),
            ],
        },
        Fix {
            what: "a field of text",
            code: "E309",
            also: &[],
            path: DESIGN,
            before: r#"request fix v1 "Fix"

use organization org from "shared/company.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY
  place  : text "Place"  count <=20

route
  check  manager
  audit  board    when place is "Osaka"

who
  manager = manager of requester
  board   = team board

every stage
  when empty escalate, then board

ticket
  self approval  not allowed
"#,
            said: &["choice"],
            ledger: &["choice"],
            edits: &[("  place  : text \"Place\"  count <=20\n", "  place  : choice \"Place\"\n    osaka  \"Osaka\"\n    other  \"Elsewhere\"\n"), ("when place is \"Osaka\"", "when place is osaka")],
        },
        // ── E502: a stage where the ticket would wait ──
        Fix {
            what: "nobody above, and the stage skipped",
            code: "E502",
            also: &[],
            path: DESIGN,
            before: r#"request fix v1 "Fix"

use organization org from "shared/company.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check   manager
  decide  directors

who
  manager   = manager of requester
  directors = team board

every stage
  when empty escalate

ticket
  self approval  not allowed
"#,
            said: &["when empty escalate, then skip"],
            ledger: &["when empty escalate, then skip"],
            edits: &[("  when empty escalate\n", "  when empty escalate, then skip\n")],
        },
        Fix {
            what: "nobody above, and the stage sent to the board",
            code: "E502",
            also: &[],
            path: DESIGN,
            before: r#"request fix v1 "Fix"

use organization org from "shared/company.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  manager

who
  manager = manager of requester

every stage
  when empty escalate

ticket
  self approval  not allowed
"#,
            said: &["when empty escalate, then board", "board = team board"],
            ledger: &["when empty escalate, then board", "board = team board"],
            edits: &[("  manager = manager of requester\n", "  manager = manager of requester\n  board   = team board\n"), ("  when empty escalate\n", "  when empty escalate, then board\n")],
        },
        Fix {
            what: "the way after `then` finds nobody",
            code: "E502",
            also: &[],
            path: DESIGN,
            before: r#"request fix v1 "Fix"

use organization org from "shared/company.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  manager

who
  manager = manager of requester
  auditor = person "e9999"

every stage
  when empty escalate, then auditor

ticket
  self approval  not allowed
"#,
            said: &["then"],
            ledger: &["then"],
            edits: &[("  auditor = person \"e9999\"\n", "  auditor = person \"e0201\"\n")],
        },
        Fix {
            what: "a stage that stays",
            code: "E502",
            also: &[],
            path: DESIGN,
            before: r#"request fix v1 "Fix"

use organization org from "shared/company.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check   manager
  decide  directors

who
  manager   = manager of requester
  directors = team board

ticket
  self approval  not allowed
"#,
            said: &["when empty"],
            ledger: &["when empty escalate, then skip"],
            edits: &[("ticket\n", "every stage\n  when empty escalate, then skip\n\nticket\n")],
        },
        // ── W510: a request nobody answers ──
        Fix {
            what: "nobody answers the president, and the board does",
            code: "W510",
            also: &[],
            path: DESIGN,
            before: r#"request fix v1 "Fix"

use organization org from "shared/company.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  manager

who
  manager = manager of requester

every stage
  when empty escalate, then skip

ticket
  self approval  not allowed
"#,
            said: &["when empty escalate, then board", "board = team board"],
            ledger: &["when empty escalate, then board", "board = team board"],
            edits: &[("  manager = manager of requester\n", "  manager = manager of requester\n  board   = team board\n"), ("then skip", "then board")],
        },
        Fix {
            what: "nobody answers the president, who does not submit it",
            code: "W510",
            also: &[],
            path: DESIGN,
            before: r#"request fix v1 "Fix"

use organization org from "shared/company.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  manager

who
  manager = manager of requester

every stage
  when empty escalate, then skip

ticket
  self approval  not allowed
"#,
            said: &["submitted by … except …"],
            ledger: &["submitted by … except …"],
            edits: &[("ticket\n", "ticket\n  submitted by   everyone except person \"e0001\"\n")],
        },
        // ── W506, W507: one person twice, a late stage with nowhere to go ──
        Fix {
            what: "one person checks twice, and approves once",
            code: "W506",
            also: &[],
            path: SMALL,
            before: r#"request fix v1 "Fix"

use organization org from "shared/small.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check   finance
  decide  head

who
  finance = team finance
  head    = person "e04"

ticket
  self approval  allowed
"#,
            said: &["same approver approves once", "ticket"],
            ledger: &["same approver approves once", "ticket"],
            edits: &[("  self approval  allowed\n", "  self approval  allowed\n  same approver approves once\n")],
        },
        Fix {
            what: "one person checks twice with a stage between, and answers once at the last",
            code: "W506",
            also: &[],
            path: SMALL,
            before: r#"request fix v1 "Fix"

use organization org from "shared/small.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check   finance
  review  sales
  decide  head

who
  finance = team finance
  sales   = person "e02"
  head    = person "e04"

ticket
  self approval  allowed
"#,
            said: &["same approver approves once, at the last stage", "ticket"],
            ledger: &["same approver approves once, at the last stage", "ticket"],
            edits: &[("  self approval  allowed\n", "  self approval  allowed\n  same approver approves once, at the last stage\n")],
        },
        Fix {
            what: "one person checks twice with a stage between, where the line written does not do it",
            code: "W506",
            also: &[],
            path: SMALL,
            before: r#"request fix v1 "Fix"

use organization org from "shared/small.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check   finance
  review  sales
  decide  head

who
  finance = team finance
  sales   = person "e02"
  head    = person "e04"

ticket
  self approval  allowed
  same approver  approves once
"#,
            said: &["same approver approves once, at the last stage"],
            ledger: &["same approver approves once, at the last stage"],
            edits: &[("  same approver  approves once\n", "  same approver  approves once, at the last stage\n")],
        },
        Fix {
            what: "a late stage with nobody above, sent on by a table",
            code: "W507",
            also: &[],
            path: SMALL,
            before: r#"request fix v1 "Fix"

use organization org from "shared/no_president.ringi"
use calendar office from "shared/office.cal"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  decide  sales_head

who
  sales_head = person "e02"

every stage
  due in 3 business days at end of day
  when late escalate, then expire

ticket
  self approval  allowed
"#,
            said: &["when late escalate by <table>"],
            ledger: &["when late escalate by <table>"],
            edits: &[
                ("  sales_head = person \"e02\"\n", "  sales_head   = person \"e02\"\n  finance_head = person \"e04\"\n"),
                ("  when late escalate, then expire\n", "  when late escalate by next_up, then expire\n\ntable next_up\n| approver     | -> goes_to   |\n| sales_head   | finance_head |\n| finance_head | finance_head |\n"),
            ],
        },
        // ── `submitted by` ──
        Fix {
            what: "a person who is not there",
            code: "E508",
            also: &[],
            path: SMALL,
            before: r#"request fix v1 "Fix"

use organization org from "shared/small.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  finance

who
  finance = team finance

ticket
  submitted by   person "e02", person "e09"
  self approval  not allowed
"#,
            said: &[],
            ledger: &[],
            edits: &[("person \"e09\"", "person \"e03\"")],
        },
        Fix {
            what: "everyone left out",
            code: "E509",
            also: &[],
            path: SMALL,
            before: r#"request fix v1 "Fix"

use organization org from "shared/small.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  finance

who
  finance = team finance

ticket
  submitted by   position dept_head or higher in sales except person "e02"
  self approval  not allowed
"#,
            said: &["submitted by"],
            ledger: &["submitted by"],
            edits: &[("position dept_head or higher in sales", "position staff or higher in sales")],
        },
        Fix {
            what: "`submitted by` twice",
            code: "E006",
            also: &[],
            path: SMALL,
            before: r#"request fix v1 "Fix"

use organization org from "shared/small.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  manager

who
  manager = manager of requester

ticket
  submitted by   team sales
  self approval  not allowed
  submitted by   person "e04"
"#,
            said: &[],
            ledger: &["ticket"],
            edits: &[("  submitted by   team sales\n", "  submitted by   team sales, person \"e04\"\n"), ("  submitted by   person \"e04\"\n", "")],
        },
        Fix {
            what: "a position `submitted by` does not know",
            code: "E004",
            also: &[],
            path: SMALL,
            before: r#"request fix v1 "Fix"

use organization org from "shared/small.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  finance

who
  finance = team finance

ticket
  submitted by   position manager or higher
  self approval  allowed
"#,
            said: &["dept_head"],
            ledger: &[],
            edits: &[("position manager or higher", "position dept_head or higher")],
        },
        // ── `when empty escalate, then …` ──
        Fix {
            what: "a way after `then` not in `who`",
            code: "E004",
            also: &[],
            path: DESIGN,
            before: r#"request fix v1 "Fix"

use organization org from "shared/company.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  manager

who
  manager = manager of requester

every stage
  when empty escalate, then board

ticket
  self approval  not allowed
"#,
            said: &["board", "who"],
            ledger: &[],
            edits: &[("  manager = manager of requester\n", "  manager = manager of requester\n  board   = team board\n")],
        },
        Fix {
            what: "a way named `stay`",
            code: "E002",
            also: &[],
            path: SMALL,
            before: r#"request fix v1 "Fix"

use organization org from "shared/small.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check  manager

who
  manager = manager of requester
  stay    = person "e04"

every stage
  when empty escalate, then stay

ticket
  self approval  not allowed
"#,
            said: &[],
            ledger: &[],
            edits: &[("  stay    = person \"e04\"\n", "  auditor = person \"e04\"\n"), ("then stay", "then auditor")],
        },
        Fix {
            what: "going up twice when empty",
            code: "E001",
            also: &[],
            path: DESIGN,
            before: r#"request fix v1 "Fix"

use organization org from "shared/company.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check   manager
  decide  directors

who
  manager   = manager of requester
  directors = team board

every stage
  when empty escalate, then escalate

ticket
  self approval  not allowed
"#,
            said: &["skip"],
            ledger: &[],
            edits: &[("then escalate", "then skip")],
        },
        Fix {
            what: "`, then …` after what is not `escalate`",
            code: "E001",
            also: &[],
            path: DESIGN,
            before: r#"request fix v1 "Fix"

use organization org from "shared/company.ringi"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY

route
  check   manager
  decide  directors

who
  manager   = manager of requester
  directors = team board

every stage
  when empty skip, then directors

ticket
  self approval  not allowed
"#,
            said: &[", then …", "escalate"],
            ledger: &[],
            edits: &[("when empty skip, then directors", "when empty escalate, then directors")],
        },
        // ── E405: a hold that reads what approvers may edit ──
        Fix {
            what: "a hold of an amount approvers edit, and the line taken out",
            code: "E405",
            also: &[],
            path: SMALL,
            before: r#"request fix v1 "Fix"

use calendar office from "shared/office.cal"
use book budget from "shared/budget.book"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY
  dept   : text "Department"  count >=1 <=20

route
  check  finance

who
  finance = team finance

every stage
  due in 3 business days at end of day
  when late expire

ticket
  self approval   not allowed
  approvers edit  amount
  on submitted    hold budget.reserve(request: ticket.id, attempt: ticket.attempt, dept: dept, amount: amount)
  on approved     post budget.reserve(request: ticket.id, attempt: ticket.attempt)
  on rejected, expired  void budget.reserve(request: ticket.id, attempt: ticket.attempt)
"#,
            said: &[],
            ledger: &[],
            edits: &[("  approvers edit  amount\n", "")],
        },
        Fix {
            what: "a hold on a condition approvers edit, and the field taken out",
            code: "E405",
            also: &[],
            path: SMALL,
            before: r#"request fix v1 "Fix"

use calendar office from "shared/office.cal"
use book budget from "shared/budget.book"

form
  amount : money[JPY] "Amount"  range >=1JPY <=100_000JPY
  dept   : text "Department"  count >=1 <=20
  kind   : choice "Kind"
    budgeted  "From the budget"
    free      "At no cost"
  memo   : long text "Memo"  count <=200  approvers only  optional

route
  check  finance

who
  finance = team finance

every stage
  due in 3 business days at end of day
  when late expire

ticket
  self approval   not allowed
  approvers edit  kind, memo
  on submitted    hold budget.reserve(request: ticket.id, attempt: ticket.attempt, dept: dept, amount: amount)  when kind is budgeted
  on approved     post budget.reserve(request: ticket.id, attempt: ticket.attempt)  when kind is budgeted
  on rejected, expired  void budget.reserve(request: ticket.id, attempt: ticket.attempt)  when kind is budgeted
"#,
            said: &[],
            ledger: &[],
            edits: &[("  approvers edit  kind, memo\n", "  approvers edit  memo\n")],
        },
    ]
}

/// Each file gives its code and shows the way to write it away, in English and in Japanese; written
/// as shown, it passes with nothing to say.
#[test]
fn what_the_notes_say_to_write_passes() {
    let mut failures = Vec::new();
    for f in fixes() {
        let o = check(f.path, f.before);
        let shown = common::shown(&o, Lang::En);
        let mut codes = common::codes(&o);
        codes.sort();
        codes.dedup();
        let mut meant: Vec<&str> = f.also.iter().copied().chain([f.code]).collect();
        meant.sort();
        if codes != meant {
            failures.push(format!("{} ({}): the file gives {codes:?}, not {meant:?}:\n{shown}", f.what, f.code));
            continue;
        }
        for lang in [Lang::En, Lang::Ja] {
            let (s, l) = (said(&o, f.code, lang), ledger_fix(f.code, lang));
            for (ways, text, place) in [(f.said, &s, "the diagnostic"), (f.ledger, &l, "the ledger's fix")] {
                for w in ways {
                    let w = if lang == Lang::Ja { in_japanese(w) } else { w.to_string() };
                    if !text.contains(&format!("`{w}`")) {
                        failures.push(format!("{} ({}): {place} in {lang:?} does not show `{w}`:\n{text}", f.what, f.code));
                    }
                }
            }
        }
        let mut after = f.before.to_string();
        for (old, new) in f.edits {
            assert_eq!(after.matches(old).count(), 1, "{}: {old:?} is not in the file once", f.what);
            after = after.replacen(old, new, 1);
        }
        for w in f.said.iter().chain(f.ledger) {
            if !writes(&after, w) {
                failures.push(format!("{} ({}): the file written does not write `{w}`:\n{after}", f.what, f.code));
            }
        }
        let o = check(f.path, &after);
        if !o.diags.is_empty() {
            failures.push(format!("{} ({}): written as shown, the file is still said of:\n{}\n{after}", f.what, f.code, common::shown(&o, Lang::En)));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// The other way the notes of W502 and W507 show: correct the data of the version. The data of
/// the small organization is the data of the vacant seats with the one cell filled; read instead
/// (`--org`), the request passes with nothing to say.
#[test]
fn correcting_the_data_as_the_notes_say_passes() {
    for (path, code, vacant, row) in [
        ("tests/mutants/W502_the_head_of_finance_is_empty.ringi", "W502", "org-vacant", "finance,Finance,財務部,department,company,e04"),
        ("tests/mutants/W507_nobody_above_the_head_of_sales.ringi", "W507", "org-no-president", "company,Example Co. (fictional),株式会社サンプル（架空）,company,,e01"),
    ] {
        let o = common::check(path);
        assert_eq!(common::codes(&o), [code], "{}", common::shown(&o, Lang::En));
        assert!(said(&o, code, Lang::En).contains("Correct the data of the version"), "{}", common::shown(&o, Lang::En));
        // the two differ in the row of the empty seat alone
        let read = |dir: &str| std::fs::read_to_string(format!("tests/mutants/shared/{dir}/2026-04-01/teams.csv")).unwrap();
        let (was, is) = (read(vacant), read("org-small"));
        let changed: Vec<(&str, &str)> = was.lines().zip(is.lines()).filter(|(a, b)| a != b).collect();
        assert_eq!(changed.len(), 1, "{vacant}: {changed:?}");
        assert_eq!(changed[0].1, row);
        assert_eq!(changed[0].0, row.trim_end_matches(char::is_alphanumeric), "{vacant}");
        for f in ["members.csv", "people.csv"] {
            let read = |dir: &str| std::fs::read_to_string(format!("tests/mutants/shared/{dir}/2026-04-01/{f}")).unwrap();
            assert_eq!(read(vacant), read("org-small"), "{vacant}/{f}");
        }
        let o = Run::with(common::joined(), Options { org: Some("tests/mutants/shared/org-small".to_string()), ..Options::default() }).file(path).unwrap();
        assert!(o.diags.is_empty(), "{}", common::shown(&o, Lang::En));
    }
}

/// The rows E301 shows to add (DESIGN 6.5), each `?` answered as the table's first row answers it
/// and pasted after the table's rows, close the gap: at once where rulec lists every part, and over
/// the checks where it lists the first eight. Each E301 mutant, and two tables of more parts: one
/// of two columns, one of ten parts. The rows of `fix` are the ones the text lists.
#[test]
fn the_rows_e301_shows_close_the_gap() {
    let mut files: Vec<(String, String)> = std::fs::read_dir("tests/mutants")
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|n| n.starts_with("E301_") && n.ends_with(".ringi"))
        .map(|n| (format!("tests/mutants/{n}"), std::fs::read_to_string(format!("tests/mutants/{n}")).unwrap()))
        .collect();
    files.sort();
    assert_eq!(files.len(), 4, "{files:?}");
    files.push((SMALL.to_string(), TWO_COLUMNS.to_string()));
    files.push((SMALL.to_string(), TEN_PARTS.to_string()));
    let mut failures = Vec::new();
    for (path, before) in files {
        let mut text = before.clone();
        let mut runs = 0;
        let mut rows_seen = 0;
        loop {
            let o = check(&path, &text);
            let Some(d) = o.diags.iter().find(|d| d.code == "E301") else {
                if !o.diags.is_empty() {
                    failures.push(format!("{path}: with the rows written, the file is still said of:\n{}\n{text}", common::shown(&o, Lang::En)));
                }
                break;
            };
            runs += 1;
            if runs > 2 {
                failures.push(format!("{path}: E301 again after {} checks:\n{}\n{text}", runs - 1, common::shown(&o, Lang::En)));
                break;
            }
            let Some(fix) = d.fixed_line() else {
                failures.push(format!("{path}: E301 has no rows in `fix`:\n{}", common::shown(&o, Lang::En)));
                break;
            };
            for lang in [Lang::En, Lang::Ja] {
                let said = d.render(lang);
                for row in fix.lines() {
                    if !said.lines().any(|l| l.trim_end().ends_with(row.trim())) {
                        failures.push(format!("{path} ({lang:?}): the text does not list the row `{row}` of `fix`:\n{said}"));
                    }
                }
            }
            let answered: Vec<String> = fix.lines().map(|r| answered(&text, r)).collect();
            rows_seen += answered.len();
            text = pasted(&text, &answered);
        }
        if runs == 0 {
            failures.push(format!("{path}: no E301:\n{before}"));
        }
        // ten parts: eight rows on the first check, the two others on the next
        if before == TEN_PARTS && (runs != 2 || rows_seen != 10) {
            failures.push(format!("{path}: ten parts closed in {runs} checks, with {rows_seen} rows"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// A table of two columns with four parts no row takes.
const TWO_COLUMNS: &str = r#"request fix v1 "Fix"

form
  amount : money[JPY] "Amount"  range >=1JPY <=1_000_000JPY
  kind   : choice "Kind"
    travel  "Travel"
    meals   "Meals"
    goods   "Goods"

route
| kind   | amount                   | -> check | decide  |
| travel | <=100_000JPY             | finance  | manager |
| meals  | -                        | finance  |         |
| goods  | >200_000JPY <=300_000JPY | finance  | manager |

who
  finance = team finance
  manager = team managers

ticket
  self approval  not allowed
"#;

/// A table with ten parts no row takes, two more than rulec lists at once.
const TEN_PARTS: &str = r#"request fix v1 "Fix"

form
  amount : money[JPY] "Amount"  range >=1JPY <=200_000JPY

route
| amount                   | -> decide |
| <=10_000JPY              | manager   |
| >20_000JPY <=30_000JPY   | manager   |
| >40_000JPY <=50_000JPY   | manager   |
| >60_000JPY <=70_000JPY   | manager   |
| >80_000JPY <=90_000JPY   | manager   |
| >100_000JPY <=110_000JPY | manager   |
| >120_000JPY <=130_000JPY | manager   |
| >140_000JPY <=150_000JPY | manager   |
| >160_000JPY <=170_000JPY | manager   |
| >180_000JPY <=190_000JPY | manager   |

who
  manager = team managers

ticket
  self approval  not allowed
"#;

/// The cells of a line of a table, between its bars.
fn cells(line: &str) -> Vec<String> {
    line.trim().trim_matches('|').split('|').map(|c| c.trim().to_string()).collect()
}

/// The lines of the route's table, by their places in the text: the head, then the rows.
fn table_lines(text: &str) -> Vec<(usize, String)> {
    let lines: Vec<&str> = text.lines().collect();
    let at = lines.iter().position(|l| l.trim() == "route").expect("a route");
    lines.iter().enumerate().skip(at + 1).take_while(|(_, l)| l.starts_with('|')).map(|(i, l)| (i, l.to_string())).collect()
}

/// A row E301 shows, each `?` written as the table's first row writes that column.
fn answered(text: &str, row: &str) -> String {
    let first = cells(&table_lines(text)[1].1);
    let out: Vec<String> = cells(row).into_iter().enumerate().map(|(k, c)| if c == "?" { first[k].clone() } else { c }).collect();
    format!("| {} |", out.join(" | "))
}

/// The text with the rows written after the last row of the route's table.
fn pasted(text: &str, rows: &[String]) -> String {
    let last = table_lines(text).last().expect("a row").0;
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    for (k, r) in rows.iter().enumerate() {
        lines.insert(last + 1 + k, r.clone());
    }
    lines.join("\n") + "\n"
}

/// The pieces of a way shown: `…` and a word in angle brackets stand for what the file fills in,
/// and a comparison is no word in angle brackets.
#[test]
fn the_pieces_of_a_way_shown() {
    assert_eq!(pieces("submitted by … except …"), ["submitted by", "except"]);
    assert_eq!(pieces("when late escalate by <table>"), ["when late escalate by"]);
    assert_eq!(pieces("when late escalate by <表>"), ["when late escalate by"]);
    assert_eq!(pieces("when margin < 0JPY"), ["when margin < 0JPY"]);
    assert_eq!(pieces("route capped(cap: …)"), ["route capped(cap:", ")"]);
    assert!(writes("  submitted by   everyone except person \"e0001\"\n", "submitted by … except …"));
    assert!(!writes("  except person \"e0001\" submitted by\n", "submitted by … except …"));
}
