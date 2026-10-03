//! Naming an artifact, or a thing in one (DESIGN 6.2): `<tool> "<path>" [<kind> <name>]...`.
//!
//! yuen and sakai settled this form together and each wrote it; `tests/fixtures/naming.tsv` is
//! the table both were held to, and is now this module's. Reading a naming from text, checking
//! its tool, its path and its kinds, writing it back as one line, and turning it into JSON are
//! all here, so no part of ritsu can spell a naming two ways.
//!
//! What is wrong with a naming is an [`Error`]: what is wrong ([`ErrorKind`]) and the character
//! it is at. [`Error::text`] says it in this module's words; a language that keeps its own
//! codes and words reads the kind.

use crate::json::Json;
use crate::paths::{self, PathError};
use crate::text::Text;
use crate::tr;

/// The nine tools a naming can start with (DESIGN 6.2, item 2). `ritsu` is not one: it is not
/// a language.
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

/// A tool's kinds, each with the kinds that can come right after it.
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

    /// The extension of the tool's files; `file` names any file.
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

    /// The kinds (DESIGN 6.2, item 4), in the order the design lists them, each with the kinds
    /// that come right after it.
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

    /// The kinds a naming of the tool can start its pairs with.
    pub fn top_kinds(self) -> Vec<&'static str> {
        self.kinds().iter().map(|(k, _)| *k).collect()
    }

    /// The kinds that come right after `kind`; None when `kind` is not a kind a pair starts with.
    pub fn children(self, kind: &str) -> Option<&'static [&'static str]> {
        self.kinds().iter().find(|(k, _)| *k == kind).map(|(_, c)| *c)
    }

    /// The kind a child kind comes right after (`value` → `enum`).
    pub fn parent(self, child: &str) -> Option<&'static str> {
        self.kinds().iter().find(|(_, cs)| cs.contains(&child)).map(|(k, _)| *k)
    }

    /// Whether a naming of the tool can have a pair under a pair (proto, rulec).
    pub fn nests(self) -> bool {
        self.kinds().iter().any(|(_, cs)| !cs.is_empty())
    }
}

/// A naming, checked: the tool, the path from the root, and the pairs of kind and name.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Name {
    pub tool: Tool,
    /// From the root, `/` between the parts; `.` for the root itself.
    pub path: String,
    pub items: Vec<(String, String)>,
}

impl Name {
    /// A whole file.
    pub fn file(tool: Tool, path: impl Into<String>) -> Name {
        Name { tool, path: path.into(), items: vec![] }
    }

    /// The same naming with one more pair.
    pub fn with(mut self, kind: &str, name: impl Into<String>) -> Name {
        self.items.push((kind.to_string(), name.into()));
        self
    }

    /// The naming as one line (DESIGN 6.2, item 7): the path from the root, always in quotes; a
    /// name bare when it can be a word, else in quotes. Read again, it is the same naming.
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

    /// `{"text", "tool", "path", "items"}`, in that order.
    pub fn to_json(&self) -> Json {
        Json::obj([
            ("text", Json::str(self.text())),
            ("tool", Json::str(self.tool.word())),
            ("path", Json::str(&self.path)),
            ("items", Json::arr(self.items.iter().map(|(k, n)| Json::arr([Json::str(k), Json::str(n)])))),
        ])
    }

    /// Whether `self` contains `other`, not counting itself (DESIGN 6.2, item 6): the same file,
    /// and `self`'s pairs are the first of `other`'s, which has more. A file contains everything
    /// in it; a parent pair its children.
    pub fn contains(&self, other: &Name) -> bool {
        self.tool == other.tool && self.path == other.path && self.items.len() < other.items.len() && other.items.starts_with(&self.items)
    }

    /// Whether `self` is `other` or contains it.
    pub fn is_or_contains(&self, other: &Name) -> bool {
        self == other || self.contains(other)
    }

    /// The file the naming is in: the tool and the path, no pairs.
    pub fn whole_file(&self) -> Name {
        Name { tool: self.tool, path: self.path.clone(), items: vec![] }
    }

    /// The kind of the last pair; None for a whole file.
    pub fn kind(&self) -> Option<&str> {
        self.items.last().map(|(k, _)| k.as_str())
    }
}

