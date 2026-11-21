//! The checks of every combination (PLAN A3), with rulec, koyomi, chobo and dandori joined as
//! `ritsu sekisho` joins them: the example's two versions pass and come to the same counts, the
//! table of `refund_order` merges to its five rows that allow, relations are counted by which terms
//! are one entity, the budget stops a walk (E307), an answer that rests on a value no language
//! vouches for is undecided (W303), an enum and a bool given to a rule are held to their values,
//! an enum given inside the range a rule holds its input to is counted exactly,
//! how far a workflow and a role are allowed an action (the port `Gates::allowed`), a condition
//! on another action's answer (`allowed`) counted exactly, and values that read the same data
//! counted together: only the combinations of them that happen (`tests/walk/together/`), with an
//! attribute given as two inputs of a rule cut between the two values rulec gave it. The
//! mutants of the example are `tests/mutants/`, run by `tests/mutants.rs`.
//!
//! The goldens are `tests/walk/golden/`; `SEKISHO_BLESS=1` (or `RITSU_BLESS=1`) writes them again.

mod common;

use ritsu_base::text::Lang;
use ritsu_ports::{DaySet, Found, RuleError, Said, Values};
use sekisho::check::Options;
use sekisho::checks::{self, Checked};
use sekisho::diag::Diag;
use sekisho::suite::Suite;
use std::path::Path;
use std::rc::Rc;

const EN: &str = "examples/refunds/refunds.gate";
const JA: &str = "examples/refunds/refunds.ja.gate";

/// rulec, koyomi, chobo and dandori joined as `ritsu sekisho` joins them.
fn joined() -> Suite {
    common::joined()
}

fn check(path: &str, budget: u64) -> (Vec<Diag>, Checked) {
    check_with(path, &joined(), budget)
}

fn check_with(path: &str, suite: &Suite, budget: u64) -> (Vec<Diag>, Checked) {
    let o = sekisho::check::check_file(path, suite, &Options { budget, ..Options::default() }).unwrap();
    let said: String = o.diags.iter().map(|d| d.render(Lang::En)).collect();
    (o.diags, o.walked.unwrap_or_else(|| panic!("{path}: the names do not hold:\n{said}")))
}

fn rendered(diags: &[Diag]) -> String {
    let mut out = String::new();
    for lang in [Lang::En, Lang::Ja] {
        for d in diags {
            out.push_str(&d.render(lang));
        }
        out.push('\n');
    }
    out
}

#[test]
fn the_example_passes_in_both_versions_with_the_same_counts() {
    let (de, en) = check(EN, checks::BUDGET as u64);
    let (dj, ja) = check(JA, checks::BUDGET as u64);
    assert!(de.is_empty(), "{}", rendered(&de));
    assert!(dj.is_empty(), "{}", rendered(&dj));
    let total: u128 = en.report.actions.iter().map(|a| a.combinations).sum();
    assert_eq!(total, 1_078);
    // rulec says exactly what the band comes to over each cell of amount, and koyomi every day:
    // nothing is counted that the languages do not vouch for
    for c in [&en, &ja] {
        assert!(c.report.spaces.iter().flatten().all(|s| !s.inexact()));
    }
    // the twins: every count the same, action by action and policy by policy
    assert_eq!(en.report.actions, ja.report.actions);
    assert_eq!(en.report.policies, ja.report.policies);
    assert_eq!(en.report.expects, ja.report.expects);
    assert_eq!(en.report.separations, ja.report.separations);
    assert_eq!(en.report.reach, ja.report.reach);
    ritsu_testkit::golden("tests/walk/golden/refunds.txt", &(checks::lines(&en.gate, &en.report).join("\n") + "\n"));
    ritsu_testkit::golden("tests/walk/golden/refunds.ja.txt", &(checks::lines(&ja.gate, &ja.report).join("\n") + "\n"));
}

#[test]
fn the_table_of_refund_order_merges_to_five_rows_that_allow() {
    for (path, golden) in [(EN, "tests/walk/golden/refund_order.table.md"), (JA, "tests/walk/golden/refund_order.table.ja.md")] {
        let (_, c) = check(path, checks::BUDGET as u64);
        let s = c.report.spaces[1].as_ref().unwrap();
        let t = sekisho::table::table(&c.gate, s);
        assert_eq!(t.allowing(), 5, "{path}");
        assert_eq!(t.rows.len(), 26, "{path}");
        let lang = if path == JA { Lang::Ja } else { Lang::En };
        let md = t.markdown(&c.gate, lang);
        // the rule's answer by the name the `.rule` writes, as the `.gate` beside the table reads it
        let (within, over) = if path == JA { ("上限まで", "上限超え") } else { ("within_limit", "over_limit") };
        assert!(md.contains(&format!("| {within} |")) && md.contains(&format!("| {over} |")), "{md}");
        ritsu_testkit::golden(golden, &md);
    }
}

/// A condition on another action's answer (`allowed`, DESIGN 2.11): the examples of asking for an
/// approval only of what one cannot approve, and of archiving what can be archived rather than
/// deleting it, pass in both versions with the same counts. The walk of an action that asks holds
/// what the action asked reads, and the asked action's band and the asking action's own are one
/// computation, so nothing is counted that the languages do not vouch for. The answer read is a
/// column of the table, and what the asked action computes for it is not.
#[test]
fn an_answer_of_another_action_is_read_exactly() {
    for (en_path, ja_path, name) in [("examples/approvals/approvals.gate", "examples/approvals/approvals.ja.gate", "approvals"), ("examples/documents/documents.gate", "examples/documents/documents.ja.gate", "documents")] {
        let (de, en) = check(en_path, checks::BUDGET as u64);
        let (dj, ja) = check(ja_path, checks::BUDGET as u64);
        assert!(de.is_empty(), "{}", rendered(&de));
        assert!(dj.is_empty(), "{}", rendered(&dj));
        for c in [&en, &ja] {
            assert!(c.report.spaces.iter().flatten().all(|s| !s.inexact()), "{name}: a value counted though no language vouches for it");
        }
        assert_eq!(en.report.actions, ja.report.actions, "{name}");
        assert_eq!(en.report.policies, ja.report.policies, "{name}");
        assert_eq!(en.report.expects, ja.report.expects, "{name}");
        assert_eq!(en.report.reach, ja.report.reach, "{name}");
        ritsu_testkit::golden(format!("tests/walk/golden/{name}.txt"), &(checks::lines(&en.gate, &en.report).join("\n") + "\n"));
        ritsu_testkit::golden(format!("tests/walk/golden/{name}.ja.txt"), &(checks::lines(&ja.gate, &ja.report).join("\n") + "\n"));
    }
    for (path, lang, golden) in [("examples/approvals/approvals.gate", Lang::En, "tests/walk/golden/request_approval.table.md"), ("examples/approvals/approvals.ja.gate", Lang::Ja, "tests/walk/golden/request_approval.table.ja.md")] {
        let (_, c) = check(path, checks::BUDGET as u64);
        let ai = c.gate.actions.iter().position(|a| a.named.alias == "request_approval").unwrap();
        let t = sekisho::table::table(&c.gate, c.report.spaces[ai].as_ref().unwrap());
        let cols: Vec<String> = t.columns.iter().map(|x| x.get(lang).to_string()).collect();
        let (answer, band) = if lang == Lang::Ja { ("allowed 承認する", "区分") } else { ("allowed approve", "band") };
        assert!(cols.iter().any(|x| x == answer), "{cols:?}");
        // the band of request_approval; approve's is approve's table's
        assert_eq!(cols.iter().filter(|x| *x == band).count(), 1, "{cols:?}");
        ritsu_testkit::golden(golden, &t.markdown(&c.gate, lang));
    }
}

/// `tests/gen/asked.gate` asks in every way a condition can: an input of the same name, an
/// attribute, a constant of each kind, today, an input given nothing, an action that asks one
/// that asks another, an answer only an expectation reads, and two resource types.
#[test]
fn every_way_of_asking_is_counted() {
    let (d, c) = check("tests/gen/asked.gate", checks::BUDGET as u64);
    assert!(d.is_empty(), "{}", rendered(&d));
    assert!(c.report.spaces.iter().flatten().all(|s| !s.inexact()));
    // an answer a policy reads is Cedar's; the one only an expectation reads is not
    let escalate = c.gate.actions.iter().find(|a| a.named.alias == "escalate").unwrap();
    let read: Vec<(String, bool)> = escalate.allowed.iter().map(|x| (c.gate.actions[x.action].named.alias.clone(), x.policy)).collect();
    assert_eq!(read, vec![("review".to_string(), true), ("refund".to_string(), false)]);
    ritsu_testkit::golden("tests/walk/golden/asked.txt", &(checks::lines(&c.gate, &c.report).join("\n") + "\n"));
}

#[test]
fn the_budget_stops_a_walk_past_it() {
    let (diags, c) = check(EN, 1_000);
    let codes: Vec<&str> = diags.iter().map(|d| d.code).collect();
    assert_eq!(codes, vec!["E307"], "{}", rendered(&diags));
    assert!(c.report.spaces[1].is_none());
    assert_eq!(c.report.actions[0].combinations, 18);
    ritsu_testkit::golden("tests/walk/golden/E307_budget.txt", &rendered(&diags));
}

/// A rulec that cannot say what refund_limit's output comes to, and whose evaluator gives no
/// answer: what the walk counts of the band rests on nothing, and an answer from it is undecided.
struct Silent(rulec::ports::Engine);

impl ritsu_ports::Rules for Silent {
    fn facts(&self, rule: &Path) -> Result<ritsu_ports::RuleFacts, Vec<Said>> {
        self.0.facts(rule)
    }
    fn preconditions_hold(&self, rule: &Path, ranges: &[(String, Option<i128>, Option<i128>)], min_len: Option<i128>, max_len: Option<i128>) -> Result<Vec<(ritsu_ports::Precondition, ritsu_ports::Answer<Values>)>, Vec<Said>> {
        self.0.preconditions_hold(rule, ranges, min_len, max_len)
    }
    fn output_values(&self, rule: &Path, output: &str) -> Result<Found<ritsu_ports::OutputValues>, Vec<Said>> {
        self.0.output_values(rule, output)
    }
    fn checked_over(&self, rule: &Path, input: &str, days: &DaySet) -> Result<ritsu_ports::Answer<ritsu_base::text::Text>, Vec<Said>> {
        self.0.checked_over(rule, input, days)
    }
    fn eval(&self, _rule: &Path, _inputs: &Values) -> Result<Values, RuleError> {
        Err(RuleError::Contradiction(ritsu_base::text::Text::same("silent")))
    }
    fn doc(&self, rule: &Path, shown: &str, html: bool, lang: Lang) -> Result<String, Vec<Said>> {
        self.0.doc(rule, shown, html, lang)
    }
}

