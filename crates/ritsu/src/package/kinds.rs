//! The list of the kinds of request a package holds (ringi's DESIGN 12.2): in each language's
//! package, each kind's ASCII name and version, with its entries `start` and `next`, which take the
//! contract's JSON and answer it (ringi's DESIGN 12.4), and `worked`, which answers what the form
//! works out for a form filled in so far (ringi's DESIGN 13.7); the version of the contract, and the
//! language of the sentences of the entries' errors (`lang`: TypeScript's `lang`, Python's and
//! Rust's `LANG`, Go's `Lang`). A server
//! runs every kind from the list alone, with no code of its own for any of them. Written only in
//! the package of a project that has requests, so that a package of none is as it was.
//!
//! `worked` calls the glue's `worked_out`; for a kind whose form works nothing out, the glue's
//! `worked_none` reads what it is given as the contract's, and the entry answers no field
//! (`{"fields":{}}`).
//!
//! The entries of TypeScript, Python and Go read and write the JSON as kaname's harness does
//! ([`kaname::r#gen::harness::json_entries`]); Rust's answer through the crate's door
//! (`glue::door::answer`, kaname's DESIGN 36.11), which reads a call written as a line of JSON. JSON
//! that is no value of its type is an error of the entry in all four, with the same sentence in the
//! package's language (kaname's DESIGN 16.4): the place, what is there and what it must be. The
//! error holds the place apart too, from the argument's name (`submit.form.days`), so that a server
//! need not read the sentence for it: the `path` of kaname's `KanameError` of the kind `input`
//! (TypeScript, Python, Go's `Path`), and Rust's `EntryError`'s (kaname's DESIGN 4.2).

use super::{Options, Target};
use kaname::check::Checked;
use kaname::r#gen::harness::{JsonEntry, json_entries};
use ritsu_emit::header::Comment;
use ritsu_project::requests::Kind;
use std::rc::Rc;

/// The file of the list in the package of `target`, from the package's root (Python's under its
/// package).
pub fn path(target: Target, o: &Options) -> String {
    match target {
        Target::TypeScript => "ticket_kinds.ts".to_string(),
        Target::Python => format!("{}/ticket_kinds.py", o.name),
        Target::Go => "ticketkinds/ticketkinds.go".to_string(),
        Target::Rust => "ticket_kinds.rs".to_string(),
    }
}

