//! Diagnostics (DESIGN 6.1): a code, a place, the message in both languages, notes, and what
//! led there — the diff of what changed, the requirements of a cycle, the versions around a
//! gap, the names to pick from — and the fix, down to the line to write or the command to run.
//!
//! The shape follows koyomi's and chobo's `src/diag.rs`: the headline, the source line, `= `
//! notes, then what led there. The JSON keeps its keys in English whatever the language.

use crate::i18n::{Lang, Text, say};
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
}

/// One line of a diff: `-` gone, `+` added, ` ` the same, `@` where a hunk starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffLine {
    pub op: char,
    pub text: String,
}

/// One step of what led to a diagnostic: a requirement of a cycle, a version and its period.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChainItem {
    pub text: Text,
    /// Where it is, as the diagnostic names files, with its line; empty when nowhere.
    pub file: String,
    pub line: usize,
}

/// What fixes it: a line to write in the `.req`, or a command to run once a person has read
/// what the diagnostic shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fix {
    Line(String),
    Command(Text),
}

#[derive(Clone, Debug)]
pub struct Diag {
    pub code: &'static str,
    pub severity: Severity,
    /// The file, as the command line named it (a directory joined with what was found in it).
    pub file: String,
    /// The same file from the root (DESIGN 2.2), for the JSON.
    pub rel: String,
    pub line: usize,
    pub col: usize,
    /// The text of that line.
    pub src: Option<String>,
    pub message: Text,
    pub notes: Vec<Text>,
    /// The heading over `diff`, and the diff.
    pub diff_head: Option<Text>,
    pub diff: Vec<DiffLine>,
    /// The heading over `chain`, and the chain.
    pub chain_head: Option<Text>,
    pub chain: Vec<ChainItem>,
    pub candidates: Vec<String>,
    pub fix: Option<Fix>,
}

impl Diag {
    fn new(code: &'static str, severity: Severity, file: &str, rel: &str, line: usize, col: usize, message: Text) -> Diag {
        Diag {
            code,
            severity,
            file: file.to_string(),
            rel: rel.to_string(),
            line,
            col,
            src: None,
            message,
            notes: vec![],
            diff_head: None,
            diff: vec![],
            chain_head: None,
            chain: vec![],
            candidates: vec![],
            fix: None,
        }
    }

    pub fn error(code: &'static str, file: &str, rel: &str, line: usize, col: usize, message: Text) -> Diag {
        Diag::new(code, Severity::Error, file, rel, line, col, message)
    }

    pub fn warning(code: &'static str, file: &str, rel: &str, line: usize, col: usize, message: Text) -> Diag {
        Diag::new(code, Severity::Warning, file, rel, line, col, message)
    }

    /// The line's text, from the file's source.
    pub fn source(mut self, src: &str) -> Diag {
        if self.line > 0 {
            self.src = src.lines().nth(self.line - 1).map(|s| s.trim_end().to_string());
        }
        self
    }

    pub fn note(mut self, t: Text) -> Diag {
        self.notes.push(t);
        self
    }

    pub fn fix_line(mut self, f: impl Into<String>) -> Diag {
        self.fix = Some(Fix::Line(f.into()));
        self
    }

    pub fn fix_command(mut self, f: Text) -> Diag {
        self.fix = Some(Fix::Command(f));
        self
    }

    pub fn diff(mut self, head: Text, lines: Vec<DiffLine>) -> Diag {
        self.diff_head = Some(head);
        self.diff = lines;
        self
    }

    pub fn chain(mut self, head: Text, items: Vec<ChainItem>) -> Diag {
        self.chain_head = Some(head);
        self.chain = items;
        self
    }

    pub fn candidates(mut self, c: Vec<String>) -> Diag {
        self.candidates = c;
        self
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }

    /// What a person reads.
    pub fn render(&self, lang: Lang) -> String {
        let kind = match (self.severity, lang) {
            (Severity::Error, Lang::En) => "error",
            (Severity::Warning, Lang::En) => "warning",
            (Severity::Error, Lang::Ja) => "エラー",
            (Severity::Warning, Lang::Ja) => "警告",
        };
        let place = if self.line > 0 { format!("{}:{}:{}", self.file, self.line, self.col) } else { self.file.clone() };
        let mut out = format!("{kind}[{}]: {place}: {}\n", self.code, say(&self.message, lang));
        if let Some(s) = &self.src {
            out.push_str(&format!("  {:>4} | {}\n", self.line, s));
        }
        for n in &self.notes {
            out.push_str(&format!("  = {}\n", say(n, lang)));
        }
        if let Some(h) = &self.chain_head {
            out.push_str(&format!("  {}:\n", h.get(lang)));
            for c in &self.chain {
                let at = if c.line > 0 { format!(" ({}:{})", c.file, c.line) } else { String::new() };
                let at = if lang == Lang::Ja && c.line > 0 { format!("（{}:{}）", c.file, c.line) } else { at };
                out.push_str(&format!("      {}{at}\n", c.text.get(lang)));
            }
        }
        if let Some(h) = &self.diff_head {
            out.push_str(&format!("  {}:\n", h.get(lang)));
            for d in &self.diff {
                let l = if d.op == '@' { format!("      {}", d.text) } else { format!("      {} {}", d.op, d.text) };
                out.push_str(l.trim_end());
                out.push('\n');
            }
        }
        if !self.candidates.is_empty() {
            let head = if lang == Lang::Ja { "候補" } else { "Did you mean" };
            out.push_str(&format!("  = {head}: {}\n", self.candidates.join(", ")));
        }
        match &self.fix {
            Some(Fix::Line(l)) => {
                let head = if lang == Lang::Ja { "直した行" } else { "The line, fixed" };
                out.push_str(&format!("  = {head}: {}\n", l.trim()));
            }
            Some(Fix::Command(c)) => {
                let head = if lang == Lang::Ja { "確かめたら" } else { "Once a person has looked" };
                out.push_str(&format!("  = {head}: {}\n", c.get(lang)));
            }
            None => {}
        }
        out
    }

    pub fn to_json(&self, lang: Lang) -> Value {
        json!({
            "code": self.code,
            "severity": match self.severity { Severity::Error => "error", Severity::Warning => "warning" },
            "file": self.rel,
            "line": self.line,
            "col": self.col,
            "message": say(&self.message, lang),
            "notes": self.notes.iter().map(|n| say(n, lang)).collect::<Vec<_>>(),
            "diff": self.diff.iter().map(|d| json!({
                "op": if d.op == '@' { "@@".to_string() } else { d.op.to_string() },
                "text": d.text,
            })).collect::<Vec<_>>(),
            "chain": self.chain.iter().map(|c| json!({
                "text": c.text.get(lang),
                "file": c.file,
                "line": c.line,
            })).collect::<Vec<_>>(),
            "candidates": self.candidates,
            "fix": match &self.fix {
                Some(Fix::Line(l)) => json!(l.trim()),
                Some(Fix::Command(c)) => json!(c.get(lang)),
                None => Value::Null,
            },
        })
    }
}

pub fn has_errors(diags: &[Diag]) -> bool {
    diags.iter().any(|d| d.is_error())
}

pub fn count(diags: &[Diag]) -> (usize, usize) {
    let e = diags.iter().filter(|d| d.is_error()).count();
    (e, diags.len() - e)
}
