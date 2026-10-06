//! What the names come to beyond the mutants (DESIGN 2, 3.1–3.3): what passes as well as what does
//! not — the files read with `use gate`, their forbids for every action, the relations, `Workflow`,
//! a policy over two actions, and the values given to a rule.

mod common;

use ritsu_base::text::Lang;
use sekisho::check::{Options, Outcome, check_text};

/// A file beside the example (so that `rules/…` and `dates/…` are the example's), checked with
/// every language joined.
fn checked(src: &str) -> Outcome {
    check_text("examples/refunds/t.gate", src, &common::joined(), &Options::default())
}

/// Whether a code is one of the names' (E0xx, E1xx, E201, E209): what the checks after the names
/// say of a file whose names hold (the borders, the contracts, every combination) is for other tests.
fn of_names(code: &str) -> bool {
    code.starts_with("E0") || code.starts_with("E1") || code == "E201" || code == "E209" || code == "E210" || code == "E211"
}

fn codes(src: &str) -> Vec<&'static str> {
    let o = checked(src);
    o.diags.iter().map(|d| d.code).filter(|c| of_names(c)).collect()
}

fn passes(src: &str) {
    let o = checked(src);
    let said: String = o.diags.iter().filter(|d| of_names(d.code)).map(|d| d.render(Lang::En)).collect();
    assert!(said.is_empty(), "{said}");
    assert!(o.scope.is_some());
}

const SHOP: &str = "gate t v1\n\nenum department = sales | support\n\nrole clerk\nrole auditor\n\nprincipal User\n  roles clerk, auditor\n  attributes\n    suspended  : bool\n    department : department\n    tenant     : Tenant\n\nprincipal Customer\n\nworkflow returns from \"flows/returns.flow\"\n\nresource Tenant\n\nresource Team\n\nresource Order\n  attributes\n    department : department\n    tenant     : Tenant\n    team       : Team\n    customer   : Customer\n    note       : bool?\n\n";

#[test]
fn relations_of_one_step_pass() {
    passes(&format!(
        "{SHOP}action view\n  principal User, Customer, Workflow\n  resource Order\n\npermit same_tenant\n  principal is User\n  action view\n  when resource.tenant is principal.tenant\n  when principal.department is resource.department\n  when principal in resource.team\n\npermit owners\n  principal is Customer\n  action view\n  when resource.customer is principal\n\npermit the_workflow\n  principal is workflow returns\n  action view\n"
    ));
}

#[test]
fn a_relation_between_types_that_differ() {
    assert_eq!(codes(&format!("{SHOP}action view\n  principal User\n  resource Order\n\npermit p\n  principal is User\n  action view\n  when resource.customer is principal\n")), vec!["E104"]);
    assert_eq!(codes(&format!("{SHOP}action view\n  principal User\n  resource Order\n\npermit p\n  principal is User\n  action view\n  when resource.team is principal.tenant\n")), vec!["E104"]);
}

#[test]
fn workflow_needs_a_workflow_line() {
    let src = "gate t v1\n\nprincipal User\n\nresource Order\n\naction view\n  principal User, Workflow\n  resource Order\n";
    assert_eq!(codes(src), vec!["E101"]);
}

#[test]
fn the_principal_line_against_the_actions() {
    let base = format!("{SHOP}action view\n  principal User, Customer\n  resource Order\n\naction refund\n  principal User\n  resource Order\n\n");
    // one of the two actions never takes a Customer
    assert_eq!(codes(&format!("{base}permit p\n  principal is Customer\n  action view, refund\n")), vec!["E106"]);
    // `action any`: some action takes one
    assert!(codes(&format!("{base}forbid p\n  principal is Customer\n  action any\n")).is_empty());
    // a role no principal type of the action holds
    let roles = "gate t v1\n\nrole clerk\nrole manager\n\nprincipal User\n  roles clerk\n\nprincipal Admin\n  roles manager\n\nresource Order\n\naction refund\n  principal User\n  resource Order\n\n";
    assert_eq!(codes(&format!("{roles}permit p\n  principal in manager\n  action refund\n")), vec!["E106"]);
    assert!(codes(&format!("{roles}permit p\n  principal in clerk, manager\n  action refund\n")).is_empty());
}

#[test]
fn a_bare_name_is_an_input_or_a_computed_value_of_every_action_picked() {
    let src = format!("{SHOP}action view\n  principal User\n  resource Order\n\naction refund\n  principal User\n  resource Order\n  input\n    amount : money[GBP, incl_tax]  range >=1GBP <=100GBP\n\npermit p\n  action view, refund\n  when amount <= 50GBP\n");
    assert_eq!(codes(&src), vec!["E101"]);
}

