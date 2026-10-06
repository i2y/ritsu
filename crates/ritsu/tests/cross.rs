//! The checks across the languages, as `ritsu check` runs them on a project (DESIGN 7; PLAN E.4).
//! Each check has a project where it holds, and variants where there is an example that it does
//! not and where it cannot be decided; the text (English and Japanese) and the JSON's `borders`
//! are held to golden files in `tests/golden/cross/` (`RITSU_BLESS=1` writes them). The projects
//! are small and English: the ledger's reproductions (`ritsu_cross::codes`) are laid out from the
//! same files.
//!
//! - X2 (a rule's preconditions where a workflow calls it): `refund` — the amount asked is kept
//!   below what was paid by the ranges (held), can pass it (E201), or comes from a task with no
//!   range (W201).
//! - X3 (b) (`range from koyomi`, rulec's own check): `settlement` — a rule over the days a koyomi
//!   date comes to, checked by `ritsu check` with koyomi joined, passing and with a payment day no
//!   row takes (rulec's E101 with that day).
//! - X3 (a) (the days of a koyomi date given to a rule): `billing` — the payment day given to a rule
//!   whose range holds every payment day (held), starts after the first (E202), or a day that can
//!   also be the day received (W202). With `range from koyomi`, the days are the rule's
//!   precondition (X2): the same date's days (held), the closing days (E201), the day received
//!   (W201).
//! - X4 (a rule's output as a transfer's amount): `booking` — the seats an event needs given to the
//!   hall at once: every kind fits (held), a handback gives 20 seats back (−20, E203: chobo takes 0
//!   to 2⁶³ − 1, so a negative amount is a transfer the other way), the seats can come from an
//!   answer with no range (W203), a concert needs more than the hall holds (E204), the task handles
//!   the hall being full, which no kind comes to (W204).
//! - X6 (the day given to a koyomi date): `reminding` — the payment day given to a reminder whose
//!   range holds every payment day (held), ends before the last (E205), or the day received (W205).
//! - X5 (a hold against its expiry): `invoice` — goods held three days, with timeouts (held), until
//!   the payment day at 09:00 when the hold lasts 14 days (E206), or 60 days (W206).
//!
//! - X15 (the operations a context opens, against the actions that guard them; sekisho's DESIGN
//!   4.6): `orders` — a map whose one context opens two operations of its OpenAPI document: both
//!   guarded (held), one guarded and the other open to anyone (`security: []`, held), one guarded and
//!   the other not (E907); and none guarded while Payments, beside it, guards its own (W907: Orders
//!   has written no authorization yet). A pair of Cedar written by hand guards one of them too,
//!   named by the map (`cedar "…"` under `owns`) or by a requirement (held).
//! - X16 (what a workflow calls, against what the gate that names it allows it): `returns` — the
//!   workflow refunds a returned order: allowed when the order has come back, and the task declares
//!   the error of a denial (held); allowed in no combination (E908); allowed in some, the task
//!   declaring nothing (W909); allowed to look at an order too, which it never does (W908).
//!
//! The day a flow's date call is given from its own input says nothing of what day it is (W205):
//! the projects of X3 (a) and X5 show it alongside what they are about.

use ritsu_testkit::TempDir;
use std::path::{Path, PathBuf};
use std::process::Command;

fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn ritsu_in(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
    c.current_dir(dir).args(args);
    for v in ["RITSU_LANG", "RULEC_LANG", "DANDORI_LANG", "KOYOMI_LANG"] {
        c.env_remove(v);
    }
    let o = c.output().expect("could not run ritsu");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

fn golden(name: &str, got: &str, failures: &mut Vec<String>) {
    if let Err(e) = ritsu_testkit::golden::check(&here().join("tests/golden/cross").join(name), got) {
        failures.push(e);
    }
}

/// The files of a reproduction in ritsu's ledger.
fn repro(code: &str) -> Vec<(&'static str, &'static str)> {
    let l = ritsu_cross::codes::ledger();
    match &l.find(code).expect("the code is in the ledger").repro {
        ritsu_base::ledger::Repro::Dir { files, .. } => files.clone(),
        other => panic!("{code} has no files: {other:?}"),
    }
}

fn lay_out(files: &[(&str, String)], tag: &str) -> TempDir {
    let t = TempDir::new(tag);
    for (name, body) in files {
        let p = t.path().join(name);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    }
    t
}

/// The text in both languages, the exit, and the JSON's `borders`, each held to its golden file.
fn run(name: &str, files: &[(&str, String)], want_code: i32, borders: (u64, u64, u64), failures: &mut Vec<String>) {
    let t = lay_out(files, name);
    for lang in ["en", "ja"] {
        let (code, out, err) = ritsu_in(t.path(), &["check", ".", "--lang", lang]);
        assert_eq!(code, want_code, "{name} ({lang}):\n{out}{err}");
        golden(&format!("{name}.{lang}.txt"), &out, failures);
    }
    let (_, out, _) = ritsu_in(t.path(), &["check", ".", "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let b = &v["borders"];
    assert_eq!((b["held"].as_u64(), b["failed"].as_u64(), b["undecided"].as_u64()), (Some(borders.0), Some(borders.1), Some(borders.2)), "{name}: {b}");
}

#[test]
fn x2_a_rules_preconditions_where_a_flow_calls_it() {
    let mut failures = Vec::new();
    let files = repro("E201");
    let rule = files.iter().find(|(n, _)| n.ends_with(".rule")).unwrap().1.to_string();
    let flow = files.iter().find(|(n, _)| n.ends_with(".flow")).unwrap().1.to_string();
    // held: what was paid is at least 5,000, and what is asked at most 5,000
    let held = flow.replace("paid  : int  range >=0 <=10000", "paid  : int  range >=5000 <=10000").replace("-> int range >=0 <=10000", "-> int range >=0 <=5000");
    run("x2-held", &[("refund_check.rule", rule.clone()), ("refund.flow", held)], 0, (1, 0, 0), &mut failures);
    // an example: what is asked can pass what was paid
    run("x2-E201", &[("refund_check.rule", rule.clone()), ("refund.flow", flow.clone())], 1, (0, 1, 0), &mut failures);
    // undecided: what is asked comes from a task whose answer has no range
    let open = repro("W201").iter().find(|(n, _)| n.ends_with(".flow")).unwrap().1.to_string();
    run("x2-W201", &[("refund_check.rule", rule.clone()), ("refund.flow", open)], 0, (0, 0, 1), &mut failures);
    // the same value on both sides of `<=` holds whatever it is
    let same = flow.replace("check(paid: paid, asked: asked)", "check(paid: asked, asked: asked)");
    run("x2-same-value", &[("refund_check.rule", rule), ("refund.flow", same)], 0, (1, 0, 0), &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn x3b_a_rule_over_koyomis_days() {
    let mut failures = Vec::new();
    let days = here().join("../rulec/tests/days");
    let cal = std::fs::read_to_string(days.join("payment_terms.cal")).unwrap();
    let rule = std::fs::read_to_string(days.join("settlement.rule")).unwrap();
    run("x3b-held", &[("payment_terms.cal", cal.clone()), ("settlement.rule", rule.clone())], 0, (1, 0, 0), &mut failures);
    let gap = rule.replace(">=2026-07-10 <=2026-12-10", ">=2026-07-11 <=2026-12-10").replace("| 2026-07-10 | second_half |\n", "");
    run("x3b-E101", &[("payment_terms.cal", cal), ("settlement.rule", gap)], 1, (0, 0, 0), &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A reproduction's files, each with a change made to it.
fn changed(code: &str, edits: &[(&str, &str, &str)]) -> Vec<(&'static str, String)> {
    repro(code)
        .into_iter()
        .map(|(name, body)| {
            let mut body = body.to_string();
            for (file, from, to) in edits {
                if *file == name {
                    assert!(body.contains(from), "{code}: {name} has no {from:?}");
                    body = body.replacen(from, to, 1);
                }
            }
            (name, body)
        })
        .collect()
}

#[test]
fn x3a_the_days_of_a_koyomi_date_given_to_a_rule() {
    let mut failures = Vec::new();
    let held = changed("E202", &[("batch.rule", ">=2026-03-01 <=2027-01-31", ">=2026-02-01 <=2027-01-31")]);
    run("x3a-held", &held, 0, (1, 0, 1), &mut failures);
    run("x3a-E202", &changed("E202", &[]), 1, (0, 1, 1), &mut failures);
    run("x3a-W202", &changed("W202", &[]), 0, (0, 0, 2), &mut failures);
    // with `range from koyomi`, the days are a precondition of the rule (X2), held a day at a time;
    // rulec checks the rule over the days itself (X3 (b)), one border more held
    let days = here().join("../rulec/tests/days");
    let rule = std::fs::read_to_string(days.join("settlement.rule")).unwrap();
    let flow = changed("E202", &[])
        .into_iter()
        .find(|(n, _)| *n == "billing.flow")
        .unwrap()
        .1
        .replace("use rule batch from \"batch.rule\"", "use rule settlement from \"settlement.rule\"")
        .replace("run: batch.run", "run: settlement.run")
        .replace("let pick = batch(pay_day: due.day)", "let pick = settlement(pay_day: due.day)")
        .replace("function:batch", "function:settlement");
    let cal = changed("E202", &[]).into_iter().find(|(n, _)| *n == "payment_terms.cal").unwrap().1;
    let files = |flow: String| vec![("payment_terms.cal", cal.clone()), ("settlement.rule", rule.clone()), ("billing.flow", flow)];
    run("x3a-days-held", &files(flow.clone()), 0, (2, 0, 1), &mut failures);
    let closing = flow.replace("let pick = settlement(pay_day: due.day)", "let close = terms.closing(received: received)\n  let pick = settlement(pay_day: close.day)");
    run("x3a-days-E201", &files(closing), 1, (1, 1, 2), &mut failures);
    let received = flow.replace("settlement(pay_day: due.day)", "settlement(pay_day: received)");
    run("x3a-days-W201", &files(received), 0, (1, 0, 2), &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn x4_a_rules_output_as_a_transfers_amount() {
    let mut failures = Vec::new();
    let held = changed("E204", &[("seats.rule", "enum kind = workshop | talk | concert", "enum kind = workshop | talk"), ("seats.rule", "| concert    | 400                |\n", "")]);
    run("x4-held", &held, 0, (2, 0, 0), &mut failures);
    run("x4-E203", &changed("E203", &[]), 1, (1, 1, 0), &mut failures);
    run("x4-W203", &changed("W203", &[]), 0, (0, 0, 2), &mut failures);
    run("x4-E204", &changed("E204", &[]), 1, (1, 1, 0), &mut failures);
    run("x4-W204", &changed("W204", &[]), 0, (1, 0, 1), &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn x6_the_day_given_to_a_koyomi_date() {
    let mut failures = Vec::new();
    run("x6-held", &changed("W205", &[]), 0, (1, 0, 1), &mut failures);
    run("x6-E205", &changed("E205", &[]), 1, (0, 1, 1), &mut failures);
    let open = changed("W205", &[("reminding.flow", "reminders.reminder(due: due.day)", "reminders.reminder(due: received)")]);
    run("x6-W205", &open, 0, (0, 0, 2), &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn x5_a_hold_against_its_expiry() {
    let mut failures = Vec::new();
    // held three days, every call with a timeout: the post comes before the 14 days are up
    let held = changed(
        "E206",
        &[
            ("invoice.flow", "use dates terms from \"payment_terms.cal\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:payment-terms\"\n", ""),
            ("invoice.flow", "  errors out_of_stock\n", "  errors out_of_stock\n  timeout 30 seconds\n"),
            ("invoice.flow", "  errors expired\n", "  errors expired\n  timeout 30 seconds\n"),
            ("invoice.flow", "  let due = terms.payment(received: now)\n  wait until due.at\n", "  wait 3 days\n"),
        ],
    );
    run("x5-held", &held, 0, (1, 0, 0), &mut failures);
    run("x5-E206", &changed("E206", &[]), 1, (0, 1, 1), &mut failures);
    run("x5-W206", &changed("W206", &[]), 0, (0, 0, 2), &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn x15_the_operations_a_context_opens() {
    let mut failures = Vec::new();
    // held: an action guards each operation
    let both = changed("E907", &[("orders/refunds.gate", "  guards orders refundOrder\n", "  guards orders refundOrder\n  guards orders getOrder\n")]);
    run("x15-held", &both, 0, (2, 0, 0), &mut failures);
    // held: the one not guarded is open to anyone, as its document says
    let open = changed("E907", &[("orders/api/orders.json", "\"operationId\": \"getOrder\",\n", "\"operationId\": \"getOrder\",\n        \"security\": [],\n")]);
    run("x15-open", &open, 0, (2, 0, 0), &mut failures);
    run("x15-E907", &changed("E907", &[]), 1, (1, 1, 0), &mut failures);
    // none of Orders' operations is guarded, and Payments guards its one
    run("x15-W907", &changed("W907", &[]), 0, (1, 0, 2), &mut failures);
    // held: the look at an order guarded by a pair of Cedar written by hand, which Orders names
    // under `owns`; its schema's `@guards` names the operation as a gate's `guards` does
    let mut named = changed("E907", &[("contexts/orders.ctx", "  dir \"../orders\"\n", "  dir \"../orders\"\n  cedar \"../orders/policies/orders.cedar\", \"../orders/policies/orders.cedarschema\"\n")]);
    named.extend(cedar_pair());
    run("x15-cedar", &named, 0, (2, 0, 0), &mut failures);
    // the same pair, named by a requirement instead of the map: read all the same (the link has no
    // record of a look yet, which yuen says; that is beside X15)
    let mut required = changed("E907", &[]);
    required.extend(cedar_pair());
    required.push(("orders.req", ORDERS_REQ.to_string()));
    run("x15-cedar-requirement", &required, 1, (2, 0, 0), &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A pair of Cedar written by hand for X15's project: who may look at an order.
fn cedar_pair() -> Vec<(&'static str, String)> {
    vec![
        (
            "orders/policies/orders.cedarschema",
            "namespace Orders {\n  entity User;\n  entity Order;\n\n  @guards(\"openapi \\\"orders/api/orders.json\\\" operation getOrder\")\n  action \"view_order\" appliesTo {\n    principal: [User],\n    resource: [Order],\n    context: {}\n  };\n}\n".to_string(),
        ),
        ("orders/policies/orders.cedar", "@id(\"staff_look\")\npermit (principal, action == Orders::Action::\"view_order\", resource);\n".to_string()),
    ]
}

/// A requirement the policy of [`cedar_pair`] meets.
const ORDERS_REQ: &str = "requirements orders v1\ndescription \"Who may look at an order\"\n\nrole orders \"Keeps the orders\"\n\nrequirement staff_look\n  text \"A member of the staff looks at an order\"\n  owner orders\n  decided 2026-10-06 by orders \"The staff answer the customers\"\n  satisfied by cedar \"orders/policies/orders.cedar\" policy staff_look\n  not verified \"Looked at by hand\"\n";

#[test]
fn x16_what_a_workflow_calls() {
    let mut failures = Vec::new();
    // held: allowed when the order has come back, and the task handles a denial
    let handled = changed(
        "W909",
        &[("returns.flow", "  key\n", "  errors denied = 403\n  key\n"), ("returns.flow", "  refund_order(orderId: order)\n", "  refund_order(orderId: order)\n    on denied => fail Denied \"the gate did not let the workflow refund\"\n")],
    );
    run("x16-held", &handled, 0, (1, 0, 0), &mut failures);
    run("x16-E908", &changed("E908", &[]), 1, (0, 1, 0), &mut failures);
    run("x16-W909", &changed("W909", &[]), 0, (0, 0, 1), &mut failures);
    run("x16-W908", &changed("W908", &[]), 0, (1, 0, 0), &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
