//! Diagnostics (DESIGN 4.1): a code, a place, the message in both languages, notes, and —
//! for what only shows on some inputs — the input and its computation, one step a line.
//!
//! The shape follows dandori's `src/diag.rs`: the headline, the source line, `= ` notes, then
//! the example. The JSON keeps its keys in English whatever the language (DESIGN 4.1).

use crate::date::Day;
use crate::i18n::{Lang, Text, pad, width};
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// One line of a computation.
#[derive(Clone, Debug)]
pub enum Step {
    /// A date, or one operation on it: the name in the first column on the first line of a
    /// date, the day it gives (None when it could not be computed), what the line of the
    /// `.cal` does, and why the day moved or did not.
    Value {
        line: usize,
        name: String,
        day: Option<Day>,
        label: Text,
        note: Option<Text>,
        /// Shown by `eval` only: which period a closing day closes.
        detail: Option<Text>,
    },
    /// A sentence under the dates: the time, or what a claim compared.
    Say { line: usize, text: Text },
}

impl Step {
    pub fn line(&self) -> usize {
        match self {
            Step::Value { line, .. } | Step::Say { line, .. } => *line,
        }
    }
}

/// A run of inputs that fail the same way: consecutive days, with the integer inputs fixed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
    pub from: Day,
    pub to: Day,
    /// The integer inputs, by name, when the file has any.
    pub params: Vec<(String, i64)>,
}

impl Run {
    pub fn days(&self) -> i64 {
        (self.to.0 - self.from.0) as i64 + 1
    }
}

#[derive(Clone, Debug)]
pub struct Diag {
    pub code: &'static str,
    pub severity: Severity,
    /// The file the line is in, as the command line named it (or as `use calendar` reached it).
    pub file: String,
    pub line: usize,
    pub col: usize,
    /// The text of that line.
    pub src: Option<String>,
    pub message: Text,
    pub notes: Vec<Text>,
    /// The heading of the computation shown under the notes.
    pub example: Option<Text>,
    pub steps: Vec<Step>,
    /// The example's input, name by name, in the form `koyomi eval` takes.
    pub inputs: Vec<(String, String)>,
    /// Every input that fails, as runs (`--format json` lists them all).
    pub fails: Vec<Run>,
    /// What to paste into the `.cal` instead.
    pub fix: Option<String>,
    /// Whether the text shows `fix` on a line of its own; not when a note already says it.
    pub show_fix: bool,
}

impl Diag {
    fn new(code: &'static str, severity: Severity, file: &str, line: usize, col: usize, message: Text) -> Diag {
        Diag {
            code,
            severity,
            file: file.to_string(),
            line,
            col,
            src: None,
            message,
            notes: vec![],
            example: None,
            steps: vec![],
            inputs: vec![],
            fails: vec![],
            fix: None,
            show_fix: true,
        }
    }

    pub fn error(code: &'static str, file: &str, line: usize, col: usize, message: Text) -> Diag {
        Diag::new(code, Severity::Error, file, line, col, message)
    }

    pub fn warning(code: &'static str, file: &str, line: usize, col: usize, message: Text) -> Diag {
        Diag::new(code, Severity::Warning, file, line, col, message)
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

    pub fn fix(mut self, f: impl Into<String>) -> Diag {
        self.fix = Some(f.into());
        self
    }

    /// The fix, which a note has already spelled out.
    pub fn fix_in_notes(mut self, f: impl Into<String>) -> Diag {
        self.fix = Some(f.into());
        self.show_fix = false;
        self
    }

    pub fn example(mut self, heading: Text, steps: Vec<Step>, inputs: Vec<(String, String)>) -> Diag {
        self.example = Some(heading);
        self.steps = steps;
        self.inputs = inputs;
        self
    }

    pub fn fails(mut self, runs: Vec<Run>) -> Diag {
        self.fails = runs;
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
        let place = if self.line > 0 {
            format!("{}:{}:{}", self.file, self.line, self.col)
        } else {
            self.file.clone()
        };
        let mut out = format!("{kind}[{}]: {place}: {}\n", self.code, say(&self.message, lang));
        if let Some(s) = &self.src {
            out.push_str(&format!("  {:>4} | {}\n", self.line, s));
        }
        for n in &self.notes {
            out.push_str(&format!("  = {}\n", say(n, lang)));
        }
        if let Some(f) = self.fix.as_ref().filter(|_| self.show_fix) {
            let head = if lang == Lang::Ja { "直した行" } else { "The line, fixed" };
            out.push_str(&format!("  = {head}: {}\n", f.trim()));
        }
        if let Some(h) = &self.example {
            out.push_str(&format!("  {}:\n", h.get(lang)));
            for l in render_steps(&self.steps, lang, false) {
                out.push_str(&format!("      {l}\n"));
            }
        }
        out
    }

    pub fn to_json(&self, lang: Lang) -> Value {
        json!({
            "code": self.code,
            "severity": match self.severity { Severity::Error => "error", Severity::Warning => "warning" },
            "file": self.file,
            "line": self.line,
            "col": self.col,
            "message": say(&self.message, lang),
            "notes": self.notes.iter().map(|n| say(n, lang)).collect::<Vec<_>>(),
            "inputs": if self.inputs.is_empty() { Value::Null } else {
                Value::Object(self.inputs.iter().map(|(k, v)| (k.clone(), Value::String(v.clone()))).collect())
            },
            "steps": steps_json(&self.steps, lang),
            "fails": self.fails.iter().map(|r| json!({
                "from": r.from.to_string(),
                "to": r.to.to_string(),
                "params": Value::Object(r.params.iter().map(|(k, v)| (k.clone(), json!(v))).collect()),
            })).collect::<Vec<_>>(),
            "fix": self.fix,
        })
    }
}

/// An English sentence starts with a capital, whatever the template it came from began with.
pub fn capitalize(s: &str) -> String {
    let mut cs = s.chars();
    match cs.next() {
        Some(c) if c.is_ascii_lowercase() => c.to_ascii_uppercase().to_string() + cs.as_str(),
        _ => s.to_string(),
    }
}

fn is_kana_or_kanji(c: char) -> bool {
    matches!(c as u32, 0x3040..=0x30FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0x3005)
}

/// Japanese text with a space between an ASCII word and the Japanese around it: a name such
/// as `invoice_date` reads `invoice_date は`, as the Japanese of this project writes `10 日`.
/// Digits are left alone (`第140条`), and so is `_`, which joins a name like `満了日_翌日`.
pub fn ja_spacing(s: &str) -> String {
    let cs: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len() + 8);
    let mut in_code = false;
    for (i, c) in cs.iter().enumerate() {
        if *c == '`' {
            in_code = !in_code;
        }
        if i > 0 && !in_code {
            let p = cs[i - 1];
            if (p.is_ascii_alphabetic() && is_kana_or_kanji(*c)) || (is_kana_or_kanji(p) && c.is_ascii_alphabetic()) {
                out.push(' ');
            }
        }
        out.push(*c);
    }
    out
}

