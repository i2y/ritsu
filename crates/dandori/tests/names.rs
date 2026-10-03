//! The names of the rules a flow calls, in the code dandori writes (DESIGN 1.15). The code rulec
//! generates for a rule goes by the rule's alias and its enums' aliases, and the files dandori
//! writes around it import those names beside their own: a name those files use already is
//! refused (E006), and so are two rules of one module and two activities of one name. What is not
//! refused builds: tests/flows/names.flow, which every platform runs (tests/examples.rs), calls
//! rules whose names come close, and two of them take enums of one name.

use dandori::check::Checked;
use std::path::Path;
use std::rc::Rc;

/// A rule aliased `alias` that takes an enum (aliased `enum_alias`, `Room` in the generated code
/// when it is `room`), a bool, a string and an amount.
fn rule(alias: &str, enum_alias: &str) -> String {
    format!(
        "rule 名前の試し({alias}) v1\n\nenum 客室({enum_alias}) = standard | deluxe | suite\nenum 扱い(handling) = 自動(auto) | 確認(review)\n\ninputs\n  客室(room) : 客室\n  会員(member) : bool\n  名前(name) : string\n  金額(amount) : money[JPY, incl_tax]  range >=0JPY <=1000JPY\n\noutputs\n  扱い(handling) : 扱い\n  額(charge) : money[JPY, incl_tax]  round down(1JPY)\n\ntable 客室ごとの扱い(decide)\npolicy unique\n| 客室     | 会員  | -> 扱い | 額     |\n| standard | -     | 自動    | 金額   |\n| deluxe   | -     | 自動    | 金額   |\n| suite    | true  | 自動    | 0JPY   |\n| suite    | false | 確認    | 金額   |\n"
    )
}

/// A rule aliased `alias` that takes no enum.
fn plain_rule(alias: &str) -> String {
    format!("rule 名前の試し({alias}) v1\n\ninputs\n  会員(member) : bool\n\noutputs\n  急ぎ(urgent) : bool\n\ntable 急ぎ(decide)\npolicy unique\n| 会員  | -> 急ぎ |\n| true  | true    |\n| false | false   |\n")
}

/// A flow that calls each rule of `rules` (its name in the flow, its file), in `dir`, checked with rulec's port.
fn check(dir: &Path, rules: &[(&str, &str)], more: &str) -> Checked {
    let mut src = String::from("workflow 試し v1\n\n");
    for (name, file) in rules {
        src.push_str(&format!("use rule {name} from \"{file}\"\n"));
    }
    src.push_str(more);
    let takes_room = |file: &str| std::fs::read_to_string(dir.join(file)).unwrap().contains("客室(room)");
    src.push_str("\ninputs\n  会員 : bool\n  金額 : money[JPY, incl_tax]  range >=0 <=1000\n");
    for (name, file) in rules {
        if takes_room(file) {
            src.push_str(&format!("  k_{name} : {name}.客室\n"));
        }
    }
    src.push_str("\nflow\n");
    for (name, file) in rules {
        if takes_room(file) {
            src.push_str(&format!("  let r_{name} = {name}(客室: k_{name}, 会員: 会員, 名前: \"x\", 金額: 金額)\n"));
        } else {
            src.push_str(&format!("  let r_{name} = {name}(会員: 会員)\n"));
        }
    }
    src.push_str("  pass\n");
    dandori::sources::with_rules(Rc::new(rulec::ports::Engine::new()), || dandori::check::check_source(&src, &dir.join("試し.flow")))
}

fn said(c: &Checked) -> Vec<(String, usize, String, Vec<String>)> {
    c.diags.iter().map(|d| (d.code.to_string(), d.line, d.en.clone(), d.notes.iter().map(|n| n.en.clone()).collect())).collect()
}

fn write(dir: &Path, file: &str, text: &str) {
    std::fs::write(dir.join(file), text).unwrap();
}

/// Where each list's names would clash, as the diagnostic's note names it.
const TS: &str = "rules.ts (Temporal's TypeScript, Argo)";
const PY: &str = "rules.py (Temporal's Python, pydantic-graph)";
const LAMBDA: &str = "the Lambda function (Step Functions, Lambda durable functions)";
const GO: &str = "rules.go (Temporal's Go), as a Go package";