#[test]
fn an_answer_no_language_vouches_for_is_undecided() {
    // the clerks' permit is the only one that reads the band: whether it allows anything rests on it
    let mut suite = joined();
    suite.rules = Some(Rc::new(Silent(rulec::ports::Engine::default())));
    let (diags, _) = check_with("tests/walk/undecided/W303_undecided.gate", &suite, checks::BUDGET as u64);
    assert!(diags.iter().any(|d| d.code == "W303"), "{}", rendered(&diags));
    ritsu_testkit::golden("tests/walk/golden/W303_undecided.txt", &rendered(&diags));
}

/// A rule given an enum attribute its policies read too, an enum and a bool as constants, and an
/// enum attribute with fewer values than the rule's enum: rulec is asked with each held to its
/// values (DESIGN 3.2), so what the rule answers is exact, nothing is counted in case, and the
/// expectations that rest on it hold, rather than being undecided (W303). The English version and
/// the Japanese one come to the same counts.
#[test]
fn an_enum_and_a_bool_given_to_a_rule_are_held_to_their_values() {
    let mut reports = Vec::new();
    for path in ["tests/walk/held/held.gate", "tests/walk/held/held.ja.gate"] {
        let (diags, c) = check(path, checks::BUDGET as u64);
        assert!(diags.is_empty(), "{path}: {}", rendered(&diags));
        assert!(c.report.spaces.iter().flatten().all(|s| !s.inexact()), "{path}: a value is counted in case");
        // cancel: 4 states x rushed or not, each with the one answer the rule gives, 3 of them
        // allowed (received, and paid without a hurry); pay: an order first received, paid or
        // shipped, which a payment leaves paid or shipped (never cancelled, the rule's fourth
        // state, which the gate's enum does not have), 1 of the 2 allowed
        let counts: Vec<(u128, u128)> = c.report.actions.iter().map(|a| (a.combinations, a.allowed)).collect();
        assert_eq!(counts, [(8, 3), (2, 1)], "{path}");
        let expects: Vec<(u128, u128)> = c.report.expects.iter().map(|e| (e.picked, e.broken)).collect();
        assert_eq!(expects, [(4, 0), (1, 0), (2, 0)], "{path}");
        reports.push(c.report);
    }
    assert_eq!(reports[0].actions, reports[1].actions);
    assert_eq!(reports[0].policies, reports[1].policies);
    assert_eq!(reports[0].expects, reports[1].expects);
}

/// A rule that holds an input of an ordered enum to a range (`campaign : tier  range >=basic
/// <=gold`), given only values the range lets in: an attribute of an enum that stops at gold, and
/// gold as a constant. The gate passes, rulec is asked with each held to its values, so what the
/// rule answers is exact, and the expectations hold. The English version and the Japanese one come
/// to the same counts. The same gate with a campaign that can be platinum is E206 (the mutants
/// `E206_an_enum_outside_the_rule` and `E206_規則の列挙の範囲の外`; rulec's §15.251).
#[test]
fn an_enum_inside_the_range_a_rule_holds_it_to_is_counted_exactly() {
    let mut reports = Vec::new();
    for path in ["tests/walk/ranges/tiers.gate", "tests/walk/ranges/tiers.ja.gate"] {
        let (diags, c) = check(path, checks::BUDGET as u64);
        assert!(diags.is_empty(), "{path}: {}", rendered(&diags));
        assert!(c.report.spaces.iter().flatten().all(|s| !s.inexact()), "{path}: a value is counted in case");
        // ship_lifted: 4 tiers of the member x the 3 campaigns the gate's enum has, 3 of them
        // lifted (silver under gold, basic under silver or gold); ship_at_gold: a member gold
        // lifts or not, the one allowed
        let counts: Vec<(u128, u128)> = c.report.actions.iter().map(|a| (a.combinations, a.allowed)).collect();
        assert_eq!(counts, [(12, 3), (2, 1)], "{path}");
        // a member at gold or platinum under each of the 3 campaigns, none lifted; a basic member
        // under gold, lifted
        let expects: Vec<(u128, u128)> = c.report.expects.iter().map(|e| (e.picked, e.broken)).collect();
        assert_eq!(expects, [(6, 0), (1, 0)], "{path}");
        reports.push(c.report);
    }
    assert_eq!(reports[0].actions, reports[1].actions);
    assert_eq!(reports[0].policies, reports[1].policies);
    assert_eq!(reports[0].expects, reports[1].expects);
}

#[test]
fn relations_are_counted_by_which_terms_are_one_entity() {
    // the owner is the principal or not; the two tenants are one or not; the principal is in the
    // team or not; the two departments are each of two values: 2 x 2 x 2 x 4 combinations
    let (diags, c) = check("tests/walk/relations/tenants.gate", checks::BUDGET as u64);
    assert!(diags.is_empty(), "{}", rendered(&diags));
    assert_eq!(c.report.actions[0].combinations, 32);
    // allowed: the owner (16), or within the tenant a member of the team or of the same department
    // (6 of the 16 where the principal is not the owner)
    assert_eq!(c.report.actions[0].allowed, 22);
    let holds: Vec<u128> = c.report.policies.iter().map(|p| p.holds).collect();
    assert_eq!(holds, vec![16, 8, 8]);
    let s = c.report.spaces[0].as_ref().unwrap();
    ritsu_testkit::golden("tests/walk/golden/read_doc.table.md", &sekisho::table::table(&c.gate, s).markdown(&c.gate, Lang::En));
}

#[test]
fn how_far_a_workflow_and_a_role_are_allowed() {
    use ritsu_ports::{Allowance, Asker, Found};
    let suite = joined();
    let (_, c) = check_with(EN, &suite, checks::BUDGET as u64);
    let refund = c.report.spaces[1].as_ref().unwrap();
    let langs = sekisho::borders::Langs { rules: suite.rules.as_deref(), dates: suite.dates.as_deref(), books: None, flows: None };
    // the returns workflow refunds a returned order up to 50 pounds: sometimes
    match checks::allowance(&c.gate, refund, &c.known, &langs, &Asker::Workflow("returns".into())) {
        Found::Value(Allowance::Sometimes { allowed, denied }) => {
            assert!(allowed.en.contains("status: returned"), "{}", allowed.en);
            assert!(!denied.en.is_empty());
        }
        other => panic!("{other:?}"),
    }
    // an auditor alone never refunds, and looks at an order unless suspended: sometimes
    let auditor = |action: usize| checks::allowance(&c.gate, c.report.spaces[action].as_ref().unwrap(), &c.known, &langs, &Asker::Roles { ty: "User".into(), roles: vec!["auditor".into()] });
    assert_eq!(auditor(1), Found::Value(Allowance::Never));
    assert!(matches!(auditor(0), Found::Value(Allowance::Sometimes { .. })));
    // a customer may look at an order: their own
    let customer = checks::allowance(&c.gate, c.report.spaces[0].as_ref().unwrap(), &c.known, &langs, &Asker::Roles { ty: "Customer".into(), roles: vec![] });
    assert!(matches!(customer, Found::Value(Allowance::Sometimes { .. })), "{customer:?}");
}

fn check_src(src: &str) -> (Vec<Diag>, Checked) {
    let o = sekisho::check::check_text("tests/walk/inline.gate", src, &joined(), &Options::default());
    let said: String = o.diags.iter().map(|d| d.render(Lang::En)).collect();
    (o.diags, o.walked.unwrap_or_else(|| panic!("the names do not hold:\n{said}")))
}

#[test]
fn nobody_holds_or_is_broken() {
    let head = "gate shop v1\n\nrole clerk\n  description \"Answers customers\"\n\nprincipal User\n  roles clerk\n  attributes\n    suspended : bool\n\nresource Order\n\naction erase_order\n  principal User\n  resource Order\n  nobody \"orders are kept\"\n\naction view_order\n  principal User\n  resource Order\n\npermit clerks_view\n  principal in clerk\n  action view_order\n";
    // no one may erase: nothing to say
    let (diags, _) = check_src(head);
    assert!(diags.is_empty(), "{}", rendered(&diags));
    // a permit that lets a clerk erase breaks `nobody`
    let (diags, _) = check_src(&format!("{head}\npermit clerks_erase\n  principal in clerk\n  action erase_order\n  unless principal.suspended\n"));
    let codes: Vec<&str> = diags.iter().map(|d| d.code).collect();
    assert_eq!(codes, vec!["E304"], "{}", rendered(&diags));
    // a clerk who is not suspended: one of the four sets of the clerk role and suspension
    assert!(diags[0].render(Lang::En).contains("`nobody` says no one may `erase_order`, and 1 of its 4 combinations is allowed"), "{}", rendered(&diags));
}

/// The line of `src` that is `line`, counted from 1.
fn line_in(src: &str, line: &str) -> usize {
    src.lines().position(|l| l == line).unwrap_or_else(|| panic!("no line {line:?}")) + 1
}

/// Each note of a diagnostic, in a language.
fn notes_in(d: &Diag, lang: Lang) -> Vec<String> {
    d.notes.iter().map(|n| ritsu_base::text::spaced(n, lang)).collect()
}

