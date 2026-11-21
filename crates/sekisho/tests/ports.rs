//! What sekisho answers through ritsu's ports (DESIGN 8.2, 8.3): what a gate holds (`Items`), what
//! it names outside itself (`References`), and what the checks across the borders ask of it
//! (`Gates`). The answers for both versions of the example are golden files in
//! `tests/golden/ports/` (`SEKISHO_BLESS=1` writes them); how far an asker is allowed an action is
//! held to what the check of every combination counts.

mod common;

use ritsu_base::naming::{self, Tool};
use ritsu_ports::{Allowance, Asker, Found, GateGiven, GateGivenValue, Gates, Items, References};
use sekisho::ports::Engine;
use std::path::{Path, PathBuf};

fn engine() -> Engine {
    Engine::new(common::joined())
}

fn root() -> PathBuf {
    std::fs::canonicalize("examples/refunds").unwrap()
}

/// What the ports say of a gate of the example: its things, its references, and its facts.
fn answers(e: &Engine, file: &str) -> String {
    let root = root();
    let mut out = String::from("items\n");
    let src = std::fs::read_to_string(root.join(file)).unwrap();
    let n = src.lines().count();
    for i in e.items(&root, file).unwrap() {
        assert!(i.lines.0 >= 1 && i.lines.0 <= i.lines.1 && i.lines.1 <= n, "{file}: {i:?}");
        assert_eq!(naming::parse_one(&i.naming.text()).ok().as_ref(), Some(&i.naming), "{file}: the naming reads back");
        out.push_str(&format!("  {}-{} {}\n", i.lines.0, i.lines.1, i.naming.text()));
        for l in i.text.lines() {
            out.push_str(&format!("      {l}\n"));
        }
    }
    out.push_str("references\n");
    for r in e.references(&root, file).unwrap() {
        assert!(root.join(&r.target.path).is_file(), "{file}: {} is not there", r.target.text());
        out.push_str(&format!("  {} {} [{}]\n", r.line, r.target.text(), r.how));
    }
    let f = e.facts(&root, file).unwrap();
    out.push_str(&format!("facts\n  {} ({}) {} {} sha256:{}\n", f.name, f.alias, f.version, f.namespace, &f.sha256[..16]));
    for a in &f.actions {
        out.push_str(&format!("  action {} ({}) line {}: principals {:?}, resources {:?}, nobody {:?}\n", a.name, a.alias, a.line, a.principals, a.resources, a.nobody));
        for (g, l) in &a.guards {
            out.push_str(&format!("    guards {} (line {l})\n", g.text()));
        }
        for i in &a.inputs {
            out.push_str(&format!("    input {} ({}) line {}: {}\n", i.name, i.alias, i.line, holds(&i.holds)));
        }
    }
    for w in &f.workflows {
        out.push_str(&format!("  workflow {} ({}) line {}: {}\n", w.name, w.alias, w.line, w.flow.text()));
    }
    for p in &f.policies {
        out.push_str(&format!("  {} {} @id {} line {}\n", if p.permit { "permit" } else { "forbid" }, p.name, p.id, p.line));
    }
    for (x, l) in &f.expects {
        out.push_str(&format!("  expect {x} line {l}\n"));
    }
    for (x, l) in &f.separations {
        out.push_str(&format!("  separate {x} line {l}\n"));
    }
    out
}

/// What an input holds, as the golden files write it: a number's type as the gate writes it, the
/// type of its unit, and its range on the wire; an enum's values; `?` where it may be absent.
fn holds(c: &ritsu_ports::Carried) -> String {
    use ritsu_ports::Carried;
    match c {
        Carried::Num { written, unit, min, max } => format!("{written} ({}) {min:?}..{max:?}", unit.as_ref().map_or_else(|| "no unit".into(), |u| u.to_string())),
        Carried::Enum { name, values } => format!("{name} [{}]", values.iter().map(|v| format!("{} {}", v.written, v.public)).collect::<Vec<_>>().join(", ")),
        Carried::Opt(t) => format!("{}?", holds(t)),
        other => format!("{other:?}"),
    }
}

