//! Diagnostics (DESIGN 4.1): a code, a place, the message in both languages, notes, and —
//! for what only shows on some inputs — the input and its computation, one step a line.
//!
//! What every language's diagnostic has is ritsu-base's ([`ritsu_base::diag`]): the headline,
//! the source line, `= ` notes, the fixed line, and the JSON with its keys in English whatever
//! the language. What is koyomi's is [`Example`]: the computation under the fixed line, and in
//! the JSON the example's input, its steps and every input that fails. A message is spaced and
//! capitalized as the suite's Japanese and English write it ([`ritsu_base::text::spaced`]).

use crate::date::Day;
use ritsu_base::diag::Extra;
use ritsu_base::json::Json;
use ritsu_base::text::{Lang, Text, ja_spacing, pad, spaced, width};

pub use ritsu_base::diag::Severity;

/// A diagnostic of koyomi's: ritsu-base's, with the computation that led there.
pub type Diag = ritsu_base::diag::Diag<Example>;

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

/// koyomi's part of a diagnostic: the computation shown under the fixed line.
#[derive(Clone, Debug, Default)]
pub struct Example {
    /// The heading of the computation.
    pub heading: Option<Text>,
    pub steps: Vec<Step>,
    /// The example's input, name by name, in the form `koyomi eval` takes.
    pub inputs: Vec<(String, String)>,
    /// Every input that fails, as runs (`--format json` lists them all).
    pub fails: Vec<Run>,
}

impl Extra for Example {
    fn after_fix(&self, lang: Lang, out: &mut String) {
        if let Some(h) = &self.heading {
            out.push_str(&format!("  {}:\n", h.get(lang)));
            for l in render_steps(&self.steps, lang, false) {
                out.push_str(&format!("      {l}\n"));
            }
        }
    }

    fn json(&self, lang: Lang) -> Vec<(String, Json)> {
        let inputs = if self.inputs.is_empty() { Json::Null } else { Json::obj(self.inputs.iter().map(|(k, v)| (k.clone(), Json::str(v)))) };
        let fails = self.fails.iter().map(|r| {
            Json::obj([
                ("from", Json::str(r.from.to_string())),
                ("to", Json::str(r.to.to_string())),
                ("params", Json::obj(r.params.iter().map(|(k, v)| (k.clone(), Json::int(*v))))),
            ])
        });
        vec![("inputs".into(), inputs), ("steps".into(), steps_json(&self.steps, lang)), ("fails".into(), Json::arr(fails))]
    }

    fn say(&self, t: &Text, lang: Lang) -> String {
        spaced(t, lang)
    }
}

/// What koyomi adds to a diagnostic as it is built.
pub trait DiagExt {
    /// The computation of an input that shows it, under a heading, and that input as
    /// `koyomi eval` takes it.
    fn example(self, heading: Text, steps: Vec<Step>, inputs: Vec<(String, String)>) -> Self;
    /// Every input that fails, as runs.
    fn fails(self, runs: Vec<Run>) -> Self;
}

impl DiagExt for Diag {
    fn example(mut self, heading: Text, steps: Vec<Step>, inputs: Vec<(String, String)>) -> Diag {
        self.extra.heading = Some(heading);
        self.extra.steps = steps;
        self.extra.inputs = inputs;
        self
    }

    fn fails(mut self, runs: Vec<Run>) -> Diag {
        self.extra.fails = runs;
        self
    }
}

/// ritsu-base's JSON as serde_json's, for the outputs koyomi writes with serde_json.
pub fn value(j: &Json) -> serde_json::Value {
    serde_json::from_str(&j.compact()).expect("ritsu-base writes JSON serde_json reads")
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

pub fn steps_json(steps: &[Step], lang: Lang) -> Json {
    Json::arr(steps.iter().map(|s| match s {
        Step::Value { line, name, day, label, note, detail } => Json::obj([
            ("line", Json::from(*line)),
            ("name", if name.is_empty() { Json::Null } else { Json::str(name) }),
            ("date", Json::opt_str(day.map(|d| d.to_string()))),
            ("label", Json::str(label.get(lang))),
            ("note", Json::opt_str(note.as_ref().map(|n| n.get(lang)))),
            ("detail", Json::opt_str(detail.as_ref().map(|n| n.get(lang)))),
        ]),
        Step::Say { line, text } => Json::obj([("line", Json::from(*line)), ("text", Json::str(text.get(lang)))]),
    }))
}

pub fn has_errors(diags: &[Diag]) -> bool {
    diags.iter().any(|d| d.is_error())
}
