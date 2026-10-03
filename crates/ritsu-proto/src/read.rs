//! Reading one `.proto`: the tokens, then the statements. `extend`, `reserved` and `extensions`
//! are passed over; a proto2 `group` is refused, since its fields are a message of a kind the
//! languages do not read.

use crate::model::*;
use crate::validate::{self, MsgRules, OneofRule};
use crate::value::Value;
use ritsu_base::text::Text;
use ritsu_base::tr;

/// Why a file cannot be read, and where.
#[derive(Clone, Debug, PartialEq)]
pub struct ReadError {
    pub line: usize,
    pub col: usize,
    pub what: Problem,
}

/// What stopped the reader.
#[derive(Clone, Debug, PartialEq)]
pub enum Problem {
    UnclosedComment,
    UnclosedString,
    /// `what` was expected (`a name`, `` `;` ``) where `got` is: a token as it reads in the
    /// message (`` `}` ``, `"a"`), or None at the end of the file.
    Expected { what: String, got: Option<String> },
    MissingBrace,
    /// A `syntax` neither `proto2` nor `proto3`.
    UnknownSyntax(String),
    /// A proto2 `group`.
    Group,
}

impl ReadError {
    /// What a person reads; `tool` is the program that read the file (`sakai`), named where the
    /// reader says it does not know something.
    pub fn message(&self, tool: &str) -> Text {
        match &self.what {
            Problem::UnclosedComment => tr!("閉じていないコメントがあります", "a comment is not closed"),
            Problem::UnclosedString => tr!("閉じていない文字列があります", "a string is not closed"),
            Problem::Expected { what, got } => {
                let (ja, en) = match got {
                    Some(g) => (g.clone(), g.clone()),
                    None => ("ファイルの終わり".to_string(), "the end of the file".to_string()),
                };
                tr!("{what} が要るところに {} があります", "{what} is expected where {} is", ja; en)
            }
            Problem::MissingBrace => tr!("`}}` が足りません", "a `}}` is missing"),
            Problem::UnknownSyntax(s) => tr!("syntax \"{s}\" は知りません", "syntax \"{s}\" is not one {tool} knows"),
            Problem::Group => tr!(
                "proto2 の `group` は読みません。フィールドの型を、別に宣言したメッセージにします",
                "a proto2 `group` is not read; give the field a message declared on its own"
            ),
        }
    }
}

pub const SCALARS: &[&str] = &["double", "float", "int32", "int64", "uint32", "uint64", "sint32", "sint64", "fixed32", "fixed64", "sfixed32", "sfixed64", "bool", "string", "bytes"];

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
                    return Err(ReadError { line: sl, col: sc, what: Problem::UnclosedComment });
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
                    None | Some('\n') => return Err(ReadError { line, col, what: Problem::UnclosedString }),
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

