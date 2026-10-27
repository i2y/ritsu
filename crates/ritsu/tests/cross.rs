//! The checks across the languages, as `ritsu check` runs them on a project (DESIGN 7; PLAN E.4).
//! Each check has a project where it holds, and variants where there is an example that it does
//! not and where it cannot be decided; the text (English and Japanese) and the JSON's `borders`
//! are held to golden files in `tests/golden/cross/` (`RITSU_BLESS=1` writes them). The projects
//! are small and English: the ledger's reproductions (`ritsu_cross::codes`) are laid out from the
//! same files.
//!
//! - X1 (the unit where a value crosses, which each language holds itself; DESIGN 7.3), for the
//!   units written with a sign (DESIGN 5.1): `cold_chain` — a rule takes the temperature of a hold
//!   in ℃ and its humidity in steps of 1%, and a gate, a flow and a module of kaname give it each in
//!   the rule's units (held); in ℉, from the gate (sekisho's E102), the flow (dandori's E003) or the
//!   module (kaname's E102); in steps of 0.1%, from the gate or the flow. The same with Japanese
//!   names, in ℃ (held) and in ℉ from each.
//! - X2 (a rule's preconditions where a workflow calls it): `refund` — the amount asked is kept
//!   below what was paid by the ranges (held), can pass it (E201), or comes from a task with no
//!   range (W201). `quota` is the same with a sum on one side (`used + requested <= quota`).
//! - The range of a rule's input of an ordered enum (rulec's §15.251), which the languages that give
//!   the rule a value hold it to themselves, as they hold a number's: `lift` and `tiers` — a flow
//!   and a gate that give only values the range lets in (held), a flow that holds the campaign it is
//!   given to the range where it comes in and gives it as it is (held), the flow giving a campaign
//!   that can be platinum (dandori's E014), the gate giving one (sekisho's E206).
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
//!   the payment day at 09:00 when the hold lasts 14 days (E206), or 60 days (W206). The hold
//!   voided when an approval does not come in 15 days: an event, which only its own timer times
//!   out (E206), or a task, whose timeout can come sooner (W206).
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
//!   declaring nothing (W909); allowed to look at an order too, which it never does (W908). Its
//!   permit can read another action's answer (sekisho's `allowed`): the workflow refunds a returned
//!   order unless it would be allowed to have the refund reviewed, which it is for a disputed
//!   order (held, the answer counted with what decides it).
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

