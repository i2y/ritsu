//! Diagnostics: a code, a place, the message in English and in Japanese, notes, and
//! the run that gets there. The shape is dandori's, so a reader of one reads the
//! other: `error[E004]: <file>:<line>:<col>: <message>`, the line itself, `= ` notes,
//! then the steps of the run.

use crate::json;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Ja,
}

impl Lang {
    /// `--lang` wins, then `GEAS_LANG`, then English. The machine's locale is not
    /// read: what geas prints must not change with the machine it runs on.
    pub fn pick(flag: Option<Lang>) -> Lang {
        if let Some(l) = flag {
            return l;
        }
        match std::env::var("GEAS_LANG") {
            Ok(v) => Lang::parse(&v).unwrap_or(Lang::En),
            Err(_) => Lang::En,
        }
    }

    /// `en` or `ja`, as `--lang` and `GEAS_LANG` take them.
    pub fn parse(s: &str) -> Option<Lang> {
        match s {
            "en" => Some(Lang::En),
            "ja" => Some(Lang::Ja),
            _ => None,
        }
    }

    pub fn tr<'a>(self, en: &'a str, ja: &'a str) -> &'a str {
        match self {
            Lang::En => en,
            Lang::Ja => ja,
        }
    }
}

/// One text, written in both languages next to each other.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Text {
    pub en: String,
    pub ja: String,
}

impl Text {
    pub fn get(&self, lang: Lang) -> &str {
        lang.tr(&self.en, &self.ja)
    }
}

pub fn t(en: impl Into<String>, ja: impl Into<String>) -> Text {
    Text { en: en.into(), ja: ja.into() }
}

/// A count and its noun, singular for one: `1 claim`, `4 claims`.
pub fn count(n: usize, one: &str, many: &str) -> String {
    if n == 1 { format!("{n} {one}") } else { format!("{n} {many}") }
}

/// A text that reads the same in both languages: a command, a step of a run.
pub fn same(s: impl Into<String>) -> Text {
    let s = s.into();
    Text { en: s.clone(), ja: s }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

/// One step of the run that gets there: the line of the spec, and what happened.
#[derive(Clone, Debug)]
pub struct Step {
    pub line: usize,
    pub text: Text,
}

#[derive(Clone, Debug)]
pub struct Diag {
    pub code: &'static str,
    pub severity: Severity,
    /// 0 when the problem has no line (a missing file, the command line).
    pub line: usize,
    /// 0 when the place is a whole line.
    pub col: usize,
    pub msg: Text,
    pub notes: Vec<Text>,
    pub path: Vec<Step>,
}

/// The longest line of a file a diagnostic quotes; a baseline's lines can be long.
const EXCERPT: usize = 120;

impl Diag {
    pub fn error(code: &'static str, line: usize, col: usize, msg: Text) -> Diag {
        Diag { code, severity: Severity::Error, line, col, msg, notes: vec![], path: vec![] }
    }

    pub fn note(mut self, n: Text) -> Diag {
        self.notes.push(n);
        self
    }

    pub fn with_path(mut self, path: Vec<Step>) -> Diag {
        self.path = path;
        self
    }

    /// The text a person reads: the headline, the line it points at, the notes, and
    /// the run. `file` is the file the line is in, or "" for the command line.
    pub fn render(&self, file: &str, src: &str, lang: Lang) -> String {
        let kind = match (self.severity, lang) {
            (Severity::Error, Lang::En) => "error",
            (Severity::Warning, Lang::En) => "warning",
            (Severity::Error, Lang::Ja) => "エラー",
            (Severity::Warning, Lang::Ja) => "警告",
        };
        let place = match (file.is_empty(), self.line, self.col) {
            (true, _, _) => String::new(),
            (false, 0, _) => format!("{file}: "),
            (false, l, 0) => format!("{file}:{l}: "),
            (false, l, c) => format!("{file}:{l}:{c}: "),
        };
        let mut out = format!("{kind}[{}]: {place}{}\n", self.code, self.msg.get(lang));
        if self.line > 0
            && let Some(text) = src.lines().nth(self.line - 1)
        {
            out.push_str(&format!("  {:>4} | {}\n", self.line, cut(&visible(text), EXCERPT)));
        }
        for n in &self.notes {
            out.push_str(&format!("  = {}\n", n.get(lang)));
        }
        if !self.path.is_empty() {
            out.push_str(lang.tr("  the run that gets there:\n", "  ここまでの実行:\n"));
            for s in &self.path {
                out.push_str(&format!("    {:>4}  {}\n", s.line, s.text.get(lang)));
            }
        }
        out
    }

    /// As data, with dandori's keys, and the file the line is in.
    pub fn to_json(&self, file: &str, lang: Lang) -> String {
        let notes: Vec<String> = self.notes.iter().map(|n| json::quote(n.get(lang))).collect();
        let path: Vec<String> = self
            .path
            .iter()
            .map(|s| format!("{{\"line\":{},\"step\":{}}}", s.line, json::quote(s.text.get(lang))))
            .collect();
        format!(
            "{{\"code\":\"{}\",\"severity\":\"{}\",\"file\":{},\"line\":{},\"col\":{},\"message\":{},\"notes\":[{}],\"path\":[{}]}}",
            self.code,
            match self.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
            },
            if file.is_empty() { "null".to_string() } else { json::quote(file) },
            self.line,
            self.col,
            json::quote(self.msg.get(lang)),
            notes.join(","),
            path.join(",")
        )
    }
}