/// A field's or a value's number: decimal, `0x…`, or octal with a leading `0`.
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

    fn fail<X>(&self, what: Problem) -> Result<X, ReadError> {
        let (line, col) = self.here();
        Err(ReadError { line, col, what })
    }

    fn expected<X>(&self, what: &str) -> Result<X, ReadError> {
        let got = match self.peek() {
            Some(T::Word(w)) | Some(T::Num(w)) => Some(format!("`{w}`")),
            Some(T::Str(s)) => Some(format!("\"{s}\"")),
            Some(T::Sym(c)) => Some(format!("`{c}`")),
            None => None,
        };
        self.fail(Problem::Expected { what: what.to_string(), got })
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
        if depth > 0 { self.fail(Problem::MissingBrace) } else { Ok(()) }
    }

    /// The source text from token `a` to the token before `b`.
    fn text(&self, a: usize, b: usize) -> String {
        if a >= b {
            return String::new();
        }
        let (s, e) = (self.t[a].start, self.t[b - 1].end);
        self.src[s..e].iter().collect()
    }

    /// `option <name> = <value>;`, after `option`: the name and the value as written, and the
    /// value read.
    fn option(&mut self) -> Result<Opt, ReadError> {
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
        let opt = Opt { name, text: self.text(b, self.i), value: value_of(&self.t[b..self.i]) };
        self.i += 1;
        Ok(opt)
    }

    /// `[a = 1, (b).c = {…}, json_name = "x"]` after a field or a value: every option, in the
    /// order written. `json_name` takes a string.
    fn field_options(&mut self) -> Result<Vec<Opt>, ReadError> {
        let mut out = Vec::new();
        if !self.is_sym('[') {
            return Ok(out);
        }
        self.i += 1;
        loop {
            if self.is_word("json_name") && matches!(self.t.get(self.i + 1).map(|x| &x.t), Some(T::Sym('='))) {
                let a = self.i;
                self.i += 2;
                match self.peek().cloned() {
                    Some(T::Str(s)) => {
                        out.push(Opt { name: self.text(a, a + 1), text: self.text(self.i, self.i + 1), value: Some(Value::Str(s)) });
                        self.i += 1;
                    }
                    _ => return self.expected("a string"),
                }
            } else {
                // One option: to the `,` or `]` at its own depth, its name before its first `=`.
                let a = self.i;
                let mut eq = None;
                let mut depth = 0i32;
                loop {
                    match self.peek() {
                        None => return self.expected("`]`"),
                        Some(T::Sym('{')) | Some(T::Sym('[')) | Some(T::Sym('(')) => depth += 1,
                        Some(T::Sym('}')) | Some(T::Sym(')')) => depth -= 1,
                        Some(T::Sym(']')) if depth > 0 => depth -= 1,
                        Some(T::Sym(']')) | Some(T::Sym(',')) if depth == 0 => break,
                        Some(T::Sym('=')) if depth == 0 && eq.is_none() => eq = Some(self.i),
                        _ => {}
                    }
                    self.i += 1;
                }
                if let Some(e) = eq {
                    out.push(Opt { name: self.text(a, e), text: self.text(e + 1, self.i), value: value_of(&self.t[e + 1..self.i]) });
                }
            }
            if self.is_sym(',') {
                self.i += 1;
                continue;
            }
            self.sym(']')?;
            return Ok(out);
        }
    }
}

/// The tokens of an option's value read as protobuf's text format, when all of them are one
/// value.
fn value_of(t: &[Tk]) -> Option<Value> {
    let mut i = 0;
    let v = value_at(t, &mut i)?;
    (i == t.len()).then_some(v)
}

fn value_at(t: &[Tk], i: &mut usize) -> Option<Value> {
    match &t.get(*i)?.t {
        T::Sym(open @ ('{' | '<')) => {
            let close = if *open == '{' { '}' } else { '>' };
            *i += 1;
            let mut kv = Vec::new();
            loop {
                match &t.get(*i)?.t {
                    T::Sym(c) if *c == close => {
                        *i += 1;
                        return Some(Value::Msg(kv));
                    }
                    T::Sym(',' | ';') => *i += 1,
                    T::Word(w) => {
                        let k = w.clone();
                        *i += 1;
                        if matches!(t.get(*i).map(|x| &x.t), Some(T::Sym(':'))) {
                            *i += 1;
                        }
                        kv.push((k, value_at(t, i)?));
                    }
                    T::Sym('[') => {
                        // An extension, or the type of an `Any`: `[type.example.com/a.B]`.
                        let a = *i;
                        while !matches!(t.get(*i)?.t, T::Sym(']')) {
                            *i += 1;
                        }
                        *i += 1;
                        let inner: String = t[a + 1..*i - 1].iter().map(tok_text).collect::<Vec<_>>().join("");
                        if matches!(t.get(*i).map(|x| &x.t), Some(T::Sym(':'))) {
                            *i += 1;
                        }
                        kv.push((format!("[{inner}]"), value_at(t, i)?));
                    }
                    _ => return None,
                }
            }
        }
        T::Sym('[') => {
            *i += 1;
            let mut vs = Vec::new();
            loop {
                match &t.get(*i)?.t {
                    T::Sym(']') => {
                        *i += 1;
                        return Some(Value::List(vs));
                    }
                    T::Sym(',') => *i += 1,
                    _ => vs.push(value_at(t, i)?),
                }
            }
        }
        T::Str(s) => {
            // Strings written side by side are one.
            let mut text = s.clone();
            *i += 1;
            while let Some(T::Str(more)) = t.get(*i).map(|x| &x.t) {
                text.push_str(more);
                *i += 1;
            }
            Some(Value::Str(text))
        }
        T::Num(n) => {
            *i += 1;
            Some(Value::Num(n.clone()))
        }
        T::Word(w) => {
            *i += 1;
            Some(Value::Id(w.clone()))
        }
        _ => None,
    }
}

