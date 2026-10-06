//! The checks of security that are sekisho's own (ritsu's DESIGN 16): a key of a known shape written
//! in a `.gate` (W901, with ritsu-base's scan, as every language of ritsu looks at its own files);
//! and an input of an action that the contract of the operation it guards marks secret (W910,
//! `contracts.rs`), whose value goes into the request Cedar is asked.

use crate::diag::Diag;
use ritsu_base::secrets;

/// W901: every key of a known shape in the text of the file, in a string or in a comment, but the
/// ones marked as values for tests (`ritsu: test secret` on the line). The line is not shown: it
/// holds the key, which would then be in the log of every run.
pub fn keys(path: &str, src: &str) -> Vec<Diag> {
    secrets::scan(src).iter().filter(|k| !k.test).map(|k| key(path, k)).collect()
}

fn key(path: &str, k: &secrets::Found) -> Diag {
    let (name, prefix, n) = (&k.kind.name, &k.shown, k.len);
    let revoke = match k.kind.provider {
        "" => tr!(
            "本物の鍵なら、まず新しい鍵に替えて、古い鍵を使えないようにしてください。ファイルから消しても、リポジトリの履歴には残ります。",
            "If this key is real, replace it with a new one first, and see that the old one is no longer accepted: taking it out of the file leaves it in the history of the repository."
        ),
        p => tr!(
            "本物の鍵なら、まず {p} で無効にしてください。ファイルから消しても、リポジトリの履歴には残ります。",
            "If this key is real, revoke it with {p} first: taking it out of the file leaves it in the history of the repository."
        ),
    };
    Diag::at("W901", path, k.line, k.col, tr!("{}がここに書かれています（{prefix}、{n} 文字）", "{} is written here ({prefix}, {n} characters)", name.ja; name.en))
        .note(tr!(
            "ファイルに書いた鍵は、リポジトリとその履歴とビルドを読めるすべての人に渡ります。鍵はコードが動くところ（環境変数、プラットフォームの接続やシークレットの置き場）に置き、そこから読んでください。",
            "A key in a file reaches everyone who can read the repository, its history and its builds. Keep it where the code runs (an environment variable, the platform's connection or secret store) and read it from there."
        ))
        .note(revoke)
        .note(tr!(
            "テスト用の値なら、同じ行のコメントに `{}` と書いてください。",
            "If it is a value for tests, write `{}` in a comment on the same line.",
            secrets::TEST_MARK
        ))
}