/// A permit that names an action where a forbid denies everything it would allow, while it allows
/// something of another it names (W306, DESIGN 4.3): said once, at the permit, with the action, the
/// count, the forbids that deny and an example. Not said of a permit of `action any`, nor where
/// none of the combinations the permit applies to on that action is shown to occur: the walk counts
/// a rule's answer that no input was run for in case, and the permit may apply to none of it.
#[test]
fn a_permit_that_allows_nothing_of_an_action_it_names() {
    let head = "gate library v1\n\nrole staff\n\nprincipal User\n  roles staff\n\nresource Doc\n\naction read_doc\n  principal User\n  resource Doc\n\naction purge_doc\n  principal User\n  resource Doc\n  nobody \"documents are kept\"\n\nforbid no_one_purges\n  action purge_doc\n";
    let src = format!("{head}\npermit staff_read_and_purge\n  principal in staff\n  action read_doc, purge_doc\n");
    let (diags, _) = check_src(&src);
    assert_eq!(diags.iter().map(|d| (d.code, d.line)).collect::<Vec<_>>(), vec![("W306", Some(line_in(&src, "permit staff_read_and_purge")))], "{}", rendered(&diags));
    let en = (
        "The permit `staff_read_and_purge` names the action `purge_doc`, and allows nothing of it: a forbid denies every combination it would allow there",
        [
            "On `purge_doc` it would allow 1 combination; `no_one_purges` denies it.",
            "For example: User holding staff, Doc; denied by `no_one_purges`.",
            "On the other actions it names, it allows 1 combination of `read_doc`.",
            "Remove `purge_doc` from its `action` line, or narrow the forbid if the permit is meant to allow `purge_doc`.",
        ],
    );
    let ja = (
        "permit `staff_read_and_purge` は action `purge_doc` を挙げていますが、`purge_doc` では何も許しません。許すはずの組み合わせを、どれも forbid が拒みます",
        [
            "`purge_doc` で許すはずの組み合わせは 1 通りで、`no_one_purges` が 1 通りを拒みます。",
            "例：staff を持つ User、Doc。`no_one_purges` が拒みます。",
            "ほかに挙げた action では、`read_doc` の 1 通りを許します。",
            "`action` の行から `purge_doc` を外すか、`purge_doc` も許すつもりなら forbid を狭めてください。",
        ],
    );
    for (lang, (message, notes)) in [(Lang::En, en), (Lang::Ja, ja)] {
        assert_eq!(diags[0].message.get(lang), message);
        assert_eq!(notes_in(&diags[0], lang), notes);
    }
    // a permit of `action any` is written for every action, whatever it applies to
    let (diags, _) = check_src(&format!("{head}\npermit staff_do_anything\n  principal in staff\n  action any\n"));
    assert!(diags.is_empty(), "{}", rendered(&diags));
    // `same` of an amount given twice (`compare_doc`), which a forbid denies (as it denies `same` of
    // two amounts, `match_doc`): run through the rule, a combination of `compare_doc` the permit
    // applies to occurs, and the warning is said; with no evaluator to run the rule with, `same` of
    // the amount given twice is counted in case, and nothing is said
    let dir = ritsu_testkit::TempDir::new("w306-in-case");
    std::fs::copy("tests/walk/together/sides.rule", dir.path().join("sides.rule")).unwrap();
    let gate = "gate twice v1\n\nuse rule same_side from \"sides.rule\"\n\nprincipal User\n\nresource Doc\n  attributes\n    amount       : money[JPY]  range >=1JPY <=1_000JPY\n    other_amount : money[JPY]  range >=1JPY <=1_000JPY\n\naction read_doc\n  principal User\n  resource Doc\n\naction compare_doc\n  principal User\n  resource Doc\n  nobody \"no one compares\"\n  context\n    side = same_side(first: resource.amount, second: resource.amount).side\n\naction match_doc\n  principal User\n  resource Doc\n  nobody \"no one matches\"\n  context\n    side = same_side(first: resource.amount, second: resource.other_amount).side\n\npermit users_read_and_compare\n  principal is User\n  action read_doc, compare_doc\n\nforbid no_one_compares_or_matches\n  action compare_doc, match_doc\n  when side is same\n";
    let path = dir.path().join("twice.gate");
    std::fs::write(&path, gate).unwrap();
    let path = path.to_str().unwrap();
    let (diags, c) = check(path, checks::BUDGET as u64);
    assert_eq!(diags.iter().map(|d| (d.code, d.line)).collect::<Vec<_>>(), vec![("W306", Some(line_in(gate, "permit users_read_and_compare")))], "{}", rendered(&diags));
    assert_eq!(counted_together(&c, "compare_doc"), [(vec!["same".to_string()], true)]);
    let mut suite = joined();
    suite.rules = Some(Rc::new(NoEval(rulec::ports::Engine::default())));
    let (diags, c) = check_with(path, &suite, checks::BUDGET as u64);
    assert!(diags.is_empty(), "{}", rendered(&diags));
    assert_eq!(counted_together(&c, "compare_doc"), [(vec!["same".to_string()], false)]);
}

/// A line of a permit that changes no decision (W307, DESIGN 4.3): every combination it leaves out
/// is one a forbid denies. Not said: a line whose combinations another permit allows (permits
/// written case by case), a line of a forbid that leaves out only what no permit allows, and the
/// lines of a permit W301 names. Two lines that change nothing taken out one at a time, but may
/// change an answer taken out together, each say so; where a forbid denies what neither is met on
/// too, they do not.
#[test]
fn a_line_of_a_permit_that_changes_no_decision() {
    let head = "gate library v1\n\nprincipal Service\n  attributes\n    trusted : bool\n\nresource Doc\n  attributes\n    open_to_all : bool\n\naction index_doc\n  principal Service\n  resource Doc\n";
    let index = "\npermit services_index\n  principal is Service\n  action index_doc\n  when resource.open_to_all\n";
    let skip = "\nforbid services_skip_closed_docs\n  action index_doc\n  unless resource.open_to_all\n";
    // two forbids: the second reads whether the service is trusted, which makes two combinations
    // of each document
    let src = format!("{head}{index}{skip}\nforbid untrusted_services_skip_closed_docs\n  action index_doc\n  unless principal.trusted\n  unless resource.open_to_all\n");
    let (diags, _) = check_src(&src);
    assert_eq!(diags.iter().map(|d| (d.code, d.line)).collect::<Vec<_>>(), vec![("W307", Some(line_in(&src, "  when resource.open_to_all")))], "{}", rendered(&diags));
    let en = (
        "The line `when resource.open_to_all` of the permit `services_index` changes no decision: a forbid denies every combination it leaves out",
        [
            "Without it, the permit would also apply to the 2 combinations only this line is not met on: `services_skip_closed_docs` denies 2 of them, `untrusted_services_skip_closed_docs` 1.",
            "For example: Service (trusted: no), Doc (open_to_all: no); denied by `services_skip_closed_docs` and `untrusted_services_skip_closed_docs`.",
            "Taking it out changes no answer: remove it, or correct it if it is meant to leave out something else.",
        ],
    );
    let ja = (
        "permit `services_index` の行 `when resource.open_to_all` は、どの決定も変えません。この行が外す組み合わせを、どれも forbid が拒みます",
        [
            "この行が無ければ、permit は、この行だけが満たされない 2 通りにも当てはまります。そのうち、`services_skip_closed_docs` が 2 通り、`untrusted_services_skip_closed_docs` が 1 通りを拒みます。",
            "例：Service（trusted：いいえ）、Doc（open_to_all：いいえ）。`services_skip_closed_docs`、`untrusted_services_skip_closed_docs` が拒みます。",
            "行を消しても、答えは変わりません。行を消すか、ほかの組み合わせを外すつもりだったなら、行を直してください。",
        ],
    );
    for (lang, (message, notes)) in [(Lang::En, en), (Lang::Ja, ja)] {
        assert_eq!(diags[0].message.get(lang), message);
        assert_eq!(notes_in(&diags[0], lang), notes);
    }
    // permits written case by case: what one leaves out, the other allows
    let (diags, _) = check_src(&format!("{head}{index}\npermit trusted_services_index_closed_docs\n  principal is Service\n  action index_doc\n  unless resource.open_to_all\n"));
    assert!(diags.is_empty(), "{}", rendered(&diags));
    // a forbid's line that leaves out only what no permit allows: the decision is the same, and
    // the forbid keeps it out a second time on purpose
    let (diags, _) = check_src(&format!("{head}{index}\nforbid untrusted_services_skip_open_docs\n  action index_doc\n  when resource.open_to_all\n  unless principal.trusted\n"));
    assert!(diags.is_empty(), "{}", rendered(&diags));
    // a permit another covers (W301) is said whole; its lines are not said one by one
    let src = format!("{head}{index}\npermit trusted_services_index\n  principal is Service\n  action index_doc\n  when resource.open_to_all\n  when principal.trusted\n{skip}");
    let (diags, _) = check_src(&src);
    let at = |code: &'static str, line: usize| (code, Some(line));
    let open = line_in(&src, "  when resource.open_to_all");
    assert_eq!(diags.iter().map(|d| (d.code, d.line)).collect::<Vec<_>>(), vec![at("W307", open), at("W301", line_in(&src, "permit trusted_services_index"))], "{}", rendered(&diags));
    // two lines, each of which changes nothing alone, and which together would let in what no forbid
    // denies (neither attribute set)
    let src = "gate pairs v1\n\nprincipal User\n\nresource Doc\n  attributes\n    x : bool\n    y : bool\n\naction read_doc\n  principal User\n  resource Doc\n\npermit read_both\n  principal is User\n  action read_doc\n  when resource.x\n  when resource.y\n\nforbid only_x\n  action read_doc\n  when resource.x\n  unless resource.y\n\nforbid only_y\n  action read_doc\n  unless resource.x\n  when resource.y\n";
    let (diags, _) = check_src(src);
    let read_both = line_in(src, "permit read_both");
    assert_eq!(diags.iter().map(|d| (d.code, d.line)).collect::<Vec<_>>(), vec![at("W307", read_both + 3), at("W307", read_both + 4)], "{}", rendered(&diags));
    for (d, other) in diags.iter().zip(["when resource.y", "when resource.x"]) {
        assert_eq!(
            notes_in(d, Lang::En).last().unwrap(),
            &format!("`{other}` changes nothing taken out alone too, but taken out together with this line, the two may change an answer: take out one at a time, and check again.")
        );
        assert_eq!(notes_in(d, Lang::Ja).last().unwrap(), &format!("`{other}` も、一つだけ消すなら何も変えませんが、この行と一緒に消すと、答えが変わることがあります。一つずつ消して、検査し直してください。"));
    }
    // with a forbid that denies what neither line is met on, the two change nothing together either
    let src = format!("{src}\nforbid neither\n  action read_doc\n  unless resource.x\n  unless resource.y\n");
    let (diags, _) = check_src(&src);
    assert_eq!(diags.iter().map(|d| (d.code, d.line)).collect::<Vec<_>>(), vec![at("W307", read_both + 3), at("W307", read_both + 4)], "{}", rendered(&diags));
    assert!(diags.iter().all(|d| !notes_in(d, Lang::En).iter().any(|n| n.contains("taken out together"))), "{}", rendered(&diags));
}

