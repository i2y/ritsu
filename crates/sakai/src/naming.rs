//! Naming an artifact, or a thing in one (DESIGN 2): `<tool> "<path>" [<kind> <name>]...`.
//!
//! yuen names things the same way, letter for letter; `tests/fixtures/naming.tsv` is the table
//! both repositories hold and test against. Reading a name from text, checking its kinds,
//! printing it, and turning it into JSON are all here, so `api`, the diagnostics and the `.ctx`
//! reader cannot spell a name two ways.

use crate::i18n::Text;
use crate::paths::{self, PathError};
use serde_json::{Value, json};

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

    /// The kinds a name of the tool starts with (DESIGN 2.2). A child kind (`value`, `field`,
    /// `method`) is not among them: it comes only right after its parent.
    pub fn kinds(self) -> &'static [&'static str] {
        match self {
            Tool::Rulec => &["input", "output", "enum", "table", "clause", "define", "derive", "machine", "source"],
            Tool::Koyomi => &["input", "date", "claim", "source"],
            Tool::Chobo => &["unit", "account", "transfer"],
            Tool::Geas => &["claim"],
            Tool::Proto => &["service", "message", "enum"],
            Tool::Yuen => &["requirement", "source"],
            Tool::Sakai => &["context", "term"],
            Tool::Dandori | Tool::File => &[],
        }
    }

    /// The kind that can come right after `kind`, for the tools whose names nest.
    pub fn child(self, kind: &str) -> Option<&'static str> {
        match (self, kind) {
            (Tool::Proto, "service") => Some("method"),
            (Tool::Proto, "message") => Some("field"),
            (Tool::Proto | Tool::Rulec, "enum") => Some("value"),
            _ => None,
        }
    }

    /// The kind a child kind comes right after, for this tool.
    pub fn parent(self, child: &str) -> Option<&'static str> {
        self.kinds().iter().copied().find(|k| self.child(k) == Some(child))
    }

    fn nests(self) -> bool {
        matches!(self, Tool::Proto | Tool::Rulec)
    }
}

/// One name: the tool, the path from the root, and the pairs of kind and name.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Name {
    pub tool: Tool,
    pub path: String,
    pub items: Vec<(String, String)>,
}

impl Name {
    pub fn file(tool: Tool, path: impl Into<String>) -> Name {
        Name { tool, path: path.into(), items: vec![] }
    }

    pub fn with(mut self, kind: &str, name: impl Into<String>) -> Name {
        self.items.push((kind.to_string(), name.into()));
        self
    }

    /// The name as it is written (DESIGN 2.6): the path always in quotes, a name in quotes only
    /// when it cannot be written as a word.
    /// A diagnostic writes a name this way too, its path from the root: read again, it is the same
    /// name (DESIGN 2.4).
    pub fn text(&self) -> String {
        let mut s = format!("{} {}", self.tool.word(), quote(&self.path));
        for (k, n) in &self.items {
            s.push(' ');
            s.push_str(k);
            s.push(' ');
            s.push_str(&word_or_quoted(n));
        }
        s
    }

    /// `{"text", "tool", "path", "items"}`, in that order (DESIGN 2.6).
    pub fn to_json(&self) -> Value {
        json!({
            "text": self.text(),
            "tool": self.tool.word(),
            "path": self.path,
            "items": self.items.iter().map(|(k, n)| json!([k, n])).collect::<Vec<_>>(),
        })
    }

    /// Whether `self` holds `other` (DESIGN 2.5): the same file, and `self`'s pairs are the first
    /// of `other`'s. A file holds everything in it; a parent holds its children.
    pub fn contains(&self, other: &Name) -> bool {
        self.tool == other.tool && self.path == other.path && other.items.starts_with(&self.items)
    }
}

/// Whether a name can be written without quotes: no blank, no `"`, no `#`, not empty.
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

pub fn word_or_quoted(s: &str) -> String {
    if is_word(s) { s.to_string() } else { quote(s) }
}

/// A name as written, before its path is made a path from the root: each part with the
/// character offset it starts at in the text read.
#[derive(Clone, Debug, PartialEq)]
pub struct Written {
    pub tool: Tool,
    pub path: String,
    pub path_at: usize,
    /// (kind, where the kind starts, name, where the name starts)
    pub items: Vec<(String, usize, String, usize)>,
    /// How many characters the name took.
    pub len: usize,
}