/// The list of `kinds` in the package of `target`, whose modules of kaname are `modules` (each with
/// its path from the root); None when no kind's glue is among them.
pub fn file(target: Target, modules: &[(Rc<Checked>, String)], kinds: &[Kind], o: &Options) -> Option<(String, String)> {
    let found: Vec<(&Kind, usize)> = kinds.iter().filter_map(|k| modules.iter().position(|(_, rel)| *rel == k.glue).map(|i| (k, i))).collect();
    if found.is_empty() {
        return None;
    }
    let contract = ringi::lower::shape::CONTRACT;
    let lang = o.lang.code();
    let checked: Vec<Rc<Checked>> = modules.iter().map(|(c, _)| c.clone()).collect();
    let generated = ritsu_emit::header::generated("ritsu");
    // whether a kind's form works something out: its glue's `worked_out`, else its `worked_none`
    let works = |i: usize| checked[i].decls.fns.iter().any(|f| f.name.text == WORKED_OUT);
    // the entries of each kind: `start` and `next`, and `worked` (for a kind whose form works
    // nothing out, the reading of what it is given, which the list's `worked` calls)
    let entries = |start: &dyn Fn(&str) -> String, next: &dyn Fn(&str) -> String, worked: &dyn Fn(&str) -> String, read: &dyn Fn(&str) -> String| -> Vec<JsonEntry> {
        found
            .iter()
            .flat_map(|(k, i)| {
                let w = if works(*i) { JsonEntry { module: *i, function: WORKED_OUT.into(), name: worked(&k.name) } } else { JsonEntry { module: *i, function: WORKED_NONE.into(), name: read(&k.name) } };
                [JsonEntry { module: *i, function: "start".into(), name: start(&k.name) }, JsonEntry { module: *i, function: "next".into(), name: next(&k.name) }, w]
            })
            .collect()
    };
    let body = match target {
        Target::TypeScript => {
            let head = format!("{}// {}\n\n", Comment::Slashes.line(&generated), ABOUT);
            let mut tail = format!(
                "/** {CONTRACT_DOC} */\nexport const contract = {contract};\n\n/** {LANG_DOC} */\nexport const lang = \"{lang}\";\n\n/** {KIND_DOC} */\nexport interface Kind {{\n  /** {NAME_DOC} */\n  readonly name: string;\n  /** {VERSION_DOC} */\n  readonly version: number;\n  /** {START_DOC} */\n  readonly start: (submit: string, organization: string, now: bigint | number) => string;\n  /** {NEXT_DOC} */\n  readonly next: (ticket: string, event: string, organization: string, now: bigint | number) => string;\n  /** {WORKED_DOC} */\n  readonly worked: (input: string) => string;\n}}\n\n"
            );
            for (k, i) in &found {
                if !works(*i) {
                    tail.push_str(&format!("/** {} */\nfunction worked_{n}(input: string): string {{\n  read_worked_{n}(input);\n  return {NONE_WORKED_TS};\n}}\n\n", none_doc(&k.name), n = k.name));
                }
            }
            tail.push_str(&format!("/** {KINDS_DOC} */\nexport const kinds: readonly Kind[] = [\n"));
            for (k, _) in &found {
                tail.push_str(&format!("  {{ name: {}, version: {}, start: start_{n}, next: next_{n}, worked: worked_{n} }},\n", ritsu_emit::lit::json(&k.name), k.version, n = k.name));
            }
            tail.push_str("];\n");
            json_entries(&checked, kaname::r#gen::Target::TypeScript, &o.name, &o.module, &entries(&|n| format!("start_{n}"), &|n| format!("next_{n}"), &|n| format!("worked_{n}"), &|n| format!("read_worked_{n}")), &head, &tail, o.lang)
        }
        Target::Python => {
            let head = format!("{}\"\"\"{}\"\"\"\n\nfrom collections.abc import Callable\nfrom dataclasses import dataclass\n", Comment::Hash.line(&generated), ABOUT);
            let mut tail = String::new();
            for (k, i) in &found {
                if !works(*i) {
                    tail.push_str(&format!("def _worked_{n}(input: str) -> str:\n    \"\"\"{}\"\"\"\n    _read_worked_{n}(input)\n    return {NONE_WORKED_PY}\n\n\n", none_doc(&k.name), n = k.name));
                }
            }
            tail.push_str(&format!(
                "CONTRACT = {contract}\n\"\"\"{CONTRACT_DOC}\"\"\"\n\nLANG = \"{lang}\"\n\"\"\"{LANG_DOC}\"\"\"\n\n\n@dataclass(frozen=True)\nclass Kind:\n    \"\"\"{KIND_DOC}\"\"\"\n\n    name: str\n    \"\"\"{NAME_DOC}\"\"\"\n    version: int\n    \"\"\"{VERSION_DOC}\"\"\"\n    start: Callable[[str, str, int], str]\n    \"\"\"{START_DOC}\"\"\"\n    next: Callable[[str, str, str, int], str]\n    \"\"\"{NEXT_DOC}\"\"\"\n    worked: Callable[[str], str]\n    \"\"\"{WORKED_DOC}\"\"\"\n\n\nKINDS: tuple[Kind, ...] = (\n"
            ));
            for (k, _) in &found {
                tail.push_str(&format!("    Kind(name={}, version={}, start=_start_{n}, next=_next_{n}, worked=_worked_{n}),\n", ritsu_emit::lit::json(&k.name), k.version, n = k.name));
            }
            tail.push_str(&format!(")\n\"\"\"{KINDS_DOC}\"\"\"\n"));
            json_entries(&checked, kaname::r#gen::Target::Python, &o.name, &o.module, &entries(&|n| format!("_start_{n}"), &|n| format!("_next_{n}"), &|n| format!("_worked_{n}"), &|n| format!("_read_worked_{n}")), &head, &tail, o.lang)
        }
        Target::Go => {
            let head = format!("{}\n{}package ticketkinds\n\n", Comment::Slashes.line(&generated), GO_DOC);
            let mut tail = format!(
                "// Contract is {}.\nconst Contract = {contract}\n\n// Lang is {}.\nconst Lang = \"{lang}\"\n\n// Kind is {}.\ntype Kind struct {{\n\t// Name is the request's name, in ASCII.\n\tName string\n\t// Version is the request's version.\n\tVersion int\n\t// Start is the entry for a request made: {}\n\tStart func(submit json.RawMessage, organization json.RawMessage, now int64) (json.RawMessage, error)\n\t// Next is the entry for an event of a ticket: {}\n\tNext func(ticket json.RawMessage, event json.RawMessage, organization json.RawMessage, now int64) (json.RawMessage, error)\n\t// Worked is the entry for what the form works out: {}\n\tWorked func(in json.RawMessage) (json.RawMessage, error)\n}}\n\n",
                lower_first(CONTRACT_DOC),
                lower_first(LANG_DOC),
                lower_first(KIND_DOC),
                after_colon(START_DOC),
                after_colon(NEXT_DOC),
                after_colon(WORKED_DOC)
            );
            for (k, i) in &found {
                if !works(*i) {
                    let c = camel(&k.name);
                    tail.push_str(&format!(
                        "// worked{c} is the entry Worked of the kind {}, whose form works nothing out: {}\nfunc worked{c}(in json.RawMessage) (json.RawMessage, error) {{\n\tif _, err := readWorked{c}(in); err != nil {{\n\t\treturn nil, err\n\t}}\n\treturn json.RawMessage({NONE_WORKED_GO}), nil\n}}\n\n",
                        k.name,
                        after_colon(&none_doc(&k.name))
                    ));
                }
            }
            tail.push_str("// Kinds are the kinds of request of the package, in the order of their names.\nvar Kinds = []Kind{\n");
            for (k, _) in &found {
                let c = camel(&k.name);
                tail.push_str(&format!("\t{{Name: {}, Version: {}, Start: start{c}, Next: next{c}, Worked: worked{c}}},\n", ritsu_emit::lit::go(&k.name), k.version));
            }
            tail.push_str("}\n");
            json_entries(
                &checked,
                kaname::r#gen::Target::Go,
                &o.name,
                &o.module,
                &entries(&|n| format!("start{}", camel(n)), &|n| format!("next{}", camel(n)), &|n| format!("worked{}", camel(n)), &|n| format!("readWorked{}", camel(n))),
                &head,
                &tail,
                o.lang,
            )
        }
        Target::Rust => rust(&found, &checked, contract, &generated, o.lang, &works),
    };
    Some((path(target, o), body))
}

const ABOUT: &str = "The kinds of request of the package, each with its name, its version and its entries `start`, `next` and `worked`, which take the JSON of the contract and answer it; a server runs every kind from this list alone.";
const CONTRACT_DOC: &str = "The version of the contract the kinds keep: the JSON of a ticket, an event, the part of the organization and an answer, and what `start`, `next` and `worked` take and answer. A server refuses a package whose number it does not know.";
const LANG_DOC: &str = "The language the sentences of the entries' errors are written in, `en` or `ja`: the language the package was written in (`ritsu gen --lang`). A refusal of the machine holds its sentence in both (`text` and `text_ja`).";
const KIND_DOC: &str = "A kind of request: its name and version, and its entries, which take the JSON of the contract and answer it.";
const NAME_DOC: &str = "The request's name, in ASCII.";
const VERSION_DOC: &str = "The request's version.";
const START_DOC: &str = "A request is made: given the JSON of what is submitted and of the part of the organization, and the time in seconds, the answer as JSON (`ok` with the ticket and the things to do, or `err` with the refusal). A call that is no call of the contract is an error.";
const NEXT_DOC: &str = "An event of a ticket: given the JSON of the ticket, the event and the part of the organization, and the time in seconds, the answer as JSON (`ok` with the ticket and the things to do, or `err` with the refusal). A call that is no call of the contract is an error.";
const KINDS_DOC: &str = "The kinds of request of the package, in the order of their names.";
const WORKED_DOC: &str = "What the form works out: given the JSON of a form filled in so far and of what an approver gives or corrects in it (`{\"form\": …, \"edited\": … | null}`), the answer as JSON (`{\"fields\": …}`: whether each field shown or asked for on a condition is, and the value of each field the form works out, null where it is not decided). A call that is no call of the contract is an error.";

/// The names of the glue's functions the entry `worked` calls: what the form works out, and, for a
/// form that works nothing out, the reading of what it is given.
const WORKED_OUT: &str = "worked_out";
const WORKED_NONE: &str = "worked_none";

/// What `worked` answers of a kind whose form works nothing out, in each language.
const NONE_WORKED_TS: &str = r#""{\"fields\":{}}""#;
const NONE_WORKED_PY: &str = r#"'{"fields":{}}'"#;
const NONE_WORKED_GO: &str = r#"`{"fields":{}}`"#;
const NONE_WORKED_RS: &str = r#""{\"fields\":{}}""#;

/// The doc of the entry `worked` of the kind `name`, whose form works nothing out.
fn none_doc(name: &str) -> String {
    format!("The entry `worked` of `{name}`, whose form works nothing out: it reads what it is given as the contract's, and answers no field.")
}

const ENTRY_ERROR_DOC: &str = "Why a call of an entry is no call of the contract: the sentence, in `LANG`, and where an argument is not what the entry takes, the place of it.";
const ENTRY_PATH_DOC: &str = "The place of what is not taken, from the argument's name: `submit.form.days`, `organization.people[0]`, `now`; None where the call is refused as a whole.";
const ENTRY_MESSAGE_DOC: &str = "The sentence: the place, what is there and what it must be.";

/// Go's package comment.
const GO_DOC: &str = "// Package ticketkinds is the list of the kinds of request of the package: each kind's name, version\n// and entries Start, Next and Worked, which take the JSON of the contract and answer it. A server\n// runs every kind from this list alone.\n//\n// An entry answers a refusal in its JSON (`err`); a call that is no call of the contract (JSON that is\n// no value of its type, or a number past the machine's limits) is answered with an error.\n";

/// A sentence to follow `<name> is` in Go's comment: its first letter small, without its full stop.
fn lower_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_lowercase().chain(c).collect::<String>().trim_end_matches('.').to_string(),
        None => String::new(),
    }
}

