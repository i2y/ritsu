//! What a `.flow` says in prose stays in the comment the generated code writes it in (DESIGN 4.7).
//!
//! The description of a workflow heads its file on every platform, as a comment. A string of a
//! `.flow` may hold `\n`, and the line breaks some targets end a line at besides it (`\r` for
//! Python, U+2028 and U+2029 for TypeScript, those and U+0085 for YAML 1.1), so each of its lines
//! is a comment of its own. Written after one comment mark, as it was, the rest of a description
//! after a break was a line of the TypeScript, the Python and the Go, and in the YAML for Argo it
//! could be a document of its own, which `kubectl apply -f` creates beside the WorkflowTemplate.

use std::path::Path;

/// What every line of the description that matters carries, to be found in the generated files.
const MARK: &str = "PAYLOAD";

/// A workflow every platform builds, with `description` as its description, written in the
/// `.flow` as it is (`\n` is the escape; the other breaks are the characters themselves).
fn flow(description: &str) -> String {
    format!(
        "workflow notice v1\ndescription \"{description}\"\n\ninputs\n  name : string\n\noutputs\n  said : string\n\ntask greet(name: string) -> string\n  lambda \"arn:aws:lambda:eu-west-2:123456789012:function:greet\"\n  idempotent\n\nflow\n  let said = greet(name: name)\n  succeed said = said\n"
    )
}

/// The comment mark of a file dandori writes, by its name; None for JSON, which holds the
/// description as a string.
fn mark(path: &str) -> Option<&'static str> {
    if path.ends_with(".json") {
        None
    } else if path.ends_with(".py") || path.ends_with(".yaml") || path.ends_with(".yml") || path.ends_with("Dockerfile") {
        Some("#")
    } else {
        Some("//")
    }
}

#[test]
fn a_description_stays_in_its_comment_on_every_platform() {
    let description = format!(
        "Says hello.\\n{MARK} after a newline:\\n---\\napiVersion: v1\\nkind: Pod\r{MARK} after a CR\u{85}{MARK} after a NEL\u{2028}{MARK} after a line separator\u{2029}{MARK} after a paragraph separator"
    );
    let checked = dandori::check::check_source(&flow(&description), Path::new("notice.flow"));
    let m = checked.model.unwrap_or_else(|| panic!("the flow does not pass its check: {:?}", checked.diags));
    for target in dandori::commands::TARGETS {
        let files = dandori::commands::build(&m, target).expect("a target dandori builds for").unwrap_or_else(|d| panic!("{target}: {d:?}"));
        let mut seen = 0;
        for (path, text) in &files {
            let found = match mark(path) {
                None => {
                    let v: serde_json::Value = serde_json::from_str(text).unwrap_or_else(|e| panic!("{target}: {path} is not JSON: {e}"));
                    v.to_string().matches(MARK).count()
                }
                Some(mark) => {
                    let mut found = 0;
                    for line in text.split('\n').filter(|l| l.contains(MARK)) {
                        let line = line.trim_start();
                        if line.starts_with(mark) {
                            // a comment ends at the first break of the line, so it holds none
                            assert!(!line.contains(['\r', '\u{85}', '\u{2028}', '\u{2029}']), "{target}: {path}: a comment holds a line break: {line:?}");
                        } else {
                            // else the description is a quoted string, as the annotation of the YAML for
                            // Argo is: `"key": "…"`, a member of an object of JSON
                            let member = format!("{{{}}}", line.trim_end_matches(','));
                            assert!(serde_json::from_str::<serde_json::Value>(&member).is_ok(), "{target}: {path}: the description is out of its comment: {line:?}");
                        }
                        found += line.matches(MARK).count();
                    }
                    found
                }
            };
            // a file that writes the description writes each of its five lines, in comments or a string
            assert!(found % 5 == 0, "{target}: {path}: {found} lines of the description");
            seen += found;
        }
        assert!(seen > 0, "{target}: no file writes the description");
    }
}

/// A description of one line is written as before: one comment after the name and the version.
#[test]
fn a_description_of_one_line_is_one_comment() {
    let checked = dandori::check::check_source(&flow("Says hello."), Path::new("notice.flow"));
    let m = checked.model.expect("the flow passes its check");
    let ts = dandori::commands::build(&m, "temporal").unwrap().unwrap();
    let (_, workflow) = ts.iter().find(|(p, _)| p.ends_with("workflow.ts")).unwrap();
    assert!(workflow.contains("\n// notice v1: Says hello.\n\n"), "{workflow}");
    let argo = dandori::commands::build(&m, "argo").unwrap().unwrap();
    let (_, yaml) = argo.iter().find(|(p, _)| p.ends_with(".yaml")).unwrap();
    assert!(yaml.contains("\n# notice v1: Says hello.\n#\n"), "{yaml}");
}
