//! Diagnostics: a code, a place, the message in English and Japanese, and, when the
//! problem only shows along some run of the workflow, the shortest such run.

use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Ja,
}

impl Lang {
    /// `--lang` wins, then `DANDORI_LANG`, then English.
    pub fn pick(flag: Option<&str>) -> Lang {
        let env = std::env::var("DANDORI_LANG").ok();
        match flag.or(env.as_deref()) {
            Some("ja") => Lang::Ja,
            _ => Lang::En,
        }
    }
}

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
    pub fn new(line: usize, en: impl Into<String>, ja: impl Into<String>) -> Step {
        Step { line, en: en.into(), ja: ja.into(), at: None }
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
    pub notes: Vec<(String, String)>,
    pub path: Vec<Step>,
}

impl Diag {
    pub fn error(code: &'static str, line: usize, col: usize, en: impl Into<String>, ja: impl Into<String>) -> Diag {
        Diag { code, severity: Severity::Error, line, col, en: en.into(), ja: ja.into(), notes: vec![], path: vec![] }
    }

    pub fn warning(code: &'static str, line: usize, col: usize, en: impl Into<String>, ja: impl Into<String>) -> Diag {
        Diag { code, severity: Severity::Warning, line, col, en: en.into(), ja: ja.into(), notes: vec![], path: vec![] }
    }

    pub fn note(mut self, en: impl Into<String>, ja: impl Into<String>) -> Diag {
        self.notes.push((en.into(), ja.into()));
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

    /// The text a person reads: the headline, the source line, the notes, and the run.
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
                out.push_str(&format!("  {:>4} | {}\n", self.line, text));
            }
        }
        for (en, ja) in &self.notes {
            let n = if lang == Lang::Ja { ja } else { en };
            out.push_str(&format!("  = {n}\n"));
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
            "notes": self.notes.iter().map(|(en, ja)| if lang == Lang::Ja { ja.clone() } else { en.clone() }).collect::<Vec<_>>(),
            "path": self.path.iter().map(|s| json!({
                "line": s.line,
                "step": if lang == Lang::Ja { s.ja.clone() } else { s.en.clone() },
            })).collect::<Vec<_>>(),
        })
    }
}

pub fn has_errors(diags: &[Diag]) -> bool {
    diags.iter().any(|d| d.severity == Severity::Error)
}