#[test]
fn what_the_ports_say_of_the_example() {
    let e = engine();
    let mut failures = Vec::new();
    for (file, golden) in [("refunds.gate", "refunds.txt"), ("refunds.ja.gate", "refunds.ja.txt")] {
        if let Err(f) = ritsu_testkit::golden::check(&Path::new("tests/golden/ports").join(golden), &answers(&e, file)) {
            failures.push(f);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// How far each asker is allowed each action of the example, over every combination the check
/// walks, the two versions alike.
#[test]
fn how_far_an_asker_is_allowed_an_action() {
    let e = engine();
    let root = root();
    let roles = |ty: &str, rs: &[&str]| Asker::Roles { ty: ty.into(), roles: rs.iter().map(|r| r.to_string()).collect() };
    for (file, returns) in [("refunds.gate", "returns"), ("refunds.ja.gate", "返品")] {
        let ask = |action: &str, asker: &Asker| -> &'static str {
            match e.allowed(&root, file, action, asker).unwrap_or_else(|s| panic!("{file} {action}: {s:?}")) {
                Found::Value(Allowance::Always) => "always",
                Found::Value(Allowance::Sometimes { .. }) => "sometimes",
                Found::Value(Allowance::Never) => "never",
                Found::Undecided(_) => "undecided",
            }
        };
        // the workflow refunds a returned order up to 50 pounds, and looks at no order
        assert_eq!(ask("refund_order", &Asker::Workflow(returns.into())), "sometimes", "{file}");
        assert_eq!(ask("view_order", &Asker::Workflow(returns.into())), "never", "{file}");
        // a clerk refunds within the limit while the period lasts, unless suspended; an auditor
        // never refunds; a manager looks at an order, as the clerk the role includes
        assert_eq!(ask("refund_order", &roles("User", &["clerk"])), "sometimes", "{file}");
        assert_eq!(ask("refund_order", &roles("User", &["auditor"])), "never", "{file}");
        assert_eq!(ask("view_order", &roles("User", &["manager"])), "sometimes", "{file}");
        assert_eq!(ask("export_refunds", &roles("User", &["clerk"])), "never", "{file}");
        // a customer looks at the orders that are the customer's own
        assert_eq!(ask("view_order", &roles("Customer", &[])), "sometimes", "{file}");
    }
    // the examples of each answer are in both languages
    let Found::Value(Allowance::Sometimes { allowed, denied }) = e.allowed(&root, "refunds.gate", "refund_order", &Asker::Workflow("returns".into())).unwrap() else { panic!() };
    assert!(!allowed.en.is_empty() && !allowed.ja.is_empty() && !denied.en.is_empty() && !denied.ja.is_empty());
    // an action the gate does not have is no question it answers
    assert!(e.allowed(&root, "refunds.gate", "delete_order", &Asker::Workflow("returns".into())).is_err());
}

/// A gate of who does what to a ticket (fictional): the approvers of a stage answer it, no one
/// decides their own request unless its route lets them, and an urgent stage is not handed over.
const TICKETS: &str = "gate company v1
description \"Who does what to a ticket\"
namespace Tickets

enum level = normal | urgent

principal Staff
principal Robot
resource Approvers
resource Ticket
resource Stage
  attributes
    requester     : Staff
    approvers     : Approvers
    self_approval : bool
    urgency       : level

action submit
  principal Staff
  resource Ticket
action approve
  principal Staff, Robot
  resource Stage
action hand_over
  principal Staff
  resource Stage

permit staff_submit_and_hand_over
  principal is Staff
  action submit, hand_over

permit approvers_answer_their_stage
  principal is Staff
  action approve
  when principal in resource.approvers

forbid no_one_decides_their_own
  principal is Staff
  action approve
  when resource.requester is principal
  unless resource.self_approval

forbid an_urgent_stage_stays
  principal is Staff
  action hand_over
  when resource.urgency is urgent
";

/// How far an action is allowed where some attributes are held to values (`Gates::allowed_with`):
/// over the combinations the check walks that agree with what is held, and every value of the rest.
#[test]
fn how_far_an_action_is_allowed_with_attributes_held() {
    let e = engine();
    let t = ritsu_testkit::TempDir::new("held");
    std::fs::write(t.path().join("company.gate"), TICKETS).unwrap();
    let ask = |action: &str, principal: Option<&str>, given: &[GateGiven]| e.allowed_with(t.path(), "company.gate", action, principal, given).unwrap_or_else(|s| panic!("{action}: {s:?}"));
    let word = |a: &Found<Allowance>| match a {
        Found::Value(Allowance::Always) => "always",
        Found::Value(Allowance::Sometimes { .. }) => "sometimes",
        Found::Value(Allowance::Never) => "never",
        Found::Undecided(_) => "undecided",
    };
    let requester = |is: bool| GateGiven::resource("requester", GateGivenValue::IsPrincipal(is));
    let approvers = |holds: bool| GateGiven::resource("approvers", GateGivenValue::HoldsPrincipal(holds));
    let own = |allowed: bool| GateGiven::resource("self_approval", GateGivenValue::Bool(allowed));
    let urgency = |v: &str| GateGiven::resource("urgency", GateGivenValue::Enum(v.to_string()));
    // nothing held is the question of every combination: an approver is allowed, another is not
    assert_eq!(word(&ask("approve", Some("Staff"), &[])), "sometimes");
    // an approver who is not the requester is allowed in every combination
    assert_eq!(word(&ask("approve", Some("Staff"), &[approvers(true), requester(false), own(false)])), "always");
    assert_eq!(word(&ask("approve", Some("Staff"), &[approvers(false)])), "never");
    // the requester: never where the route does not let them, as far as they are an approver where
    // it does
    assert_eq!(word(&ask("approve", Some("Staff"), &[requester(true), own(false)])), "never");
    let some = ask("approve", Some("Staff"), &[requester(true), own(true)]);
    let Found::Value(Allowance::Sometimes { allowed, denied }) = &some else { panic!("{some:?}") };
    assert!(allowed.en.contains("principal is in resource.approvers") && denied.en.contains("principal is not in resource.approvers"), "{allowed:?} {denied:?}");
    assert!(!allowed.ja.is_empty() && !denied.ja.is_empty());
    assert_eq!(word(&ask("approve", Some("Staff"), &[requester(true), approvers(true), own(true)])), "always");
    // a value of an enum
    assert_eq!(word(&ask("hand_over", None, &[urgency("urgent")])), "never");
    assert_eq!(word(&ask("hand_over", None, &[urgency("normal")])), "always");
    assert_eq!(word(&ask("hand_over", None, &[])), "sometimes");
    // the type of the principal: every type the action takes for None, and a type no policy
    // permits is never allowed; a type the gate does not have is never allowed either
    assert_eq!(word(&ask("approve", None, &[approvers(true), requester(false)])), "sometimes");
    assert_eq!(word(&ask("approve", Some("Robot"), &[approvers(true), requester(false)])), "never");
    assert_eq!(word(&ask("approve", Some("Nobody"), &[])), "never");
    // an attribute held to be the principal, of another type than the principal's, leaves no
    // combination
    assert_eq!(word(&ask("approve", Some("Robot"), &[requester(true), approvers(true), own(true)])), "never");
    // an attribute the action's resource does not have holds nothing
    assert_eq!(word(&ask("submit", None, &[own(false), urgency("urgent")])), "always");
    // a value the attribute's type cannot have is undecided, with the reason
    for (given, why) in [
        (GateGiven::resource("requester", GateGivenValue::Bool(true)), "`resource.requester` is not a bool"),
        (GateGiven::resource("self_approval", GateGivenValue::Enum("urgent".into())), "`resource.self_approval` is not an enum"),
        (GateGiven::resource("self_approval", GateGivenValue::IsPrincipal(true)), "`resource.self_approval` does not point to an entity"),
    ] {
        match ask("approve", Some("Staff"), &[given.clone()]) {
            Found::Undecided(t) => assert_eq!(t.en, why, "{given:?}"),
            other => panic!("{given:?}: {other:?}"),
        }
    }
    assert_eq!(word(&ask("hand_over", None, &[urgency("slow")])), "undecided");
    // what is held, as a message shows it
    assert_eq!(requester(true).text().en, "`resource.requester` is the principal");
    assert_eq!(approvers(true).text().en, "the principal is in `resource.approvers`");
    assert_eq!(own(false).text().en, "`resource.self_approval` is false");
    assert_eq!(GateGiven::principal("team", GateGivenValue::Enum("sales".into())).place(), "principal.team");
    // an action the gate does not have is no question it answers; Cedar written by hand keeps the
    // port's default
    assert!(e.allowed_with(t.path(), "company.gate", "delete", None, &[]).is_err());
    assert_eq!(word(&e.allowed_with(t.path(), "policies.cedar", "approve", None, &[own(true)]).unwrap()), "undecided");
    // the question with nothing held agrees with the question of an asker
    let asker = e.allowed(t.path(), "company.gate", "approve", &Asker::Roles { ty: "Staff".into(), roles: Vec::new() }).unwrap();
    assert_eq!(word(&asker), "sometimes");
}

/// A gate that does not pass its check answers no question of the checks across the borders, but
/// still says what it holds and names.
#[test]
fn a_gate_that_does_not_pass_says_why() {
    let e = engine();
    let here = std::fs::canonicalize(".").unwrap();
    let file = "tests/mutants/E101_unknown_role.gate";
    let said = e.facts(&here, file).unwrap_err();
    assert!(said.iter().any(|s| s.code == "E101"), "{said:?}");
    assert!(e.allowed(&here, file, "refund_order", &Asker::Workflow("returns".into())).is_err());
    assert!(e.items(&here, file).unwrap().iter().any(|i| i.kind() == "policy"));
    // one whose names hold names the operations its actions guard, as its check found them
    let covered = "tests/mutants/E302_forbid_covers_permit.gate";
    assert!(e.facts(&here, covered).unwrap_err().iter().any(|s| s.code == "E302"));
    let refs = e.references(&here, covered).unwrap();
    assert!(refs.iter().any(|r| r.how == "use rule" && r.target.text() == "rulec \"examples/refunds/rules/refund_limit.rule\""), "{refs:?}");
    assert!(refs.iter().any(|r| r.how == "guards" && r.target.text() == "openapi \"examples/refunds/api/orders.json\" operation refundOrder"), "{refs:?}");
    // a file that does not parse says what the parser says, and one that is not there why
    let t = ritsu_testkit::TempDir::new("ports");
    std::fs::write(t.path().join("broken.gate"), "gate broken v1\ndescription \"not closed\n").unwrap();
    let said = e.items(t.path(), "broken.gate").unwrap_err();
    assert_eq!(said[0].code, "E001", "{said:?}");
    assert_eq!(e.items(t.path(), "missing.gate").unwrap_err()[0].code, "");
    assert_eq!(Tool::from_word("sekisho"), Some(Tool::Sekisho));
}

/// A gate's check is kept for the run its engine is made for (ritsu's DESIGN 4.15): each command
/// and each question of the browser page makes its own engine (`ritsu_project::Joined`), and nothing
/// in a run writes a gate, so the engine reads nothing again; the next run's engine checks the gate
/// as it is then.
#[test]
fn a_gate_is_kept_for_the_run_and_read_again_by_the_next() {
    let t = ritsu_testkit::TempDir::new("kept-gate");
    let gate = t.write("company.gate", TICKETS);
    let e = engine();
    for _ in 0..2 {
        assert!(matches!(e.outcome_of(t.path(), &gate).as_ref(), Ok(o) if !o.has_errors()));
    }
    assert_eq!(e.checks(), 1);
    t.write("company.gate", TICKETS.replace("action submit, hand_over", "action submit, hand_off"));
    let next = engine();
    assert!(matches!(next.outcome_of(t.path(), &gate).as_ref(), Ok(o) if o.has_errors()));
    assert_eq!(next.checks(), 1);
}

/// A gate is one file however its path is written (ritsu's DESIGN 4.15): `ritsu check` given it under
/// four spellings of its path, then the ports and `ritsu gen` asking of it, check it once in a run,
/// and what the ports say of one that does not pass names it one way.
#[test]
fn a_gate_is_one_file_however_its_path_is_written() {
    let e = engine();
    let t = ritsu_testkit::TempDir::new("spelled");
    t.write("company.gate", TICKETS);
    t.write("wrong.gate", TICKETS.replace("action submit, hand_over", "action submit, hand_off"));
    let _run = ritsu_base::paths::run_from(".");
    let given: Vec<String> = t.spellings("company.gate").iter().map(|p| p.display().to_string()).collect();
    let units = e.checked(t.path(), &given, ritsu_base::text::Lang::En);
    assert!(units.iter().all(|u| u.verdict == ritsu_ports::Verdict::Passes), "{}", units[0].text());
    for f in ["company.gate", "./company.gate", "aside/../company.gate"] {
        assert_eq!(e.facts(t.path(), f).unwrap_or_else(|s| panic!("{f}: {s:?}")).name, "company");
    }
    for p in t.spellings("company.gate") {
        assert!(e.outcome_of(t.path(), &p).is_ok(), "{}", p.display());
    }
    assert_eq!(e.checks(), 1, "one check for every spelling");
    let said: Vec<_> = ["wrong.gate", "./wrong.gate", "aside/../wrong.gate"].iter().map(|f| e.facts(t.path(), f).unwrap_err()).collect();
    assert!(!said[0].is_empty() && said[0].iter().all(|s| !s.code.is_empty()), "{:?}", said[0]);
    assert!(said.windows(2).all(|w| w[0] == w[1]), "{said:?}");
    assert_eq!(e.checks(), 2);
}
