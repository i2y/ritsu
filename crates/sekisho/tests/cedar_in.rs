//! Cedar written by hand, as the port `Gates` answers for it (DESIGN 1.3): a policy set and the
//! schema beside it. The material is in `tests/cedar_in/`, in English and in Japanese:
//!
//! - `refunds.cedar` and `refunds.cedarschema`: what sekisho generates of the example's
//!   `refunds.gate`, copied as a person who keeps Cedar by hand would keep it (`refunds.ja.*`, of
//!   `refunds.ja.gate`). The port reads the same actions and operations from it as from the gate,
//!   and answers how far each asker is allowed each action as the gate does.
//! - `outside.cedar` and `outside.cedarschema`: a library whose policies for printing read what
//!   sekisho does not count (`like`, arithmetic); the questions about printing are undecided, and the
//!   others are answered.

mod common;

use ritsu_ports::{Allowance, Asker, Found, GateFacts, Gates, References};
use sekisho::ports::Engine;
use std::path::PathBuf;

const DIR: &str = "tests/cedar_in";

fn here() -> PathBuf {
    std::fs::canonicalize(".").unwrap()
}

fn answer(f: Found<Allowance>) -> String {
    match f {
        Found::Value(Allowance::Always) => "always".into(),
        Found::Value(Allowance::Sometimes { .. }) => "sometimes".into(),
        Found::Value(Allowance::Never) => "never".into(),
        Found::Undecided(why) => format!("undecided: {}", why.en),
    }
}

/// The actions of a gate or of a pair of Cedar: each name, the operations it guards, and the types
/// it applies to, in the order of the names.
fn actions(f: &GateFacts) -> Vec<String> {
    let mut out: Vec<String> = f
        .actions
        .iter()
        .map(|a| {
            let mut guards: Vec<String> = a.guards.iter().map(|(n, _)| n.text()).collect();
            guards.sort();
            let (mut ps, mut rs) = (a.principals.clone(), a.resources.clone());
            ps.sort();
            rs.sort();
            format!("{} guards {guards:?} principals {ps:?} resources {rs:?}", a.alias)
        })
        .collect();
    out.sort();
    out
}

#[test]
fn a_copy_of_what_sekisho_generates_holds_what_the_gate_does() {
    let e = Engine::new(common::joined());
    let example = std::fs::canonicalize("examples/refunds").unwrap();
    for (gate, stem, ids) in [("refunds.gate", "refunds", "refunds/"), ("refunds.ja.gate", "refunds.ja", "refunds_ja/")] {
        let g = e.facts(&example, gate).unwrap();
        let c = e.facts(&here(), &format!("{DIR}/{stem}.cedar")).unwrap();
        assert_eq!(actions(&c), actions(&g), "{stem}");
        assert_eq!(c.namespace, g.namespace, "{stem}");
        // the policies by their @id
        let mut want: Vec<(String, bool)> = g.policies.iter().map(|p| (p.id.clone(), p.permit)).collect();
        let mut got: Vec<(String, bool)> = c.policies.iter().map(|p| (p.id.clone(), p.permit)).collect();
        want.sort();
        got.sort();
        assert_eq!(got, want, "{stem}");
        assert!(got.iter().all(|(id, _)| id.starts_with(ids)), "{got:?}");
        // the schema of the pair answers for the pair too
        assert_eq!(e.facts(&here(), &format!("{DIR}/{stem}.cedarschema")).unwrap().actions, c.actions, "{stem}");
        // the operations it names, for sakai
        let refs: Vec<String> = e.references(&here(), &format!("{DIR}/{stem}.cedarschema")).unwrap().iter().map(|r| format!("{} [{}]", r.target.text(), r.how)).collect();
        assert_eq!(refs.len(), 3, "{refs:?}");
        assert!(refs.contains(&"openapi \"api/orders.json\" operation refundOrder [guards]".to_string()), "{refs:?}");
        assert!(e.references(&here(), &format!("{DIR}/{stem}.cedar")).unwrap().is_empty());
    }
}

