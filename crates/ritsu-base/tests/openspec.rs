//! OpenSpec's specs and changes (DESIGN 4.16). The reader is held to what OpenSpec's own readers
//! make of the same files: `tests/fixtures/openspec/expected.json`, which `expected.mjs` writes
//! with OpenSpec 1.14.0's `extractRequirementsSection`, `MarkdownParser` and `parseDeltaSpec`, so
//! the test needs neither Node nor OpenSpec.

use ritsu_base::json::{self, Json};
use ritsu_base::openspec::{self, Op, SpecError};
use std::path::{Path, PathBuf};

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/openspec").join(rel)
}

fn read(rel: &str) -> Vec<u8> {
    std::fs::read(fixture(rel)).unwrap()
}

fn expected() -> Json {
    json::parse(&String::from_utf8(read("expected.json")).unwrap()).unwrap()
}

fn strs(j: &Json) -> Vec<String> {
    j.as_arr().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect()
}

#[test]
fn the_requirements_of_a_spec_are_the_blocks_archive_reads() {
    let want = expected();
    assert_eq!(want.get("openspec").and_then(|v| v.as_str()), Some("1.14.0"));
    for cap in ["greeting", "edges"] {
        let spec = openspec::read_spec(&read(&format!("openspec/specs/{cap}/spec.md"))).unwrap();
        let blocks = want.get("specs").unwrap().get(cap).unwrap().get("blocks").unwrap().as_arr().unwrap();
        let got: Vec<(&str, &str)> = spec.requirements.iter().map(|r| (r.name.as_str(), r.block.as_str())).collect();
        let exp: Vec<(&str, &str)> = blocks.iter().map(|b| (b.get("name").unwrap().as_str().unwrap(), b.get("block").unwrap().as_str().unwrap())).collect();
        assert_eq!(got, exp, "{cap}");
        // the scenarios `openspec show --json` gives, for each requirement archive also reads
        let shown = want.get("specs").unwrap().get(cap).unwrap().get("shown").unwrap().as_arr().unwrap();
        for r in &spec.requirements {
            let s = shown.iter().find(|s| s.get("name").unwrap().as_str() == Some(r.name.as_str())).unwrap();
            let names: Vec<String> = r.scenarios.iter().map(|x| x.name.clone()).collect();
            assert_eq!(names, strs(s.get("scenarios").unwrap()), "{cap}: {}", r.name);
        }
    }
}

#[test]
fn lines_are_counted_in_the_file_as_written() {
    let spec = openspec::read_spec(&read("openspec/specs/greeting/spec.md")).unwrap();
    let lines: Vec<(usize, usize)> = spec.requirements.iter().map(|r| (r.line, r.last)).collect();
    assert_eq!(lines, vec![(8, 18), (20, 26), (28, 33)]);
    let g = spec.get("Greeting by name").unwrap();
    assert_eq!(g.scenarios.iter().map(|s| s.line).collect::<Vec<_>>(), vec![11, 16]);
    assert_eq!(g.scenarios[1].body, vec!["- **WHEN** a client asks for `/greet?name=`", "- **THEN** the status is 400"]);
    // CR LF and a byte order mark change no line number
    let edges = openspec::read_spec(&read("openspec/specs/edges/spec.md")).unwrap();
    assert_eq!(edges.requirements.iter().map(|r| (r.line, r.last)).collect::<Vec<_>>(), vec![(10, 33), (35, 42)]);
    let fenced = edges.get("Fenced headers").unwrap();
    assert_eq!(fenced.scenarios.iter().map(|s| (s.name.as_str(), s.line)).collect::<Vec<_>>(), vec![("a fence of tildes", 18), ("Edge case without the word", 29)]);
    assert_eq!(fenced.scenarios[1].body.last().map(String::as_str), Some("stays in the scenario's body"));
}

#[test]
fn a_name_is_compared_as_written_and_a_near_one_is_named() {
    assert_eq!(openspec::normalize_name("Foo ###"), "Foo");
    assert_eq!(openspec::normalize_name("Foo\t#  "), "Foo");
    assert_eq!(openspec::normalize_name("C#"), "C#");
    assert_eq!(openspec::normalize_name("  Spaced  "), "Spaced");
    assert_eq!(openspec::fold("Greeting  By name"), "greeting by name");
    let spec = openspec::read_spec(&read("openspec/specs/greeting/spec.md")).unwrap();
    assert!(spec.get("greeting by name").is_none());
    assert_eq!(spec.near("greeting by  name"), vec!["Greeting by name"]);
    assert!(spec.near("Greeting by name").is_empty());
}