#[test]
fn what_a_condition_cannot_read() {
    let base = format!("{SHOP}action view\n  principal User\n  resource Order\n  input\n    on : date  range >=2026-01-01 <=2026-12-31\n    n  : number  range >=0 <=10\n\n");
    for (cond, code) in [("on is 2026-01-01", "E102"), ("principal.department", "E102"), ("principal.department < 3", "E102"), ("n is sales", "E102"), ("n <= 11", "E103"), ("resource.note", "")] {
        let got = codes(&format!("{base}permit p\n  principal is User\n  action view\n  when {cond}\n"));
        let want: Vec<&str> = if code.is_empty() { vec![] } else { vec![code] };
        assert_eq!(got, want, "{cond}");
    }
    // an input that is an entity, and types of other languages
    assert_eq!(codes(&format!("{SHOP}action view\n  principal User\n  resource Order\n  input\n    who : Customer\n")), vec!["E102"]);
    for ty in ["string", "Long", "money"] {
        let o = checked(&format!("{SHOP}action view\n  principal User\n  resource Order\n  input\n    x : {ty}\n"));
        assert_eq!(o.diags.iter().map(|d| d.code).collect::<Vec<_>>(), vec!["E101"], "{ty}");
        assert_eq!(o.diags[0].notes.len(), 1, "{ty}: a note says what to write");
    }
}

#[test]
fn the_values_given_to_a_rule() {
    let head = "gate t v1\n\nuse rule refund_limit from \"rules/refund_limit.rule\"\n\nprincipal User\n  attributes\n    limit : money[GBP, incl_tax]  range >=0GBP <=10_000GBP\n\nresource Order\n\naction refund\n  principal User\n  resource Order\n  input\n    amount : money[GBP, incl_tax]  range >=1GBP <=10_000GBP\n  context\n";
    passes(&format!("{head}    band = refund_limit(amount: amount, limit: principal.limit).band\n"));
    passes(&format!("{head}    band = refund_limit(amount: 50GBP, limit: principal.limit).band\n"));
    // an input left out, one given twice, one the rule does not have
    assert_eq!(codes(&format!("{head}    band = refund_limit(amount: amount).band\n")), vec!["E105"]);
    assert_eq!(codes(&format!("{head}    band = refund_limit(amount: amount, amount: amount, limit: principal.limit).band\n")), vec!["E006"]);
    assert_eq!(codes(&format!("{head}    band = refund_limit(amount: amount, limit: principal.limit, extra: 1GBP).band\n")), vec!["E101"]);
    // a value of the rule's enum compared, by its alias and by a value the rule does not have
    let cond = |v: &str| format!("{head}    band = refund_limit(amount: amount, limit: principal.limit).band\n\npermit p\n  action refund\n  when band is {v}\n");
    passes(&cond("within_limit"));
    assert_eq!(codes(&cond("under_limit")), vec!["E101"]);
}

#[test]
fn the_dates_and_today() {
    let head = "gate t v1\n\nuse dates refund_terms from \"dates/refund_terms.cal\"\n\ntoday range >=2026-10-01 <=2028-10-31 offset +00:00\n\nprincipal User\n\nresource Order\n  attributes\n    paid_on : date  range >=2026-01-01 <=2028-09-30\n\naction refund\n  principal User\n  resource Order\n  context\n";
    passes(&format!("{head}    in_period = today <= refund_terms.last_day(paid_on: resource.paid_on)\n"));
    passes(&format!("{head}    in_period = today <= resource.paid_on\n"));
    assert_eq!(codes(&format!("{head}    in_period = today <= refund_terms.first_day(paid_on: resource.paid_on)\n")), vec!["E101"]);
    assert_eq!(codes(&format!("{head}    in_period = today <= refund_terms.last_day(paid_on: today)\n")), Vec::<&str>::new());
    let backwards = head.replace("today range >=2026-10-01 <=2028-10-31 offset +00:00", "today range >=2028-10-01 <=2026-10-31 offset +25:00");
    assert_eq!(codes(&format!("{backwards}    in_period = today <= resource.paid_on\n")), vec!["E103", "E107"]);
}

/// A circle of `includes` is said once, with its roles in order, whatever its length.
#[test]
fn a_circle_of_roles_is_said_once() {
    for (src, shown) in [
        ("gate t v1\n\nrole a\n  includes b\nrole b\n  includes c\nrole c\n  includes a\nrole d\n  includes a\n", "a → b → c → a"),
        ("gate t v1\n\nrole a\n  includes a\n", "a → a"),
        ("gate t v1\n\nrole 係(clerk)\n  includes 責任者\nrole 責任者(manager)\n  includes 係\n", "係 → 責任者 → 係"),
    ] {
        let o = checked(src);
        assert_eq!(o.diags.iter().map(|d| d.code).collect::<Vec<_>>(), vec!["E108"], "{src}");
        assert!(o.diags[0].message.en.ends_with(shown), "{}", o.diags[0].message.en);
    }
}

