//! YAML and JSON, read into values that know the line and the column they were written at
//! (DESIGN 4.17). The YAML read is the part of YAML 1.2 that goes to JSON and back, as RFC 9512
//! (section 3.4) draws it and as OpenAPI 3.2 and AsyncAPI 3.1 ask their documents to keep to: one
//! document, in UTF-8, whose keys are strings and whose values are JSON's.
//!
//! What is read: block mappings and sequences, flow mappings and sequences, the four kinds of
//! scalar (plain, single-quoted, double-quoted, and the block scalars `|` and `>` with their
//! indentation and chomping indicators), comments, `---` and `...`, `%YAML 1.2`, anchors and
//! aliases (an alias reads as a copy of what its anchor is on), and the tags of JSON's types
//! (`!!str`, `!!int`, `!!float`, `!!bool`, `!!null`, `!!seq`, `!!map`) and `!`. A plain scalar's
//! type is the core schema's (`true`, `null`, `~`, `0x1F`, `1e3`); a key is the string it is
//! written as, as the failsafe schema reads it (`200:` is the key `"200"`).
//!
//! What is not read, and said by name with where it is: a second document, `%YAML 1.1` and
//! `%TAG`, the other tags, a key written with `?`, a key that is no scalar, a key written twice,
//! an alias of what its anchor is still on, `.inf` and `.nan`, a tab in the indentation, a
//! character that is not printable. A reader of the part a file happens to use goes wrong on the
//! next file without a word; this one reads its part whole and stops at the edge of it.
//! `tests/yaml.rs` holds it to the YAML test suite: on every case it reads the value the suite
//! gives, or does not read the case.

use crate::text::Text;
use crate::tr;
use std::collections::HashMap;

/// A value and where it starts (lines and columns from 1; a column counts characters).
#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub value: Value,
    pub line: usize,
    pub col: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i128),
    /// A number with a fraction or an exponent, or a whole number too large for an `i128`, as
    /// the digits that were written.
    Float(String),
    Str(String),
    Seq(Vec<Node>),
    /// The entries in the order they were written.
    Map(Vec<(Key, Node)>),
}

/// A key of a mapping, and where it starts.
#[derive(Clone, Debug, PartialEq)]
pub struct Key {
    pub name: String,
    pub line: usize,
    pub col: usize,
}

/// Why a text is not read, and where.
#[derive(Clone, Debug, PartialEq)]
pub struct Error {
    pub line: usize,
    pub col: usize,
    pub message: Text,
}

impl Node {
    /// The value of a key of a mapping.
    pub fn get(&self, key: &str) -> Option<&Node> {
        self.entry(key).map(|(_, v)| v)
    }

