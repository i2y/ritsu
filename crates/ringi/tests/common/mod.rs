//! What the tests share: the languages joined as `ritsu ringi` joins them, and a file checked with
//! them.

#![allow(dead_code)]

use ringi::check::{Outcome, Run};
use ringi::suite::Suite;
use ritsu_base::text::Lang;
use std::rc::Rc;
use std::sync::Arc;

/// Every language joined, as ritsu-project's `Joined::ringi` hands them over: rulec, koyomi, chobo,
/// dandori, sekisho and kaname, each through its own engine (rulec reads the days of a koyomi date
/// through koyomi's, and sekisho reads the rules, dates files, books and flows of a gate through
/// the same ones).
pub fn joined() -> Suite {
    let rulec = Rc::new(rulec::ports::Engine::with_dates(Arc::new(koyomi::ports::Engine)));
    let koyomi = Rc::new(koyomi::ports::Engine);
    let chobo = Rc::new(chobo::ports::Engine);
    let dandori = Rc::new(dandori::ports::Engine::default());
    let gates = ritsu_ports::GatePorts { rules: rulec.clone(), dates: koyomi.clone(), books: chobo.clone(), flows: dandori.clone(), items: dandori.clone() };
    let sekisho = Rc::new(sekisho::ports::Engine::new(gates.into()));
    Suite { rules: Some(rulec), dates: Some(koyomi), books: Some(chobo), flows: Some(dandori), gates: Some(sekisho), programs: Some(Rc::new(kaname::ports::Engine)) }
}

/// The file at `path`, checked with every language joined.
pub fn check(path: &str) -> Rc<Outcome> {
    Run::new(joined()).file(path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// The ID of ritsu's catalog the tests make the screens with (DESIGN 13.2).
pub const CATALOG: &str = "https://requests.example.com/a2ui/catalog.json";

/// What `ritsu gen --target requests` says of the screens of the request at `path` before it
/// writes them (DESIGN 13.3, 4.8), as an outcome of its check; the check's own, when the screens
/// say nothing (or the file is no kind of request).
pub fn check_screens(path: &str) -> Rc<Outcome> {
    let engine = ringi::ports::Engine::new(joined());
    // as `ritsu gen` runs it (DESIGN 12.3)
    engine.keep_listing_apart();
    let root = std::path::Path::new(path).parent().map(std::path::Path::to_path_buf).unwrap_or_default();
    let given = ringi::screens::Given { catalog: CATALOG.to_string(), rules: Vec::new() };
    match engine.screens_checked(&root, path, &given) {
        Some(o) => Rc::new(o),
        None => check(path),
    }
}

/// The text `src`, checked as the file `path` with every language joined.
pub fn check_text(path: &str, src: &str) -> Outcome {
    Run::new(joined()).text(path, src)
}

/// The text `src`, checked as the file `path` with no other language joined, as the binary of
/// ringi's own crate checks it.
pub fn check_alone(path: &str, src: &str) -> Outcome {
    Run::new(Suite::default()).text(path, src)
}

/// The codes it says, in order.
pub fn codes(o: &Outcome) -> Vec<&'static str> {
    o.diags.iter().map(|d| d.code).collect()
}

/// What `ringi check` prints for it.
pub fn shown(o: &Outcome, lang: Lang) -> String {
    ringi::check::render(o, lang)
}

/// The `.ringi` files of `examples/`, the shared ones too, in order.
pub fn example_files() -> Vec<String> {
    let mut v = Vec::new();
    for dir in ["examples", "examples/shared"] {
        for e in std::fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.extension().is_some_and(|x| x == "ringi") {
                v.push(p.display().to_string());
            }
        }
    }
    v.sort();
    v
}

/// What is wrong with an HTML page: a tag that is not closed, or closed out of order, and text
/// escaped twice or not at all. The style is passed over.
pub fn html_problems(html: &str) -> Vec<String> {
    const VOID: [&str; 8] = ["meta", "br", "hr", "img", "input", "link", "col", "wbr"];
    let mut probs = Vec::new();
    let mut open: Vec<String> = Vec::new();
    let mut rest = html;
    let mut text = String::new();
    while let Some(i) = rest.find('<') {
        text.push_str(&rest[..i]);
        rest = &rest[i + 1..];
        if let Some(after) = rest.strip_prefix("!--") {
            let Some(j) = after.find("-->") else {
                probs.push("a comment is not closed".to_string());
                break;
            };
            rest = &after[j + 3..];
            continue;
        }
        let Some(j) = rest.find('>') else {
            probs.push("a tag is not closed".to_string());
            break;
        };
        let tag = &rest[..j];
        rest = &rest[j + 1..];
        if tag.starts_with('!') {
            continue;
        }
        if let Some(name) = tag.strip_prefix('/') {
            match open.pop() {
                Some(o) if o == name => {}
                o => probs.push(format!("`</{name}>` closes {o:?}")),
            }
            continue;
        }
        let name = tag.split(|c: char| c.is_whitespace()).next().unwrap_or("").to_ascii_lowercase();
        if VOID.contains(&name.as_str()) {
            continue;
        }
        if name == "style" {
            let Some(k) = rest.find("</style>") else {
                probs.push("the style is not closed".to_string());
                break;
            };
            rest = &rest[k + "</style>".len()..];
            continue;
        }
        open.push(name);
    }
    text.push_str(rest);
    probs.extend(open.iter().map(|t| format!("`<{t}>` is not closed")));
    for twice in ["&amp;amp;", "&amp;lt;", "&amp;gt;", "&amp;quot;", "&amp;#"] {
        if html.contains(twice) {
            probs.push(format!("escaped twice: `{twice}`"));
        }
    }
    let mut at = text.as_str();
    while let Some(i) = at.find('&') {
        at = &at[i + 1..];
        if !["amp;", "lt;", "gt;", "quot;", "#"].iter().any(|e| at.starts_with(e)) {
            probs.push(format!("an `&` not escaped: `&{}`", at.chars().take(12).collect::<String>()));
        }
    }
    if text.contains('>') {
        probs.push("a `>` not escaped in the text".to_string());
    }
    probs
}