/// What is wrong with a type is said once, where the type is written, not again where it is read.
#[test]
fn a_wrong_type_is_said_once() {
    let src = "gate t v1\n\nprincipal User\n  attributes\n    limit : money[GBX, incl_tax]  range >=0GBP <=10GBP\n\nresource Order\n\naction refund\n  principal User\n  resource Order\n  input\n    amount : money[GBP, incl_tax]\n\npermit p\n  action refund\n  when principal.limit is 5GBP\n  when amount <= 5GBP\n";
    assert_eq!(codes(src), vec!["E101", "E103"]);
}

#[test]
fn names_cedar_and_the_targets_cannot_take() {
    assert_eq!(codes("gate t v1\nnamespace 店\n"), vec!["E007"]);
    assert_eq!(codes("gate t v1\n\nrole if\n"), vec!["E008"]);
    assert_eq!(codes("gate t v1\n\nrole 役(class)\n"), vec!["E008"]);
    assert_eq!(codes("gate t v1\n\nprincipal 職員(user)\n"), vec!["E007"]);
    assert!(codes("gate t v1\n\nrole open\nrole description\n").is_empty());
}

/// A `.gate` read with `use gate`: its declarations are this file's to use, its forbids for every
/// action hold for this file's actions, and what it declares cannot be declared again.
#[test]
fn a_gate_read_with_use_gate() {
    let dir = ritsu_testkit::TempDir::new("use-gate");
    let people = "gate people v1\nnamespace Shop\n\nrole clerk\n\nprincipal User\n  roles clerk\n  attributes\n    suspended : bool\n\nresource Placeholder\n\naction nothing\n  principal User\n  resource Placeholder\n  nobody \"only here to hold the forbid\"\n\nforbid suspended_do_nothing\n  principal is User\n  action any\n  when principal.suspended\n";
    std::fs::write(dir.path().join("people.gate"), people).unwrap();
    let path = dir.path().join("orders.gate");
    let run = |src: &str| {
        std::fs::write(&path, src).unwrap();
        sekisho::check::check_file(path.to_str().unwrap(), &common::joined(), &Options::default()).unwrap()
    };
    let o = run("gate orders v1\nnamespace Shop\n\nuse gate \"people.gate\"\n\nresource Order\n\naction view\n  principal User\n  resource Order\n\npermit clerks_view\n  principal in clerk\n  action view\n");
    assert!(!o.diags.iter().any(|d| of_names(d.code)), "{}", sekisho::check::render(&o, Lang::En));
    let s = o.scope.unwrap();
    assert_eq!(s.files.len(), 2);
    assert_eq!(s.imported_forbids().len(), 1);
    // a declaration of the file read, declared again
    let o = run("gate orders v1\nnamespace Shop\n\nuse gate \"people.gate\"\n\nrole clerk\n");
    assert_eq!(o.diags.iter().map(|d| d.code).collect::<Vec<_>>(), vec!["E006"]);
    assert!(o.diags[0].render(Lang::En).contains("people.gate"));
    // the forbid of the file read holds for no action here (none takes a User): nothing is said
    let o = run("gate orders v1\nnamespace Shop\n\nuse gate \"people.gate\"\n\nprincipal Robot\n\nresource Order\n\naction view\n  principal Robot\n  resource Order\n");
    assert!(!o.diags.iter().any(|d| of_names(d.code)), "{}", sekisho::check::render(&o, Lang::En));
    // the file read reads a rule, and this sekisho holds no rulec: E209, as for this file
    std::fs::write(dir.path().join("people.gate"), "gate people v1\nnamespace Shop\n\nuse rule limit from \"limit.rule\"\n").unwrap();
    std::fs::write(&path, "gate orders v1\nnamespace Shop\n\nuse gate \"people.gate\"\n").unwrap();
    let o = sekisho::check::check_file(path.to_str().unwrap(), &sekisho::suite::Suite::default(), &Options::default()).unwrap();
    assert_eq!((o.diags.iter().map(|d| d.code).collect::<Vec<_>>(), o.exit()), (vec!["E209"], 2));
    // what the file read says comes with its code first and its place after it
    let said = sekisho::check::render(&o, Lang::En);
    assert!(said.contains("  = E209: This sekisho cannot read rules (rulec is not in it), and the file reads `limit` ("), "{said}");
    assert!(said.contains("/people.gate:4:1)\n"), "{said}");
    // the file read is in another namespace (E210)
    std::fs::write(dir.path().join("people.gate"), people).unwrap();
    let o = run("gate orders v1\nnamespace Orders\n\nuse gate \"people.gate\"\n");
    assert_eq!(o.diags.iter().map(|d| d.code).collect::<Vec<_>>(), vec!["E210"]);
    // two files read declare one role, one by the alias of the other's name (E211); the same file
    // read by two ways is one file
    std::fs::write(dir.path().join("staff.gate"), "gate staff v1\nnamespace Shop\n\nrole 係(clerk)\n").unwrap();
    std::fs::write(dir.path().join("both.gate"), "gate both v1\nnamespace Shop\n\nuse gate \"people.gate\"\n").unwrap();
    let o = run("gate orders v1\nnamespace Shop\n\nuse gate \"people.gate\"\nuse gate \"both.gate\"\nuse gate \"staff.gate\"\n");
    let said = sekisho::check::render(&o, Lang::En);
    assert_eq!(o.diags.iter().map(|d| d.code).collect::<Vec<_>>(), vec!["E211"], "{said}");
    assert_eq!(o.diags[0].line, Some(6), "{said}");
    assert!(said.contains("staff.gate") && said.contains("people.gate") && said.contains("role `係`"), "{said}");
    // two files that read each other
    std::fs::write(dir.path().join("people.gate"), "gate people v1\nnamespace Shop\n\nuse gate \"orders.gate\"\n").unwrap();
    let o = run("gate orders v1\nnamespace Shop\n\nuse gate \"people.gate\"\n");
    assert_eq!(o.diags.iter().map(|d| d.code).collect::<Vec<_>>(), vec!["E201"]);
}

