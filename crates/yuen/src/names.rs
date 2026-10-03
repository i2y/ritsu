//! Naming an artifact (DESIGN 2): `<tool> "<path>" [<kind> <name>]...`.
//!
//! sakai names artifacts the same way, to the letter, and `tests/fixtures/naming.tsv` is the
//! table both languages are held to. This module reads a naming the lexer cut out (or a line
//! of that table), checks the tool, the kinds and their nesting and the path, and writes the
//! naming back as text and as JSON.

use crate::i18n::Text;
use serde_json::{Value, json};

/// The nine tools a naming can start with (DESIGN 2.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Tool {
    Rulec,
    Dandori,
    Koyomi,
    Chobo,
    Geas,
    Proto,
    File,
    Yuen,
    Sakai,
}

/// A kind of a tool, and the kinds that can come right after it (DESIGN 2.1).
pub type Kinds = &'static [(&'static str, &'static [&'static str])];

impl Tool {
    pub const ALL: [Tool; 9] = [Tool::Rulec, Tool::Dandori, Tool::Koyomi, Tool::Chobo, Tool::Geas, Tool::Proto, Tool::File, Tool::Yuen, Tool::Sakai];

    pub fn word(self) -> &'static str {
        match self {
            Tool::Rulec => "rulec",
            Tool::Dandori => "dandori",
            Tool::Koyomi => "koyomi",
            Tool::Chobo => "chobo",
            Tool::Geas => "geas",
            Tool::Proto => "proto",
            Tool::File => "file",
            Tool::Yuen => "yuen",
            Tool::Sakai => "sakai",
        }
    }

    pub fn from_word(w: &str) -> Option<Tool> {
        Tool::ALL.into_iter().find(|t| t.word() == w)
    }

    /// The extension of the tool's files, for a directory in a `scope` (DESIGN 1.8). `file`
    /// takes every file.
    pub fn extension(self) -> Option<&'static str> {
        match self {
            Tool::Rulec => Some("rule"),
            Tool::Dandori => Some("flow"),
            Tool::Koyomi => Some("cal"),
            Tool::Chobo => Some("book"),
            Tool::Geas => Some("geas"),
            Tool::Proto => Some("proto"),
            Tool::File => None,
            Tool::Yuen => Some("req"),
            Tool::Sakai => Some("ctx"),
        }
    }

    /// The kinds, in the order DESIGN 2.3 lists them, each with the kinds under it.
    pub fn kinds(self) -> Kinds {
        match self {
            Tool::Rulec => &[
                ("input", &[]),
                ("output", &[]),
                ("enum", &["value"]),
                ("table", &[]),
                ("clause", &[]),
                ("define", &[]),
                ("derive", &[]),
                ("machine", &[]),
                ("source", &[]),
            ],
            Tool::Koyomi => &[("input", &[]), ("date", &[]), ("claim", &[]), ("source", &[])],
            Tool::Chobo => &[("unit", &[]), ("account", &[]), ("transfer", &[])],
            Tool::Geas => &[("claim", &[])],
            Tool::Dandori => &[],
            Tool::Proto => &[("service", &["method"]), ("message", &["field"]), ("enum", &["value"])],
            Tool::File => &[],
            Tool::Yuen => &[("requirement", &[]), ("source", &[])],
            Tool::Sakai => &[("context", &[]), ("term", &[])],
        }
    }

    fn top(self, kind: &str) -> Option<&'static [&'static str]> {
        self.kinds().iter().find(|(k, _)| *k == kind).map(|(_, c)| *c)
    }

    /// The parent a child kind comes right after (`value` → `enum`).
    fn parent_of(self, kind: &str) -> Option<&'static str> {
        self.kinds().iter().find(|(_, cs)| cs.contains(&kind)).map(|(k, _)| *k)
    }

    fn nests(self) -> bool {
        self.kinds().iter().any(|(_, cs)| !cs.is_empty())
    }
}

/// A word of a naming as it was written: its text, its column, and whether it was in quotes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Word {
    pub text: String,
    pub col: usize,
    pub quoted: bool,
}

/// A naming as it was written, not yet checked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Written {
    pub tool: Word,
    pub path: Option<Word>,
    /// The words after the path: kinds and names, and for a `scope` the kind it gathers.
    pub rest: Vec<Word>,
    /// The column after the last word, for a diagnostic about something missing.
    pub end: usize,
}

/// A naming, checked: the path from the root, collapsed, and the pairs.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Name {
    pub tool: Tool,
    /// From the root, `/` between the parts; `.` for the root itself (DESIGN 2.2).
    pub path: String,
    pub items: Vec<(String, String)>,
}

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

/// Whether a name can be written without quotes (DESIGN 2.4).
pub fn is_word(s: &str) -> bool {
    !s.is_empty() && !s.chars().any(|c| c.is_whitespace() || c == '"' || c == '#')
}

/// A string as the language writes it: in quotes, `"` and `\` escaped.
pub fn quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// A name as a naming writes it: bare when it can be a word, else quoted.
pub fn word_or_quote(s: &str) -> String {
    if is_word(s) { s.to_string() } else { quote(s) }
}

