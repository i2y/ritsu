//! Diagnostics: a code, a place, the message in English and in Japanese, notes, and
//! the run that gets there. The shape is dandori's, so a reader of one reads the
//! other: `error[E004]: <file>:<line>:<col>: <message>`, the line itself, `= ` notes,
//! then the steps of the run.
//!
//! What every language's diagnostic has is ritsu-base's (`ritsu_base::diag`); what is
//! geas's is [`Run`], the steps of the run, and the excerpt of the line, which geas
//! shows with its control characters made visible and cut to 120 characters. A check
//! finds a diagnostic before it is told the file, so the file and its source are put
//! in when it is printed ([`Show`]).

use ritsu_base::diag::Extra;
use ritsu_base::json::Json;
use ritsu_base::text::{Lang, Text};

pub use ritsu_base::diag::Severity;

/// `--lang` wins, then `GEAS_LANG`, then `RITSU_LANG`, then English. The machine's
/// locale is not read: what geas prints must not change with the machine it runs on.
pub fn pick(flag: Option<Lang>) -> Lang {
    flag.unwrap_or_else(|| Lang::pick(None, "GEAS_LANG"))
}

/// `en` or `ja`, as `--lang` takes them.
pub fn parse_lang(s: &str) -> Option<Lang> {
    match s {
        "en" => Some(Lang::En),
        "ja" => Some(Lang::Ja),
        _ => None,
    }
}

/// A count and its noun, singular for one: `1 claim`, `4 claims`.
pub fn count(n: usize, one: &str, many: &str) -> String {
    if n == 1 { format!("{n} {one}") } else { format!("{n} {many}") }
}

/// One step of the run that gets there: the line of the spec, and what happened.
#[derive(Clone, Debug)]
pub struct Step {
    pub line: usize,
    pub text: Text,
}

/// geas's part of a diagnostic: the run that gets there.
#[derive(Clone, Debug, Default)]
pub struct Run {
    pub path: Vec<Step>,
}

impl Extra for Run {
    fn after_fix(&self, lang: Lang, out: &mut String) {
        if !self.path.is_empty() {
            out.push_str(tr!("  ここまでの実行:\n", "  the run that gets there:\n").get(lang));
            for s in &self.path {
                out.push_str(&format!("    {:>4}  {}\n", s.line, s.text.get(lang)));
            }
        }
    }

    fn json(&self, lang: Lang) -> Vec<(String, Json)> {
        let path = self.path.iter().map(|s| Json::obj([("line", Json::from(s.line)), ("step", Json::str(s.text.get(lang)))]));
        vec![("path".into(), Json::arr(path))]
    }

    fn fix_key(&self) -> bool {
        false
    }
}

/// A diagnostic of geas's: ritsu-base's, with the run that gets there.
pub type Diag = ritsu_base::diag::Diag<Run>;

/// The longest line of a file a diagnostic quotes; a baseline's lines can be long.
const EXCERPT: usize = 120;

/// An error at a line and a column (0 for either is none) of a file not yet named.
pub fn error(code: &'static str, line: usize, col: usize, msg: Text) -> Diag {
    Diag::error(code, "", line, col, msg)
}

/// What geas adds to a diagnostic as it is built.
pub trait DiagExt {
    /// The run that gets there.
    fn with_path(self, path: Vec<Step>) -> Self;
}

impl DiagExt for Diag {
    fn with_path(mut self, path: Vec<Step>) -> Diag {
        self.extra.path = path;
        self
    }
}

/// A diagnostic printed for a file: its name, or "" for the command line, and its source.
pub trait Show {
    /// The same diagnostic in the file, the excerpt of its line made visible and cut.
    fn placed(&self, file: &str, src: &str) -> Diag;
    /// The text a person reads: the headline, the line it points at, the notes, and
    /// the run.
    fn shown(&self, file: &str, src: &str, lang: Lang) -> String;
    /// As data, with dandori's keys, and the file the line is in (null for none).
    fn json_in(&self, file: &str, lang: Lang) -> String;
}

impl Show for Diag {
    fn placed(&self, file: &str, src: &str) -> Diag {
        let mut d = self.clone();
        d.file = file.to_string();
        d.rel = file.to_string();
        d.src = d.line.and_then(|l| src.lines().nth(l - 1)).map(|t| cut(&visible(t), EXCERPT));
        d
    }