/// What the scope says of the example, for the checks of every combination.
#[test]
fn the_scope_of_the_example() {
    let o = common::check("examples/refunds/refunds.gate");
    let s = o.scope.expect("the example passes");
    let f = s.file();
    let refund = f.action("refund_order").unwrap();
    let band = refund.computed("refund_band").unwrap();
    // the rule's output, as rulec's port names its values (the alias is the member of the generated
    // code), and who it is computed for: not a workflow, which has no refund_limit
    assert_eq!(s.domain(band), sekisho::names::Domain::Enum(vec![("within_limit".into(), "WithinLimit".into()), ("over_limit".into(), "OverLimit".into())]));
    assert_eq!(s.computed_for(refund, band), (vec!["User".to_string()], vec!["Order".to_string()]));
    let read = s.rule_of(band).unwrap();
    assert!(read.path.ends_with("rules/refund_limit.rule"));
    assert_eq!(sekisho::names::rule_input(&read.facts, "limit").map(|c| c.name.as_str()), Some("limit"));
    let in_period = refund.computed("in_period").unwrap();
    assert_eq!(s.computed_for(refund, in_period), (vec!["User".to_string(), "Workflow".to_string()], vec!["Order".to_string()]));
    assert!(s.dates_of(in_period).is_some());
    // the types of the fields, on the wire
    let amount = refund.input("amount").unwrap();
    match s.field_type(amount) {
        sekisho::names::FieldType::Number { lo, hi, .. } => assert_eq!((lo, hi), (1, 10_000)),
        other => panic!("{other:?}"),
    }
    assert_eq!(s.held("manager").into_iter().collect::<Vec<_>>(), vec!["clerk".to_string(), "manager".to_string()]);
    assert_eq!(s.namespace(), "Shop");
    let (lo, hi, offset) = s.today().unwrap();
    assert_eq!((sekisho::types::day_text(lo), sekisho::types::day_text(hi), offset), ("2026-10-01".to_string(), "2028-10-31".to_string(), 0));
    // the Japanese version: the same, by its aliases
    let ja = common::check("examples/refunds/refunds.ja.gate").scope.expect("the Japanese version passes");
    assert_eq!(ja.namespace(), "ShopJa");
    let refund = ja.file().action("refund_order").unwrap();
    let band = refund.computed("refund_band").unwrap();
    assert_eq!(ja.domain(band), sekisho::names::Domain::Enum(vec![("上限まで".into(), "WithinLimit".into()), ("上限超え".into(), "OverLimit".into())]));
    assert_eq!(ja.computed_for(refund, band), (vec!["User".to_string()], vec!["Order".to_string()]));
}

/// A value of a rule's enum can be named three ways: by the rule's name for it, by the member of
/// the generated code, and by the alias the `.rule` writes (the string Cedar is given), so the
/// Japanese version of the example reads the same with `within_limit` as with `上限まで`.
#[test]
fn a_rule_value_is_named_by_its_name_its_member_or_its_alias() {
    let ja = std::fs::read_to_string("examples/refunds/refunds.ja.gate").unwrap();
    let by_name = "is 上限まで";
    assert!(ja.contains(by_name), "the Japanese example names the value by its name");
    for word in ["上限まで", "WithinLimit", "within_limit"] {
        let o = check_text("examples/refunds/t.ja.gate", &ja.replace(by_name, &format!("is {word}")), &common::joined(), &Options::default());
        let said: String = o.diags.iter().filter(|d| of_names(d.code)).map(|d| d.render(Lang::En)).collect();
        assert!(said.is_empty(), "{word}: {said}");
    }
}