fn tok_text(t: &Tk) -> String {
    match &t.t {
        T::Word(w) | T::Num(w) => w.clone(),
        T::Str(s) => format!("\"{s}\""),
        T::Sym(c) => c.to_string(),
    }
}

/// Read one `.proto`. `path` is its path from the root.
pub fn read(path: &str, src: &str) -> Result<ProtoFile, ReadError> {
    let chars: Vec<char> = src.chars().collect();
    let mut p = P { t: tokens(src)?, i: 0, src: &chars };
    let mut f = ProtoFile { path: path.to_string(), syntax: "proto2".to_string(), ..ProtoFile::default() };
    while let Some(t) = p.peek().cloned() {
        match t {
            T::Word(w) if w == "syntax" => {
                f.syntax_line = Some(p.line());
                p.i += 1;
                p.sym('=')?;
                match p.peek().cloned() {
                    Some(T::Str(s)) if s == "proto2" || s == "proto3" => {
                        f.syntax = s;
                        p.i += 1;
                    }
                    Some(T::Str(s)) => return p.fail(Problem::UnknownSyntax(s)),
                    _ => return p.expected("a string"),
                }
                p.sym(';')?;
            }
            T::Word(w) if w == "edition" => {
                // An edition reads as proto3 does for what the languages take from it.
                f.syntax_line = Some(p.line());
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
                let o = p.option()?;
                f.options.push(o);
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
    f.messages.push(Message { name: name.clone(), line, fields: vec![], options: vec![], oneofs: vec![], rules: MsgRules::default() });
    let mut fields: Vec<Field> = Vec::new();
    let mut options = Vec::new();
    let mut oneofs: Vec<Oneof> = Vec::new();
    let mut rules = MsgRules::default();
    // The `oneof` the reader is in, and what Protovalidate asks of it so far.
    let mut oneof: Option<(String, OneofRule)> = None;
    loop {
        match p.peek().cloned() {
            None => return p.expected("`}`"),
            Some(T::Sym('}')) => {
                p.i += 1;
                if let Some((_, rule)) = oneof.take() {
                    rules.oneofs.push(rule);
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
                let oline = p.line();
                p.i += 1;
                let oname = p.word()?;
                p.sym('{')?;
                oneofs.push(Oneof { name: oname.clone(), line: oline, fields: vec![], options: vec![] });
                oneof = Some((oname, OneofRule::default()));
            }
            Some(T::Word(w)) if w == "option" => {
                p.i += 1;
                let o = p.option()?;
                match oneof.as_mut() {
                    Some((_, rule)) => {
                        validate::oneof_option(&o, rule);
                        oneofs.last_mut().expect("the oneof the reader is in").options.push(o);
                    }
                    None => {
                        validate::message_option(&o, &mut rules);
                        options.push(o);
                    }
                }
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
                    return p.fail(Problem::Group);
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
                let opts = p.field_options()?;
                p.sym(';')?;
                let json_name = opts.iter().rev().find(|o| o.name == "json_name").and_then(|o| if let Some(Value::Str(s)) = &o.value { Some(s.clone()) } else { None });
                let member = oneof.as_ref().map(|(n, _)| n.clone());
                if let Some((_, rule)) = oneof.as_mut() {
                    rule.fields.push(fname.clone());
                    oneofs.last_mut().expect("the oneof the reader is in").fields.push(fname.clone());
                }
                let rules = validate::field_rules(&opts);
                fields.push(Field { name: fname, number, label, ty, json_name, oneof: member, line: fline, options: opts, rules });
            }
            Some(_) => return p.expected("a field"),
        }
    }
    if rules.disabled {
        validate::disable(&mut fields, &mut rules);
    }
    let m = &mut f.messages[at];
    m.fields = fields;
    m.options = options;
    m.oneofs = oneofs;
    m.rules = rules;
    Ok(())
}

fn enumeration(p: &mut P, outer: &str, f: &mut ProtoFile) -> Result<(), ReadError> {
    let line = p.line();
    let short = p.word()?;
    let name = if outer.is_empty() { short } else { format!("{outer}.{short}") };
    p.sym('{')?;
    let mut values = Vec::new();
    let mut options = Vec::new();
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
                options.push(p.option()?);
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
                let opts = p.field_options()?;
                p.sym(';')?;
                values.push(EnumValue { name: w, number: n, line: vline, options: opts });
            }
            Some(_) => return p.expected("a value"),
        }
    }
    f.enums.push(Enum { name, line, values, options });
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
