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
//! - `documents.cedar` and `documents.cedarschema`: a company's documents, folders and templates,
//!   the staff and a bot that indexes them, written with every form of condition sekisho counts —
//!   some the Cedar it generates never has (two strings compared, two booleans compared, `if`, a
//!   resource in a folder of folders, one entity written in two policies) — and principals of a
//!   type the schema keeps out of a group or a team.
//!
//! Small pairs written here (`SMALL`) each have a question whose answer turns on what the schema
//! rules out: a combination no request makes is not counted.
//!
//! Then every combination sekisho counts is held to the official Cedar CLI 4.13.0 (DESIGN 6.5):
//! each question of each action, asked of each type of principal the action takes with every set
//! of the roles its policies name (and of each workflow they name, and one they do not), comes to
//! tests of `cedar run-tests` — the request and the entities of every combination (an integer at
//! both ends of its cell), with the decision and the policies that decide it. The CLI runs them
//! with the pair's policies and schema, and with each policy alone, made a permit: the decision and
//! the policies that decide it are the CLI's, test by test. A question sekisho does not decide is
//! held to the policy and the part of it the reason names. This is done for the material, for the
//! Cedar sekisho generates of every `.gate` that passes its check (read as Cedar written by hand:
//! every question is decided), and for every change of the material at one place.

mod common;

use common::cedar_cli::{Cli, each_on_threads, gates, policy_changes};
use ritsu_base::cedar::{self, ActionScope, BinOp, Effect, EntityOrSlot, Expr, ExprKind, Name, Policy, PolicySet, Schema, Scope, Var};
use ritsu_base::json::Json;
use ritsu_ports::{Allowance, Asker, Found, GateFacts, Gates, References};
use ritsu_testkit::TempDir;
use sekisho::cedar_in::{self, Undecided};
use sekisho::ports::Engine;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

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

/// A small pair whose answers turn on what the schema rules out, or on a form a count can get
/// wrong (DESIGN 1.3): what it shows, its schema and its policies, and the answers to some of its
/// questions (the action, the type of the principal, its roles, the answer).
struct Small {
    what: &'static str,
    schema: &'static str,
    policies: &'static str,
    answers: &'static [(&'static str, &'static str, &'static [&'static str], &'static str)],
}

