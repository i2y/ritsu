//! Diagnostics: a code, a place, what is wrong in English and Japanese, and, when it only
//! shows when someone calls the book, the operations that get there.
//!
//! Every sentence a person reads is written twice, side by side, with `tr!`, so that neither
//! language can be written without the other. A diagnostic keeps both and is rendered in the
//! one asked for, so the same check serves `--lang en` and `--lang ja`.
//!
//! What every language's diagnostic has is ritsu-base's ([`ritsu_base::diag`]): the headline,
//! the source line, `= ` notes, and the JSON with its keys in English. What is chobo's is
//! [`Ops`]: the operations that get there, as a person reads them and as scenario steps, and the
//! hint. A check finds a diagnostic before it knows the file's name, so the file and its line are
//! put in when it is printed ([`Show`]).

use ritsu_base::diag::Extra;
use ritsu_base::json::Json;
use ritsu_base::text::{Lang, Text};
use serde_json::Value;

pub use ritsu_base::diag::Severity;

/// One operation of the list that shows a finding: the call as a person reads it, with its
/// result, and the same call as a scenario step with its result, for a program.
#[derive(Clone, Debug, PartialEq)]
pub struct OpLine {
    pub text: Text,
    pub json: Value,
}

/// chobo's part of a diagnostic.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ops {
    pub ops: Vec<OpLine>,
    pub hint: Option<Text>,
    /// The line of the book it is about, as written (the JSON's `excerpt`).
    pub excerpt: String,
    /// The line is not quoted, in the text nor in the JSON: it holds a key (W901).
    pub unquoted: bool,
}

impl Extra for Ops {
    fn after_fix(&self, lang: Lang, out: &mut String) {
        if !self.ops.is_empty() {
            out.push_str(if lang == Lang::Ja { "  そうなる例:\n" } else { "  the operations that get there:\n" });
            for (i, op) in self.ops.iter().enumerate() {
                out.push_str(&format!("    {:>4}  {}\n", i + 1, op.text.get(lang)));
            }
        }
        if let Some(h) = &self.hint {
            out.push_str(&format!("  {}: {}\n", if lang == Lang::Ja { "ヒント" } else { "hint" }, h.get(lang)));
        }
    }

    fn json(&self, lang: Lang) -> Vec<(String, Json)> {
        let ops = self.ops.iter().map(|o| ritsu_base::json::parse(&o.json.to_string()).expect("serde_json writes JSON ritsu-base reads"));
        vec![
            ("excerpt".into(), Json::str(&self.excerpt)),
            ("operations".into(), Json::arr(ops)),
            ("hint".into(), Json::opt_str(self.hint.as_ref().map(|h| h.get(lang)))),
        ]
    }
}

/// A diagnostic of chobo's: ritsu-base's, with the operations that get there.
pub type Diag = ritsu_base::diag::Diag<Ops>;

/// An error at a line and a column of the book.
pub fn error(code: &'static str, line: usize, col: usize, msg: Text) -> Diag {
    Diag::error(code, "", line, col, msg)
}

/// A warning at a line and a column of the book.
pub fn warning(code: &'static str, line: usize, col: usize, msg: Text) -> Diag {
    Diag::warning(code, "", line, col, msg)
}

/// What chobo adds to a diagnostic as it is built.
pub trait DiagExt {
    /// How to fix it, in a sentence.
    fn hint(self, t: Text) -> Self;
    /// The operations that get there.
    fn with_ops(self, ops: Vec<OpLine>) -> Self;
}

impl DiagExt for Diag {
    fn hint(mut self, t: Text) -> Diag {
        self.extra.hint = Some(t);
        self
    }

    fn with_ops(mut self, ops: Vec<OpLine>) -> Diag {
        self.extra.ops = ops;
        self
    }
}

/// A diagnostic printed for a file: its name as the command line gave it, and its source.
pub trait Show {
    /// The same diagnostic in the file, its line's text kept as written (not trimmed).
    fn placed(&self, file: &str, src: &str) -> Diag;
    /// The text a person reads: the headline, the source line, the notes, the operations
    /// that get there, and how to fix it.
    fn shown(&self, file: &str, src: &str, lang: Lang) -> String;
    /// The same finding for a program. The keys are English whatever the language.
    fn json_in(&self, file: &str, src: &str, lang: Lang) -> Value;
}

impl Show for Diag {
    fn placed(&self, file: &str, src: &str) -> Diag {
        let mut d = self.clone();
        d.file = file.to_string();
        d.rel = file.to_string();
        // a key on the line is masked (ritsu-base's `secrets::mask`): a line must not carry a key
        // into the logs, whatever the diagnostic
        d.src = if d.extra.unquoted { None } else { d.line.and_then(|l| src.lines().nth(l - 1)).map(ritsu_base::secrets::mask) };
        d.extra.excerpt = d.src.clone().unwrap_or_default();
        d
    }

    fn shown(&self, file: &str, src: &str, lang: Lang) -> String {
        self.placed(file, src).render(lang)
    }

    fn json_in(&self, file: &str, src: &str, lang: Lang) -> Value {
        serde_json::from_str(&self.placed(file, src).to_json(lang).compact()).expect("ritsu-base writes JSON serde_json reads")
    }
}

pub fn has_errors(diags: &[Diag]) -> bool {
    ritsu_base::diag::has_errors(diags)
}

/// W901 (ritsu's DESIGN 16.3): a key written in the book, as the headline and the notes say it.
/// They give the kind of key, its fixed prefix and its length, never the key; and the line is not
/// quoted, since it holds the key.
pub fn key_written(f: &ritsu_base::secrets::Found) -> Diag {
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
    let mut d = warning("W901", f.line, f.col, msg)
        .note(tr!(
            "ファイルに書いた鍵は、リポジトリとその履歴とビルドを読めるすべての人に渡ります。鍵はコードが動くところ（環境変数、プラットフォームの接続やシークレットの置き場）に置き、そこから読んでください。",
            "A key in a file reaches everyone who can read the repository, its history and its builds. Keep it where the code runs (an environment variable, the platform's connection or secret store) and read it from there."
        ))
        .note(revoke)
        .note(test);
    d.extra.unquoted = true;
    d
}

/// W901 for every key of the book's text, but those whose line says `ritsu: test secret`.
pub fn keys(src: &str) -> Vec<Diag> {
    ritsu_base::secrets::scan(src).iter().filter(|f| !f.test).map(key_written).collect()
}

/// The errors, and the rest (chobo has no notes).
pub fn count(diags: &[Diag]) -> (usize, usize) {
    let c = ritsu_base::diag::count(diags);
    (c.errors, c.warnings + c.notes)
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