impl Name {
    /// The naming as one line (DESIGN 2.4): the path from the root in quotes, a name bare
    /// when it can be a word.
    pub fn text(&self) -> String {
        let mut s = format!("{} {}", self.tool.word(), quote(&self.path));
        for (k, n) in &self.items {
            s.push(' ');
            s.push_str(k);
            s.push(' ');
            s.push_str(&word_or_quote(n));
        }
        s
    }

    /// The JSON of DESIGN 2.6, its keys in their order.
    pub fn to_json(&self) -> Value {
        json!({
            "text": self.text(),
            "tool": self.tool.word(),
            "path": self.path,
            "items": self.items.iter().map(|(k, n)| json!([k, n])).collect::<Vec<_>>(),
        })
    }

    /// Whether `self` contains `other` (DESIGN 2.5): a file contains everything in it, a
    /// parent pair its children. A naming does not contain itself.
    pub fn contains(&self, other: &Name) -> bool {
        self.tool == other.tool && self.path == other.path && self.items.len() < other.items.len() && other.items.starts_with(&self.items)
    }

    /// The file part of the naming: the tool and the path, no pairs.
    pub fn file(&self) -> Name {
        Name { tool: self.tool, path: self.path.clone(), items: vec![] }
    }

    /// The kind of the last pair, or None for a whole file.
    pub fn kind(&self) -> Option<&str> {
        self.items.last().map(|(k, _)| k.as_str())
    }
}

/// `dir` and `path` joined, `.`, `..` and empty parts collapsed by the letters (DESIGN 2.2).
/// None when it climbs above the root. `dir` is from the root, `""` for the root itself.
pub fn collapse(dir: &str, path: &str) -> Option<String> {
    let mut parts: Vec<&str> = Vec::new();
    for p in dir.split('/').chain(path.split('/')) {
        match p {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            _ => parts.push(p),
        }
    }
    Some(if parts.is_empty() { ".".to_string() } else { parts.join("/") })
}

/// Whether a path is absolute: `/…`, `\…`, or a drive letter (`C:`).
pub fn is_absolute(path: &str) -> bool {
    let b = path.as_bytes();
    path.starts_with('/') || path.starts_with('\\') || (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':')
}

fn tool_list() -> String {
    Tool::ALL.iter().map(|t| t.word()).collect::<Vec<_>>().join(", ")
}

fn tool_list_ja() -> String {
    Tool::ALL.iter().map(|t| t.word()).collect::<Vec<_>>().join("、")
}

fn kind_list(t: Tool) -> (String, String) {
    let ks: Vec<&str> = t.kinds().iter().map(|(k, _)| *k).collect();
    let en = match ks.len() {
        0 => String::new(),
        1 => ks[0].to_string(),
        n => format!("{} and {}", ks[..n - 1].join(", "), ks[n - 1]),
    };
    (ks.join("、"), en)
}

/// Check a naming written in a `.req` whose directory is `dir` (from the root). For a
/// `scope`, `gather` lets one kind follow the pairs (DESIGN 1.8): it is returned apart.
pub fn resolve(w: &Written, dir: &str, gather: bool) -> Result<(Name, Option<Word>), NameError> {
    let tw = &w.tool;
    let tool = match Tool::from_word(&tw.text) {
        Some(t) if !tw.quoted => t,
        _ => {
            let t = &tw.text;
            let mut e = err(
                "E011",
                tw.col,
                tr!(
                    "`{t}` というツールはありません。名指しは {} のどれかで始めます",
                    "`{t}` is not a tool; a naming starts with one of {}",
                    tool_list_ja();
                    tool_list()
                ),
            );
            if t == "dir" {
                e = e.note(tr!(
                    "ディレクトリは名指しの語にしません。範囲なら `scope file \"src/\"` のように書きます。",
                    "A directory is not named with a tool word of its own; in a scope, write `scope file \"src/\"`."
                ));
            }
            return Err(e);
        }
    };
    let path = match &w.path {
        None => {
            return Err(err("E013", w.end, tr!("パスがありません。名指しは `<ツール> \"<パス>\" …` の形です", "The path is missing; a naming is `<tool> \"<path>\" …`")));
        }
        Some(p) if !p.quoted => {
            let t = &p.text;
            return Err(err("E013", p.col, tr!("パス `{t}` を `\"…\"` で囲みます", "Write the path `{t}` in quotes"))
                .note(tr!("名指しのパスは、いつも `\"{t}\"` のように書きます。", "The path of a naming is always quoted, like `\"{t}\"`.")));
        }
        Some(p) => p,
    };
    if path.text.is_empty() {
        return Err(err("E013", path.col, tr!("パスが空です", "The path is empty")));
    }
    if is_absolute(&path.text) {
        let t = &path.text;
        return Err(err("E013", path.col, tr!("`{t}` は絶対パスです。名指しを書いたファイルのディレクトリからの相対で書きます", "`{t}` is an absolute path; write it from the directory of the file the naming is in"))
            .note(tr!("絶対パスは、ほかの人の機械では別の場所を指します。", "An absolute path points somewhere else on someone else's machine.")));
    }
    let Some(full) = collapse(dir, &path.text) else {
        let t = &path.text;
        return Err(err("E013", path.col, tr!("`{t}` はルートの外に出ます", "`{t}` goes outside the root"))
            .note(tr!(
                "ルートは、渡したパスの上で `.git` を持つ一番近いディレクトリです（無ければ渡したディレクトリ、`--root` で替えられます）。",
                "The root is the nearest directory above the path given that has a `.git` (else the directory given; `--root` changes it)."
            )));
    };
    let mut rest: Vec<&Word> = w.rest.iter().collect();
    let gathered = if gather && rest.len() % 2 == 1 { rest.pop().cloned() } else { None };
    let mut items: Vec<(String, String)> = Vec::new();
    let mut i = 0;
    while i < rest.len() {
        let k = rest[i];
        if k.quoted {
            return Err(err("E012", k.col, tr!("種類は `\"…\"` ではなく語で書きます", "A kind is a word, not a string in quotes")));
        }
        check_kind(tool, &items, k)?;
        let Some(n) = rest.get(i + 1) else {
            let kt = &k.text;
            return Err(err("E012", w.end, tr!("種類 `{kt}` のあとに名前がありません", "The kind `{kt}` has no name after it")));
        };
        items.push((k.text.clone(), n.text.clone()));
        i += 2;
    }
    if let Some(g) = &gathered {
        if g.quoted {
            return Err(err("E012", g.col, tr!("種類は `\"…\"` ではなく語で書きます", "A kind is a word, not a string in quotes")));
        }
        check_kind(tool, &items, g)?;
    }
    Ok((Name { tool, path: full, items }, gathered))
}

