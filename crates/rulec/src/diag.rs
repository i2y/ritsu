//! Diagnostics: stable codes, Rust-style frames, JSON output (§11).
//!
//! §11 principle 5: the code and the JSON shape are a stable API; the prose may improve.
//!
//! A diagnostic carries **two** descriptions of the same finding: the prose a person reads,
//! and the structured fields a program acts on (`table`, `row`, `witness`, `rows`, `fix`).
//! The structured part is filled in where the diagnostic is raised, never parsed back out of
//! the sentence — a reader that has to take the prose apart is a reader that breaks the next
//! time the wording improves, which principle 5 explicitly allows.

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

/// A value inside a witness. The wire shape is the one the vectors use (§10.2): an integer
/// in the canonical unit, an enum value by name, a boolean, a date as `YYYY-MM-DD`.
#[derive(Debug, Clone, PartialEq)]
pub enum WVal {
    Int(i128),
    Str(String),
    Bool(bool),
}

impl WVal {
    /// As JSON. Language independent — a witness is data, not prose.
    pub fn json(&self) -> String {
        match self {
            WVal::Int(n) => n.to_string(),
            WVal::Str(s) => crate::json::quote(s),
            WVal::Bool(b) => b.to_string(),
        }
    }
    /// As it is written in a `.rule` cell or in a terse line.
    pub fn text(&self) -> String {
        match self {
            WVal::Int(n) => n.to_string(),
            WVal::Str(s) => s.clone(),
            WVal::Bool(b) => (if *b { crate::kw::TRUE } else { crate::kw::FALSE }).to_string(),
        }
    }
}

/// A concrete assignment of values that exhibits the finding (§11 principle 2).
///
/// It carries **only assignments** — inputs, the outputs they produce, and what an example
/// said they should produce. An interval or a spread over rounding modes is not an
/// assignment and stays in the prose (and, where it is actionable, in `fix.text`), so that a
/// witness always has one shape: something a caller can hand straight back as a vector or a
/// fixture (§10.2).
#[derive(Debug, Clone, Default)]
pub struct Witness {
    pub inputs: Vec<(String, WVal)>,
    /// What the rule actually produces for those inputs.
    pub outputs: Vec<(String, WVal)>,
    /// What the source said it should produce. Only E107 has one.
    pub expected: Vec<(String, WVal)>,
}

impl Witness {
    pub fn is_empty(&self) -> bool {
        self.inputs.is_empty() && self.outputs.is_empty() && self.expected.is_empty()
    }
}

/// A row of a table, named the way a person names it. `row` is 1-based, as it is printed.
#[derive(Debug, Clone)]
pub struct RowRef {
    pub table: String,
    pub row: usize,
}

/// What kind of edit removes the finding. The set is closed so a caller can switch on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixKind {
    AddRow,
    RemoveRow,
    AddRounding,
    AddRange,
    WidenRange,
    AddAlias,
    MarkDefault,
    MarkContractOnly,
    ChangePolicy,
    AddExpected,
    /// The pin of a source, as `rulec source pin` would write it: a `source` line with its
    /// digest, or one `  <fragment> sha256:…` line (§15.68).
    PinSource,
    /// The cell rewritten with one boundary's strictness toggled, so that the boundary value
    /// falls on the side the copy puts it on: `<60cm` becomes `<=60cm`, the direction left
    /// alone because it is the table's geometry and not the copy's to decide (§15.124).
    FlipBound,
    /// The contract narrowed so that what it lets through is what the rule takes: the
    /// Protovalidate option or JSON Schema keywords to write on the field, as they are written
    /// there (§15.132).
    NarrowContract,
    /// No single mechanical edit is right. The reason is in the notes.
    None,
}

