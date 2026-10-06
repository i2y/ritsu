//! Diagnostics (DESIGN 6.1): a code, a place, the message in both languages, notes, and what
//! led there — the diff of what changed, the requirements of a cycle, the versions around a
//! gap, the names to pick from — and the fix, down to the line to write or the command to run.
//!
//! What every language's diagnostic has is ritsu-base's (`ritsu_base::diag`): the headline,
//! the source line, `= ` notes, the fix, and the JSON with its keys in English whatever the
//! language. What is yuen's is [`Trail`]: the chain, the diff and the names to pick from,
//! printed before the fix. A message is printed as written ([`ritsu_base::text::as_written`]).

use ritsu_base::diag::Extra;
use ritsu_base::json::Json;
use ritsu_base::text::{Lang, Text};

pub use ritsu_base::diag::{Fix, Severity};

/// One line of a diff: `-` gone, `+` added, ` ` the same, `@` where a hunk starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffLine {
    pub op: char,
    pub text: String,
}

/// One step of what led to a diagnostic: a requirement of a cycle, a version and its period.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChainItem {
    pub text: Text,
    /// Where it is, as the diagnostic names files, with its line; empty when nowhere.
    pub file: String,
    pub line: usize,
}

/// yuen's part of a diagnostic: what led there.
#[derive(Clone, Debug, Default)]
pub struct Trail {
    /// The heading over `diff`, and the diff.
    pub diff_head: Option<Text>,
    pub diff: Vec<DiffLine>,
    /// The heading over `chain`, and the chain.
    pub chain_head: Option<Text>,
    pub chain: Vec<ChainItem>,
    pub candidates: Vec<String>,
}

impl Extra for Trail {
    fn before_fix(&self, lang: Lang, out: &mut String) {
        if let Some(h) = &self.chain_head {
            out.push_str(&format!("  {}:\n", h.get(lang)));
            for c in &self.chain {
                let at = if c.line > 0 { format!(" ({}:{})", c.file, c.line) } else { String::new() };
                let at = if lang == Lang::Ja && c.line > 0 { format!("（{}:{}）", c.file, c.line) } else { at };
                out.push_str(&format!("      {}{at}\n", c.text.get(lang)));
            }
        }
        if let Some(h) = &self.diff_head {
            out.push_str(&format!("  {}:\n", h.get(lang)));
            for d in &self.diff {
                let l = if d.op == '@' { format!("      {}", d.text) } else { format!("      {} {}", d.op, d.text) };
                out.push_str(l.trim_end());
                out.push('\n');
            }
        }
        if !self.candidates.is_empty() {
            let head = if lang == Lang::Ja { "候補" } else { "Did you mean" };
            out.push_str(&format!("  = {head}: {}\n", self.candidates.join(", ")));
        }
    }

    fn json(&self, lang: Lang) -> Vec<(String, Json)> {
        let diff = self.diff.iter().map(|d| Json::obj([("op", Json::str(if d.op == '@' { "@@".to_string() } else { d.op.to_string() })), ("text", Json::str(&d.text))]));
        let chain = self.chain.iter().map(|c| Json::obj([("text", Json::str(c.text.get(lang))), ("file", Json::str(&c.file)), ("line", Json::from(c.line))]));
        vec![
            ("diff".into(), Json::arr(diff)),
            ("chain".into(), Json::arr(chain)),
            ("candidates".into(), Json::arr(self.candidates.iter().map(Json::str))),
        ]
    }
}

/// A diagnostic of yuen's: ritsu-base's, with what led there.
pub type Diag = ritsu_base::diag::Diag<Trail>;

/// An error at a line and a column of a file: the file as the command line named it, and the
/// same file from the root (DESIGN 2.2), for the JSON.
pub fn error(code: &'static str, file: &str, rel: &str, line: usize, col: usize, message: Text) -> Diag {
    Diag::error(code, file, line, col, message).rel(rel)
}

/// A warning, the same way.
pub fn warning(code: &'static str, file: &str, rel: &str, line: usize, col: usize, message: Text) -> Diag {
    Diag::warning(code, file, line, col, message).rel(rel)
}