/// A line of a policy met wherever its other lines are (W308, DESIGN 4.3): it acts only on
/// combinations that cannot occur. A line another line of its policy implies, of a permit and of a
/// forbid, with the count of the combinations it is met on. Lines the rules decide say which value
/// never comes and why (`values_that_read_the_same_data_are_counted_together`); a line that leaves
/// out only what is counted in case is not said
/// (`a_rulec_that_says_nothing_leaves_the_pairs_undecided_not_wrong`).
#[test]
fn a_line_that_leaves_out_no_combination() {
    let head = "gate shop v1\n\nrole clerk\n\nprincipal User\n  roles clerk\n\nresource Order\n  attributes\n    amount : money[JPY]  range >=1JPY <=100_000JPY\n\naction refund_order\n  principal User\n  resource Order\n";
    let src = format!("{head}\npermit clerks_refund_large_orders\n  principal in clerk\n  action refund_order\n  when resource.amount >= 10_000JPY\n  when resource.amount > 1_000JPY\n");
    let (diags, _) = check_src(&src);
    assert_eq!(diags.iter().map(|d| (d.code, d.line)).collect::<Vec<_>>(), vec![("W308", Some(line_in(&src, "  when resource.amount > 1_000JPY")))], "{}", rendered(&diags));
    let en = (
        "The line `when resource.amount > 1_000JPY` of the permit `clerks_refund_large_orders` leaves out no combination that can happen",
        ["It is met on the 1 combination its `principal` line and its other lines are met on.", "Remove the line, or correct it so that it leaves out what it is meant to."],
    );
    let ja = (
        "permit `clerks_refund_large_orders` の行 `when resource.amount > 1_000JPY` は、起こりうるどの組み合わせも外しません",
        ["`principal` の行とほかの行が満たされる 1 通りで満たされます。", "行を消すか、外すつもりの組み合わせを外すよう行を直してください。"],
    );
    for (lang, (message, notes)) in [(Lang::En, en), (Lang::Ja, ja)] {
        assert_eq!(diags[0].message.get(lang), message);
        assert_eq!(notes_in(&diags[0], lang), notes);
    }
    // of a forbid, over the combinations of both settings of the role
    let src = format!("{head}\npermit clerks_refund\n  principal in clerk\n  action refund_order\n\nforbid large_refunds_wait\n  action refund_order\n  when resource.amount > 50_000JPY\n  when resource.amount >= 10_000JPY\n");
    let (diags, _) = check_src(&src);
    assert_eq!(diags.iter().map(|d| (d.code, d.line)).collect::<Vec<_>>(), vec![("W308", Some(line_in(&src, "  when resource.amount >= 10_000JPY")))], "{}", rendered(&diags));
    assert_eq!(diags[0].message.get(Lang::En), "The line `when resource.amount >= 10_000JPY` of the forbid `large_refunds_wait` leaves out no combination that can happen");
    assert_eq!(notes_in(&diags[0], Lang::En)[0], "It is met on each of the 2 combinations its other lines are met on.");
    assert_eq!(notes_in(&diags[0], Lang::Ja)[0], "ほかの行が満たされる 2 通りの、どれでも満たされます。");
}

#[test]
fn an_owner_that_may_be_absent_is_absent_or_one_entity() {
    // the owner is absent, the principal, or another user: 3 ways
    let src = "gate docs v1\n\nprincipal User\n\nresource Doc\n  attributes\n    owner : User?\n\naction read_doc\n  principal User\n  resource Doc\n\npermit owners_read\n  action read_doc\n  when resource.owner is principal\n\npermit unowned_read\n  action read_doc\n  unless resource.owner is principal\n";
    let (diags, c) = check_src(src);
    assert!(diags.is_empty(), "{}", rendered(&diags));
    assert_eq!(c.report.actions[0].combinations, 3);
    let holds: Vec<u128> = c.report.policies.iter().map(|p| p.holds).collect();
    assert_eq!(holds, vec![1, 2]);
}

/// A gate beside a dates file, checked with every language joined.
fn check_with_dates(gate: &str, dates: &str) -> (Vec<Diag>, Checked) {
    let dir = ritsu_testkit::TempDir::new("dates");
    std::fs::write(dir.path().join("plus.cal"), dates).unwrap();
    let path = dir.path().join("example.gate");
    std::fs::write(&path, gate).unwrap();
    check_with(path.to_str().unwrap(), &joined(), checks::BUDGET as u64)
}

const PLUS: &str = "dates plus v1\ndescription \"Three days after a day\"\n\ninputs\n  d : date  range >=2026-01-01 <=2026-12-31\n\ndate later = d\n  + 3 days\n";

#[test]
fn a_date_given_today_is_computed_again_for_every_day() {
    // today is always before three days after itself: one truth, one combination, and the line
    // that asks for it leaves out nothing (W308)
    let gate = "gate t v1\n\nuse dates plus from \"plus.cal\"\n\ntoday range >=2026-03-01 <=2026-03-31 offset +00:00\n\nprincipal User\n\nresource Doc\n\naction read_doc\n  principal User\n  resource Doc\n  context\n    early = today < plus.later(d: today)\n\npermit early_reads\n  principal is User\n  action read_doc\n  when early\n";
    let (diags, c) = check_with_dates(gate, PLUS);
    assert_eq!(diags.iter().map(|d| (d.code, d.line)).collect::<Vec<_>>(), vec![("W308", Some(20))], "{}", rendered(&diags));
    assert_eq!((c.report.actions[0].combinations, c.report.actions[0].allowed), (1, 1));
    // a permit that asks for it not to hold never applies
    let (diags, _) = check_with_dates(&format!("{gate}\npermit late_reads\n  principal is User\n  action read_doc\n  unless early\n"), PLUS);
    assert_eq!(diags.iter().map(|d| d.code).collect::<Vec<_>>(), vec!["W308", "E303"], "{}", rendered(&diags));
}

#[test]
fn a_date_attribute_against_today() {
    // a due day in March and today in March: before, on and after it all happen
    let gate = "gate t v1\n\ntoday range >=2026-03-01 <=2026-03-31 offset +00:00\n\nprincipal User\n\nresource Bill\n  attributes\n    due_on : date  range >=2026-03-10 <=2026-03-20\n\naction pay_bill\n  principal User\n  resource Bill\n  context\n    overdue = today > resource.due_on\n\npermit users_pay_until_due\n  principal is User\n  action pay_bill\n  unless overdue\n";
    let (diags, c) = check_with_dates(gate, PLUS);
    assert!(diags.is_empty(), "{}", rendered(&diags));
    assert_eq!((c.report.actions[0].combinations, c.report.actions[0].allowed), (2, 1));
}

const THREE: &str = "rule three v1\ndescription \"Whether three amounts come to more than 150\"\n\nenum level = low | high\n\ninputs\n  a : number  range >=0 <=100\n  b : number  range >=0 <=100\n  c : number  range >=0 <=100\n\noutputs\n  level : level\n\nderive total : number = a + b + c  range >=0 <=300\n\ntable decide\npolicy unique\n| total | -> level : level |\n| <=150 | low              |\n| >150  | high             |\n";

#[test]
fn a_rule_over_many_cells_is_asked_once_and_tried_with_inputs() {
    // three numbers, each cut by sixteen constants into 17 cells: 4,913 combinations of cells, more
    // than rulec is asked over one by one
    let mut gate = String::from("gate many v1\n\nuse rule three from \"three.rule\"\n\nprincipal User\n  attributes\n    a : number  range >=0 <=100\n    b : number  range >=0 <=100\n    c : number  range >=0 <=100\n\nresource Doc\n\naction read_doc\n  principal User\n  resource Doc\n  context\n    level = three(a: principal.a, b: principal.b, c: principal.c).level\n\npermit high_reads\n  principal is User\n  action read_doc\n  when level is high\n");
    for x in ["a", "b", "c"] {
        let conds: Vec<String> = (1..=16).map(|k| format!("principal.{x} > {}", k * 5)).collect();
        gate.push_str(&format!("\nforbid {x}_bounds\n  principal is User\n  action read_doc\n  when {} and principal.{x} < 0\n", conds.join(" and ")));
    }
    let dir = ritsu_testkit::TempDir::new("many");
    std::fs::write(dir.path().join("three.rule"), THREE).unwrap();
    let path = dir.path().join("example.gate");
    std::fs::write(&path, &gate).unwrap();
    let t = std::time::Instant::now();
    let (diags, c) = check_with(path.to_str().unwrap(), &joined(), checks::BUDGET as u64);
    assert!(t.elapsed().as_secs() < 60, "{:?}", t.elapsed());
    // the three forbids never apply (E303), and nothing else is wrong: the permit allows, which a
    // concrete input confirms, though the level is counted in case over every cell
    let codes: Vec<&str> = diags.iter().map(|d| d.code).collect();
    assert_eq!(codes, vec!["E303", "E303", "E303"], "{}", rendered(&diags));
    assert_eq!(c.report.actions[0].combinations, 17 * 17 * 17 * 2);
    assert!(c.report.spaces[0].as_ref().unwrap().inexact());
}

/// rulec, saying it cannot decide what an output comes to over ranges (as for a table over a value
/// it computes), and answering everything else itself.
struct Unsure(rulec::ports::Engine);

impl ritsu_ports::Rules for Unsure {
    fn facts(&self, rule: &Path) -> Result<ritsu_ports::RuleFacts, Vec<Said>> {
        self.0.facts(rule)
    }
    fn preconditions_hold(&self, rule: &Path, ranges: &[(String, Option<i128>, Option<i128>)], min_len: Option<i128>, max_len: Option<i128>) -> Result<Vec<(ritsu_ports::Precondition, ritsu_ports::Answer<Values>)>, Vec<Said>> {
        self.0.preconditions_hold(rule, ranges, min_len, max_len)
    }
    fn output_values(&self, rule: &Path, output: &str) -> Result<Found<ritsu_ports::OutputValues>, Vec<Said>> {
        self.0.output_values(rule, output)
    }
    fn checked_over(&self, rule: &Path, input: &str, days: &DaySet) -> Result<ritsu_ports::Answer<ritsu_base::text::Text>, Vec<Said>> {
        self.0.checked_over(rule, input, days)
    }
    fn eval(&self, rule: &Path, inputs: &Values) -> Result<Values, RuleError> {
        self.0.eval(rule, inputs)
    }
    fn doc(&self, rule: &Path, shown: &str, html: bool, lang: Lang) -> Result<String, Vec<Said>> {
        self.0.doc(rule, shown, html, lang)
    }
}

const GRADES: &str = "rule 等級の割引(grade_discount) v1\ndescription \"等級で割引があるか\"\n\nenum 区分(kind) = 金(gold) | 銀(silver)\nenum 割引の有無(discount_kind) = あり(yes) | なし(no)\n\ninputs\n  等級(grade) : 区分\n\noutputs\n  割引(discount) : 割引の有無\n\ntable 決める\npolicy unique\n| 等級 | -> 割引 : 割引の有無 |\n| 金   | あり                |\n| 銀   | なし                |\n";

#[test]
fn a_value_named_by_its_public_name_is_given_to_the_rule_by_the_rule_s_name() {
    // the gate's enum names the rule's values by their public names (`gold`), not by the rule's
    // names (`金`): tried with a concrete input, each is given to rulec's evaluator as the rule names it
    let gate = "gate 店(shop_ja) v1\n\nuse rule 等級の割引 from \"grades.rule\"\n\nenum 等級(grade) = ゴールド(gold) | シルバー(silver)\n\nprincipal 会員(Member)\n  attributes\n    等級(grade) : 等級\n\nresource 注文(Order)\n\naction 注文する(place_order)\n  principal 会員\n  resource 注文\n  context\n    割引(discount) = 等級の割引(等級: principal.等級).割引\n\npermit 割引のある会員は注文できる(discounted_order)\n  principal is 会員\n  action 注文する\n  when 割引 is あり\n";
    let dir = ritsu_testkit::TempDir::new("public");
    std::fs::write(dir.path().join("grades.rule"), GRADES).unwrap();
    let path = dir.path().join("example.gate");
    std::fs::write(&path, gate).unwrap();
    let p = sekisho::parse::parse(path.to_str().unwrap(), gate);
    let f = p.file.unwrap();
    let rules = Unsure(rulec::ports::Engine::default());
    let dates = koyomi::ports::Engine;
    let langs = sekisho::borders::Langs { rules: Some(&rules), dates: Some(&dates), books: None, flows: None };
    let (diags, c) = checks::all(&f, &[], &langs, checks::BUDGET, dir.path());
    // the discount is counted in case (inexact) for each grade, and a concrete input confirms it:
    // nothing is undecided
    assert!(diags.is_empty(), "{}", rendered(&diags));
    let s = c.report.spaces[0].as_ref().unwrap();
    assert!(s.inexact());
    // the grade is given to the rule only, so the walk counts the discount's two values
    assert_eq!((c.report.actions[0].combinations, c.report.actions[0].allowed), (2, 1));
}

