//! `geas scenarios` (DESIGN §17): the scenarios of OpenSpec specs held to the claims of the same
//! names, on the greeter's claims and its spec in OpenSpec (`examples/greeter/openspec/`, and
//! `ja/openspec/` in Japanese), whose change `trim-names` asks for two scenarios no claim answers
//! yet. Nothing is run, so the tests run in the crate's directory and print its paths.

mod common;
use common::*;

fn scenarios(args: &[&str]) -> (String, String, i32) {
    let mut a = vec!["scenarios"];
    a.extend(args);
    run(&root(), &a, &[])
}

#[test]
fn every_scenario_of_the_spec_has_its_claim() {
    let (out, err, code) = scenarios(&["examples/greeter/greeter.geas", "--openspec", "examples/greeter/openspec/specs"]);
    assert_eq!((err.as_str(), code), ("", 0));
    golden("en/scenarios/spec.txt", &out);
    let (out, err, code) = scenarios(&["examples/greeter/greeter.ja.geas", "--openspec", "examples/greeter/ja/openspec/specs/greeting/spec.md", "--lang", "ja"]);
    assert_eq!((err.as_str(), code), ("", 0));
    golden("ja/scenarios/spec.txt", &out);
}

#[test]
fn a_change_asks_for_scenarios_no_claim_answers_yet() {
    for (lang, spec, change) in [("en", "examples/greeter/greeter.geas", "examples/greeter/openspec/changes/trim-names"), ("ja", "examples/greeter/greeter.ja.geas", "examples/greeter/ja/openspec/changes/trim-names")] {
        let (out, err, code) = scenarios(&[spec, "--openspec", change, "--lang", lang]);
        assert_eq!((err.as_str(), code), ("", 1), "{out}");
        golden(&format!("{lang}/scenarios/change.txt"), &out);
        let (out, err, code) = scenarios(&[spec, "--openspec", change, "--lang", lang, "--json"]);
        assert_eq!((err.as_str(), code), ("", 1));
        let j = json(&out);
        assert_eq!(j.get("unanswered").num(), 2.0, "{out}");
        assert_eq!(j.get("specs").arr().len(), 1);
        assert_eq!(j.get("specs").arr()[0].get("requirements").arr()[0].get("op").str(), "MODIFIED");
    }
}

#[test]
fn a_draft_is_a_claim_check_refuses_until_its_steps_are_written() {
    let s = Scratch::new("scenarios-draft");
    for (lang, spec, change) in [("en", "examples/greeter/greeter.geas", "examples/greeter/openspec/changes/trim-names"), ("ja", "examples/greeter/greeter.ja.geas", "examples/greeter/ja/openspec/changes/trim-names")] {
        let (out, err, code) = scenarios(&[spec, "--openspec", change, "--draft", "--lang", lang]);
        assert_eq!((err.as_str(), code), ("", 0));
        golden(&format!("{lang}/scenarios/draft.txt"), &out);
        // pasted under the claims file as it is, the draft does not pass
        let mut text = std::fs::read_to_string(root().join(spec)).unwrap();
        text.push('\n');
        text.push_str(&out);
        let file = s.path().join(format!("pasted.{lang}.geas"));
        std::fs::write(&file, text).unwrap();
        let (_, err, code) = run(s.path(), &["check", file.to_str().unwrap()], &[]);
        assert_eq!(code, 2, "{err}");
        assert!(err.contains("error[E005]"), "{err}");
    }
    // with every scenario answered, there is nothing to draft
    let (out, err, code) = scenarios(&["examples/greeter/greeter.geas", "--openspec", "examples/greeter/openspec/specs", "--draft"]);
    assert_eq!((out.as_str(), err.as_str(), code), ("", "every scenario has a claim of its name\n", 0));
}

#[test]
fn a_claim_of_a_near_name_is_pointed_out() {
    let (out, err, code) = scenarios(&["tests/openspec/near.geas", "--openspec", "examples/greeter/openspec/specs/greeting/spec.md"]);
    assert_eq!((err.as_str(), code), ("", 1));
    golden("en/scenarios/near.txt", &out);
}

#[test]
fn what_is_not_an_openspec_spec() {
    let s = Scratch::new("scenarios-errors");
    std::fs::write(s.path().join("notes.md"), "# Notes\n\nNothing here is a spec.\n").unwrap();
    std::fs::create_dir_all(s.path().join("empty")).unwrap();
    let spec = root().join("examples/greeter/greeter.geas");
    let spec = spec.to_str().unwrap();
    for (what, args, code, want) in [
        ("not a spec", vec![spec, "--openspec", "notes.md"], 2, "error[E090]: notes.md: does not read as an OpenSpec spec"),
        ("an empty directory", vec![spec, "--openspec", "empty"], 2, "error[E090]: empty: does not read as an OpenSpec spec: there is no spec.md under it"),
        ("nothing there", vec![spec, "--openspec", "nowhere"], 2, "error[E081]: nowhere: cannot read this file: it is not there"),
        ("no --openspec", vec![spec], 2, "error[E080]: `geas scenarios` needs `--openspec <path>`"),
        ("--draft and --json", vec![spec, "--openspec", "notes.md", "--draft", "--json"], 2, "error[E080]: `--draft` writes claims"),
    ] {
        let mut a = vec!["scenarios"];
        a.extend(args);
        let (out, err, c) = run(s.path(), &a, &[]);
        assert_eq!((out.as_str(), c), ("", code), "{what}: {err}");
        assert!(err.starts_with(want), "{what}: {err}");
    }
}