impl FixKind {
    pub fn word(self) -> &'static str {
        match self {
            FixKind::AddRow => "add_row",
            FixKind::RemoveRow => "remove_row",
            FixKind::AddRounding => "add_rounding",
            FixKind::AddRange => "add_range",
            FixKind::WidenRange => "widen_range",
            FixKind::AddAlias => "add_alias",
            FixKind::MarkDefault => "mark_default",
            FixKind::MarkContractOnly => "mark_contract_only",
            FixKind::ChangePolicy => "change_policy",
            FixKind::AddExpected => "add_expected",
            FixKind::PinSource => "pin_source",
            FixKind::FlipBound => "flip_bound",
            FixKind::NarrowContract => "narrow_contract",
            FixKind::None => "none",
        }
    }
}

/// §11 principle 3 as data: the hint says the rewritten form, and this is that form in a
/// shape a program can paste. `text` is **language independent and carries no prose** — the
/// caveats (a rounding direction is a business decision, an amount has to come from the
/// written rule) live in the notes, which are prose and do change with the language.
#[derive(Debug, Clone)]
pub struct Fix {
    pub kind: FixKind,
    pub text: Option<String>,
}

impl Default for Fix {
    fn default() -> Fix {
        Fix { kind: FixKind::None, text: None }
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
    /// The table the finding is about, when it is about one.
    pub table: Option<String>,
    /// The row of that table, 1-based, when a single row is at fault.
    pub row: Option<usize>,
    /// Every row that takes part (both sides of an overlap, the rows an example fired).
    pub rows: Vec<RowRef>,
    pub witness: Witness,
    pub fix: Fix,
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
            table: None,
            row: None,
            rows: Vec::new(),
            witness: Witness::default(),
            fix: Fix::default(),
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

    /// A note that is only sometimes there, so the caller does not have to branch.
    pub fn maybe_note(self, n: Option<String>) -> Self {
        match n {
            Some(n) => self.note(n),
            None => self,
        }
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

    /// The table (and optionally the row) the finding is about.
    pub fn table(mut self, t: impl Into<String>) -> Self {
        self.table = Some(t.into());
        self
    }

    pub fn row(mut self, r: usize) -> Self {
        self.row = Some(r);
        self
    }

    /// One more row that takes part in the finding.
    pub fn rowref(mut self, table: impl Into<String>, row: usize) -> Self {
        self.rows.push(RowRef { table: table.into(), row });
        self
    }

    /// One input of the witness.
    pub fn win(mut self, name: impl Into<String>, v: WVal) -> Self {
        self.witness.inputs.push((name.into(), v));
        self
    }

    /// One output of the witness: what the rule really produces.
    pub fn wout(mut self, name: impl Into<String>, v: WVal) -> Self {
        self.witness.outputs.push((name.into(), v));
        self
    }

    /// The whole witness at once.
    pub fn wit(mut self, w: Witness) -> Self {
        self.witness = w;
        self
    }

    /// §11 principle 3 as data. `text` is the rewritten form, ready to paste.
    pub fn fix(mut self, kind: FixKind, text: impl Into<String>) -> Self {
        self.fix = Fix { kind, text: Some(text.into()) };
        self
    }

    /// A kind of edit with no single text (deleting a row, say).
    pub fn fix_kind(mut self, kind: FixKind) -> Self {
        self.fix = Fix { kind, text: None };
        self
    }

    /// `witness: あて先 = 山梨県, サイズ = S60` — the one line `--terse` keeps.
    pub fn witness_line(&self) -> Option<String> {
        if self.witness.is_empty() {
            return None;
        }
        let show = |ps: &[(String, WVal)]| {
            ps.iter().map(|(n, v)| format!("{n} = {}", v.text())).collect::<Vec<_>>().join(", ")
        };
        let mut s = show(&self.witness.inputs);
        if !self.witness.outputs.is_empty() {
            if !s.is_empty() {
                s.push(' ');
            }
            s.push_str(&format!("-> {}", show(&self.witness.outputs)));
        }
        if !self.witness.expected.is_empty() {
            s.push_str(&tr!(
                "（期待 {}）",
                " (expected {})",
                show(&self.witness.expected)
            ));
        }
        Some(s)
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
        || (0xFFE0..=0xFFE6).contains(&u)
        // ℃ and ℉ are "ambiguous" to Unicode, but every monospace face that has
        // them at all draws them two columns wide, a Latin one (Noto Sans Mono)
        // included; counted as one, a table with `<=-15℃` in it is aligned for no
        // font anyone reads it in.
        || u == 0x2103
        || u == 0x2109;
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

/// `--terse`: the first line, the position, and the witness — nothing else. For a caller
/// that reads a hundred findings and only needs to know what and where; `rulec explain
/// <code>` has the rest, and `check` prints that pointer once at the end of the run.
pub fn render_terse(d: &Diag) -> String {
    let mut out = format!("{}[{}]: {}\n", d.severity.word(), d.code, d.title);
    if !d.where_.is_empty() {
        let _ = writeln!(out, "  --> {}", d.where_);
    }
    if let Some(w) = d.witness_line() {
        let _ = writeln!(out, "  {}", tr!("その入力: {w}", "witness: {w}"));
    }
    out
}

/// The one line `--terse` prints at the end of a run, after every finding.
pub fn terse_footer() -> String {
    tr!(
        "details: rulec explain <code>\n",
        "details: rulec explain <code>\n"
    )
}

/// `--format json`, version 2 (§11 principle 6).
///
/// Every v1 field is still here and still means what it meant, so a reader written against
/// v1 keeps working; `v` says which version wrote the line. What v2 adds is the finding as
/// **data** — where it is, which rows take part, a witness whose values are in the canonical
/// unit, and the rewritten form that removes it — so that acting on a diagnostic no longer
/// means taking a sentence apart.
pub fn render_json(d: &Diag, path: &str) -> String {
    let line = d.marks.first().map(|m| m.span.line).unwrap_or(1);
    let col = d.marks.first().map(|m| m.span.col + 1).unwrap_or(1);

    let where_ = crate::json::Obj::new()
        .str("file", path)
        .int("line", line as i128)
        .int("column", col as i128)
        .opt_str("table", d.table.as_deref())
        .opt_raw("row", d.row.map(|r| r.to_string()))
        .finish();

    let spans: Vec<String> = d
        .marks
        .iter()
        .map(|m| {
            crate::json::Obj::new()
                .int("line", m.span.line as i128)
                .int("column", (m.span.col + 1) as i128)
                .int("length", m.span.len as i128)
                .str("label", &m.label)
                .finish()
        })
        .collect();

    let pairs = |ps: &[(String, WVal)]| -> String {
        let mut o = crate::json::Obj::new();
        for (n, v) in ps {
            o = o.raw(n, v.json());
        }
        o.finish()
    };
    let mut witness = crate::json::Obj::new();
    if !d.witness.inputs.is_empty() {
        witness = witness.raw("inputs", pairs(&d.witness.inputs));
    }
    if !d.witness.outputs.is_empty() {
        witness = witness.raw("outputs", pairs(&d.witness.outputs));
    }
    if !d.witness.expected.is_empty() {
        witness = witness.raw("expected", pairs(&d.witness.expected));
    }

    let rows: Vec<String> = d
        .rows
        .iter()
        .map(|r| crate::json::Obj::new().str("table", &r.table).int("row", r.row as i128).finish())
        .collect();

    let fix = crate::json::Obj::new()
        .str("kind", d.fix.kind.word())
        .opt_str("text", d.fix.text.as_deref())
        .finish();

    crate::json::Obj::new()
        .int("v", 2)
        .str("severity", d.severity.word())
        .str("code", d.code)
        .str("file", path)
        .int("line", line as i128)
        .int("column", col as i128)
        .str("title", &d.title)
        .raw("notes", crate::json::strs(&d.notes))
        .raw("where", where_)
        .raw("spans", crate::json::arr(&spans))
        .raw("witness", witness.finish())
        .raw("rows", crate::json::arr(&rows))
        .raw("fix", fix)
        .opt_str("key", d.key.as_deref())
        .finish()
}