/// The values the rule outputs of an action come to together, as the walk counts them: for each
/// point of the factor that holds them, the values of its places (the attributes and inputs a
/// policy reads too, then the outputs) in words, and whether the point is marked exact.
fn counted_together(c: &Checked, action: &str) -> Vec<(Vec<String>, bool)> {
    use sekisho::walk::{FactorKind, Val};
    let ai = c.gate.actions.iter().position(|a| a.named.alias == action).unwrap_or_else(|| panic!("no action {action}"));
    let s = c.report.spaces[ai].as_ref().unwrap_or_else(|| panic!("{action} was not walked"));
    let f = &s.frames[0];
    let x = f.factors.iter().find(|x| matches!(x.kind, FactorKind::Rules { .. })).unwrap_or_else(|| panic!("{action} computes nothing by a rule"));
    x.points
        .iter()
        .map(|p| {
            let said = p
                .vals
                .iter()
                .zip(&x.places)
                .map(|(v, at)| match (v, &f.slots[*at]) {
                    (Val::Enum(k), sekisho::walk::Slot::Computed(i)) => s.names[i][*k].alias.clone(),
                    (Val::Cell(cell), _) => format!("{}..{}", cell.lo, cell.hi),
                    (Val::Absent, _) => "absent".to_string(),
                    (other, _) => format!("{other:?}"),
                })
                .collect();
            (said, p.exact)
        })
        .collect()
}

/// Values that read the same data are counted together (DESIGN 4.1): only the combinations of
/// them that happen, each marked exact. `tests/walk/together/together.gate` and its Japanese twin
/// take, action by action,
///
/// - two outputs of one call that one table writes (`a` and `b` are low together up to 500 yen and
///   high together above it): two combinations, not four, over an amount no policy reads; and with
///   the amount cut at 100 yen by an expectation, three;
/// - two outputs of one call from a table each (`a` cut at 500 yen, `c` at 300): three, not four;
/// - two rules given the same amount (`a`, and `step` cut at 300 yen): three, not four;
/// - one amount given as both inputs of a rule: both low or both high, never one of each;
/// - one amount given as both inputs of a rule that answers the same on either side of 500 yen:
///   `same` alone, never `apart`, though no two documents differ to show where to cut the amount;
/// - two outputs of an amount that may be absent: both absent together, never one of them.
///
/// The expectations that say so hold — every document is read, filed, sent, weighed and
/// compared — and the two that pick what never happens (`a` low and `b` high; an amount apart from
/// itself) pick nothing (W304), where counting every pair found the first broken (E304) by a
/// combination no input comes to. The lines of the permits that ask for what always happens (not
/// `mixed`; `same`) leave out nothing (W308).
#[test]
fn values_that_read_the_same_data_are_counted_together() {
    let mut reports = Vec::new();
    for (path, lang, golden) in [("tests/walk/together/together.gate", Lang::En, "tests/walk/golden/together.tables.md"), ("tests/walk/together/together.ja.gate", Lang::Ja, "tests/walk/golden/together.tables.ja.md")] {
        let (diags, c) = check(path, checks::BUDGET as u64);
        let codes: Vec<&str> = diags.iter().map(|d| d.code).collect();
        assert_eq!(codes, vec!["W308", "W308", "W304", "W304"], "{path}: {}", rendered(&diags));
        assert_eq!(diags.iter().map(|d| d.line).collect::<Vec<_>>(), vec![Some(109), Some(115), Some(166), Some(172)], "{path}");
        // why the line leaves out nothing, and that what the rules decide is the rules' to hold
        let notes = |d: &Diag| -> Vec<String> { d.notes.iter().map(|n| ritsu_base::text::spaced(n, lang)).collect() };
        let (why, fix) = match lang {
            Lang::En => (
                "`kind` is never `mixed`: it is computed from `resource.amount` (given as both `first` and `second`), and the rule answers `mixed` for none of the values the gate gives it.",
                "The rules decide this, so the line changes nothing of what the permit applies to. To hold what a rule answers, write it in the rule (its `examples`, which rulec checks); here, remove the line or correct it.",
            ),
            Lang::Ja => (
                "`どちら側` が `片方ずつ` になることはありません。`どちら側` は `一つ目` と `二つ目` の両方に渡す `resource.金額` から計算する値で、ゲートが渡す値のどれにも、規則は `片方ずつ` を答えません。",
                "これは規則が決めていることで、この行は、permit が当てはまる組み合わせを何も変えません。規則の答えを確かめたいなら規則の側に書き（`examples`。rulec が検査します）、この行は消すか、直してください。",
            ),
        };
        assert_eq!(notes(&diags[0])[1..], [why.to_string(), fix.to_string()], "{path}");
        assert!(c.report.spaces.iter().flatten().all(|s| !s.inexact()), "{path}: a value is counted in case");
        let said = |action: &str| -> Vec<Vec<String>> {
            let points = counted_together(&c, action);
            assert!(points.iter().all(|(_, exact)| *exact), "{path} {action}: {points:?}");
            points.into_iter().map(|(v, _)| v).collect()
        };
        let row = |vals: &[&str]| -> Vec<String> { vals.iter().map(|v| v.to_string()).collect() };
        assert_eq!(said("read_doc"), [row(&["1..100", "low", "low"]), row(&["101..1000", "low", "low"]), row(&["101..1000", "high", "high"])], "{path}");
        assert_eq!(said("file_doc"), [row(&["low", "low"]), row(&["low", "high"]), row(&["high", "high"])], "{path}");
        assert_eq!(said("send_doc"), [row(&["low", "small"]), row(&["low", "large"]), row(&["high", "large"])], "{path}");
        assert_eq!(said("weigh_doc"), [row(&["both_low"]), row(&["both_high"])], "{path}");
        assert_eq!(said("compare_doc"), [row(&["same"])], "{path}");
        assert_eq!(said("price_doc"), [row(&["low", "low"]), row(&["high", "high"]), row(&["absent", "absent"])], "{path}");
        // each with the staff role held or not
        let counts: Vec<(u128, u128)> = c.report.actions.iter().map(|a| (a.combinations, a.allowed)).collect();
        assert_eq!(counts, [(6, 3), (6, 3), (6, 3), (4, 2), (2, 1), (6, 2)], "{path}");
        // every expectation holds; the last two pick nothing
        let expects: Vec<(u128, u128)> = c.report.expects.iter().map(|e| (e.picked, e.broken)).collect();
        assert_eq!(expects, [(3, 0), (1, 0), (3, 0), (3, 0), (2, 0), (1, 0), (2, 0), (0, 0), (0, 0)], "{path}");
        // the tables of the page hold no row of a pair that never happens
        let mut tables = String::new();
        for (ai, a) in c.gate.actions.iter().enumerate() {
            let t = sekisho::table::table(&c.gate, c.report.spaces[ai].as_ref().unwrap());
            tables.push_str(&format!("### {}\n\n{}\n", a.named.name, t.markdown(&c.gate, lang)));
        }
        ritsu_testkit::golden(golden, &tables);
        reports.push(c.report);
    }
    assert_eq!(reports[0].actions, reports[1].actions);
    assert_eq!(reports[0].policies, reports[1].policies);
    assert_eq!(reports[0].expects, reports[1].expects);
}

/// A rule's input that may be left out, given an attribute (`tests/walk/given/given.gate` and its
/// twin). The gate never calls the rule with the input left out: an attribute that is always there
/// gives it a value, and an attribute that is absent leaves the computed value absent too. So the
/// value the rule keeps for the input left out (`unknown`) is not counted, exactly: four
/// combinations of `read_doc` (the role held or not, `child` or `adult`) and three of `skim_doc`
/// (`child`, `adult`, absent), where counting what the rule comes to over every input gave a
/// third band that never happens, counted in case (W303 for the expectation that every reader
/// reads). The expectation that picks `unknown` picks nothing (W304), and the line of the permit
/// that leaves `unknown` out leaves out nothing (W308). In the table, the row of the absent band
/// is a row of its own, not folded into the row of `child` beside it.
#[test]
fn an_input_that_may_be_left_out_is_given_by_the_attribute_passed_to_it() {
    let mut reports = Vec::new();
    for (path, lang, golden) in [("tests/walk/given/given.gate", Lang::En, "tests/walk/golden/given.tables.md"), ("tests/walk/given/given.ja.gate", Lang::Ja, "tests/walk/golden/given.tables.ja.md")] {
        let (diags, c) = check(path, checks::BUDGET as u64);
        let codes: Vec<&str> = diags.iter().map(|d| d.code).collect();
        assert_eq!(codes, vec!["W308", "W304"], "{path}: {}", rendered(&diags));
        assert_eq!(diags[0].line, Some(42), "{path}");
        assert!(c.report.spaces.iter().flatten().all(|s| !s.inexact()), "{path}: a value is counted in case");
        let said = |action: &str| -> Vec<Vec<String>> {
            let points = counted_together(&c, action);
            assert!(points.iter().all(|(_, exact)| *exact), "{path} {action}: {points:?}");
            points.into_iter().map(|(v, _)| v).collect()
        };
        let row = |vals: &[&str]| -> Vec<String> { vals.iter().map(|v| v.to_string()).collect() };
        assert_eq!(said("read_doc"), [row(&["child"]), row(&["adult"])], "{path}");
        assert_eq!(said("skim_doc"), [row(&["child"]), row(&["adult"]), row(&["absent"])], "{path}");
        let counts: Vec<(u128, u128)> = c.report.actions.iter().map(|a| (a.combinations, a.allowed)).collect();
        assert_eq!(counts, [(4, 2), (3, 1)], "{path}");
        let expects: Vec<(u128, u128)> = c.report.expects.iter().map(|e| (e.picked, e.broken)).collect();
        assert_eq!(expects, [(2, 0), (2, 0), (0, 0)], "{path}");
        let mut tables = String::new();
        for (ai, a) in c.gate.actions.iter().enumerate() {
            let t = sekisho::table::table(&c.gate, c.report.spaces[ai].as_ref().unwrap());
            tables.push_str(&format!("### {}\n\n{}\n", a.named.name, t.markdown(&c.gate, lang)));
        }
        ritsu_testkit::golden(golden, &tables);
        reports.push(c.report);
    }
    assert_eq!(reports[0].actions, reports[1].actions);
    assert_eq!(reports[0].policies, reports[1].policies);
    assert_eq!(reports[0].expects, reports[1].expects);
}

