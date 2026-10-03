//! Naming an artifact (DESIGN 2): `<tool> "<path>" [<kind> <name>]...`.
//!
//! The form is ritsu's, and so is the one implementation of it (`ritsu_base::naming`): reading
//! the tool, the kinds and their nesting, and the path, and writing the naming back as text and
//! as JSON. sakai names artifacts the same way, to the letter, and ritsu-base's
//! `tests/fixtures/naming.tsv` is the table both are held to. What is yuen's is how a naming is
//! cut out of a `.req` (the lexer's tokens, each word with its column), and the codes and the
//! words a diagnostic says what is wrong in: E011 for the tool, E012 for a kind, E013 for the
//! path.

pub use ritsu_base::naming::{Name, Tool, Word, Written, is_word, quote, word_or_quote};
use ritsu_base::naming::{self as base, ErrorKind};
use ritsu_base::text::Text;

/// What is wrong with a naming.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NameError {
    pub code: &'static str,
    pub col: usize,
    pub msg: Text,
    pub notes: Vec<Text>,
}

fn err(code: &'static str, col: usize, msg: Text) -> NameError {
    NameError { code, col, msg, notes: vec![] }
}

impl NameError {
    fn note(mut self, t: Text) -> NameError {
        self.notes.push(t);
        self
    }
}

fn tool_list() -> String {
    Tool::ALL.iter().map(|t| t.word()).collect::<Vec<_>>().join(", ")
}

fn tool_list_ja() -> String {
    Tool::ALL.iter().map(|t| t.word()).collect::<Vec<_>>().join("、")
}

/// Check a naming written in a `.req` whose directory is `dir` (from the root). For a
/// `scope`, `gather` lets one kind follow the pairs (DESIGN 1.8): it is returned apart. The
/// words carry their columns, which a diagnostic points at.
pub fn resolve(w: &Written, dir: &str, gather: bool) -> Result<(Name, Option<Word>), NameError> {
    base::resolve(w, dir, gather).map_err(|e| said(e))
}

/// What is wrong, in yuen's codes and words.
fn said(e: base::Error) -> NameError {
    let col = e.at;
    match e.kind {
        ErrorKind::UnknownTool(t) | ErrorKind::QuotedTool(t) => {
            let mut x = err(
                "E011",
                col,
                tr!(
                    "`{t}` というツールはありません。名指しは {} のどれかで始めます",
                    "`{t}` is not a tool; a naming starts with one of {}",
                    tool_list_ja();
                    tool_list()
                ),
            );
            if t == "dir" {
                x = x.note(tr!(
                    "ディレクトリは名指しの語にしません。範囲なら `scope file \"src/\"` のように書きます。",
                    "A directory is not named with a tool word of its own; in a scope, write `scope file \"src/\"`."
                ));
            }
            x
        }
        ErrorKind::MissingPath => err("E013", col, tr!("パスがありません。名指しは `<ツール> \"<パス>\" …` の形です", "The path is missing; a naming is `<tool> \"<path>\" …`")),
        ErrorKind::UnquotedPath(t) => err("E013", col, tr!("パス `{t}` を `\"…\"` で囲みます", "Write the path `{t}` in quotes"))
            .note(tr!("名指しのパスは、いつも `\"{t}\"` のように書きます。", "The path of a naming is always quoted, like `\"{t}\"`.")),
        ErrorKind::EmptyPath => err("E013", col, tr!("パスが空です", "The path is empty")),
        ErrorKind::AbsolutePath(t) => err("E013", col, tr!("`{t}` は絶対パスです。名指しを書いたファイルのディレクトリからの相対で書きます", "`{t}` is an absolute path; write it from the directory of the file the naming is in"))
            .note(tr!("絶対パスは、ほかの人の機械では別の場所を指します。", "An absolute path points somewhere else on someone else's machine.")),
        ErrorKind::OutsideRoot(t) => err("E013", col, tr!("`{t}` はルートの外に出ます", "`{t}` goes outside the root")).note(tr!(
            "ルートは、渡したパスの上で `.git` を持つ一番近いディレクトリです（無ければ渡したディレクトリ、`--root` で替えられます）。",
            "The root is the nearest directory above the path given that has a `.git` (else the directory given; `--root` changes it)."
        )),
        ErrorKind::QuotedKind(_) => err("E012", col, tr!("種類は `\"…\"` ではなく語で書きます", "A kind is a word, not a string in quotes")),
        ErrorKind::NoKinds(Tool::Dandori) => err("E012", col, tr!("dandori にはまだ種類がありません。ファイルで名指します", "dandori has no kinds yet; name the file")).note(tr!(
            "dandori はタスクや案件の一覧を JSON で出さないので、yuen は `.flow` の中を名指せません（DESIGN 2.7）。`dandori \"order.flow\"` のように書き、どのタスクかは要件の文か `decided` に書きます。",
            "dandori does not list its tasks and cases as JSON, so yuen cannot name what is inside a `.flow` (DESIGN 2.7). Write `dandori \"order.flow\"`, and say which task in the requirement's text or a `decided`."
        )),
        ErrorKind::NoKinds(_) => err("E012", col, tr!("file に種類はありません。file はファイルを丸ごと名指します", "file has no kinds; it names a whole file")),
        // The rest of what a kind can do wrong is said in ritsu-base's words, which were yuen's.
        kind @ (ErrorKind::ChildFirst { .. } | ErrorKind::UnknownKind { .. } | ErrorKind::NoNesting(_) | ErrorKind::NothingUnder(_) | ErrorKind::WrongChild { .. } | ErrorKind::TooManyPairs(_) | ErrorKind::MissingName(_)) => {
            err("E012", col, base::Error { kind, at: col }.text())
        }
        // What the lexer reads before a naming is resolved; resolve does not say it.
        kind => err("E001", col, base::Error { kind, at: col }.text()),
    }
}

