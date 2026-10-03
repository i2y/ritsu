//! Diagnostics: a code, a place, what is wrong in English and Japanese, and, when it only
//! shows when someone calls the book, the operations that get there.
//!
//! Every sentence a person reads is written twice, side by side, with `tr!` (in `lib.rs`), so
//! that neither language can be written without the other. A diagnostic keeps both and is
//! rendered in the one asked for, so the same check serves `--lang en` and `--lang ja`.

use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Ja,
}

impl Lang {
    /// `--lang` wins, then `CHOBO_LANG`, then English. The system locale is not read: the
    /// output is diffed in tests and CI, so it must not change with the machine.
    pub fn pick(flag: Option<&str>) -> Lang {
        let env = std::env::var("CHOBO_LANG").ok();
        match flag.or(env.as_deref()) {
            Some(s) if s.trim().to_ascii_lowercase().starts_with("ja") => Lang::Ja,
            _ => Lang::En,
        }
    }
}

/// One sentence in both languages.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Text {
    pub ja: String,
    pub en: String,
}

impl Text {
    pub fn get(&self, lang: Lang) -> &str {
        match lang {
            Lang::En => &self.en,
            Lang::Ja => &self.ja,
        }
    }

    /// The same words in both languages: a name, a number.
    pub fn same(s: impl Into<String>) -> Text {
        let s = s.into();
        Text { ja: s.clone(), en: s }
    }

    /// Put `val` where the sentence has `{key}`, in each language its own words. A sentence
    /// that takes words that differ by language is written with `{{key}}` in `tr!`.
    pub fn sub(mut self, key: &str, val: &Text) -> Text {
        let k = format!("{{{key}}}");
        self.ja = self.ja.replace(&k, &val.ja);
        self.en = self.en.replace(&k, &val.en);
        self
    }

    pub fn join(parts: &[Text], ja_sep: &str, en_sep: &str) -> Text {
        Text {
            ja: parts.iter().map(|t| t.ja.as_str()).collect::<Vec<_>>().join(ja_sep),
            en: parts.iter().map(|t| t.en.as_str()).collect::<Vec<_>>().join(en_sep),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
}

/// One operation of the list that shows a finding: the call as a person reads it, with its
/// result, and the same call as a scenario step with its result, for a program.
#[derive(Clone, Debug, PartialEq)]
pub struct OpLine {
    pub text: Text,
    pub json: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Diag {
    pub code: &'static str,
    pub severity: Severity,
    pub line: usize,
    pub col: usize,
    pub msg: Text,
    pub notes: Vec<Text>,
    pub ops: Vec<OpLine>,
    pub hint: Option<Text>,
}

impl Diag {
    pub fn error(code: &'static str, line: usize, col: usize, msg: Text) -> Diag {
        Diag { code, severity: Severity::Error, line, col, msg, notes: vec![], ops: vec![], hint: None }
    }

    pub fn warning(code: &'static str, line: usize, col: usize, msg: Text) -> Diag {
        Diag { code, severity: Severity::Warning, line, col, msg, notes: vec![], ops: vec![], hint: None }
    }

    pub fn note(mut self, t: Text) -> Diag {
        self.notes.push(t);
        self
    }

    pub fn hint(mut self, t: Text) -> Diag {
        self.hint = Some(t);
        self
    }

    pub fn with_ops(mut self, ops: Vec<OpLine>) -> Diag {
        self.ops = ops;
        self
    }

    /// The text a person reads: the headline, the source line, the notes, the operations
    /// that get there, and how to fix it.
    pub fn render(&self, file: &str, src: &str, lang: Lang) -> String {
        let kind = match (self.severity, lang) {
            (Severity::Error, Lang::En) => "error",
            (Severity::Warning, Lang::En) => "warning",
            (Severity::Error, Lang::Ja) => "エラー",
            (Severity::Warning, Lang::Ja) => "警告",
        };
        let mut out = format!("{kind}[{}]: {file}:{}:{}: {}\n", self.code, self.line, self.col, self.msg.get(lang));
        if self.line > 0 {
            if let Some(text) = src.lines().nth(self.line - 1) {
                out.push_str(&format!("  {:>4} | {}\n", self.line, text));
            }
        }
        for n in &self.notes {
            out.push_str(&format!("  = {}\n", n.get(lang)));
        }
        if !self.ops.is_empty() {
            out.push_str(if lang == Lang::Ja { "  そうなる例:\n" } else { "  the operations that get there:\n" });
            for (i, op) in self.ops.iter().enumerate() {
                out.push_str(&format!("    {:>4}  {}\n", i + 1, op.text.get(lang)));
            }
        }
        if let Some(h) = &self.hint {
            out.push_str(&format!("  {}: {}\n", if lang == Lang::Ja { "ヒント" } else { "hint" }, h.get(lang)));
        }
        out
    }

    /// The same finding for a program. The keys are English whatever the language.
    pub fn to_json(&self, file: &str, src: &str, lang: Lang) -> Value {
        let excerpt = if self.line > 0 { src.lines().nth(self.line - 1).unwrap_or("") } else { "" };
        json!({
            "v": 1,
            "severity": match self.severity { Severity::Error => "error", Severity::Warning => "warning" },
            "code": self.code,
            "file": file,
            "line": self.line,
            "column": self.col,
            "title": self.msg.get(lang),
            "excerpt": excerpt,
            "notes": self.notes.iter().map(|n| n.get(lang)).collect::<Vec<_>>(),
            "operations": self.ops.iter().map(|o| o.json.clone()).collect::<Vec<_>>(),
            "hint": self.hint.as_ref().map(|h| h.get(lang)),
        })
    }
}

pub fn has_errors(diags: &[Diag]) -> bool {
    diags.iter().any(|d| d.severity == Severity::Error)
}

pub fn count(diags: &[Diag]) -> (usize, usize) {
    let e = diags.iter().filter(|d| d.severity == Severity::Error).count();
    (e, diags.len() - e)
}

/// The line after the diagnostics: `ok`, or how many of each.
pub fn summary(file: &str, diags: &[Diag], lang: Lang) -> String {
    let (e, w) = count(diags);
    match (e, w, lang) {
        (0, 0, _) => format!("{file}: ok"),
        (0, w, Lang::En) => format!("{file}: ok, {w} warning(s)"),
        (0, w, Lang::Ja) => format!("{file}: ok、警告 {w} 件"),
        (e, w, Lang::En) => format!("{file}: {e} error(s), {w} warning(s)"),
        (e, w, Lang::Ja) => format!("{file}: エラー {e} 件、警告 {w} 件"),
    }
}