#[test]
fn every_name_the_code_around_a_rule_uses_is_refused() {
    let dir = ritsu_testkit::TempDir::new("names");
    let lists: [(&[&str], &str); 4] = [(dandori::temporal::AROUND_RULES, TS), (dandori::temporal_py::AROUND_RULES, PY), (dandori::asl::AROUND_RULE, LAMBDA), (dandori::temporal_go::AROUND_RULES, GO)];
    let mut names: Vec<&str> = lists.iter().flat_map(|(l, _)| l.iter().copied()).collect();
    names.sort();
    names.dedup();
    for name in names {
        write(dir.path(), "r.rule", &rule(name, "room"));
        let c = check(dir.path(), &[("規則", "r.rule")], "");
        // the name each file's code calls the rule by: the alias, and in Go the package
        let mut want: Vec<(String, Vec<&str>)> = Vec::new();
        for (i, (list, place)) in lists.iter().enumerate() {
            let gen = if i == 3 { name.replace('_', "").to_lowercase() } else { name.to_string() };
            if list.contains(&gen.as_str()) {
                match want.iter_mut().find(|(g, _)| *g == gen) {
                    Some((_, ps)) => ps.push(place),
                    None => want.push((gen, vec![place])),
                }
            }
        }
        let want: Vec<(String, usize, String, Vec<String>)> = want
            .into_iter()
            .map(|(gen, places)| {
                (
                    "E006".to_string(),
                    3,
                    format!("the rule `規則` is `{gen}` in the code rulec generates, a name the code dandori writes around it uses already"),
                    vec![format!("Where: {}", places.join("; ")), "Change the rule's alias in its file (`rule <name>(<alias>) v1`); its name can stay as it is.".to_string()],
                )
            })
            .collect();
        assert_eq!(said(&c), want, "{name}");
    }
    // an enum's alias reaches rules.py and the Lambda function as a class: `any` is `Any`
    write(dir.path(), "r.rule", &rule("fee", "any"));
    let c = check(dir.path(), &[("規則", "r.rule")], "");
    assert_eq!(said(&c).iter().map(|(code, line, en, _)| (code.as_str(), *line, en.as_str())).collect::<Vec<_>>(), [("E006", 3, "the enum `客室` of the rule `規則` is `Any` in the code rulec generates, a name the code dandori writes around it uses already")]);
    // Go reads an enum's value into `ok`: a package of that name clashes only with a rule that takes an enum
    write(dir.path(), "r.rule", &rule("ok", "room"));
    assert_eq!(said(&check(dir.path(), &[("規則", "r.rule")], "")).iter().map(|(_, _, _, n)| n[0].as_str()).collect::<Vec<_>>(), [format!("Where: {GO}")]);
    write(dir.path(), "r.rule", &plain_rule("ok"));
    assert!(check(dir.path(), &[("規則", "r.rule")], "").diags.is_empty());
    // the alias is what goes into the code; the name the flow gives the rule is not
    write(dir.path(), "r.rule", &rule("fee", "room"));
    assert!(check(dir.path(), &[("rules", "r.rule")], "").diags.is_empty());
    // a Go package is the alias in lower case without `_`
    write(dir.path(), "r.rule", &plain_rule("Con_text"));
    assert_eq!(said(&check(dir.path(), &[("規則", "r.rule")], "")).iter().map(|(_, _, en, _)| en.as_str()).collect::<Vec<_>>(), ["the rule `規則` is `context` in the code rulec generates, a name the code dandori writes around it uses already"]);
}

/// A rule called at its Connect service has no code that goes with the workflow, so its names are
/// imported nowhere.
#[test]
fn a_rule_at_its_service_is_not_refused_for_its_names() {
    let dir = ritsu_testkit::TempDir::new("names");
    write(dir.path(), "r.rule", &plain_rule("rules"));
    let c = check(dir.path(), &[("規則", "r.rule")], "  connect \"https://rules.example.com\"\n");
    assert!(c.diags.is_empty(), "{:?}", said(&c));
    let c = check(dir.path(), &[("規則", "r.rule")], "");
    assert_eq!(c.diags.len(), 1, "{:?}", said(&c));
}

/// rules.py defines each rule's activity, `rule_<rule>`, beside what it imports: an activity of a
/// name of the generated code would take its place, and the name to change is the flow's.
#[test]
fn an_activity_of_a_name_the_generated_code_has_is_refused() {
    let dir = ritsu_testkit::TempDir::new("names");
    write(dir.path(), "r.rule", &plain_rule("rule_x"));
    let c = check(dir.path(), &[("x", "r.rule")], "");
    assert_eq!(
        said(&c),
        [(
            "E006".to_string(),
            3,
            "`rule_x`, the activity of the rule `x`, is also a name of the code rulec generates for the rule `x`, and the two clash in rules.py".to_string(),
            vec!["Give the rule another name than `x` in `use rule`.".to_string()]
        )]
    );
    assert!(check(dir.path(), &[("y", "r.rule")], "").diags.is_empty());
    // and the activity of one rule against the function of another
    write(dir.path(), "s.rule", &plain_rule("fee"));
    let c = check(dir.path(), &[("x", "s.rule"), ("y", "r.rule")], "");
    assert_eq!(
        said(&c).iter().map(|(code, line, en, _)| (code.as_str(), *line, en.as_str())).collect::<Vec<_>>(),
        [("E006", 3, "`rule_x`, the activity of the rule `x`, is also a name of the code rulec generates for the rule `y`, and the two clash in rules.py")]
    );
}

