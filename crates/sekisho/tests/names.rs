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

/// A `rate` with no step counts 100% as 1 (DESIGN 19): a constant or an end of a range that is no
/// whole number of 100% is E103, and its note gives the step that makes it one; with that step
/// written on the type, the file passes.
#[test]
fn a_rate_with_no_step_is_told_to_write_one() {
    let gate = |ty: &str, range: &str, when: &str| {
        format!("gate t v1\n\nprincipal User\n\nresource Order\n  attributes\n    discount : {ty}  range {range}\n\naction view\n  principal User\n  resource Order\n\npermit small_discounts\n  principal is User\n  action view\n  when resource.discount <= {when}\n")
    };
    // the ends 0% and 100%, and a constant of 100%, are whole numbers of 100%
    passes(&gate("rate", ">=0% <=100%", "100%"));
    // the constant a condition compares, named by the condition's left side, and the end of a range,
    // named by the attribute
    for (range, when, step, at, named) in [(">=0% <=100%", "30%", "1%", "30%", "`resource.discount`"), (">=0% <=100%", "12.5%", "0.1%", "12.5%", "`resource.discount`"), (">=0% <=30%", "0%", "1%", "30%", "`discount`")] {
        let o = checked(&gate("rate", range, when));
        let ds: Vec<_> = o.diags.iter().filter(|d| of_names(d.code)).collect();
        assert_eq!(ds.iter().map(|d| d.code).collect::<Vec<_>>(), ["E103"], "{range} {when}");
        let d = ds[0];
        assert!(d.message.en.contains(&format!("The constant `{at}`, counted in rate, is not a whole number")), "{}", d.message.en);
        for lang in [Lang::En, Lang::Ja] {
            assert!(d.notes.iter().any(|n| n.get(lang).contains(&format!("`rate[step {step}]`")) && n.get(lang).contains(named)), "{lang:?}: {:?}", d.notes);
        }
        passes(&gate(&format!("rate[step {step}]"), range, when));
    }
    // a rate with a step says nothing of the step to write
    let o = checked(&gate("rate[step 1%]", ">=0% <=100%", "12.5%"));
    let d = o.diags.iter().find(|d| d.code == "E103").unwrap();
    assert_eq!(d.notes.len(), 1, "{:?}", d.notes);
}

/// A rule whose names a `.gate` cannot all write as the rule spells them: an apostrophe in a name
/// or in an alias, a value with no alias, a value named like a word of sekisho's conditions
/// (`today`, `in`, `any`). Its enum input comes first and its inputs of `bool` after it; its enum
/// output comes first.
const RULE_OF_HARD_NAMES: &str = "rule t v1

enum section = Home | Men's(mens) | today | in(in_store) | Children's | Ma'am(ma'am)
enum desk_kind = floor | any | Customer's(customers) | Staff's

inputs
  sold_in                       : section
  seller's_desk(desk_of_seller) : bool
  in                            : bool

outputs
  desk                           : desk_kind
  refunder's_desk(refunder_desk) : bool
  staff's_note(staff's)          : bool

table t
policy unique
| sold_in    | seller's_desk | in | -> desk    | refunder's_desk | staff's_note |
| Home       | -             | -  | floor      | true            | true         |
| Men's      | -             | -  | any        | true            | true         |
| today      | -             | -  | Customer's | true            | false        |
| in         | -             | -  | Staff's    | false           | false        |
| Children's | -             | -  | floor      | false           | false        |
| Ma'am      | -             | -  | floor      | false           | true         |
";

/// The Japanese version: names with `＆`, `＋` and full-width parentheses, which a `.gate` does not
/// write in a name. No Japanese name is a word of sekisho's conditions, so that case is the English
/// version's alone.
const RULE_OF_HARD_NAMES_JA: &str = "rule 窓口(t) v1

enum 売り場(section) = 紳士服(mens) | ホーム＆ガーデン(home) | ベビー＆キッズ | 松＋竹(松＋竹)
enum 窓口の種類(desk_kind) = 売り場の窓口(floor) | サービス＆カウンター(service) | 係（応援）

inputs
  売った売り場(sold_in)  : 売り場
  売った人（係）(seller) : bool

outputs
  窓口(desk)                 : 窓口の種類
  返金する人（係）(refunder) : bool
  メモ（係）(メモ（係）)     : bool