/// Two outputs of the sum of two numbers (`tests/walk/together/sums.gate` and its twin): neither
/// changes along one of the numbers alone, so the walk cannot cut the numbers into parts where
/// rulec's answers say which pairs happen. The pairs a concrete input comes to are marked exact;
/// the one no input was found for (`x` low and `y` high: a sum of 100 or less over 150) is counted
/// in case, and the expectation that rests on it is undecided (W303), not said to be broken.
#[test]
fn a_pair_no_input_is_found_for_is_counted_in_case_and_undecided() {
    for path in ["tests/walk/together/sums.gate", "tests/walk/together/sums.ja.gate"] {
        let t = std::time::Instant::now();
        let (diags, c) = check(path, checks::BUDGET as u64);
        assert!(t.elapsed().as_secs() < 60, "{path}: {:?}", t.elapsed());
        let codes: Vec<&str> = diags.iter().map(|d| d.code).collect();
        assert_eq!(codes, vec!["W303"], "{path}: {}", rendered(&diags));
        let row = |vals: &[&str]| -> Vec<String> { vals.iter().map(|v| v.to_string()).collect() };
        assert_eq!(counted_together(&c, "read_doc"), [(row(&["low", "low"]), true), (row(&["low", "high"]), false), (row(&["high", "low"]), true), (row(&["high", "high"]), true)], "{path}");
        assert!(c.report.spaces[0].as_ref().unwrap().inexact(), "{path}");
        if path.ends_with("sums.gate") {
            ritsu_testkit::golden("tests/walk/golden/W303_sums.txt", &rendered(&diags));
        }
    }
}

/// With a rulec that says nothing of what outputs come to over ranges (the port's defaults), the
/// walk still cuts the amount where concrete inputs show the outputs change, and marks exact the
/// pairs an input comes to. The pairs it cannot rule out are counted in case: an expectation that
/// rests on one is undecided (W303), and none is said to be broken. A line that leaves out only
/// what is counted in case (`unless kind is mixed`, `when side is same`) is not said to leave out
/// nothing (W308): the combination counted in case is one it leaves out.
#[test]
fn a_rulec_that_says_nothing_leaves_the_pairs_undecided_not_wrong() {
    let mut suite = joined();
    suite.rules = Some(Rc::new(Unsure(rulec::ports::Engine::default())));
    let (diags, c) = check_with("tests/walk/together/together.gate", &suite, checks::BUDGET as u64);
    assert!(diags.iter().all(|d| d.code == "W303" || d.code == "W304"), "{}", rendered(&diags));
    assert!(diags.iter().any(|d| d.code == "W303"), "{}", rendered(&diags));
    let kinds = counted_together(&c, "weigh_doc");
    assert!(kinds.iter().any(|(v, exact)| !*exact && *v == ["mixed".to_string()]), "{kinds:?}");
    // file_doc: the three pairs that happen were each found by an input; the fourth is in case
    let points = counted_together(&c, "file_doc");
    let exact: Vec<Vec<String>> = points.iter().filter(|(_, e)| *e).map(|(v, _)| v.clone()).collect();
    let row = |vals: &[&str]| -> Vec<String> { vals.iter().map(|v| v.to_string()).collect() };
    assert_eq!(exact, [row(&["low", "low"]), row(&["low", "high"]), row(&["high", "high"])]);
    assert!(points.iter().any(|(v, e)| !*e && *v == row(&["high", "low"])), "{points:?}");
}

const SAME_KIND: &str = "rule same_kind v1\ndescription \"Whether two kinds are one kind\"\n\nenum kind = paper | film\nenum side = same | apart\n\ninputs\n  first  : kind\n  second : kind\n\noutputs\n  side : side\n\ntable decide\npolicy unique\n| first | second | -> side |\n| paper | paper  | same    |\n| film  | film   | same    |\n| paper | film   | apart   |\n| film  | paper  | apart   |\n";

const SAME_FLAG: &str = "rule same_flag v1\ndescription \"Whether two flags are one flag\"\n\nenum side = same | apart\n\ninputs\n  first  : bool\n  second : bool\n\noutputs\n  side : side\n\ntable decide\npolicy unique\n| first | second | -> side |\n| true  | true   | same    |\n| false | false  | same    |\n| true  | false  | apart   |\n| false | true   | apart   |\n";

/// A gate that gives one attribute as both inputs of a rule, three times over: an amount, an enum
/// and a bool. Each rule answers `same` wherever its two inputs are one value, and `apart` only
/// where they are two, which no request gives it. A permit and an expectation read `apart`.
fn given_twice(dir: &Path) -> String {
    std::fs::copy("tests/walk/together/sides.rule", dir.join("sides.rule")).unwrap();
    std::fs::write(dir.join("kinds.rule"), SAME_KIND).unwrap();
    std::fs::write(dir.join("flags.rule"), SAME_FLAG).unwrap();
    let mut gate = String::from("gate twice v1\n\nuse rule same_side from \"sides.rule\"\nuse rule same_kind from \"kinds.rule\"\nuse rule same_flag from \"flags.rule\"\n\nenum kind = paper | film\n\nprincipal User\n\nresource Doc\n  attributes\n    amount : money[JPY]  range >=1JPY <=1_000JPY\n    kind   : kind\n    sealed : bool\n");
    for (by, rule, attribute) in [("amount", "same_side", "amount"), ("kind", "same_kind", "kind"), ("flag", "same_flag", "sealed")] {
        gate.push_str(&format!("\naction by_{by}\n  principal User\n  resource Doc\n  context\n    side = {rule}(first: resource.{attribute}, second: resource.{attribute}).side\n"));
    }
    for by in ["amount", "kind", "flag"] {
        gate.push_str(&format!("\npermit same_{by}\n  principal is User\n  action by_{by}\n  when side is same\n\npermit apart_{by}\n  principal is User\n  action by_{by}\n  when side is apart\n"));
    }
    for by in ["amount", "kind", "flag"] {
        gate.push_str(&format!("\nexpect deny never_apart_{by}\n  action by_{by}\n  when side is apart\n"));
    }
    let path = dir.join("twice.gate");
    std::fs::write(&path, &gate).unwrap();
    path.to_str().unwrap().to_string()
}

/// One attribute given as two inputs of a rule whose answer is the same wherever the two are one
/// value (DESIGN 4.1). No two concrete inputs differ, so nothing shows where to cut the attribute;
/// rulec, asked as if the two inputs were given apart, says `apart` happens, and adds an input
/// that gives the attribute two values. The walk cuts the attribute between the two and asks
/// again: `apart` is not counted, the permit that reads it applies to nothing (E303), the line of
/// the permit that reads `same` leaves out nothing (W308), the expectation that picks `apart`
/// picks nothing (W304), and the table has no row of it. Counting `apart` in case left both
/// undecided (W303).
#[test]
fn a_thing_given_twice_is_cut_between_the_two_values_rulec_gave_it() {
    let dir = ritsu_testkit::TempDir::new("twice");
    let path = given_twice(dir.path());
    let (diags, c) = check(&path, checks::BUDGET as u64);
    let said: Vec<(&str, String)> = diags.iter().map(|d| (d.code, d.render(Lang::En).lines().next().unwrap_or_default().to_string())).collect();
    for by in ["amount", "kind", "flag"] {
        assert!(said.iter().any(|(code, line)| *code == "E303" && line.contains(&format!("`apart_{by}`"))), "{by}: {}", rendered(&diags));
        assert!(said.iter().any(|(code, line)| *code == "W308" && line.contains(&format!("`same_{by}`"))), "{by}: {}", rendered(&diags));
        assert!(said.iter().any(|(code, line)| *code == "W304" && line.contains(&format!("`never_apart_{by}`"))), "{by}: {}", rendered(&diags));
        assert_eq!(counted_together(&c, &format!("by_{by}")), [(vec!["same".to_string()], true)], "{by}");
    }
    assert_eq!(diags.len(), 9, "{}", rendered(&diags));
    assert!(c.report.spaces.iter().flatten().all(|s| !s.inexact()), "a value is counted in case");
    for (ai, a) in c.gate.actions.iter().enumerate() {
        let t = sekisho::table::table(&c.gate, c.report.spaces[ai].as_ref().unwrap()).markdown(&c.gate, Lang::En);
        assert!(t.contains("same") && !t.contains("| apart |"), "{}: {t}", a.named.name);
    }
}

/// The same gate with a rulec that says nothing of what an output comes to over ranges, with
/// its evaluator and without: nothing tells the walk that `apart` never happens, so it is counted
/// in case. Whether the permits that read it apply, and whether the expectations hold with them,
/// is undecided (W303): no permit is said to apply to nothing, and no expectation is said to be
/// broken or to pick nothing.
#[test]
fn a_thing_given_twice_is_left_undecided_by_a_rulec_that_says_nothing() {
    let dir = ritsu_testkit::TempDir::new("twice-unsure");
    let path = given_twice(dir.path());
    let stand_ins: [(&str, Rc<dyn ritsu_ports::Rules>); 2] = [("with its evaluator", Rc::new(Unsure(rulec::ports::Engine::default()))), ("without", Rc::new(Silent(rulec::ports::Engine::default())))];
    for (which, rules) in stand_ins {
        let mut suite = joined();
        suite.rules = Some(rules);
        let (diags, c) = check_with(&path, &suite, checks::BUDGET as u64);
        assert!(diags.iter().all(|d| d.code == "W303"), "{which}: {}", rendered(&diags));
        for by in ["amount", "kind", "flag"] {
            assert!(diags.iter().any(|d| d.render(Lang::En).contains(&format!("`apart_{by}`"))), "{which} {by}: {}", rendered(&diags));
            let points = counted_together(&c, &format!("by_{by}"));
            assert!(points.iter().any(|(v, exact)| !*exact && *v == ["apart".to_string()]), "{which} {by}: {points:?}");
        }
    }
}

/// rulec, answering everything itself but for its evaluator, which gives no answer.
struct NoEval(rulec::ports::Engine);