/// What an entry's doc says after its first `: `, to follow the name of the entry in Go's comment
/// (`Start is the entry for a request made: given the JSON …`).
fn after_colon(s: &str) -> &str {
    s.split_once(": ").map_or(s, |(_, rest)| rest)
}

/// A name in ASCII with `_` between its words, as Go writes the rest of an identifier: `expense_ja`
/// as `ExpenseJa`.
fn camel(n: &str) -> String {
    n.split('_').filter(|w| !w.is_empty()).map(|w| w[..1].to_uppercase() + &w[1..]).collect()
}

/// The list in Rust: each kind's entries call the crate's door with a call written as a line of
/// JSON, each part given whole as one value, and read what it answers: the value, or the sentence of
/// why the call is no call of the contract, in `lang`. The door knows a module by the name the module
/// declares (`match_value` for the kind `match`, whose name kaname reserves), which no sentence says.
fn rust(found: &[(&Kind, usize)], modules: &[Rc<Checked>], contract: u32, generated: &str, lang: ritsu_base::text::Lang, works: &dyn Fn(usize) -> bool) -> String {
    let mut s = format!("{}//! {ABOUT}\n\n/// {CONTRACT_DOC}\npub const CONTRACT: u32 = {contract};\n\n/// {LANG_DOC}\npub const LANG: &str = \"{}\";\n\n", Comment::Slashes.line(generated), lang.code());
    s.push_str(&format!(
        "/// {KIND_DOC}\npub struct Kind {{\n    /// {NAME_DOC}\n    pub name: &'static str,\n    /// {VERSION_DOC}\n    pub version: u32,\n    /// {START_DOC}\n    pub start: fn(submit: &str, organization: &str, now: i64) -> Result<String, EntryError>,\n    /// {NEXT_DOC}\n    pub next: fn(ticket: &str, event: &str, organization: &str, now: i64) -> Result<String, EntryError>,\n    /// {WORKED_DOC}\n    pub worked: fn(input: &str) -> Result<String, EntryError>,\n}}\n\n/// {ENTRY_ERROR_DOC}\n#[derive(Clone, Debug, PartialEq, Eq)]\npub struct EntryError {{\n    /// {ENTRY_PATH_DOC}\n    pub path: Option<String>,\n    /// {ENTRY_MESSAGE_DOC}\n    pub message: String,\n}}\n\nimpl std::fmt::Display for EntryError {{\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{\n        f.write_str(&self.message)\n    }}\n}}\n\nimpl std::error::Error for EntryError {{}}\n\n/// {KINDS_DOC}\npub const KINDS: &[Kind] = &[\n"
    ));
    for (k, _) in found {
        s.push_str(&format!("    Kind {{ name: {}, version: {}, start: start_{n}, next: next_{n}, worked: worked_{n} }},\n", ritsu_emit::lit::json(&k.name), k.version, n = k.name));
    }
    s.push_str("];\n\n");
    for (k, i) in found {
        let (n, m) = (&k.name, ritsu_emit::lit::json(&modules[*i].module.name.text));
        // a kind named in capitals (`fooBar`) is no name of a function in snake case, which Rust
        // warns of: its entries say they are named so
        let allow = if n.chars().any(|c| c.is_ascii_uppercase()) { "#[allow(non_snake_case)]\n" } else { "" };
        s.push_str(&format!(
            "{allow}fn start_{n}(submit: &str, organization: &str, now: i64) -> Result<String, EntryError> {{\n    call({m}, \"start\", &[(\"submit\", submit), (\"organization\", organization)], Some(now))\n}}\n\n{allow}fn next_{n}(ticket: &str, event: &str, organization: &str, now: i64) -> Result<String, EntryError> {{\n    call({m}, \"next\", &[(\"ticket\", ticket), (\"event\", event), (\"organization\", organization)], Some(now))\n}}\n\n"
        ));
        let worked = if works(*i) {
            format!("call({m}, \"{WORKED_OUT}\", &[(\"input\", input)], None)")
        } else {
            format!("call({m}, \"{WORKED_NONE}\", &[(\"input\", input)], None)?;\n    Ok({NONE_WORKED_RS}.to_string())")
        };
        let doc = if works(*i) { String::new() } else { format!("/// {}\n", none_doc(&k.name)) };
        s.push_str(&format!("{doc}{allow}fn worked_{n}(input: &str) -> Result<String, EntryError> {{\n    {worked}\n}}\n\n"));
    }
    // the sentence of a part that is no value of JSON, as kaname's readers say it (its DESIGN 16.4)
    let not_json = match lang {
        ritsu_base::text::Lang::En => "The argument `{name}` of `{function}` is not one value of JSON",
        ritsu_base::text::Lang::Ja => "関数 `{function}` の引数 `{name}` は、一つの JSON の値ではありません",
    };
    s.push_str(&RUST_CALL.replace("@NOT_JSON@", not_json));
    s
}

