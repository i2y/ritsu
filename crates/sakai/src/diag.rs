//! Diagnostics (DESIGN 5.1): a code, a place, what is wrong in English and Japanese, notes,
//! what to write instead, and what is involved — the artifacts, the lines of the `.ctx` and the
//! relationships the diagnostic is about, one a line.
//!
//! The shape follows koyomi's and chobo's `src/diag.rs`: the headline, the line of the source,
//! `= ` notes, then the things involved. The JSON keeps its keys in English whatever the
//! language.

use crate::i18n::{Lang, Text, pad, say, width};
use crate::naming::Name;
use crate::paths::shown;
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

impl Severity {
    /// E… is an error, W… a warning, N… a note.
    pub fn of(code: &str) -> Severity {
        match code.as_bytes().first() {
            Some(b'W') => Severity::Warning,
            Some(b'N') => Severity::Note,
            _ => Severity::Error,
        }
    }

    pub fn word(self, lang: Lang) -> &'static str {
        match (self, lang) {
            (Severity::Error, Lang::En) => "error",
            (Severity::Warning, Lang::En) => "warning",
            (Severity::Note, Lang::En) => "note",
            (Severity::Error, Lang::Ja) => "エラー",
            (Severity::Warning, Lang::Ja) => "警告",
            (Severity::Note, Lang::Ja) => "備考",
        }
    }

    pub fn key(self) -> &'static str {
        self.word(Lang::En)
    }
}

/// One thing a diagnostic is about: the context it belongs to, where it is (a name, or a line of
/// a file), and what it is there.
#[derive(Clone, Debug, PartialEq)]
pub struct Ref {
    pub context: Option<String>,
    pub name: Option<Name>,
    pub file: Option<String>,
    pub line: Option<usize>,
    pub what: Text,
    /// Where sakai read it: `proto import`, or (from stage C) the field of a tool's api.
    pub via: Option<String>,
}

impl Ref {
    /// A line of a file: `ctx/請求.ctx:12`, or an import line of a `.proto`.
    pub fn line(context: Option<&str>, file: &str, line: usize, what: Text) -> Ref {
        Ref { context: context.map(String::from), name: None, file: Some(file.to_string()), line: Some(line), what, via: None }
    }

    /// A named artifact or element.
    pub fn name(context: Option<&str>, name: Name, what: Text) -> Ref {
        Ref { context: context.map(String::from), name: Some(name), file: None, line: None, what, via: None }
    }

    pub fn via(mut self, v: &str) -> Ref {
        self.via = Some(v.to_string());
        self
    }

    /// `ctx/請求.ctx:12`, `proto/x.proto:5`, or the name's text. The place in a file is written from
    /// where sakai was run; a name keeps its path from the root, as names do everywhere (DESIGN 2.4).
    pub fn place(&self) -> String {
        match (&self.file, self.line, &self.name) {
            (Some(f), Some(l), _) => format!("{}:{l}", shown(f)),
            (Some(f), None, _) => shown(f),
            (None, _, Some(n)) => n.text(),
            (None, _, None) => String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Diag {
    pub code: &'static str,
    pub severity: Severity,
    /// The file, as a path from the root.
    pub file: String,
    pub line: Option<usize>,
    pub col: Option<usize>,
    /// The text of that line.
    pub src: Option<String>,
    pub message: Text,
    pub notes: Vec<Text>,
    pub refs: Vec<Ref>,
    /// The line to write in the `.ctx` instead, ready to paste.
    pub fix: Option<String>,
}

impl Diag {
    pub fn new(code: &'static str, file: &str, line: Option<usize>, col: Option<usize>, message: Text) -> Diag {
        Diag { code, severity: Severity::of(code), file: file.to_string(), line, col, src: None, message, notes: vec![], refs: vec![], fix: None }
    }

    /// At a line and column of a file.
    pub fn at(code: &'static str, file: &str, line: usize, col: usize, message: Text) -> Diag {
        Diag::new(code, file, Some(line), Some(col), message)
    }

    /// About a whole file: a tool's api says no line, or the finding is about the file itself.
    pub fn file(code: &'static str, file: &str, message: Text) -> Diag {
        Diag::new(code, file, None, None, message)
    }

    /// The line's text, from the file's source.
    pub fn source(mut self, src: &str) -> Diag {
        if let Some(l) = self.line
            && l > 0
        {
            self.src = src.lines().nth(l - 1).map(|s| s.trim_end().to_string());
        }
        self
    }

    pub fn note(mut self, t: Text) -> Diag {
        self.notes.push(t);
        self
    }

    pub fn with(mut self, r: Ref) -> Diag {
        self.refs.push(r);
        self
    }

    pub fn fix(mut self, f: impl Into<String>) -> Diag {
        self.fix = Some(f.into());
        self
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }

    /// Where it is, the path written from where sakai was run (DESIGN 5.1).
    pub fn place(&self) -> String {
        let f = shown(&self.file);
        match (self.line, self.col) {
            (Some(l), Some(c)) => format!("{f}:{l}:{c}"),
            (Some(l), None) => format!("{f}:{l}"),
            _ => f,
        }
    }

    /// What a person reads.
    pub fn render(&self, lang: Lang) -> String {
        let mut out = format!("{}[{}]: {}: {}\n", self.severity.word(lang), self.code, self.place(), say(&self.message, lang));
        if let (Some(s), Some(l)) = (&self.src, self.line) {
            out.push_str(&format!("  {l:>4} | {s}\n"));
        }
        for n in &self.notes {
            out.push_str(&format!("  = {}\n", say(n, lang)));
        }
        if let Some(f) = &self.fix {
            let head = if lang == Lang::Ja { "直した行" } else { "The line, fixed" };
            out.push_str(&format!("  = {head}: {}\n", f.trim()));
        }
        if !self.refs.is_empty() {
            out.push_str(if lang == Lang::Ja { "  関わるもの:\n" } else { "  involved:\n" });
            let cw = self.refs.iter().map(|r| r.context.as_deref().map(width).unwrap_or(0)).max().unwrap_or(0);
            let pw = self.refs.iter().map(|r| width(&r.place())).max().unwrap_or(0);
            for r in &self.refs {
                // The context's column, when any line has one.
                let c = if cw == 0 { String::new() } else { format!("{}  ", pad(r.context.as_deref().unwrap_or(""), cw)) };
                // What a reference is, is often a line of a `.ctx` or a proto: written as it is.
                let what = r.what.get(lang).to_string();
                let line = if what.is_empty() { format!("{c}{}", r.place()) } else { format!("{c}{}  {what}", pad(&r.place(), pw)) };
                out.push_str(&format!("      {}\n", line.trim_end()));
            }
        }
        out
    }

    /// The same finding for a program (DESIGN 5.1). The files are written as the text writes them,
    /// from where sakai was run; a name keeps its path from the root, as names do in JSON.
    pub fn to_json(&self, lang: Lang) -> Value {
        json!({
            "code": self.code,
            "severity": self.severity.key(),
            "file": shown(&self.file),
            "line": self.line,
            "col": self.col,
            "message": say(&self.message, lang),
            "notes": self.notes.iter().map(|n| say(n, lang)).collect::<Vec<_>>(),
            "references": self.refs.iter().map(|r| json!({
                "context": r.context,
                "name": r.name.as_ref().map(Name::to_json),
                "file": r.file.as_deref().map(shown),
                "line": r.line,
                "what": r.what.get(lang),
                "via": r.via,
            })).collect::<Vec<_>>(),
            "fix": self.fix,
        })
    }
}

pub fn has_errors(diags: &[Diag]) -> bool {
    diags.iter().any(Diag::is_error)
}