    fn shown(&self, file: &str, src: &str, lang: Lang) -> String {
        self.placed(file, src).render(lang)
    }

    fn json_in(&self, file: &str, lang: Lang) -> String {
        self.placed(file, "").to_json(lang).compact()
    }
}

pub fn has_errors(diags: &[Diag]) -> bool {
    ritsu_base::diag::has_errors(diags)
}

/// Sorts diagnostics into line order; ones on the same place keep the order found.
pub fn sort(diags: &mut [Diag]) {
    diags.sort_by_key(|d| (d.line, d.col));
}

/// Every line of `text` with `by` in front, empty lines left empty.
pub fn indent(text: &str, by: &str) -> String {
    text.lines()
        .map(|l| if l.is_empty() { "\n".to_string() } else { format!("{by}{l}\n") })
        .collect()
}

/// A line as a terminal can show it: a control character other than a tab becomes
/// U+FFFD, so it is seen at its place and never sent to the terminal raw.
fn visible(s: &str) -> String {
    s.chars().map(|c| if c.is_control() && c != '\t' { '\u{FFFD}' } else { c }).collect()
}

/// `s` cut to at most `max` characters, ending in `…` when cut.
pub fn cut(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max - 1).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "target calc {\n}\n";

    #[test]
    fn a_diagnostic_in_both_languages() {
        let d = error("E004", 1, 1, tr!("ターゲット `calc` に `run` も `serve` もありません", "the target `calc` has neither `run` nor `serve`"))
            .note(tr!("ひとこと", "a note"))
            .with_path(vec![Step { line: 1, text: Text::same("when calc.run()") }]);
        assert_eq!(
            d.shown("x.geas", SRC, Lang::En),
            "error[E004]: x.geas:1:1: the target `calc` has neither `run` nor `serve`\n     1 | target calc {\n  = a note\n  the run that gets there:\n       1  when calc.run()\n"
        );
        assert_eq!(
            d.shown("x.geas", SRC, Lang::Ja),
            "エラー[E004]: x.geas:1:1: ターゲット `calc` に `run` も `serve` もありません\n     1 | target calc {\n  = ひとこと\n  ここまでの実行:\n       1  when calc.run()\n"
        );
    }

    #[test]
    fn places_without_a_line_or_a_file() {
        let d = error("E081", 0, 0, tr!("読めません", "cannot read it"));
        assert_eq!(d.shown("missing.geas", "", Lang::En), "error[E081]: missing.geas: cannot read it\n");
        assert_eq!(d.shown("", "", Lang::En), "error[E081]: cannot read it\n");
        let w = Diag { severity: Severity::Warning, ..error("W060", 3, 0, tr!("警告の文", "w")) };
        assert_eq!(w.shown("x.geas", "a\nb\nc\n", Lang::Ja), "警告[W060]: x.geas:3: 警告の文\n     3 | c\n");
    }

    #[test]
    fn as_json() {
        let d = error("E002", 2, 3, tr!("ターゲット `api` はありません", "there is no target `api`")).note(tr!("注", "n"));
        assert_eq!(
            d.json_in("x.geas", Lang::En),
            "{\"code\":\"E002\",\"severity\":\"error\",\"file\":\"x.geas\",\"line\":2,\"col\":3,\"message\":\"there is no target `api`\",\"notes\":[\"n\"],\"path\":[]}"
        );
        assert!(d.json_in("", Lang::Ja).contains("\"file\":null"));
        assert!(d.json_in("", Lang::Ja).contains("ターゲット `api` はありません"));
    }

    #[test]
    fn cutting_and_indenting() {
        assert_eq!(cut("abcdef", 4), "abc…");
        assert_eq!(cut("abcd", 4), "abcd");
        assert_eq!(cut("日本語の文", 3), "日本…");
        assert_eq!(indent("a\n\nb\n", "  "), "  a\n\n  b\n");
    }

    #[test]
    fn lang_from_the_flag_first() {
        assert_eq!(pick(Some(Lang::Ja)), Lang::Ja);
        assert_eq!(parse_lang("fr"), None);
    }
}
