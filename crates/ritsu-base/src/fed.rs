//! `fed by <naming>` at the end of a line that declares a value (DESIGN 7.16): where the words stand
//! in a line, the naming they hold, what a naming can name to be a source of values, and the text of
//! a file with the words taken off. Each language that takes the form reads it here, so that no
//! language reads a naming again in words of its own.
//!
//! The form says where hand-written code takes the value from, for `ritsu check` to ask the other
//! side what values can come. It changes no answer and nothing generated, so what a file's meaning
//! is taken from (a generated head's hash, an item's definition) is the file without it ([`without`]).

use crate::naming::{self, Error, ErrorKind, Name, Tool};
use crate::paths::{self, PathError};
use crate::text::Text;
use crate::tr;
use std::borrow::Cow;

/// Where `fed by` stands in a line, in bytes of the line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cut {
    /// Where `fed` starts.
    pub at: usize,
    /// Where the naming starts: the first character after `by` and the blanks. The length of the
    /// line's text when there is nothing after `by`.
    pub naming_at: usize,
    /// Where the naming ends: at the comment or the end of the line, without the blanks before it.
    pub end: usize,
}

impl Cut {
    /// The naming's text, in the line it was cut from.
    pub fn text<'a>(&self, line: &'a str) -> &'a str {
        &line[self.naming_at..self.end]
    }
}

/// Where a line says `fed by`: the two words outside a string and before the comment, with blanks
/// before them and between them, and a blank or the end of the line after them. A name of a
/// language can be `fed`, so what makes the form is the two words together; no language writes two
/// names in a row where a name goes.
pub fn cut(line: &str) -> Option<Cut> {
    let (mut in_str, mut escaped, mut blank_before) = (false, false, false);
    let mut chars = line.char_indices();
    while let Some((i, c)) = chars.next() {
        if in_str {
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_str = false,
                _ => {}
            }
            continue;
        }
        match c {
            '#' => return None,
            '"' => in_str = true,
            'f' if blank_before => {
                if let Some(c) = words_at(line, i) {
                    return Some(c);
                }
            }
            _ => {}
        }
        blank_before = c == ' ' || c == '\t';
    }
    None
}

/// The form, when `fed by` starts at the byte `at`.
fn words_at(line: &str, at: usize) -> Option<Cut> {
    let blanks = |s: &str| s.len() - s.trim_start_matches([' ', '\t']).len();
    let rest = line[at..].strip_prefix("fed")?;
    let gap = blanks(rest);
    if gap == 0 {
        return None;
    }
    let after = rest[gap..].strip_prefix("by")?;
    if after.chars().next().is_some_and(|c| c != ' ' && c != '\t' && c != '#') {
        return None;
    }
    let naming_at = line.len() - after.len() + blanks(after);
    let end = naming_end(line, naming_at);
    Some(Cut { at, naming_at, end })
}

/// Where the naming that starts at `from` ends: at a `#` outside a string, or at the end of the
/// line, the blanks before either left out.
fn naming_end(line: &str, from: usize) -> usize {
    let mut in_str = false;
    let mut escaped = false;
    let mut stop = line.len();
    for (i, c) in line[from..].char_indices() {
        if in_str {
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_str = false,
                _ => {}
            }
        } else if c == '"' {
            in_str = true;
        } else if c == '#' {
            stop = from + i;
            break;
        }
    }
    from + line[from..stop].trim_end_matches([' ', '\t']).len()
}

/// The text of a file with every `fed by` taken off, the blanks before it too, and with whatever
/// comment the line has kept: the same bytes a file has that never had them, when the form is the
/// last thing on its line. What a generated file's head hashes, so that nothing generated moves.
pub fn without(src: &str) -> Cow<'_, str> {
    if !src.contains("fed") {
        return Cow::Borrowed(src);
    }
    let mut out = String::with_capacity(src.len());
    let mut any = false;
    for line in src.split_inclusive('\n') {
        let (body, newline) = match line.strip_suffix('\n') {
            Some(b) => (b, "\n"),
            None => (line, ""),
        };
        match cut(body) {
            Some(c) => {
                any = true;
                out.push_str(body[..c.at].trim_end_matches([' ', '\t']));
                let comment = &body[c.end..];
                if !comment.trim().is_empty() {
                    out.push(' ');
                    out.push_str(comment.trim_start());
                }
                out.push_str(newline);
            }
            None => out.push_str(line),
        }
    }
    if any { Cow::Owned(out) } else { Cow::Borrowed(src) }
}

/// The SHA-256 of the file's text with every `fed by` taken off: what a file's generated code and
/// what a language says of its meaning names it by.
pub fn hash(src: &str) -> String {
    crate::sha256::hex(without(src).as_bytes())
}

/// What is wrong with what `fed by` holds.
#[derive(Clone, Debug, PartialEq)]
pub enum Problem {
    /// It does not read as a naming.
    Naming(Error),
    /// It reads, and what it names holds no value: a term, a claim, a table.
    NoValue(Name),
}

impl Problem {
    /// What is wrong, in the words the languages share.
    pub fn text(&self) -> Text {
        match self {
            Problem::Naming(e) => e.text(),
            Problem::NoValue(n) => {
                let t = n.text();
                tr!("`{t}` は値を持つものではありません。{}", "`{t}` holds no value. {}", sources_ja(n.tool); sources_en(n.tool))
            }
        }
    }