/// Whether a name can be written without quotes: not empty, and no blank, `"` or `#`.
pub fn is_word(s: &str) -> bool {
    !s.is_empty() && !s.chars().any(|c| c.is_whitespace() || c == '"' || c == '#')
}

/// `"…"`, with `\"` and `\\` the only escapes.
pub fn quote(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        if c == '"' || c == '\\' {
            o.push('\\');
        }
        o.push(c);
    }
    o.push('"');
    o
}

/// A name bare when it can be a word, else in quotes.
pub fn word_or_quote(s: &str) -> String {
    if is_word(s) { s.to_string() } else { quote(s) }
}

/// A word of a naming as written: its text, the character it starts at, whether it was in
/// quotes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Word {
    pub text: String,
    pub at: usize,
    pub quoted: bool,
}

/// A naming as written, not yet checked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Written {
    pub tool: Word,
    pub path: Option<Word>,
    /// The words after the path: kinds and names, and for a scope the kind it gathers.
    pub rest: Vec<Word>,
    /// The character after the last word, for what is missing.
    pub end: usize,
}

/// What is wrong with a naming.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    /// A full-width space outside a string: it splits a name where the eye sees no break.
    FullWidthSpace,
    UnclosedString,
    /// An escape other than `\"` and `\\`: what follows the backslash.
    BadEscape(String),
    /// A `#` outside a string, which starts a comment in a `.ctx` and a `.req`.
    Hash,
    /// Nothing at all.
    Missing,
    UnknownTool(String),
    /// The tool written in quotes.
    QuotedTool(String),
    MissingPath,
    UnquotedPath(String),
    EmptyPath,
    AbsolutePath(String),
    OutsideRoot(String),
    /// A kind written in quotes.
    QuotedKind(String),
    /// A kind for a tool that has none (dandori, file).
    NoKinds(Tool),
    /// A child kind (`value`) that does not come right after its parent (`enum`).
    ChildFirst { kind: String, parent: &'static str },
    UnknownKind { tool: Tool, kind: String },
    /// A second pair for a tool whose names do not nest.
    NoNesting(Tool),
    /// A pair under a kind that has nothing under it.
    NothingUnder(String),
    /// A pair under a parent that takes other kinds.
    WrongChild { kind: String, parent: String, allowed: &'static [&'static str] },
    /// A pair after a child pair.
    TooManyPairs(String),
    /// A kind with no name after it.
    MissingName(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub kind: ErrorKind,
    /// The character of the text read it is at, from 0.
    pub at: usize,
}

fn tool_list(sep: &str) -> String {
    Tool::ALL.iter().map(|t| t.word()).collect::<Vec<_>>().join(sep)
}