const SMALL: &[Small] = &[
    Small {
        what: "a bot owns no document: the owner is a user",
        schema: "entity User;\nentity Bot;\nentity Doc = { owner: User };\naction \"edit\" appliesTo { principal: [User, Bot], resource: [Doc] };\n",
        policies: "permit (principal, action == Action::\"edit\", resource)\nwhen { resource.owner == principal };\n",
        answers: &[("edit", "User", &[], "sometimes"), ("edit", "Bot", &[], "never")],
    },
    Small {
        what: "a bot is in no team: the schema puts only users in teams",
        schema: "entity Team;\nentity User in [Team];\nentity Bot;\nentity Doc = { team: Team };\naction \"edit\" appliesTo { principal: [User, Bot], resource: [Doc] };\n",
        policies: "permit (principal, action == Action::\"edit\", resource)\nwhen { principal in resource.team };\n",
        answers: &[("edit", "User", &[], "sometimes"), ("edit", "Bot", &[], "never")],
    },
    Small {
        what: "a customer holds no role, whatever the asker says: the schema puts only users in roles",
        schema: "entity Role;\nentity User in [Role];\nentity Customer;\nentity Order;\naction \"view\" appliesTo { principal: [User, Customer], resource: [Order] };\n",
        policies: "permit (principal in Role::\"clerk\", action == Action::\"view\", resource);\n",
        answers: &[("view", "User", &["clerk"], "always"), ("view", "Customer", &["clerk"], "never")],
    },
    Small {
        what: "a template is in no folder, so the archive's forbid never reaches one",
        schema: "entity User;\nentity Folder in [Folder];\nentity Template;\naction \"use\" appliesTo { principal: [User], resource: [Template] };\n",
        policies: "forbid (principal, action, resource)\nwhen { resource in Folder::\"archive\" };\n\npermit (principal, action == Action::\"use\", resource);\n",
        answers: &[("use", "User", &[], "always")],
    },
    Small {
        what: "the archive is in itself, so the archive's forbid reaches the archive",
        schema: "entity User;\nentity Folder;\naction \"list\" appliesTo { principal: [User], resource: [Folder] };\n",
        policies: "forbid (principal, action, resource)\nwhen { resource in Folder::\"archive\" };\n\npermit (principal, action == Action::\"list\", resource)\nwhen { resource == Folder::\"archive\" };\n",
        answers: &[("list", "User", &[], "never")],
    },
    Small {
        what: "one entity written in two policies is one entity",
        schema: "entity User;\nentity Bot;\nentity Folder;\naction \"reindex\" appliesTo { principal: [User, Bot], resource: [Folder] };\n",
        policies: "forbid (principal, action == Action::\"reindex\", resource)\nunless { principal == Bot::\"indexer\" };\n\npermit (principal == Bot::\"indexer\", action == Action::\"reindex\", resource);\n",
        answers: &[("reindex", "Bot", &[], "sometimes"), ("reindex", "User", &[], "never")],
    },
    Small {
        what: "an integer that may be absent, and that no policy compares, is sometimes there",
        schema: "entity User;\nentity Doc;\naction \"read\" appliesTo { principal: [User], resource: [Doc], context: { incident?: Long } };\n",
        policies: "permit (principal, action == Action::\"read\", resource)\nwhen { context has incident };\n",
        answers: &[("read", "User", &[], "sometimes")],
    },
    Small {
        what: "an attribute one resource type requires and another may lack is sometimes absent",
        schema: "entity User;\nentity Doc = { owner: User };\nentity Folder = { owner?: User };\naction \"delete\" appliesTo { principal: [User], resource: [Doc, Folder] };\n",
        policies: "forbid (principal, action == Action::\"delete\", resource)\nunless { resource has owner };\n\npermit (principal, action == Action::\"delete\", resource);\n",
        answers: &[("delete", "User", &[], "sometimes")],
    },
    Small {
        what: "a boolean is never a string (the policies do not validate, and Cedar answers them all the same)",
        schema: "entity User = { contractor: Bool };\nentity Doc;\naction \"read\" appliesTo { principal: [User], resource: [Doc] };\n",
        policies: "permit (principal, action == Action::\"read\", resource)\nwhen { principal.contractor == \"yes\" };\n",
        answers: &[("read", "User", &[], "never")],
    },
    Small {
        what: "a policy on a group of actions reaches every action in the group",
        schema: "entity User;\nentity Doc;\naction \"write\";\naction \"edit\" in [\"write\"] appliesTo { principal: [User], resource: [Doc] };\naction \"append\" in [\"write\"] appliesTo { principal: [User], resource: [Doc] };\naction \"read\" appliesTo { principal: [User], resource: [Doc] };\n",
        policies: "permit (principal, action in Action::\"write\", resource);\n\nforbid (principal, action == Action::\"append\", resource);\n",
        answers: &[("edit", "User", &[], "always"), ("append", "User", &[], "never"), ("read", "User", &[], "never")],
    },
    Small {
        what: "a policy on a group of actions reaches every action in the group, in policies that do not validate (the run without the schema gives the actions' groups)",
        schema: "entity User;\nentity Doc;\naction \"write\";\naction \"edit\" in [\"write\"] appliesTo { principal: [User], resource: [Doc] };\naction \"append\" in [\"write\"] appliesTo { principal: [User], resource: [Doc] };\naction \"read\" appliesTo { principal: [User], resource: [Doc] };\n",
        policies: "permit (principal, action in Action::\"write\", resource);\n\nforbid (principal, action == Action::\"append\", resource);\n\nforbid (principal, action == Action::\"read\", resource)\nwhen { resource.title == \"draft\" };\n",
        answers: &[("edit", "User", &[], "always"), ("append", "User", &[], "never"), ("read", "User", &[], "never")],
    },
    Small {
        what: "a type of the same name in another namespace is another type",
        schema: "namespace A {\n  entity User;\n  entity Doc;\n  action \"read\" appliesTo { principal: [User], resource: [Doc] };\n}\n\nnamespace B {\n  entity User;\n}\n",
        policies: "permit (principal is B::User, action == A::Action::\"read\", resource);\n",
        answers: &[("read", "User", &[], "never")],
    },
];

