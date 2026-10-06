//! Keys written in the contracts of a project (W901, DESIGN 16.3): its `.proto` files, and the
//! OpenAPI, AsyncAPI and JSON Schema documents its languages read (a document a file names that is
//! a `.json`, a `.yaml` or a `.yml`, as the language's `References` give it: dandori's `use openapi`
//! and `use smithy`, rulec's `import jsonschema` and `shape … jsonschema`, the `openapi` and
//! `asyncapi` of a published language of sakai's map), each looked at once. A contract is no language's source:
//! rulec, dandori and sakai can read the same one, and if each of them looked, `ritsu check` would
//! say one key three times. The languages look at their own files, with the same scan
//! (ritsu-base's `secrets`).

use ritsu_base::diag::Diag;
use ritsu_base::naming::Tool;
use ritsu_base::secrets::Found;
use ritsu_base::text::{Lang, Text, capitalize};
use ritsu_base::tr;
use ritsu_ports::Finding;
use ritsu_project::{Joined, Landing, Project};
use std::collections::BTreeSet;

/// How a contract writes a comment, which decides how a key in it is marked as a value for tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Comments {
    /// `//`: a `.proto`.
    Slashes,
    /// `#`: a YAML document.
    Hash,
    /// None: a JSON document.
    None,
}

impl Comments {
    fn of(rel: &str) -> Comments {
        if rel.ends_with(".proto") {
            Comments::Slashes
        } else if rel.ends_with(".json") {
            Comments::None
        } else {
            Comments::Hash
        }
    }
}

/// Whether a file a language names is a contract document ritsu looks at: a `.json`, a `.yaml` or a
/// `.yml`.
fn is_document(rel: &str) -> bool {
    [".json", ".yaml", ".yml"].iter().any(|e| rel.ends_with(e))
}

/// The contracts of the project, from the root, each once and in the order of their paths: its
/// `.proto` files, and the documents its files name that are there.
pub fn contracts(project: &Project, joined: &Joined) -> Vec<String> {
    let mut out: BTreeSet<String> = project.files.iter().filter(|f| f.tool == Tool::Proto).map(|f| f.rel.clone()).collect();
    let (refs, _) = project.references(joined);
    for r in refs {
        if matches!(r.landing, Landing::File) && is_document(&r.target.path) && ritsu_base::fs::is_file(ritsu_base::paths::on_disk(&project.root, &r.target.path)) {
            out.insert(r.target.path.clone());
        }
    }
    out.into_iter().collect()
}

/// W901 for each key written in a contract of the project, but those whose line says `ritsu: test
/// secret`. The diagnostic gives the kind of key, its prefix and its length, never the key; and it
/// quotes no line, since the line holds the key.
pub fn keys(project: &Project, joined: &Joined, lang: Lang) -> Vec<Finding> {
    let mut out = Vec::new();
    for rel in contracts(project, joined) {
        let disk = ritsu_base::paths::on_disk(&project.root, &rel);
        // a `.proto` that does not read as text is E101's to say; a document, its language's
        let Ok(src) = ritsu_base::fs::read_to_string(&disk) else { continue };
        let shown = project.shown.path(&rel);
        for f in ritsu_base::secrets::scan(&src).iter().filter(|f| !f.test) {
            let d = key_written(&shown, &rel, f);
            out.push(Finding::of(&d, Some(rel.clone()), lang));
        }
    }
    out
}

/// The diagnostic of one key in the contract at `rel` (from the root), shown as `shown`.
fn key_written(shown: &str, rel: &str, f: &Found) -> Diag {
    let name = &f.kind.name;
    let private = f.kind.provider.is_empty();
    let msg = if private {
        tr!("{}がここに書かれています（{}）", "{} is written here ({})", name.ja, f.shown; capitalize(&name.en), f.shown)
    } else {
        tr!("{}がここに書かれています（{}、{} 文字）", "{} is written here ({}, {} characters)", name.ja, f.shown, f.len; capitalize(&name.en), f.shown, f.len)
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
    let line = if private { tr!("`-----BEGIN` の行", "its `-----BEGIN` line") } else { tr!("同じ行", "the same line") };
    let test = match Comments::of(rel) {
        Comments::Slashes => test_mark(&line, private, "// ritsu: test secret"),
        Comments::Hash => test_mark(&line, private, "# ritsu: test secret"),
        Comments::None => tr!(
            "JSON にはコメントが書けません。テスト用の値なら、例の値（`EXAMPLE` で終わる AWS のアクセスキー ID や、接頭辞のあとが一つの文字の繰り返しの値）に替えてください。",
            "JSON has no comments: if it is a value for tests, put an example value in its place (an AWS access key ID that ends in `EXAMPLE`, or one character over and over after the prefix)."
        ),
    };
    Diag::warning("W901", shown, f.line, f.col, msg)
        .rel(rel)
        .note(tr!(
            "ファイルに書いた鍵は、リポジトリとその履歴とビルドを読めるすべての人に渡ります。鍵はコードが動くところ（環境変数、プラットフォームの接続やシークレットの置き場）に置き、そこから読んでください。",
            "A key in a file reaches everyone who can read the repository, its history and its builds. Keep it where the code runs (an environment variable, the platform's connection or secret store) and read it from there."
        ))
        .note(revoke)
        .note(test)
}

/// What a value (or a key) for tests says, on `line`, in the comment the contract writes.
fn test_mark(line: &Text, private: bool, comment: &str) -> Text {
    if private {
        tr!("テスト用の鍵なら、{}に `{comment}` と書いてください。", "If it is a key for tests, write `{comment}` on {}.", line.ja; line.en)
    } else {
        tr!("テスト用の値なら、{}に `{comment}` と書いてください。", "If it is a value for tests, write `{comment}` on {}.", line.ja; line.en)
    }
}