/// U+3000, the full-width space of Japanese text.
const FULL_WIDTH_SPACE: char = '\u{3000}';

/// A word or a string of the text of a name, with the offset it starts at.
#[derive(Clone, Debug)]
enum Part {
    Word(String, usize),
    Str(String, usize),
}

/// The words and strings of `text`, which ends where the name ends (a `.ctx` has taken its
/// comment off). A `#` outside a string is refused: in a `.ctx` it starts a comment.
fn parts(text: &str) -> Result<Vec<Part>, (usize, Text)> {
    let cs: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        let c = cs[i];
        if c == FULL_WIDTH_SPACE {
            // In a string it is a letter of the name; outside one it would split the name where the
            // eye sees no break (`受領から　60日`), and the `.ctx` lexer refuses it too (E001).
            return Err((i, tr!(
                "文字列の外に全角の空白があります。名前に空白を含めるなら、名前を `\"…\"` で囲みます",
                "there is a full-width space outside a string; a name with a blank in it is written in `\"…\"`"
            )));
        } else if c.is_whitespace() {
            i += 1;
        } else if c == '"' {
            let start = i;
            i += 1;
            let mut s = String::new();
            loop {
                match cs.get(i) {
                    None => return Err((start, tr!("閉じていない文字列があります", "a string is not closed"))),
                    Some('"') => {
                        i += 1;
                        break;
                    }
                    Some('\\') => match cs.get(i + 1) {
                        Some(e @ ('"' | '\\')) => {
                            s.push(*e);
                            i += 2;
                        }
                        other => {
                            let e = other.map(|c| c.to_string()).unwrap_or_default();
                            return Err((i, tr!(
                                "文字列の中のエスケープ `\\{e}` は使えません。使えるのは `\\\"` と `\\\\` だけです",
                                "the escape `\\{e}` is not taken in a string; only `\\\"` and `\\\\` are"
                            )));
                        }
                    },
                    Some(ch) => {
                        s.push(*ch);
                        i += 1;
                    }
                }
            }
            out.push(Part::Str(s, start));
        } else if c == '#' {
            return Err((i, tr!("名前に `#` を書くときは、名前を `\"…\"` で囲みます", "a name with `#` in it is written in `\"…\"`")));
        } else {
            let start = i;
            while i < cs.len() && !cs[i].is_whitespace() && cs[i] != '"' && cs[i] != '#' {
                i += 1;
            }
            out.push(Part::Word(cs[start..i].iter().collect(), start));
        }
    }
    Ok(out)
}

/// Read a name from text (DESIGN 2.1): the tool, the path, and the pairs, with the kinds checked
/// against the tool (DESIGN 2.2). The error is the offset it is at and what is wrong.
pub fn read(text: &str) -> Result<Written, (usize, Text)> {
    let ps = parts(text)?;
    let mut it = ps.into_iter().peekable();
    let tool = match it.next() {
        Some(Part::Word(w, at)) => match Tool::from_word(&w) {
            Some(t) => t,
            None => {
                return Err((at, tr!(
                    "知らないツールの語 `{w}` です。書けるのは rulec、dandori、koyomi、chobo、geas、proto、file、yuen、sakai です",
                    "`{w}` is not a tool; the tools are rulec, dandori, koyomi, chobo, geas, proto, file, yuen and sakai"
                )));
            }
        },
        Some(Part::Str(_, at)) => return Err((at, tr!("名指しはツールの語で始めます", "a name starts with its tool"))),
        None => return Err((0, tr!("名指しがありません", "there is no name"))),
    };
    let (path, path_at) = match it.next() {
        Some(Part::Str(s, at)) => (s, at),
        Some(Part::Word(_, at)) => return Err((at, tr!("パスは `\"…\"` で囲んで書きます", "the path is written in `\"…\"`"))),
        None => return Err((text.chars().count(), tr!("ツールの語のあとに、パスを `\"…\"` で書きます", "the tool is followed by the path, in `\"…\"`"))),
    };
    let mut items: Vec<(String, usize, String, usize)> = Vec::new();
    while let Some(p) = it.next() {
        let (kind, kat) = match p {
            Part::Word(w, at) => (w, at),
            Part::Str(_, at) => return Err((at, tr!("ここには種類の語を書きます", "a kind goes here"))),
        };
        let (name, nat) = match it.next() {
            Some(Part::Word(w, at)) | Some(Part::Str(w, at)) => (w, at),
            None => return Err((kat, tr!("種類 `{kind}` のあとに名前がありません", "the kind `{kind}` has no name after it"))),
        };
        items.push((kind, kat, name, nat));
    }
    check_kinds(tool, &items)?;
    Ok(Written { tool, path, path_at, items, len: text.trim_end().chars().count() })
}