/// Each small pair answers as the schema says a request can be: a combination no request makes
/// (a bot that owns a user's document, a customer in a role, a template in a folder, an archive not
/// in itself, …) is not counted, and one that some request makes is.
#[test]
fn what_the_schema_rules_out_is_not_counted() {
    let e = Engine::new(common::joined());
    for s in SMALL {
        let t = TempDir::new("cedar-in-small");
        std::fs::write(t.path().join("p.cedar"), s.policies).unwrap();
        std::fs::write(t.path().join("p.cedarschema"), s.schema).unwrap();
        for (action, ty, roles, want) in s.answers {
            let asker = Asker::Roles { ty: ty.to_string(), roles: roles.iter().map(|r| r.to_string()).collect() };
            assert_eq!(answer(e.allowed(t.path(), "p.cedar", action, &asker).unwrap()), *want, "{}: {action} asked by {asker:?}", s.what);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Every combination, held to the official CLI (DESIGN 6.5)

/// The actions of a schema, each with the types of principal it takes, by their names without the
/// namespace (as an asker names them).
fn schema_actions(schema: &Schema) -> Vec<(String, Vec<String>)> {
    schema.namespaces.iter().flat_map(|ns| ns.actions.iter()).map(|a| (a.name.clone(), a.applies_to.as_ref().map(|t| t.principal_types.iter().map(|n| n.id.clone()).collect()).unwrap_or_default())).collect()
}

/// Whether a schema puts an action in a group of actions (`action "a" in ["g"]`).
fn groups_actions(schema: &Schema) -> bool {
    schema.namespaces.iter().flat_map(|ns| ns.actions.iter()).any(|a| a.member_of.as_ref().is_some_and(|m| !m.is_empty()))
}

/// The action of this name and the groups it is in, through the schema (the Cedar the tests read
/// declares its actions in one namespace).
fn with_groups(schema: &Schema, action: &str) -> Vec<String> {
    let mut out = vec![action.to_string()];
    let mut i = 0;
    while i < out.len() {
        let parents: Vec<String> = schema.namespaces.iter().flat_map(|ns| ns.actions.iter()).filter(|a| a.name == out[i]).flat_map(|a| a.member_of.iter().flatten().map(|r| r.id.clone())).collect();
        for g in parents {
            if !out.contains(&g) {
                out.push(g);
            }
        }
        i += 1;
    }
    out
}

/// Whether a policy's scope takes the action of this name, in the groups the schema puts it in.
fn on(p: &Policy, schema: &Schema, action: &str) -> bool {
    let names = with_groups(schema, action);
    match &p.action {
        ActionScope::Any => true,
        ActionScope::Eq(u) => u.id == action,
        ActionScope::In(u) => names.contains(&u.id),
        ActionScope::InList(us) => us.iter().any(|u| names.contains(&u.id)),
    }
}

/// The ids of the entities the policies on an action ask the principal to be in, and of the
/// workflows they compare the principal with.
fn named(set: &PolicySet, schema: &Schema, action: &str) -> (Vec<String>, Vec<String>) {
    let (mut roles, mut workflows) = (BTreeSet::new(), BTreeSet::new());
    let principal = |e: &Expr| matches!(e.kind, ExprKind::Var(Var::Principal));
    for p in set.policies.iter().filter(|p| on(p, schema, action)) {
        match &p.principal {
            Scope::In(EntityOrSlot::Entity(u)) | Scope::IsIn(_, EntityOrSlot::Entity(u)) => {
                roles.insert(u.id.clone());
            }
            Scope::Eq(EntityOrSlot::Entity(u)) if u.ty.id == "Workflow" => {
                workflows.insert(u.id.clone());
            }
            _ => {}
        }
        for c in &p.conditions {
            cedar::walk(&c.body, &mut |e| match &e.kind {
                ExprKind::Binary { op: BinOp::In, left, right } if principal(left) => {
                    if let ExprKind::Entity(u) = &right.kind {
                        roles.insert(u.id.clone());
                    }
                }
                ExprKind::Is { expr, in_expr: Some(x), .. } if principal(expr) => {
                    if let ExprKind::Entity(u) = &x.kind {
                        roles.insert(u.id.clone());
                    }
                }
                ExprKind::Binary { op: BinOp::Eq | BinOp::NotEq, left, right } => {
                    for (a, b) in [(left, right), (right, left)] {
                        if let ExprKind::Entity(u) = &b.kind
                            && principal(a)
                            && u.ty.id == "Workflow"
                        {
                            workflows.insert(u.id.clone());
                        }
                    }
                }
                _ => {}
            });
        }
    }
    (roles.into_iter().collect(), workflows.into_iter().collect())
}

/// Who asks about an action: each type of principal it takes, with every set of the roles its
/// policies name; and for the type `Workflow`, each workflow they name and one they do not.
fn askers(set: &PolicySet, schema: &Schema, action: &str, principals: &[String]) -> Vec<Asker> {
    let (roles, workflows) = named(set, schema, action);
    assert!(roles.len() <= 10, "{action}: {} roles, {} sets of them", roles.len(), 1u64 << roles.len());
    let mut out = Vec::new();
    for ty in principals {
        for mask in 0..(1u32 << roles.len()) {
            let held: Vec<String> = roles.iter().enumerate().filter(|(i, _)| mask & (1 << i) != 0).map(|(_, r)| r.clone()).collect();
            out.push(Asker::Roles { ty: ty.clone(), roles: held });
        }
        if ty == "Workflow" {
            out.extend(workflows.iter().map(|w| Asker::Workflow(w.clone())));
            let mut other = "another_workflow".to_string();
            while workflows.contains(&other) {
                other.push('_');
            }
            out.push(Asker::Workflow(other));
        }
    }
    out
}

/// What each policy on an action comes to in a combination, by its id: true, false, or None for an
/// error.
type Statuses = Vec<(String, Option<bool>)>;

/// What sekisho answers for a pair: every question of the actions asked about, asked of every
/// asker.
struct Asked {
    /// The actions asked about.
    actions: Vec<String>,
    questions: usize,
    combinations: usize,
    allowed: usize,
    /// Each test, with what each policy on its action comes to (true, false, or None for an error).
    tests: Vec<(Json, Statuses)>,
    /// The questions undecided: the action, who asked, and why.
    undecided: Vec<(String, Asker, Undecided)>,
    /// The combinations sekisho counts that no request makes.
    unmade: Vec<String>,
}

/// Every question of every action of the pair (or of `only` those), asked of every asker.
fn ask_all(root: &Path, file: &str, schema: &Schema, set: &PolicySet, only: Option<&[String]>) -> Result<Asked, String> {
    let mut a = Asked { actions: Vec::new(), questions: 0, combinations: 0, allowed: 0, tests: Vec::new(), undecided: Vec::new(), unmade: Vec::new() };
    for (action, principals) in schema_actions(schema) {
        if only.is_some_and(|o| !o.contains(&action)) {
            continue;
        }
        a.actions.push(action.clone());
        for asker in askers(set, schema, &action, &principals) {
            a.questions += 1;
            match cedar_in::cases(root, file, &action, &asker) {
                Err(said) => return Err(format!("{file}, {action}, {asker:?}: {said:?}")),
                Ok(Err(u)) => a.undecided.push((action.clone(), asker, u)),
                Ok(Ok(cases)) => {
                    for c in cases {
                        a.combinations += 1;
                        a.allowed += usize::from(c.allow);
                        match c.tests {
                            Ok(ts) => a.tests.extend(ts.into_iter().map(|t| (t, c.policies.clone()))),
                            Err(why) => a.unmade.push(format!("{action}, {asker:?}: {}: {}", c.shown.en, why.en)),
                        }
                    }
                }
            }
        }
    }
    Ok(a)
}

/// A test with another answer: the same request and entities.
fn answered(t: &Json, allow: bool, reason: Option<&str>, errors: usize) -> Json {
    let Json::Obj(pairs) = t else { unreachable!("a test is an object") };
    Json::Obj(
        pairs
            .iter()
            .map(|(k, v)| {
                let v = match k.as_str() {
                    "decision" => Json::str(if allow { "allow" } else { "deny" }),
                    "reason" => Json::arr(reason.map(Json::str)),
                    "num_errors" => Json::Int(errors as i128),
                    _ => v.clone(),
                };
                (k.clone(), v)
            })
            .collect(),
    )
}

/// The files of a pair the CLI is run on, in `dir`.
struct Pair<'a> {
    dir: &'a Path,
    policies: &'a str,
    schema: &'a str,
}

impl Pair<'_> {
    /// `run-tests` on the policies `p`: with the schema, or without it.
    fn run(&self, cli: &Cli, p: &str, tests: &str, n: usize, with_schema: bool) -> Result<(), String> {
        if with_schema { cli.run_tests(self.dir, p, self.schema, tests, n, false) } else { cli.run_tests_without_schema(self.dir, p, tests, n) }
    }
}

/// The entities of the actions, with the groups each is in as its parents: a test run without the
/// schema gives them (with a schema, Cedar takes the actions' groups from it).
fn action_entities(schema: &Schema) -> Vec<Json> {
    let mut out = Vec::new();
    for ns in &schema.namespaces {
        let ty = match &ns.name {
            Some(n) => format!("{n}::Action"),
            None => "Action".to_string(),
        };
        for a in &ns.actions {
            let parents = a.member_of.iter().flatten().map(|r| {
                let pty = r.ty.as_ref().filter(|n| !n.path.is_empty()).map(Name::to_string).unwrap_or_else(|| ty.clone());
                Json::obj([("type", Json::str(pty)), ("id", Json::str(r.id.clone()))])
            });
            out.push(Json::obj([("uid", Json::obj([("type", Json::str(ty.clone())), ("id", Json::str(a.name.clone()))])), ("attrs", Json::Obj(Vec::new())), ("parents", Json::arr(parents))]));
        }
    }
    out
}

/// A test with more entities.
fn with_entities(t: &Json, more: &[Json]) -> Json {
    let Json::Obj(pairs) = t else { unreachable!("a test is an object") };
    Json::Obj(
        pairs
            .iter()
            .map(|(k, v)| {
                let v = match (k.as_str(), v) {
                    ("entities", Json::Arr(es)) => Json::Arr(es.iter().chain(more).cloned().collect()),
                    _ => v.clone(),
                };
                (k.clone(), v)
            })
            .collect(),
    )
}

/// The CLI on every test sekisho made of a pair: `run-tests` with the pair's policies and its
/// schema (when the policies validate against it, strict; `run-tests` given a schema validates the
/// policies for every test, and fails every test when they do not, so then without it); and each
/// policy on the actions asked about alone, made a permit, which holds where sekisho says it holds
/// and errs where it says it errs. With `run-tests`, the decision and the policies that decide it
/// are the CLI's, test by test. The run of the whole set holds the requests and the entities to the
/// schema; each policy alone is run without it, more than twice as fast, when no action is in a
/// group (Cedar takes the groups from the schema). A test run without the schema of a pair that
/// puts actions in groups gives the actions as entities. Whether the policies validate.
fn held(cli: &Cli, pair: &Pair, set: &PolicySet, schema: &Schema, asked: &Asked) -> Result<bool, String> {
    let validated = cli.validate(pair.dir, pair.policies, pair.schema, false).is_ok();
    let n = asked.tests.len();
    let groups = groups_actions(schema);
    let actions = if groups { action_entities(schema) } else { Vec::new() };
    let fit = |t: &Json, with_schema: bool| if with_schema { t.clone() } else { with_entities(t, &actions) };
    std::fs::write(pair.dir.join("all.tests.json"), Json::arr(asked.tests.iter().map(|(t, _)| fit(t, validated))).compact()).unwrap();
    pair.run(cli, pair.policies, "all.tests.json", n, validated)?;
    let alone_with_schema = validated && groups;
    for (i, p) in set.policies.iter().enumerate().filter(|(_, p)| asked.actions.iter().any(|a| on(p, schema, a))) {
        let mut alone = p.clone();
        alone.effect = Effect::Permit;
        // alone, a policy with no `@id` would be `policy0`: it keeps the id it has in the set
        if !alone.annotations.iter().any(|a| a.key == "id") {
            alone.annotations.insert(0, cedar::Annotation { key: "id".into(), value: Some(p.id.clone()), line: 0, col: 0 });
        }
        let text = cedar::write_policies(&PolicySet { policies: vec![alone] }).map_err(|e| format!("{}: ritsu-base does not write it: {e:?}", p.id))?;
        let tests = asked.tests.iter().map(|(t, on)| {
            // a policy not on the action of the test holds in none of it
            let (allow, errors) = match on.iter().find(|(id, _)| *id == p.id).map(|(_, v)| *v) {
                Some(Some(true)) => (true, 0),
                Some(None) => (false, 1),
                _ => (false, 0),
            };
            answered(&fit(t, alone_with_schema), allow, allow.then_some(p.id.as_str()), errors)
        });
        let (pf, tf) = (format!("alone-{i}.cedar"), format!("alone-{i}.tests.json"));
        std::fs::write(pair.dir.join(&pf), text).unwrap();
        std::fs::write(pair.dir.join(&tf), Json::arr(tests).compact()).unwrap();
        pair.run(cli, &pf, &tf, n, alone_with_schema).map_err(|e| format!("`{}` alone, made a permit: {e}", p.id))?;
    }
    Ok(validated)
}

/// The word sekisho gives a part of an expression no condition it counts is made of, when it is
/// one: a pattern, arithmetic, a method, a function, a set, a record, a slot.
fn never_counted(e: &Expr) -> Option<String> {
    Some(match &e.kind {
        ExprKind::Like { .. } => "like".into(),
        ExprKind::Binary { op: op @ (BinOp::Add | BinOp::Sub | BinOp::Mul), .. } => op.as_str().into(),
        ExprKind::Neg(x) if !matches!(x.kind, ExprKind::Long(_)) => "-".into(),
        ExprKind::Method { name, .. } => format!(".{name}()"),
        ExprKind::Call { func, .. } => format!("{func}()"),
        ExprKind::Set(_) => "set".into(),
        ExprKind::Record(_) => "record".into(),
        ExprKind::Slot(_) => "slot".into(),
        _ => return None,
    })
}

/// The first part of a policy's conditions, in the order Cedar reads them, that no condition
/// sekisho counts is made of: what it is, its line and its column.
fn first_never_counted(p: &Policy) -> Option<(String, usize, usize)> {
    let mut found = None;
    for c in &p.conditions {
        cedar::walk(&c.body, &mut |e| {
            if found.is_none() {
                found = never_counted(e).map(|w| (w, e.line, e.col));
            }
        });
    }
    found
}

/// What sekisho says of each question it does not decide, held to the policies: of an action whose
/// policies have a part no condition it counts is made of (in the material, a pattern or
/// arithmetic), every question is undecided, and the reason names the first policy on the action
/// with such a part, and the first such part, at its line and column; of any other action, no
/// question is undecided. The actions undecided.
fn undecided_right(set: &PolicySet, schema: &Schema, asked: &Asked) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for (action, principals) in schema_actions(schema).into_iter().filter(|(a, _)| asked.actions.contains(a)) {
        let first = set.policies.iter().filter(|p| on(p, schema, &action)).find_map(|p| first_never_counted(p).map(|(what, line, col)| Undecided::Outside { policy: p.id.clone(), what, line, col }));
        let of_action: Vec<&(String, Asker, Undecided)> = asked.undecided.iter().filter(|(a, _, _)| *a == action).collect();
        match &first {
            None if !of_action.is_empty() => return Err(format!("{action}: no policy on it has a part sekisho does not count, and {} of its questions are undecided: {}", of_action.len(), of_action[0].2.text().en)),
            None => {}
            Some(want) => {
                let questions = askers(set, schema, &action, &principals).len();
                if of_action.len() != questions {
                    return Err(format!("{action}: {} of its {questions} questions are undecided, where every one reads {}", of_action.len(), want.text().en));
                }
                if let Some((_, who, got)) = of_action.iter().find(|(_, _, u)| u != want) {
                    return Err(format!("{action}, {who:?}: undecided as {}, where the first part sekisho does not count is {}", got.text().en, want.text().en));
                }
                out.push(action);
            }
        }
    }
    Ok(out)
}