impl Error {
    /// What is wrong, in this module's words.
    pub fn text(&self) -> Text {
        match &self.kind {
            ErrorKind::FullWidthSpace => tr!(
                "文字列の外に全角の空白があります。名前に空白を含めるなら、名前を `\"…\"` で囲みます",
                "There is a full-width space outside a string; a name with a blank in it is written in `\"…\"`"
            ),
            ErrorKind::UnclosedString => tr!("閉じていない文字列があります", "A string is not closed"),
            ErrorKind::BadEscape(e) => tr!(
                "文字列の中のエスケープ `\\{e}` は使えません。使えるのは `\\\"` と `\\\\` だけです",
                "The escape `\\{e}` is not taken in a string; only `\\\"` and `\\\\` are"
            ),
            ErrorKind::Hash => tr!("名前に `#` を書くときは、名前を `\"…\"` で囲みます", "A name with `#` in it is written in `\"…\"`"),
            ErrorKind::Missing => tr!("名指しがありません", "There is no naming"),
            ErrorKind::UnknownTool(t) => tr!(
                "`{t}` というツールはありません。名指しは {} のどれかで始めます",
                "`{t}` is not a tool; a naming starts with one of {}",
                tool_list("、");
                tool_list(", ")
            ),
            ErrorKind::QuotedTool(t) => tr!("ツールの語 `{t}` は `\"…\"` で囲まずに書きます", "The tool `{t}` is a word, not a string in quotes"),
            ErrorKind::MissingPath => tr!("パスがありません。名指しは `<ツール> \"<パス>\" …` の形です", "The path is missing; a naming is `<tool> \"<path>\" …`"),
            ErrorKind::UnquotedPath(p) => tr!("パス `{p}` を `\"…\"` で囲みます", "Write the path `{p}` in quotes"),
            ErrorKind::EmptyPath => PathError::Empty.text(""),
            ErrorKind::AbsolutePath(p) => PathError::Absolute.text(p),
            ErrorKind::OutsideRoot(p) => PathError::Outside.text(p),
            ErrorKind::QuotedKind(k) => tr!("種類 `{k}` は `\"…\"` ではなく語で書きます", "The kind `{k}` is a word, not a string in quotes"),
            ErrorKind::NoKinds(t) => {
                let t = t.word();
                tr!("{t} に種類はありません。{t} はファイルで名指します", "{t} has no kinds; name the file")
            }
            ErrorKind::ChildFirst { kind, parent } => tr!("`{kind}` は `{parent}` のすぐあとにしか書けません", "`{kind}` comes only right after `{parent}`"),
            ErrorKind::UnknownKind { tool, kind } => {
                let ks = tool.top_kinds();
                let t = tool.word();
                let en = match ks.len() {
                    0 => String::new(),
                    1 => ks[0].to_string(),
                    n => format!("{} and {}", ks[..n - 1].join(", "), ks[n - 1]),
                };
                tr!("{t} に `{kind}` という種類はありません。{t} の種類は {} です", "`{kind}` is not a kind of {t}; the kinds of {t} are {}", ks.join("、"); en)
            }
            ErrorKind::NoNesting(t) => {
                let t = t.word();
                tr!("{t} には入れ子の種類がありません。組は一つまでです", "{t} has no nested kinds; a naming of {t} has one pair at most")
            }
            ErrorKind::NothingUnder(p) => tr!("`{p}` の下には何も書けません。組は一つにします", "Nothing comes under `{p}`; write one pair"),
            ErrorKind::WrongChild { parent, allowed, .. } => {
                let (ja, en) = (allowed.join("、"), allowed.join(", "));
                tr!("`{parent}` の下に書けるのは {ja} だけです", "Only {en} comes under `{parent}`")
            }
            ErrorKind::TooManyPairs(k) => tr!("子の組は一つまでです。`{k}` の組は書けません", "One child pair at most; `{k}` cannot follow it"),
            ErrorKind::MissingName(k) => tr!("種類 `{k}` のあとに名前がありません", "The kind `{k}` has no name after it"),
        }
    }
}

fn err(kind: ErrorKind, at: usize) -> Error {
    Error { kind, at }
}

/// U+3000, the full-width space of Japanese text.
const FULL_WIDTH_SPACE: char = '\u{3000}';

/// The words of `text`, which ends where the naming ends (a file has taken its comment off),
/// each with the character it starts at. A `#` outside a string is refused.
pub fn words(text: &str) -> Result<Vec<Word>, Error> {
    let cs: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c == FULL_WIDTH_SPACE {
            return Err(err(ErrorKind::FullWidthSpace, i));
        } else if c.is_whitespace() {
            i += 1;
        } else if c == '"' {
            let start = i;
            i += 1;
            let mut s = String::new();
            loop {
                match cs.get(i) {
                    None => return Err(err(ErrorKind::UnclosedString, start)),
                    Some('"') => {
                        i += 1;
                        break;
                    }
                    Some('\\') => match cs.get(i + 1) {
                        Some(e @ ('"' | '\\')) => {
                            s.push(*e);
                            i += 2;
                        }
                        other => return Err(err(ErrorKind::BadEscape(other.map(|c| c.to_string()).unwrap_or_default()), i)),
                    },
                    Some(ch) => {
                        s.push(*ch);
                        i += 1;
                    }
                }
            }
            out.push(Word { text: s, at: start, quoted: true });
        } else if c == '#' {
            return Err(err(ErrorKind::Hash, i));
        } else {
            let start = i;
            while i < cs.len() && !cs[i].is_whitespace() && cs[i] != '"' && cs[i] != '#' {
                i += 1;
            }
            out.push(Word { text: cs[start..i].iter().collect(), at: start, quoted: false });
        }
    }
    Ok(out)
}

/// A naming as written, from its words: the tool, the path, the rest. `end` is the character
/// after the text. None when there are no words.
pub fn written(ws: Vec<Word>, end: usize) -> Option<Written> {
    let mut it = ws.into_iter();
    let tool = it.next()?;
    let path = it.next();
    Some(Written { tool, path, rest: it.collect(), end })
}

