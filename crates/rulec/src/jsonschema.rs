//! Reading a JSON Schema — the schemas inside an OpenAPI document are the same thing — for
//! the values of one enum (§15.60).
//!
//! A schema has no single name for an enum the way a `.proto` does: the same document holds
//! hundreds of them, nested under `components`, `$defs`, or inline in a property. So the rule
//! names one with a **JSON Pointer**, which is what every other tool in that world uses:
//!
//! ```text
//! import jsonschema "api/openapi.json" "#/components/schemas/MemberTier" -> 会員区分
//! ```
//!
//! The pointer may land on the schema (its `enum` is then read) or on the array itself. What
//! comes back are the strings that go on the wire, and they are the aliases the rule has to
//! carry — **exactly**, with no case folding and no prefix taken off. A `.proto` earns its
//! transformation from a convention `buf lint` enforces; a schema has no such convention, so
//! guessing one would mean a rule and a schema that look like they agree while sending
//! different strings.
//!
//! YAML is not read. It is the usual spelling of an OpenAPI document and the reason is not
//! that it would be hard to start: a reader for the subset the file happens to use is a
//! reader that goes wrong quietly on the next file, and a value set that is quietly wrong is
//! the one thing this must never be. The refusal says so by name and says how to get JSON.

use crate::json::Json;

/// What the pointer found, or why what it found is not a set of names.
pub enum Found {
    Values(Vec<String>),
    /// The pointer left the document. Carries the part of it that did resolve and the keys
    /// that were there, so the next attempt is a correction rather than a guess.
    NoSuchPointer { at: String, keys: Vec<String> },
    /// Something was there, but it is not an enum.
    NotAnEnum(&'static str),
    /// An enum of numbers or nulls. A rule's values are names.
    NotNames(String),
}

/// Parse the document, refusing YAML by name rather than half-reading it.
pub fn read(src: &str, file: &str) -> Result<Json, String> {
    let head = src.trim_start();
    let looks_json = head.starts_with('{') || head.starts_with('[');
    if !looks_json {
        let yaml = file.ends_with(".yaml") || file.ends_with(".yml");
        return Err(if yaml {
            tr!(
                "YAML は読めません。JSON にしたものを指してください（多くの道具が `openapi.json` を書き出せます）。",
                "YAML is not read. Point at a JSON form of it — most toolchains can write `openapi.json`."
            )
        } else {
            tr!(
                "JSON として読めません（`{{` か `[` で始まっていません）。",
                "It does not read as JSON (it does not start with `{{` or `[`)."
            )
        });
    }
    crate::json::parse(src).map_err(|e| tr!("JSON として読めません: {e}", "It does not read as JSON: {e}"))
}

/// The values the pointer names.
pub fn values(doc: &Json, pointer: &str) -> Found {
    let mut here = doc;
    let mut walked = String::new();
    for raw in steps(pointer) {
        let key = unescape(&raw);
        let next = match here {
            Json::Obj(m) => m.get(&key),
            Json::Arr(a) => key.parse::<usize>().ok().and_then(|i| a.get(i)),
            _ => None,
        };
        match next {
            Some(v) => {
                walked.push('/');
                walked.push_str(&raw);
                here = v;
            }
            None => {
                return Found::NoSuchPointer {
                    at: if walked.is_empty() { "#".into() } else { format!("#{walked}") },
                    keys: match here {
                        Json::Obj(m) => m.keys().cloned().collect(),
                        _ => Vec::new(),
                    },
                }
            }
        }
    }
    // The pointer may name the schema or the array. Both are ordinary ways to write it.
    let arr = match here {
        Json::Arr(a) => a,
        Json::Obj(_) => match here.get("enum") {
            Some(Json::Arr(a)) => a,
            Some(other) => return Found::NotAnEnum(other.kind()),
            None => return Found::NotAnEnum("object"),
        },
        other => return Found::NotAnEnum(other.kind()),
    };
    let mut out = Vec::new();
    for v in arr {
        match v {
            Json::Str(s) => out.push(s.clone()),
            other => return Found::NotNames(format!("{other}")),
        }
    }
    Found::Values(out)
}

/// The steps of a pointer. A leading `#` and a leading `/` are both optional, so the three
/// spellings a person actually writes all mean the same thing.
fn steps(pointer: &str) -> Vec<String> {
    pointer
        .trim_start_matches('#')
        .split('/')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// RFC 6901: `~1` is `/` and `~0` is `~`, in that order.
fn unescape(s: &str) -> String {
    s.replace("~1", "/").replace("~0", "~")
}