/// `n` with the noun, one or many.
fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// What a pair comes to, held to the CLI, for the line a test prints.
fn line_of(asked: &Asked, set: &PolicySet, schema: &Schema, validated: bool) -> String {
    if asked.questions == 0 {
        return "no action, so no question".to_string();
    }
    format!(
        "{} of {} ({} allowed) over {} {} run-tests{}, and so does each of the {} alone; {}",
        count(asked.tests.len(), "test", "tests"),
        count(asked.combinations, "combination", "combinations"),
        asked.allowed,
        count(asked.questions, "question", "questions"),
        if asked.tests.len() == 1 { "passes" } else { "pass" },
        if validated { "" } else { " (without the schema: the policies do not validate)" },
        count(set.policies.iter().filter(|p| asked.actions.iter().any(|a| on(p, schema, a))).count(), "policy", "policies"),
        match asked.undecided.len() {
            0 => "every question is decided".to_string(),
            n => format!("{} undecided, each by the first part sekisho does not count of the first policy on its action that has one", count(n, "question is", "questions are")),
        }
    )
}

/// What one pair came to, held to the CLI: the line the test prints, the actions undecided, whether
/// the policies validate, and how many tests the CLI ran.
struct Came {
    line: String,
    undecided: Vec<String>,
    validated: bool,
    tests: usize,
}