/// What Rust's entries call the door with, and read its answer by.
const RUST_CALL: &str = r#"/// The call of the function `function` of the module `module` through the crate's door: each part
/// (a JSON text, one value) and the time where the function takes one, written as a line of JSON;
/// what it answers, the value of the function as a JSON text, or why the call is no call of the
/// contract (the message and the place of the entry's failure, or of the door's refusal).
fn call(module: &str, function: &str, parts: &[(&str, &str)], now: Option<i64>) -> Result<String, EntryError> {
    let mut line = format!("{{\"module\":\"{module}\",\"function\":\"{function}\",\"in\":{{");
    for (name, text) in parts {
        let b = text.as_bytes();
        if value_end(b, 0, 0).map(|i| space(b, i)) != Some(b.len()) {
            return Err(EntryError { path: Some(name.to_string()), message: format!("@NOT_JSON@") });
        }
        line.push_str(&format!("\"{name}\":{text},"));
    }
    match now {
        Some(now) => line.push_str(&format!("\"now\":{now}}}}}")),
        None => {
            if line.ends_with(',') {
                line.pop();
            }
            line.push_str("}}");
        }
    }
    let answer = crate::glue::door::answer(&line);
    let (mut out, mut why, mut path) = (None, None, None);
    for (key, value) in members(&answer) {
        match key {
            "\"out\"" => out = Some(value.to_string()),
            "\"fails\"" => {
                for (k, v) in members(value) {
                    match k {
                        "\"message\"" => why = Some(text_of(v)),
                        "\"path\"" => path = Some(text_of(v)),
                        _ => {}
                    }
                }
            }
            "\"error\"" => why = Some(text_of(value)),
            "\"path\"" => path = Some(text_of(value)),
            _ => {}
        }
    }
    match (out, why) {
        (Some(v), _) => Ok(v),
        (None, Some(message)) => Err(EntryError { path, message }),
        (None, None) => Err(EntryError { path: None, message: answer }),
    }
}