impl ritsu_ports::Rules for NoEval {
    fn facts(&self, rule: &Path) -> Result<ritsu_ports::RuleFacts, Vec<Said>> {
        self.0.facts(rule)
    }
    fn preconditions_hold(&self, rule: &Path, ranges: &[(String, Option<i128>, Option<i128>)], min_len: Option<i128>, max_len: Option<i128>) -> Result<Vec<(ritsu_ports::Precondition, ritsu_ports::Answer<Values>)>, Vec<Said>> {
        self.0.preconditions_hold(rule, ranges, min_len, max_len)
    }
    fn output_values(&self, rule: &Path, output: &str) -> Result<Found<ritsu_ports::OutputValues>, Vec<Said>> {
        self.0.output_values(rule, output)
    }
    fn outputs_over(&self, rule: &Path, output: &str, ranges: &[(String, Option<i128>, Option<i128>)]) -> Result<Found<Vec<(ritsu_ports::Value, Values)>>, Vec<Said>> {
        self.0.outputs_over(rule, output, ranges)
    }
    fn outputs_together(&self, rule: &Path, outputs: &[String], ranges: &[(String, Option<i128>, Option<i128>)]) -> Result<Found<Vec<(Vec<ritsu_ports::Value>, Values)>>, Vec<Said>> {
        self.0.outputs_together(rule, outputs, ranges)
    }
    fn checked_over(&self, rule: &Path, input: &str, days: &DaySet) -> Result<ritsu_ports::Answer<ritsu_base::text::Text>, Vec<Said>> {
        self.0.checked_over(rule, input, days)
    }
    fn eval(&self, _rule: &Path, _inputs: &Values) -> Result<Values, RuleError> {
        Err(RuleError::Contradiction(ritsu_base::text::Text::same("no evaluator")))
    }
    fn doc(&self, rule: &Path, shown: &str, html: bool, lang: Lang) -> Result<String, Vec<Said>> {
        self.0.doc(rule, shown, html, lang)
    }
}

/// The same gate with rules whose evaluator gives no answer: the walk cuts each attribute by the
/// inputs rulec found alone. `apart` is not counted (E303, W304, and W308 for the line of the
/// permit that reads `same`), and where rulec was still asked of two inputs given apart (the
/// amount, each side of 500 yen), `same` rests on no input run: whether its permit applies, and so
/// whether anyone is allowed the action, is undecided (W303), not said either way, and its line is
/// not said to leave out nothing.
#[test]
fn a_thing_given_twice_is_cut_without_an_evaluator_and_nothing_wrong_is_said() {
    let dir = ritsu_testkit::TempDir::new("twice-no-eval");
    let path = given_twice(dir.path());
    let mut suite = joined();
    suite.rules = Some(Rc::new(NoEval(rulec::ports::Engine::default())));
    let (diags, c) = check_with(&path, &suite, checks::BUDGET as u64);
    let said: Vec<(&str, String)> = diags.iter().map(|d| (d.code, d.render(Lang::En).lines().next().unwrap_or_default().to_string())).collect();
    for by in ["amount", "kind", "flag"] {
        assert!(said.iter().any(|(code, line)| *code == "E303" && line.contains(&format!("`apart_{by}`"))), "{by}: {}", rendered(&diags));
        assert!(said.iter().any(|(code, line)| *code == "W304" && line.contains(&format!("`never_apart_{by}`"))), "{by}: {}", rendered(&diags));
    }
    for subject in ["`by_amount`", "`same_amount`"] {
        assert!(said.iter().any(|(code, line)| *code == "W303" && line.contains(subject)), "{subject}: {}", rendered(&diags));
    }
    for by in ["kind", "flag"] {
        assert!(said.iter().any(|(code, line)| *code == "W308" && line.contains(&format!("`same_{by}`"))), "{by}: {}", rendered(&diags));
    }
    assert!(!said.iter().any(|(code, line)| *code == "W308" && line.contains("`same_amount`")), "{}", rendered(&diags));
    assert_eq!(diags.len(), 10, "{}", rendered(&diags));
    assert_eq!(counted_together(&c, "by_amount"), [(vec!["same".to_string()], false)]);
    assert_eq!(counted_together(&c, "by_kind"), [(vec!["same".to_string()], true)]);
    assert_eq!(counted_together(&c, "by_flag"), [(vec!["same".to_string()], true)]);
}

const HEAD: &str = "use rule sizes from \"together/sizes.rule\"\nuse rule steps from \"together/steps.rule\"\nuse rule same_side from \"together/sides.rule\"\n\nrole staff\n\nprincipal User\n  roles staff\n\nresource Doc\n  attributes\n    amount : money[JPY]  range >=1JPY <=1_000JPY\n";

