//! OpenSpec's specs as sources (DESIGN 20), on the examples `openspec_greeter` (a change not yet
//! archived) and `openspec_greeter_archived` (the same project after `openspec archive`, whose
//! spec OpenSpec 1.14.0 wrote), in English and in Japanese. What the commands print is held to the
//! golden files in `tests/golden/openspec/` (`YUEN_BLESS=1` writes them again).

mod common;

use ritsu_base::openspec;
use ritsu_base::sha256;
use std::path::Path;

const EX: &str = "examples/openspec_greeter";
const ARCHIVED: &str = "examples/openspec_greeter_archived";

fn run_in(dir: &str, args: &[&str]) -> common::Ran {
    let mut a: Vec<&str> = args.to_vec();
    a.extend(["--root", dir]);
    common::run(&a)
}

fn golden(name: &str, r: &common::Ran, failures: &mut Vec<String>) {
    let text = format!("{}{}exit {}\n", r.stdout, r.stderr, r.code);
    common::golden(&format!("tests/golden/openspec/{name}.txt"), &text, failures);
}

fn done(failures: Vec<String>) {
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn each_requirement_is_pinned_by_its_block() {
    for (req, spec) in [("greeter.req", "openspec/specs/greeting/spec.md"), ("greeter.ja.req", "ja/openspec/specs/greeting/spec.md")] {
        let r = run_in(EX, &["check", &format!("{EX}/{req}")]);
        assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
        assert!(r.stdout.contains(" ok — ") || r.stdout.contains(": ok"), "{}", r.stdout);
        let blocks = openspec::read_spec(&std::fs::read(Path::new(EX).join(spec)).unwrap()).unwrap();
        let api = run_in(EX, &["api", &format!("{EX}/{req}")]);
        let v: serde_json::Value = serde_json::from_str(&api.stdout).unwrap();
        let src = &v["sources"][0];
        assert_eq!(src["kind"], "openspec");
        assert_eq!(src["path"], spec);
        for pin in src["pins"].as_array().unwrap() {
            let name = pin["fragment"].as_str().unwrap();
            let block = &blocks.get(name).unwrap().block;
            assert_eq!(pin["sha256"].as_str().unwrap(), sha256::short(block.as_bytes()), "{req}: {name}");
        }
        assert_eq!(src["pins"].as_array().unwrap().len(), blocks.requirements.len(), "{req}: every requirement of the spec is pinned");
    }
}

#[test]
fn an_archived_change_stops_the_check_on_what_it_changed() {
    let mut failures = Vec::new();
    for (req, tag) in [("greeter.req", "check-archived"), ("greeter.ja.req", "check-archived.ja")] {
        let lang = if tag.ends_with(".ja") { "ja" } else { "en" };
        let r = run_in(ARCHIVED, &["check", &format!("{ARCHIVED}/{req}"), "--lang", lang]);
        assert_eq!(r.code, 1, "{}", r.stdout);
        assert_eq!(common::codes(&r.stdout), ["W102", "E103", "W402"], "{}", r.stdout);
        golden(tag, &r, &mut failures);
    }
    done(failures);
}

/// After `source pin`, the link from the requirement of the spec, and every link below the
/// requirement that reads it, are marked; the other requirements of the spec mark nothing.
#[test]
fn pinned_again_the_links_below_the_requirement_are_marked() {
    for (req, from_line) in [("greeter.req", 15), ("greeter.ja.req", 15)] {
        let t = common::TempDir::new("openspec-pin");
        let dir = t.path().join("x");
        common::copy_dir(Path::new(ARCHIVED), &dir);
        let d = dir.to_str().unwrap();
        let file = format!("{d}/{req}");
        let r = run_in(d, &["source", "pin", &file]);
        assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
        let r = run_in(d, &["check", &file]);
        assert_eq!(r.code, 1);
        let codes = common::codes(&r.stdout);
        assert_eq!(codes, ["W102", "E302", "E302", "E302", "E302", "W402"], "{req}: {}", r.stdout);
        assert!(r.stdout.contains(&format!("{req}:{from_line}:3")), "{}", r.stdout);
        assert!(r.stdout.contains("what changed in the requirement (the spec "), "{}", r.stdout);
        assert_eq!(r.stdout.matches("comes from something that changed").count(), 1, "{}", r.stdout);
        // once a person has looked, the check passes but for the requirement added, and the scenario
        // added, which no claim answers yet
        let r = run_in(d, &["review", &file, "--all", "--by", if req.contains(".ja.") { "開発" } else { "development" }, "--date", "2026-10-06"]);
        assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
        let r = run_in(d, &["check", &file]);
        assert_eq!((r.code, common::codes(&r.stdout)), (0, vec!["W102".to_string(), "W402".to_string()]), "{}", r.stdout);
    }
}

#[test]
fn outdated_says_what_a_change_not_yet_archived_does() {
    let mut failures = Vec::new();
    for (req, tag, lang) in [("greeter.req", "outdated", "en"), ("greeter.ja.req", "outdated.ja", "ja")] {
        let r = run_in(EX, &["source", "outdated", &format!("{EX}/{req}"), "--lang", lang]);
        assert_eq!(r.code, 1, "a change modifies a requirement pinned: {}", r.stdout);
        golden(tag, &r, &mut failures);
    }
    // after the archive, no change is pending
    let r = run_in(ARCHIVED, &["source", "outdated", &format!("{ARCHIVED}/greeter.req")]);
    assert_eq!(r.code, 0, "{}", r.stdout);
    assert!(r.stdout.contains("no change not yet archived touches the requirements of"), "{}", r.stdout);
    done(failures);
}

#[test]
fn fetch_has_nothing_to_fetch_and_pin_writes_the_pins() {
    let r = run_in(EX, &["source", "fetch", &format!("{EX}/greeter.req")]);
    assert_eq!(r.code, 0);
    assert!(r.stdout.contains("is an OpenSpec spec, a file written in the project, so there is nothing to fetch"), "{}", r.stdout);
    let r = run_in(EX, &["source", "pin", &format!("{EX}/greeter.req")]);
    assert_eq!(r.code, 0);
    assert!(r.stdout.contains("greeting: all 3 requirements already pinned"), "{}", r.stdout);
    // a source without its pins gets them, after the source line
    let t = common::TempDir::new("openspec-pin-new");
    let dir = t.path().join("x");
    common::copy_dir(Path::new(EX), &dir);
    let file = dir.join("greeter.req");
    let full = std::fs::read_to_string(&file).unwrap();
    let bare: String = full.lines().filter(|l| !(l.starts_with("  \"") && l.contains("sha256:"))).map(|l| format!("{l}\n")).collect();
    std::fs::write(&file, &bare).unwrap();
    let d = dir.to_str().unwrap();
    let r = run_in(d, &["source", "pin", file.to_str().unwrap()]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert!(r.stdout.contains("greeting: added a pin line for the cited \"Greeting by name\" (sha256:1c3d865f4a521275)"), "{}", r.stdout);
    assert_eq!(std::fs::read_to_string(&file).unwrap(), full, "pin writes the lines the example has, byte for byte");
}

#[test]
fn affected_reads_the_spec_and_the_delta_spec_of_a_proposal() {
    let mut failures = Vec::new();
    for (diff, tag) in [("propose", "affected-propose"), ("archive", "affected-archive")] {
        for (req, lang) in [("greeter.req", "en"), ("greeter.ja.req", "ja")] {
            let d = if lang == "ja" { format!("{EX}/diffs/{diff}.ja.diff") } else { format!("{EX}/diffs/{diff}.diff") };
            let r = run_in(EX, &["affected", &format!("{EX}/{req}"), "--diff", &d, "--lang", lang]);
            assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
            golden(&if lang == "ja" { format!("{tag}.ja") } else { tag.to_string() }, &r, &mut failures);
        }
    }
    let r = run_in(EX, &["affected", &format!("{EX}/greeter.req"), "--diff", &format!("{EX}/diffs/propose.diff"), "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    let ch = &v["openspec_changes"][0];
    assert_eq!(ch["change"], "trim-names");
    assert_eq!(ch["spec"], "openspec/specs/greeting/spec.md");
    let ops: Vec<(&str, &str)> = ch["requirements"].as_array().unwrap().iter().map(|x| (x["op"].as_str().unwrap(), x["name"].as_str().unwrap())).collect();
    assert_eq!(ops, [("MODIFIED", "Greeting by name"), ("ADDED", "Health check")]);
    // the archive's diff reaches the requirement it modified and the one it added, and no other
    let r = run_in(EX, &["affected", &format!("{EX}/greeter.req"), "--diff", &format!("{EX}/diffs/archive.diff"), "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["copies"][0]["source"], "greeting \"Greeting by name\", \"Health check\"");
    let reqs: Vec<&str> = v["requirements"].as_array().unwrap().iter().map(|x| x["name"].as_str().unwrap()).collect();
    assert_eq!(reqs, ["greeting_by_name"]);
    done(failures);
}

#[test]
fn trace_and_doc_quote_the_requirement_from_the_spec() {
    let mut failures = Vec::new();
    let r = run_in(EX, &["trace", &format!("{EX}/greeter.req"), "--source", "@greeting \"Running total\""]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    golden("trace-source", &r, &mut failures);
    let r = run_in(EX, &["trace", &format!("{EX}/greeter.ja.req"), "--requirement", "足した数の合計", "--lang", "ja"]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    golden("trace-requirement.ja", &r, &mut failures);
    let r = run_in(EX, &["doc", &format!("{EX}/greeter.req")]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    for want in ["The OpenSpec spec `openspec/specs/greeting/spec.md`", "| Running total | `sha256:f94d564d4cf48f1f` | `running_total` |", "> ### Requirement: Running total"] {
        if !r.stdout.contains(want) {
            failures.push(format!("doc: no {want:?}"));
        }
    }
    done(failures);
}

/// The archived example is the passing one after `openspec archive trim-names`: the same `.req`
/// files and records, the spec OpenSpec wrote, and the change moved under `archive/`.
#[test]
fn the_archived_example_is_the_other_after_the_archive() {
    for f in ["greeter.req", "greeter.ja.req", "greeter.geas", "greeter.ja.geas", "server.py"] {
        assert_eq!(std::fs::read(Path::new(EX).join(f)).unwrap(), std::fs::read(Path::new(ARCHIVED).join(f)).unwrap(), "{f}");
    }
    for (base, change) in [("", "2026-10-05-trim-names"), ("ja/", "2026-10-06-trim-names")] {
        let delta = std::fs::read(format!("{EX}/{base}openspec/changes/trim-names/specs/greeting/spec.md")).unwrap();
        assert_eq!(delta, std::fs::read(format!("{ARCHIVED}/{base}openspec/changes/archive/{change}/specs/greeting/spec.md")).unwrap());
        assert!(!Path::new(&format!("{ARCHIVED}/{base}openspec/changes/trim-names")).exists());
        let before = openspec::read_spec(&std::fs::read(format!("{EX}/{base}openspec/specs/greeting/spec.md")).unwrap()).unwrap();
        let after = openspec::read_spec(&std::fs::read(format!("{ARCHIVED}/{base}openspec/specs/greeting/spec.md")).unwrap()).unwrap();
        let d = openspec::read_delta(&delta).unwrap();
        // what the delta modifies is the block archive wrote; what it leaves is byte for byte
        for r in &before.requirements {
            let now = after.get(&r.name).unwrap();
            match d.changes.iter().find(|c| c.name == r.name) {
                Some(c) => assert_eq!(now.block, c.requirement.as_ref().unwrap().block),
                None => assert_eq!(now.block, r.block),
            }
        }
        assert_eq!(after.requirements.len(), before.requirements.len() + 1, "the requirement added comes last");
    }
}

/// A change that renames a requirement, removes one and modifies the renamed one: `source outdated`
/// names each by the name the spec has now, and says who cites it; after the archive, the check
/// stops on the names the spec no longer has (E108).
#[test]
fn outdated_says_what_a_rename_and_a_removal_do() {
    let t = common::TempDir::new("openspec-rework");
    let dir = t.path().join("x");
    common::copy_dir(Path::new(EX), &dir);
    let delta = "## RENAMED Requirements\n- FROM: `### Requirement: Unknown paths`\n- TO: `### Requirement: Paths it does not know`\n\n## REMOVED Requirements\n\n### Requirement: Running total\n**Reason**: The total moves to another service.\n\n## MODIFIED Requirements\n\n### Requirement: Paths it does not know\nThe service MUST answer any path it does not know with status 404 and the body `not found`.\n\n#### Scenario: unknown paths are 404\n- **WHEN** a client asks for `/nope`\n- **THEN** the status is 404\n";
    let at = dir.join("openspec/changes/rework/specs/greeting");
    std::fs::create_dir_all(&at).unwrap();
    std::fs::write(at.join("spec.md"), delta).unwrap();
    let d = dir.to_str().unwrap();
    let r = run_in(d, &["source", "outdated", &format!("{d}/greeter.req")]);
    assert_eq!(r.code, 1, "{}{}", r.stdout, r.stderr);
    for want in [
        "greeting: the change rework, not yet archived, renames \"Unknown paths\" to \"Paths it does not know\"",
        "greeting: the change rework, not yet archived, removes \"Running total\"",
        "greeting: the change rework, not yet archived, modifies \"Unknown paths\"",
        "cited by: running_total (owned by api,",
        "cited by: unknown_paths (owned by api,",
        "once it is archived, the requirements citing it lose where they come from, and yuen check stops with E108",
        "once it is archived, correct the name in the pin and the citations, then yuen source pin pins it",
    ] {
        assert!(r.stdout.contains(want), "no {want:?} in\n{}", r.stdout);
    }
    // the same change, read from the diff of a pull request that adds it
    let rel = "openspec/changes/rework/specs/greeting/spec.md";
    let lines: Vec<&str> = delta.lines().collect();
    let mut diff = format!("diff --git a/{rel} b/{rel}\nnew file mode 100644\n--- /dev/null\n+++ b/{rel}\n@@ -0,0 +1,{} @@\n", lines.len());
    for l in &lines {
        diff.push_str(&format!("+{l}\n"));
    }
    std::fs::write(dir.join("rework.diff"), diff).unwrap();
    let r = run_in(d, &["affected", &format!("{d}/greeter.req"), "--diff", &format!("{d}/rework.diff"), "--format", "json"]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    let ops: Vec<(String, String, Option<String>, usize)> = v["openspec_changes"][0]["requirements"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| (x["op"].as_str().unwrap().to_string(), x["name"].as_str().unwrap().to_string(), x["to"].as_str().map(str::to_string), x["cited_by"].as_array().unwrap().len()))
        .collect();
    assert_eq!(
        ops,
        [
            ("RENAMED".to_string(), "Unknown paths".to_string(), Some("Paths it does not know".to_string()), 1),
            ("REMOVED".to_string(), "Running total".to_string(), None, 1),
            ("MODIFIED".to_string(), "Unknown paths".to_string(), None, 1),
        ]
    );
    let reqs: Vec<&str> = v["requirements"].as_array().unwrap().iter().map(|x| x["name"].as_str().unwrap()).collect();
    assert_eq!(reqs, ["running_total", "unknown_paths"]);
}