fn kind_list(tool: Tool) -> String {
    tool.kinds().join(", ")
}

/// The kinds of a name, held to its tool (DESIGN 2.1, 2.2).
fn check_kinds(tool: Tool, items: &[(String, usize, String, usize)]) -> Result<(), (usize, Text)> {
    let t = tool.word();
    if items.is_empty() {
        return Ok(());
    }
    if tool == Tool::Dandori {
        return Err((items[0].1, tr!(
            "dandori の成果物には、まだ種類を書けません。dandori が名前を JSON で出さないので、.flow はファイルでだけ名指します",
            "the tool dandori has no kinds yet: dandori does not print its names as JSON, so a .flow is named as a file only"
        )));
    }
    if tool == Tool::File {
        return Err((items[0].1, tr!("`file` には種類を書けません", "`file` has no kinds")));
    }
    let (k0, at0) = (&items[0].0, items[0].1);
    if !tool.kinds().contains(&k0.as_str()) {
        if let Some(p) = tool.parent(k0) {
            return Err((at0, tr!("`{k0}` は `{p}` のすぐあとにだけ書けます", "`{k0}` comes only right after `{p}`")));
        }
        let list = kind_list(tool);
        return Err((at0, tr!("{t} に種類 `{k0}` はありません。書けるのは {list} です", "the tool {t} has no kind `{k0}`; its kinds are {list}")));
    }
    if items.len() == 1 {
        return Ok(());
    }
    let (k1, at1) = (&items[1].0, items[1].1);
    if !tool.nests() {
        return Err((at1, tr!("{t} の種類は入れ子にできません", "the tool {t} has no nested kinds")));
    }
    if tool.child(k0) != Some(k1.as_str()) {
        return Err((at1, match tool.parent(k1) {
            Some(p) => tr!("`{k1}` は `{p}` のすぐあとにだけ書けます", "`{k1}` comes only right after `{p}`"),
            None => tr!("`{k1}` は `{k0}` の下に書けません", "`{k1}` cannot come under `{k0}`"),
        }));
    }
    if items.len() > 2 {
        return Err((items[2].1, tr!("子の組は一つまでです", "a name has one child at most")));
    }
    Ok(())
}

/// A name read from text written in a file whose directory is `base` (a path from the root):
/// what `.ctx` files and the table `tests/fixtures/naming.tsv` both go through.
pub fn parse(text: &str, base: &str) -> Result<Name, Text> {
    let w = read(text).map_err(|(_, t)| t)?;
    let path = paths::join(base, &w.path).map_err(|e: PathError| e.text(&w.path))?;
    Ok(Name { tool: w.tool, path, items: w.items.into_iter().map(|(k, _, n, _)| (k, n)).collect() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_prints_as_it_reads() {
        let n = parse("proto \"shop/v1/order.proto\" message Order.Line field quantity", ".").unwrap();
        assert_eq!(n.text(), "proto \"shop/v1/order.proto\" message Order.Line field quantity");
        let g = parse("geas \"a.geas\" claim \"say \\\"hi\\\" #1\"", ".").unwrap();
        assert_eq!(g.items[0].1, "say \"hi\" #1");
        assert_eq!(g.text(), "geas \"a.geas\" claim \"say \\\"hi\\\" #1\"");
    }

    #[test]
    fn a_parent_holds_its_children() {
        let e = Name::file(Tool::Proto, "a.proto").with("enum", "E");
        let v = e.clone().with("value", "V");
        assert!(e.contains(&v));
        assert!(!v.contains(&e));
        assert!(Name::file(Tool::Proto, "a.proto").contains(&v));
    }
}