/// One pair of Cedar written by hand, read by sekisho at `root`/`file`, held to the CLI in `dir`.
fn hold_pair(cli: &Cli, dir: &Path, root: &Path, file: &str, policies_text: &str, schema_text: &str, only: Option<&[String]>) -> Result<Came, String> {
    let set = cedar::parse_policies(policies_text).map_err(|e| format!("ritsu-base does not read the policies: {e:?}"))?;
    let schema = cedar::parse_schema(schema_text).map_err(|e| format!("ritsu-base does not read the schema: {e:?}"))?;
    let asked = ask_all(root, file, &schema, &set, only)?;
    if !asked.unmade.is_empty() {
        return Err(format!("{} combinations sekisho counts are no request:\n  {}", asked.unmade.len(), asked.unmade.iter().take(8).cloned().collect::<Vec<_>>().join("\n  ")));
    }
    let undecided = undecided_right(&set, &schema, &asked)?;
    std::fs::write(dir.join("p.cedar"), policies_text).unwrap();
    std::fs::write(dir.join("p.cedarschema"), schema_text).unwrap();
    let validated = held(cli, &Pair { dir, policies: "p.cedar", schema: "p.cedarschema" }, &set, &schema, &asked)?;
    Ok(Came { line: line_of(&asked, &set, &schema, validated), undecided, validated, tests: asked.tests.len() })
}