/// Every asker allowed every action as far as the gate allows it: the policies are the gate's, and
/// sekisho counts them the same, in Cedar as in the `.gate`. A role that includes another is given
/// with the one it includes, as Cedar's entities would hold it.
#[test]
fn a_copy_of_what_sekisho_generates_allows_what_the_gate_does() {
    let e = Engine::new(common::joined());
    let example = std::fs::canonicalize("examples/refunds").unwrap();
    let roles = |ty: &str, rs: &[&str]| Asker::Roles { ty: ty.into(), roles: rs.iter().map(|r| r.to_string()).collect() };
    let askers = [
        (roles("User", &["clerk"]), roles("User", &["clerk"])),
        (roles("User", &["manager"]), roles("User", &["manager", "clerk"])),
        (roles("User", &["auditor"]), roles("User", &["auditor"])),
        (roles("User", &["clerk", "auditor"]), roles("User", &["clerk", "auditor"])),
        (roles("User", &[]), roles("User", &[])),
        (roles("Customer", &[]), roles("Customer", &[])),
        (Asker::Workflow("returns".into()), Asker::Workflow("returns".into())),
    ];
    let mut seen = std::collections::BTreeSet::new();
    for (gate, stem) in [("refunds.gate", "refunds"), ("refunds.ja.gate", "refunds.ja")] {
        for action in ["view_order", "refund_order", "export_refunds"] {
            for (of_gate, of_cedar) in &askers {
                let want = answer(e.allowed(&example, gate, action, of_gate).unwrap());
                let got = answer(e.allowed(&here(), &format!("{DIR}/{stem}.cedar"), action, of_cedar).unwrap());
                assert_eq!(got, want, "{stem} {action} {of_cedar:?}");
                seen.insert(got);
            }
        }
    }
    // no one is allowed anything of the example in every combination (a suspended member of the
    // staff does nothing, an order refunded is not refunded again)
    assert_eq!(seen.into_iter().collect::<Vec<_>>(), ["never", "sometimes"]);
}

/// A policy that reads what sekisho does not count leaves the questions about its action
/// undecided, and says which policy and what; the other actions are answered.
#[test]
fn what_sekisho_does_not_count_is_undecided() {
    let e = Engine::new(common::joined());
    let reader = Asker::Roles { ty: "User".into(), roles: vec!["reader".into()] };
    let nobody = Asker::Roles { ty: "User".into(), roles: vec![] };
    for (stem, line) in [("outside", 19), ("outside.ja", 21)] {
        let file = format!("{DIR}/{stem}.cedar");
        let ask = |action: &str, who: &Asker| answer(e.allowed(&here(), &file, action, who).unwrap());
        assert_eq!(ask("read", &reader), "sometimes", "{stem}");
        assert_eq!(ask("read", &nobody), "never", "{stem}");
        assert_eq!(ask("archive", &nobody), "sometimes", "{stem}");
        // the first policy of the action that reads what is not counted: `like`
        assert_eq!(ask("print", &reader), format!("undecided: the like at line {line}, column 8 is outside the finite part sekisho counts, in the policy drafts_are_not_printed"), "{stem}");
        let Found::Undecided(why) = e.allowed(&here(), &file, "print", &reader).unwrap() else { panic!() };
        assert_eq!(why.ja, format!("ポリシー drafts_are_not_printed の、{line} 行 8 列の like は、sekisho が数える有限の部分の外です"));
        // what the pair holds is read all the same
        let f = e.facts(&here(), &file).unwrap();
        assert_eq!(actions(&f).len(), 3);
        assert_eq!(f.policies.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["readers_read", "drafts_are_not_printed", "readers_print_within_their_limit", "owners_archive"]);
    }
}

/// A pair that does not read, or reads but names an operation in no form of a reference, answers
/// with why; so does a question about an action the schema does not have.
#[test]
fn a_pair_that_does_not_read_says_why() {
    let e = Engine::new(common::joined());
    let t = ritsu_testkit::TempDir::new("cedar-in");
    let write = |name: &str, body: &str| std::fs::write(t.path().join(name), body).unwrap();
    // no schema beside the policies
    write("lone.cedar", "permit (principal, action, resource);\n");
    let said = e.facts(t.path(), "lone.cedar").unwrap_err();
    assert_eq!(said[0].message.en, "There is no schema (lone.cedarschema or lone.cedarschema.json) beside lone.cedar");
    // a schema whose @guards names no reference
    write("bad.cedarschema", "@guards(\"orders refundOrder\")\naction \"refund\";\n");
    let said = e.facts(t.path(), "bad.cedarschema").unwrap_err();
    assert!(said[0].message.en.starts_with("Line 1 of the `@guards` of the action `refund`, `orders refundOrder`, does not read as a reference: "), "{said:?}");
    assert!(e.references(t.path(), "bad.cedarschema").is_err());
    // a policy set that does not read
    write("broken.cedar", "permit (principal, action, resource\n");
    write("broken.cedarschema", "action \"read\";\n");
    assert!(e.facts(t.path(), "broken.cedar").is_err());
    // an action the schema does not have
    let reader = Asker::Roles { ty: "User".into(), roles: vec![] };
    assert!(e.allowed(&here(), &format!("{DIR}/outside.cedar"), "burn", &reader).is_err());
    // a schema with no policies beside it allows nothing; a permit with no condition, everything
    write("alone.cedarschema", "entity User;\nentity Doc;\naction \"read\" appliesTo { principal: [User], resource: [Doc] };\n");
    assert_eq!(answer(e.allowed(t.path(), "alone.cedarschema", "read", &reader).unwrap()), "never");
    write("open.cedarschema", "entity User;\nentity Doc;\naction \"read\" appliesTo { principal: [User], resource: [Doc] };\n");
    write("open.cedar", "permit (principal, action == Action::\"read\", resource);\n");
    assert_eq!(answer(e.allowed(t.path(), "open.cedar", "read", &reader).unwrap()), "always");
}