/// The files of a reproduction in ritsu's ledger, but for its module of kaname: the projects here
/// are the workflows', and the modules' calls are `kaname_calls_across_the_borders` (and the
/// reproductions' modules `tests/codes.rs`).
fn repro(code: &str) -> Vec<(&'static str, &'static str)> {
    let l = ritsu_cross::codes::ledger();
    match &l.find(code).expect("the code is in the ledger").repro {
        ritsu_base::ledger::Repro::Dir { files, .. } => files.iter().filter(|(n, _)| !n.ends_with(".kn")).cloned().collect(),
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

/// The project of the cold chain: a rule (its English version and its Japanese one), and a gate, a
/// flow and a module of kaname of each that give it the hold's temperature and humidity. The rule and
/// the gates are copies of sekisho's test material (`tests/gen`), the flows of dandori's
/// (`tests/fixtures`), which their own tests hold; the modules are this project's.
const COLD_CHAIN: &str = "tests/projects/cold_chain";

/// `s` with `from` replaced by `to`, which it holds.
fn swapped(s: &str, from: &str, to: &str) -> String {
    assert!(s.contains(from), "{from}");
    s.replace(from, to)
}

/// X1 for the units written with a sign (DESIGN 5.1, 7.3): the rule of the cold chain takes the
/// temperature of a hold as `temperature[℃]`, from -40℃, and its humidity as `rate[step 1%]`. A gate,
/// a flow and a module of kaname read ℃, ℉ and % with the characters ritsu-units reads a unit with,
/// and hold what they give the rule to its unit as they hold a unit written with letters: given in
/// the rule's units, every file passes; a temperature in ℉ is sekisho's E102, dandori's E003 and
/// kaname's E102, and a humidity in steps of 0.1% sekisho's E102 and dandori's E003. The same runs
/// with the Japanese versions.
#[test]
fn x1_the_units_written_with_a_sign_where_a_value_crosses() {
    let mut failures = Vec::new();
    let project = here().join(COLD_CHAIN);
    let read = |p: std::path::PathBuf| std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
    // the copies are the material sekisho's and dandori's tests hold
    for (copy, original) in [
        ("rules/cold_chain_departure.rule", "../sekisho/tests/gen/rules/cold_chain_departure.rule"),
        ("rules/出発の可否.rule", "../sekisho/tests/gen/rules/出発の可否.rule"),
        ("cold_chain.gate", "../sekisho/tests/gen/cold_chain.gate"),
        ("cold_chain.ja.gate", "../sekisho/tests/gen/cold_chain.ja.gate"),
        ("cold_chain.flow", "../dandori/tests/fixtures/cold_chain.flow"),
        ("cold_chain.ja.flow", "../dandori/tests/fixtures/cold_chain.ja.flow"),
    ] {
        assert_eq!(read(project.join(copy)), read(here().join(original)), "{COLD_CHAIN}/{copy} is a copy of {original}");
    }
    let one = |name: &str| read(project.join(name));
    let (rule, gate, flow, module) = (one("rules/cold_chain_departure.rule"), one("cold_chain.gate"), one("cold_chain.flow"), one("departure.kn"));
    let files = |gate: &str, flow: &str, module: &str| vec![("rules/cold_chain_departure.rule", rule.clone()), ("cold_chain.gate", gate.to_string()), ("cold_chain.flow", flow.to_string()), ("departure.kn", module.to_string())];
    run("x1-signs-held", &files(&gate, &flow, &module), 0, (0, 0, 0), &mut failures);
    let (celsius, fahrenheit) = ("temperature[℃]  range >=-40℃ <=40℃", "temperature[℉]  range >=-40℉ <=104℉");
    // the gate's attribute in ℉ (and its expectation's 8℃, which ℉ counts as no whole number, as 46℉)
    let gate_f = swapped(&swapped(&gate, celsius, fahrenheit), "temperature <= 8℃", "temperature <= 46℉");
    run("x1-signs-gate-fahrenheit", &files(&gate_f, &flow, &module), 1, (0, 0, 0), &mut failures);
    run("x1-signs-flow-fahrenheit", &files(&gate, &swapped(&flow, celsius, fahrenheit), &module), 1, (0, 0, 0), &mut failures);
    run("x1-signs-module-fahrenheit", &files(&gate, &flow, &swapped(&module, celsius, fahrenheit)), 1, (0, 0, 0), &mut failures);
    let (step, finer) = ("rate[step 1%]  range >=0% <=100%", "rate[step 0.1%]  range >=0% <=100%");
    run("x1-signs-gate-step", &files(&swapped(&gate, step, finer), &flow, &module), 1, (0, 0, 0), &mut failures);
    run("x1-signs-flow-step", &files(&gate, &swapped(&flow, step, finer), &module), 1, (0, 0, 0), &mut failures);
    // the Japanese versions
    let (rule, gate, flow, module) = (one("rules/出発の可否.rule"), one("cold_chain.ja.gate"), one("cold_chain.ja.flow"), one("departure.ja.kn"));
    let files = |gate: &str, flow: &str, module: &str| vec![("rules/出発の可否.rule", rule.clone()), ("cold_chain.ja.gate", gate.to_string()), ("cold_chain.ja.flow", flow.to_string()), ("departure.ja.kn", module.to_string())];
    run("x1-signs-held-ja", &files(&gate, &flow, &module), 0, (0, 0, 0), &mut failures);
    let gate_f = swapped(&swapped(&gate, celsius, fahrenheit), "温度 <= 8℃", "温度 <= 46℉");
    run("x1-signs-gate-fahrenheit-ja", &files(&gate_f, &flow, &module), 1, (0, 0, 0), &mut failures);
    run("x1-signs-flow-fahrenheit-ja", &files(&gate, &swapped(&flow, celsius, fahrenheit), &module), 1, (0, 0, 0), &mut failures);
    run("x1-signs-module-fahrenheit-ja", &files(&gate, &flow, &swapped(&module, celsius, fahrenheit)), 1, (0, 0, 0), &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
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

/// X2 with a sum on one side of the rule's `constraint` (rulec's §15.216): what the call gives the
/// inputs it adds up is taken at the ends that try the relation hardest, and their sum against the
/// other side's. Held, an example that breaks it (E201, every input of the sum named), and a value
/// with no range (W201).
#[test]
fn x2_a_sum_as_a_rules_precondition() {
    let rule = "rule quota_check v1\ndescription \"Whether a request is granted at once or reviewed. What was used and what is asked never come to more than the quota, which the rule takes for granted\"\n\nenum path = at_once | review\n\ninputs\n  used      : number  range >=0 <=10000\n  requested : number  range >=0 <=10000\n  quota     : number  range >=0 <=10000\n\nconstraint used + requested <= quota\n\noutputs\n  route : path\n\ntable pick\npolicy unique\n| requested | -> route : path |\n| <=100     | at_once         |\n| >100      | review          |\n".to_string();
    let flow = "workflow quota v1\ndescription \"Grants a request at once or sends it to review, as the rule decides\"\n\nuse rule check from \"quota_check.rule\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:quota-check\"\n\ninputs\n  account : string\n  quota   : int  range >=0 <=10000\n\ntask usage(account: string) -> int range >=0 <=10000\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:usage\"\n  idempotent\n\ntask ask_amount(account: string) -> int range >=0 <=10000\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:ask-amount\"\n  idempotent\n\ntask grant(account: string, amount: int range >=0 <=10000)\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:grant\"\n  key\n\nflow\n  let used = usage(account: account)\n  let asked = ask_amount(account: account)\n  let decision = check(used: used, requested: asked, quota: quota)\n  match decision.route\n    at_once => grant(account: account, amount: asked)\n    review => pass\n".to_string();
    let mut failures = Vec::new();
    // held: at most 5,000 used and 5,000 asked, against a quota of 10,000
    let held = flow
        .replace("quota   : int  range >=0 <=10000", "quota   : int  range >=10000 <=10000")
        .replace("task usage(account: string) -> int range >=0 <=10000", "task usage(account: string) -> int range >=0 <=5000")
        .replace("task ask_amount(account: string) -> int range >=0 <=10000", "task ask_amount(account: string) -> int range >=0 <=5000");
    assert_ne!(held, flow);
    run("x2-sum-held", &[("quota_check.rule", rule.clone()), ("quota.flow", held)], 0, (1, 0, 0), &mut failures);
    // an example: what was used and what is asked can come to more than the quota
    run("x2-sum-E201", &[("quota_check.rule", rule.clone()), ("quota.flow", flow.clone())], 1, (0, 1, 0), &mut failures);
    // undecided: what is asked comes from a task whose answer has no range
    let open = flow.replace("task ask_amount(account: string) -> int range >=0 <=10000", "task ask_amount(account: string) -> int");
    assert_ne!(open, flow);
    run("x2-sum-W201", &[("quota_check.rule", rule), ("quota.flow", open)], 0, (0, 0, 1), &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The range of a rule's input of an ordered enum, where a flow and a gate give it a value (rulec's
/// §15.251): `campaign : tier  range >=basic <=gold` reaches dandori's E014 and sekisho's E206
/// through the port, which hands over the values the range lets in. Held: the flow branches on its
/// campaign and writes out a value the range lets in, and the gate gives an attribute of an enum
/// that stops at gold, and gold as a constant. A flow that writes the rule's range on the campaign it
/// is given (dandori's DESIGN 1.20) gives it as it is, and passes too. The flow giving its own
/// campaign with no range, which can be platinum, is dandori's E014; the gate giving one is
/// sekisho's E206. The rule and the gate are sekisho's test material.
#[test]
fn the_range_of_an_enum_input_where_a_flow_and_a_gate_give_it() {
    let mut failures = Vec::new();
    let ranges = here().join("../sekisho/tests/walk/ranges");
    let rule = std::fs::read_to_string(ranges.join("campaign_lift.rule")).unwrap();
    let gate = std::fs::read_to_string(ranges.join("tiers.gate")).unwrap();
    let flow = "workflow lift v1\ndescription \"Ships an order at the tier its campaign lifts the member to, as the rule decides\"\n\nuse rule campaign_lift from \"campaign_lift.rule\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:campaign-lift\"\n\ninputs\n  member   : campaign_lift.tier\n  campaign : campaign_lift.tier\n\noutputs\n  lifted : bool\n\nflow\n  # the rule lifts a member as far as gold: a campaign of platinum is given as gold\n  match campaign\n    basic => let d = campaign_lift(member: member, campaign: basic)\n    silver => let d = campaign_lift(member: member, campaign: silver)\n    gold, platinum => let d = campaign_lift(member: member, campaign: gold)\n  succeed lifted = d.lifted\n".to_string();
    let files = |flow: &str, gate: &str| vec![("campaign_lift.rule", rule.clone()), ("lift.flow", flow.to_string()), ("tiers.gate", gate.to_string())];
    run("enum-range-held", &files(&flow, &gate), 0, (0, 0, 0), &mut failures);
    // the flow gives the campaign it is given
    let given = flow.replace(
        "  match campaign\n    basic => let d = campaign_lift(member: member, campaign: basic)\n    silver => let d = campaign_lift(member: member, campaign: silver)\n    gold, platinum => let d = campaign_lift(member: member, campaign: gold)\n",
        "  let d = campaign_lift(member: member, campaign: campaign)\n",
    );
    assert_ne!(given, flow);
    run("enum-range-E014", &files(&given, &gate), 1, (0, 0, 0), &mut failures);
    // the flow holds the campaign it is given to the rule's range where it comes in, and gives it as it is
    let on_input = given.replace("  campaign : campaign_lift.tier\n", "  campaign : campaign_lift.tier  range >=basic <=gold\n");
    assert_ne!(on_input, given);
    run("enum-range-on-input", &files(&on_input, &gate), 0, (0, 0, 0), &mut failures);
    // the gate gives a campaign of an enum that goes on to platinum
    let past = gate.replace("    campaign : campaign_tier\n", "    campaign : tier\n");
    assert_ne!(past, gate);
    run("enum-range-E206", &files(&flow, &past), 1, (0, 0, 0), &mut failures);
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
    // the hold voided when an approval does not come in 15 days: waited for as an event, which only
    // its own timer times out, the void comes 15 days or more after the hold; a task's timeout can
    // come sooner (on Temporal, as its heartbeats stop), and the void at any time
    let approval = |event: &str| {
        changed(
            "E206",
            &[
                ("invoice.flow", "use dates terms from \"payment_terms.cal\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:payment-terms\"\n", ""),
                ("invoice.flow", "  errors out_of_stock\n", "  errors out_of_stock\n  timeout 30 seconds\n"),
                (
                    "invoice.flow",
                    "case goods",
                    &format!("task approval()\n{event}  timeout 15 days\n\ntask put_back(order: string, sku: string) -> stock.reserve\n  book stock.reserve.void\n  sends void\n  errors expired\n  timeout 30 seconds\n\ncase goods"),
                ),
                (
                    "invoice.flow",
                    "  let due = terms.payment(received: now)\n  wait until due.at\n",
                    "  approval()\n    on timeout =>\n      goods <- put_back(order: order, sku: sku)\n        on expired => pass\n      fail NotApproved \"no one approved the order in time\"\n",
                ),
            ],
        )
    };
    run("x5-event-E206", &approval("  event\n"), 1, (0, 1, 1), &mut failures);
    run("x5-task-W206", &approval(""), 0, (0, 0, 2), &mut failures);
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
    // held, with the workflow's permit reading another action's answer: it refunds a returned
    // order unless it would be allowed to have the refund reviewed, which it is for a disputed one
    let allowed = changed(
        "W909",
        &[
            ("returns.flow", "  key\n", "  errors denied = 403\n  key\n"),
            ("returns.flow", "  refund_order(orderId: order)\n", "  refund_order(orderId: order)\n    on denied => fail Denied \"the gate did not let the workflow refund\"\n"),
            ("refunds.gate", "    status : order_status\n", "    status   : order_status\n    disputed : bool\n"),
            (
                "refunds.gate",
                "\npermit clerks_refund\n",
                "\naction review_refund\n  description \"Have a refund looked at by a person before it is made\"\n  principal User, Workflow\n  resource Order from orderId\n\npermit clerks_refund\n",
            ),
            (
                "refunds.gate",
                "  description \"The returns workflow refunds an order that has come back\"\n  principal is workflow returns\n  action refund_order\n  when resource.status is returned\n",
                "  description \"The returns workflow refunds an order that has come back, unless the refund is to be reviewed\"\n  principal is workflow returns\n  action refund_order\n  when resource.status is returned\n  unless allowed review_refund\n\npermit returns_has_disputed_refunds_reviewed\n  description \"The returns workflow has the refund of a disputed order reviewed\"\n  principal is workflow returns\n  action review_refund\n  when resource.disputed\n",
            ),
        ],
    );
    run("x16-allowed", &allowed, 0, (1, 0, 0), &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// One of kaname's examples (its DESIGN 17), English only: the module with `edits` made to it
/// (the module's own examples dropped where it is changed), and the files it reads, with the copies
/// of the sources a rule cites.
fn kaname_example(name: &str, files: &[&'static str], edits: &[(&str, &str)]) -> Vec<(&'static str, String)> {
    let dir = here().join("../kaname/examples").join(name);
    let mut kn = std::fs::read_to_string(dir.join(format!("{name}.kn"))).unwrap();
    for (from, to) in edits {
        assert!(kn.contains(from), "{name} has no {from:?}");
        kn = kn.replacen(from, to, 1);
    }
    if !edits.is_empty() {
        kn = kn.split("\nexamples\n").next().unwrap().to_string() + "\n";
    }
    let module: &'static str = Box::leak(format!("{name}.kn").into_boxed_str());
    let mut out = vec![(module, kn)];
    for f in files {
        out.push((*f, std::fs::read_to_string(dir.join(f)).unwrap()));
    }
    out
}

/// The checks across the borders where a module of kaname calls a rule, a koyomi date or a transfer
/// (kaname's DESIGN 14.3; PLAN C3.7): the same codes as a workflow's, with an example only where a
/// run of the function's vectors breaks a check, and the borders counted with the others. `settle`
/// and `split` are written `total`, so a check that is not shown fails kaname's own check too (E401).
///
/// - X4: the payout's amounts and the refusals of its two operations (held); the fee taken from the
///   amount the seller gets, which a payment of a cent takes below 0 (E203); the same with payments
///   of a dollar or more, which no run takes below 0 (W203).
/// - X2: the running totals keep the discount rule's preconditions (held); `upto` put together by
///   `reduce` keeps no relation, and no run breaks them (W201); `before` and `upto` swapped (E201).
/// - X3 (a) and X6: the payment day of koyomi given to a rule whose range starts after the first of
///   them (E202), and to a reminder whose range ends before the last (E205).
#[test]
fn kaname_calls_across_the_borders() {
    let mut failures = Vec::new();
    let payout = ["rules/card_fee.rule", "rules/sources/paypal-us-fees.md", "rules/sources/paypal-us-fees.md.fragments/table1.tsv", "books/ledger.book"];
    run("kaname-x4-held", &kaname_example("payout", &payout, &[]), 0, (4, 0, 0), &mut failures);
    let net = ("amount: payment.amount),", "amount: payment.amount - fee),");
    run("kaname-x4-E203", &kaname_example("payout", &payout, &[net]), 1, (3, 1, 0), &mut failures);
    let dollar = ("range >=1USDc <=100_000_000USDc", "range >=100USDc <=100_000_000USDc");
    run("kaname-x4-W203", &kaname_example("payout", &payout, &[net, dollar]), 1, (3, 0, 1), &mut failures);
    let split = ["rules/discount_share.rule", "rules/sources/23vac10-210-250.md"];
    run("kaname-x2-held", &kaname_example("discount_split", &split, &[]), 0, (2, 0, 0), &mut failures);
    let reduce = ("upto: r.upto", "upto: reduce([r.item.amount], r.before, (acc, x) => acc + x)");
    run("kaname-x2-W201", &kaname_example("discount_split", &split, &[reduce]), 1, (0, 0, 2), &mut failures);
    let swapped = ("before: r.before, upto: r.upto", "before: r.upto, upto: r.before");
    run("kaname-x2-E201", &kaname_example("discount_split", &split, &[swapped]), 1, (1, 1, 0), &mut failures);
    let file = |code: &str, name: &str| repro(code).into_iter().find(|(n, _)| *n == name).unwrap().1.to_string();
    let billing = "module billing v1\ndescription \"The billing batch of the day an order's payment is due, and the day to remind the customer\"\n\nuse dates terms from \"payment_terms.cal\"\nuse dates reminders from \"reminders.cal\"\nuse rule batch from \"batch.rule\"\n\nfn pick(received: date range >=2026-01-01 <=2026-12-20) -> batch.run\n  let due = terms.payment(received: received)\n  batch(pay_day: due).batch\n\nfn remind(received: date range >=2026-01-01 <=2026-12-20) -> date\n  let due = terms.payment(received: received)\n  reminders.reminder(due: due)\n";
    let files = vec![
        ("payment_terms.cal", file("E202", "payment_terms.cal")),
        ("batch.rule", file("E202", "batch.rule")),
        ("reminders.cal", file("E205", "reminders.cal")),
        ("billing.kn", billing.to_string()),
    ];
    run("kaname-x3a-x6-E202-E205", &files, 1, (2, 2, 0), &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
