//! A reader of `.proto` files (DESIGN 4.2, PLAN B.5), written here as rulec and dandori write
//! theirs: the syntax, the package, the imports with their lines, the messages (nested ones by
//! their dotted names), their fields, the enums and their values with their numbers and lines,
//! and the services and their methods with the options written on them (dandori's
//! `(dandori.v1.workflow)` among them). `extend`, `reserved`, `extensions` and the other options
//! are passed over; a proto2 `group` is refused (E106), since its fields are a message of a kind
//! the rest of the suite does not read.
//!
//! The type names a field or a method writes are resolved by protobuf's rule, over the file and
//! the files it imports (`import public` passing on what it imports): from the innermost scope
//! outward, one level of the package at a time, and a name that starts with `.` as it is.

use crate::i18n::Text;
use crate::paths;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProtoFile {
    /// From the root.
    pub path: String,
    /// `proto2`, `proto3`, or `edition 2023`.
    pub syntax: String,
    pub package: String,
    pub imports: Vec<Import>,
    /// Every message, nested ones too (`Order.Line`), in the order they start.
    pub messages: Vec<Message>,
    /// Every enum, nested ones too (`Order.Status`), in the order they start.
    pub enums: Vec<Enum>,
    pub services: Vec<Service>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Import {
    /// As written.
    pub path: String,
    pub line: usize,
    pub col: usize,
    pub public: bool,
    pub weak: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    /// From the package: `Order`, `Order.Line`.
    pub name: String,
    pub line: usize,
    pub fields: Vec<Field>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Label {
    None,
    Optional,
    Required,
    Repeated,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Type {
    /// `string`, `int32`, …
    Scalar(String),
    /// A message or an enum, as written (`warehouse.v1.ReserveResponse`, `Line`).
    Named(String),
    /// `map<K, V>`: the key is a scalar.
    Map(String, Box<Type>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub name: String,
    pub number: i64,
    pub label: Label,
    pub ty: Type,
    pub json_name: Option<String>,
    pub oneof: Option<String>,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Enum {
    pub name: String,
    pub line: usize,
    pub values: Vec<EnumValue>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnumValue {
    pub name: String,
    pub number: i64,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Service {
    pub name: String,
    pub line: usize,
    pub methods: Vec<Method>,
    /// `(name, value)`, as written: `("(dandori.v1.workflow)", "{name: \"引当と発送\", version: 1}")`.
    pub options: Vec<(String, String)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Method {
    pub name: String,
    pub input: String,
    pub output: String,
    pub client_streaming: bool,
    pub server_streaming: bool,
    pub line: usize,
    pub options: Vec<(String, String)>,
}

impl ProtoFile {
    pub fn message(&self, name: &str) -> Option<&Message> {
        self.messages.iter().find(|m| m.name == name)
    }

    pub fn enumeration(&self, name: &str) -> Option<&Enum> {
        self.enums.iter().find(|e| e.name == name)
    }

    pub fn service(&self, name: &str) -> Option<&Service> {
        self.services.iter().find(|s| s.name == name)
    }

    /// `warehouse.v1.Stock` for `Stock`.
    pub fn full(&self, name: &str) -> String {
        if self.package.is_empty() { name.to_string() } else { format!("{}.{name}", self.package) }
    }
}

/// Why a file cannot be read (E106), and where.
#[derive(Clone, Debug, PartialEq)]
pub struct ReadError {
    pub line: usize,
    pub col: usize,
    pub message: Text,
}

/// Google's well-known types, `buf/validate` and dandori's options: imported by many a `.proto`,
/// distributed elsewhere, known without their files and not read (DESIGN 1.3, 4.2).
pub fn is_known(import: &str) -> bool {
    import.starts_with("google/protobuf/") || import.starts_with("buf/validate/") || import == "dandori/v1/options.proto"
}

/// The packages of the known files: a type in one of them is known though not read.
fn known_package(full: &str) -> bool {
    full.starts_with("google.protobuf.") || full.starts_with("buf.validate.") || full.starts_with("dandori.v1.")
}

pub const SCALARS: &[&str] = &["double", "float", "int32", "int64", "uint32", "uint64", "sint32", "sint64", "fixed32", "fixed64", "sfixed32", "sfixed64", "bool", "string", "bytes"];

/// Where an import is looked for (DESIGN 4.2), as paths from the root, in order: under each
/// `proto root`; then, as dandori looks, from the root of the module when the importing file
/// sits where its package says (`shop/v1/order.proto` for `package shop.v1`), and last from the
/// importing file's own directory.
pub fn import_candidates(importer: &str, package: &str, roots: &[String], import: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut add = |p: Result<String, paths::PathError>| {
        if let Ok(p) = p
            && !out.contains(&p)
        {
            out.push(p);
        }
    };
    for r in roots {
        add(paths::join(r, import));
    }
    let dir = paths::parent(importer);
    if !package.is_empty() {
        let at = package.replace('.', "/");
        if dir == at {
            add(paths::join(".", import));
        } else if let Some(root) = dir.strip_suffix(&format!("/{at}")) {
            add(paths::join(root, import));
        }
    }
    add(paths::join(&dir, import));
    out
}

// ── The tokens ──────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
enum T {
    Word(String),
    Str(String),
    Num(String),
    Sym(char),
}

#[derive(Clone, Debug)]
struct Tk {
    t: T,
    line: usize,
    col: usize,
    /// The character offsets the token spans in the file.
    start: usize,
    end: usize,
}

fn tokens(src: &str) -> Result<Vec<Tk>, ReadError> {
    let c: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let (mut i, mut line, mut line_start) = (0usize, 1usize, 0usize);
    while i < c.len() {
        let ch = c[i];
        let col = i - line_start + 1;
        if ch == '\n' {
            line += 1;
            i += 1;
            line_start = i;
        } else if ch.is_whitespace() {
            i += 1;
        } else if ch == '/' && c.get(i + 1) == Some(&'/') {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
        } else if ch == '/' && c.get(i + 1) == Some(&'*') {
            let (sl, sc) = (line, col);
            i += 2;
            loop {
                if i + 1 >= c.len() {
                    return Err(ReadError { line: sl, col: sc, message: tr!("閉じていないコメントがあります", "a comment is not closed") });
                }
                if c[i] == '*' && c[i + 1] == '/' {
                    i += 2;
                    break;
                }
                if c[i] == '\n' {
                    line += 1;
                    line_start = i + 1;
                }
                i += 1;
            }
        } else if ch == '"' || ch == '\'' {
            let q = ch;
            let start = i;
            let mut s = String::new();
            i += 1;
            loop {
                match c.get(i) {
                    None | Some('\n') => return Err(ReadError { line, col, message: tr!("閉じていない文字列があります", "a string is not closed") }),
                    Some(x) if *x == q => {
                        i += 1;
                        break;
                    }
                    Some('\\') => {
                        match c.get(i + 1) {
                            Some('n') => s.push('\n'),
                            Some('t') => s.push('\t'),
                            Some(x) => s.push(*x),
                            None => {}
                        }
                        i += 2;
                    }
                    Some(x) => {
                        s.push(*x);
                        i += 1;
                    }
                }
            }
            out.push(Tk { t: T::Str(s), line, col, start, end: i });
        } else if ch.is_ascii_alphabetic() || ch == '_' || (ch == '.' && c.get(i + 1).is_some_and(|d| d.is_ascii_alphabetic() || *d == '_')) {
            let start = i;
            i += 1;
            while i < c.len() && (c[i].is_ascii_alphanumeric() || c[i] == '_' || (c[i] == '.' && c.get(i + 1).is_some_and(|d| d.is_ascii_alphabetic() || *d == '_'))) {
                i += 1;
            }
            out.push(Tk { t: T::Word(c[start..i].iter().collect()), line, col, start, end: i });
        } else if ch.is_ascii_digit() || ((ch == '-' || ch == '+' || ch == '.') && c.get(i + 1).is_some_and(|d| d.is_ascii_digit())) {
            let start = i;
            i += 1;
            while i < c.len() && (c[i].is_ascii_alphanumeric() || c[i] == '.' || ((c[i] == '+' || c[i] == '-') && matches!(c[i - 1], 'e' | 'E'))) {
                i += 1;
            }
            out.push(Tk { t: T::Num(c[start..i].iter().collect()), line, col, start, end: i });
        } else {
            out.push(Tk { t: T::Sym(ch), line, col, start: i, end: i + 1 });
            i += 1;
        }
    }
    Ok(out)
}

fn number(s: &str) -> Option<i64> {
    let (neg, body) = match s.strip_prefix('-') {
        Some(b) => (true, b),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    let v = if let Some(h) = body.strip_prefix("0x").or_else(|| body.strip_prefix("0X")) {
        i64::from_str_radix(h, 16).ok()?
    } else if body.len() > 1 && body.starts_with('0') {
        i64::from_str_radix(&body[1..], 8).ok()?
    } else {
        body.parse().ok()?
    };
    Some(if neg { -v } else { v })
}

struct P<'a> {
    t: Vec<Tk>,
    i: usize,
    src: &'a [char],
}

impl P<'_> {
    fn peek(&self) -> Option<&T> {
        self.t.get(self.i).map(|x| &x.t)
    }

    fn here(&self) -> (usize, usize) {
        self.t.get(self.i).or(self.t.last()).map(|x| (x.line, x.col)).unwrap_or((1, 1))
    }

    fn fail<X>(&self, msg: Text) -> Result<X, ReadError> {
        let (line, col) = self.here();
        Err(ReadError { line, col, message: msg })
    }

    fn expected<X>(&self, what: &str) -> Result<X, ReadError> {
        let got = match self.peek() {
            Some(T::Word(w)) | Some(T::Num(w)) => format!("`{w}`"),
            Some(T::Str(s)) => format!("\"{s}\""),
            Some(T::Sym(c)) => format!("`{c}`"),
            None => "the end of the file".to_string(),
        };
        let got_ja = if self.peek().is_none() { "ファイルの終わり".to_string() } else { got.clone() };
        self.fail(tr!("{what} が要るところに {} があります", "{what} is expected where {} is", got_ja; got))
    }

    fn word(&mut self) -> Result<String, ReadError> {
        match self.peek().cloned() {
            Some(T::Word(w)) => {
                self.i += 1;
                Ok(w)
            }
            _ => self.expected("a name"),
        }
    }

    fn is_word(&self, w: &str) -> bool {
        matches!(self.peek(), Some(T::Word(x)) if x == w)
    }

    fn is_sym(&self, s: char) -> bool {
        matches!(self.peek(), Some(T::Sym(x)) if *x == s)
    }

    fn sym(&mut self, s: char) -> Result<(), ReadError> {
        if self.is_sym(s) {
            self.i += 1;
            Ok(())
        } else {
            self.expected(&format!("`{s}`"))
        }
    }

    fn line(&self) -> usize {
        self.here().0
    }

    /// Pass over a statement to its `;`, or over a `{…}` block.
    fn skip_statement(&mut self) -> Result<(), ReadError> {
        let mut depth = 0i32;
        while let Some(t) = self.peek().cloned() {
            self.i += 1;
            match t {
                T::Sym('{') | T::Sym('[') | T::Sym('(') => depth += 1,
                T::Sym('}') | T::Sym(']') | T::Sym(')') => {
                    depth -= 1;
                    if depth <= 0 && t == T::Sym('}') {
                        return Ok(());
                    }
                }
                T::Sym(';') if depth == 0 => return Ok(()),
                _ => {}
            }
        }
        if depth > 0 { self.fail(tr!("`}}` が足りません", "a `}}` is missing")) } else { Ok(()) }
    }

    /// The source text from token `a` to the token before `b`.
    fn text(&self, a: usize, b: usize) -> String {
        if a >= b {
            return String::new();
        }
        let (s, e) = (self.t[a].start, self.t[b - 1].end);
        self.src[s..e].iter().collect()
    }

    /// `option <name> = <value>;`: the name and the value as written.
    fn option(&mut self) -> Result<(String, String), ReadError> {
        let a = self.i;
        while !self.is_sym('=') {
            if self.peek().is_none() || self.is_sym(';') {
                return self.expected("`=`");
            }
            self.i += 1;
        }
        let name = self.text(a, self.i);
        self.i += 1;
        let b = self.i;
        let mut depth = 0i32;
        loop {
            match self.peek() {
                None => return self.expected("`;`"),
                Some(T::Sym('{')) | Some(T::Sym('[')) => depth += 1,
                Some(T::Sym('}')) | Some(T::Sym(']')) => depth -= 1,
                Some(T::Sym(';')) if depth == 0 => break,
                _ => {}
            }
            self.i += 1;
        }
        let value = self.text(b, self.i);
        self.i += 1;
        Ok((name, value))
    }

    /// `[a = 1, (b).c = {…}]` after a field: `json_name`, if it is there.
    fn field_options(&mut self) -> Result<Option<String>, ReadError> {
        if !self.is_sym('[') {
            return Ok(None);
        }
        self.i += 1;
        let mut json = None;
        loop {
            if self.is_word("json_name") && matches!(self.t.get(self.i + 1).map(|x| &x.t), Some(T::Sym('='))) {
                self.i += 2;
                match self.peek().cloned() {
                    Some(T::Str(s)) => {
                        json = Some(s);
                        self.i += 1;
                    }
                    _ => return self.expected("a string"),
                }
            } else {
                // Pass over one option: to the `,` or `]` at its own depth.
                let mut depth = 0i32;
                loop {
                    match self.peek() {
                        None => return self.expected("`]`"),
                        Some(T::Sym('{')) | Some(T::Sym('[')) | Some(T::Sym('(')) => depth += 1,
                        Some(T::Sym('}')) | Some(T::Sym(')')) => depth -= 1,
                        Some(T::Sym(']')) if depth > 0 => depth -= 1,
                        Some(T::Sym(']')) | Some(T::Sym(',')) if depth == 0 => break,
                        _ => {}
                    }
                    self.i += 1;
                }
            }
            if self.is_sym(',') {
                self.i += 1;
                continue;
            }
            self.sym(']')?;
            return Ok(json);
        }
    }
}

/// Read one `.proto` (DESIGN 4.2). `path` is its path from the root.
pub fn read(path: &str, src: &str) -> Result<ProtoFile, ReadError> {
    let chars: Vec<char> = src.chars().collect();
    let mut p = P { t: tokens(src)?, i: 0, src: &chars };
    let mut f = ProtoFile { path: path.to_string(), syntax: "proto2".to_string(), ..ProtoFile::default() };
    while let Some(t) = p.peek().cloned() {
        match t {
            T::Word(w) if w == "syntax" => {
                p.i += 1;
                p.sym('=')?;
                match p.peek().cloned() {
                    Some(T::Str(s)) if s == "proto2" || s == "proto3" => {
                        f.syntax = s;
                        p.i += 1;
                    }
                    Some(T::Str(s)) => return p.fail(tr!("syntax \"{s}\" は知りません", "syntax \"{s}\" is not one sakai knows")),
                    _ => return p.expected("a string"),
                }
                p.sym(';')?;
            }
            T::Word(w) if w == "edition" => {
                // An edition reads as proto3 does for what sakai takes from it.
                p.i += 1;
                p.sym('=')?;
                match p.peek().cloned() {
                    Some(T::Str(s)) => {
                        f.syntax = format!("edition {s}");
                        p.i += 1;
                    }
                    _ => return p.expected("a string"),
                }
                p.sym(';')?;
            }
            T::Word(w) if w == "package" => {
                p.i += 1;
                f.package = p.word()?;
                p.sym(';')?;
            }
            T::Word(w) if w == "import" => {
                let (line, col) = p.here();
                p.i += 1;
                let (mut public, mut weak) = (false, false);
                if p.is_word("public") {
                    public = true;
                    p.i += 1;
                } else if p.is_word("weak") {
                    weak = true;
                    p.i += 1;
                }
                match p.peek().cloned() {
                    Some(T::Str(s)) => {
                        f.imports.push(Import { path: s, line, col, public, weak });
                        p.i += 1;
                    }
                    _ => return p.expected("the file imported"),
                }
                p.sym(';')?;
            }
            T::Word(w) if w == "option" => {
                p.i += 1;
                p.option()?;
            }
            T::Word(w) if w == "message" => {
                p.i += 1;
                message(&mut p, "", &mut f)?;
            }
            T::Word(w) if w == "enum" => {
                p.i += 1;
                enumeration(&mut p, "", &mut f)?;
            }
            T::Word(w) if w == "service" => {
                p.i += 1;
                service(&mut p, &mut f)?;
            }
            T::Word(w) if w == "extend" => p.skip_statement()?,
            T::Sym(';') => p.i += 1,
            _ => return p.expected("`message`, `enum`, `service`, `import` or `option`"),
        }
    }
    Ok(f)
}

fn message(p: &mut P, outer: &str, f: &mut ProtoFile) -> Result<(), ReadError> {
    let line = p.line();
    let short = p.word()?;
    let name = if outer.is_empty() { short } else { format!("{outer}.{short}") };
    p.sym('{')?;
    let at = f.messages.len();
    f.messages.push(Message { name: name.clone(), line, fields: vec![] });
    let mut fields = Vec::new();
    let mut oneof: Option<String> = None;
    loop {
        match p.peek().cloned() {
            None => return p.expected("`}`"),
            Some(T::Sym('}')) => {
                p.i += 1;
                if oneof.take().is_some() {
                    continue;
                }
                break;
            }
            Some(T::Sym(';')) => p.i += 1,
            Some(T::Word(w)) if w == "message" && oneof.is_none() => {
                p.i += 1;
                message(p, &name, f)?;
            }
            Some(T::Word(w)) if w == "enum" && oneof.is_none() => {
                p.i += 1;
                enumeration(p, &name, f)?;
            }
            Some(T::Word(w)) if w == "oneof" && oneof.is_none() => {
                p.i += 1;
                oneof = Some(p.word()?);
                p.sym('{')?;
            }
            Some(T::Word(w)) if w == "option" => {
                p.i += 1;
                p.option()?;
            }
            Some(T::Word(w)) if ["reserved", "extensions", "extend"].contains(&w.as_str()) => p.skip_statement()?,
            Some(T::Word(w)) => {
                let fline = p.line();
                p.i += 1;
                let (label, ty) = match w.as_str() {
                    "repeated" => (Label::Repeated, p.word()?),
                    "optional" => (Label::Optional, p.word()?),
                    "required" => (Label::Required, p.word()?),
                    _ => (Label::None, w),
                };
                if ty == "group" {
                    p.i -= 1;
                    return p.fail(tr!(
                        "proto2 の `group` は読みません。フィールドの型を、別に宣言したメッセージにします",
                        "a proto2 `group` is not read; give the field a message declared on its own"
                    ));
                }
                let ty = if ty == "map" && p.is_sym('<') {
                    p.i += 1;
                    let k = p.word()?;
                    p.sym(',')?;
                    let v = p.word()?;
                    p.sym('>')?;
                    Type::Map(k, Box::new(if SCALARS.contains(&v.as_str()) { Type::Scalar(v) } else { Type::Named(v) }))
                } else if SCALARS.contains(&ty.as_str()) {
                    Type::Scalar(ty)
                } else {
                    Type::Named(ty)
                };
                let fname = p.word()?;
                p.sym('=')?;
                let number = match p.peek().cloned() {
                    Some(T::Num(n)) => {
                        p.i += 1;
                        number(&n).ok_or(()).or_else(|_| p.expected("the field's number"))?
                    }
                    _ => return p.expected("the field's number"),
                };
                let json_name = p.field_options()?;
                p.sym(';')?;
                fields.push(Field { name: fname, number, label, ty, json_name, oneof: oneof.clone(), line: fline });
            }
            Some(_) => return p.expected("a field"),
        }
    }
    f.messages[at].fields = fields;
    Ok(())
}

fn enumeration(p: &mut P, outer: &str, f: &mut ProtoFile) -> Result<(), ReadError> {
    let line = p.line();
    let short = p.word()?;
    let name = if outer.is_empty() { short } else { format!("{outer}.{short}") };
    p.sym('{')?;
    let mut values = Vec::new();
    loop {
        match p.peek().cloned() {
            None => return p.expected("`}`"),
            Some(T::Sym('}')) => {
                p.i += 1;
                break;
            }
            Some(T::Sym(';')) => p.i += 1,
            Some(T::Word(w)) if w == "option" => {
                p.i += 1;
                p.option()?;
            }
            Some(T::Word(w)) if w == "reserved" => p.skip_statement()?,
            Some(T::Word(w)) => {
                let vline = p.line();
                p.i += 1;
                p.sym('=')?;
                let n = match p.peek().cloned() {
                    Some(T::Num(n)) => {
                        p.i += 1;
                        number(&n).ok_or(()).or_else(|_| p.expected("the value's number"))?
                    }
                    _ => return p.expected("the value's number"),
                };
                p.field_options()?;
                p.sym(';')?;
                values.push(EnumValue { name: w, number: n, line: vline });
            }
            Some(_) => return p.expected("a value"),
        }
    }
    f.enums.push(Enum { name, line, values });
    Ok(())
}

fn service(p: &mut P, f: &mut ProtoFile) -> Result<(), ReadError> {
    let line = p.line();
    let name = p.word()?;
    p.sym('{')?;
    let mut s = Service { name, line, methods: vec![], options: vec![] };
    loop {
        match p.peek().cloned() {
            None => return p.expected("`}`"),
            Some(T::Sym('}')) => {
                p.i += 1;
                break;
            }
            Some(T::Sym(';')) => p.i += 1,
            Some(T::Word(w)) if w == "option" => {
                p.i += 1;
                s.options.push(p.option()?);
            }
            Some(T::Word(w)) if w == "rpc" => {
                let mline = p.line();
                p.i += 1;
                let m = p.word()?;
                p.sym('(')?;
                let cs = p.is_word("stream");
                if cs {
                    p.i += 1;
                }
                let input = p.word()?;
                p.sym(')')?;
                if !p.is_word("returns") {
                    return p.expected("`returns`");
                }
                p.i += 1;
                p.sym('(')?;
                let ss = p.is_word("stream");
                if ss {
                    p.i += 1;
                }
                let output = p.word()?;
                p.sym(')')?;
                let mut options = Vec::new();
                if p.is_sym('{') {
                    p.i += 1;
                    loop {
                        if p.is_sym('}') {
                            p.i += 1;
                            break;
                        }
                        if p.is_sym(';') {
                            p.i += 1;
                            continue;
                        }
                        if p.is_word("option") {
                            p.i += 1;
                            options.push(p.option()?);
                        } else {
                            return p.expected("`option` or `}`");
                        }
                    }
                } else {
                    p.sym(';')?;
                }
                s.methods.push(Method { name: m, input, output, client_streaming: cs, server_streaming: ss, line: mline, options });
            }
            Some(_) => return p.expected("`rpc` or `option`"),
        }
    }
    f.services.push(s);
    Ok(())
}

// ── Names across files ───────────────────────────────────────────────────

/// A message or an enum: its full name, the file that declares it, and its name from the
/// package. Two files may declare the same full name (the two copies of a shared kernel); a
/// name is resolved to the one the file that names it can see.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Symbol {
    pub full: String,
    pub file: String,
    pub name: String,
    pub is_enum: bool,
}

impl Symbol {
    /// `proto "<file>" enum <name>` or `… message <name>`.
    pub fn naming(&self) -> crate::naming::Name {
        crate::naming::Name::file(crate::naming::Tool::Proto, self.file.clone()).with(if self.is_enum { "enum" } else { "message" }, self.name.clone())
    }
}

/// What a type name comes to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resolved {
    /// A message or an enum of a file read.
    Found(Symbol),
    /// A type of a file known without being read (`google.protobuf.Value`).
    Known(String),
    /// Not found, and an import of the file was not on the disk: it may be in that file.
    Unknown,
    /// Not found anywhere it could be.
    Missing,
}

/// The files read, what each import came to, and every message and enum by its full name.
#[derive(Clone, Debug, Default)]
pub struct Protos {
    pub files: BTreeMap<String, ProtoFile>,
    /// For each file, what each of its imports came to: a path from the root, or None (known, or
    /// not found).
    pub imports: BTreeMap<String, Vec<Option<String>>>,
    /// The files with an import that was not found (W102).
    pub unread: BTreeSet<String>,
    pub symbols: BTreeMap<String, Vec<Symbol>>,
}

impl Protos {
    pub fn add(&mut self, f: ProtoFile) {
        for m in &f.messages {
            let full = f.full(&m.name);
            self.symbols.entry(full.clone()).or_default().push(Symbol { full, file: f.path.clone(), name: m.name.clone(), is_enum: false });
        }
        for e in &f.enums {
            let full = f.full(&e.name);
            self.symbols.entry(full.clone()).or_default().push(Symbol { full, file: f.path.clone(), name: e.name.clone(), is_enum: true });
        }
        self.files.insert(f.path.clone(), f);
    }

    /// The files whose types `file` can name: itself, what it imports, and what those pass on
    /// with `import public`, through every level.
    pub fn visible(&self, file: &str) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        out.insert(file.to_string());
        for p in self.imports.get(file).into_iter().flatten().flatten() {
            self.with_public(p, &mut out);
        }
        out
    }

    fn with_public(&self, file: &str, out: &mut BTreeSet<String>) {
        if !out.insert(file.to_string()) {
            return;
        }
        let Some(f) = self.files.get(file) else { return };
        for (imp, at) in f.imports.iter().zip(self.imports.get(file).into_iter().flatten()) {
            if imp.public
                && let Some(p) = at
            {
                self.with_public(p, out);
            }
        }
    }

    /// A type name written in `scope` (the full name of the message it is in, or the package) of
    /// `file`.
    pub fn resolve(&self, file: &str, scope: &str, written: &str) -> Resolved {
        let vis = self.visible(file);
        let found = |full: &str| self.symbols.get(full).and_then(|ss| ss.iter().find(|s| vis.contains(&s.file))).cloned();
        if let Some(abs) = written.strip_prefix('.') {
            if let Some(f) = found(abs) {
                return Resolved::Found(f);
            }
            if known_package(abs) {
                return Resolved::Known(abs.to_string());
            }
        } else {
            let mut s = scope.to_string();
            loop {
                let full = if s.is_empty() { written.to_string() } else { format!("{s}.{written}") };
                if let Some(f) = found(&full) {
                    return Resolved::Found(f);
                }
                if s.is_empty() {
                    break;
                }
                s = s.rsplit_once('.').map(|(a, _)| a.to_string()).unwrap_or_default();
            }
            if known_package(written) {
                return Resolved::Known(written.to_string());
            }
        }
        if self.unread.contains(file) { Resolved::Unknown } else { Resolved::Missing }
    }

    /// Every type a field of a message names, resolved: (field, the full name).
    pub fn field_types(&self, file: &str, msg: &Message) -> Vec<(String, Resolved)> {
        let f = &self.files[file];
        let scope = f.full(&msg.name);
        let mut out = Vec::new();
        for fl in &msg.fields {
            let named = match &fl.ty {
                Type::Named(n) => Some(n),
                Type::Map(_, v) => match v.as_ref() {
                    Type::Named(n) => Some(n),
                    _ => None,
                },
                Type::Scalar(_) => None,
            };
            if let Some(n) = named {
                out.push((fl.name.clone(), self.resolve(file, &scope, n)));
            }
        }
        out
    }

    /// Every message and enum `start` reaches through the types of fields, `start` among them,
    /// in the order they are reached (DESIGN 3.3).
    pub fn reach(&self, start: &[Symbol]) -> Vec<Symbol> {
        let mut out: Vec<Symbol> = Vec::new();
        let mut todo: Vec<Symbol> = start.to_vec();
        todo.reverse();
        while let Some(sym) = todo.pop() {
            if out.contains(&sym) {
                continue;
            }
            out.push(sym.clone());
            if sym.is_enum {
                continue;
            }
            let Some(m) = self.files.get(&sym.file).and_then(|f| f.message(&sym.name)) else { continue };
            let mut next: Vec<Symbol> = Vec::new();
            for (_, r) in self.field_types(&sym.file, m) {
                if let Resolved::Found(n) = r
                    && !out.contains(&n)
                {
                    next.push(n);
                }
            }
            next.reverse();
            todo.extend(next);
        }
        out
    }
}

/// What reading the `.proto` files of a map came across (DESIGN 4.2).
#[derive(Clone, Debug, PartialEq)]
pub enum Issue {
    /// The file cannot be read (E106).
    Unreadable { file: String, err: ReadError },
    /// An import is on the disk in none of the places looked in (W102).
    NotFound { file: String, import: Import, tried: Vec<String> },
    /// An import is a file outside the scope of the map (E103).
    OutOfScope { file: String, import: Import, at: String },
}

/// Read the `.proto` files `files` (paths from the root, the artifacts of the scope) and what
/// each import comes to, looking under `roots` (the map's `proto root`s) first (DESIGN 4.2). A
/// known file (`is_known`) is not read; an import that is not found, or is outside `files`, is
/// told and its types are not known.
pub fn load(root: &std::path::Path, files: &[String], roots: &[String]) -> (Protos, Vec<Issue>) {
    let mut ps = Protos::default();
    let mut issues = Vec::new();
    let mut unreadable = BTreeSet::new();
    for f in files {
        let src = std::fs::read_to_string(paths::on_disk(root, f)).unwrap_or_default();
        match read(f, &src) {
            Ok(pf) => ps.add(pf),
            Err(err) => {
                unreadable.insert(f.clone());
                issues.push(Issue::Unreadable { file: f.clone(), err });
            }
        }
    }
    let in_scope: BTreeSet<&String> = files.iter().collect();
    let read_files: Vec<String> = ps.files.keys().cloned().collect();
    for f in read_files {
        let pf = ps.files[&f].clone();
        let mut at = Vec::new();
        for imp in &pf.imports {
            if is_known(&imp.path) {
                at.push(None);
                continue;
            }
            let tried = import_candidates(&f, &pf.package, roots, &imp.path);
            match tried.iter().find(|c| paths::on_disk(root, c).is_file()) {
                Some(c) if in_scope.contains(c) && !unreadable.contains(c) => at.push(Some(c.clone())),
                Some(c) if in_scope.contains(c) => {
                    ps.unread.insert(f.clone());
                    at.push(None);
                }
                Some(c) => {
                    ps.unread.insert(f.clone());
                    issues.push(Issue::OutOfScope { file: f.clone(), import: imp.clone(), at: c.clone() });
                    at.push(None);
                }
                None => {
                    ps.unread.insert(f.clone());
                    issues.push(Issue::NotFound { file: f.clone(), import: imp.clone(), tried });
                    at.push(None);
                }
            }
        }
        ps.imports.insert(f, at);
    }
    (ps, issues)
}

/// The prefix buf asks the values of an enum to start with: the enum's name in upper snake case
/// and `_` (`PackingStatus` → `PACKING_STATUS_`, `HTTPMethod` → `HTTP_METHOD_`). A capital starts
/// a word when the letter before it is lower case, or when the letter after it is.
pub fn value_prefix(enum_name: &str) -> String {
    let short = enum_name.rsplit('.').next().unwrap_or(enum_name);
    let cs: Vec<char> = short.chars().collect();
    let mut out = String::new();
    for (i, c) in cs.iter().enumerate() {
        if i > 0 && c.is_ascii_uppercase() {
            let before = cs[i - 1];
            let after = cs.get(i + 1).copied();
            if before.is_ascii_lowercase() || before.is_ascii_digit() || (before.is_ascii_uppercase() && after.is_some_and(|a| a.is_ascii_lowercase())) {
                out.push('_');
            }
        }
        out.push(c.to_ascii_uppercase());
    }
    out.push('_');
    out
}

/// Whether the value numbered 0 of an enum says no value is set (DESIGN 1.7): its name, with the
/// enum's prefix taken off, is `unspecified` (in any case), as with rulec's `import proto` and
/// dandori's types from a `.proto`. Any other value 0 (`HANDLING_STANDARD = 0`) is a value like
/// the rest.
pub fn is_unset(e: &Enum, v: &EnumValue) -> bool {
    if v.number != 0 {
        return false;
    }
    let rest = v.name.strip_prefix(&value_prefix(&e.name)).unwrap_or(&v.name);
    rest.eq_ignore_ascii_case("unspecified")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(src: &str) -> ProtoFile {
        read("t.proto", src).unwrap()
    }

    #[test]
    fn nested_messages_fields_and_options_are_read() {
        let f = one(
            "syntax = \"proto3\";\npackage shop.v1;\nimport public \"a/b.proto\";\nmessage Order {\n  message Line { string sku = 1 [json_name = \"品番\"]; }\n  repeated Line lines = 1;\n  map<string, Line> by_sku = 2;\n  oneof pay { string card = 3; string bank = 4; }\n  enum Status { STATUS_UNSPECIFIED = 0; STATUS_OPEN = 1; }\n  reserved 9 to 11;\n}\nservice S {\n  option (dandori.v1.workflow) = {name: \"引当\", version: 1};\n  rpc Do(stream Order) returns (Order.Line) { option idempotency_level = NO_SIDE_EFFECTS; }\n}\n",
        );
        assert_eq!(f.package, "shop.v1");
        assert!(f.imports[0].public);
        assert_eq!(f.imports[0].line, 3);
        assert_eq!(f.messages.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), vec!["Order", "Order.Line"]);
        let o = f.message("Order").unwrap();
        assert_eq!(o.fields.len(), 4);
        assert_eq!(o.fields[2].oneof.as_deref(), Some("pay"));
        assert_eq!(f.message("Order.Line").unwrap().fields[0].json_name.as_deref(), Some("品番"));
        assert_eq!(f.enums[0].name, "Order.Status");
        assert_eq!(f.services[0].options[0], ("(dandori.v1.workflow)".to_string(), "{name: \"引当\", version: 1}".to_string()));
        assert!(f.services[0].methods[0].client_streaming);
    }

    #[test]
    fn a_group_is_refused() {
        let e = read("t.proto", "syntax = \"proto2\";\nmessage M {\n  optional group G = 1 { optional int32 a = 2; }\n}\n").unwrap_err();
        assert_eq!(e.line, 3);
    }

    #[test]
    fn the_value_that_says_nothing_is_set() {
        let e = |name: &str, vals: &[(&str, i64)]| Enum { name: name.into(), line: 1, values: vals.iter().map(|(n, x)| EnumValue { name: n.to_string(), number: *x, line: 1 }).collect() };
        let os = e("OrderStatus", &[("ORDER_STATUS_UNSPECIFIED", 0)]);
        assert!(is_unset(&os, &os.values[0]));
        let st = e("Stock", &[("STOCK_UNSPECIFIED", 0)]);
        assert!(is_unset(&st, &st.values[0]));
        let dd = e("Stock", &[("unspecified", 0)]);
        assert!(is_unset(&dd, &dd.values[0]));
        let h = e("Handling", &[("HANDLING_STANDARD", 0)]);
        assert!(!is_unset(&h, &h.values[0]));
        assert_eq!(value_prefix("HTTPMethod"), "HTTP_METHOD_");
        assert_eq!(value_prefix("PackingStatus"), "PACKING_STATUS_");
    }

    #[test]
    fn names_resolve_from_the_innermost_scope() {
        let mut ps = Protos::default();
        let a = read("a/v1/a.proto", "syntax = \"proto3\";\npackage a.v1;\nmessage Outer { message In {} In x = 1; }\nmessage Top { Outer.In y = 1; .a.v1.Outer z = 2; b.v1.B w = 3; }\n").unwrap();
        let b = read("b/v1/b.proto", "syntax = \"proto3\";\npackage b.v1;\nmessage B {}\n").unwrap();
        ps.add(a);
        ps.add(b);
        ps.imports.insert("a/v1/a.proto".into(), vec![Some("b/v1/b.proto".into())]);
        let full = |r: Resolved| match r {
            Resolved::Found(s) => s.full,
            other => format!("{other:?}"),
        };
        assert_eq!(full(ps.resolve("a/v1/a.proto", "a.v1.Outer", "In")), "a.v1.Outer.In");
        assert_eq!(full(ps.resolve("a/v1/a.proto", "a.v1.Top", "Outer.In")), "a.v1.Outer.In");
        assert_eq!(full(ps.resolve("a/v1/a.proto", "a.v1.Top", ".a.v1.Outer")), "a.v1.Outer");
        assert_eq!(full(ps.resolve("a/v1/a.proto", "a.v1.Top", "b.v1.B")), "b.v1.B");
        assert_eq!(ps.resolve("a/v1/a.proto", "a.v1.Top", "c.v1.C"), Resolved::Missing);
        assert_eq!(ps.resolve("a/v1/a.proto", "a.v1.Top", "google.protobuf.Value"), Resolved::Known("google.protobuf.Value".into()));
        let Resolved::Found(top) = ps.resolve("a/v1/a.proto", "a.v1", "Top") else { panic!() };
        let reached: Vec<String> = ps.reach(&[top]).into_iter().map(|s| s.full).collect();
        assert_eq!(reached, vec!["a.v1.Top", "a.v1.Outer.In", "a.v1.Outer", "b.v1.B"]);
    }

    #[test]
    fn where_an_import_is_looked_for() {
        assert_eq!(
            import_candidates("proto/shop/v1/order.proto", "shop.v1", &["proto".into()], "warehouse/v1/stock.proto"),
            vec!["proto/warehouse/v1/stock.proto", "proto/shop/v1/warehouse/v1/stock.proto"]
        );
        assert_eq!(import_candidates("specs/f.proto", "shop.ja.v1", &[], "warehouse.proto"), vec!["specs/warehouse.proto"]);
    }
}