/// A message as the language prints it.
pub fn say(t: &Text, lang: Lang) -> String {
    match lang {
        Lang::En => capitalize(&t.en),
        Lang::Ja => ja_spacing(&t.ja),
    }
}

/// `2026-01-01 Thu` and `2026-01-01（木）`.
pub fn day_with_weekday(d: Day, lang: Lang) -> String {
    let w = d.weekday() as usize;
    match lang {
        Lang::En => format!("{d} {}", crate::date::WEEKDAY_EN[w]),
        Lang::Ja => format!("{d}（{}）", crate::date::WEEKDAY_JA[w]),
    }
}

/// The steps as lines, the names in one column and the days in the next. `detail` adds what
/// only `eval` shows.
pub fn render_steps(steps: &[Step], lang: Lang, detail: bool) -> Vec<String> {
    let name_w = steps
        .iter()
        .map(|s| match s {
            Step::Value { name, .. } => width(name),
            Step::Say { .. } => 0,
        })
        .max()
        .unwrap_or(0);
    let day_w = match lang {
        Lang::En => 14,
        Lang::Ja => 16,
    };
    let mut out = Vec::new();
    for s in steps {
        match s {
            Step::Value { name, day, label, note, detail: more, .. } => {
                let d = day.map(|d| day_with_weekday(d, lang)).unwrap_or_default();
                let fix = |t: &Text| if lang == Lang::Ja { ja_spacing(&t.ja) } else { t.en.clone() };
                let mut what = fix(label);
                let mut extra: Vec<String> = Vec::new();
                if detail && let Some(m) = more {
                    extra.push(fix(m));
                }
                if let Some(n) = note {
                    extra.push(fix(n));
                }
                if !extra.is_empty() {
                    let sep = if lang == Lang::Ja { "。" } else { "; " };
                    if what.is_empty() {
                        what = extra.join(sep);
                    } else {
                        what = format!("{what}: {}", extra.join(sep));
                    }
                }
                let line = if what.is_empty() {
                    format!("{}  {}", pad(name, name_w), d)
                } else {
                    format!("{}  {}  {}", pad(name, name_w), pad(&d, day_w), what)
                };
                out.push(line.trim_end().to_string());
            }
            Step::Say { text, .. } => {
                let t = if lang == Lang::Ja { ja_spacing(&text.ja) } else { text.en.clone() };
                out.push(format!("{}  {}", " ".repeat(name_w), t).trim_end().to_string());
            }
        }
    }
    out
}

pub fn steps_json(steps: &[Step], lang: Lang) -> Value {
    Value::Array(
        steps
            .iter()
            .map(|s| match s {
                Step::Value { line, name, day, label, note, detail } => json!({
                    "line": line,
                    "name": if name.is_empty() { Value::Null } else { json!(name) },
                    "date": day.map(|d| d.to_string()),
                    "label": label.get(lang),
                    "note": note.as_ref().map(|n| n.get(lang).to_string()),
                    "detail": detail.as_ref().map(|n| n.get(lang).to_string()),
                }),
                Step::Say { line, text } => json!({ "line": line, "text": text.get(lang) }),
            })
            .collect(),
    )
}

pub fn has_errors(diags: &[Diag]) -> bool {
    diags.iter().any(|d| d.is_error())
}