table 表(t)
policy unique
| 売った売り場     | 売った人（係） | -> 窓口              | 返金する人（係） | メモ（係） |
| 紳士服           | -              | 売り場の窓口         | true             | true       |
| ホーム＆ガーデン | -              | サービス＆カウンター | true             | true       |
| ベビー＆キッズ   | -              | 係（応援）           | false            | true       |
| 松＋竹           | -              | 売り場の窓口         | false            | false      |
";

/// A gate that reads the rule `name` from `t.rule` beside it and computes `x = <call>` for its one
/// action, with a permit that compares `x is <is>` when `is` is given.
fn gate_calling(name: &str, call: &str, is: Option<&str>) -> String {
    let permit = is.map(|v| format!("\npermit p\n  principal is User\n  action refund\n  when x is {v}\n")).unwrap_or_default();
    format!("gate g v1\n\nuse rule {name} from \"t.rule\"\n\nprincipal User\n\nresource Sale\n\naction refund\n  principal User\n  resource Sale\n  context\n    x = {call}\n{permit}")
}

/// What the notes of an E101 list, in `lang`: the names in backquotes in the note of the names there
/// are, and the names the other note says cannot be written, before it says so.
fn listed_in(notes: &[ritsu_base::text::Text], lang: Lang) -> (Vec<String>, Vec<String>) {
    let quoted = |s: &str| s.split('`').skip(1).step_by(2).map(str::to_string).collect::<Vec<_>>();
    let stop = if lang == Lang::Ja { " は、" } else { " cannot be written" };
    let (mut there, mut unwritten) = (Vec::new(), Vec::new());
    for n in notes {
        match n.get(lang).split_once(stop) {
            Some((names, _)) => unwritten = quoted(names),
            None => there = quoted(n.get(lang)),
        }
    }
    (there, unwritten)
}

/// The notes of E101 list the names of a rule as a `.gate` writes them where they go (DESIGN 20), in
/// the four places a `.gate` writes them: a value given to the rule's enum input, a value of its enum
/// output on the right of `is`, its inputs, and its outputs. `want` is, for each place in that order,
/// the names the note lists and those it says cannot be written. Each name listed, written where it is
/// listed, is the name it stands for: the names of the file hold.
fn rule_names_listed_as_written(name: &str, rule: &str, want: [(&[&str], &[&str]); 4]) {
    let dir = ritsu_testkit::TempDir::new("e101-names");
    std::fs::write(dir.path().join("t.rule"), rule).unwrap();
    let path = dir.path().join("g.gate");
    let said = |call: &str, is: Option<&str>| {
        std::fs::write(&path, gate_calling(name, call, is)).unwrap();
        let o = sekisho::check::check_file(path.to_str().unwrap(), &common::joined(), &Options::default()).unwrap();
        o.diags.into_iter().filter(|d| of_names(d.code)).map(|d| (d.code, d.notes.clone(), d.render(Lang::En))).collect::<Vec<_>>()
    };
    // the notes of the one E101, which list the same names in both languages
    let lists = |ds: &[(&str, Vec<ritsu_base::text::Text>, String)]| {
        let e101: Vec<_> = ds.iter().filter(|(c, ..)| *c == "E101").collect();
        assert_eq!(e101.len(), 1, "{ds:?}");
        let (en, ja) = (listed_in(&e101[0].1, Lang::En), listed_in(&e101[0].1, Lang::Ja));
        assert_eq!(en, ja, "{}", e101[0].2);
        en
    };
    let owned = |(there, unwritten): (&[&str], &[&str])| (there.iter().map(|s| s.to_string()).collect::<Vec<_>>(), unwritten.iter().map(|s| s.to_string()).collect::<Vec<_>>());
    let [values, outs, inputs, outputs] = want;
    // every input given, by the names the note lists: the value to the enum input, true to the others
    let call = |value: &str, ins: &[&str], output: &str| {
        let args: Vec<String> = ins.iter().enumerate().map(|(i, n)| format!("{n}: {}", if i == 0 { value } else { "true" })).collect();
        format!("{name}({}).{output}", args.join(", "))
    };
    let (v0, o0) = (values.0[0], outputs.0[0]);
    // a value given to the enum input that is not one of its values
    assert_eq!(lists(&said(&call("nowhere", inputs.0, o0), None)), owned(values), "the values given");
    for v in values.0 {
        let ds = said(&call(v, inputs.0, o0), None);
        assert!(ds.is_empty(), "{v}: {ds:?}");
    }
    // a value of the enum output the rule does not answer, on the right of `is`
    assert_eq!(lists(&said(&call(v0, inputs.0, o0), Some("nowhere"))), owned(outs), "the values compared");
    for v in outs.0 {
        let ds = said(&call(v0, inputs.0, o0), Some(v));
        assert!(ds.is_empty(), "{v}: {ds:?}");
    }
    // an input the rule does not have, in place of the first (which is then given nothing, E105)
    let mut misspelt = inputs.0.to_vec();
    misspelt[0] = "nowhere";
    assert_eq!(lists(&said(&call(v0, &misspelt, o0), None)), owned(inputs), "the inputs");
    // an output the rule does not have
    assert_eq!(lists(&said(&call(v0, inputs.0, "nowhere"), None)), owned(outputs), "the outputs");
    for o in outputs.0 {
        let ds = said(&call(v0, inputs.0, o), None);
        assert!(ds.is_empty(), "{o}: {ds:?}");
    }
}

