//! Diagnostics: stable codes, Rust-style frames, JSON output (§11).
//!
//! §11 principle 5: the code and the JSON shape are a stable API; the prose may improve.

use std::fmt::Write as _;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

impl Severity {
    fn word(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

/// One source span. Columns are byte offsets within the line, 0-based.
#[derive(Debug, Clone)]
pub struct Span {
    pub line: usize,
    pub col: usize,
    pub len: usize,
}

impl Span {
    pub fn new(line: usize, col: usize, len: usize) -> Self {
        Span { line, col, len }
    }
}

/// A labelled source line inside the frame.
#[derive(Debug, Clone)]
pub struct Marked {
    pub span: Span,
    /// Caret label, shown to the right of the `^^^` run. Empty for a bare underline.
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct Diag {
    /// Key that `--diff-base` uses to decide whether two findings are the same. Built from
    /// the canonical form of the cell rather than the line number, so a realignment by
    /// `rulec fmt` does not change it. Never displayed.
    pub key: Option<String>,
    pub severity: Severity,
    /// Stable code, e.g. "E101".
    pub code: &'static str,
    /// §11 principle 1: business words only, no IR vocabulary.
    pub title: String,
    /// `--> path:line context`
    pub where_: String,
    pub marks: Vec<Marked>,
    /// Free lines under the frame: witness, cause, hint (§11 principles 2 and 3).
    pub notes: Vec<String>,
}

impl Diag {
    pub fn error(code: &'static str, title: impl Into<String>) -> Self {
        Diag {
            key: None,
            severity: Severity::Error,
            code,
            title: title.into(),
            where_: String::new(),
            marks: Vec::new(),
            notes: Vec::new(),
        }
    }

    pub fn warning(code: &'static str, title: impl Into<String>) -> Self {
        Diag {
            severity: Severity::Warning,
            ..Self::error(code, title)
        }
    }

    pub fn at(mut self, w: impl Into<String>) -> Self {
        self.where_ = w.into();
        self
    }

    pub fn mark(mut self, span: Span, label: impl Into<String>) -> Self {
        self.marks.push(Marked { span, label: label.into() });
        self
    }

    pub fn note(mut self, n: impl Into<String>) -> Self {
        let n = n.into();
        if !n.is_empty() {
            self.notes.push(n);
        }
        self
    }

    pub fn key(mut self, k: impl Into<String>) -> Self {
        self.key = Some(k.into());
        self
    }
}

/// Display width, counting CJK and fullwidth punctuation as 2 columns so the
/// carets land under the right glyphs in a terminal.
pub fn width(s: &str) -> usize {
    s.chars().map(char_width).sum()
}

pub fn char_width(c: char) -> usize {
    let u = c as u32;
    // The ranges a rule file actually reaches: CJK, kana, fullwidth forms,
    // and the separators the syntax uses (・ → ≦).
    let wide = (0x1100..=0x115F).contains(&u)
        || (0x2E80..=0xA4CF).contains(&u)
        || (0xAC00..=0xD7A3).contains(&u)
        || (0xF900..=0xFAFF).contains(&u)
        || (0xFE30..=0xFE6F).contains(&u)
        || (0xFF00..=0xFF60).contains(&u)
        || (0xFFE0..=0xFFE6).contains(&u);
    if wide { 2 } else { 1 }
}

/// Render one diagnostic in the frame shape §11 fixes.
pub fn render(d: &Diag, src_lines: &[String]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{}[{}]: {}", d.severity.word(), d.code, d.title);
    if !d.where_.is_empty() {
        let _ = writeln!(out, "  --> {}", d.where_);
    }

    // Gutter width from the largest line number we are about to print.
    let maxline = d.marks.iter().map(|m| m.span.line).max().unwrap_or(0);
    let gw = maxline.to_string().len().max(1);

    if !d.marks.is_empty() {
        let _ = writeln!(out, "{:>w$} |", "", w = gw);
        for m in &d.marks {
            let Some(text) = src_lines.get(m.span.line.saturating_sub(1)) else { continue };
            let _ = writeln!(out, "{:>w$} | {}", m.span.line, text, w = gw);
            let pad = width(&text[..m.span.col.min(text.len())]);
            let carets = "^".repeat(width(&text[m.span.col.min(text.len())
                ..(m.span.col + m.span.len).min(text.len())])
                .max(1));
            let _ = writeln!(
                out,
                "{:>w$} | {}{}{}{}",
                "",
                " ".repeat(pad),
                carets,
                if m.label.is_empty() { "" } else { " " },
                m.label,
                w = gw
            );
        }
        let _ = writeln!(out, "{:>w$} |", "", w = gw);
    }

    for n in &d.notes {
        let _ = writeln!(out, " {}", n);
    }
    out
}

/// `--format json`: one object per diagnostic, shaped for GitHub annotations (§11 principle 6).
pub fn render_json(d: &Diag, path: &str) -> String {
    let esc = |s: &str| {
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
    };
    let line = d.marks.first().map(|m| m.span.line).unwrap_or(1);
    let col = d.marks.first().map(|m| m.span.col + 1).unwrap_or(1);
    format!(
        r#"{{"severity":"{}","code":"{}","file":"{}","line":{},"column":{},"title":"{}","notes":[{}]}}"#,
        d.severity.word(),
        d.code,
        esc(path),
        line,
        col,
        esc(&d.title),
        d.notes
            .iter()
            .map(|n| format!("\"{}\"", esc(n)))
            .collect::<Vec<_>>()
            .join(",")
    )
}