/// Whether kind `k` can come after the pairs `items` so far (DESIGN 2.1).
fn check_kind(tool: Tool, items: &[(String, String)], k: &Word) -> Result<(), NameError> {
    let kt = k.text.as_str();
    let t = tool.word();
    if tool.kinds().is_empty() {
        return Err(match tool {
            Tool::Dandori => err("E012", k.col, tr!("dandori にはまだ種類がありません。ファイルで名指します", "dandori has no kinds yet; name the file"))
                .note(tr!(
                    "dandori はタスクや案件の一覧を JSON で出さないので、yuen は `.flow` の中を名指せません（DESIGN 2.7）。`dandori \"order.flow\"` のように書き、どのタスクかは要件の文か `decided` に書きます。",
                    "dandori does not list its tasks and cases as JSON, so yuen cannot name what is inside a `.flow` (DESIGN 2.7). Write `dandori \"order.flow\"`, and say which task in the requirement's text or a `decided`."
                )),
            _ => err("E012", k.col, tr!("file に種類はありません。file はファイルを丸ごと名指します", "file has no kinds; it names a whole file")),
        });
    }
    match items.len() {
        0 => {
            if tool.top(kt).is_some() {
                return Ok(());
            }
            if let Some(p) = tool.parent_of(kt) {
                return Err(err("E012", k.col, tr!("`{kt}` は `{p}` のすぐあとにしか書けません", "`{kt}` comes only right after `{p}`")));
            }
            let (ja, en) = kind_list(tool);
            Err(err("E012", k.col, tr!("{t} に `{kt}` という種類はありません。{t} の種類は {} です", "`{kt}` is not a kind of {t}; the kinds of {t} are {}", ja; en)))
        }
        1 => {
            let parent = items[0].0.as_str();
            let children = tool.top(parent).unwrap_or(&[]);
            if children.contains(&kt) {
                return Ok(());
            }
            if !tool.nests() {
                return Err(err("E012", k.col, tr!("{t} には入れ子の種類がありません。組は一つまでです", "{t} has no nested kinds; a naming of {t} has one pair at most")));
            }
            if let Some(p) = tool.parent_of(kt) {
                return Err(err("E012", k.col, tr!("`{kt}` は `{p}` のすぐあとにしか書けません", "`{kt}` comes only right after `{p}`")));
            }
            if children.is_empty() {
                return Err(err("E012", k.col, tr!("`{parent}` の下には何も書けません。組は一つにします", "Nothing comes under `{parent}`; write one pair")));
            }
            let (ja, en) = (children.join("、"), children.join(", "));
            Err(err("E012", k.col, tr!("`{parent}` の下に書けるのは {ja} だけです", "Only {en} comes under `{parent}`")))
        }
        _ => Err(err("E012", k.col, tr!("子の組は一つまでです。`{kt}` の組は書けません", "One child pair at most; `{kt}` cannot follow it"))),
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
            Tok::Str(s) => Word { text: s.clone(), col: t.col, quoted: true },
            Tok::Word(s) => Word { text: s.clone(), col: t.col, quoted: false },
            other => Word { text: other.spelled(), col: t.col, quoted: false },
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