#[test]
fn a_delta_spec_is_read_in_the_order_archive_applies_it() {
    let want = expected();
    for id in ["trim-names", "rework"] {
        let d = openspec::read_delta(&read(&format!("openspec/changes/{id}/specs/greeting/spec.md"))).unwrap();
        let w = want.get("deltas").unwrap().get(id).unwrap();
        let renamed: Vec<(String, String)> = w.get("renamed").unwrap().as_arr().unwrap().iter().map(|p| (p.get("from").unwrap().as_str().unwrap().to_string(), p.get("to").unwrap().as_str().unwrap().to_string())).collect();
        let got_renamed: Vec<(String, String)> = d.changes.iter().filter(|c| c.op == Op::Renamed).map(|c| (c.from.clone().unwrap(), c.name.clone())).collect();
        assert_eq!(got_renamed, renamed, "{id}");
        let got_removed: Vec<String> = d.changes.iter().filter(|c| c.op == Op::Removed).map(|c| c.name.clone()).collect();
        assert_eq!(got_removed, strs(w.get("removed").unwrap()), "{id}");
        for (op, key) in [(Op::Modified, "modified"), (Op::Added, "added")] {
            let got: Vec<(&str, &str)> = d.changes.iter().filter(|c| c.op == op).map(|c| (c.name.as_str(), c.requirement.as_ref().unwrap().block.as_str())).collect();
            let exp: Vec<(&str, &str)> = w.get(key).unwrap().as_arr().unwrap().iter().map(|b| (b.get("name").unwrap().as_str().unwrap(), b.get("block").unwrap().as_str().unwrap())).collect();
            assert_eq!(got, exp, "{id} {key}");
        }
        let order: Vec<Op> = d.changes.iter().map(|c| c.op).collect();
        let mut sorted = order.clone();
        sorted.sort();
        assert_eq!(order, sorted, "{id}: RENAMED, REMOVED, MODIFIED, ADDED");
    }
    let d = openspec::read_delta(&read("openspec/changes/rework/specs/greeting/spec.md")).unwrap();
    assert_eq!(d.changes.iter().map(|c| (c.op.word(), c.line)).collect::<Vec<_>>(), vec![("RENAMED", 2), ("REMOVED", 8), ("MODIFIED", 14), ("ADDED", 29)]);
    assert!(openspec::is_delta(&String::from_utf8(read("openspec/changes/rework/specs/greeting/spec.md")).unwrap()));
    assert!(!openspec::is_delta(&String::from_utf8(read("openspec/specs/greeting/spec.md")).unwrap()));
}

#[test]
fn what_does_not_read_as_a_spec_says_why() {
    let delta = read("openspec/changes/trim-names/specs/greeting/spec.md");
    assert_eq!(openspec::read_spec(&delta), Err(SpecError::NoRequirements { delta: true }));
    assert_eq!(openspec::read_spec(b"# Notes\n\nNothing here.\n"), Err(SpecError::NoRequirements { delta: false }));
    assert_eq!(openspec::read_spec(b"## Requirements\n```\n## Requirements\n```\n"), Ok(openspec::Spec { requirements: vec![] }));
    let twice = b"## Requirements\n\n### Requirement: A\nOne.\n\n### Requirement: A ##\nTwo.\n";
    assert_eq!(openspec::read_spec(twice), Err(SpecError::Twice { name: "A".into(), first: 3, line: 6 }));
    assert_eq!(openspec::read_spec(&[0xff, 0xfe]), Err(SpecError::NotUtf8));
}

#[test]
fn a_spec_knows_its_openspec_directory_and_the_changes_under_it() {
    assert_eq!(openspec::layout("openspec/specs/greeting/spec.md"), Some(("openspec".into(), "greeting".into())));
    assert_eq!(openspec::layout("svc/openspec/specs/identity/user-auth/spec.md"), Some(("svc/openspec".into(), "identity/user-auth".into())));
    assert_eq!(openspec::layout("openspec/changes/trim-names/specs/greeting/spec.md"), None);
    assert_eq!(openspec::layout("docs/specs/greeting/spec.md"), None);
    assert_eq!(openspec::layout("openspec/specs/spec.md"), None);
    let root = fixture("");
    let active = openspec::active_changes(&root, "openspec");
    let ids: Vec<&str> = active.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(ids, vec!["rework", "trim-names"], "archive/ is left out");
    assert_eq!(active[1].dir, "openspec/changes/trim-names");
    assert_eq!(active[1].deltas, vec![("openspec/changes/trim-names/specs/greeting/spec.md".to_string(), "greeting".to_string())]);
    assert!(openspec::active_changes(&root, "nowhere").is_empty());
}