/// The text of a JSON string, written with its quotes and escapes.
fn text_of(s: &str) -> String {
    let inner = s.strip_prefix('"').and_then(|t| t.strip_suffix('"')).unwrap_or(s);
    let mut out = String::new();
    let mut cs = inner.chars();
    while let Some(c) = cs.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match cs.next() {
            Some('b') => out.push('\u{8}'),
            Some('f') => out.push('\u{c}'),
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('u') => {
                let hex: String = cs.by_ref().take(4).collect();
                let mut cp = u32::from_str_radix(&hex, 16).unwrap_or(0xFFFD);
                // a surrogate pair, written as two escapes
                if (0xD800..0xDC00).contains(&cp) {
                    let rest = cs.as_str();
                    if let Some(lo) = rest.strip_prefix("\\u").and_then(|r| r.get(..4)).and_then(|h| u32::from_str_radix(h, 16).ok()).filter(|l| (0xDC00..0xE000).contains(l)) {
                        cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                        cs = rest[6..].chars();
                    }
                }
                out.push(char::from_u32(cp).unwrap_or('\u{FFFD}'));
            }
            Some(c) => out.push(c),
            None => {}
        }
    }
    out
}

/// The keys and the values of the object of JSON a line of the door is, each as written.
fn members(line: &str) -> Vec<(&str, &str)> {
    let b = line.as_bytes();
    let mut out = Vec::new();
    let mut i = space(b, 0);
    if b.get(i) != Some(&b'{') {
        return out;
    }
    i += 1;
    loop {
        let k0 = space(b, i);
        let Some(k1) = string_end(b, k0) else { return out };
        let colon = space(b, k1);
        if b.get(colon) != Some(&b':') {
            return out;
        }
        let v0 = space(b, colon + 1);
        let Some(v1) = value_end(b, v0, 0) else { return out };
        out.push((&line[k0..k1], &line[v0..v1]));
        i = space(b, v1);
        match b.get(i) {
            Some(b',') => i += 1,
            _ => return out,
        }
    }
}