/// Check a naming written in a file whose directory is `dir` (a path from the root; `""` or
/// `.` for the root). For a scope, `gather` lets one more kind follow the pairs: it is
/// returned apart.
pub fn resolve(w: &Written, dir: &str, gather: bool) -> Result<(Name, Option<Word>), Error> {
    let tw = &w.tool;
    let tool = match Tool::from_word(&tw.text) {
        Some(t) if !tw.quoted => t,
        Some(_) => return Err(err(ErrorKind::QuotedTool(tw.text.clone()), tw.at)),
        None => return Err(err(ErrorKind::UnknownTool(tw.text.clone()), tw.at)),
    };
    let path = match &w.path {
        None => return Err(err(ErrorKind::MissingPath, w.end)),
        Some(p) if !p.quoted => return Err(err(ErrorKind::UnquotedPath(p.text.clone()), p.at)),
        Some(p) => p,
    };
    let full = match paths::join(dir, &path.text) {
        Ok(f) => f,
        Err(PathError::Empty) => return Err(err(ErrorKind::EmptyPath, path.at)),
        Err(PathError::Absolute) => return Err(err(ErrorKind::AbsolutePath(path.text.clone()), path.at)),
        Err(PathError::Outside) => return Err(err(ErrorKind::OutsideRoot(path.text.clone()), path.at)),
    };
    let mut rest: Vec<&Word> = w.rest.iter().collect();
    let gathered = if gather && rest.len() % 2 == 1 { rest.pop().cloned() } else { None };
    let mut items: Vec<(String, String)> = Vec::new();
    let mut i = 0;
    while i < rest.len() {
        let k = rest[i];
        if k.quoted {
            return Err(err(ErrorKind::QuotedKind(k.text.clone()), k.at));
        }
        check_kind(tool, &items, k)?;
        let Some(n) = rest.get(i + 1) else {
            return Err(err(ErrorKind::MissingName(k.text.clone()), w.end));
        };
        items.push((k.text.clone(), n.text.clone()));
        i += 2;
    }
    if let Some(g) = &gathered {
        if g.quoted {
            return Err(err(ErrorKind::QuotedKind(g.text.clone()), g.at));
        }
        check_kind(tool, &items, g)?;
    }
    Ok((Name { tool, path: full, items }, gathered))
}

/// Whether the kind `k` can come after the pairs so far (DESIGN 6.2, item 1).
fn check_kind(tool: Tool, items: &[(String, String)], k: &Word) -> Result<(), Error> {
    let kt = k.text.as_str();
    if tool.kinds().is_empty() {
        return Err(err(ErrorKind::NoKinds(tool), k.at));
    }
    match items.len() {
        0 => {
            if tool.children(kt).is_some() {
                return Ok(());
            }
            if let Some(p) = tool.parent(kt) {
                return Err(err(ErrorKind::ChildFirst { kind: kt.to_string(), parent: p }, k.at));
            }
            Err(err(ErrorKind::UnknownKind { tool, kind: kt.to_string() }, k.at))
        }
        1 => {
            let parent = items[0].0.as_str();
            let children = tool.children(parent).unwrap_or(&[]);
            if children.contains(&kt) {
                return Ok(());
            }
            if !tool.nests() {
                return Err(err(ErrorKind::NoNesting(tool), k.at));
            }
            if let Some(p) = tool.parent(kt) {
                return Err(err(ErrorKind::ChildFirst { kind: kt.to_string(), parent: p }, k.at));
            }
            if children.is_empty() {
                return Err(err(ErrorKind::NothingUnder(parent.to_string()), k.at));
            }
            Err(err(ErrorKind::WrongChild { kind: kt.to_string(), parent: parent.to_string(), allowed: children }, k.at))
        }
        _ => Err(err(ErrorKind::TooManyPairs(kt.to_string()), k.at)),
    }
}

/// A naming read from text written in a file whose directory is `dir` (from the root).
pub fn parse(text: &str, dir: &str) -> Result<Name, Error> {
    let ws = words(text)?;
    let w = written(ws, text.chars().count()).ok_or(err(ErrorKind::Missing, 0))?;
    resolve(&w, dir, false).map(|(n, _)| n)
}

/// One naming on its own, written in a file at the root: a line of `tests/fixtures/naming.tsv`.
pub fn parse_one(text: &str) -> Result<Name, Error> {
    parse(text, "")
}
