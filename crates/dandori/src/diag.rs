//! Diagnostics: a code, a place, the message in Japanese and English, and, when the
//! problem only shows along some run of the workflow, the shortest such run.
//!
//! A message is written `tr!("日本語", "English")` (ritsu-base's [`Text`]), and the language a
//! run prints in is ritsu-base's [`Lang`]: `--lang`, then `DANDORI_LANG`, then `RITSU_LANG`, then
//! English (ritsu's DESIGN 4.1).

use serde_json::{json, Value};

pub use ritsu_base::text::{Lang, Text};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// One step of a run: where it happened and what happened, in both languages.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Step {
    pub line: usize,
    pub en: String,
    pub ja: String,
    /// the place in the flow, for `doc` to draw the run on it; None for what happens outside it
    pub at: Option<At>,
}

/// Where in the flow a step of a run happened.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum At {
    /// a statement, by its site
    Stmt(usize),
    /// a match takes its arm, by the arm's place in the match
    Arm(usize, usize),
    /// a call's error goes to its handler, by the handler's place under the call
    Handler(usize, usize),
    /// a call fails with an error nothing at the call takes
    Fails(usize),
    /// a cancellation stops the run at a call or a wait
    Cancelled(usize),
    OnFailure,
    OnCancel,
    /// the flow runs to its end, and so do `on failure` and `on cancel`
    FlowEnd,
    OnFailureEnd,
    OnCancelEnd,
}

impl Step {
    pub fn new(line: usize, what: Text) -> Step {
        Step { line, en: what.en, ja: what.ja, at: None }
    }

    pub fn at(mut self, at: At) -> Step {
        self.at = Some(at);
        self
    }
}

#[derive(Clone, Debug)]
pub struct Diag {
    pub code: &'static str,
    pub severity: Severity,
    pub line: usize,
    pub col: usize,
    pub en: String,
    pub ja: String,
    pub notes: Vec<Text>,
    pub path: Vec<Step>,
    /// The name it says is declared nowhere, with the kind of name (`task`, `type`, `value`): the
    /// findings of one name are said once, at the first ([`once`]).
    pub same: Option<(String, String)>,
}

impl Diag {
    pub fn error(code: &'static str, line: usize, col: usize, message: Text) -> Diag {
        Diag { code, severity: Severity::Error, line, col, en: message.en, ja: message.ja, notes: vec![], path: vec![], same: None }
    }

    pub fn warning(code: &'static str, line: usize, col: usize, message: Text) -> Diag {
        Diag { code, severity: Severity::Warning, line, col, en: message.en, ja: message.ja, notes: vec![], path: vec![], same: None }
    }

    pub fn note(mut self, note: Text) -> Diag {
        self.notes.push(note);
        self
    }

    /// The name of a kind the finding says is declared nowhere ([`once`]).
    pub fn same_in(mut self, kind: &str, name: impl Into<String>) -> Diag {
        self.same = Some((kind.to_string(), name.into()));
        self
    }

    pub fn with_path(mut self, path: Vec<Step>) -> Diag {
        self.path = path;
        self
    }

    pub fn message(&self, lang: Lang) -> &str {
        match lang {
            Lang::En => &self.en,
            Lang::Ja => &self.ja,
        }
    }

    /// The text a person reads: the headline, the source line, the notes, and the run. A key on the
    /// source line is shown masked (ritsu-base's `secrets::mask`), whichever diagnostic shows the
    /// line: a key in the file must not reach the logs through the line (DESIGN 1.18).
    pub fn render(&self, file: &str, src: &str, lang: Lang) -> String {
        let kind = match (self.severity, lang) {
            (Severity::Error, Lang::En) => "error",
            (Severity::Warning, Lang::En) => "warning",
            (Severity::Error, Lang::Ja) => "エラー",
            (Severity::Warning, Lang::Ja) => "警告",
        };
        let mut out = format!("{kind}[{}]: {file}:{}:{}: {}\n", self.code, self.line, self.col, self.message(lang));
        if let Some(text) = src.lines().nth(self.line.saturating_sub(1)) {
            if self.line > 0 {
                out.push_str(&format!("  {:>4} | {}\n", self.line, ritsu_base::secrets::mask(text)));
            }
        }
        for n in &self.notes {
            out.push_str(&format!("  = {}\n", n.get(lang)));
        }
        if !self.path.is_empty() {
            out.push_str(if lang == Lang::Ja { "  そうなる例:\n" } else { "  the run that gets there:\n" });
            for s in &self.path {
                let t = if lang == Lang::Ja { &s.ja } else { &s.en };
                if s.line > 0 {
                    out.push_str(&format!("    {:>4}  {t}\n", s.line));
                } else {
                    out.push_str(&format!("          {t}\n"));
                }
            }
        }
        out
    }

    pub fn to_json(&self, lang: Lang) -> Value {
        json!({
            "code": self.code,
            "severity": match self.severity { Severity::Error => "error", Severity::Warning => "warning" },
            "line": self.line,
            "col": self.col,
            "message": self.message(lang),
            "notes": self.notes.iter().map(|n| n.get(lang)).collect::<Vec<_>>(),
            "path": self.path.iter().map(|s| json!({
                "line": s.line,
                "step": if lang == Lang::Ja { s.ja.clone() } else { s.en.clone() },
            })).collect::<Vec<_>>(),
        })
    }
}

/// A name declared nowhere is said once in a file (ritsu's DESIGN 8.3): of the findings that say
/// so of one name of one kind ([`Diag::same`]), the first is kept, with a note of the other lines
/// the name is written on, and the others are dropped. The reader fixes the declaration, or the
/// lines the note lists.
pub fn once(diags: &mut Vec<Diag>) {
    let mut order: Vec<usize> = (0..diags.len()).collect();
    order.sort_by_key(|&i| (diags[i].line, diags[i].col));
    let mut firsts: Vec<((String, String), usize, Vec<usize>)> = Vec::new();
    let mut drop = vec![false; diags.len()];
    for i in order {
        let Some(key) = diags[i].same.clone() else { continue };
        match firsts.iter_mut().find(|(k, ..)| *k == key) {
            Some((_, _, lines)) => {
                drop[i] = true;
                lines.push(diags[i].line);
            }
            None => firsts.push((key, i, Vec::new())),
        }
    }
    for ((_, name), i, mut lines) in firsts {
        lines.retain(|l| *l != diags[i].line);
        lines.sort_unstable();
        lines.dedup();
        if !lines.is_empty() {
            diags[i].notes.push(ritsu_base::diag::unstopped(ritsu_base::diag::written_too(&name, &lines)));
        }
    }
    let mut k = 0;
    diags.retain(|_| {
        k += 1;
        !drop[k - 1]
    });
}

pub fn has_errors(diags: &[Diag]) -> bool {
    diags.iter().any(|d| d.severity == Severity::Error)
}