/// `rulec gen` writes a rule's code to files named by its alias (its Go package by the alias
/// without `_`, in lower case): two rules of one would be written over each other. One rule
/// called by two names is one module.
#[test]
fn two_rules_of_one_module_are_refused() {
    let dir = ritsu_testkit::TempDir::new("names");
    write(dir.path(), "a.rule", &plain_rule("fee"));
    write(dir.path(), "b.rule", &rule("fee", "room"));
    let c = check(dir.path(), &[("甲", "a.rule"), ("乙", "b.rule")], "");
    assert_eq!(
        said(&c),
        [(
            "E006".to_string(),
            4,
            "the rules `甲` and `乙` are two rules, but one module, `fee`, in the code rulec generates: one would be written over the other".to_string(),
            vec!["Change the alias of one of them.".to_string()]
        )]
    );
    assert!(check(dir.path(), &[("甲", "a.rule"), ("乙", "a.rule")], "").diags.is_empty());
    write(dir.path(), "b.rule", &rule("Fee_", "room"));
    assert_eq!(said(&check(dir.path(), &[("甲", "a.rule"), ("乙", "b.rule")], "")).iter().map(|(_, _, en, _)| en.as_str()).collect::<Vec<_>>(), ["the rules `甲` and `乙` are two rules, but one module, `fee`, in the code rulec generates: one would be written over the other"]);
}

/// Every task is an activity by its name, and every rule called by `rule_<rule>`.
#[test]
fn a_task_of_a_rules_activity_is_refused() {
    let dir = ritsu_testkit::TempDir::new("names");
    write(dir.path(), "r.rule", &plain_rule("fee"));
    let c = check(dir.path(), &[("x", "r.rule")], "\ntask rule_x(a: string)\n");
    assert_eq!(
        said(&c),
        [(
            "E006".to_string(),
            5,
            "the task `rule_x` and the rule `x` would be one activity, `rule_x`, in the code dandori writes".to_string(),
            vec!["Rename the task: a name that starts with `rule_` is a rule's activity.".to_string()]
        )]
    );
    assert!(check(dir.path(), &[("x", "r.rule")], "\ntask rule_y(a: string)\n").diags.is_empty());
}

/// The words of the code of `text`.
fn words(text: &str) -> std::collections::BTreeSet<String> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '_')).filter(|w| !w.is_empty()).map(String::from).collect()
}

/// Each list is what its file writes: every name of it is in the file dandori writes for a rule
/// that takes each kind of value there is (so that a name the code stops using leaves the list).
#[test]
fn the_lists_are_what_the_code_around_a_rule_writes() {
    let dir = ritsu_testkit::TempDir::new("names");
    write(dir.path(), "r.rule", &rule("fee", "room"));
    let c = check(dir.path(), &[("規則", "r.rule")], "");
    let m = c.model.as_ref().unwrap_or_else(|| panic!("{:?}", said(&c)));
    let file = |files: Vec<(String, String)>, end: &str| files.into_iter().find(|(n, _)| n.ends_with(end)).map(|(_, t)| t).unwrap_or_else(|| panic!("no {end}"));
    let ts = file(dandori::temporal::build(m).unwrap(), "/rules.ts");
    let py = file(dandori::temporal_py::build(m).unwrap(), "/rules.py");
    let go = file(dandori::temporal_go::build(m).unwrap(), "/rules.go");
    let (_, lambda) = dandori::asl::lambda_handler(m, 0);
    for (list, text, what) in [(dandori::temporal::AROUND_RULES, &ts, "rules.ts"), (dandori::temporal_py::AROUND_RULES, &py, "rules.py"), (dandori::asl::AROUND_RULE, &lambda, "the Lambda handler"), (dandori::temporal_go::AROUND_RULES, &go, "rules.go")] {
        let ws = words(text);
        for name in list {
            assert!(ws.contains(*name), "{what} has no `{name}`:\n{text}");
        }
    }
    assert!(words(&go).contains("ok"), "rules.go reads an enum's value into `ok`:\n{go}");
}