    /// The character of the text it is at, from 0.
    pub fn at(&self) -> usize {
        match self {
            Problem::Naming(e) => e.at,
            Problem::NoValue(_) => 0,
        }
    }
}

/// The shapes of a naming that holds a value, for a tool (DESIGN 7.16): what `fed by` can name. A
/// place that takes a value is one too: what the code gave it is what the code gives the next.
pub fn shapes(tool: Tool) -> &'static [&'static [&'static str]] {
    match tool {
        Tool::Rulec => &[&["output"], &["input"]],
        Tool::Chobo => &[&["account"]],
        Tool::Koyomi => &[&["date"], &["input"]],
        Tool::Proto => &[&["message", "field"]],
        Tool::Openapi => &[&["schema", "property"], &["pointer"]],
        Tool::Jsonschema => &[&["def"], &["def", "property"], &["pointer"]],
        Tool::Dandori => &[&["output"], &["input"], &["record", "field"]],
        Tool::Kaname => &[&["record", "field"]],
        Tool::Sekisho => &[&["principal", "attribute"], &["resource", "attribute"], &["action", "input"]],
        Tool::Geas | Tool::Asyncapi | Tool::Cedar | Tool::File | Tool::Yuen | Tool::Sakai => &[],
    }
}

/// Every shape as it is written, for what a language says of a naming that holds no value.
fn listed(tool: Tool) -> Vec<String> {
    shapes(tool).iter().map(|s| format!("`{} \"…\" {}`", tool.word(), s.iter().map(|k| format!("{k} …")).collect::<Vec<_>>().join(" "))).collect()
}

fn sources_ja(tool: Tool) -> String {
    let own = listed(tool);
    if own.is_empty() {
        format!("{} の中に、指せるものはありません。指せるのは規則の出力、帳簿の勘定、日付、契約のフィールド、ワークフローの出力とレコードのフィールド、kaname のレコードのフィールド、それと先に値を受け取った場所（規則・日付・ワークフローの入力、ゲートの属性と入力）です。", tool.word())
    } else {
        format!("{} で指せるのは {} です。", tool.word(), own.join("、"))
    }
}

fn sources_en(tool: Tool) -> String {
    let own = listed(tool);
    if own.is_empty() {
        format!("Nothing of {} can be named: what can is a rule's output, a book's account, a date, a contract's field, a workflow's output, a field of a workflow's record or of a record of kaname's, or a place that took the value first (an input of a rule, a dates file or a workflow, an attribute or an input of a gate).", tool.word())
    } else {
        format!("Of {}, what can be named is {}.", tool.word(), own.join(", "))
    }
}

/// Whether a naming holds a value that can come from it.
pub fn holds_a_value(n: &Name) -> bool {
    let kinds: Vec<&str> = n.items.iter().map(|(k, _)| k.as_str()).collect();
    shapes(n.tool).iter().any(|s| *s == kinds.as_slice())
}

/// Read what `fed by` holds, in a file wherever it is: the naming with its path as written, from
/// the directory of the file. What a language can say of it alone is said here (the form, and that
/// it holds a value); that the file and the thing are there is what `ritsu check` asks.
pub fn read(text: &str) -> Result<Name, Problem> {
    let ws = naming::words(text).map_err(Problem::Naming)?;
    let end = text.chars().count();
    let Some(mut w) = naming::written(ws, end) else {
        return Err(Problem::Naming(Error { kind: ErrorKind::Missing, at: 0 }));
    };
    // The path is as written until a language joins it to its file: `../x.rule` leaves no root here,
    // and `@pricing/x.rule` is a dependency's. The rest is checked as any naming is.
    let written = match &mut w.path {
        Some(p) if p.quoted => {
            match paths::join("", &p.text) {
                Err(PathError::Empty) => return Err(Problem::Naming(Error { kind: ErrorKind::EmptyPath, at: p.at })),
                Err(PathError::Absolute) => return Err(Problem::Naming(Error { kind: ErrorKind::AbsolutePath(p.text.clone()), at: p.at })),
                _ => {}
            }
            Some(std::mem::replace(&mut p.text, "x".to_string()))
        }
        _ => None,
    };
    let (mut name, _) = naming::resolve(&w, "", false).map_err(Problem::Naming)?;
    if let Some(p) = written {
        name.path = p;
    }
    if !holds_a_value(&name) {
        return Err(Problem::NoValue(name));
    }
    Ok(name)
}

/// The naming with its path from the root of the project: the path written from the directory of
/// `file`, or from the root of the module of ritsu's it is in for `@<name>/…` (DESIGN 17.6).
pub fn from_root(root: &std::path::Path, file: &str, name: &Name) -> Result<Name, Text> {
    let path = crate::modules::join(root, file, &name.path).map_err(|e| e.text())?;
    Ok(Name { path, ..name.clone() })
}

/// What a page says of a place with `fed by` (DESIGN 7.16), the same in every language: where the
/// value comes from, that `ritsu check` holds what can come from there to what the place takes, and
/// that nothing holds the hand-written code to carrying it.
pub fn page_note(written: &str) -> Text {
    tr!(
        "出どころ: `{written}`（値の範囲は `ritsu check` が確かめます。値を運ぶ手書きのコードは確かめません）",
        "Comes from `{written}` (`ritsu check` holds the range of its values; the hand-written code that carries the value is not checked)"
    )
}
