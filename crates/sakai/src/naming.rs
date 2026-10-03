//! Naming an artifact, or a thing in one (DESIGN 2): `<tool> "<path>" [<kind> <name>]...`.
//!
//! yuen names things the same way, letter for letter, and the form is ritsu's, written once
//! (`ritsu_base::naming`): the words and strings of a name, the tools and their kinds, the path
//! from the root, the text and the JSON. ritsu-base's `tests/fixtures/naming.tsv` is the table
//! both are held to. What is sakai's is how a `.ctx` holds a name before its path is made a
//! path from the root ([`Written`], each part with where it starts), the order its parts are
//! checked in (the kinds before the path), and the words a diagnostic says what is wrong in.

use crate::paths;
use ritsu_base::naming::{self as base, ErrorKind};
use ritsu_base::text::Text;

pub use ritsu_base::naming::{Name, Tool, is_word, quote};

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

/// Read a name from text (DESIGN 2.1): the tool, the path, and the pairs, with the kinds checked
/// against the tool (DESIGN 2.2). The error is the offset it is at and what is wrong. The text
/// ends where the name ends (a `.ctx` has taken its comment off).
pub fn read(text: &str) -> Result<Written, (usize, Text)> {
    let ws = base::words(text).map_err(|e| (e.at, said(&e.kind, &[])))?;
    let w = base::written(ws, text.chars().count()).ok_or((0, tr!("名指しがありません", "there is no name")))?;
    if w.tool.quoted {
        return Err((w.tool.at, tr!("名指しはツールの語で始めます", "a name starts with its tool")));
    }
    // The tool, and the path's quotes: what ritsu-base checks first, on the name without its pairs
    // and with a path that is the root itself, which nothing refuses (the path comes last).
    let root_path = |w: &base::Written| w.path.clone().map(|p| base::Word { text: ".".into(), ..p });
    let bare = base::Written { rest: vec![], path: root_path(&w), ..w.clone() };
    base::resolve(&bare, ".", false).map_err(|e| (e.at, said(&e.kind, &w.rest)))?;
    // The pairs: a kind is a word, and has a name after it.
    for pair in w.rest.chunks(2) {
        if pair[0].quoted {
            return Err((pair[0].at, tr!("ここには種類の語を書きます", "a kind goes here")));
        }
        if pair.len() == 1 {
            let kind = &pair[0].text;
            return Err((pair[0].at, tr!("種類 `{kind}` のあとに名前がありません", "the kind `{kind}` has no name after it")));
        }
    }
    // The kinds, held to the tool, before the path is made a path from the root (on a path that
    // is the root itself, which nothing refuses).
    let path = w.path.clone().expect("the bare name has its path");
    let probe = base::Written { path: root_path(&w), ..w.clone() };
    base::resolve(&probe, ".", false).map_err(|e| (e.at, said(&e.kind, &w.rest)))?;
    let tool = Tool::from_word(&w.tool.text).expect("the tool was read");
    let items = w.rest.chunks(2).map(|p| (p[0].text.clone(), p[0].at, p[1].text.clone(), p[1].at)).collect();
    Ok(Written { tool, path: path.text, path_at: path.at, items, len: text.trim_end().chars().count() })
}

/// What is wrong with a name, in sakai's words. `rest` is the words after the path, for the
/// kind an error under a parent is about.
fn said(kind: &ErrorKind, rest: &[base::Word]) -> Text {
    match kind {
        ErrorKind::FullWidthSpace => tr!(
            "文字列の外に全角の空白があります。名前に空白を含めるなら、名前を `\"…\"` で囲みます",
            "there is a full-width space outside a string; a name with a blank in it is written in `\"…\"`"
        ),
        ErrorKind::UnclosedString => tr!("閉じていない文字列があります", "a string is not closed"),
        ErrorKind::BadEscape(e) => tr!(
            "文字列の中のエスケープ `\\{e}` は使えません。使えるのは `\\\"` と `\\\\` だけです",
            "the escape `\\{e}` is not taken in a string; only `\\\"` and `\\\\` are"
        ),
        ErrorKind::Hash => tr!("名前に `#` を書くときは、名前を `\"…\"` で囲みます", "a name with `#` in it is written in `\"…\"`"),
        ErrorKind::Missing => tr!("名指しがありません", "there is no name"),
        ErrorKind::UnknownTool(w) => tr!(
            "知らないツールの語 `{w}` です。書けるのは rulec、dandori、koyomi、chobo、geas、proto、file、yuen、sakai です",
            "`{w}` is not a tool; the tools are rulec, dandori, koyomi, chobo, geas, proto, file, yuen and sakai"
        ),
        ErrorKind::QuotedTool(_) => tr!("名指しはツールの語で始めます", "a name starts with its tool"),
        ErrorKind::MissingPath => tr!("ツールの語のあとに、パスを `\"…\"` で書きます", "the tool is followed by the path, in `\"…\"`"),
        ErrorKind::UnquotedPath(_) => tr!("パスは `\"…\"` で囲んで書きます", "the path is written in `\"…\"`"),
        ErrorKind::EmptyPath => paths::error_text(paths::PathError::Empty, ""),
        ErrorKind::AbsolutePath(p) => paths::error_text(paths::PathError::Absolute, p),
        ErrorKind::OutsideRoot(p) => paths::error_text(paths::PathError::Outside, p),
        ErrorKind::QuotedKind(_) => tr!("ここには種類の語を書きます", "a kind goes here"),
        ErrorKind::NoKinds(_) => tr!("`file` には種類を書けません", "`file` has no kinds"),
        ErrorKind::ChildFirst { kind, parent } => tr!("`{kind}` は `{parent}` のすぐあとにだけ書けます", "`{kind}` comes only right after `{parent}`"),
        ErrorKind::UnknownKind { tool, kind } => {
            let (t, list) = (tool.word(), tool.top_kinds().join(", "));
            tr!("{t} に種類 `{kind}` はありません。書けるのは {list} です", "the tool {t} has no kind `{kind}`; its kinds are {list}")
        }
        ErrorKind::NoNesting(tool) => {
            let t = tool.word();
            tr!("{t} の種類は入れ子にできません", "the tool {t} has no nested kinds")
        }
        ErrorKind::NothingUnder(parent) | ErrorKind::WrongChild { parent, .. } => {
            // The kind that cannot come under the parent is the second of the pairs.
            let k1 = rest.get(2).map(|w| w.text.as_str()).unwrap_or("");
            tr!("`{k1}` は `{parent}` の下に書けません", "`{k1}` cannot come under `{parent}`")
        }
        ErrorKind::TooManyPairs(_) => tr!("子の組は一つまでです", "a name has one child at most"),
        ErrorKind::MissingName(k) => tr!("種類 `{k}` のあとに名前がありません", "the kind `{k}` has no name after it"),
    }
}

/// A name read from text written in a file whose directory is `base` (a path from the root):
/// what `.ctx` files and the table `naming.tsv` both go through.
pub fn parse(text: &str, base: &str) -> Result<Name, Text> {
    let w = read(text).map_err(|(_, t)| t)?;
    let path = paths::join(base, &w.path).map_err(|e| paths::error_text(e, &w.path))?;
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
        assert!(e.is_or_contains(&v));
        assert!(!v.is_or_contains(&e));
        assert!(Name::file(Tool::Proto, "a.proto").is_or_contains(&v));
    }
}