fn text_of(f: &Path) -> String {
    std::fs::read_to_string(f).unwrap_or_else(|e| panic!("{}: {e}", f.display()))
}

/// Every combination sekisho counts for the Cedar written by hand, in English and in Japanese,
/// held to the CLI: refunds decides every question; of the library, the questions of printing are
/// undecided, by the `like` of the forbid of drafts, and the others are decided.
#[test]
fn every_combination_of_the_cedar_written_by_hand_is_held_to_the_cli() {
    let Some(cli) = Cli::find() else { return };
    for (stem, want) in [("refunds", vec![]), ("refunds.ja", vec![]), ("outside", vec!["print"]), ("outside.ja", vec!["print"]), ("documents", vec![]), ("documents.ja", vec![])] {
        let t = TempDir::new("cedar-in");
        let file = format!("{DIR}/{stem}.cedar");
        let (pt, st) = (text_of(Path::new(&file)), text_of(Path::new(&format!("{DIR}/{stem}.cedarschema"))));
        let came = hold_pair(&cli, t.path(), &here(), &file, &pt, &st, None).unwrap_or_else(|e| panic!("{stem}: {e}"));
        println!("{file}: {}", came.line);
        assert_eq!(came.undecided, want, "{stem}");
        assert!(came.validated, "{stem}: the material validates");
    }
    for s in SMALL {
        let t = TempDir::new("cedar-in-small");
        std::fs::write(t.path().join("p.cedar"), s.policies).unwrap();
        std::fs::write(t.path().join("p.cedarschema"), s.schema).unwrap();
        let came = hold_pair(&cli, t.path(), t.path(), "p.cedar", s.policies, s.schema, None).unwrap_or_else(|e| panic!("{}: {e}", s.what));
        println!("{}: {}", s.what, came.line);
        assert!(came.undecided.is_empty(), "{}", s.what);
    }
}

