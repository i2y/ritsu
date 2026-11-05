//! A file's code on the site (DESIGN 17.17): every line in an element of its own, numbered by the
//! stylesheet so that a number is neither found nor copied with the text, and coloured as the
//! playground's editor colours it, when the site is built: nothing of this runs in the browser.
//!
//! The languages write comments, strings and numbers alike, so one reader serves them all, each with
//! its words: those of the playground's words.json, which a test of the playground holds to the
//! languages' own lists, and kaname's, which the playground does not run yet, from its table.

use ritsu_base::docpage::esc;
use ritsu_base::modules::manifest;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

const WORDS: &str = include_str!("../../../../../website/docs/playground/words.json");

/// The words of a `ritsu.mod` that are not the start of a statement.
const MOD_WORDS: [&str; 7] = ["oci", "git", "path", "by", "matching", "issuer", "key"];

/// The words of a `.proto` file.
const PROTO_WORDS: [&str; 40] = [
    "syntax", "edition", "package", "import", "public", "weak", "option", "message", "enum", "service", "rpc", "returns", "stream", "repeated", "optional", "required", "map", "oneof", "reserved", "extensions", "extend", "to", "max", "true", "false", "double", "float", "int32", "int64", "uint32", "uint64", "sint32", "sint64", "fixed32", "fixed64", "sfixed32", "sfixed64", "bool", "string", "bytes",
];

/// A line longer than this, in a file with no language to colour, is wrapped: the long lines of data
/// and of a law's XML are read, and a line of a language's table is read in its columns.
const LONG_LINE: usize = 200;

/// What colours a file: what starts its comments, and its words.
struct Language {
    comment: &'static str,
    words: BTreeSet<&'static str>,
}

fn language_of(rel: &str) -> Option<Language> {
    static FILE: OnceLock<BTreeMap<String, Vec<String>>> = OnceLock::new();
    let name = rel.rsplit('/').next().unwrap_or(rel);
    let hash = |words: BTreeSet<&'static str>| Some(Language { comment: "#", words });
    if name == "ritsu.mod" {
        return hash(manifest::STATEMENTS.iter().chain(&MOD_WORDS).copied().collect());
    }
    if name == "ritsu.lock" {
        return hash(BTreeSet::new());
    }
    match name.rsplit_once('.')?.1 {
        "kn" => hash(kaname::kw::TABLE.iter().flat_map(|(_, words)| words.iter().copied()).collect()),
        "proto" => Some(Language { comment: "//", words: PROTO_WORDS.iter().copied().collect() }),
        extension => {
            let file = FILE.get_or_init(|| serde_json::from_str(WORDS).expect("words.json is a list of words for each language"));
            hash(file.get(extension)?.iter().map(String::as_str).collect())
        }
    }
}

/// How long the string at the start of `s` is, its quotes in it: to its closing quote, or to the end
/// of the line when it has none.
fn string_len(s: &str) -> usize {
    let mut chars = s.char_indices().skip(1);
    while let Some((i, c)) = chars.next() {
        match c {
            '\\' => {
                chars.next();
            }
            '"' => return i + 1,
            _ => {}
        }
    }
    s.len()
}

