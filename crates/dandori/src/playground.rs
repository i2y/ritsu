//! The playground in the browser: `check`, `build` and `doc` on the file the reader edits, with
//! the rest read from a bundle recorded from the examples (crate::sources). Each answers with what
//! the command prints for that file, word for word, and what it writes; and `rules` shows the rules
//! the flow calls. The page runs this compiled to wasm32 (crate::wasm), and tests/playground.rs
//! holds the two to the binary.
//!
//! Each answer is made by a function that reads from whatever crate::sources reads from now (the
//! `*_here` ones): the page reads the bundle, and the tests read the disk and rulec through the
//! same functions, to record the bundle and to hold it to what they read.

use crate::commands;
use crate::diag::{Diag, Lang, Severity};
use crate::sources::{self, Bundle, Playground};
use serde_json::{json, Value};
use std::path::Path;
use std::rc::Rc;

/// What the page asks of a file: its path in the bundle, and its text as the reader has it.
pub struct Request {
    pub path: String,
    pub source: String,
    pub lang: Lang,
}

impl Request {
    pub fn from_json(v: &Value) -> Result<Request, String> {
        let text = |k: &str| v[k].as_str().map(str::to_string).ok_or_else(|| format!("the request has no `{k}`"));
        Ok(Request { path: text("path")?, source: text("source")?, lang: if v["lang"] == "ja" { Lang::Ja } else { Lang::En } })
    }

    fn reading<R>(&self, bundle: &Rc<Bundle>, f: impl FnOnce() -> R) -> R {
        let s = Playground { bundle: bundle.clone(), path: sources::key(Path::new(&self.path)), text: self.source.clone(), lang: self.lang };
        sources::with(Rc::new(s), f)
    }

    fn render(&self, diags: &[Diag]) -> String {
        commands::render(diags, &self.path, &self.source, self.lang)
    }
}

fn counts(diags: &[Diag]) -> (usize, usize) {
    let errors = diags.iter().filter(|d| d.severity == Severity::Error).count();
    (errors, diags.len() - errors)
}

/// `dandori check <path>`: `{"errors", "warnings", "text"}`, where `text` is what it prints.
pub fn check(bundle: &Rc<Bundle>, r: &Request) -> Value {
    r.reading(bundle, || check_here(r))
}

pub fn check_here(r: &Request) -> Value {
    let checked = match crate::check::check_file(Path::new(&r.path)) {
        Ok((_, c)) => c,
        Err(msg) => return json!({ "error": msg }),
    };
    let (errors, warnings) = counts(&checked.diags);
    let mut text = r.render(&checked.diags);
    if checked.model.is_some() {
        text.push_str(&commands::passed(&r.path, warnings, r.lang));
    }
    json!({ "errors": errors, "warnings": warnings, "text": text })
}

/// `dandori build <path> --target <target>`: `{"ok", "errors", "warnings", "text", "files"}`, where
/// `text` is what it prints of the check and of what the platform cannot do, and `files` what it
/// writes, by their paths under `--out`. A flow that does not pass check is not built.
pub fn build(bundle: &Rc<Bundle>, r: &Request, target: &str) -> Value {
    r.reading(bundle, || build_here(r, target))
}

pub fn build_here(r: &Request, target: &str) -> Value {
    let checked = match crate::check::check_file(Path::new(&r.path)) {
        Ok((_, c)) => c,
        Err(msg) => return json!({ "error": msg }),
    };
    let (errors, warnings) = counts(&checked.diags);
    let mut text = r.render(&checked.diags);
    let mut files = Vec::new();
    if let Some(m) = &checked.model {
        match commands::build(m, target) {
            Some(Ok(written)) => files = written.into_iter().map(|(path, body)| json!({ "path": path, "body": body })).collect(),
            Some(Err(refused)) => text.push_str(&r.render(&refused)),
            None => return json!({ "error": format!("there is no target `{target}`") }),
        }
    }
    json!({ "ok": !files.is_empty(), "errors": errors, "warnings": warnings, "text": text, "files": files })
}

/// `dandori doc <path>`, as HTML and as Markdown: `{"drawn", "errors", "warnings", "text", "html",
/// "markdown"}`. A flow whose names and types resolve is drawn even when check finds errors, with
/// the runs of the errors on it.
pub fn doc(bundle: &Rc<Bundle>, r: &Request) -> Value {
    r.reading(bundle, || doc_here(r))
}

pub fn doc_here(r: &Request) -> Value {
    let (src, drawn) = match crate::check::drawable(Path::new(&r.path)) {
        Ok(x) => x,
        Err(msg) => return json!({ "error": msg }),
    };
    let (errors, warnings) = counts(&drawn.diags);
    let text = r.render(&drawn.diags);
    let Some(m) = &drawn.model else {
        return json!({ "drawn": false, "errors": errors, "warnings": warnings, "text": text, "html": "", "markdown": "" });
    };
    let input = crate::doc::Input { m, src: &src, file: &r.path, facts: &drawn.facts, diags: &drawn.diags, lang: r.lang };
    json!({
        "drawn": true,
        "errors": errors,
        "warnings": warnings,
        "text": text,
        "html": crate::doc::html(&input),
        "markdown": crate::doc::markdown(&input),
    })
}

/// The rules the flow calls: `{"errors", "warnings", "text", "rules"}`, each rule with the name the
/// flow gives it, rulec's name and version, its file, the file's text as it is, and the page
/// `rulec doc --format html` renders for it (or why there is none). The rules are those of a flow
/// whose names and types resolve, as `doc` draws; `text` is what check says, as `doc` prints it.
pub fn rules(bundle: &Rc<Bundle>, r: &Request) -> Value {
    r.reading(bundle, || rules_here(r))
}

pub fn rules_here(r: &Request) -> Value {
    let (_, drawn) = match crate::check::drawable(Path::new(&r.path)) {
        Ok(x) => x,
        Err(msg) => return json!({ "error": msg }),
    };
    let (errors, warnings) = counts(&drawn.diags);
    let text = r.render(&drawn.diags);
    let rules: Vec<Value> = drawn
        .model
        .iter()
        .flat_map(|m| &m.rules)
        .map(|ru| {
            let page = sources::rulec_doc(&ru.info.path, true, r.lang);
            json!({
                "name": ru.name,
                "rule": ru.info.rule,
                "version": ru.info.version,
                "file": sources::key(&ru.info.path),
                "source": sources::read(&ru.info.path).unwrap_or_default(),
                "page": page.as_ref().ok(),
                "error": page.as_ref().err(),
            })
        })
        .collect();
    json!({ "errors": errors, "warnings": warnings, "text": text, "rules": rules })
}

/// One request of the page, as the module takes it and answers: `what` is check, build, doc or
/// rules, and the request is `{"path", "source", "lang"}`, with `"target"` for a build.
pub fn answer(bundle: &Rc<Bundle>, what: &str, request: &str) -> String {
    let v: Value = match serde_json::from_str(request) {
        Ok(v) => v,
        Err(e) => return json!({ "error": format!("the request is not JSON: {e}") }).to_string(),
    };
    let r = match Request::from_json(&v) {
        Ok(r) => r,
        Err(e) => return json!({ "error": e }).to_string(),
    };
    match what {
        "check" => check(bundle, &r),
        "build" => build(bundle, &r, v["target"].as_str().unwrap_or_default()),
        "doc" => doc(bundle, &r),
        "rules" => rules(bundle, &r),
        _ => json!({ "error": format!("there is no command `{what}`") }),
    }
    .to_string()
}