/// The command run as a function, every language joined: its exit code, what it printed, and
/// what it printed on stderr.
fn sekisho(args: &[&str]) -> (u8, String, String) {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = sekisho::run::run(&args, common::joined(), &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

/// The Cedar gen writes for a `.gate`, read as Cedar written by hand and held to the CLI: the line
/// the test prints; None when the gate's check finds an error (gen then writes nothing).
fn generated_held(cli: &Cli, gate: &str) -> Result<Option<String>, String> {
    let t = TempDir::new("cedar-in-gen");
    let out = t.path().join("out");
    let (code, stdout, stderr) = sekisho(&["gen", gate, "--target", "cedar", "--out", out.to_str().unwrap(), "--lang", "en"]);
    match code {
        0 => {}
        1 => return Ok(None),
        c => return Err(format!("{gate}: gen exits {c}\n{stdout}{stderr}")),
    }
    let dir = out.join("cedar");
    let stem = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()?.file_name().to_str()?.strip_suffix(".cedarschema").map(str::to_string)).next().ok_or_else(|| format!("{gate}: gen writes no schema"))?;
    let (pf, sf) = (format!("{stem}.cedar"), format!("{stem}.cedarschema"));
    let (pt, st) = (text_of(&dir.join(&pf)), text_of(&dir.join(&sf)));
    let came = hold_pair(cli, t.path(), &dir, &pf, &pt, &st, None).map_err(|e| format!("{gate}: {e}"))?;
    if !came.undecided.is_empty() {
        return Err(format!("{gate}: the questions of {} are undecided", came.undecided.join(", ")));
    }
    if !came.validated {
        return Err(format!("{gate}: the generated Cedar does not validate"));
    }
    Ok(Some(format!("{gate}: {}", came.line)))
}

/// The Cedar sekisho generates of every `.gate` of the examples and the tests that passes its
/// check, read as Cedar written by hand: its policies are in the part sekisho counts, so every
/// question is decided, and every combination is held to the CLI.
#[test]
fn the_cedar_sekisho_generates_read_as_written_by_hand_is_held_to_the_cli() {
    let Some(cli) = Cli::find() else { return };
    let gates = gates();
    let said = each_on_threads(&gates, |_, g| generated_held(&cli, g));
    let (mut held, mut wrong) = (Vec::new(), Vec::new());
    for s in said {
        match s {
            Ok(Some(line)) => held.push(line),
            Ok(None) => {}
            Err(e) => wrong.push(e),
        }
    }
    for line in &held {
        println!("{line}");
    }
    println!("{} of the {} .gate files pass their check; the Cedar gen writes of each, read as Cedar written by hand, decides every question, and the CLI answers every combination as sekisho does", held.len(), gates.len());
    assert!(wrong.is_empty(), "{}", wrong.join("\n\n"));
    assert!(held.iter().any(|l| l.starts_with("examples/refunds/refunds.gate: ")), "the example is generated and held");
}

/// A pair's Cedar with what Cedar does not evaluate put aside (the namespaces' names, the ids of the
/// policies, every annotation): two versions with the same are changed alike.
fn same_but_for_names(set: &PolicySet, schema: &Schema) -> String {
    let mut set = set.clone();
    for (i, p) in set.policies.iter_mut().enumerate() {
        p.annotations.clear();
        p.id = format!("p{i}");
    }
    let mut schema = schema.clone();
    let names: Vec<String> = schema.namespaces.iter().filter_map(|n| n.name.as_ref().map(Name::to_string)).collect();
    let attrs = |t: &mut cedar::Type| {
        if let cedar::Type::Record(r) = t {
            r.attrs.iter_mut().for_each(|a| a.annotations.clear());
        }
    };
    for n in &mut schema.namespaces {
        n.annotations.clear();
        for e in &mut n.entity_types {
            e.annotations.clear();
            if let cedar::EntityKind::Standard { shape, .. } = &mut e.kind {
                attrs(shape);
            }
        }
        for a in &mut n.actions {
            a.annotations.clear();
            if let Some(ap) = &mut a.applies_to {
                attrs(&mut ap.context);
            }
        }
    }
    let mut key = format!("{}\n{}", cedar::policies_to_json(&set).compact(), cedar::schema_to_json(&schema).compact());
    for (i, n) in names.iter().enumerate() {
        key = key.replace(&format!("\"{n}\""), &format!("\"N{i}\"")).replace(&format!("{n}::"), &format!("N{i}::"));
    }
    key
}

/// The actions a change of a policy set touches: those the policies it changes are on, before the
/// change and after it. The questions of the others are the unchanged material's.
fn touched(before: &PolicySet, after: &PolicySet, schema: &Schema) -> Vec<String> {
    let changed: Vec<&Policy> = before.policies.iter().filter(|p| !after.policies.contains(p)).chain(after.policies.iter().filter(|p| !before.policies.contains(p))).collect();
    schema_actions(schema).into_iter().map(|(a, _)| a).filter(|a| changed.iter().any(|p| on(p, schema, a))).collect()
}

/// What a change comes to, beside what it is held to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Outcome {
    /// The CLI answers a test of the unchanged material otherwise.
    Answers,
    /// `run-tests` passes every test of the unchanged material on the changed policies.
    Alike,
    /// Questions that were undecided are decided (and held to the CLI).
    Decides,
    /// Every question of the actions it touches stays undecided: no combination to hold.
    Undecided,
}

