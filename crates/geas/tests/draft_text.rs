//! What `geas scenarios --draft` writes from an OpenSpec spec stays where it is written (ritsu's
//! DESIGN 9.2): the text of a requirement and of a scenario, holding a quote, the characters that end
//! a line somewhere (`\r`, U+0085, U+2028, U+2029), the end of a script (`</script>`), YAML's
//! document marker (`---`) and the end of a CDATA section (`]]>`), stays in the comments and the
//! claim's name of the draft. The draft holds one claim and nothing else outside its comments, the
//! claim's name reads back as the scenario's, and `scenarios` matches the two.

mod common;
use common::*;

const ODD: &str = "a \"quoted\" one\r b\u{85}c\u{2028}d\u{2029}e</script>f---g]]>h";

#[test]
fn the_text_of_a_scenario_stays_in_the_draft() {
    let s = Scratch::new("draft-text");
    s.write(
        "openspec/specs/greeting/spec.md",
        format!(
            "# Greeting\n\n## Purpose\nGreets people.\n\n## Requirements\n\n### Requirement: Greeting by name {ODD}\nThe service SHALL greet by name.\n\n#### Scenario: greets by name {ODD}\n- **WHEN** a client asks for `/greet?name=Alice` {ODD}\n- **THEN** the status is 200\n"
        ),
    );
    s.write("empty.geas", "target api {\n  run \"echo\"\n}\n");
    let (draft, err, code) = run(s.path(), &["scenarios", "empty.geas", "--openspec", "openspec/specs/greeting/spec.md", "--draft"], &[]);
    assert_eq!((err.as_str(), code), ("", 0));
    // outside the comments: the claim's line, its closing brace, the blank lines
    let outside: Vec<&str> = draft.lines().filter(|l| !l.trim_start().starts_with('#') && !l.trim().is_empty()).collect();
    assert_eq!(outside.len(), 2, "{draft}");
    assert!(outside[0].starts_with("claim \"greets by name a \\\"quoted\\\" one") && outside[0].ends_with(" {"), "{draft}");
    assert_eq!(outside[1], "}");
    // filled in, the draft is a spec geas runs, and its claim answers the scenario
    let filled = format!(
        "target api {{\n  run \"echo\"\n}}\n\n{}",
        draft.replace("  # when <target>.<call>(…)\n  # then <subject> <matcher>", "  when api.run(\"x\")\n  then exit is 0")
    );
    s.write("filled.geas", &filled);
    let (out, err, code) = run(s.path(), &["check", "filled.geas"], &[]);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    assert!(out.starts_with("ok 1 - greets by name a \"quoted\" one"), "{out}");
    let (out, err, code) = run(s.path(), &["scenarios", "filled.geas", "--openspec", "openspec/specs/greeting/spec.md"], &[]);
    assert_eq!((err.as_str(), code), ("", 0), "{out}");
    assert!(out.contains("1 scenarios · 1 with a claim · 0 with none"), "{out}");
}
