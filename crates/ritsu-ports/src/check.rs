//! A language's own `check`, as `ritsu check` runs it on a project (DESIGN 8.3): what the command
//! prints for each unit it checks, a diagnostic at a time, the text and the JSON, so that `ritsu
//! check` says of every file what the language's command says, and adds only the tool's word to
//! the headline (`error[rulec E032]`) and the tool and the file from the root to the JSON.
//!
//! Each language builds these with the same functions its command prints with; `ritsu check`
//! never reads a language's text back.

use ritsu_base::diag::{Diag, Extra, Severity};
use ritsu_base::json::Json;
use ritsu_base::text::Lang;

/// What a language's `check` says of one unit it checks: a file (rulec, koyomi, chobo, geas,
/// dandori), the `.req` files of a project as one (yuen), or a map with its contexts (sakai).
#[derive(Clone, Debug, PartialEq)]
pub struct Checked {
    /// The unit, as the command names it: the file as given, or the paths it was given.
    pub label: String,
    /// What the command prints for it, in order.
    pub parts: Vec<Part>,
    pub verdict: Verdict,
}

/// One piece of what a command prints.
#[derive(Clone, Debug, PartialEq)]
pub enum Part {
    /// A diagnostic.
    Finding(Finding),
    /// Anything else: the line that says a file passes (`ok rules/送料.rule`), a summary, a
    /// report, a claim that holds.
    Text(String),
}

/// One diagnostic, as the language's command prints it.
#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    pub code: String,
    pub severity: Severity,
    /// The file it is about, from the project's root; None for one that names no file.
    pub file: Option<String>,
    pub line: Option<usize>,
    /// Its text, as `<tool> check` prints it: the headline (with `[<code>]` in it) and every line
    /// under it.
    pub text: String,
    /// Its object, as `<tool> check --format json` prints it.
    pub json: Json,
}

/// What the check comes to for the unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Nothing is wrong (warnings and notes may be printed).
    Passes,
    /// Something is wrong: an error, or a claim that does not hold.
    Fails,
    /// The command could not check it at all (a file it cannot read, a command line it refuses):
    /// what its text says, and what the command exits 2 for.
    Unchecked,
}

impl Checked {
    /// A unit the command could not check, with the line it prints for it.
    pub fn unchecked(label: &str, said: String) -> Checked {
        Checked { label: label.to_string(), parts: vec![Part::Text(said)], verdict: Verdict::Unchecked }
    }

    /// The findings, in order.
    pub fn findings(&self) -> impl Iterator<Item = &Finding> {
        self.parts.iter().filter_map(|p| match p {
            Part::Finding(f) => Some(f),
            Part::Text(_) => None,
        })
    }

    /// What the command prints for the unit, as it prints it.
    pub fn text(&self) -> String {
        self.parts
            .iter()
            .map(|p| match p {
                Part::Finding(f) => f.text.as_str(),
                Part::Text(t) => t.as_str(),
            })
            .collect()
    }
}

impl Finding {
    /// A diagnostic of a language that writes them with ritsu-base and prints them as ritsu-base
    /// does (koyomi, yuen, sakai): its text and its JSON in `lang`. `file` is its file from the
    /// project's root.
    pub fn of<X: Extra>(d: &Diag<X>, file: Option<String>, lang: Lang) -> Finding {
        Finding { code: d.code.to_string(), severity: d.severity, file, line: d.line, text: d.render(lang), json: d.to_json(lang) }
    }

    /// The text with the tool's word in its headline: `error[E032]` is `error[rulec E032]`
    /// (DESIGN 8.3). The first `[<code>]` of the text is the headline's.
    pub fn text_of(&self, tool: &str) -> String {
        let tag = format!("[{}]", self.code);
        match self.text.find(&tag) {
            Some(at) => format!("{}[{tool} {}]{}", &self.text[..at], self.code, &self.text[at + tag.len()..]),
            None => self.text.clone(),
        }
    }
}