/// W901 (ritsu's DESIGN 16.3): a key written in a `.req`, as the headline and the notes say it.
/// They give the kind of key, its fixed prefix and its length, never the key; and no line is
/// quoted, since the line holds the key.
pub fn key_written(file: &str, rel: &str, f: &ritsu_base::secrets::Found) -> Diag {
    let name = &f.kind.name;
    let private = f.kind.provider.is_empty();
    let msg = if private {
        tr!("{}がここに書かれています（{}）", "{} is written here ({})", name.ja, f.shown; ritsu_base::text::capitalize(&name.en), f.shown)
    } else {
        tr!("{}がここに書かれています（{}、{} 文字）", "{} is written here ({}, {} characters)", name.ja, f.shown, f.len; ritsu_base::text::capitalize(&name.en), f.shown, f.len)
    };
    let provider = f.kind.provider;
    let revoke = if private {
        tr!(
            "本物の鍵なら、まず新しい鍵に替え、この鍵を使うのをやめてください。ファイルから消しても、リポジトリの履歴には残ります。",
            "If this key is real, replace it with a new one first and stop using this one: taking it out of the file leaves it in the history of the repository."
        )
    } else {
        tr!(
            "本物の鍵なら、まず {provider} で無効にしてください。ファイルから消しても、リポジトリの履歴には残ります。",
            "If this key is real, revoke it with {provider} first: taking it out of the file leaves it in the history of the repository."
        )
    };
    let test = if private {
        tr!("テスト用の鍵なら、`-----BEGIN` の行のコメントに `ritsu: test secret` と書いてください。", "If it is a key for tests, write `ritsu: test secret` in a comment on its `-----BEGIN` line.")
    } else {
        tr!("テスト用の値なら、同じ行のコメントに `ritsu: test secret` と書いてください。", "If it is a value for tests, write `ritsu: test secret` in a comment on the same line.")
    };
    warning("W901", file, rel, f.line, f.col, msg)
        .note(tr!(
            "ファイルに書いた鍵は、リポジトリとその履歴とビルドを読めるすべての人に渡ります。鍵はコードが動くところ（環境変数、プラットフォームの接続やシークレットの置き場）に置き、そこから読んでください。",
            "A key in a file reaches everyone who can read the repository, its history and its builds. Keep it where the code runs (an environment variable, the platform's connection or secret store) and read it from there."
        ))
        .note(revoke)
        .note(test)
}

/// W901 for every key of a `.req`'s text, but those whose line says `ritsu: test secret`.
pub fn keys(file: &str, rel: &str, src: &str) -> Vec<Diag> {
    ritsu_base::secrets::scan(src).iter().filter(|f| !f.test).map(|f| key_written(file, rel, f)).collect()
}

/// What yuen adds to a diagnostic as it is built.
pub trait DiagExt {
    /// The line to write in the `.req`. The text and the JSON both write it trimmed.
    fn fix_trimmed(self, f: impl AsRef<str>) -> Self;
    fn diff(self, head: Text, lines: Vec<DiffLine>) -> Self;
    fn chain(self, head: Text, items: Vec<ChainItem>) -> Self;
    fn candidates(self, c: Vec<String>) -> Self;
}

impl DiagExt for Diag {
    fn fix_trimmed(self, f: impl AsRef<str>) -> Diag {
        self.fix_line(f.as_ref().trim())
    }

    fn diff(mut self, head: Text, lines: Vec<DiffLine>) -> Diag {
        self.extra.diff_head = Some(head);
        self.extra.diff = lines;
        self
    }

    fn chain(mut self, head: Text, items: Vec<ChainItem>) -> Diag {
        self.extra.chain_head = Some(head);
        self.extra.chain = items;
        self
    }

    fn candidates(mut self, c: Vec<String>) -> Diag {
        self.extra.candidates = c;
        self
    }
}

/// ritsu-base's JSON as serde_json's, for the outputs yuen writes with serde_json.
pub fn value(j: &Json) -> serde_json::Value {
    serde_json::from_str(&j.compact()).expect("ritsu-base writes JSON serde_json reads")
}

pub fn has_errors(diags: &[Diag]) -> bool {
    ritsu_base::diag::has_errors(diags)
}

/// The errors, and the rest.
pub fn count(diags: &[Diag]) -> (usize, usize) {
    let c = ritsu_base::diag::count(diags);
    (c.errors, c.warnings + c.notes)
}