/// An example says the combination as the walk counts it and, in brackets after its computed
/// values, the input that was run through the rules and came to them (DESIGN 4.2):
///
/// - one input, where the walk knows no more: `(as when amount is 301JPY)` for `a` low and `step`
///   large (301 to 500 yen, which the walk does not cut any finer than 301 to 1,000), and for the
///   one output of one rule;
/// - a region, where rulec says every input of it comes to these values and to nothing else:
///   `(whenever amount is 101JPY to 300JPY)` inside the cell over 100 yen a policy cuts, and
///   `(whenever amount is 1JPY to 500JPY)` of an attribute no policy reads.
///
/// The combination itself is said as it is counted: the cell a policy cuts stays the cell
/// (`amount: 101JPY to 1000JPY`), and an attribute no policy reads is not in it. The JSON keeps
/// the combination under `example` and has the inputs under `example_inputs`. Each input said
/// comes to the values it stands beside when the rules are run with it, at both ends of a region.
/// Without the rules' evaluator no input is run: the example is the combination alone, and the
/// JSON has no `example_inputs`.
#[test]
fn an_example_says_the_input_that_comes_to_it() {
    let gate = format!(
        "gate inputs v1\n\n{HEAD}\n{}\n{}\n{}\n{}\n{}",
        "action send_doc\n  principal User\n  resource Doc\n  context\n    a    = sizes(amount: resource.amount).a\n    step = steps(amount: resource.amount).step\n",
        "action compare_doc\n  principal User\n  resource Doc\n  context\n    side = same_side(first: resource.amount, second: resource.amount).side\n",
        "action step_doc\n  principal User\n  resource Doc\n  context\n    step = steps(amount: resource.amount).step\n",
        "permit staff_send\n  principal in staff\n  action send_doc\n\npermit staff_compare\n  principal in staff\n  action compare_doc\n  when side is same\n\npermit staff_step_small\n  principal in staff\n  action step_doc\n  when step is small\n",
        "expect deny low_and_large_is_not_sent\n  principal in staff\n  action send_doc\n  when a is low and step is large\n\nexpect deny small_ones_over_100_are_not_sent\n  principal in staff\n  action send_doc\n  when resource.amount > 100JPY\n  when step is small\n\nexpect deny nothing_is_compared\n  principal in staff\n  action compare_doc\n\nexpect allow staff_step_every_doc\n  principal in staff\n  action step_doc\n",
    );
    let o = sekisho::check::check_text("tests/walk/inline.gate", &gate, &joined(), &Options::default());
    let diags = &o.diags;
    let codes: Vec<&str> = diags.iter().map(|d| d.code).collect();
    // `side` is never `apart`: the line of `staff_compare` that asks for `same` leaves out nothing
    assert_eq!(codes, vec!["W308", "E304", "E304", "E304", "E304"], "{}", rendered(diags));
    let line = |d: &Diag, lang: Lang| ritsu_base::text::spaced(&d.notes[0], lang);
    let counted = |d: &Diag, key: &str| d.extra.values.iter().find(|(k, _)| k == key).map(|(_, v)| v.en.clone());
    // the line in both languages; the amount of the combination; the values the input stands
    // beside, whether it is a region, and the amount it says
    type Want = (&'static str, &'static str, Option<&'static str>, &'static [&'static str], bool, &'static str);
    let wants: [Want; 4] = [
        (
            "For example: User holding staff, Doc (amount: 101JPY to 1000JPY), a: low, step: large (as when amount is 301JPY); allowed by `staff_send`.",
            "例：staff を持つ User、Doc（amount：101JPY〜1000JPY）、a：low、step：large（たとえば amount が 301JPY のとき）。`staff_send` が許します。",
            Some("101JPY to 1000JPY"),
            &["a", "step"],
            false,
            "301JPY",
        ),
        (
            "For example: User holding staff, Doc (amount: 101JPY to 1000JPY), a: low, step: small (whenever amount is 101JPY to 300JPY); allowed by `staff_send`.",
            "例：staff を持つ User、Doc（amount：101JPY〜1000JPY）、a：low、step：small（amount が 101JPY〜300JPY のときはいつも）。`staff_send` が許します。",
            Some("101JPY to 1000JPY"),
            &["a", "step"],
            true,
            "101JPY to 300JPY",
        ),
        (
            "For example: User holding staff, Doc, side: same (whenever amount is 1JPY to 500JPY); allowed by `staff_compare`.",
            "例：staff を持つ User、Doc、side：same（amount が 1JPY〜500JPY のときはいつも）。`staff_compare` が許します。",
            None,
            &["side"],
            true,
            "1JPY to 500JPY",
        ),
        (
            "For example: User holding staff, Doc, step: large (as when amount is 301JPY); denied: no permit applies.",
            "例：staff を持つ User、Doc、step：large（たとえば amount が 301JPY のとき）。どの permit も当てはまらないので拒まれます。",
            None,
            &["step"],
            false,
            "301JPY",
        ),
    ];
    // each computed value's rule, its output, and the inputs the amount is given as
    let rules: [(&str, &str, &[&str]); 3] = [("a", "tests/walk/together/sizes.rule", &["amount"]), ("step", "tests/walk/together/steps.rule", &["amount"]), ("side", "tests/walk/together/sides.rule", &["first", "second"])];
    let engine = rulec::ports::Engine::default();
    for (d, (en, ja, amount, computed, region, given)) in diags.iter().filter(|d| d.code == "E304").zip(wants) {
        assert_eq!(line(d, Lang::En), en, "{}", rendered(diags));
        assert_eq!(line(d, Lang::Ja), ja, "{}", rendered(diags));
        // the combination is the one counted: the cell a policy cuts, and nothing no policy reads
        assert_eq!(counted(d, "resource.amount").as_deref(), amount, "{en}");
        let [c] = d.extra.inputs.as_slice() else { panic!("{en}: {:?}", d.extra.inputs) };
        assert_eq!(c.computed, computed, "{en}");
        assert_eq!(c.region, region, "{en}");
        let [x] = c.given.as_slice() else { panic!("{en}: {:?}", c.given) };
        assert_eq!((x.name.as_str(), x.ty.as_deref(), x.value.en.as_str()), ("resource.amount", Some("Doc"), given), "{en}");
        // run with what is said, the rules come to the values it stands beside
        let sekisho::walk::Val::Cell(cell) = &x.val else { panic!("{en}: {:?}", x.val) };
        assert_eq!(cell.lo < cell.hi, region, "{en}");
        for yen in [cell.lo, cell.hi] {
            for name in computed {
                let (_, rule, inputs) = rules.iter().find(|(n, _, _)| n == name).unwrap();
                let given: Values = inputs.iter().map(|i| (i.to_string(), ritsu_ports::Value::Int(yen))).collect();
                let outs = ritsu_ports::Rules::eval(&engine, Path::new(rule), &given).unwrap_or_else(|e| panic!("{rule} at {yen}: {e:?}"));
                let got = outs.iter().find(|(n, _)| n == name).map(|(_, v)| v.clone());
                assert_eq!(got, counted(d, name).map(ritsu_ports::Value::Enum), "{en}: `{name}` at {yen}JPY");
            }
        }
    }
    let e304: Vec<Diag> = diags.iter().filter(|d| d.code == "E304").cloned().collect();
    ritsu_testkit::golden("tests/walk/golden/E304_inputs.txt", &rendered(&e304));
    // the JSON: the combination under `example` as it is counted, the inputs after it
    let keys = |d: &ritsu_base::json::Json| -> Vec<String> { d.as_obj().unwrap().iter().map(|(k, _)| k.clone()).collect() };
    let with_inputs = ["code", "severity", "file", "line", "col", "message", "notes", "example", "example_inputs", "fix"];
    for (lang, large, small) in [(Lang::En, "301JPY", "101JPY to 300JPY"), (Lang::Ja, "301JPY", "101JPY〜300JPY")] {
        let v = sekisho::check::to_json(&o, lang);
        let ds = v.get("diagnostics").and_then(|x| x.as_arr()).unwrap();
        for (d, (kind, value)) in ds.iter().filter(|d| d.get("code").and_then(|x| x.as_str()) == Some("E304")).zip([("point", large), ("region", small)]) {
            assert_eq!(keys(d), with_inputs);
            assert_eq!(d.get("example").and_then(|e| e.get("resource.amount")).and_then(|x| x.as_str()), Some(if lang == Lang::En { "101JPY to 1000JPY" } else { "101JPY〜1000JPY" }));
            assert_eq!(d.get("example_inputs").map(|x| x.compact()), Some(format!(r#"[{{"computed":["a","step"],"kind":"{kind}","given":[{{"name":"resource.amount","type":"Doc","value":"{value}"}}]}}]"#)));
        }
    }
    // no input is run without the evaluator: the example is the combination alone, as the walk
    // counts it, and the JSON has the keys it has for any diagnostic
    let mut suite = joined();
    suite.rules = Some(Rc::new(NoEval(rulec::ports::Engine::default())));
    let bare = sekisho::check::check_text("tests/walk/inline.gate", &gate, &suite, &Options::default());
    let e304: Vec<&Diag> = bare.diags.iter().filter(|d| d.code == "E304").collect();
    let said: Vec<String> = e304.iter().map(|d| line(d, Lang::En)).collect();
    assert!(said.iter().any(|x| x == "For example: User holding staff, Doc, step: large; denied: no permit applies."), "{said:?}");
    assert!(said.iter().all(|x| !x.contains("(as when") && !x.contains("(whenever")), "{said:?}");
    assert!(e304.iter().all(|d| d.extra.inputs.is_empty()));
    let v = sekisho::check::to_json(&bare, Lang::En);
    for d in v.get("diagnostics").and_then(|x| x.as_arr()).unwrap() {
        assert_eq!(keys(d), ["code", "severity", "file", "line", "col", "message", "notes", "example", "fix"]);
    }
}

/// Lines met on no combination because of what the rules compute say so (DESIGN 4.2): `a` is
/// high over 500 yen and `step` is small up to 300, each on some amount and never on one. The
/// permit that asks for both applies to nothing (E303), and the expectation that picks both picks
/// nothing (W304); beside the counts of the lines, each says which values never come together
/// and what they are computed from, and how to mend it: the lines of a permit to values the rules
/// give together, and for an expectation, that the rules decide it whatever the policies say. A
/// value and the part of an attribute it is computed from that it never comes with are said the
/// same way. Lines that are met nowhere for another reason (a line and its own opposite) keep the
/// plain note.
#[test]
fn lines_met_nowhere_because_of_the_rules_say_which_values_never_come_together() {
    let gate = format!(
        "gate apart v1\n\n{HEAD}\n{}\n{}\n{}",
        "action send_doc\n  principal User\n  resource Doc\n  context\n    a    = sizes(amount: resource.amount).a\n    step = steps(amount: resource.amount).step\n",
        "permit staff_send\n  principal in staff\n  action send_doc\n\npermit high_and_small\n  principal in staff\n  action send_doc\n  when a is high and step is small\n\npermit low_and_dear\n  principal in staff\n  action send_doc\n  when a is low\n  when resource.amount > 600JPY\n\nforbid never_met\n  principal in staff\n  action send_doc\n  when a is low\n  unless a is low\n",
        "expect deny high_small_is_not_sent\n  action send_doc\n  when a is high\n  when step is small\n",
    );
    let (diags, _) = check_src(&gate);
    let codes: Vec<&str> = diags.iter().map(|d| d.code).collect();
    assert_eq!(codes, vec!["E303", "E303", "E303", "W304"], "{}", rendered(&diags));
    let notes = |d: &Diag, lang: Lang| -> Vec<String> { d.notes.iter().map(|t| t.get(lang).to_string()).collect() };
    // the permit: the two values, what both are computed from, and the lines to mend
    assert_eq!(notes(&diags[0], Lang::En)[1..], ["`a` is never `high` while `step` is `small`: both are computed from `resource.amount`.", "Correct the lines to values the rules give together, or remove the permit itself."], "{}", rendered(&diags));
    assert_eq!(notes(&diags[0], Lang::Ja)[1..], ["`a` が `high` で、`step` が `small` になることはありません。どちらも `resource.amount` から計算する値です。", "規則が一緒に出す値に行を直すか、permit そのものを消してください。"], "{}", rendered(&diags));
    // a value and the part of the attribute it is computed from
    assert_eq!(notes(&diags[1], Lang::En)[1], "`a` is never `low` while `resource.amount` is 601JPY to 1000JPY: `a` is computed from `resource.amount`.", "{}", rendered(&diags));
    assert_eq!(notes(&diags[1], Lang::Ja)[1], "`a` が `low` で、`resource.amount` が 601JPY〜1000JPY になることはありません。`a` は `resource.amount` から計算する値です。", "{}", rendered(&diags));
    // a line and its own opposite: the rules are not why
    assert_eq!(notes(&diags[2], Lang::En)[1..], ["Remove the line that cannot be met, or the forbid itself."], "{}", rendered(&diags));
    // the expectation: the same values, and whose it is to decide
    assert_eq!(notes(&diags[3], Lang::En)[1], "`a` is never `high` while `step` is `small`: both are computed from `resource.amount`.", "{}", rendered(&diags));
    assert!(notes(&diags[3], Lang::En)[2].starts_with("The rules decide this whatever the policies say"), "{}", rendered(&diags));
    assert!(notes(&diags[3], Lang::Ja)[2].starts_with("これは規則が決めていることで"), "{}", rendered(&diags));
    ritsu_testkit::golden("tests/walk/golden/E303_W304_apart.txt", &rendered(&diags));
}

/// A value a rule never answers for what the gate gives it is said so (DESIGN 4.2): one amount
/// given as both inputs of a rule that asks whether two amounts are on one side of 500 yen is
/// never `apart`; an amount the gate holds to 300 yen is never over 500; and a call given
/// constants comes to one answer. The permit that asks for such a value applies to nothing
/// (E303), and the expectation that picks it picks nothing (W304); each says the value, what it
/// is computed from (and that one thing is given as two inputs), and how to mend it.
#[test]
fn a_value_a_rule_never_answers_is_said_with_what_it_is_computed_from() {
    let gate = format!(
        "gate never v1\n\n{HEAD}\n{}\n{}\n{}\n{}\n{}",
        "resource Note\n  attributes\n    amount : money[JPY]  range >=1JPY <=300JPY\n",
        "action compare_doc\n  principal User\n  resource Doc\n  context\n    side = same_side(first: resource.amount, second: resource.amount).side\n",
        "action read_note\n  principal User\n  resource Note\n  context\n    a = sizes(amount: resource.amount).a\n\naction fixed_doc\n  principal User\n  resource Doc\n  context\n    far = same_side(first: 100JPY, second: 700JPY).side\n",
        "permit staff_compare\n  principal in staff\n  action compare_doc\n\npermit apart_side\n  principal in staff\n  action compare_doc\n  when side is apart\n\npermit staff_note\n  principal in staff\n  action read_note\n\npermit high_note\n  principal in staff\n  action read_note\n  when a is high\n\npermit staff_fixed\n  principal in staff\n  action fixed_doc\n\npermit same_fixed\n  principal in staff\n  action fixed_doc\n  when far is same\n",
        "expect deny never_apart\n  action compare_doc\n  when side is apart\n",
    );
    let (diags, _) = check_src(&gate);
    let codes: Vec<&str> = diags.iter().map(|d| d.code).collect();
    assert_eq!(codes, vec!["E303", "E303", "E303", "W304"], "{}", rendered(&diags));
    let notes = |d: &Diag, lang: Lang| -> Vec<String> { d.notes.iter().map(|t| t.get(lang).to_string()).collect() };
    // one thing given as two inputs
    assert_eq!(
        notes(&diags[0], Lang::En)[1..],
        [
            "`side` is never `apart`: it is computed from `resource.amount` (given as both `first` and `second`), and the rule answers `apart` for none of the values the gate gives it.",
            "Correct the lines to a value the rule gives, or remove the permit itself."
        ],
        "{}",
        rendered(&diags)
    );
    assert_eq!(
        notes(&diags[0], Lang::Ja)[1..],
        ["`side` が `apart` になることはありません。`side` は `first` と `second` の両方に渡す `resource.amount` から計算する値で、ゲートが渡す値のどれにも、規則は `apart` を答えません。", "規則が出す値に行を直すか、permit そのものを消してください。"],
        "{}",
        rendered(&diags)
    );
    // an attribute the gate holds to less than the rule takes
    assert_eq!(notes(&diags[1], Lang::En)[1], "`a` is never `high`: it is computed from `resource.amount`, and the rule answers `high` for none of the values the gate gives it.", "{}", rendered(&diags));
    // constants
    assert_eq!(notes(&diags[2], Lang::En)[1], "`far` is never `same`: the rule answers `same` for none of the values the gate gives it.", "{}", rendered(&diags));
    assert_eq!(notes(&diags[2], Lang::Ja)[1], "`far` が `same` になることはありません。ゲートが渡す値のどれにも、規則は `same` を答えません。", "{}", rendered(&diags));
    // the expectation: the same value, and whose it is to decide
    assert_eq!(notes(&diags[3], Lang::En)[1], notes(&diags[0], Lang::En)[1], "{}", rendered(&diags));
    assert!(notes(&diags[3], Lang::En)[2].starts_with("The rules decide this whatever the policies say"), "{}", rendered(&diags));
    ritsu_testkit::golden("tests/walk/golden/E303_W304_never.txt", &rendered(&diags));
}
