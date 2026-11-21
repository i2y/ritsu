//! The checks across the languages, as `ritsu check` runs them on a project (DESIGN 7; PLAN E.4).
//! Each check has a project where it holds, and variants where there is an example that it does
//! not and where it cannot be decided; the text (English and Japanese) is held to golden files in
//! `tests/golden/cross/` (`RITSU_BLESS=1` writes them), and the JSON's `borders` to the counts, with
//! a record of each border that agrees with them (DESIGN 8.3). The projects are small and English:
//! the ledger's reproductions (`ritsu_cross::codes`) are laid out from the same files. One of them,
//! E202's, whose rule, dates file, flow and module of kaname cross four languages, has the records
//! of its borders held to a golden file too.
//!
//! - X1 (the unit where a value crosses, which each language holds itself; DESIGN 7.3), for the
//!   units written with a sign (DESIGN 5.1): `cold_chain` — a rule takes the temperature of a hold
//!   in ℃ and its humidity in steps of 1%, and a gate, a flow and a module of kaname give it each in
//!   the rule's units (held); in ℉, from the gate (sekisho's E102), the flow (dandori's E003) or the
//!   module (kaname's E102); in steps of 0.1%, from the gate or the flow. The same with Japanese
//!   names, in ℃ (held) and in ℉ from each.
//! - X2 (a rule's preconditions where a workflow calls it): `refund` — the amount asked is kept
//!   below what was paid by the ranges (held), can pass it (E201), comes from a task with no range
//!   (W201), or is what another rule's `result` comes to, with no range written on its line, which
//!   can pass it too (E201 with the range of the `result`, rulec's §15.271); where the amount asked
//!   is a variable given a small amount first and a task's answer in one arm of a `match`, the call
//!   in the other arm is held and the call in that arm is E201. `quota` is the same
//!   with a sum on one side (`used + requested <= quota`). `pay` is the range a fee the rule works
//!   out is accepted in (rulec's §15.313): the amounts the flow and the module give keep it inside
//!   (held), can take it past its top (E201), or come from a task with no range (W201); and a
//!   module's lines keep the subtotal each line works out inside the range accepted for it (held),
//!   or can take it past (E201).
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
//!   answer with no range (W203), the seats still to give, which a constraint keeps at 0 or more
//!   though rulec counts them from −50 (W203: none of the rule's vectors comes to a negative
//!   amount), the same with five seats kept back (−5, E203: the fewest the rule's vectors come to,
//!   though the low end of the range, −55, is never reached), a concert needs more than the hall
//!   holds (E204), the task handles the hall being full, which no kind comes to (W204).
//! - X6 (the day given to a koyomi date): `reminding` — the payment day given to a reminder whose
//!   range holds every payment day (held), ends before the last (E205), or the day received (W205);
//!   where the day is a variable given the payment day first and the day received in one arm of a
//!   `match`, the call in the other arm is held and the call in that arm is W205.
//! - X5 (a hold against its expiry): `invoice` — goods held three days, with timeouts (held), until
//!   the payment day at 09:00 when the hold lasts 14 days (E206), or 60 days (W206). The hold
//!   voided when an approval does not come in 15 days: an event, which only its own timer times
//!   out (E206), or a task, whose timeout can come sooner (W206). The hold voided when the approval
//!   does not come by the payment day at 09:00 (`timeout until`): an event (E206), a callback, whose
//!   id is handed on first (W206, the day that makes the wait the longest in a note), the same with
//!   a hold of 60 days (held), and a callback waiting until a time an input gives (W206: nothing
//!   bounds it). A callback answered within 59 minutes between a hold of an hour and the post: the
//!   minute it may take to hand its id on makes the post come up to 20 seconds after the hold
//!   expires (W206).
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
//!   order (held, the answer counted with what decides it). Each call is asked over what the runs
//!   that reach it can come to. The amount the call gives is the action's input: up to 50 pounds
//!   where the permit lets in up to 50 (held, the task declaring nothing), where it lets in more
//!   than 100 only (E908), and up to 100 where it lets in up to 50 (W909, with amounts the flow
//!   gives); where the amount is a variable given up to 50 first and up to 10,000 in one arm of a
//!   `match`, the call in the other arm is held and the call in that arm is W909. A rule says who refunds, and the flow refunds under an arm of its answer; the gate
//!   reads the same rule of the order's total, which a `resource` line under the workflow says is
//!   the flow's amount: under the arm the gate allows (held where the task declares the error;
//!   W909 where it does not, since nothing checks the line of the order the service reads), under
//!   the other (E908, the error declared or not), and with no line (W909: nothing says what the
//!   flow holds of the total). An argument the gate's input counts in another unit or step (E909):
//!   a fee the task gives in steps of 0.1% and the gate reads in steps of 1%, the error of a denial
//!   declared or not, with E908 beside it where no permit allows the workflow, and with the
//!   Japanese names (the input taking the argument by its alias); the same steps pass; an amount the
//!   gate counts in pence, or without the tax.
//! - X18 (a request's route, against the gate the request reads; ringi's DESIGN 9.3): `purchase` —
//!   a request that lets no requester decide their own request, and a gate with the same rule
//!   (held); the gate without it (E911, with a combination it allows); a route that allows self
//!   approval, with the gate that has no rule against it (held) and with one that forbids it
//!   whatever the route says (E911); a gate that lets only the requester answer a stage (E911, the
//!   approvers allowed in no combination); a gate with no action for an operation, one and two of
//!   them (E911, with the line of the request that gives the tickets the operation), `see` among
//!   them (E911, with who sees a ticket without it); a gate whose action for an operation takes
//!   inputs of its own, which the server has nothing to give (E911, one action and two); a gate that
//!   declares `self_approval` as an enum (W911). The same in Japanese names (E911).
//! - X19 (a hold a ticket makes, against the book it is made in): `purchase` — a round of ten
//!   business days and a hold of thirty days (held), of seven days (E209, with the day that makes it
//!   the longest), one that never expires (held); a request whose stages send back to an earlier
//!   stage, the round walked three times (E209); a stage with no deadline (W209); the days counted
//!   in two calendars (W209). With an organization: a hold that takes the requester's department,
//!   which the president has none of (W209, with who), the same line kept to the requesters who
//!   have one (held), and an organization with no data (W209).
//! - X20 (what a ticket gives a flow, against what the flow takes): `purchase` starts the order's
//!   flow with an amount up to 1,000,000 JPY — the flow takes it (held), takes half (E210), the
//!   `on` line's `when` keeps it to what the flow takes (held) or to more (E210, with the range
//!   where the `when` holds), a `when` ringi does not follow (W210, beside the other side's W210 of
//!   the ledger's reproduction); a call with no value for an
//!   input, a value the flow has no input for, and a value of another type (E210). An agent's flow
//!   for the small amounts, by the rows of the route (held), which takes less than its rows give
//!   (E210), takes a field a request may leave empty, one the form does not have or one of another
//!   type (E210), or does not answer `reason` (E210). A summary's flow at a stage with a `when` (held), without it
//!   (E210), with an `or` (W210), with no output (E210), and with outputs a note holds or does not:
//!   rows (held), a record on its own, a list of strings, a `json` (E210).
//! - X20 on the side where a flow opens a ticket: `restock` submits a purchase through the API
//!   `ritsu gen --target requests` wrote (held); sends a value of another type, or a range wider than
//!   the form's (dandori's E016 against the document, and nothing more); the form narrowed, the
//!   version raised or the request renamed after the document was written (E210, with where they
//!   differ); the request's file gone (W210). `ritsu gen` stops for the document out of date when it
//!   writes another target; when it writes `requests`, it holds the flow to the document it writes
//!   (dandori's E016 where the flow sends what the request no longer takes, and nothing written).
//! - X18 to X20 say the same of a request however it is given: by its name, by its absolute path
//!   and found in its directory, the gate, the book and the flow beside it or in a directory beside
//!   its own, which a request given by its name in a directory that is no project's reaches outside
//!   the root.
//!
//! The day a flow's date call is given from its own input says nothing of what day it is (W205):
//! the projects of X3 (a) and X5 show it alongside what they are about.

mod common;

use ritsu_testkit::TempDir;
use std::path::{Path, PathBuf};
use std::process::Command;

fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn ritsu_in(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_ritsu"));
    c.current_dir(dir).args(args);
    for v in common::LANG_VARS {
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

/// The files of a code's reproduction with Japanese names. A code without one fails here, rather
/// than running its reproduction with English names under a Japanese test's name.
fn repro_ja(code: &str) -> Vec<(&'static str, String)> {
    let l = ritsu_cross::codes::ledger();
    match l.find(code).expect("the code is in the ledger").repro_ja.as_ref() {
        Some(ritsu_base::ledger::Repro::Dir { files, .. }) => files.iter().map(|(n, b)| (*n, b.to_string())).collect(),
        other => panic!("{code} has no files in Japanese names: {other:?}"),
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
    run_at(name, t.path(), want_code, borders, failures);
}

/// [`run`], of the project at `dir`.
fn run_at(name: &str, dir: &Path, want_code: i32, borders: (u64, u64, u64), failures: &mut Vec<String>) {
    for lang in ["en", "ja"] {
        let (code, out, err) = ritsu_in(dir, &["check", ".", "--lang", lang]);
        assert_eq!(code, want_code, "{name} ({lang}):\n{out}{err}");
        golden(&format!("{name}.{lang}.txt"), &out, failures);
    }
    let (_, out, _) = ritsu_in(dir, &["check", ".", "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let b = &v["borders"];
    assert_eq!((b["held"].as_u64(), b["failed"].as_u64(), b["undecided"].as_u64()), (Some(borders.0), Some(borders.1), Some(borders.2)), "{name}: {b}");
    records_agree(name, &v);
}

/// Where a check's records come in their order (`ritsu_cross::Check::ALL`).
fn rank(check: &str) -> usize {
    ritsu_cross::Check::ALL.iter().position(|c| c.word() == check).unwrap_or_else(|| panic!("`{check}` is no check"))
}

/// What the JSON of `ritsu check` says of each border (DESIGN 8.3): as many records as the counts
/// say, each result as many times as its count, in the order of the checks, then of the file and
/// the line of the place; each with its keys in their order, a code where it does not hold and none
/// where it does, and that code among what ritsu says in the file of the place.
fn records_agree(name: &str, v: &serde_json::Value) {
    let b = &v["borders"];
    let records = b["records"].as_array().unwrap_or_else(|| panic!("{name}: no records: {b}"));
    let sum: u64 = ["held", "failed", "undecided"].iter().map(|k| b[k].as_u64().unwrap()).sum();
    assert_eq!(records.len() as u64, sum, "{name}: {b}");
    for k in ["held", "failed", "undecided"] {
        assert_eq!(Some(records.iter().filter(|r| r["result"] == k).count() as u64), b[k].as_u64(), "{name}: {k}: {b}");
    }
    let key = |r: &serde_json::Value| (rank(r["check"].as_str().unwrap()), r["at"]["file"].as_str().map(str::to_string), r["at"]["line"].as_u64());
    assert!(records.windows(2).all(|w| key(&w[0]) <= key(&w[1])), "{name}: the records are out of their order: {b}");
    let diagnostics = v["diagnostics"].as_array().unwrap();
    for r in records {
        let keys: Vec<&str> = r.as_object().unwrap().keys().map(String::as_str).collect();
        assert_eq!(keys[..6], ["check", "about", "at", "to", "result", "code"], "{name}: {r}");
        for side in [&r["at"], &r["to"]].into_iter().filter(|s| !s.is_null()) {
            let keys: Vec<&str> = side.as_object().unwrap().keys().map(String::as_str).collect();
            assert_eq!(keys, ["tool", "file", "line"], "{name}: {r}");
        }
        assert_eq!(r["code"].is_null(), r["result"] == "held", "{name}: {r}");
        if let Some(code) = r["code"].as_str() {
            let said = diagnostics.iter().any(|d| d["tool"] == "ritsu" && d["code"] == code && d["file"] == r["at"]["file"]);
            assert!(said, "{name}: ritsu says no {code} in {}: {r}", r["at"]["file"]);
        }
    }
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
    // undecided: what was paid is an input of the workflow with no range (the Japanese note puts a
    // space after the input's name, as dandori's W104 does)
    let unpaid = flow.replace("paid  : int  range >=0 <=10000", "paid  : int");
    run("x2-W201-input", &[("refund_check.rule", rule.clone()), ("refund.flow", unpaid)], 0, (0, 0, 1), &mut failures);
    // an example: what is asked is what another rule's `result` comes to, with no range written on
    // its line, which dandori reads from the rule's facts (rulec's §15.271): up to 9,000
    let share = "rule refund_share v1\ndescription \"What a refund asks for: the price of an item times the items sent back\"\n\ninputs\n  price : number  range >=0 <=3000\n  items : number  range >=0 <=3\n\noutputs\n  asked : number  round down(1)\n\nresult asked = price * items\n\nexamples\n| price | items | -> asked |\n| 1000  | 2     | 2000     |\n".to_string();
    let by_rule = flow
        .replace("task ask_amount(order: string) -> int range >=0 <=10000\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:ask-amount\"\n  idempotent\n\n", "")
        .replace("  paid  : int  range >=0 <=10000\n", "  paid  : int  range >=0 <=10000\n  price : int  range >=0 <=3000\n  items : int  range >=0 <=3\n")
        .replace("use rule check from", "use rule share from \"refund_share.rule\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:refund-share\"\nuse rule check from")
        .replace("  let asked = ask_amount(order: order)\n", "  let shared = share(price: price, items: items)\n  let asked = shared.asked\n");
    assert!(!by_rule.contains("ask_amount") && by_rule.contains("refund_share.rule"), "{by_rule}");
    run("x2-result-E201", &[("refund_check.rule", rule.clone()), ("refund_share.rule", share), ("refund.flow", by_rule)], 1, (0, 1, 0), &mut failures);
    // a variable given a value in two places is, where the rule is called, what the runs that reach
    // the call last put in it (dandori's DESIGN 1.3): `asked` is a small amount first, and what the
    // task answers in one arm of a `match`
    let body = "  let asked = ask_amount(order: order)\n  let decision = check(paid: paid, asked: asked)\n  match decision.route\n    at_once => pay_back(order: order, amount: asked)\n    review => pass\n";
    let call = "      let decision = check(paid: paid, asked: asked)\n      match decision.route\n        at_once => pay_back(order: order, amount: asked)\n        review => pass\n";
    let rebound = |arms: String| {
        let inputs = swapped(&flow, "  paid  : int  range >=0 <=10000\n", "  quick : bool\n  paid  : int  range >=5000 <=10000\n  small : int  range >=0 <=5000\n");
        swapped(&inputs, body, &format!("  let asked = small\n  match quick\n{arms}"))
    };
    // held: the rule is called in the other arm, where `asked` is still the small amount
    let other_arm = rebound(format!("    true =>\n{call}    false =>\n      let asked = ask_amount(order: order)\n      pass\n"));
    run("x2-rebound-held", &[("refund_check.rule", rule.clone()), ("refund.flow", other_arm)], 0, (1, 0, 0), &mut failures);
    // an example: the rule is called in the arm that asks the task
    let same_arm = rebound(format!("    true => pass\n    false =>\n      let asked = ask_amount(order: order)\n{call}"));
    run("x2-rebound-E201", &[("refund_check.rule", rule.clone()), ("refund.flow", same_arm)], 1, (0, 1, 0), &mut failures);
    // the same value on both sides of `<=` holds whatever it is
    let same = flow.replace("check(paid: paid, asked: asked)", "check(paid: asked, asked: asked)");
    run("x2-same-value", &[("refund_check.rule", rule), ("refund.flow", same)], 0, (1, 0, 0), &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A rule with `never` lines where a workflow calls it (rulec's §15.282).
const NEVER_RULE: &str = "rule refund_route v1\ndescription \"Whether a refund is paid back at once. An order with nothing shipped has nothing to refund, and an express order of 50 or more is held back before it is made.\"\n\nenum path = at_once | review\n\ninputs\n  shipped : number  range >=0 <=100\n  asked   : number  range >=0 <=10000\n  express : bool\n\nnever shipped = 0, asked >= 1\nnever express = true, shipped >= 50\n\noutputs\n  route : path\n\ntable pick\npolicy unique\n| asked | -> route : path |\n| <=100 | at_once         |\n| >100  | review          |\n";
const NEVER_FLOW: &str = "workflow refund v1\ndescription \"Pays a refund back at once or sends it to review, as the rule decides\"\n\nuse rule check from \"refund_route.rule\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:refund-route\"\n\ninputs\n  order   : string\n  shipped : int  range >=1 <=100\n  express : bool\n\ntask ask_amount(order: string) -> int range >=0 <=10000\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:ask-amount\"\n  idempotent\n\ntask pay_back(order: string, amount: int range >=0 <=10000)\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:pay-back\"\n  key\n\nflow\n  let asked = ask_amount(order: order)\n  let decision = check(shipped: shipped, asked: asked, express: express)\n  match decision.route\n    at_once => pay_back(order: order, amount: asked)\n    review => pass\n";
const NEVER_MODULE: &str = "module refund v1\ndescription \"Whether a refund is paid at once or reviewed, as the rule decides for what was shipped and what the customer asks for\"\n\nuse rule check from \"refund_route.rule\"\n\nfn route(shipped: number range >=1 <=49, asked: number range >=0 <=10000, express: bool) -> check.path\n  check(shipped: shipped, asked: asked, express: express).route\n";

/// X2 with a rule's `never` lines (rulec's §15.282). A number that cannot meet its condition shows
/// a line holds, whatever the truth value beside it; numbers that can all meet theirs are an example
/// (E201); a truth value that can be either, or a number with no range, leaves it undecided (W201),
/// and the workflow checks it when it runs (`crates/dandori/tests/flows/never_lines.flow`). A truth
/// value the flow writes out, or gives in the arm of a `match` on it, is that value at the call: it
/// keeps the line when it does not meet its condition, and with the numbers that can meet theirs it
/// is an example when it does; the example's note says the value and what to give instead. So is a
/// variable a truth value was put in, by what the runs that reach the call last put in it (one
/// value, or either where the ways that reach the call put in both). kaname holds its calls to the
/// lines by their numbers.
#[test]
fn x2_a_rules_never_lines_where_a_call_gives_it_values() {
    let mut failures = Vec::new();
    let rule = NEVER_RULE.to_string();
    let files = |flow: &str| vec![("refund_route.rule", rule.clone()), ("refund.flow", flow.to_string())];
    let few = NEVER_FLOW.replace("shipped : int  range >=1 <=100", "shipped : int  range >=1 <=49");
    run("x2-never-held", &files(&few), 0, (2, 0, 0), &mut failures);
    run("x2-never-truth-W201", &files(NEVER_FLOW), 0, (1, 0, 1), &mut failures);
    let none = NEVER_FLOW.replace("shipped : int  range >=1 <=100", "shipped : int  range >=0 <=49");
    run("x2-never-E201", &files(&none), 1, (1, 1, 0), &mut failures);
    let open = NEVER_FLOW.replace("shipped : int  range >=1 <=100", "shipped : int");
    run("x2-never-W201", &files(&open), 0, (0, 0, 2), &mut failures);
    let call = "check(shipped: shipped, asked: asked, express: express)";
    run("x2-never-written-held", &files(&NEVER_FLOW.replace(call, "check(shipped: shipped, asked: asked, express: false)")), 0, (2, 0, 0), &mut failures);
    run("x2-never-written-E201", &files(&NEVER_FLOW.replace(call, "check(shipped: shipped, asked: asked, express: true)")), 1, (1, 1, 0), &mut failures);
    // every condition met by a value written out: the note says what to give instead, and no branch helps
    run("x2-never-all-written-E201", &files(&NEVER_FLOW.replace(call, "check(shipped: 60, asked: asked, express: true)")), 1, (1, 1, 0), &mut failures);
    let body = "  let decision = check(shipped: shipped, asked: asked, express: express)\n  match decision.route\n    at_once => pay_back(order: order, amount: asked)\n    review => pass\n";
    let arms = |calls: &str, passes: &str| format!("  match express\n    {passes} => pass\n    {calls} =>\n      let decision = check(shipped: shipped, asked: asked, express: express)\n      match decision.route\n        at_once => pay_back(order: order, amount: asked)\n        review => pass\n");
    assert!(NEVER_FLOW.contains(body));
    run("x2-never-arm-held", &files(&NEVER_FLOW.replace(body, &arms("false", "true"))), 0, (2, 0, 0), &mut failures);
    run("x2-never-arm-E201", &files(&NEVER_FLOW.replace(body, &arms("true", "false"))), 1, (1, 1, 0), &mut failures);
    // a variable a truth value is put in, given on
    let given = body.replace("express: express", "express: fast");
    let put = |puts: &str| NEVER_FLOW.replace(body, &format!("{puts}{given}"));
    run("x2-never-variable-held", &files(&put("  let fast = false\n")), 0, (2, 0, 0), &mut failures);
    run("x2-never-variable-E201", &files(&put("  let fast = true\n")), 1, (1, 1, 0), &mut failures);
    // put in on one way only: the runs that reach the call can bring either value
    let both = "  let fast = false\n  match express\n    true =>\n      let fast = true\n      pass\n    false => pass\n";
    run("x2-never-variable-W201", &files(&put(both)), 0, (1, 0, 1), &mut failures);
    // the call on the way that puts in false keeps the line
    let on_the_way = format!("  let fast = false\n  match express\n    true =>\n      let fast = true\n      pass\n    false =>\n{}", given.lines().map(|l| format!("    {l}\n")).collect::<String>());
    run("x2-never-variable-arm-held", &files(&NEVER_FLOW.replace(body, &on_the_way)), 0, (2, 0, 0), &mut failures);
    let module = |text: &str| vec![("refund_route.rule", rule.clone()), ("refund.kn", text.to_string())];
    run("kaname-x2-never-held", &module(NEVER_MODULE), 0, (2, 0, 0), &mut failures);
    let none = NEVER_MODULE.replace("shipped: number range >=1 <=49", "shipped: number range >=0 <=49");
    run("kaname-x2-never-E201", &module(&none), 1, (1, 1, 0), &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A rule that works out a fee and accepts it only up to 20,000 (rulec's §15.313), where a workflow
/// and a module of kaname call it; and one that works out a subtotal on each line it walks.
const DEFINE_RULE: &str = "rule card_fee v1\ndescription \"The fee for paying by card: 3% of the amount, which the card service takes only up to 20,000\"\n\ninputs\n  amount : number  range >=1 <=1_000_000\n\ndefine fee : number = down(amount * 3%, 1)  range >=0 <=20_000\n\noutputs\n  fee : number  round down(1)\n";
const DEFINE_FLOW: &str = "workflow pay v1\ndescription \"Charges a card payment with its fee, as the rule works it out\"\n\nuse rule card_fee from \"card_fee.rule\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:card-fee\"\n\ninputs\n  order : string\n\ntask amount_of(order: string) -> int range >=1 <=500000\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:amount-of\"\n  idempotent\n\ntask charge(order: string, fee: int range >=0 <=20000)\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:charge\"\n  key\n\nflow\n  let amount = amount_of(order: order)\n  let worked = card_fee(amount: amount)\n  charge(order: order, fee: worked.fee)\n";
const DEFINE_MODULE: &str = "module pay v1\ndescription \"The fee for paying by card, as the rule works it out\"\n\nuse rule card_fee from \"card_fee.rule\"\n\nfn fee_of(amount: number range >=1 <=500000) -> number\n  card_fee(amount: amount).fee\n";
const LINES_RULE: &str = "rule order_lines v1\ndescription \"The amount of an order. A line's subtotal is its quantity times its price, and one line is accepted up to 10,000,000\"\n\nelements lines\n  quantity : number  range >=1 <=10_000\n  price    : number  range >=1 <=50_000_000\n\ndefine subtotal : number = quantity * price  range >=1 <=10_000_000\n\nsum amount over lines of subtotal  range >=1 <=100_000_000\n\noutputs\n  amount : number  round down(1)\n";
const LINES_MODULE: &str = "module order v1\ndescription \"The amount of an order, as the rule works it out\"\n\nuse rule order_lines from \"order_lines.rule\"\n\nrecord Line\n  quantity : number  range >=1 <=100\n  price    : number  range >=1 <=1000\n\nfn amount_of(lines: list[Line] count >=1 <=10) -> number\n  order_lines(lines: map(lines, l => order_lines.lines { quantity: l.quantity, price: l.price })).amount\n";

/// X2 with the range a value the rule works out is accepted in (rulec's §15.313): rulec decides
/// what the value reaches over the ranges of the inputs the call gives, and of the fields of the
/// elements a module gives. Held, an example past the top (E201), and an amount with no range
/// (W201): the workflow's code does not work the fee out, and the note says the rule's generated
/// code refuses the call where it works the value out, not at its door. A module's call is broken
/// by a run of its vectors; one whose ranges reach past the top while no run of its vectors does
/// is W201, with the same words.
#[test]
fn x2_the_range_a_value_the_rule_works_out_is_accepted_in() {
    let mut failures = Vec::new();
    let flow = |text: &str| vec![("card_fee.rule", DEFINE_RULE.to_string()), ("pay.flow", text.to_string())];
    run("x2-define-held", &flow(DEFINE_FLOW), 0, (1, 0, 0), &mut failures);
    let past = DEFINE_FLOW.replace("-> int range >=1 <=500000", "-> int range >=1 <=1000000");
    assert_ne!(past, DEFINE_FLOW);
    run("x2-define-E201", &flow(&past), 1, (0, 1, 0), &mut failures);
    let open = DEFINE_FLOW.replace("-> int range >=1 <=500000", "-> int");
    run("x2-define-W201", &flow(&open), 0, (0, 0, 1), &mut failures);
    let module = |text: &str| vec![("card_fee.rule", DEFINE_RULE.to_string()), ("pay.kn", text.to_string())];
    run("kaname-x2-define-held", &module(DEFINE_MODULE), 0, (1, 0, 0), &mut failures);
    let past = DEFINE_MODULE.replace("amount: number range >=1 <=500000", "amount: number range >=1 <=1000000");
    assert_ne!(past, DEFINE_MODULE);
    run("kaname-x2-define-E201", &module(&past), 1, (0, 1, 0), &mut failures);
    // the ranges find the amount up to 699,999, which no amount up to 500,000 comes to
    let wide = DEFINE_MODULE.replace("card_fee(amount: amount).fee", "card_fee(amount: mod(amount, 300_000) + mod(amount, 400_000) + 1).fee");
    assert_ne!(wide, DEFINE_MODULE);
    run("kaname-x2-define-W201", &module(&wide), 0, (0, 0, 1), &mut failures);
    let lines = |text: &str| vec![("order_lines.rule", LINES_RULE.to_string()), ("order.kn", text.to_string())];
    run("kaname-x2-define-lines-held", &lines(LINES_MODULE), 0, (2, 0, 0), &mut failures);
    let past = LINES_MODULE.replace("price    : number  range >=1 <=1000\n", "price    : number  range >=1 <=1000000\n");
    assert_ne!(past, LINES_MODULE);
    run("kaname-x2-define-lines-E201", &lines(&past), 1, (1, 1, 0), &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// X2 with the bottom of the range of a total (rulec's §15.314): the rule refuses a call whose
/// total ends under it, an empty list among them, and rulec decides it over the fewest elements
/// the call gives, each at the bottom of its range. One line at the least, of 1 at the least,
/// reaches an amount accepted from 1 (held, in the test above). A list that may be empty does
/// not, and the empty list among the function's vectors is the example (E201). One line of 1 does
/// not reach an amount accepted from 5, and no run of the vectors gives that line (W201). A total of
/// money says its ends in yen (DESIGN 7.1), from the unit the precondition carries (rulec's
/// §15.333): it adds up a `define`, whose type the facts do not carry, whether an output of its
/// name returns it or not. The total no output returns was said in digits (`1 <= total <=
/// 100_000_000`).
#[test]
fn x2_the_bottom_of_a_total_over_a_list() {
    let mut failures = Vec::new();
    let empty = LINES_MODULE.replace("count >=1 <=10", "count >=0 <=10");
    assert_ne!(empty, LINES_MODULE);
    run("kaname-x2-total-empty-E201", &[("order_lines.rule", LINES_RULE.to_string()), ("order.kn", empty.clone())], 1, (1, 1, 0), &mut failures);
    let from_five = LINES_RULE.replace("range >=1 <=100_000_000", "range >=5 <=100_000_000");
    assert_ne!(from_five, LINES_RULE);
    run("kaname-x2-total-bottom-W201", &[("order_lines.rule", from_five), ("order.kn", LINES_MODULE.to_string())], 0, (1, 0, 1), &mut failures);
    let mut yen = LINES_RULE.to_string();
    for (from, to) in [
        ("price    : number  range >=1 <=50_000_000", "price    : money[JPY]  range >=1JPY <=50_000_000JPY"),
        ("define subtotal : number = quantity * price  range >=1 <=10_000_000", "define subtotal : money[JPY] = quantity * price  range >=1JPY <=10_000_000JPY"),
        ("range >=1 <=100_000_000", "range >=1JPY <=100_000_000JPY"),
        ("amount : number  round down(1)", "amount : money[JPY]  round down(1JPY)"),
    ] {
        yen = swapped(&yen, from, to);
    }
    let module = swapped(&swapped(&empty, "price    : number  range >=1 <=1000\n", "price    : money[JPY]  range >=1JPY <=1000JPY\n"), ") -> number", ") -> money[JPY]");
    run("kaname-x2-total-yen-E201", &[("order_lines.rule", yen.clone()), ("order.kn", module.clone())], 1, (1, 1, 0), &mut failures);
    // the total under a name of its own, which the output works out from
    let unseen = swapped(
        &swapped(&yen, "sum amount over lines of subtotal", "sum total over lines of subtotal"),
        "amount : money[JPY]  round down(1JPY)\n",
        "amount : money[JPY]  round down(1JPY)\n\ndefine amount : money[JPY] = total\n",
    );
    run("kaname-x2-total-unseen-yen-E201", &[("order_lines.rule", unseen), ("order.kn", module)], 1, (1, 1, 0), &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A rule whose total adds up a value worked out on each line that keeps its fractions: 8% of a
/// line's price, to the sen.
const TAX_RULE: &str = "rule line_tax v1\ndescription \"The tax on the lines of an order, each line's worked out to the sen\"\n\nelements lines\n  price : money[JPY]  range >=0JPY <=10_000JPY\n\ndefine tax : money[JPY] = price * 8%  range >=0JPY <=800JPY\n\nsum total_tax over lines of tax  range >=0JPY <=8_000JPY\n\noutputs\n  due : money[JPY]  round down(1JPY)\n\ndefine due : money[JPY] = total_tax\n";
const TAX_MODULE: &str = "module order v1\ndescription \"The tax on an order, as the rule works it out\"\n\nuse rule line_tax from \"line_tax.rule\"\n\nrecord Line\n  price : money[JPY]  range >=0JPY <=10_000JPY\n\nfn tax_of(lines: list[Line] count >=0 <=20) -> money[JPY]\n  line_tax(lines: map(lines, l => line_tax.lines { price: l.price })).due\n";

/// X2 with a total held finer than its unit: the tax of each line keeps its fractions, and the
/// precondition on their total counts hundredths of a yen (rulec's §15.314). The tops of the
/// elements were added up in whole yen and compared with the top of the total in hundredths, so
/// twenty lines of up to 800JPY of tax, 16_000JPY, were said to keep a total accepted up to
/// 8_000JPY, and the call passed. It is not decided now, and the precondition is said at the value
/// the rule writes, in yen (§15.333), where it was the integer on the wire (`total_tax <= 800_000`).
#[test]
fn x2_a_total_held_finer_than_its_unit() {
    let mut failures = Vec::new();
    run("kaname-x2-total-sen-W201", &[("line_tax.rule", TAX_RULE.to_string()), ("order.kn", TAX_MODULE.to_string())], 0, (0, 0, 1), &mut failures);
    // ten lines keep it: 10 × 800JPY is 8_000JPY
    let ten = swapped(TAX_MODULE, "count >=0 <=20", "count >=0 <=10");
    run("kaname-x2-total-sen-held", &[("line_tax.rule", TAX_RULE.to_string()), ("order.kn", ten)], 0, (1, 0, 0), &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A rule that walks two sequences, where a module gives each its own list: what is still due on
/// an order, its lines added up less what was paid.
const TWO_LISTS_RULE: &str = "rule settle v1\ndescription \"What is still due on an order, its lines added up less what was paid, and how many of its entries wait. Up to five lines and up to twenty payments\"\n\nelements lines\n  price : number  range >=0 <=100\n  held  : bool\n\nelements payments\n  paid    : number  range >=0 <=100\n  pending : bool\n\ncount held_lines over lines where held  range >=0 <=5\ncount pending_payments over payments where pending  range >=0 <=20\nsum subtotal over lines of price  range >=0 <=500\nsum paid_total over payments of paid  range >=0 <=2000\n\noutputs\n  due     : number  round down(1)\n  waiting : number  round down(1)\n\ndefine due : number = subtotal - paid_total\ndefine waiting : number = held_lines + pending_payments\n";
const TWO_LISTS_MODULE: &str = "module settle_up v1\ndescription \"What is still due on an order, as the rule works it out\"\n\nuse rule settle from \"settle.rule\"\n\nrecord Line\n  price : number  range >=0 <=100\n  held  : bool\n\nrecord Payment\n  paid    : number  range >=0 <=100\n  pending : bool\n\nfn due_of(lines: list[Line] count >=0 <=5, payments: list[Payment] count >=0 <=20) -> number\n  settle(lines: map(lines, l => settle.lines { price: l.price, held: l.held }), payments: map(payments, p => settle.payments { paid: p.paid, pending: p.pending })).due\n";

/// X2 where a rule walks two sequences and the call gives each a list of its own: a bound the rule
/// puts on one walk is asked with the fewest and the most elements of the list given to that
/// sequence. They were asked with the shortest and the longest of every list of the call, so five
/// lines of up to 100 beside twenty payments were said to reach a subtotal past 500 at "up to 20
/// elements" and to pass the five lines the rule takes (two W201 for a call that keeps all four
/// bounds), and two lines of 1 at the least beside payments that may be none were said to come
/// under a subtotal accepted from 2 at "as few as 0 elements". Held, all four; six lines are an
/// example past the five (E201) and reach 600 at "up to 6 elements" (W201); and the bottom is
/// held with the two lines the call gives at the least.
#[test]
fn x2_each_list_is_asked_with_its_own_length() {
    let mut failures = Vec::new();
    let files = |rule: &str, module: &str| vec![("settle.rule", rule.to_string()), ("settle.kn", module.to_string())];
    run("kaname-x2-two-lists-held", &files(TWO_LISTS_RULE, TWO_LISTS_MODULE), 0, (4, 0, 0), &mut failures);
    let six = TWO_LISTS_MODULE.replace("lines: list[Line] count >=0 <=5", "lines: list[Line] count >=0 <=6");
    assert_ne!(six, TWO_LISTS_MODULE);
    run("kaname-x2-two-lists-six-lines", &files(TWO_LISTS_RULE, &six), 1, (2, 1, 1), &mut failures);
    let from_two = TWO_LISTS_RULE.replace("  price : number  range >=0 <=100", "  price : number  range >=1 <=100").replace("of price  range >=0 <=500", "of price  range >=2 <=500");
    let two_lines = TWO_LISTS_MODULE
        .replace("  price : number  range >=0 <=100", "  price : number  range >=1 <=100")
        .replace("lines: list[Line] count >=0 <=5, payments: list[Payment] count >=0 <=20", "lines: list[Line] count >=2 <=5, payments: list[Payment] count >=0 <=5");
    assert!(from_two.contains(">=2 <=500") && two_lines.contains("count >=2 <=5"));
    run("kaname-x2-two-lists-bottom-held", &files(&from_two, &two_lines), 0, (4, 0, 0), &mut failures);
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
    // undecided, not an example: rulec counts the seats still to give as -50 to 100, but the rule's
    // constraint keeps every input from the rule's vectors at 0 or more (rulec does not narrow the
    // range by it); the flow keeps the constraint (X2)
    let tied = "rule seats v1\ndescription \"The seats an event still needs: what it asked for, less what it holds, which is never more than it asked for\"\n\ninputs\n  asked   : number  range >=0 <=100\n  holding : number  range >=0 <=50\n\nconstraint holding <= asked\n\noutputs\n  needed : number  round down(1)\n\nresult needed = asked - holding\n".to_string();
    let flow = "workflow booking v1\ndescription \"Gives an event the seats it still needs\"\n\nuse rule seats from \"seats.rule\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:seats\"\nuse book hall from \"hall.book\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:hall\"\n\ninputs\n  event   : string\n  asked   : int  range >=50 <=100\n  holding : int  range >=0 <=50\n\ntask give_seats(event: string, count: int)\n  book hall.assign.do\n\nflow\n  let need = seats(asked: asked, holding: holding)\n  give_seats(event: event, count: need.needed)\n".to_string();
    let book = changed("W203", &[]).into_iter().find(|(n, _)| *n == "hall.book").unwrap().1;
    // given the same number for both, 0 to 50, the rule counts -50 to 50 over what the call gives
    // it, and its vectors there keep the constraint
    let same = flow
        .replace("  asked   : int  range >=50 <=100\n  holding : int  range >=0 <=50\n", "  count   : int  range >=0 <=50\n")
        .replace("seats(asked: asked, holding: holding)", "seats(asked: count, holding: count)");
    run("x4-W203-unshown", &[("seats.rule", tied.clone()), ("hall.book", book.clone()), ("booking.flow", same)], 0, (2, 0, 1), &mut failures);
    // asked 50 to 100: over what the call gives the rule, the seats still to give are 0 to 100
    run("x4-held-ranges", &[("seats.rule", tied), ("hall.book", book.clone()), ("booking.flow", flow.clone())], 0, (3, 0, 0), &mut failures);
    // an example from the vectors (rulec DESIGN §15.270): with five seats kept back, the seats still
    // to give never come to the low end of their range, -55, under the constraint, and the fewest
    // the rule's vectors come to, -5, is an amount chobo does not take
    let short = "rule seats v1\ndescription \"The seats an event still needs: what it asked for, less what it holds and five kept back; it never holds more than it asked for\"\n\ninputs\n  asked   : number  range >=0 <=100\n  holding : number  range >=0 <=50\n\nconstraint holding <= asked\n\noutputs\n  needed : number  round down(1)\n\nresult needed = asked - holding - 5\n".to_string();
    run("x4-E203-vectors", &[("seats.rule", short), ("hall.book", book), ("booking.flow", flow)], 1, (2, 1, 0), &mut failures);
    run("x4-E204", &changed("E204", &[]), 1, (1, 1, 0), &mut failures);
    run("x4-W204", &changed("W204", &[]), 0, (1, 0, 1), &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// X4 along the runs of the flow (DESIGN 7.6; dandori's DESIGN 18): a rule's output given to a
/// transfer is counted over what the runs that reach the transfer can give the call of the rule.
/// The arms of a `match` on the kind, before the transfer, leave out the handback, which gives seats
/// back; an arm that ends in `fail` is not joined; a call made in an arm is given the arm's kinds.
/// Another value put in the variable the call was given says nothing of the call, and an amount no
/// run gives a rule's output is no border.
#[test]
fn x4_follows_the_flow_to_the_transfer() {
    let mut failures = Vec::new();
    let body = "  let need = seats(event_kind: kind)\n  give_seats(event: event, count: need.needed)\n";
    let flow = |to: &str| changed("E203", &[("booking.flow", body, to)]);
    let arm = "  let need = seats(event_kind: kind)\n  match kind\n    handback => pass\n    workshop, talk => give_seats(event: event, count: need.needed)\n";
    run("x4-branch-arm", &flow(arm), 0, (2, 0, 0), &mut failures);
    let guard = "  let need = seats(event_kind: kind)\n  match kind\n    handback => fail Handback \"a handback gives no seats\"\n    workshop, talk => pass\n  give_seats(event: event, count: need.needed)\n";
    run("x4-branch-guard", &flow(guard), 0, (2, 0, 0), &mut failures);
    let call = "  match kind\n    handback => pass\n    workshop, talk =>\n      let need = seats(event_kind: kind)\n      give_seats(event: event, count: need.needed)\n";
    run("x4-branch-call", &flow(call), 0, (2, 0, 0), &mut failures);
    // the arm that gives the seats still takes the handback: the note says what the call is given,
    // and the fix names the value to branch on
    let still = "  let need = seats(event_kind: kind)\n  match kind\n    talk => pass\n    handback, workshop => give_seats(event: event, count: need.needed)\n";
    run("x4-branch-still", &flow(still), 1, (1, 1, 0), &mut failures);
    let reassigned = changed(
        "E203",
        &[
            ("booking.flow", "  kind  : seats.kind\n", "  kind  : seats.kind\n  other : seats.kind\n"),
            ("booking.flow", body, "  let k = kind\n  let need = seats(event_kind: k)\n  let k = other\n  match k\n    handback => pass\n    workshop, talk => give_seats(event: event, count: need.needed)\n"),
        ],
    );
    run("x4-branch-reassigned", &reassigned, 1, (1, 1, 0), &mut failures);
    run("x4-literal", &flow("  let need = seats(event_kind: workshop)\n  give_seats(event: event, count: need.needed)\n"), 0, (2, 0, 0), &mut failures);
    // a concert, which the hall cannot seat, stopped before the transfer: the search for the
    // refusals is held to what the other kinds give
    let concert = "  let need = seats(event_kind: kind)\n  match kind\n    concert => fail TooLarge \"the hall cannot seat a concert\"\n    workshop, talk => pass\n  give_seats(event: event, count: need.needed)\n";
    run("x4-E204-branch", &changed("E204", &[("booking.flow", body, concert)]), 0, (2, 0, 0), &mut failures);
    let overwritten = "  let need = seats(event_kind: kind)\n  let n = need.needed\n  let n = 5\n  give_seats(event: event, count: n)\n";
    run("x4-overwritten", &flow(overwritten), 0, (0, 0, 0), &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn x6_the_day_given_to_a_koyomi_date() {
    let mut failures = Vec::new();
    run("x6-held", &changed("W205", &[]), 0, (1, 0, 1), &mut failures);
    run("x6-E205", &changed("E205", &[]), 1, (0, 1, 1), &mut failures);
    let open = changed("W205", &[("reminding.flow", "reminders.reminder(due: due.day)", "reminders.reminder(due: received)")]);
    run("x6-W205", &open, 0, (0, 0, 2), &mut failures);
    // a variable given a day in two places is, where the date is called, what the runs that reach
    // the call last put in it (dandori's DESIGN 1.3): `day` is the day the payment is due first,
    // and the day the order was received, which nothing bounds, in one arm of a `match`
    let tail = "  let note = reminders.reminder(due: due.day)\n  remind(order: order, on: note.day)\n";
    let call = "      let note = reminders.reminder(due: day)\n      remind(order: order, on: note.day)\n";
    let rebound = |arms: String| changed("W205", &[("reminding.flow", "  received : date\n", "  received : date\n  late     : bool\n"), ("reminding.flow", tail, &format!("  let day = due.day\n  match late\n{arms}"))]);
    // held: the date is called in the other arm, where `day` is still the day the payment is due
    run("x6-rebound-held", &rebound(format!("    true =>\n      let day = received\n      pass\n    false =>\n{call}")), 0, (1, 0, 1), &mut failures);
    // undecided: the date is called in the arm that puts the day of the receipt in it
    run("x6-rebound-W205", &rebound(format!("    true =>\n      let day = received\n{call}    false => pass\n")), 0, (0, 0, 2), &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// X6 over the ranges a `.flow` writes (dandori's DESIGN 20): a date held to a range is given to a
/// koyomi date with every day of the range, a time held to one and `now` in the period the workflow
/// runs in with the days their moments are at the calendar's offset. A range inside the input's is
/// held; one past it is E205, and so is one with an open end, past which any day can come; a period
/// written in UTC goes a day past the input's range in a calendar nine hours east (E205), and the
/// same period written in its offset is held; `now` with no period, and `now` in `on failure`, which
/// the period does not hold, say nothing of what day they are (W205).
#[test]
fn x6_a_day_from_a_range_or_the_period() {
    let mut failures = Vec::new();
    let calendar = "calendar weekdays v1\ndescription \"Open Monday to Friday, in Tokyo\"\noffset +09:00\n\nclosed weekly sat, sun\n".to_string();
    let terms = "dates terms v1\ndescription \"Closes on the 20th and pays on the 10th of the next month, or the business day before\"\nuse calendar \"weekdays.cal\"\n\ninputs\n  received : date  range >=2026-01-01 <=2026-12-20\n\ndate closing = received\n  close day 20\n\ndate payment = closing\n  day 10 of month +1\n  roll preceding\n".to_string();
    let flow = |period: &str, input: &str, given: &str, on_failure: &str| {
        format!(
            "workflow reminding v1\n{period}description \"Reminds a customer of the day the payment of an order is due\"\n\nuse dates terms from \"terms.cal\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:terms\"\n\ninputs\n  order    : string\n  received : {input}\n\ntask remind(order: string, on: date)\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:remind\"\n  key\n\nflow\n  let due = terms.payment(received: {given})\n  remind(order: order, on: due.day)\n{on_failure}"
        )
    };
    let files = |flow: String| vec![("weekdays.cal", calendar.clone()), ("terms.cal", terms.clone()), ("reminding.flow", flow)];
    run("x6-range-held", &files(flow("", "date  range >=2026-01-01 <=2026-12-20", "received", "")), 0, (1, 0, 0), &mut failures);
    run("x6-range-E205", &files(flow("", "date  range >=2026-01-01 <=2026-12-31", "received", "")), 1, (0, 1, 0), &mut failures);
    run("x6-range-open-E205", &files(flow("", "date  range >=2026-01-01", "received", "")), 1, (0, 1, 0), &mut failures);
    // a time in its offset, and one in UTC, which runs nine hours into a day past the range at the calendar's offset
    run("x6-time-held", &files(flow("", "timestamp  range >=2026-01-01 <=2026-12-20 offset +09:00", "received", "")), 0, (1, 0, 0), &mut failures);
    run("x6-time-E205", &files(flow("", "timestamp  range >=2026-01-01 <=2026-12-20", "received", "")), 1, (0, 1, 0), &mut failures);
    run("x6-period-held", &files(flow("  now range >=2026-01-01 <=2026-12-20 offset +09:00\n", "date", "now", "")), 0, (1, 0, 0), &mut failures);
    run("x6-period-E205", &files(flow("  now range >=2026-01-01 <=2026-12-20\n", "date", "now", "")), 1, (0, 1, 0), &mut failures);
    run("x6-now-W205", &files(flow("", "date", "now", "")), 0, (0, 0, 1), &mut failures);
    let settle = "\non failure\n  let late = terms.payment(received: now)\n  remind(order: order, on: late.day)\n";
    run("x6-on-failure-W205", &files(flow("  now range >=2026-01-01 <=2026-12-20 offset +09:00\n", "date", "now", settle)), 0, (1, 0, 1), &mut failures);
    // the records of the borders say where each day comes from: a date's range, a time's, the
    // period, and `now` in `on failure`, which says nothing of what day it is
    let all = flow("  now range >=2026-01-01 <=2026-12-20 offset +09:00\n", "date  range >=2026-01-01 <=2026-12-20\n  at       : timestamp  range >=2026-01-01 <=2026-12-20", "received", settle).replace(
        "  remind(order: order, on: due.day)\n",
        "  remind(order: order, on: due.day)\n  let late = terms.payment(received: at)\n  remind(order: order, on: late.day)\n  let today = terms.payment(received: now)\n  remind(order: order, on: today.day)\n",
    );
    let t = lay_out(&files(all), "x6-records");
    let (code, out, err) = ritsu_in(t.path(), &["check", ".", "--format", "json"]);
    assert_eq!(code, 1, "{out}{err}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    records_agree("x6-records", &v);
    let b = &v["borders"];
    assert_eq!((b["held"].as_u64(), b["failed"].as_u64(), b["undecided"].as_u64()), (Some(2), Some(1), Some(1)), "{b}");
    golden("records-x6.json", &(serde_json::to_string_pretty(b).unwrap() + "\n"), &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// X3 (a) and X2 over a date the `.flow` holds to a range (dandori's DESIGN 20): given to a rule's
/// date input, every day of the range is held to the range the rule declares, the rule's in-range
/// day held and one before it E202; given to an input whose range is a koyomi date's days (`range
/// from koyomi`), every day of the range is held to those days, a range of one payment day held and
/// one with the days between two of them E201.
#[test]
fn x3a_a_date_held_to_a_range_given_to_a_rule() {
    let mut failures = Vec::new();
    let flow = |rule: &str, range: &str| {
        format!(
            "workflow billing v1\ndescription \"Bills an order in the batch the rule picks for the day it is paid on\"\n\nuse rule {rule} from \"{rule}.rule\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:{rule}\"\n\ninputs\n  order : string\n  paid  : date  range {range}\n\ntask bill(order: string, run: {rule}.run)\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:bill\"\n  key\n\nflow\n  let pick = {rule}(pay_day: paid)\n  bill(order: order, run: pick.batch)\n"
        )
    };
    let batch = repro("E202").into_iter().find(|(n, _)| *n == "batch.rule").unwrap().1.to_string();
    let rule = |range: &str| vec![("batch.rule", batch.clone()), ("billing.flow", flow("batch", range))];
    run("x3a-range-held", &rule(">=2026-03-01 <=2026-12-31"), 0, (1, 0, 0), &mut failures);
    run("x3a-range-E202", &rule(">=2026-02-01 <=2026-12-31"), 1, (0, 1, 0), &mut failures);
    let days = here().join("../rulec/tests/days");
    let cal = std::fs::read_to_string(days.join("payment_terms.cal")).unwrap();
    let settlement = std::fs::read_to_string(days.join("settlement.rule")).unwrap();
    let koyomi = |range: &str| vec![("payment_terms.cal", cal.clone()), ("settlement.rule", settlement.clone()), ("billing.flow", flow("settlement", range))];
    run("x3a-range-days-held", &koyomi(">=2026-02-10 <=2026-02-10"), 0, (2, 0, 0), &mut failures);
    run("x3a-range-days-E201", &koyomi(">=2026-02-10 <=2026-03-10"), 1, (1, 1, 0), &mut failures);
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
    // the hold voided when the approval does not come by the payment day at 09:00 (`timeout until`):
    // an event times out only at that time, which is 17 days and 9 hours after the hold at the
    // fewest; a callback's timeout can come as soon as the id is handed on, and its wait lasts 51
    // days and 9 hours at the most, from a `now` on 2026-07-21; a time an input gives bounds nothing
    let until = |code: &str, task: &str, by: &str, extra: &[(&'static str, &'static str, &'static str)]| {
        let task = format!("{task}\n\ntask put_back(order: string, sku: string) -> stock.reserve\n  book stock.reserve.void\n  sends void\n  errors expired\n  timeout 30 seconds\n\ncase goods");
        let call = format!("  approval(order: order, by: {by})\n    on timeout =>\n      goods <- put_back(order: order, sku: sku)\n        on expired => pass\n      fail NotApproved \"no one approved the order by the payment day\"\n");
        let mut edits: Vec<(&str, &str, &str)> = vec![
            ("invoice.flow", "  errors out_of_stock\n", "  errors out_of_stock\n  timeout 30 seconds\n"),
            ("invoice.flow", "  errors expired\n", "  errors expired\n  timeout 30 seconds\n"),
        ];
        edits.extend_from_slice(extra);
        edits.push(("invoice.flow", "case goods", &task));
        edits.push(("invoice.flow", "  wait until due.at\n", &call));
        changed(code, &edits)
    };
    let event = "task approval(order: string, by: timestamp)\n  event\n  timeout until by";
    let callback = "task approval(order: string, by: timestamp)\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:approval\"\n  callback\n  timeout until by";
    // an event takes no parameter but its time
    let event = event.replace("approval(order: string, by: timestamp)", "approval(by: timestamp)");
    let event_files: Vec<(&str, String)> = until("E206", &event, "due.at", &[]).into_iter().map(|(n, b)| (n, b.replace("approval(order: order, by: due.at)", "approval(by: due.at)"))).collect();
    run("x5-until-event-E206", &event_files, 1, (0, 1, 2), &mut failures);
    run("x5-until-callback-W206", &until("E206", callback, "due.at", &[]), 0, (0, 0, 3), &mut failures);
    run("x5-until-callback-held", &until("W206", callback, "due.at", &[]), 0, (2, 0, 1), &mut failures);
    let input = [("invoice.flow", "  qty   : int  range >=1 <=100\n", "  qty   : int  range >=1 <=100\n  by    : timestamp\n")];
    run("x5-until-open-W206", &until("E206", callback, "by", &input), 0, (0, 0, 3), &mut failures);
    // the warehouse confirms the order within 59 minutes, between a hold of an hour and the post,
    // whose calls take 10 seconds at the most: handing the callback's id on takes a minute more at
    // the most, on every target (dandori's DESIGN 17), and the post can come after the hold expires
    let confirm = changed(
        "E206",
        &[
            ("stock.book", "for 14 days at most", "for an hour at most"),
            ("stock.book", "pending expires after 14 days", "pending expires after 1 hour"),
            ("invoice.flow", "use dates terms from \"payment_terms.cal\"\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:payment-terms\"\n", ""),
            ("invoice.flow", "  errors out_of_stock\n", "  errors out_of_stock\n  timeout 10 seconds\n"),
            ("invoice.flow", "  errors expired\n", "  errors expired\n  timeout 10 seconds\n"),
            ("invoice.flow", "case goods", "task confirm(order: string)\n  lambda \"arn:aws:lambda:us-east-1:123456789012:function:confirm\"\n  callback\n  timeout 59 minutes\n\ncase goods"),
            ("invoice.flow", "  let due = terms.payment(received: now)\n  wait until due.at\n", "  confirm(order: order)\n"),
        ],
    );
    run("x5-callback-W206", &confirm, 0, (0, 0, 1), &mut failures);
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
    // the amount the call gives is the action's input: the flow gives `range`, the permit lets in
    // `when`
    let given = |range: &str, when: &str| {
        changed(
            "W909",
            &[
                (
                    "orders.json",
                    "        \"responses\": {\"201\": {\"description\": \"Refunded\"}",
                    "        \"requestBody\": {\"required\": true, \"content\": {\"application/json\": {\"schema\": {\"type\": \"object\", \"required\": [\"amount\"], \"properties\": {\"amount\": {\"type\": \"integer\", \"minimum\": 1, \"maximum\": 10000}}}}}},\n        \"responses\": {\"201\": {\"description\": \"Refunded\"}",
                ),
                ("returns.flow", "  order : string\n", &format!("  order  : string\n  amount : money[GBP, incl_tax]  range {range}\n")),
                ("returns.flow", "task refund_order(orderId: string)\n", "task refund_order(orderId: string, amount: money[GBP, incl_tax] range >=1 <=10000)\n"),
                ("returns.flow", "  refund_order(orderId: order)\n", "  refund_order(orderId: order, amount: amount)\n"),
                ("refunds.gate", "  resource Order from orderId\n", "  resource Order from orderId\n  input\n    amount : money[GBP, incl_tax]  range >=1GBP <=10_000GBP\n"),
                ("refunds.gate", "  when resource.status is returned\n", &format!("  when {when}\n")),
            ],
        )
    };
    run("x16-given-held", &given(">=1 <=50", "amount <= 50GBP"), 0, (1, 0, 0), &mut failures);
    run("x16-given-E908", &given(">=1 <=50", "amount > 100GBP"), 1, (0, 1, 0), &mut failures);
    run("x16-given-W909", &given(">=1 <=100", "amount <= 50GBP"), 0, (0, 0, 1), &mut failures);
    // a variable given an amount in two places is, where the task is called, what the runs that
    // reach the call last put in it (dandori's DESIGN 1.3): `sum` is up to 50 pounds first, and up
    // to 10,000 in one arm of a `match`
    let rebound = |arms: &str| {
        let mut files = given(">=1 <=50", "amount <= 50GBP");
        let flow = &mut files.iter_mut().find(|(n, _)| *n == "returns.flow").unwrap().1;
        *flow = swapped(flow, "  order  : string\n", "  order  : string\n  bulk   : bool\n  wide   : money[GBP, incl_tax]  range >=1 <=10000\n");
        *flow = swapped(flow, "  refund_order(orderId: order, amount: amount)\n", &format!("  let sum = amount\n  match bulk\n{arms}"));
        files
    };
    // held: the task is called in the other arm, where `sum` is still up to 50
    run("x16-given-rebound-held", &rebound("    true =>\n      let sum = wide\n      pass\n    false => refund_order(orderId: order, amount: sum)\n"), 0, (1, 0, 0), &mut failures);
    // undecided: the task is called in the arm that puts the wider amount in it
    run("x16-given-rebound-W909", &rebound("    true =>\n      let sum = wide\n      refund_order(orderId: order, amount: sum)\n    false => pass\n"), 0, (0, 0, 1), &mut failures);
    // E909: the task gives the fee in steps of 0.1%, and the gate reads it in steps of 1%, so the
    // flow's 0.5% (5 steps) is 5% to it. The error whether the task declares the error of a denial
    // or not (0.55.2 said nothing of the first, and W909 of the second, that the flow's number is not
    // of the gate's type); beside it, of the gate's answer only E908, where no permit allows the
    // workflow
    let declared = [("returns.flow", "  errors denied = 403\n", ""), ("returns.flow", "    on denied => fail Denied \"the gate did not let the workflow refund\"\n", "")];
    run("x16-E909", &changed("E909", &[]), 1, (0, 1, 0), &mut failures);
    run("x16-E909-undeclared", &changed("E909", &declared), 1, (0, 1, 0), &mut failures);
    // the fee written as a whole number of steps, as 0.55.2 read it: the same 5, so the same
    // error, and dandori's W106 beside it, asking for the unit
    run("x16-E909-whole", &changed("E909", &[("returns.flow", "fee: 0.5%)", "fee: 5)")]), 1, (0, 1, 0), &mut failures);
    let no_permit = "\npermit returns_keeps_a_small_fee\n  description \"The returns workflow refunds an order, keeping a fee of up to 1%\"\n  principal is workflow returns\n  action refund_order\n  when fee <= 1%\n";
    run("x16-E909-E908", &changed("E909", &[("refunds.gate", no_permit, "")]), 1, (0, 1, 0), &mut failures);
    // the Japanese names: the gate's input `手数料(fee)` takes the argument `fee` by its alias
    run("x16-E909-ja", &repro_ja("E909"), 1, (0, 1, 0), &mut failures);
    // the same fee in the same steps passes as before: the gate allows the flow's 0.5%
    run("x16-E909-same-step", &changed("E909", &[("refunds.gate", "    fee : rate[step 1%]  range >=0% <=10%\n", "    fee : rate[step 0.1%]  range >=0% <=10%\n")]), 0, (1, 0, 0), &mut failures);
    // a quantity in another unit is the same error: the flow's 5g is 5kg to the gate
    let mass = [
        ("returns.flow", "fee: rate[step 0.1%] range >=0% <=10%", "fee: mass[g] range >=0g <=100g"),
        ("returns.flow", "fee: 0.5%)", "fee: 5g)"),
        ("refunds.gate", "    fee : rate[step 1%]  range >=0% <=10%\n", "    fee : mass[kg]  range >=0kg <=100kg\n"),
        ("refunds.gate", "  when fee <= 1%\n", "  when fee <= 1kg\n"),
    ];
    run("x16-E909-mass", &changed("E909", &mass), 1, (0, 1, 0), &mut failures);
    // an amount the gate counts in pence (the integers the operation takes, as the flow's pounds
    // are), and one it counts without the tax: the second reads the same number as the same
    // pounds, so the error says the two types only
    let retyped = |ty: &str| {
        let mut files = given(">=1 <=50", "amount <= 50GBP");
        let gate = &mut files.iter_mut().find(|(n, _)| *n == "refunds.gate").unwrap().1;
        *gate = swapped(gate, "    amount : money[GBP, incl_tax]  range >=1GBP <=10_000GBP\n", &format!("    amount : {ty}\n"));
        files
    };
    run("x16-given-E909-pence", &retyped("money[GBPc, incl_tax]  range >=1GBPc <=10_000GBPc"), 1, (0, 1, 0), &mut failures);
    run("x16-given-E909-tax", &retyped("money[GBP]  range >=1GBP <=10_000GBP"), 1, (0, 1, 0), &mut failures);
    // and an amount in another currency: the flow's pound is a yen to the gate
    let mut yen = given(">=1 <=50", "amount <= 5_000JPY");
    let gate = &mut yen.iter_mut().find(|(n, _)| *n == "refunds.gate").unwrap().1;
    *gate = swapped(gate, "    amount : money[GBP, incl_tax]  range >=1GBP <=10_000GBP\n", "    amount : money[JPY, incl_tax]  range >=1JPY <=10_000JPY\n");
    run("x16-given-E909-currency", &yen, 1, (0, 1, 0), &mut failures);
    // a rule says who refunds, and the flow refunds under the arm `arm` of its answer; the gate
    // reads the same rule of the order's total, which `lines` under the workflow say is the flow's
    // amount; `declared`, the task declares the error of a denial
    let routed = |arm: &str, lines: &str, declared: bool| {
        let (errors, on_denied) = if declared { ("  errors denied = 403\n", "        on denied => fail Denied \"the gate did not let the workflow refund\"\n") } else { ("", "") };
        let other = if arm == "itself" { "clerk" } else { "itself" };
        let mut files = changed(
            "W909",
            &[
                ("returns.flow", "use openapi orders from \"orders.json\"\n", "use rule route from \"refund_route.rule\"\nuse openapi orders from \"orders.json\"\n"),
                ("returns.flow", "  order : string\n", "  order  : string\n  amount : money[GBP, incl_tax]  range >=1 <=10000\n"),
                ("returns.flow", "  key\n", &format!("{errors}  key\n")),
                ("returns.flow", "  refund_order(orderId: order)\n", &format!("  let r = route(amount: amount)\n  match r.who\n    {arm} =>\n      refund_order(orderId: order)\n{on_denied}    {other} => pass\n")),
                ("refunds.gate", "use openapi orders from \"orders.json\"\n", "use rule route from \"refund_route.rule\"\nuse openapi orders from \"orders.json\"\n"),
                ("refunds.gate", "  description \"Refunds an order once the item is back\"\n", &format!("  description \"Refunds an order once the item is back\"\n{lines}")),
                ("refunds.gate", "    status : order_status\n", "    status : order_status\n    total  : money[GBP, incl_tax]  range >=1GBP <=10_000GBP\n"),
                ("refunds.gate", "  resource Order from orderId\n", "  resource Order from orderId\n  context\n    who = route(amount: resource.total).who\n"),
                ("refunds.gate", "  when resource.status is returned\n", "  when who is itself\n"),
            ],
        );
        files.push(("refund_route.rule", REFUND_ROUTE.to_string()));
        files
    };
    let lines = "  resource Order from order\n    total = amount\n";
    run("x16-arm-held", &routed("itself", lines, true), 0, (1, 0, 0), &mut failures);
    run("x16-arm-W909", &routed("itself", lines, false), 0, (0, 0, 1), &mut failures);
    run("x16-arm-E908", &routed("clerk", lines, true), 1, (0, 1, 0), &mut failures);
    run("x16-arm-unsaid-W909", &routed("itself", "", false), 0, (0, 0, 1), &mut failures);
    // the answer reads who asked for the refund and who approved it, which no value of a flow is
    // said for: the examples are the gate's own, in Japanese with the spaces of the suite's
    let related = changed(
        "W909",
        &[
            ("refunds.gate", "    status : order_status\n", "    status    : order_status\n    requester : User\n    approver  : User\n"),
            ("refunds.gate", "  when resource.status is returned\n", "  unless resource.requester is resource.approver\n"),
        ],
    );
    run("x16-relation-W909", &related, 0, (0, 0, 1), &mut failures);
    // the records of the borders say which lines of the gate an answer rests on (`assumes`): the
    // call under the arm the gate allows, and the one under the arm it denies; none for a call
    // that holds because the flow handles a denial
    let mut both = routed("itself", lines, false);
    for (name, body) in &mut both {
        if *name == "returns.flow" {
            assert!(body.contains("    clerk => pass\n"), "{body}");
            *body = body.replace("    clerk => pass\n", "    clerk =>\n      refund_order(orderId: order)\n");
        }
    }
    let t = lay_out(&both, "x16-records");
    for (lang, file) in [("en", "records-x16.json"), ("ja", "records-x16.ja.json")] {
        let (code, out, err) = ritsu_in(t.path(), &["check", ".", "--format", "json", "--lang", lang]);
        assert_eq!(code, 1, "{out}{err}");
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        records_agree("x16-records", &v);
        let b = &v["borders"];
        assert_eq!((b["held"].as_u64(), b["failed"].as_u64(), b["undecided"].as_u64()), (Some(0), Some(1), Some(1)), "{b}");
        golden(file, &(serde_json::to_string_pretty(b).unwrap() + "\n"), &mut failures);
    }
    let t = lay_out(&routed("itself", lines, true), "x16-records-held");
    let (_, out, _) = ritsu_in(t.path(), &["check", ".", "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["borders"]["records"][0]["assumes"], serde_json::json!([]), "{}", v["borders"]);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The rule the company's gate holds every request to: no one decides their own request, unless
/// its route allows it.
const OWN_REQUEST: &str = "\nforbid no_one_decides_their_own\n  principal is Staff\n  action approve, reject\n  when resource.requester is principal\n  unless resource.self_approval\n";

#[test]
fn x18_a_requests_route_against_its_gate() {
    let mut failures = Vec::new();
    let permit = "permit approvers_answer_their_stage\n  principal is Staff\n  action approve, reject\n  when principal in resource.approvers\n";
    let with_rule = [("company.gate", permit, &*format!("{permit}{OWN_REQUEST}"))];
    // held: the route allows no self approval, and the gate forbids it unless the route allows it
    run("x18-held", &changed("E911", &with_rule), 0, (3, 0, 0), &mut failures);
    // the gate lets the approvers of a stage answer it, the requester among them
    run("x18-E911", &changed("E911", &[]), 1, (2, 1, 0), &mut failures);
    // a route that allows it: the gate says nothing against it (held), or forbids it whatever the
    // route says
    let allowed = ("purchase.ringi", "  self approval  not allowed\n", "  self approval  allowed\n");
    run("x18-allowed-held", &changed("E911", &[allowed]), 0, (3, 0, 0), &mut failures);
    let whatever = OWN_REQUEST.replace("  unless resource.self_approval\n", "");
    run("x18-allowed-E911", &changed("E911", &[allowed, ("company.gate", permit, &format!("{permit}{whatever}"))]), 1, (2, 1, 0), &mut failures);
    // only the requester answers a stage: the approvers are allowed nothing, and the requester is
    let requester_only = ("company.gate", "  when principal in resource.approvers\n", "  when resource.requester is principal\n");
    run("x18-approvers-E911", &changed("E911", &[requester_only]), 1, (1, 2, 0), &mut failures);
    // an operation the gate has no action for: one every ticket has
    let all = "  action submit, see, add_approver, hand_over, change_request_type, read_audit, import_organization, read_organization_audit\n";
    let no_hand_over = [
        with_rule[0],
        ("company.gate", "action hand_over\n  principal Staff\n  resource Stage\n", ""),
        ("company.gate", all, "  action submit, see, add_approver, change_request_type, read_audit, import_organization, read_organization_audit\n"),
    ];
    run("x18-operation-E911", &changed("E911", &no_hand_over), 1, (2, 1, 0), &mut failures);
    // no `see`: no one off a ticket sees it, an administrator among them (ringi's DESIGN 12.4)
    let no_see = [
        with_rule[0],
        ("company.gate", "action see\n  principal Staff\n  resource Ticket\n", ""),
        ("company.gate", all, "  action submit, add_approver, hand_over, change_request_type, read_audit, import_organization, read_organization_audit\n"),
    ];
    run("x18-see-E911", &changed("E911", &no_see), 1, (2, 1, 0), &mut failures);
    // the operations of the server's administration (ringi's DESIGN 14): a gate that declares only
    // the switch of a request type, as a gate did before the record and the organization were
    // administered, and then one with no switch of a request type either and no `hand_over`
    let admin = "action read_audit\n  principal Staff\n  resource RequestType\naction import_organization\n  principal Staff\n  resource Organization\naction read_organization_audit\n  principal Staff\n  resource Organization\n";
    let switch_only = [with_rule[0], ("company.gate", admin, ""), ("company.gate", "resource Organization\n", ""), ("company.gate", all, "  action submit, see, add_approver, hand_over, change_request_type\n")];
    run("x18-administration-E911", &changed("E911", &switch_only), 1, (2, 1, 0), &mut failures);
    let and_a_ticket = [
        with_rule[0],
        ("company.gate", "action hand_over\n  principal Staff\n  resource Stage\n", ""),
        ("company.gate", "action change_request_type\n  principal Staff\n  resource RequestType\n", ""),
        ("company.gate", all, "  action submit, see, add_approver, read_audit, import_organization, read_organization_audit\n"),
    ];
    run("x18-ticket-and-administration-E911", &changed("E911", &and_a_ticket), 1, (2, 1, 0), &mut failures);
    // an action of the administration that takes an input of its own
    let admin_input = [with_rule[0], ("company.gate", "action import_organization\n  principal Staff\n  resource Organization\n", "action import_organization\n  principal Staff\n  resource Organization\n  input\n    dry_run : bool\n")];
    run("x18-administration-input-E911", &changed("E911", &admin_input), 1, (2, 1, 0), &mut failures);
    // and two the request gives its tickets, each with the line that does
    let sends_back = [with_rule[0], ("purchase.ringi", "ticket\n", "ticket\n  send back      to the requester, at most 2 times\n")];
    run("x18-operations-E911", &changed("E911", &sends_back), 1, (2, 1, 0), &mut failures);
    // an action an operation asks that takes inputs of its own: the server asks the gate with the
    // person, the resource and the time, and has nothing to give them; one action, then two
    let own_input = [with_rule[0], ("company.gate", "action approve\n  principal Staff\n  resource Stage\n", "action approve\n  principal Staff\n  resource Stage\n  input\n    urgent : bool\n")];
    run("x18-input-E911", &changed("E911", &own_input), 1, (2, 1, 0), &mut failures);
    let own_inputs = [own_input[0], own_input[1], ("company.gate", "action comment\n  principal Staff\n  resource Stage\n", "action comment\n  principal Staff\n  resource Stage\n  input\n    length : number  range >=1 <=2000\n    shared : bool\n")];
    run("x18-inputs-E911", &changed("E911", &own_inputs), 1, (2, 1, 0), &mut failures);
    // `self_approval` declared as an enum, where ringi hands the gate a bool
    run("x18-W911", &changed("W911", &[]), 0, (1, 0, 2), &mut failures);
    // in Japanese names
    run("x18-E911-ja", &repro_ja("E911"), 1, (2, 1, 0), &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A calendar that closes other days than the office's.
const PLANT_CAL: &str = "calendar plant v1\ndescription \"The business days of the plant: Sundays and Mondays are closed\"\noffset +00:00\n\nclosed weekly sun, mon\n";

/// A small organization (fictional): the president, two departments, and their people.
const COMPANY: &str = "organization company v1 \"Company\"\ndescription \"The shape of the company's organization (fictional)\"\n\npositions\n  staff     \"Staff\"\n  dept_head \"Head of department\"\n  president \"President\"\n\nteam kinds\n  company    \"Company\"\n  department \"Department\"\n\ndata \"org\"\n";
const TEAMS: &str = "id,name,name_ja,kind,parent,head\ncompany,Example Co. (fictional),株式会社サンプル（架空）,company,,e0001\nsales,Sales,営業部,department,company,e1001\nfinance,Finance,財務部,department,company,e2001\n";
const PEOPLE: &str = "id,name,name_ja,email\ne0001,President (fictional),社長（架空）,e0001@example.com\ne1001,Head of Sales,営業部長,e1001@example.com\ne1002,Sales staff,営業部員,e1002@example.com\ne2001,Head of Finance,財務部長,e2001@example.com\ne2002,Finance staff,財務部員,e2002@example.com\n";
const MEMBERS: &str = "person,team,position,primary\ne0001,company,president,yes\ne1001,sales,dept_head,yes\ne1002,sales,staff,yes\ne2001,finance,dept_head,yes\ne2002,finance,staff,yes\n";

#[test]
fn x19_a_hold_a_ticket_makes() {
    let mut failures = Vec::new();
    let days = |n: u32| ("budget.book", "  pending expires after 7 days\n".to_string(), format!("  pending expires after {n} days\n"));
    let edits = |e: &[(&'static str, String, String)]| -> Vec<(&'static str, String)> { changed("E209", &e.iter().map(|(f, a, b)| (*f, a.as_str(), b.as_str())).collect::<Vec<_>>()) };
    // ten business days are fifteen days at the most: a hold of thirty lasts, one of seven does not
    run("x19-held", &edits(&[days(30)]), 0, (1, 0, 0), &mut failures);
    run("x19-E209", &changed("E209", &[]), 1, (0, 1, 0), &mut failures);
    // a hold that ends on the last second of the fifteenth day has expired when the round ends
    run("x19-E209-to-the-second", &edits(&[days(15)]), 1, (0, 1, 0), &mut failures);
    run("x19-never-expires", &changed("E209", &[("budget.book", "  pending expires after 7 days\n", "  pending never expires\n")]), 0, (1, 0, 0), &mut failures);
    // a stage sends the ticket back to an earlier stage, which starts no new round: the round of
    // twenty business days is walked three times
    let back = [
        days(60),
        ("purchase.ringi", "  check  finance\n".to_string(), "  check   finance\n  decide  finance\n".to_string()),
        ("purchase.ringi", "ticket\n".to_string(), "ticket\n  send back     to any earlier stage, at most 2 times\n".to_string()),
        ("purchase.ringi", "  on rejected, expired  void".to_string(), "  on sent back, rejected, expired  void".to_string()),
    ];
    run("x19-earlier-stage-E209", &edits(&back), 1, (0, 1, 0), &mut failures);
    // a stage with no deadline
    run("x19-W209", &changed("W209", &[]), 0, (0, 0, 1), &mut failures);
    // the stages counted in the office's calendar, the pause in the plant's
    let mut two = edits(&[
        days(21),
        ("purchase.ringi", "use calendar office from \"office.cal\"\n".to_string(), "use calendar office from \"office.cal\"\nuse calendar plant from \"plant.cal\"\n".to_string()),
        ("purchase.ringi", "  due in 10 business days at end of day\n".to_string(), "  due in 10 business days in office at end of day\n".to_string()),
        ("purchase.ringi", "ticket\n".to_string(), "ticket\n  pause         at most 5 business days in plant\n".to_string()),
    ]);
    two.push(("plant.cal", PLANT_CAL.to_string()));
    run("x19-two-calendars-W209", &two, 0, (0, 0, 1), &mut failures);
    // the hold takes the requester's department, and the president is above every department
    let org = |edits: &[(&'static str, String, String)], company: &str| {
        let mut with = vec![
            days(30),
            ("purchase.ringi", "use calendar office".to_string(), "use organization org from \"company.ringi\"\nuse calendar office".to_string()),
            ("purchase.ringi", "  dept   : text \"Department\"  count >=1 <=20\n".to_string(), String::new()),
            ("purchase.ringi", "dept: dept,".to_string(), "dept: requester.department,".to_string()),
        ];
        with.extend(edits.iter().cloned());
        let mut files = self::changed("E209", &with.iter().map(|(f, a, b)| (*f, a.as_str(), b.as_str())).collect::<Vec<_>>());
        files.extend([("company.ringi", company.to_string()), ("org/2026-04-01/teams.csv", TEAMS.to_string()), ("org/2026-04-01/people.csv", PEOPLE.to_string()), ("org/2026-04-01/members.csv", MEMBERS.to_string())]);
        files
    };
    run("x19-requesters-W209", &org(&[], COMPANY), 0, (1, 0, 1), &mut failures);
    // four directors beside the president, two of them gone in the next version: the note names
    // three people and says how many more, as ringi's check lists the requesters of a finding
    let mut many = org(&[], COMPANY);
    many.retain(|(f, _)| !f.starts_with("org/"));
    let directors = |ids: &[u32]| {
        let people: String = ids.iter().map(|n| format!("e000{n},Director {},取締役 {},e000{n}@example.com\n", n - 1, n - 1)).collect();
        let members: String = ids.iter().map(|n| format!("e000{n},company,staff,yes\n")).collect();
        (format!("{PEOPLE}{people}"), format!("{MEMBERS}{members}"))
    };
    let ((people, members), (people_next, members_next)) = (directors(&[2, 3, 4, 5]), directors(&[2, 3]));
    many.extend([
        ("org/2026-04-01/teams.csv", TEAMS.to_string()),
        ("org/2026-04-01/people.csv", people),
        ("org/2026-04-01/members.csv", members),
        ("org/2027-04-01/teams.csv", TEAMS.to_string()),
        ("org/2027-04-01/people.csv", people_next),
        ("org/2027-04-01/members.csv", members_next),
    ]);
    run("x19-many-requesters-W209", &many, 0, (1, 0, 1), &mut failures);
    // kept to the requesters who have one, the line holds nothing for the president
    let kept = ("purchase.ringi", "amount: amount)\n  on approved".to_string(), "amount: amount)  when requester.department is not none\n  on approved".to_string());
    run("x19-requesters-held", &org(&[kept], COMPANY), 0, (1, 0, 0), &mut failures);
    // an organization with no data says nothing of who has a department
    run("x19-no-data-W209", &org(&[], &COMPANY.replace("\ndata \"org\"\n", "")), 0, (1, 0, 1), &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A request whose small amounts an agent's flow answers in place of a person, by the rows of its
/// route.
const AGENT_REQUEST: &str = "request purchase v1 \"Purchase request\"\n\nuse flow precheck from \"precheck.flow\"\n\nform\n  amount  : money[JPY] \"Amount\"  range >=1JPY <=1_000_000JPY\n  purpose : long text \"Purpose\"  count >=1 <=200\n  vendor  : text \"Vendor\"  count <=100  optional\n\nroute\n| amount      | -> check | decide  |\n| <=30_000JPY |          | small   |\n| >30_000JPY  | finance  | finance |\n\nwho\n  finance = team finance\n  small   = agent precheck, else finance\n\nticket\n  self approval  not allowed\n";
const AGENT_FLOW: &str = "workflow purchase_precheck v1\ndescription \"The check of a small purchase request in place of a person: ok, or doubt with the reason\"\n\nenum Check = ok | doubt\n\nrecord Reading\n  verdict : Check\n  reason  : string\n\ninputs\n  request_id : string\n  amount     : money[JPY]  range >=1JPY <=30_000JPY\n  purpose    : string\n\noutputs\n  verdict : Check\n  reason  : string\n\ntask read(amount: money[JPY], purpose: string) -> Reading\n  timeout 60 seconds\n  idempotent\n\nflow\n  let r = read(amount: amount, purpose: purpose)\n  succeed verdict = r.verdict, reason = r.reason\n";

/// A request with a stage that shows a flow's summary to the people who review it, for the amounts
/// the stage's `when` names.
const NOTE_REQUEST: &str = "request contract v1 \"Contract review\"\n\nuse flow summarize from \"summary.flow\"\n\nform\n  title  : text \"Title\"  count >=1 <=100\n  terms  : long text \"Terms\"  count >=1 <=2000\n  amount : money[JPY] \"Amount\"  range >=0JPY <=100_000_000JPY\n  urgent : bool \"Urgent\"\n\nroute\n  summary  note from summarize  when amount >= 1_000_000JPY and amount <= 50_000_000JPY\n  review   legal\n\nwho\n  legal = team legal\n\nticket\n  self approval  not allowed\n";
const NOTE_FLOW: &str = "workflow contract_summary v1\ndescription \"A summary of a contract for the people who review it, shown on the approval card\"\n\ninputs\n  request_id : string\n  title      : string\n  terms      : string\n  amount     : money[JPY]  range >=1_000_000JPY <=50_000_000JPY\n\noutputs\n  summary : string\n\nrecord Note\n  summary : string\n\ntask summarize(title: string, terms: string, amount: money[JPY]) -> Note\n  timeout 5 minutes\n  idempotent\n\nflow\n  let n = summarize(title: title, terms: terms, amount: amount)\n  succeed summary = n.summary\n";

#[test]
fn x20_what_a_ticket_gives_a_flow() {
    let mut failures = Vec::new();
    // a flow an ending starts: the amount the form lets in, against the amount the flow takes
    run("x20-held", &changed("E210", &[("order.flow", "<=500_000JPY", "<=1_000_000JPY")]), 0, (2, 0, 0), &mut failures);
    run("x20-E210", &changed("E210", &[]), 1, (1, 1, 0), &mut failures);
    // the `on` line's `when` holds the amount to what the flow takes, or to more than that
    let when = |cond: &str| changed("E210", &[("purchase.ringi", "amount: amount)\n", &format!("amount: amount)  when {cond}\n"))]);
    run("x20-when-held", &when("amount <= 500_000JPY"), 0, (2, 0, 0), &mut failures);
    run("x20-when-E210", &when("amount <= 800_000JPY"), 1, (1, 1, 0), &mut failures);
    // written with `not`, ringi does not follow it
    // the ledger's W210 holds the other side too: a flow that submits a request through an API
    // whose request's file the project does not have
    run("x20-W210", &changed("W210", &[]), 0, (1, 0, 2), &mut failures);
    // the call itself: no value for an input, a value with no input, a value of another type
    let call = |args: &str| changed("E210", &[("order.flow", "<=500_000JPY", "<=1_000_000JPY"), ("purchase.ringi", "launch order(request_id: ticket.id, amount: amount)", &format!("launch order({args})"))]);
    run("x20-call-not-given-E210", &call("request_id: ticket.id"), 1, (1, 1, 0), &mut failures);
    run("x20-call-no-input-E210", &call("request_id: ticket.id, amount: amount, round: ticket.round"), 1, (2, 1, 0), &mut failures);
    run("x20-call-types-E210", &call("request_id: amount, amount: amount"), 1, (1, 1, 0), &mut failures);
    // an agent's flow: the amounts its rows of the route give it
    let agent = |edits: &[(&str, &str)]| {
        let mut flow = AGENT_FLOW.to_string();
        for (from, to) in edits {
            assert!(flow.contains(from), "the agent's flow has no {from:?}");
            flow = flow.replacen(from, to, 1);
        }
        vec![("purchase.ringi", AGENT_REQUEST.to_string()), ("precheck.flow", flow)]
    };
    run("x20-agent-held", &agent(&[]), 0, (4, 0, 0), &mut failures);
    run("x20-agent-row-E210", &agent(&[("<=30_000JPY", "<=10_000JPY")]), 1, (3, 1, 0), &mut failures);
    // a field a request may leave empty is taken as `T?`
    let more = "  purpose    : string\n\noutputs";
    run("x20-agent-optional-held", &agent(&[(more, "  purpose    : string\n  vendor     : string?\n\noutputs")]), 0, (5, 0, 0), &mut failures);
    run("x20-agent-optional-E210", &agent(&[(more, "  purpose    : string\n  vendor     : string\n\noutputs")]), 1, (4, 1, 0), &mut failures);
    // an input that is no field of the form
    run("x20-agent-no-field-E210", &agent(&[(more, "  purpose    : string\n  budget     : string\n\noutputs")]), 1, (4, 1, 0), &mut failures);
    // an input of another type than the field
    run("x20-agent-types-E210", &agent(&[("  purpose    : string\n\noutputs", "  purpose    : int\n\noutputs"), ("task read(amount: money[JPY], purpose: string)", "task read(amount: money[JPY], purpose: int)")]), 1, (3, 1, 0), &mut failures);
    // what ringi reads as the agent's answer
    let no_reason = [("outputs\n  verdict : Check\n  reason  : string\n", "outputs\n  verdict : Check\n"), ("  succeed verdict = r.verdict, reason = r.reason", "  succeed verdict = r.verdict")];
    run("x20-agent-answer-E210", &agent(&no_reason), 1, (3, 1, 0), &mut failures);
    // a summary's flow: the amounts the stage's `when` names
    let note = |request: &[(&str, &str)], flow: &[(&str, &str)]| {
        let made = |body: &str, edits: &[(&str, &str)]| {
            let mut body = body.to_string();
            for (from, to) in edits {
                assert!(body.contains(from), "the summary's project has no {from:?}");
                body = body.replacen(from, to, 1);
            }
            body
        };
        vec![("contract.ringi", made(NOTE_REQUEST, request)), ("summary.flow", made(NOTE_FLOW, flow))]
    };
    let cond = "  when amount >= 1_000_000JPY and amount <= 50_000_000JPY";
    run("x20-note-held", &note(&[], &[]), 0, (5, 0, 0), &mut failures);
    run("x20-note-E210", &note(&[(cond, "")], &[]), 1, (4, 1, 0), &mut failures);
    run("x20-note-when-E210", &note(&[("amount <= 50_000_000JPY", "amount <= 80_000_000JPY")], &[]), 1, (4, 1, 0), &mut failures);
    run("x20-note-W210", &note(&[(cond, "  when urgent or amount >= 1_000_000JPY")], &[]), 0, (4, 0, 1), &mut failures);
    let silent = [("outputs\n  summary : string\n\n", ""), ("  let n = summarize(title: title, terms: terms, amount: amount)\n  succeed summary = n.summary\n", "  summarize(title: title, terms: terms, amount: amount)\n")];
    run("x20-note-no-output-E210", &note(&[], &silent), 1, (4, 1, 0), &mut failures);
    // what a note holds: text, a whole number, a truth, rows of records, nothing; not a record on
    // its own, a list of what is no record, or `json`
    let output = |ty: &str, returns: &str, whole: bool| {
        let mut edits = vec![("outputs\n  summary : string\n", format!("outputs\n  summary : {ty}\n"))];
        if whole {
            edits.push(("succeed summary = n.summary", "succeed summary = n".to_string()));
        }
        if !returns.is_empty() {
            edits.push(("amount: money[JPY]) -> Note", format!("amount: money[JPY]) -> {returns}")));
        }
        let edits: Vec<(&str, &str)> = edits.iter().map(|(a, b)| (*a, b.as_str())).collect();
        note(&[], &edits)
    };
    run("x20-note-rows-held", &output("list[Note]", "list[Note]", true), 0, (5, 0, 0), &mut failures);
    run("x20-note-record-E210", &output("Note", "", true), 1, (4, 1, 0), &mut failures);
    run("x20-note-list-E210", &output("list[string]", "list[string]", true), 1, (4, 1, 0), &mut failures);
    run("x20-note-json-E210", &output("json", "", false), 1, (4, 1, 0), &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The reproductions of E911, E209 and E210, the request in `requests/` and what it reads in
/// `shared/`: the diagnostic is the same, in both languages, whether the request is found in the
/// directory, given by its name (the root is then `requests/`, and the gate, the book and the flow
/// are outside it) or given by its absolute path, but for the path given; and the JSON's records
/// are the same but for the files from the root.
/// A request with an organization, whose API `ritsu gen --target requests` writes.
const API_REQUEST: &str = "request purchase v1 \"Purchase request\"\n\nuse organization org from \"company.ringi\"\n\nform\n  amount  : money[JPY] \"Amount\"  range >=1JPY <=1_000_000JPY\n  purpose : text \"Purpose\"  count >=1 <=200\n\nroute\n  check  finance\n\nwho\n  finance = team finance\n\nticket\n  self approval  not allowed\n";

/// A flow that submits a purchase request through the API.
const API_FLOW: &str = "workflow restock v1\ndescription \"Asks for a purchase of the supplies that run low, through the server of the requests\"\n\nuse openapi requests from \"out/requests/openapi.json\"\n  url \"https://requests.example.com\"\n\nrecord Purchase\n  amount  : money[JPY]  range >=1JPY <=500_000JPY\n  purpose : string\n\nrecord Ticket\n  id : string\n\nrecord Moved\n  ticket : Ticket\n\ninputs\n  purchase : Purchase\n\noutputs\n  ticket : string\n\ntask submit_purchase(form: Purchase) -> Moved\n  http POST requests \"/requests/purchase/tickets\"\n  key\n\nflow\n  let moved = submit_purchase(form: purchase)\n  succeed ticket = moved.ticket.id\n";

/// The catalog the screens `requests` writes name.
const CATALOG: &str = "https://requests.example.com/a2ui/catalog.json";

#[test]
fn x20_a_flow_that_opens_a_ticket() {
    // the API written once, the flow put beside it after; each case a copy of the project
    let base = lay_out(&[("purchase.ringi", API_REQUEST.to_string()), ("company.ringi", COMPANY.to_string()), ("org/2026-04-01/teams.csv", TEAMS.to_string()), ("org/2026-04-01/people.csv", PEOPLE.to_string()), ("org/2026-04-01/members.csv", MEMBERS.to_string())], "x20-api");
    let gen_requests = ["gen", ".", "--target", "requests", "--catalog", CATALOG, "--out", "out"];
    let (code, out, err) = ritsu_in(base.path(), &gen_requests);
    assert_eq!(code, 0, "{out}{err}");
    std::fs::write(base.path().join("restock.flow"), API_FLOW).unwrap();
    let case = |name: &str, edits: &[(&str, &str, &str)], gone: &[&str]| {
        let t = TempDir::new(name);
        ritsu_testkit::tmp::copy_dir(base.path(), t.path());
        for (file, from, to) in edits {
            let p = t.path().join(file);
            let body = std::fs::read_to_string(&p).unwrap();
            assert!(body.contains(from), "{name}: {file} has no {from:?}");
            std::fs::write(&p, body.replacen(from, to, 1)).unwrap();
        }
        for g in gone {
            std::fs::remove_file(t.path().join(g)).unwrap();
        }
        t
    };
    let mut failures = Vec::new();
    run_at("x20-api-held", case("x20-api-held", &[], &[]).path(), 0, (1, 0, 0), &mut failures);
    // what the flow sends against a document that is the request's now: dandori's alone
    let types = case("x20-api-types", &[("restock.flow", "  purpose : string\n", "  purpose : int\n")], &[]);
    run_at("x20-api-types-E016", types.path(), 1, (0, 0, 0), &mut failures);
    let wide = case("x20-api-wide", &[("restock.flow", "range >=1JPY <=500_000JPY", "range >=1JPY <=2_000_000JPY")], &[]);
    run_at("x20-api-wide-E016", wide.path(), 1, (0, 0, 0), &mut failures);
    // the request changed after the document was written: its form, its version, its name
    let narrowed = ("purchase.ringi", "range >=1JPY <=1_000_000JPY", "range >=1JPY <=300_000JPY");
    let stale = case("x20-api-E210", &[narrowed], &[]);
    run_at("x20-api-E210", stale.path(), 1, (0, 1, 0), &mut failures);
    run_at("x20-api-version-E210", case("x20-api-version", &[("purchase.ringi", "request purchase v1", "request purchase v2")], &[]).path(), 1, (0, 1, 0), &mut failures);
    run_at("x20-api-renamed-E210", case("x20-api-renamed", &[("purchase.ringi", "request purchase v1", "request supplies v1")], &[]).path(), 1, (0, 1, 0), &mut failures);
    // the request's file gone: nothing to hold the document to
    run_at("x20-api-W210", case("x20-api-gone", &[], &["purchase.ringi"]).path(), 0, (0, 0, 1), &mut failures);
    // `ritsu gen` stops for the document out of date, as for any E210, when it writes another target
    let (code, out, err) = ritsu_in(stale.path(), &["gen", ".", "--target", "typescript", "--out", "other"]);
    assert!(code == 1 && err.contains("error[ritsu E210]: restock.flow:28:") && !stale.path().join("other").exists(), "{out}{err}");
    // and when it writes `requests`, the flow is held to the document the run writes, the request as
    // it is now: dandori says the flow sends what the request no longer takes, and nothing is
    // written (ringi's DESIGN 9.3)
    let document = stale.path().join("out/requests/openapi.json");
    let before = std::fs::read_to_string(&document).unwrap();
    let (code, out, err) = ritsu_in(stale.path(), &gen_requests);
    let said = format!("{out}{err}");
    // the range said in its unit, as the files write it (dandori's DESIGN 39.2)
    assert!(code == 1 && said.contains("dandori E016") && said.contains("`>=1JPY <=300_000JPY` is taken") && !said.contains("E210"), "{said}");
    assert_eq!(std::fs::read_to_string(&document).unwrap(), before, "nothing is written");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn x18_to_x20_say_the_same_however_the_request_is_given() {
    for code in ["E911", "E209", "E210"] {
        let files: Vec<(String, String)> = repro(code)
            .into_iter()
            .map(|(name, body)| match name {
                "purchase.ringi" => {
                    let mut body = body.to_string();
                    for (other, _) in repro(code).iter().filter(|(n, _)| *n != "purchase.ringi") {
                        assert!(body.contains(&format!("\"{other}\"")), "{code}: the request does not read {other}");
                        body = body.replace(&format!("\"{other}\""), &format!("\"../shared/{other}\""));
                    }
                    (format!("requests/{name}"), body)
                }
                other => (format!("shared/{other}"), body.to_string()),
            })
            .collect();
        let files: Vec<(&str, String)> = files.iter().map(|(n, b)| (n.as_str(), b.clone())).collect();
        let t = lay_out(&files, &format!("ritsu-given-{code}"));
        let top = t.path().to_string_lossy().to_string();
        let absolute = format!("{top}/requests/purchase.ringi");
        // what ritsu says across the borders: the diagnostic of the code, with its notes
        let said = |out: &str| -> String {
            let mut on = false;
            let mut kept = String::new();
            for l in out.lines() {
                if !l.starts_with("  ") && !l.starts_with("    ") {
                    on = l.contains(&format!("[ritsu {code}]"));
                }
                if on {
                    kept.push_str(l);
                    kept.push('\n');
                }
            }
            kept
        };
        for lang in ["en", "ja"] {
            let (code_dir, found, _) = ritsu_in(t.path(), &["check", ".", "--lang", lang]);
            let (code_name, named, _) = ritsu_in(t.path(), &["check", "requests/purchase.ringi", "--lang", lang]);
            let (code_abs, abs, _) = ritsu_in(t.path(), &["check", &absolute, "--lang", lang]);
            assert_eq!((code_dir, code_name, code_abs), (1, 1, 1), "{code} ({lang}):\n{found}\n{named}\n{abs}");
            let want = said(&found);
            assert!(want.contains(&format!("[ritsu {code}]: requests/purchase.ringi:")) && want.contains("shared/"), "{code} ({lang}): {found}");
            assert_eq!(said(&named), want, "{code} ({lang}), given by its name");
            assert_eq!(said(&abs).replace(&format!("{top}/"), ""), want, "{code} ({lang}), given by its absolute path");
            // and from the request's own directory, the files are written from there
            let (_, inside, _) = ritsu_in(&t.path().join("requests"), &["check", "purchase.ringi", "--lang", lang]);
            assert_eq!(said(&inside).replace("../shared/", "shared/"), want.replace("requests/purchase.ringi", "purchase.ringi"), "{code} ({lang}), given in its own directory");
        }
        // the records: the same checks about the same things with the same results
        let records = |args: &[&str]| -> Vec<(String, String, String, Option<String>)> {
            let (_, out, _) = ritsu_in(t.path(), args);
            let v: serde_json::Value = serde_json::from_str(&out).unwrap();
            records_agree(code, &v);
            v["borders"]["records"].as_array().unwrap().iter().map(|r| (r["check"].as_str().unwrap().to_string(), r["about"].as_str().unwrap().to_string(), r["result"].as_str().unwrap().to_string(), r["code"].as_str().map(str::to_string))).collect()
        };
        let found = records(&["check", ".", "--format", "json"]);
        assert!(found.iter().any(|r| r.3.as_deref() == Some(code)), "{code}: {found:?}");
        assert_eq!(records(&["check", "requests/purchase.ringi", "--format", "json"]), found, "{code}");
        assert_eq!(records(&["check", &absolute, "--format", "json"]), found, "{code}");
    }
}

/// Requests beside a project change nothing of what X2 to X6 say of it (ringi's PLAN C4.4): the
/// files a request is lowered to are no files of the project, and no module of kaname is lowered
/// yet, so the calls ritsu holds to the rules, the dates and the books are the flows' and the
/// modules' own. The reproductions of E201 (a rule's precondition), E202 (a day given to a rule),
/// E203 (a transfer's amount) and E206 (a hold against its expiry), each with its module of kaname
/// where it has one: the same records and the same diagnostics, with three requests and what they
/// read laid beside them.
#[test]
fn requests_beside_a_project_leave_the_other_borders_as_they_are() {
    let l = ritsu_cross::codes::ledger();
    let files_of = |code: &str| -> Vec<(String, String)> {
        match &l.find(code).expect("the code is in the ledger").repro {
            ritsu_base::ledger::Repro::Dir { files, .. } => files.iter().map(|(n, b)| (n.to_string(), b.to_string())).collect(),
            other => panic!("{code} has no files: {other:?}"),
        }
    };
    let own = |file: &serde_json::Value| file.as_str().is_some_and(|f| f.starts_with("requests-"));
    let mut modules = 0;
    for code in ["E201", "E202", "E203", "E206"] {
        let mut files = files_of(code);
        modules += files.iter().filter(|(n, _)| n.ends_with(".kn")).count();
        let said = |files: &[(String, String)]| -> (Vec<serde_json::Value>, Vec<serde_json::Value>, usize) {
            let laid: Vec<(&str, String)> = files.iter().map(|(n, b)| (n.as_str(), b.clone())).collect();
            let t = lay_out(&laid, &format!("ritsu-beside-{code}"));
            let (_, out, _) = ritsu_in(t.path(), &["check", ".", "--format", "json"]);
            let v: serde_json::Value = serde_json::from_str(&out).unwrap();
            let all = v["borders"]["records"].as_array().unwrap();
            let records: Vec<_> = all.iter().filter(|r| !own(&r["at"]["file"])).cloned().collect();
            let diagnostics = v["diagnostics"].as_array().unwrap().iter().filter(|d| !own(&d["file"])).cloned().collect();
            (records, diagnostics, all.len())
        };
        let (records, diagnostics, all) = said(&files);
        assert!(records.iter().any(|r| r["code"] == code) && records.len() == all, "{code}: {records:?}");
        for request in ["E911", "E209", "E210"] {
            files.extend(files_of(request).into_iter().map(|(n, b)| (format!("requests-{request}/{n}"), b)));
        }
        let (with_records, with_diagnostics, with_all) = said(&files);
        assert_eq!((with_records, with_diagnostics), (records, diagnostics), "{code}");
        // and the requests' own borders are there beside them: 3 of the gate, 1 of the hold, 2 of the flow
        assert_eq!(with_all - all, 6, "{code}");
    }
    assert_eq!(modules, 3, "three of the four have a module of kaname");
}
/// Who refunds an order (fictional): the returns workflow itself up to 50 pounds, a clerk above.
const REFUND_ROUTE: &str = "rule refund_route v1\ndescription \"Who refunds an order: the returns workflow itself up to 50 pounds, a clerk above it\"\n\nenum refunder = itself \"Itself\" | clerk \"Clerk\"\n\ninputs\n  amount : money[GBP, incl_tax]  range >=1GBP <=10_000GBP\n\noutputs\n  who : refunder\n\ntable route\n| amount  | -> who |\n| <=50GBP | itself |\n| >50GBP  | clerk  |\n\nexamples\n| amount | -> who |\n| 50GBP  | itself |\n| 51GBP  | clerk  |\n";

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

/// The borders of one project in the JSON, one record each (DESIGN 8.3): the reproduction of E202,
/// whose dates file, rule, flow and module of kaname cross koyomi, rulec, dandori and kaname. The
/// flow's call of the rule and the module's fail (E202), the flow's day given to the dates file is
/// undecided (W205), and the module's holds, its range being the input's. The records agree with
/// the counts, take in all three results and the four languages, come the same on a second run,
/// and are held to a golden file; the module's file has kaname's own summary and the effect of its
/// function.
#[test]
fn each_border_has_its_record() {
    let l = ritsu_cross::codes::ledger();
    let files: Vec<(&str, String)> = match &l.find("E202").expect("E202 is in the ledger").repro {
        ritsu_base::ledger::Repro::Dir { files, .. } => files.iter().map(|(n, b)| (*n, b.to_string())).collect(),
        other => panic!("E202 has no files: {other:?}"),
    };
    assert!(files.iter().any(|(n, _)| n.ends_with(".kn")), "the reproduction calls from a module of kaname");
    let t = lay_out(&files, "records");
    let (code, out, err) = ritsu_in(t.path(), &["check", ".", "--format", "json"]);
    assert_eq!(code, 1, "{out}{err}");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    records_agree("E202", &v);
    let records = v["borders"]["records"].as_array().unwrap();
    let results: std::collections::BTreeSet<&str> = records.iter().map(|r| r["result"].as_str().unwrap()).collect();
    assert_eq!(results.into_iter().collect::<Vec<_>>(), ["failed", "held", "undecided"]);
    let tools: std::collections::BTreeSet<&str> = records.iter().flat_map(|r| [&r["at"], &r["to"]]).filter_map(|s| s["tool"].as_str()).collect();
    assert_eq!(tools.into_iter().collect::<Vec<_>>(), ["dandori", "kaname", "koyomi", "rulec"]);
    let (_, again, _) = ritsu_in(t.path(), &["check", ".", "--format", "json"]);
    assert_eq!(out, again, "the same project, the same JSON");
    let mut failures = Vec::new();
    golden("records-E202.json", &(serde_json::to_string_pretty(&v["borders"]).unwrap() + "\n"), &mut failures);
    let module = v["files"].as_array().unwrap().iter().find(|f| f["tool"] == "kaname").expect("the module is a file of the project");
    assert_eq!(module["check"]["summary"], "1 function, fails rule", "{module}");
    assert_eq!(module["check"]["functions"][0]["effect"], serde_json::json!({"fails": ["rule"], "diverges": false}), "{module}");
    for k in ["file", "diagnostics"] {
        assert!(module["check"].get(k).is_none(), "{k} is the JSON's own: {module}");
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
