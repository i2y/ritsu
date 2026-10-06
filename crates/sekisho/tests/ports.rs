//! What sekisho answers through ritsu's ports (DESIGN 8.2, 8.3): what a gate holds (`Items`), what
//! it names outside itself (`References`), and what the checks across the borders ask of it
//! (`Gates`). The answers for both versions of the example are golden files in
//! `tests/golden/ports/` (`SEKISHO_BLESS=1` writes them); how far an asker is allowed an action is
//! held to what the check of every combination counts.

mod common;

use ritsu_base::naming::{self, Tool};
use ritsu_ports::{Allowance, Asker, Found, Gates, Items, References};
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