/// One naming on its own, as a line of `tests/fixtures/naming.tsv` writes it (the file it is
/// written in at the root).
pub fn parse_one(s: &str) -> Result<Name, NameError> {
    let tokens = crate::lex::naming_tokens(s).map_err(|(col, msg)| err("E001", col, msg))?;
    let w = read(&tokens, s.chars().count() + 1).ok_or_else(|| err("E011", 1, tr!("名指しがありません", "There is no naming")))?;
    resolve(&w, "", false).map(|(n, _)| n)
}

/// The words of a naming, from the lexer's tokens: the tool, the path, and the rest. None when
/// there are no tokens at all.
pub fn read(tokens: &[crate::lex::Token], end: usize) -> Option<Written> {
    use crate::lex::Tok;
    let word = |t: &crate::lex::Token| -> Word {
        match &t.tok {
            Tok::Str(s) => Word { text: s.clone(), at: t.col, quoted: true },
            Tok::Word(s) => Word { text: s.clone(), at: t.col, quoted: false },
            other => Word { text: other.spelled(), at: t.col, quoted: false },
        }
    };
    let first = tokens.first()?;
    Some(Written { tool: word(first), path: tokens.get(1).map(word), rest: tokens.iter().skip(2).map(word).collect(), end })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapsing() {
        let collapse = |dir: &str, path: &str| ritsu_base::paths::join(dir, path).ok();
        assert_eq!(collapse("", "./calendars/../支払条件.cal").as_deref(), Some("支払条件.cal"));
        assert_eq!(collapse("sub", "../x/./y.rule").as_deref(), Some("x/y.rule"));
        assert_eq!(collapse("a", "b//c/").as_deref(), Some("a/b/c"));
        assert_eq!(collapse("", ".").as_deref(), Some("."));
        assert_eq!(collapse("", "../outside.txt"), None);
        assert_eq!(collapse("a/b", "../../x").as_deref(), Some("x"));
        assert_eq!(collapse("a/b", "../../../x"), None);
    }

    #[test]
    fn words_and_quotes() {
        assert!(is_word("40営業日以内"));
        assert!(is_word("Order.Line"));
        assert!(!is_word("rejects an empty name"));
        assert!(!is_word("a#b"));
        assert!(!is_word(""));
        assert_eq!(quote("say \"hi\" #1"), "\"say \\\"hi\\\" #1\"");
    }
}