    /// The key and the value of a key of a mapping.
    pub fn entry(&self, key: &str) -> Option<(&Key, &Node)> {
        match &self.value {
            Value::Map(es) => es.iter().find(|(k, _)| k.name == key).map(|(k, v)| (k, v)),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match &self.value {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_map(&self) -> Option<&[(Key, Node)]> {
        match &self.value {
            Value::Map(es) => Some(es),
            _ => None,
        }
    }

    pub fn as_seq(&self) -> Option<&[Node]> {
        match &self.value {
            Value::Seq(xs) => Some(xs),
            _ => None,
        }
    }

    /// The node a JSON Pointer (RFC 6901) names from this one: `/components/schemas/Order`, with
    /// `~1` for `/` and `~0` for `~`; the empty pointer is this node.
    pub fn pointer(&self, pointer: &str) -> Option<&Node> {
        let mut at = self;
        for token in pointer_tokens(pointer)? {
            at = match &at.value {
                Value::Map(es) => &es.iter().find(|(k, _)| k.name == token)?.1,
                Value::Seq(xs) => {
                    if token.is_empty() || (token.len() > 1 && token.starts_with('0')) || !token.bytes().all(|b| b.is_ascii_digit()) {
                        return None;
                    }
                    xs.get(token.parse::<usize>().ok()?)?
                }
                _ => return None,
            };
        }
        Some(at)
    }

    /// The value as JSON's text, without blanks: what the tests compare.
    pub fn to_json_text(&self) -> String {
        let mut out = String::new();
        self.write_json(&mut out);
        out
    }

    fn write_json(&self, out: &mut String) {
        match &self.value {
            Value::Null => out.push_str("null"),
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Value::Int(n) => out.push_str(&n.to_string()),
            Value::Float(f) => out.push_str(f),
            Value::Str(s) => write_json_string(s, out),
            Value::Seq(xs) => {
                out.push('[');
                for (i, x) in xs.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    x.write_json(out);
                }
                out.push(']');
            }
            Value::Map(es) => {
                out.push('{');
                for (i, (k, v)) in es.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_json_string(&k.name, out);
                    out.push(':');
                    v.write_json(out);
                }
                out.push('}');
            }
        }
    }
}

fn write_json_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

/// The tokens of a JSON Pointer, unescaped; None for a pointer that does not start with `/`.
pub fn pointer_tokens(pointer: &str) -> Option<Vec<String>> {
    if pointer.is_empty() {
        return Some(vec![]);
    }
    let rest = pointer.strip_prefix('/')?;
    Some(rest.split('/').map(|t| t.replace("~1", "/").replace("~0", "~")).collect())
}

/// How many nodes the copies made for aliases may add up to, so that a small document of
/// aliases of aliases cannot grow into a huge value.
const MAX_COPIED: usize = 1_000_000;

/// How deep collections may nest.
const MAX_DEPTH: usize = 512;

/// Read a YAML document.
pub fn read_yaml(src: &str) -> Result<Node, Error> {
    let mut p = P::new(src, true)?;
    p.document()
}

/// Read a JSON text (RFC 8259): a key written twice is not read, as in YAML.
pub fn read_json(src: &str) -> Result<Node, Error> {
    let mut p = P::new(src, false)?;
    p.json_text()
}

/// Read a file's text as JSON when its name ends in `.json`, else as YAML.
pub fn read_file_text(name: &str, src: &str) -> Result<Node, Error> {
    if name.ends_with(".json") { read_json(src) } else { read_yaml(src) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum After {
    /// `key:`
    Key,
    /// `-`
    Entry,
    /// `---`
    Start,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tag {
    Str,
    Int,
    Float,
    Bool,
    Null,
    Seq,
    Map,
    /// `!`
    NonSpecific,
}

#[derive(Clone, Debug, Default)]
struct Props {
    anchor: Option<String>,
    tag: Option<(Tag, usize)>,
}

impl Props {
    fn any(&self) -> bool {
        self.anchor.is_some() || self.tag.is_some()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Chomp {
    Strip,
    Clip,
    Keep,
}

struct P {
    s: Vec<char>,
    i: usize,
    /// Where each line starts.
    starts: Vec<usize>,
    anchors: HashMap<String, Node>,
    copied: usize,
    depth: usize,
}

fn is_ws(c: char) -> bool {
    c == ' ' || c == '\t'
}

fn is_flow_indicator(c: char) -> bool {
    matches!(c, ',' | '[' | ']' | '{' | '}')
}

/// Whether YAML 1.2 lets the character be in a stream (`c-printable`), besides the line break.
fn printable(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\u{20}'..='\u{7E}' | '\u{85}' | '\u{A0}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}')
}

impl P {
    fn new(src: &str, yaml: bool) -> Result<P, Error> {
        let src = src.strip_prefix('\u{FEFF}').unwrap_or(src);
        let mut s: Vec<char> = Vec::with_capacity(src.len());
        let mut cs = src.chars().peekable();
        while let Some(c) = cs.next() {
            if c == '\r' {
                if cs.peek() == Some(&'\n') {
                    cs.next();
                }
                s.push('\n');
            } else {
                s.push(c);
            }
        }
        let mut starts = vec![0];
        for (i, c) in s.iter().enumerate() {
            if *c == '\n' {
                starts.push(i + 1);
            }
        }
        let p = P { s, i: 0, starts, anchors: HashMap::new(), copied: 0, depth: 0 };
        if yaml && let Some(at) = p.s.iter().position(|c| !printable(*c)) {
            let u = format!("U+{:04X}", p.s[at] as u32);
            return p.err_at(at, tr!("YAML の文書に書けない文字 {u} があります", "the character {u} cannot be in a YAML document"));
        }
        Ok(p)
    }

    fn c(&self, k: usize) -> char {
        self.s.get(self.i + k).copied().unwrap_or('\0')
    }

    fn eof(&self) -> bool {
        self.i >= self.s.len()
    }

    fn at(&self, i: usize) -> char {
        self.s.get(i).copied().unwrap_or('\0')
    }

    fn pos_of(&self, i: usize) -> (usize, usize) {
        let line = match self.starts.binary_search(&i) {
            Ok(l) => l,
            Err(l) => l - 1,
        };
        (line + 1, i - self.starts[line] + 1)
    }

    fn col(&self) -> usize {
        self.pos_of(self.i).1 - 1
    }

    fn line_start(&self) -> usize {
        self.starts[self.pos_of(self.i).0 - 1]
    }

    fn err_at<T>(&self, i: usize, message: Text) -> Result<T, Error> {
        let (line, col) = self.pos_of(i.min(self.s.len()));
        Err(Error { line, col, message })
    }

    fn err<T>(&self, message: Text) -> Result<T, Error> {
        self.err_at(self.i, message)
    }

    fn node(&self, value: Value, at: usize) -> Node {
        let (line, col) = self.pos_of(at.min(self.s.len()));
        Node { value, line, col }
    }

    fn ws_or_end(&self, k: usize) -> bool {
        let c = self.c(k);
        is_ws(c) || c == '\n' || c == '\0' && self.i + k >= self.s.len()
    }

    fn skip_inline_ws(&mut self) -> usize {
        let from = self.i;
        while is_ws(self.c(0)) {
            self.i += 1;
        }
        self.i - from
    }

    /// Whether the rest of the line is blank or a comment.
    fn rest_is_blank(&self, from: usize) -> bool {
        let mut j = from;
        while is_ws(self.at(j)) {
            j += 1;
        }
        let c = self.at(j);
        j >= self.s.len() || c == '\n' || (c == '#' && (j == from || is_ws(self.at(j - 1)) || j == self.line_start_of(j)))
    }

    fn line_start_of(&self, i: usize) -> usize {
        self.starts[self.pos_of(i).0 - 1]
    }

    /// Past the lines that are blank or hold only a comment, from the start of a line.
    fn skip_lines(&mut self) {
        while !self.eof() && self.rest_is_blank(self.i) {
            match self.s[self.i..].iter().position(|c| *c == '\n') {
                Some(k) => self.i += k + 1,
                None => self.i = self.s.len(),
            }
        }
    }

    /// The indentation of the line that starts here, in spaces; a tab in it is not read.
    fn indent_here(&self) -> Result<usize, Error> {
        let mut k = 0;
        while self.at(self.i + k) == ' ' {
            k += 1;
        }
        if self.at(self.i + k) == '\t' {
            return self.err_at(self.i + k, tr!("字下げにタブがあります。YAML の字下げは空白で書きます", "a tab in the indentation; YAML indents with spaces"));
        }
        Ok(k)
    }

    /// `---` or `...` at the start of a line, followed by a blank or the end of the line.
    fn at_marker(&self, m: &str) -> bool {
        self.marker_at(self.i, m)
    }

    fn marker_at(&self, i: usize, m: &str) -> bool {
        if i != self.line_start_of(i) || i + 3 > self.s.len() {
            return false;
        }
        let mc: Vec<char> = m.chars().collect();
        self.s[i..i + 3] == mc[..] && {
            let c = self.at(i + 3);
            i + 3 >= self.s.len() || is_ws(c) || c == '\n'
        }
    }

    fn at_eol_or_comment(&self) -> bool {
        let c = self.c(0);
        self.eof() || c == '\n' || (c == '#' && (self.i == self.line_start() || is_ws(self.at(self.i - 1))))
    }

    /// To the start of the next line, past blanks and a comment; anything else is not read.
    fn end_of_line(&mut self) -> Result<(), Error> {
        self.skip_inline_ws();
        if self.c(0) == '#' && (self.i == self.line_start() || is_ws(self.at(self.i - 1))) {
            while !self.eof() && self.c(0) != '\n' {
                self.i += 1;
            }
        }
        if self.eof() {
            return Ok(());
        }
        if self.c(0) != '\n' {
            let c = self.c(0);
            return self.err(tr!("値のあとに余分な `{c}` があります", "an extra `{c}` after the value"));
        }
        self.i += 1;
        Ok(())
    }

    fn deeper(&mut self) -> Result<(), Error> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return self.err(tr!("入れ子が深すぎます（{MAX_DEPTH} 段まで）", "nested deeper than {MAX_DEPTH} levels"));
        }
        Ok(())
    }

    // ---- the document

    fn document(&mut self) -> Result<Node, Error> {
        let mut directives = false;
        let mut yaml_seen = false;
        loop {
            self.skip_lines();
            if !self.eof() && self.c(0) == '%' && self.i == self.line_start() {
                self.directive(&mut yaml_seen)?;
                directives = true;
                continue;
            }
            break;
        }
        if self.eof() {
            if directives {
                return self.err(tr!("指示のあとに文書がありません", "a directive with no document after it"));
            }
            return self.err(no_document());
        }
        let node = if self.at_marker("---") {
            self.i += 3;
            self.value_after(-1, After::Start)?
        } else {
            if directives {
                return self.err(tr!("指示のあとは `---` で文書を始めてください", "after a directive, `---` starts the document"));
            }
            if self.at_marker("...") {
                return self.err(no_document());
            }
            let k = self.indent_here()?;
            self.i += k;
            self.block_fresh(-1, Props::default())?
        };
        self.skip_lines();
        if self.at_marker("...") {
            self.i += 3;
            self.end_of_line()?;
            self.skip_lines();
            if !self.eof() {
                return self.err(tr!("`...` のあとに二つ目の文書があります。読むのは一つの文書だけです", "a second document after `...`; one document is read"));
            }
        } else if self.at_marker("---") {
            return self.err(tr!("二つ目の文書があります。読むのは一つの文書だけです", "a second document; one document is read"));
        } else if !self.eof() {
            let k = self.indent_here()?;
            return self.err_at(self.i + k, tr!("文書の値のあとに、続かない行があります（字下げを確かめてください）", "a line that does not go on from the document's value (look at the indentation)"));
        }
        Ok(node)
    }

    fn directive(&mut self, yaml_seen: &mut bool) -> Result<(), Error> {
        let at = self.i;
        self.i += 1;
        let name: String = self.s[self.i..].iter().take_while(|c| !is_ws(**c) && **c != '\n').collect();
        self.i += name.chars().count();
        self.skip_inline_ws();
        match name.as_str() {
            "YAML" => {
                let v: String = self.s[self.i..].iter().take_while(|c| !is_ws(**c) && **c != '\n').collect();
                if *yaml_seen {
                    return self.err_at(at, tr!("`%YAML` の指示が二つあります", "two `%YAML` directives"));
                }
                if v != "1.2" {
                    return self.err_at(at, tr!("YAML {v} の文書です。読むのは YAML 1.2 の文書です", "a YAML {v} document; YAML 1.2 is read"));
                }
                *yaml_seen = true;
                self.i += v.chars().count();
                self.end_of_line()
            }
            "TAG" => self.err_at(at, tr!("`%TAG` の指示は読みません。JSON の型に当たるタグ（`!!str` など）だけを読みます", "the `%TAG` directive is not read; only the tags of JSON's types (`!!str` and the like) are")),
            _ => self.err_at(at, tr!("知らない指示 `%{name}` です", "an unknown directive, `%{name}`")),
        }
    }

    // ---- block nodes

    /// The value after `key:`, `-` or `---`, on this line or on the lines below. `n` is the
    /// indentation of the collection it is in (-1 at the top).
    fn value_after(&mut self, n: isize, after: After) -> Result<Node, Error> {
        let start = self.i;
        if !self.eof() && !is_ws(self.c(0)) && self.c(0) != '\n' {
            // `-x` and `key:x` are read as scalars before this point; `---x` is no marker
            return self.err(tr!("ここに空白が要ります", "a blank goes here"));
        }
        let sep_from = self.i;
        self.skip_inline_ws();
        let props = self.properties(false)?;
        self.skip_inline_ws();
        if self.at_eol_or_comment() {
            self.end_of_line()?;
            return self.block_below(n, after, props, start);
        }
        let at = self.i;
        let col = self.col();
        let tabbed = self.s[sep_from..self.i].contains(&'\t');
        if (self.c(0) == '-' && self.ws_or_end(1)) || (after == After::Entry && self.looks_like_key()) {
            if tabbed {
                return self.err(tr!("タブのあとに、同じ行でコレクションは始められません", "a collection cannot start on a line after a tab"));
            }
            if props.any() && self.c(0) == '-' {
                return self.err(tr!("アンカーやタグと同じ行では、シーケンスは始められません", "a sequence cannot start on the line of an anchor or a tag"));
            }
        }
        match self.c(0) {
            '|' | '>' => {
                let v = self.block_scalar(n, &props)?;
                self.finish(v, props)
            }
            '-' if self.ws_or_end(1) => {
                if after == After::Entry {
                    self.block_sequence(col, props)
                } else {
                    self.err(tr!("シーケンスは、`{}` と同じ行では始められません", "a sequence cannot start on the line of `{}`", if after == After::Key { ":" } else { "---" }; if after == After::Key { ":" } else { "---" }))
                }
            }
            '?' if self.ws_or_end(1) => self.err(explicit_key()),
            _ if self.looks_like_key() => {
                if after == After::Entry {
                    self.block_mapping(col, Props::default(), props)
                } else {
                    self.err(tr!("マップは、`{}` と同じ行では始められません", "a mapping cannot start on the line of `{}`", if after == After::Key { ":" } else { "---" }; if after == After::Key { ":" } else { "---" }))
                }
            }
            _ => {
                let v = self.inline_node(n, &props, at)?;
                self.finish(v, props)
            }
        }
    }

    /// A flow collection, an alias or a scalar on one line of a block (a plain or quoted scalar
    /// may go on over the lines below).
    fn inline_node(&mut self, n: isize, props: &Props, at: usize) -> Result<Node, Error> {
        match self.c(0) {
            '[' | '{' => {
                let v = self.flow_collection(n)?;
                self.skip_inline_ws();
                if self.c(0) == ':' && self.ws_or_end(1) {
                    return self.err_at(at, non_scalar_key());
                }
                self.end_of_line()?;
                Ok(v)
            }
            '*' => {
                if props.any() {
                    return self.err(tr!("エイリアスにアンカーやタグは付けられません", "an alias takes no anchor and no tag"));
                }
                let v = self.alias()?;
                self.skip_inline_ws();
                if self.c(0) == ':' && self.ws_or_end(1) {
                    return self.err_at(at, alias_key());
                }
                self.end_of_line()?;
                Ok(v)
            }
            '"' | '\'' => {
                let text = if self.c(0) == '"' { self.double_quoted(n, false)? } else { self.single_quoted(n, false)? };
                self.skip_inline_ws();
                if self.c(0) == ':' && self.ws_or_end(1) {
                    return self.err(tr!("ここでマップは始められません", "a mapping cannot start here"));
                }
                self.end_of_line()?;
                let v = self.resolve(text, false, props, at)?;
                Ok(self.node(v, at))
            }
            _ => self.plain_block(n, props, at),
        }
    }

    /// The node below a line that ended after `key:`, `-`, `---` or properties.
    fn block_below(&mut self, n: isize, after: After, props: Props, start: usize) -> Result<Node, Error> {
        self.skip_lines();
        if self.eof() || self.at_marker("---") || self.at_marker("...") {
            return self.empty(props, start);
        }
        let k = self.indent_here()? as isize;
        let seq_here = after == After::Key && k == n && self.at(self.i + k as usize) == '-' && {
            let c = self.at(self.i + k as usize + 1);
            is_ws(c) || c == '\n' || self.i + k as usize + 1 >= self.s.len()
        };
        if k > n || seq_here {
            self.i += k as usize;
            return self.block_fresh(n, props);
        }
        self.empty(props, start)
    }

    fn empty(&mut self, props: Props, at: usize) -> Result<Node, Error> {
        if let Some((_, tat)) = props.tag {
            return self.err_at(tat, tr!("値の無いタグは読みません", "a tag on no value is not read"));
        }
        let v = self.resolve(String::new(), true, &props, at)?;
        let node = self.node(v, at);
        self.finish(node, props)
    }

    /// A node that starts a line, at the column it is at; `n` is the indentation of the collection
    /// it is in.
    fn block_fresh(&mut self, n: isize, mut props: Props) -> Result<Node, Error> {
        let k = self.col();
        let at = self.i;
        let mut same_line_props = false;
        if matches!(self.c(0), '&' | '!') {
            let mine = self.properties(false)?;
            self.skip_inline_ws();
            if self.at_eol_or_comment() {
                if props.any() {
                    return self.err_at(at, tr!("一つの値に、アンカーやタグが二度付いています", "one value has two anchors or two tags"));
                }
                self.end_of_line()?;
                return self.block_below(n, After::Start, mine, at);
            }
            if self.looks_like_key() {
                return self.block_mapping(k, props, mine);
            }
            if props.any() && mine.any() {
                return self.err_at(at, tr!("一つの値に、アンカーやタグが二度付いています", "one value has two anchors or two tags"));
            }
            props = mine;
            same_line_props = true;
        }
        let at = self.i;
        if same_line_props && self.c(0) == '-' && self.ws_or_end(1) {
            return self.err(tr!("アンカーやタグと同じ行では、シーケンスは始められません", "a sequence cannot start on the line of an anchor or a tag"));
        }
        match self.c(0) {
            '-' if self.ws_or_end(1) => self.block_sequence(self.col(), props),
            '?' if self.ws_or_end(1) => self.err(explicit_key()),
            ':' if self.ws_or_end(1) => self.err(empty_key()),
            '|' | '>' => {
                let v = self.block_scalar(n, &props)?;
                self.finish(v, props)
            }
            _ if self.looks_like_key() => self.block_mapping(self.col(), props, Props::default()),
            _ => {
                let v = self.inline_node(n, &props, at)?;
                self.finish(v, props)
            }
        }
    }

    /// Whether the line holds an implicit key here: a scalar on this line followed by `:` and a
    /// blank or the end of the line.
    fn looks_like_key(&self) -> bool {
        let mut j = self.i;
        match self.at(j) {
            '"' => {
                j += 1;
                loop {
                    if j >= self.s.len() {
                        return false;
                    }
                    match self.at(j) {
                        '\\' => j += 2,
                        '"' => break,
                        '\n' => return false,
                        _ => j += 1,
                    }
                }
                j += 1;
            }
            '\'' => {
                j += 1;
                loop {
                    match self.at(j) {
                        '\'' if self.at(j + 1) == '\'' => j += 2,
                        '\'' => break,
                        '\n' => return false,
                        _ if j >= self.s.len() => return false,
                        _ => j += 1,
                    }
                }
                j += 1;
            }
            '*' => {
                while j < self.s.len() && !is_ws(self.at(j)) && self.at(j) != '\n' && !is_flow_indicator(self.at(j)) {
                    j += 1;
                }
            }
            '[' | '{' => return false,
            _ => {
                while j < self.s.len() && self.at(j) != '\n' {
                    let c = self.at(j);
                    if c == ':' {
                        let d = self.at(j + 1);
                        if j + 1 >= self.s.len() || is_ws(d) || d == '\n' {
                            return true;
                        }
                    }
                    if is_ws(c) && self.at(j + 1) == '#' {
                        return false;
                    }
                    j += 1;
                }
                return false;
            }
        }
        while is_ws(self.at(j)) {
            j += 1;
        }
        self.at(j) == ':' && {
            let d = self.at(j + 1);
            j + 1 >= self.s.len() || is_ws(d) || d == '\n'
        }
    }

    fn block_mapping(&mut self, m: usize, props: Props, mut key_props: Props) -> Result<Node, Error> {
        self.deeper()?;
        let at = self.i;
        let mut entries: Vec<(Key, Node)> = Vec::new();
        loop {
            if !key_props.any() && matches!(self.c(0), '&' | '!') {
                key_props = self.properties(false)?;
                self.skip_inline_ws();
                if self.at_eol_or_comment() {
                    return self.err(tr!("キーのアンカーやタグのあとに、キーがありません", "an anchor or a tag of a key with no key after it"));
                }
            }
            let key = self.block_key(&key_props)?;
            key_props = Props::default();
            self.skip_inline_ws();
            if !(self.c(0) == ':' && self.ws_or_end(1)) {
                return self.err(tr!("キーのあとに `:` がありません", "no `:` after the key"));
            }
            self.i += 1;
            let v = self.value_after(m as isize, After::Key)?;
            if entries.iter().any(|(k, _)| k.name == key.name) {
                let (line, col) = (key.line, key.col);
                let name = key.name.clone();
                return Err(Error { line, col, message: tr!("キー `{name}` が二度あります", "the key `{name}` is written twice") });
            }
            entries.push((key, v));
            self.skip_lines();
            if self.eof() || self.at_marker("---") || self.at_marker("...") {
                break;
            }
            let k = self.indent_here()?;
            if k < m {
                break;
            }
            if k > m {
                return self.err_at(self.i + k, tr!("字下げが合いません。マップの値のあとの行が、深く字下げされています", "the indentation does not fit: a line after a mapping's value is indented deeper"));
            }
            self.i += k;
            if self.c(0) == '-' && self.ws_or_end(1) {
                return self.err(tr!("マップの中に、同じ字下げのシーケンスの項があります", "an entry of a sequence inside a mapping, at the mapping's indentation"));
            }
            if !(matches!(self.c(0), '&' | '!') || self.looks_like_key()) {
                if self.c(0) == '?' && self.ws_or_end(1) {
                    return self.err(explicit_key());
                }
                if self.c(0) == ':' && self.ws_or_end(1) {
                    return self.err(empty_key());
                }
                return self.err(tr!("マップの中に、`キー: 値` でない行があります", "a line in a mapping that is not `key: value`"));
            }
        }
        self.depth -= 1;
        let node = self.node(Value::Map(entries), at);
        self.finish(node, props)
    }

    fn block_key(&mut self, props: &Props) -> Result<Key, Error> {
        let at = self.i;
        let name = match self.c(0) {
            '"' => self.double_quoted(-1, false)?,
            '\'' => self.single_quoted(-1, false)?,
            '*' => return self.err(alias_key()),
            '[' | '{' => return self.err(non_scalar_key()),
            '?' if self.ws_or_end(1) => return self.err(explicit_key()),
            ':' if self.ws_or_end(1) => return self.err(empty_key()),
            _ => {
                self.check_plain_start(false)?;
                self.plain_line(false)
            }
        };
        if self.pos_of(self.i).0 != self.pos_of(at).0 {
            return self.err_at(at, tr!("キーが二行以上にわたっています", "a key over more than one line"));
        }
        if name.chars().count() > 1024 {
            return self.err_at(at, tr!("キーが 1024 文字を超えています", "a key longer than 1024 characters"));
        }
        self.key_props(props, &name, at)?;
        let (line, col) = self.pos_of(at);
        Ok(Key { name, line, col })
    }

    fn key_props(&mut self, props: &Props, name: &str, at: usize) -> Result<(), Error> {
        if let Some((t, tat)) = props.tag
            && !matches!(t, Tag::Str | Tag::NonSpecific)
        {
            return self.err_at(tat, tr!("キーは文字列です。キーに付けられるタグは `!!str` だけです", "a key is a string; the only tag a key takes is `!!str`"));
        }
        if let Some(a) = &props.anchor {
            let node = self.node(Value::Str(name.to_string()), at);
            self.anchors.insert(a.clone(), node);
        }
        Ok(())
    }

    fn block_sequence(&mut self, s: usize, props: Props) -> Result<Node, Error> {
        self.deeper()?;
        let at = self.i;
        let mut items = Vec::new();
        loop {
            self.i += 1;
            let v = self.value_after(s as isize, After::Entry)?;
            items.push(v);
            self.skip_lines();
            if self.eof() || self.at_marker("---") || self.at_marker("...") {
                break;
            }
            let k = self.indent_here()?;
            if k > s {
                return self.err_at(self.i + k, tr!("字下げが合いません。シーケンスの項のあとの行が、深く字下げされています", "the indentation does not fit: a line after an entry of a sequence is indented deeper"));
            }
            if k < s {
                break;
            }
            let c = self.at(self.i + k);
            let d = self.at(self.i + k + 1);
            if !(c == '-' && (is_ws(d) || d == '\n' || self.i + k + 1 >= self.s.len())) {
                break;
            }
            self.i += k;
        }
        self.depth -= 1;
        let node = self.node(Value::Seq(items), at);
        self.finish(node, props)
    }

    /// Give a node its anchor (and check its tag against a collection).
    fn finish(&mut self, node: Node, props: Props) -> Result<Node, Error> {
        if let Some((t, tat)) = props.tag {
            let ok = match (&node.value, t) {
                (Value::Seq(_), Tag::Seq | Tag::NonSpecific) | (Value::Map(_), Tag::Map | Tag::NonSpecific) => true,
                (Value::Seq(_) | Value::Map(_), _) => false,
                (_, Tag::Seq | Tag::Map) => false,
                _ => true,
            };
            if !ok {
                return self.err_at(tat, tr!("タグが値の種類と合いません", "the tag does not fit the kind of the value"));
            }
        }
        if let Some(a) = props.anchor {
            self.anchors.insert(a, node.clone());
        }
        Ok(node)
    }

    // ---- properties and aliases

    /// An anchor and a tag, in either order, each followed by a blank (or, in a flow collection,
    /// by the end of the entry).
    fn properties(&mut self, flow: bool) -> Result<Props, Error> {
        self.properties_in(flow, -1)
    }

    fn properties_in(&mut self, flow: bool, n: isize) -> Result<Props, Error> {
        let mut p = Props::default();
        loop {
            match self.c(0) {
                '&' if p.anchor.is_none() => {
                    self.i += 1;
                    let name = self.anchor_name()?;
                    p.anchor = Some(name);
                }
                '!' if p.tag.is_none() => {
                    let at = self.i;
                    let t = self.tag()?;
                    p.tag = Some((t, at));
                }
                '&' | '!' => return self.err(tr!("一つの値に、アンカーやタグが二度付いています", "one value has two anchors or two tags")),
                _ => return Ok(p),
            }
            let c = self.c(0);
            if !(is_ws(c) || c == '\n' || self.eof() || (flow && is_flow_indicator(c))) {
                return self.err(tr!("アンカーやタグのあとに空白が要ります", "a blank goes after an anchor or a tag"));
            }
            self.skip_inline_ws();
            if flow {
                self.flow_ws(n)?;
            }
        }
    }

    fn anchor_name(&mut self) -> Result<String, Error> {
        let name: String = self.s[self.i..].iter().take_while(|c| !is_ws(**c) && **c != '\n' && !is_flow_indicator(**c)).collect();
        if name.is_empty() {
            return self.err(tr!("アンカーかエイリアスの名前がありません", "an anchor or an alias with no name"));
        }
        self.i += name.chars().count();
        Ok(name)
    }

    fn tag(&mut self) -> Result<Tag, Error> {
        let at = self.i;
        let text: String = self.s[self.i..].iter().take_while(|c| !is_ws(**c) && **c != '\n' && !is_flow_indicator(**c)).collect();
        self.i += text.chars().count();
        Ok(match text.as_str() {
            "!" => Tag::NonSpecific,
            "!!str" => Tag::Str,
            "!!int" => Tag::Int,
            "!!float" => Tag::Float,
            "!!bool" => Tag::Bool,
            "!!null" => Tag::Null,
            "!!seq" => Tag::Seq,
            "!!map" => Tag::Map,
            _ => return self.err_at(at, tr!("タグ `{text}` は読みません。読むのは JSON の型に当たるタグ（`!!str`、`!!int`、`!!float`、`!!bool`、`!!null`、`!!seq`、`!!map`）と `!` です", "the tag `{text}` is not read; the tags read are those of JSON's types (`!!str`, `!!int`, `!!float`, `!!bool`, `!!null`, `!!seq`, `!!map`) and `!`")),
        })
    }

    fn alias(&mut self) -> Result<Node, Error> {
        let at = self.i;
        self.i += 1;
        let name = self.anchor_name()?;
        let Some(node) = self.anchors.get(&name) else {
            return self.err_at(at, tr!("エイリアス `*{name}` の前に、アンカー `&{name}` の付いた値がありません", "no value with the anchor `&{name}` before the alias `*{name}`"));
        };
        let mut node = node.clone();
        self.copied += size(&node);
        if self.copied > MAX_COPIED {
            return self.err_at(at, tr!("エイリアスがコピーする値が多すぎます（{MAX_COPIED} 個を超えます）", "the aliases copy too many values (more than {MAX_COPIED})"));
        }
        let (line, col) = self.pos_of(at);
        node.line = line;
        node.col = col;
        Ok(node)
    }

    // ---- scalars

    /// Whether a plain scalar can start here (YAML's indicators cannot start one, but `-`, `?`
    /// and `:` can when a character that is no blank follows).
    fn check_plain_start(&self, flow: bool) -> Result<(), Error> {
        let c = self.c(0);
        let ok = match c {
            '-' | '?' | ':' => {
                let d = self.c(1);
                !(is_ws(d) || d == '\n' || self.i + 1 >= self.s.len() || (flow && is_flow_indicator(d)))
            }
            ',' | '[' | ']' | '{' | '}' | '#' | '&' | '*' | '!' | '|' | '>' | '\'' | '"' | '%' | '@' | '`' => false,
            _ => true,
        };
        if ok {
            Ok(())
        } else {
            self.err(tr!("`{c}` で始まる値は、引用符で囲んでください", "a value that starts with `{c}` is written in quotes"))
        }
    }

    /// The text of a plain scalar on this line: up to `: `, ` #`, the end of the line, and in a
    /// flow collection a flow indicator; without the blanks at its end.
    fn plain_line(&mut self, flow: bool) -> String {
        let from = self.i;
        let mut end = self.i;
        let mut j = self.i;
        while j < self.s.len() {
            let c = self.s[j];
            if c == '\n' {
                break;
            }
            if c == ':' {
                let d = self.at(j + 1);
                if j + 1 >= self.s.len() || is_ws(d) || d == '\n' || (flow && is_flow_indicator(d)) {
                    break;
                }
            }
            if flow && is_flow_indicator(c) {
                break;
            }
            if is_ws(c) {
                if self.at(j + 1) == '#' {
                    break;
                }
            } else {
                end = j + 1;
            }
            j += 1;
        }
        self.i = end.max(from);
        self.s[from..self.i].iter().collect()
    }

    /// A plain scalar in a block, over the lines below it that are indented deeper than `n`.
    fn plain_block(&mut self, n: isize, props: &Props, at: usize) -> Result<Node, Error> {
        self.check_plain_start(false)?;
        let mut text = self.plain_line(false);
        loop {
            self.skip_inline_ws();
            if self.c(0) == ':' && self.ws_or_end(1) {
                return self.err(tr!("値の中に `: ` があります。マップは、ここでは始められません", "`: ` inside a value; a mapping cannot start here"));
            }
            if self.c(0) == '#' || self.eof() {
                break;
            }
            // at the end of the line: the next lines may go on with the scalar
            let save = self.i;
            let mut j = self.i + 1;
            let mut blanks = 0;
            loop {
                let mut k = j;
                while is_ws(self.at(k)) {
                    k += 1;
                }
                if k < self.s.len() && self.at(k) == '\n' {
                    blanks += 1;
                    j = k + 1;
                    continue;
                }
                break;
            }
            if j >= self.s.len() {
                break;
            }
            let mut sp = 0;
            while self.at(j + sp) == ' ' {
                sp += 1;
            }
            let mut k = j + sp;
            while is_ws(self.at(k)) {
                k += 1;
            }
            if (sp as isize) <= n || self.at(k) == '#' || self.marker_at(j, "---") || self.marker_at(j, "...") {
                self.i = save;
                break;
            }
            self.i = k;
            let more = self.plain_line(false);
            if blanks == 0 {
                text.push(' ');
            } else {
                text.extend(std::iter::repeat_n('\n', blanks));
            }
            text.push_str(&more);
        }
        self.end_of_line()?;
        let v = self.resolve(text, true, props, at)?;
        Ok(self.node(v, at))
    }

    /// The lines a quoted scalar goes over: the blanks before the break, the empty lines, and
    /// the blanks at the start of the next line. A break with no empty line folds to a space,
    /// each empty line is a line feed. The next line is to be indented deeper than `n` in a
    /// block, and no line is a document marker.
    fn quoted_break(&mut self, n: isize, text: &mut String, keep: usize) -> Result<(), Error> {
        // the blanks at the end of the line are not part of the text
        while text.len() > keep && text.ends_with([' ', '\t']) {
            text.pop();
        }
        self.i += 1;
        let mut blanks = 0;
        loop {
            let ls = self.i;
            if self.marker_at(ls, "---") || self.marker_at(ls, "...") {
                return self.err(tr!("引用符の中に文書の区切りがあります", "a document marker inside quotes"));
            }
            let mut sp = 0;
            while self.at(ls + sp) == ' ' {
                sp += 1;
            }
            let mut k = ls + sp;
            while is_ws(self.at(k)) {
                k += 1;
            }
            if k >= self.s.len() {
                self.i = k;
                return self.err(tr!("閉じていない引用符があります", "a quote is not closed"));
            }
            if self.at(k) == '\n' {
                blanks += 1;
                self.i = k + 1;
                continue;
            }
            if (sp as isize) <= n {
                return self.err_at(k, tr!("引用符の中の行の字下げが足りません", "a line inside quotes is not indented enough"));
            }
            self.i = k;
            break;
        }
        if blanks == 0 {
            text.push(' ');
        } else {
            text.extend(std::iter::repeat_n('\n', blanks));
        }
        Ok(())
    }

    fn single_quoted(&mut self, n: isize, _flow: bool) -> Result<String, Error> {
        let at = self.i;
        self.i += 1;
        let mut text = String::new();
        loop {
            match self.c(0) {
                _ if self.eof() => return self.err_at(at, tr!("閉じていない引用符があります", "a quote is not closed")),
                '\'' if self.c(1) == '\'' => {
                    text.push('\'');
                    self.i += 2;
                }
                '\'' => {
                    self.i += 1;
                    return Ok(text);
                }
                '\n' => {
                    let keep = 0;
                    self.quoted_break(n, &mut text, keep)?;
                }
                c => {
                    text.push(c);
                    self.i += 1;
                }
            }
        }
    }

    fn double_quoted(&mut self, n: isize, _flow: bool) -> Result<String, Error> {
        let at = self.i;
        self.i += 1;
        let mut text = String::new();
        // the blanks at the end of the text that an escape wrote, which a break does not take off
        let mut keep = 0;
        loop {
            match self.c(0) {
                _ if self.eof() => return self.err_at(at, tr!("閉じていない引用符があります", "a quote is not closed")),
                '"' => {
                    self.i += 1;
                    return Ok(text);
                }
                '\\' => {
                    let e = self.c(1);
                    let eat = self.i;
                    self.i += 2;
                    let ch = match e {
                        '0' => '\0',
                        'a' => '\u{7}',
                        'b' => '\u{8}',
                        't' | '\t' => '\t',
                        'n' => '\n',
                        'v' => '\u{B}',
                        'f' => '\u{C}',
                        'r' => '\r',
                        'e' => '\u{1B}',
                        ' ' => ' ',
                        '"' => '"',
                        '/' => '/',
                        '\\' => '\\',
                        'N' => '\u{85}',
                        '_' => '\u{A0}',
                        'L' => '\u{2028}',
                        'P' => '\u{2029}',
                        'x' | 'u' | 'U' => {
                            let len = match e {
                                'x' => 2,
                                'u' => 4,
                                _ => 8,
                            };
                            let hex: String = self.s[self.i.min(self.s.len())..].iter().take(len).collect();
                            if hex.chars().count() != len || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
                                return self.err_at(eat, tr!("`\\{e}` のあとに 16 進の数字が {len} 桁要ります", "`\\{e}` takes {len} hexadecimal digits"));
                            }
                            self.i += len;
                            match char::from_u32(u32::from_str_radix(&hex, 16).unwrap()) {
                                Some(c) => c,
                                None => return self.err_at(eat, tr!("`\\{e}{hex}` は文字ではありません", "`\\{e}{hex}` is no character")),
                            }
                        }
                        '\n' => {
                            // an escaped break: no space, the empty lines are line feeds
                            let mut blanks = 0;
                            loop {
                                let ls = self.i;
                                if self.marker_at(ls, "---") || self.marker_at(ls, "...") {
                                    return self.err(tr!("引用符の中に文書の区切りがあります", "a document marker inside quotes"));
                                }
                                let mut sp = 0;
                                while self.at(ls + sp) == ' ' {
                                    sp += 1;
                                }
                                let mut k = ls + sp;
                                while is_ws(self.at(k)) {
                                    k += 1;
                                }
                                if k >= self.s.len() {
                                    self.i = k;
                                    return self.err_at(at, tr!("閉じていない引用符があります", "a quote is not closed"));
                                }
                                if self.at(k) == '\n' {
                                    blanks += 1;
                                    self.i = k + 1;
                                    continue;
                                }
                                if (sp as isize) <= n {
                                    return self.err_at(k, tr!("引用符の中の行の字下げが足りません", "a line inside quotes is not indented enough"));
                                }
                                self.i = k;
                                break;
                            }
                            text.extend(std::iter::repeat_n('\n', blanks));
                            keep = text.len();
                            continue;
                        }
                        _ => return self.err_at(eat, tr!("引用符の中のエスケープ `\\{e}` は読めません", "the escape `\\{e}` in quotes is not read")),
                    };
                    text.push(ch);
                    keep = text.len();
                }
                '\n' => self.quoted_break(n, &mut text, keep)?,
                c => {
                    text.push(c);
                    self.i += 1;
                }
            }
        }
    }

    /// `|` or `>`: the lines below indented deeper than `n`.
    fn block_scalar(&mut self, n: isize, props: &Props) -> Result<Node, Error> {
        let at = self.i;
        let folded = self.c(0) == '>';
        self.i += 1;
        let mut chomp: Option<Chomp> = None;
        let mut ind: Option<usize> = None;
        for _ in 0..2 {
            match self.c(0) {
                '-' if chomp.is_none() => chomp = Some(Chomp::Strip),
                '+' if chomp.is_none() => chomp = Some(Chomp::Keep),
                d @ '1'..='9' if ind.is_none() => ind = Some(d as usize - '0' as usize),
                _ => break,
            }
            self.i += 1;
        }
        let chomp = chomp.unwrap_or(Chomp::Clip);
        if !(self.eof() || is_ws(self.c(0)) || self.c(0) == '\n') {
            let c = self.c(0);
            return self.err(tr!("ブロックのスカラーの頭に `{c}` は書けません", "`{c}` does not go in the header of a block scalar"));
        }
        self.end_of_line()?;
        // the indentation of the content
        let min = (n + 1).max(0) as usize;
        let indent = match ind {
            Some(d) => (n + d as isize).max(0) as usize,
            None => {
                let mut j = self.i;
                let mut longest_blank = 0;
                let mut found = None;
                while j < self.s.len() {
                    let mut sp = 0;
                    while self.at(j + sp) == ' ' {
                        sp += 1;
                    }
                    let c = self.at(j + sp);
                    if c == '\t' && sp < min {
                        return self.err_at(j + sp, tr!("ブロックのスカラーの字下げにタブがあります", "a tab in the indentation of a block scalar"));
                    }
                    if j + sp >= self.s.len() || c == '\n' {
                        longest_blank = longest_blank.max(sp);
                        j += sp + 1;
                        continue;
                    }
                    found = Some(sp);
                    break;
                }
                match found {
                    Some(sp) if sp >= min => {
                        if longest_blank > sp {
                            return self.err(tr!("ブロックのスカラーの最初の空の行が、中身の行より深く字下げされています", "an empty line at the start of a block scalar is indented deeper than its content"));
                        }
                        sp
                    }
                    _ => min.max(longest_blank).max(min),
                }
            }
        };
        // the lines
        struct Line {
            text: String,
            spaced: bool,
        }
        let mut lead: usize = 0;
        let mut lines: Vec<(usize, Line)> = Vec::new(); // (empty lines before it, the line)
        let mut trailing = 0; // empty lines after the last content line
        loop {
            if self.eof() {
                break;
            }
            let ls = self.i;
            if self.marker_at(ls, "---") || self.marker_at(ls, "...") {
                break;
            }
            let mut sp = 0;
            while self.at(ls + sp) == ' ' && sp < indent {
                sp += 1;
            }
            let c = self.at(ls + sp);
            let at_end = ls + sp >= self.s.len();
            if at_end || c == '\n' {
                // an empty line
                if lines.is_empty() {
                    lead += 1;
                } else {
                    trailing += 1;
                }
                self.i = if at_end { ls + sp } else { ls + sp + 1 };
                continue;
            }
            if sp < indent {
                if c == '\t' {
                    return self.err_at(ls + sp, tr!("ブロックのスカラーの字下げにタブがあります", "a tab in the indentation of a block scalar"));
                }
                // a shorter line ends the scalar
                break;
            }
            let from = ls + sp;
            let mut e = from;
            while e < self.s.len() && self.s[e] != '\n' {
                e += 1;
            }
            let text: String = self.s[from..e].iter().collect();
            let spaced = text.starts_with([' ', '\t']);
            let before = if lines.is_empty() { lead } else { trailing };
            if !lines.is_empty() {
                trailing = 0;
            }
            lines.push((before, Line { text, spaced }));
            self.i = if e < self.s.len() { e + 1 } else { e };
        }
        // the value
        let mut out = String::new();
        if lines.is_empty() {
            if chomp == Chomp::Keep {
                out.extend(std::iter::repeat_n('\n', lead));
            }
        } else {
            for (idx, (before, line)) in lines.iter().enumerate() {
                if idx == 0 {
                    out.extend(std::iter::repeat_n('\n', *before));
                } else {
                    let prev = &lines[idx - 1].1;
                    if folded && !prev.spaced && !line.spaced {
                        if *before == 0 {
                            out.push(' ');
                        } else {
                            out.extend(std::iter::repeat_n('\n', *before));
                        }
                    } else {
                        out.push('\n');
                        out.extend(std::iter::repeat_n('\n', *before));
                    }
                }
                out.push_str(&line.text);
            }
            // the last line ends with a line break, at the end of the text as well (the suite's
            // L24T/01)
            match chomp {
                Chomp::Strip => {}
                Chomp::Clip => out.push('\n'),
                Chomp::Keep => {
                    out.push('\n');
                    out.extend(std::iter::repeat_n('\n', trailing));
                }
            }
        }
        let v = self.resolve(out, false, props, at)?;
        Ok(self.node(v, at))
    }

    /// The value of a scalar: a plain one by the core schema, unless a tag says what it is.
    fn resolve(&self, text: String, plain: bool, props: &Props, at: usize) -> Result<Value, Error> {
        let tag = props.tag.map(|(t, _)| t);
        let tat = props.tag.map(|(_, a)| a).unwrap_or(at);
        let not = |what: &str| -> Result<Value, Error> {
            let t = text.clone();
            self.err_at(tat, tr!("`{t}` は {what} として読めません", "`{t}` does not read as {what}"))
        };
        match tag {
            None if plain => {
                let v = core(&text);
                if let Some(Value::Float(f)) = &v
                    && is_inf_or_nan(f)
                {
                    return self.err_at(at, tr!("`{text}` は JSON の値ではありません（`.inf` と `.nan` は読みません）", "`{text}` is no value of JSON (`.inf` and `.nan` are not read)"));
                }
                Ok(v.unwrap_or(Value::Str(text)))
            }
            None | Some(Tag::Str) | Some(Tag::NonSpecific) => Ok(Value::Str(text)),
            Some(Tag::Null) => {
                if matches!(text.as_str(), "" | "~" | "null" | "Null" | "NULL") {
                    Ok(Value::Null)
                } else {
                    not("null")
                }
            }
            Some(Tag::Bool) => match text.as_str() {
                "true" | "True" | "TRUE" => Ok(Value::Bool(true)),
                "false" | "False" | "FALSE" => Ok(Value::Bool(false)),
                _ => not("bool"),
            },
            Some(Tag::Int) => match int(&text) {
                Some(v) => Ok(v),
                None => not("int"),
            },
            Some(Tag::Float) => match int(&text).or_else(|| float(&text)) {
                Some(Value::Float(f)) if is_inf_or_nan(&f) => self.err_at(at, tr!("`{text}` は JSON の値ではありません（`.inf` と `.nan` は読みません）", "`{text}` is no value of JSON (`.inf` and `.nan` are not read)")),
                Some(Value::Int(i)) => Ok(Value::Float(i.to_string())),
                Some(v) => Ok(v),
                None => not("float"),
            },
            Some(Tag::Seq) | Some(Tag::Map) => self.err_at(tat, tr!("タグが値の種類と合いません", "the tag does not fit the kind of the value")),
        }
    }

    // ---- flow collections

    /// Past blanks, line breaks and comments inside a flow collection in a block indented `n`:
    /// a line of it that holds anything is indented deeper than `n`.
    fn flow_ws(&mut self, n: isize) -> Result<(), Error> {
        loop {
            match self.c(0) {
                ' ' | '\t' => self.i += 1,
                '\n' => {
                    self.i += 1;
                    if self.at_marker("---") || self.at_marker("...") {
                        return self.err(tr!("フローのコレクションの中に文書の区切りがあります", "a document marker inside a flow collection"));
                    }
                    self.flow_line_indent(n)?;
                }
                '#' if self.i == self.line_start() || is_ws(self.at(self.i - 1)) => {
                    while !self.eof() && self.c(0) != '\n' {
                        self.i += 1;
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    /// A line inside a flow collection that holds anything is indented deeper than the block
    /// it is in.
    fn flow_line_indent(&self, n: isize) -> Result<(), Error> {
        let mut sp = 0;
        while self.at(self.i + sp) == ' ' {
            sp += 1;
        }
        let mut k = self.i + sp;
        while is_ws(self.at(k)) {
            k += 1;
        }
        if k >= self.s.len() || self.at(k) == '\n' || self.at(k) == '#' {
            return Ok(());
        }
        if (sp as isize) <= n {
            return self.err_at(k, tr!("フローのコレクションの中の行の字下げが足りません", "a line inside a flow collection is not indented enough"));
        }
        Ok(())
    }

    fn flow_collection(&mut self, n: isize) -> Result<Node, Error> {
        self.deeper()?;
        let v = if self.c(0) == '[' { self.flow_seq(n) } else { self.flow_map(n) }?;
        self.depth -= 1;
        Ok(v)
    }

    fn flow_seq(&mut self, n: isize) -> Result<Node, Error> {
        let at = self.i;
        self.i += 1;
        let mut items = Vec::new();
        loop {
            self.flow_ws(n)?;
            if self.eof() {
                return self.err_at(at, tr!("閉じていない `[` があります", "a `[` is not closed"));
            }
            if self.c(0) == ']' {
                self.i += 1;
                break;
            }
            if self.c(0) == ',' {
                return self.err(tr!("値の無い `,` があります", "a `,` with no value before it"));
            }
            if self.c(0) == '?' && (self.ws_or_end(1) || is_flow_indicator(self.c(1))) {
                return self.err(explicit_key());
            }
            let entry = self.flow_node(n)?;
            self.flow_ws(n)?;
            if self.c(0) == ':' {
                return self.err(tr!("フローのシーケンスの中の `キー: 値` は読みません。`{{キー: 値}}` と書いてください", "a `key: value` inside a flow sequence is not read; write it as `{{key: value}}`"));
            }
            items.push(entry);
            match self.c(0) {
                ',' => self.i += 1,
                ']' => {
                    self.i += 1;
                    break;
                }
                _ if self.eof() => return self.err_at(at, tr!("閉じていない `[` があります", "a `[` is not closed")),
                c => return self.err(tr!("ここには `,` か `]` が要ります（`{c}` があります）", "a `,` or a `]` goes here, not `{c}`")),
            }
        }
        Ok(self.node(Value::Seq(items), at))
    }

    fn flow_map(&mut self, n: isize) -> Result<Node, Error> {
        let at = self.i;
        self.i += 1;
        let mut entries: Vec<(Key, Node)> = Vec::new();
        loop {
            self.flow_ws(n)?;
            if self.eof() {
                return self.err_at(at, tr!("閉じていない `{{` があります", "a `{{` is not closed"));
            }
            if self.c(0) == '}' {
                self.i += 1;
                break;
            }
            if self.c(0) == ',' {
                return self.err(tr!("値の無い `,` があります", "a `,` with no value before it"));
            }
            if self.c(0) == '?' && (self.ws_or_end(1) || is_flow_indicator(self.c(1))) {
                return self.err(explicit_key());
            }
            let props = self.properties_in(true, n)?;
            let kat = self.i;
            let name = match self.c(0) {
                '"' => self.double_quoted(n, true)?,
                '\'' => self.single_quoted(n, true)?,
                '[' | '{' => return self.err(non_scalar_key()),
                '*' => return self.err(alias_key()),
                ':' if self.ws_or_end(1) || is_flow_indicator(self.c(1)) => return self.err(empty_key()),
                ',' | '}' => return self.err(empty_key()),
                _ => {
                    self.check_plain_start(true)?;
                    self.plain_flow_text(n)?
                }
            };
            if self.pos_of(self.i).0 != self.pos_of(kat).0 {
                return self.err_at(kat, tr!("キーが二行以上にわたっています", "a key over more than one line"));
            }
            self.key_props(&props, &name, kat)?;
            let (kl, kc) = self.pos_of(kat);
            let key = Key { name, line: kl, col: kc };
            self.flow_ws(n)?;
            let value = if self.c(0) == ':' {
                self.i += 1;
                self.flow_ws(n)?;
                if matches!(self.c(0), ',' | '}') {
                    self.node(Value::Null, self.i)
                } else {
                    self.flow_node(n)?
                }
            } else if matches!(self.c(0), ',' | '}') {
                return self.err_at(kat, tr!("フローのマップの中の、`:` の無いキーは読みません", "a key with no `:` in a flow mapping is not read"));
            } else {
                let c = self.c(0);
                return self.err(tr!("キーのあとに `:` がありません（`{c}` があります）", "no `:` after the key (there is `{c}`)"));
            };
            if entries.iter().any(|(k, _)| k.name == key.name) {
                let name = key.name.clone();
                return Err(Error { line: key.line, col: key.col, message: tr!("キー `{name}` が二度あります", "the key `{name}` is written twice") });
            }
            entries.push((key, value));
            self.flow_ws(n)?;
            match self.c(0) {
                ',' => self.i += 1,
                '}' => {
                    self.i += 1;
                    break;
                }
                _ if self.eof() => return self.err_at(at, tr!("閉じていない `{{` があります", "a `{{` is not closed")),
                c => return self.err(tr!("ここには `,` か `}}` が要ります（`{c}` があります）", "a `,` or a `}}` goes here, not `{c}`")),
            }
        }
        Ok(self.node(Value::Map(entries), at))
    }

    fn flow_node(&mut self, n: isize) -> Result<Node, Error> {
        let props = self.properties_in(true, n)?;
        let at = self.i;
        let node = match self.c(0) {
            '[' | '{' => self.flow_collection(n)?,
            '*' => {
                if props.any() {
                    return self.err(tr!("エイリアスにアンカーやタグは付けられません", "an alias takes no anchor and no tag"));
                }
                return self.alias();
            }
            '"' => {
                let t = self.double_quoted(n, true)?;
                let v = self.resolve(t, false, &props, at)?;
                self.node(v, at)
            }
            '\'' => {
                let t = self.single_quoted(n, true)?;
                let v = self.resolve(t, false, &props, at)?;
                self.node(v, at)
            }
            ',' | ']' | '}' if props.any() => {
                let v = self.resolve(String::new(), true, &props, at)?;
                self.node(v, at)
            }
            _ => {
                self.check_plain_start(true)?;
                let t = self.plain_flow_text(n)?;
                let v = self.resolve(t, true, &props, at)?;
                self.node(v, at)
            }
        };
        self.finish(node, props)
    }

    /// A plain scalar inside a flow collection, over lines.
    fn plain_flow_text(&mut self, n: isize) -> Result<String, Error> {
        let mut text = self.plain_line(true);
        loop {
            let save = self.i;
            self.skip_inline_ws();
            if self.c(0) != '\n' {
                self.i = save;
                break;
            }
            let mut j = self.i + 1;
            let mut blanks = 0;
            loop {
                let mut k = j;
                while is_ws(self.at(k)) {
                    k += 1;
                }
                if k < self.s.len() && self.at(k) == '\n' {
                    blanks += 1;
                    j = k + 1;
                    continue;
                }
                break;
            }
            let mut k = j;
            while is_ws(self.at(k)) {
                k += 1;
            }
            let c = self.at(k);
            if k >= self.s.len() || c == '#' || is_flow_indicator(c) || (c == ':' && {
                let d = self.at(k + 1);
                is_ws(d) || d == '\n' || is_flow_indicator(d) || k + 1 >= self.s.len()
            }) {
                self.i = save;
                break;
            }
            if self.marker_at(j, "---") || self.marker_at(j, "...") {
                self.i = j;
                return self.err(tr!("フローのコレクションの中に文書の区切りがあります", "a document marker inside a flow collection"));
            }
            self.i = j;
            self.flow_line_indent(n)?;
            self.i = k;
            let more = self.plain_line(true);
            if blanks == 0 {
                text.push(' ');
            } else {
                text.extend(std::iter::repeat_n('\n', blanks));
            }
            text.push_str(&more);
        }
        Ok(text)
    }

    // ---- JSON

    fn json_ws(&mut self) {
        while matches!(self.c(0), ' ' | '\t' | '\n') {
            self.i += 1;
        }
    }

    fn json_text(&mut self) -> Result<Node, Error> {
        self.json_ws();
        let v = self.json_value()?;
        self.json_ws();
        if !self.eof() {
            return self.err(tr!("JSON の値のあとに余分なものがあります", "extra text after the JSON value"));
        }
        Ok(v)
    }

    fn json_value(&mut self) -> Result<Node, Error> {
        let at = self.i;
        match self.c(0) {
            '{' => {
                self.deeper()?;
                self.i += 1;
                let mut entries: Vec<(Key, Node)> = Vec::new();
                self.json_ws();
                if self.c(0) == '}' {
                    self.i += 1;
                } else {
                    loop {
                        self.json_ws();
                        let kat = self.i;
                        if self.c(0) != '"' {
                            return self.err(tr!("ここには `\"` で始まるキーが要ります", "a key in `\"` goes here"));
                        }
                        let name = self.json_string()?;
                        self.json_ws();
                        if self.c(0) != ':' {
                            return self.err(tr!("キーのあとに `:` がありません", "no `:` after the key"));
                        }
                        self.i += 1;
                        self.json_ws();
                        let v = self.json_value()?;
                        let (line, col) = self.pos_of(kat);
                        if entries.iter().any(|(k, _)| k.name == name) {
                            return Err(Error { line, col, message: tr!("キー `{name}` が二度あります", "the key `{name}` is written twice") });
                        }
                        entries.push((Key { name, line, col }, v));
                        self.json_ws();
                        match self.c(0) {
                            ',' => self.i += 1,
                            '}' => {
                                self.i += 1;
                                break;
                            }
                            _ => return self.err(tr!("ここには `,` か `}}` が要ります", "a `,` or a `}}` goes here")),
                        }
                    }
                }
                self.depth -= 1;
                Ok(self.node(Value::Map(entries), at))
            }
            '[' => {
                self.deeper()?;
                self.i += 1;
                let mut items = Vec::new();
                self.json_ws();
                if self.c(0) == ']' {
                    self.i += 1;
                } else {
                    loop {
                        self.json_ws();
                        items.push(self.json_value()?);
                        self.json_ws();
                        match self.c(0) {
                            ',' => self.i += 1,
                            ']' => {
                                self.i += 1;
                                break;
                            }
                            _ => return self.err(tr!("ここには `,` か `]` が要ります", "a `,` or a `]` goes here")),
                        }
                    }
                }
                self.depth -= 1;
                Ok(self.node(Value::Seq(items), at))
            }
            '"' => {
                let s = self.json_string()?;
                Ok(self.node(Value::Str(s), at))
            }
            't' | 'f' | 'n' => {
                for (w, v) in [("true", Value::Bool(true)), ("false", Value::Bool(false)), ("null", Value::Null)] {
                    let wc: Vec<char> = w.chars().collect();
                    if self.s.get(self.i..self.i + wc.len()) == Some(&wc[..]) {
                        self.i += wc.len();
                        return Ok(self.node(v, at));
                    }
                }
                self.err(tr!("JSON の値として読めません", "this does not read as a JSON value"))
            }
            c if c == '-' || c.is_ascii_digit() => {
                let from = self.i;
                if self.c(0) == '-' {
                    self.i += 1;
                }
                let digits = |p: &mut P| {
                    let s = p.i;
                    while p.c(0).is_ascii_digit() {
                        p.i += 1;
                    }
                    p.i - s
                };
                let lead = self.c(0);
                let n = digits(self);
                if n == 0 || (lead == '0' && n > 1) {
                    return self.err_at(from, tr!("JSON の数として読めません", "this does not read as a JSON number"));
                }
                let mut whole = true;
                if self.c(0) == '.' {
                    self.i += 1;
                    whole = false;
                    if digits(self) == 0 {
                        return self.err_at(from, tr!("JSON の数として読めません", "this does not read as a JSON number"));
                    }
                }
                if matches!(self.c(0), 'e' | 'E') {
                    self.i += 1;
                    whole = false;
                    if matches!(self.c(0), '+' | '-') {
                        self.i += 1;
                    }
                    if digits(self) == 0 {
                        return self.err_at(from, tr!("JSON の数として読めません", "this does not read as a JSON number"));
                    }
                }
                let text: String = self.s[from..self.i].iter().collect();
                let v = match (whole, text.parse::<i128>()) {
                    (true, Ok(n)) => Value::Int(n),
                    _ => Value::Float(text),
                };
                Ok(self.node(v, at))
            }
            _ if self.eof() => self.err(tr!("値がありません", "there is no value")),
            c => self.err(tr!("JSON の値は `{c}` では始まりません", "a JSON value does not start with `{c}`")),
        }
    }

    fn json_string(&mut self) -> Result<String, Error> {
        let at = self.i;
        self.i += 1;
        let mut out = String::new();
        loop {
            let c = self.c(0);
            if self.eof() || c == '\n' {
                return self.err_at(at, tr!("閉じていない文字列があります", "a string is not closed"));
            }
            self.i += 1;
            match c {
                '"' => return Ok(out),
                '\\' => {
                    let e = self.c(0);
                    self.i += 1;
                    match e {
                        '"' => out.push('"'),
                        '\\' => out.push('\\'),
                        '/' => out.push('/'),
                        'b' => out.push('\u{8}'),
                        'f' => out.push('\u{C}'),
                        'n' => out.push('\n'),
                        'r' => out.push('\r'),
                        't' => out.push('\t'),
                        'u' => {
                            let hex = |p: &mut P| -> Option<u32> {
                                let h: String = p.s.get(p.i..p.i + 4)?.iter().collect();
                                let v = u32::from_str_radix(&h, 16).ok()?;
                                p.i += 4;
                                Some(v)
                            };
                            let Some(u) = hex(self) else {
                                return self.err(tr!("`\\u` のあとに 16 進の数字が 4 桁要ります", "`\\u` takes four hexadecimal digits"));
                            };
                            let code = if (0xD800..0xDC00).contains(&u) {
                                if self.c(0) == '\\' && self.c(1) == 'u' {
                                    self.i += 2;
                                    match hex(self) {
                                        Some(l) if (0xDC00..0xE000).contains(&l) => 0x10000 + ((u - 0xD800) << 10) + (l - 0xDC00),
                                        _ => return self.err(tr!("サロゲートの対が崩れています", "a broken surrogate pair")),
                                    }
                                } else {
                                    return self.err(tr!("サロゲートの対が崩れています", "a broken surrogate pair"));
                                }
                            } else {
                                u
                            };
                            match char::from_u32(code) {
                                Some(ch) => out.push(ch),
                                None => return self.err(tr!("サロゲートの対が崩れています", "a broken surrogate pair")),
                            }
                        }
                        _ => return self.err(tr!("文字列の中のエスケープ `\\{e}` は読めません", "the escape `\\{e}` in a string is not read")),
                    }
                }
                c if (c as u32) < 0x20 => return self.err_at(self.i - 1, tr!("文字列の中に制御文字があります", "a control character inside a string")),
                c => out.push(c),
            }
        }
    }
}

fn size(n: &Node) -> usize {
    1 + match &n.value {
        Value::Seq(xs) => xs.iter().map(size).sum(),
        Value::Map(es) => es.iter().map(|(_, v)| size(v)).sum(),
        _ => 0,
    }
}

fn no_document() -> Text {
    tr!("文書がありません", "there is no document")
}

fn explicit_key() -> Text {
    tr!("`?` で書くキーは読みません", "a key written with `?` is not read")
}

fn empty_key() -> Text {
    tr!("キーがありません", "a key is missing")
}

fn non_scalar_key() -> Text {
    tr!("キーがスカラーではありません。キーは文字列です", "a key that is no scalar; a key is a string")
}

fn alias_key() -> Text {
    tr!("エイリアスをキーにはできません", "an alias cannot be a key")
}

fn is_inf_or_nan(f: &str) -> bool {
    let f = f.trim_start_matches(['+', '-']);
    matches!(f, ".inf" | ".Inf" | ".INF" | ".nan" | ".NaN" | ".NAN")
}

/// A plain scalar by YAML 1.2's core schema: null, a bool, an int or a float; None for a string.
fn core(t: &str) -> Option<Value> {
    match t {
        "" | "~" | "null" | "Null" | "NULL" => return Some(Value::Null),
        "true" | "True" | "TRUE" => return Some(Value::Bool(true)),
        "false" | "False" | "FALSE" => return Some(Value::Bool(false)),
        _ => {}
    }
    int(t).or_else(|| float(t))
}

fn int(t: &str) -> Option<Value> {
    let big = |digits: &str, radix: u32, neg: bool| -> Option<Value> {
        if digits.is_empty() || !digits.chars().all(|c| c.is_digit(radix)) {
            return None;
        }
        match i128::from_str_radix(digits, radix) {
            Ok(n) => Some(Value::Int(if neg { -n } else { n })),
            Err(_) if radix == 10 => Some(Value::Float(format!("{}{digits}", if neg { "-" } else { "" }))),
            Err(_) => None,
        }
    };
    if let Some(h) = t.strip_prefix("0x") {
        return big(h, 16, false);
    }
    if let Some(o) = t.strip_prefix("0o") {
        return big(o, 8, false);
    }
    let (neg, rest) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    big(rest, 10, neg)
}

fn float(t: &str) -> Option<Value> {
    if is_inf_or_nan(t) {
        return Some(Value::Float(t.to_string()));
    }
    let b = t.as_bytes();
    let mut i = 0;
    if matches!(b.first(), Some(b'+' | b'-')) {
        i += 1;
    }
    let int_start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let int_digits = i - int_start;
    let mut frac_digits = 0;
    if i < b.len() && b[i] == b'.' {
        i += 1;
        let s = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        frac_digits = i - s;
    }
    if int_digits == 0 && frac_digits == 0 {
        return None;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        let s = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == s {
            return None;
        }
    }
    (i == b.len()).then(|| Value::Float(t.to_string()))
}