/// The end of the white space at `i`.
fn space(b: &[u8], mut i: usize) -> usize {
    while matches!(b.get(i), Some(b' ' | b'\t' | b'\n' | b'\r')) {
        i += 1;
    }
    i
}

/// The end of the string at `i`, its quotes and escapes (each an escape JSON has).
fn string_end(b: &[u8], i: usize) -> Option<usize> {
    if b.get(i) != Some(&b'"') {
        return None;
    }
    let mut j = i + 1;
    loop {
        match *b.get(j)? {
            b'\\' => match *b.get(j + 1)? {
                b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't' => j += 2,
                b'u' if b.get(j + 2..j + 6).is_some_and(|h| h.iter().all(u8::is_ascii_hexdigit)) => j += 6,
                _ => return None,
            },
            b'"' => return Some(j + 1),
            c if c < 0x20 => return None,
            _ => j += 1,
        }
    }
}

/// The end of the digits at `i`.
fn digits(b: &[u8], mut i: usize) -> usize {
    while b.get(i).is_some_and(u8::is_ascii_digit) {
        i += 1;
    }
    i
}

/// The end of the number at `i`.
fn number_end(b: &[u8], i: usize) -> Option<usize> {
    let mut j = if b.get(i) == Some(&b'-') { i + 1 } else { i };
    let k = digits(b, j);
    if k == j {
        return None;
    }
    j = k;
    if b.get(j) == Some(&b'.') {
        let k = digits(b, j + 1);
        if k == j + 1 {
            return None;
        }
        j = k;
    }
    if matches!(b.get(j), Some(b'e' | b'E')) {
        j += 1;
        if matches!(b.get(j), Some(b'+' | b'-')) {
            j += 1;
        }
        let k = digits(b, j);
        if k == j {
            return None;
        }
        j = k;
    }
    Some(j)
}

/// The end of the one value of JSON at `i`, after its white space; None where there is none, or it
/// is nested deeper than the door reads.
fn value_end(b: &[u8], i: usize, depth: usize) -> Option<usize> {
    if depth > 1024 {
        return None;
    }
    let i = space(b, i);
    let word = |w: &[u8]| b.get(i..i + w.len()).filter(|x| *x == w).map(|_| i + w.len());
    match *b.get(i)? {
        b'"' => string_end(b, i),
        b't' => word(b"true"),
        b'f' => word(b"false"),
        b'n' => word(b"null"),
        b'-' | b'0'..=b'9' => number_end(b, i),
        open @ (b'{' | b'[') => {
            let close = if open == b'{' { b'}' } else { b']' };
            let mut j = space(b, i + 1);
            if b.get(j) == Some(&close) {
                return Some(j + 1);
            }
            loop {
                if open == b'{' {
                    j = space(b, string_end(b, space(b, j))?);
                    if b.get(j) != Some(&b':') {
                        return None;
                    }
                    j += 1;
                }
                j = space(b, value_end(b, j, depth + 1)?);
                match *b.get(j)? {
                    b',' => j += 1,
                    c if c == close => return Some(j + 1),
                    _ => return None,
                }
            }
        }
        _ => None,
    }
}
"#;
