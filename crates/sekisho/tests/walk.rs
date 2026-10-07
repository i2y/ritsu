//! The checks of every combination (PLAN A3), with rulec, koyomi, chobo and dandori joined as
//! `ritsu sekisho` joins them: the example's two versions pass and come to the same counts, the
//! table of `refund_order` merges to its five rows that allow, relations are counted by which terms
//! are one entity, the budget stops a walk (E307), an answer that rests on a value no language
//! vouches for is undecided (W303), an enum and a bool given to a rule are held to their values,
//! and how far a workflow and a role are allowed an action (the port `Gates::allowed`). The
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
    fn preconditions_hold(&self, rule: &Path, ranges: &[(String, Option<i128>, Option<i128>)], max_len: Option<i128>) -> Result<Vec<(ritsu_ports::Precondition, ritsu_ports::Answer<Values>)>, Vec<Said>> {
        self.0.preconditions_hold(rule, ranges, max_len)
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
    // today is always before three days after itself: one truth, one combination
    let gate = "gate t v1\n\nuse dates plus from \"plus.cal\"\n\ntoday range >=2026-03-01 <=2026-03-31 offset +00:00\n\nprincipal User\n\nresource Doc\n\naction read_doc\n  principal User\n  resource Doc\n  context\n    early = today < plus.later(d: today)\n\npermit early_reads\n  principal is User\n  action read_doc\n  when early\n";
    let (diags, c) = check_with_dates(gate, PLUS);
    assert!(diags.is_empty(), "{}", rendered(&diags));
    assert_eq!((c.report.actions[0].combinations, c.report.actions[0].allowed), (1, 1));
    // a permit that asks for it not to hold never applies
    let (diags, _) = check_with_dates(&format!("{gate}\npermit late_reads\n  principal is User\n  action read_doc\n  unless early\n"), PLUS);
    assert_eq!(diags.iter().map(|d| d.code).collect::<Vec<_>>(), vec!["E303"], "{}", rendered(&diags));
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
    fn preconditions_hold(&self, rule: &Path, ranges: &[(String, Option<i128>, Option<i128>)], max_len: Option<i128>) -> Result<Vec<(ritsu_ports::Precondition, ritsu_ports::Answer<Values>)>, Vec<Said>> {
        self.0.preconditions_hold(rule, ranges, max_len)
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