#[test]
fn the_names_of_a_rule_a_note_lists_are_written_as_the_gate_takes_them() {
    rule_names_listed_as_written(
        "t",
        RULE_OF_HARD_NAMES,
        [
            // by the name, by the alias, by the member of the generated code where the name is a word
            // of the conditions, and by the alias where it is; neither by the name nor by an alias
            (&["Home", "mens", "Today", "in_store"], &["Children's", "Ma'am"]),
            (&["floor", "Any", "customers"], &["Staff's"]),
            // where a name goes, a word of the conditions is written as it is
            (&["sold_in", "desk_of_seller", "in"], &[]),
            (&["desk", "refunder_desk"], &["staff's_note"]),
        ],
    );
}

/// An input a `.gate` can write by neither its name nor its alias can be given nothing, and the note
/// of E101 says so of it, beside the inputs it can write (DESIGN 20). The English version's alias
/// holds an apostrophe, the Japanese version's full-width parentheses.
#[test]
fn an_input_the_gate_cannot_write_is_said_to_be_one() {
    for (name, rule, listed, unwritten) in [
        ("t", "rule t v1\n\ninputs\n  sold_in        : bool\n  o'clock(o'clk) : bool\n\noutputs\n  floor : bool\n\ntable t\npolicy unique\n| sold_in | o'clock | -> floor |\n| true    | -       | true     |\n| false   | -       | false    |\n", "sold_in", "o'clock"),
        (
            "窓口",
            "rule 窓口(t) v1\n\ninputs\n  売った(sold)                       : bool\n  受付時刻（任意）(受付時刻（任意）) : bool\n\noutputs\n  売り場(floor) : bool\n\ntable 表(t)\npolicy unique\n| 売った | 受付時刻（任意） | -> 売り場 |\n| true   | -                | true      |\n| false  | -                | false     |\n",
            "売った",
            "受付時刻（任意）",
        ),
    ] {
        let dir = ritsu_testkit::TempDir::new("e101-input");
        std::fs::write(dir.path().join("t.rule"), rule).unwrap();
        let path = dir.path().join("g.gate");
        std::fs::write(&path, gate_calling(name, &format!("{name}(nowhere: true).floor"), None)).unwrap();
        let o = sekisho::check::check_file(path.to_str().unwrap(), &common::joined(), &Options::default()).unwrap();
        let d = o.diags.iter().find(|d| d.code == "E101").unwrap_or_else(|| panic!("{}", sekisho::check::render(&o, Lang::En)));
        for lang in [Lang::En, Lang::Ja] {
            assert_eq!(listed_in(&d.notes, lang), (vec![listed.to_string()], vec![unwritten.to_string()]), "{lang:?}: {:?}", d.notes);
        }
    }
}

#[test]
fn the_names_of_a_rule_a_note_lists_are_written_as_the_gate_takes_them_in_japanese() {
    rule_names_listed_as_written(
        "窓口",
        RULE_OF_HARD_NAMES_JA,
        [
            (&["紳士服", "home"], &["ベビー＆キッズ", "松＋竹"]),
            (&["売り場の窓口", "service"], &["係（応援）"]),
            (&["売った売り場", "seller"], &[]),
            (&["窓口", "refunder"], &["メモ（係）"]),
        ],
    );
}