pub fn has_errors(diags: &[Diag]) -> bool {
    diags.iter().any(|d| d.severity == Severity::Error)
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
        let d = Diag::error("E004", 1, 1, t("the target `calc` has neither `run` nor `serve`", "ターゲット `calc` に `run` も `serve` もありません"))
            .note(t("a note", "ひとこと"))
            .with_path(vec![Step { line: 1, text: same("when calc.run()") }]);
        assert_eq!(
            d.render("x.geas", SRC, Lang::En),
            "error[E004]: x.geas:1:1: the target `calc` has neither `run` nor `serve`\n     1 | target calc {\n  = a note\n  the run that gets there:\n       1  when calc.run()\n"
        );
        assert_eq!(
            d.render("x.geas", SRC, Lang::Ja),
            "エラー[E004]: x.geas:1:1: ターゲット `calc` に `run` も `serve` もありません\n     1 | target calc {\n  = ひとこと\n  ここまでの実行:\n       1  when calc.run()\n"
        );
    }

    #[test]
    fn places_without_a_line_or_a_file() {
        let d = Diag::error("E081", 0, 0, t("cannot read it", "読めません"));
        assert_eq!(d.render("missing.geas", "", Lang::En), "error[E081]: missing.geas: cannot read it\n");
        assert_eq!(d.render("", "", Lang::En), "error[E081]: cannot read it\n");
        let w = Diag { severity: Severity::Warning, ..Diag::error("W060", 3, 0, t("w", "警告の文")) };
        assert_eq!(w.render("x.geas", "a\nb\nc\n", Lang::Ja), "警告[W060]: x.geas:3: 警告の文\n     3 | c\n");
    }

    #[test]
    fn as_json() {
        let d = Diag::error("E002", 2, 3, t("there is no target `api`", "ターゲット `api` はありません")).note(t("n", "注"));
        assert_eq!(
            d.to_json("x.geas", Lang::En),
            "{\"code\":\"E002\",\"severity\":\"error\",\"file\":\"x.geas\",\"line\":2,\"col\":3,\"message\":\"there is no target `api`\",\"notes\":[\"n\"],\"path\":[]}"
        );
        assert!(d.to_json("", Lang::Ja).contains("\"file\":null"));
        assert!(d.to_json("", Lang::Ja).contains("ターゲット `api` はありません"));
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
        assert_eq!(Lang::pick(Some(Lang::Ja)), Lang::Ja);
        assert_eq!(Lang::parse("fr"), None);
    }
}