/// How long the number at the start of `s` is: a date, or digits with their separators and decimals
/// (a version's too); and then the unit written against it (`480min`, `1000円`, `5%`).
fn number_len(s: &str) -> usize {
    let b = s.as_bytes();
    let digits = |from: usize, to: usize| to <= b.len() && b[from..to].iter().all(u8::is_ascii_digit);
    let n = if digits(0, 4) && b.get(4) == Some(&b'-') && digits(5, 7) && b.get(7) == Some(&b'-') && digits(8, 10) {
        10
    } else {
        let mut n = 1;
        loop {
            while b.get(n).is_some_and(|c| c.is_ascii_digit() || *c == b',' || *c == b'_') {
                n += 1;
            }
            if b.get(n) == Some(&b'.') && b.get(n + 1).is_some_and(u8::is_ascii_digit) {
                n += 1;
            } else {
                break n;
            }
        }
    };
    n + run_len(&s[n..], |_, c| is_word(c) || c == '%')
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// How long the run of characters at the start of `s` is whose every character `keep` keeps.
fn run_len(s: &str, keep: impl Fn(&str, char) -> bool) -> usize {
    s.char_indices().find(|(i, c)| !keep(&s[*i..], *c)).map_or(s.len(), |(i, _)| i)
}

/// `token` as HTML. On a row of a table each wide character is in a box two columns wide (`w`), as the
/// playground's editor sets it and as the languages count it when they line a table up: the face a
/// browser falls back to for such a character does not draw it two columns wide, and the `|` of a
/// table written in Japanese would not line up. On any other line the box would set the characters
/// of a name apart.
fn text_of(token: &str, row: bool) -> String {
    if !row || !token.chars().any(is_wide) {
        return esc(token);
    }
    let mut o = String::new();
    let mut rest = token;
    while let Some(c) = rest.chars().next() {
        let n = if is_wide(c) { c.len_utf8() } else { run_len(rest, |_, c| !is_wide(c)) };
        let (part, after) = rest.split_at(n);
        if is_wide(c) {
            o.push_str(&format!("<span class=\"w\">{part}</span>"));
        } else {
            o.push_str(&esc(part));
        }
        rest = after;
    }
    o
}

fn is_wide(c: char) -> bool {
    rulec::diag::char_width(c) == 2 || (0x20000..=0x3FFFD).contains(&(c as u32))
}

/// A line in its colours: `k` a word of its language, `s` a string, `n` a number, `c` a comment and
/// `p` the marks between them. A word alone in brackets is a type's unit or parameter (`duration[min]`,
/// `money[円]`), whatever else the word may be.
fn coloured(line: &str, language: &Language, o: &mut String) {
    let row = line.trim_start().starts_with('|');
    let mut rest = line;
    while let Some(c) = rest.chars().next() {
        let (class, n) = if rest.starts_with(language.comment) {
            ("c", rest.len())
        } else if c == '"' {
            ("s", string_len(rest))
        } else if c.is_ascii_digit() {
            ("n", number_len(rest))
        } else if is_word(c) {
            let n = run_len(rest, |_, c| is_word(c));
            let done = line.len() - rest.len();
            let bracketed = line[..done].ends_with('[') && rest[n..].starts_with(']');
            (if !bracketed && language.words.contains(&rest[..n]) { "k" } else { "" }, n)
        } else if c.is_whitespace() {
            ("", run_len(rest, |_, c| c.is_whitespace()))
        } else {
            ("p", run_len(rest, |at, c| !is_word(c) && !c.is_whitespace() && c != '"' && !at.starts_with(language.comment)))
        };
        let (token, after) = rest.split_at(n);
        let shown = text_of(token, row && class != "c");
        if class.is_empty() {
            o.push_str(&shown);
        } else {
            o.push_str(&format!("<span class=\"{class}\">{shown}</span>"));
        }
        rest = after;
    }
}

/// The element that shows `text`, the file `rel`, and how many lines it has. `attr` goes on the
/// element. Each line's element ends with the line's own newline, so the element's text is the file.
pub(super) fn block(rel: &str, text: &str, attr: &str) -> (String, usize) {
    let language = language_of(rel);
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let wrap = language.is_none() && lines.iter().any(|l| l.chars().count() > LONG_LINE);
    let mut body = String::with_capacity(text.len() * 2);
    for (i, whole) in lines.iter().enumerate() {
        let line = whole.strip_suffix('\n').unwrap_or(whole);
        body.push_str(&format!("<span class=\"l\" id=\"L{}\">", i + 1));
        match &language {
            Some(language) => coloured(line, language, &mut body),
            None => body.push_str(&esc(line)),
        }
        if whole.ends_with('\n') {
            body.push('\n');
        }
        body.push_str("</span>");
    }
    let digits = lines.len().max(1).to_string().len();
    let wrapped = if wrap { " wrap" } else { "" };
    (format!("<pre class=\"source lines{wrapped}\" style=\"--n:{digits}ch\"{attr}><code>{body}</code></pre>\n"), lines.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(rel: &str, text: &str) -> String {
        let mut o = String::new();
        coloured(text, &language_of(rel).unwrap(), &mut o);
        o
    }

    #[test]
    fn a_line_is_coloured_by_its_language() {
        assert_eq!(
            line("a/fee.rule", "inputs  # the \"fee\""),
            "<span class=\"k\">inputs</span>  <span class=\"c\"># the &quot;fee&quot;</span>"
        );
        assert_eq!(
            line("fee.rule", "| <1950000円 | 5% | \"a # b\" | # 年"),
            "<span class=\"p\">|</span> <span class=\"p\">&lt;</span><span class=\"n\">1950000<span class=\"w\">円</span></span> <span class=\"p\">|</span> <span class=\"n\">5%</span> <span class=\"p\">|</span> <span class=\"s\">&quot;a # b&quot;</span> <span class=\"p\">|</span> <span class=\"c\"># 年</span>"
        );
        // only a row of a table sets its wide characters in boxes
        assert_eq!(line("a.kn", "let 日 = 2026-01-01"), "<span class=\"k\">let</span> 日 <span class=\"p\">=</span> <span class=\"n\">2026-01-01</span>");
        assert_eq!(line("a.rule", "  | 年(a) | -> b |"), "  <span class=\"p\">|</span> <span class=\"w\">年</span><span class=\"p\">(</span>a<span class=\"p\">)</span> <span class=\"p\">|</span> <span class=\"p\">-&gt;</span> b <span class=\"p\">|</span>");
        // a unit against its number, and one alone in brackets, is no word of the language
        assert_eq!(line("a.rule", "permitted : duration[min]  round down(1min)"), "permitted <span class=\"p\">:</span> duration<span class=\"p\">[</span>min<span class=\"p\">]</span>  <span class=\"k\">round</span> <span class=\"k\">down</span><span class=\"p\">(</span><span class=\"n\">1min</span><span class=\"p\">)</span>");
        assert_eq!(line("a.cal", "1,000.50 x_1"), "<span class=\"n\">1,000.50</span> x_1");
        assert_eq!(line("a.proto", "string id = 1; // the id"), "<span class=\"k\">string</span> id <span class=\"p\">=</span> <span class=\"n\">1</span><span class=\"p\">;</span> <span class=\"c\">// the id</span>");
        assert_eq!(line("ritsu.mod", "require a oci \"b\" 1.0.0"), "<span class=\"k\">require</span> a <span class=\"k\">oci</span> <span class=\"s\">&quot;b&quot;</span> <span class=\"n\">1.0.0</span>");
        // a string with no end runs to the end of the line, an escaped quote does not end one
        assert_eq!(line("a.rule", "\"a\\\"b"), "<span class=\"s\">&quot;a\\&quot;b</span>");
    }

    #[test]
    fn the_nine_languages_each_have_their_words() {
        for (rel, word) in [("a.rule", "rule"), ("a.flow", "workflow"), ("a.cal", "calendar"), ("a.book", "book"), ("a.gate", "gate"), ("a.req", "requirements"), ("a.ctx", "context"), ("a.geas", "claim"), ("a.kn", "module"), ("a.proto", "message"), ("ritsu.mod", "signature")] {
            assert!(language_of(rel).unwrap().words.contains(word), "{rel} has no `{word}`");
        }
        assert!(language_of("a.csv").is_none() && language_of("README.md").is_none() && language_of("LICENSE").is_none());
    }

    #[test]
    fn the_text_of_a_block_is_the_file() {
        for text in ["", "a\n", "a", "\n\n", "a\n\nb", "| x |\r\n"] {
            let (html, n) = block("a.rule", text, "");
            assert_eq!(n, text.split_inclusive('\n').count(), "{text:?}");
            let inner = html.split("<code>").nth(1).unwrap().split("</code>").next().unwrap();
            let bare = inner.split('<').enumerate().map(|(i, part)| if i == 0 { part } else { part.split_once('>').unwrap().1 }).collect::<String>();
            assert_eq!(bare, text, "{html}");
        }
        assert!(block("a.rule", "x\n", "").0.starts_with("<pre class=\"source lines\" style=\"--n:1ch\"><code><span class=\"l\" id=\"L1\">x\n</span>"));
        assert!(block("a.rule", &"x\n".repeat(100), " lang=\"ja\"").0.starts_with("<pre class=\"source lines\" style=\"--n:3ch\" lang=\"ja\">"));
    }

    #[test]
    fn only_long_lines_of_a_file_with_no_language_are_wrapped() {
        let long = "x".repeat(LONG_LINE + 1);
        assert!(block("a.xml", &long, "").0.contains("source lines wrap"));
        assert!(!block("a.xml", &"x".repeat(LONG_LINE), "").0.contains("wrap"));
        assert!(!block("a.rule", &long, "").0.contains("wrap"));
    }
}