/// One change of the material, written as ritsu-base writes Cedar, read by sekisho and held to the
/// CLI on the actions it touches: whether its policies validate, and what it comes to.
fn change_held(cli: &Cli, schema_text: &str, before: &PolicySet, after: &PolicySet, original: &Asked) -> Result<(bool, Outcome), String> {
    let text = cedar::write_policies(after).map_err(|e| format!("ritsu-base does not write it: {e:?}"))?;
    let schema = cedar::parse_schema(schema_text).map_err(|e| format!("{e:?}"))?;
    let only = touched(before, after, &schema);
    let t = TempDir::new("cedar-in-change");
    std::fs::write(t.path().join("m.cedar"), &text).unwrap();
    std::fs::write(t.path().join("m.cedarschema"), schema_text).unwrap();
    let came = hold_pair(cli, t.path(), t.path(), "m.cedar", &text, schema_text, Some(&only))?;
    // the tests of the unchanged material on the actions touched, run on the changed policies
    let tests: Vec<Json> = original
        .tests
        .iter()
        .map(|(t, _)| t.clone())
        .filter(|t| {
            let action = t.get("request").and_then(|r| r.get("action")).and_then(Json::as_str).unwrap_or("");
            only.iter().any(|a| action.ends_with(&format!("Action::\"{a}\"")))
        })
        .collect();
    if tests.is_empty() {
        return Ok((came.validated, if came.tests == 0 { Outcome::Undecided } else { Outcome::Decides }));
    }
    let n = tests.len();
    std::fs::write(t.path().join("original.tests.json"), Json::Arr(tests).compact()).unwrap();
    let alike = Pair { dir: t.path(), policies: "p.cedar", schema: "p.cedarschema" }.run(cli, "p.cedar", "original.tests.json", n, came.validated);
    Ok((came.validated, if alike.is_ok() { Outcome::Alike } else { Outcome::Answers }))
}

/// Every change of the Cedar written by hand at one place (a number moved by one, a string or a
/// role put in another's place, an operator put in another's place, a `has` or a `!` taken away, a
/// condition made the other kind or taken away, a forbid made a permit, a scope changed, a policy
/// taken away; `tests/common/cedar_cli.rs`): sekisho reads the changed policies, and every
/// combination it counts of the actions the change touches is held to the CLI, as for the material
/// itself. The changes whose policies do not validate are run without the schema.
#[test]
fn every_change_of_the_cedar_written_by_hand_is_held_to_the_cli() {
    let Some(cli) = Cli::find() else { return };
    let mut seen: Vec<(String, &str)> = Vec::new();
    let (mut all, mut wrong) = (0, Vec::new());
    for stem in ["refunds", "refunds.ja", "outside", "outside.ja", "documents", "documents.ja"] {
        let file = format!("{DIR}/{stem}.cedar");
        let schema_text = text_of(Path::new(&format!("{DIR}/{stem}.cedarschema")));
        let set = cedar::parse_policies(&text_of(Path::new(&file))).unwrap();
        let schema = cedar::parse_schema(&schema_text).unwrap();
        assert!(!groups_actions(&schema), "{stem}: no action is in a group");
        let key = same_but_for_names(&set, &schema);
        if let Some((_, first)) = seen.iter().find(|(k, _)| *k == key) {
            println!("{file}: the same Cedar as {DIR}/{first}.cedar, but for the names");
            continue;
        }
        seen.push((key, stem));
        let original = ask_all(&here(), &file, &schema, &set, None).unwrap();
        let changes = policy_changes(&set);
        let said = each_on_threads(&changes, |_, c| change_held(&cli, &schema_text, &set, c.policies.as_ref().expect("a change of the policies"), &original));
        let mut validated = 0;
        let mut came: Vec<(Outcome, &str)> = Vec::new();
        for (c, s) in changes.iter().zip(said) {
            match s {
                Ok((v, k)) => {
                    validated += usize::from(v);
                    came.push((k, &c.what));
                }
                Err(e) => wrong.push(format!("{stem}: {}: {e}", c.what)),
            }
        }
        let of = |k: Outcome| came.iter().filter(|(x, _)| *x == k).count();
        all += changes.len();
        println!(
            "{file}: {} changes, each held to the CLI as the material is ({validated} validate; {} do not, and are run without the schema); {} change the answer to a test of the material, {} make undecided questions decided, {} leave every question of the actions they touch undecided, and {} pass every test of the material:",
            changes.len(),
            changes.len() - validated,
            of(Outcome::Answers),
            of(Outcome::Decides),
            of(Outcome::Undecided),
            of(Outcome::Alike)
        );
        for (_, what) in came.iter().filter(|(k, _)| *k == Outcome::Alike) {
            println!("  {what}");
        }
    }
    println!("{all} changes of the Cedar written by hand, every combination sekisho counts of each answered as the CLI answers it");
    assert!(wrong.is_empty(), "{}", wrong.join("\n\n"));
}
