//! A reader of `.proto` files (proto3): the package, the messages and enums with their fields,
//! and the services' methods, which a `connect` task is held to. Of the options it reads what
//! dandori uses: a field's `json_name` and its Protovalidate rules (`buf.validate.field`), and the
//! options of services and methods (dandori's own marks, `dandori.v1.workflow` and the rest);
//! the others are passed over. Google's well-known types, `buf/validate/validate.proto` and
//! dandori's `dandori/v1/options.proto` are known without their files; any other import is read
//! from the root of the importing file's module when the file sits where its package says (buf's
//! layout), else from the file's directory. An import that is on the disk in neither place is
//! passed over and told in `ProtoFile::unread` (a `.proto` imports `google/api/annotations.proto`
//! for options, and the types a flow uses are often not in it): what such a file holds is not
//! known, and a type of it that a message or a method names stays a name no message or enum has.

use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// dandori's options, the file `proto/dandori/v1/options.proto` of this repository. It is known
/// as Google's well-known types are, so that a `.proto` can import it and the flow's checker still
/// needs no file; protoc and buf need the file, and a user copies it to their proto root.
pub const DANDORI_OPTIONS: &str = include_str!("../proto/dandori/v1/options.proto");

/// How a `.proto` imports dandori's options.
pub const OPTIONS_IMPORT: &str = "dandori/v1/options.proto";

#[derive(Clone, Debug, PartialEq)]
pub enum PType {
    /// `string`, `int32`, …
    Scalar(String),
    /// a message or an enum, by its full name (`warehouse.v1.Stock`)
    Named(String),
    Map(Box<PType>, Box<PType>),
}

#[derive(Clone, Debug)]
pub struct PField {
    pub name: String,
    /// the key in JSON: `json_name`, or the name in lowerCamelCase
    pub json: String,
    pub ty: PType,
    pub repeated: bool,
    /// the field says whether it is set (`optional`, a message, a member of a `oneof`): absent in
    /// JSON when it is not. Any other field is left out of JSON at its zero value.
    pub presence: bool,
    /// what `(buf.validate.field)` says of the field, as the tree its options build:
    /// `{"int32": {"gte": 1}, "required": true}`; Null when it says nothing
    pub rules: Value,
}

#[derive(Clone, Debug)]
pub struct Method {
    pub name: String,
    pub input: String,
    pub output: String,
    pub streams: bool,
    /// the options written in its `{ … }`, by the name of the option or of the extension that
    /// sets it (`dandori.v1.start`, `idempotency_level`): each a tree of what was written
    /// (`{"fails": ["A", "B"]}`). A repeated field written one value at a time comes out as a
    /// list, and written once as that value: read it with `strings_of`.
    pub options: BTreeMap<String, Value>,
}

#[derive(Clone, Debug)]
pub struct Service {
    /// the full name, as a Connect path has it: `warehouse.v1.StockService`
    pub name: String,
    pub methods: Vec<Method>,
    /// the options of the service, as a method has them
    pub options: BTreeMap<String, Value>,
    /// whether the file that wrote the service imports `dandori/v1/options.proto`, which protoc
    /// and buf want of a file that uses dandori's options
    pub imports_options: bool,
}

/// The strings of an option's value that is a string or a list of them: `fails: "A"` and
/// `fails: ["A", "B"]` alike.
pub fn strings_of(v: &Value) -> Vec<String> {
    match v {
        Value::String(s) => vec![s.clone()],
        Value::Array(a) => a.iter().filter_map(|x| x.as_str().map(String::from)).collect(),
        _ => vec![],
    }
}

#[derive(Clone, Debug, Default)]
pub struct ProtoFile {
    pub package: String,
    pub messages: BTreeMap<String, Vec<PField>>,
    /// the values by name, in order; the first is the zero value
    pub enums: BTreeMap<String, Vec<String>>,
    pub services: Vec<Service>,
    /// the imports that were not on the disk, as `import` wrote them, in the order they were met:
    /// they were passed over, and the types in them are not known
    pub unread: Vec<String>,
}

impl ProtoFile {
    /// Whether a type name is one the files read have: a message, an enum, or a well-known type.
    /// A name that is not is a type of an import that was not read.
    pub fn knows(&self, name: &str) -> bool {
        self.messages.contains_key(name) || self.enums.contains_key(name) || WELL_KNOWN.contains(&name)
    }
}

/// Google's well-known types, which a `.proto` imports from `google/protobuf/…`.
pub const WELL_KNOWN: &[&str] = &[
    "google.protobuf.Timestamp",
    "google.protobuf.Duration",
    "google.protobuf.Struct",
    "google.protobuf.Value",
    "google.protobuf.ListValue",
    "google.protobuf.Any",
    "google.protobuf.Empty",
    "google.protobuf.FieldMask",
    "google.protobuf.DoubleValue",
    "google.protobuf.FloatValue",
    "google.protobuf.Int64Value",
    "google.protobuf.UInt64Value",
    "google.protobuf.Int32Value",
    "google.protobuf.UInt32Value",
    "google.protobuf.BoolValue",
    "google.protobuf.StringValue",
    "google.protobuf.BytesValue",
    "google.protobuf.NullValue",
];

const SCALARS: &[&str] = &["double", "float", "int32", "int64", "uint32", "uint64", "sint32", "sint64", "fixed32", "fixed64", "sfixed32", "sfixed64", "bool", "string", "bytes"];

/// The key protobuf's JSON gives a field: its name in lowerCamelCase.
pub fn json_name(name: &str) -> String {
    let mut out = String::new();
    let mut up = false;
    for c in name.chars() {
        if c == '_' {
            up = true;
        } else if up {
            out.extend(c.to_uppercase());
            up = false;
        } else {
            out.push(c);
        }
    }
    out
}

#[derive(Clone, Debug, PartialEq)]
enum T {
    Word(String),
    Str(String),
    Num(String),
    Sym(char),
}

fn tokens(src: &str) -> Result<Vec<(T, usize)>, String> {
    let c: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let (mut i, mut line) = (0, 1);
    while i < c.len() {
        let ch = c[i];
        if ch == '\n' {
            line += 1;
            i += 1;
        } else if ch.is_whitespace() {
            i += 1;
        } else if ch == '/' && c.get(i + 1) == Some(&'/') {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
        } else if ch == '/' && c.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < c.len() && !(c[i] == '*' && c[i + 1] == '/') {
                if c[i] == '\n' {
                    line += 1;
                }
                i += 1;
            }
            i += 2;
        } else if ch == '"' || ch == '\'' {
            let q = ch;
            let mut s = String::new();
            i += 1;
            while i < c.len() && c[i] != q {
                if c[i] == '\\' && i + 1 < c.len() {
                    i += 1;
                }
                s.push(c[i]);
                i += 1;
            }
            i += 1;
            out.push((T::Str(s), line));
        } else if ch.is_ascii_alphabetic() || ch == '_' || (ch == '.' && c.get(i + 1).is_some_and(|d| d.is_ascii_alphabetic())) {
            let start = i;
            i += 1;
            while i < c.len() && (c[i].is_ascii_alphanumeric() || c[i] == '_' || c[i] == '.') {
                i += 1;
            }
            out.push((T::Word(c[start..i].iter().collect()), line));
        } else if ch.is_ascii_digit() || ch == '-' || ch == '+' {
            let start = i;
            i += 1;
            while i < c.len() && (c[i].is_ascii_alphanumeric() || c[i] == '.' || c[i] == '+' || c[i] == '-') {
                i += 1;
            }
            out.push((T::Num(c[start..i].iter().collect()), line));
        } else {
            out.push((T::Sym(ch), line));
            i += 1;
        }
    }
    Ok(out)
}

struct P {
    t: Vec<(T, usize)>,
    i: usize,
}

impl P {
    fn peek(&self) -> Option<&T> {
        self.t.get(self.i).map(|x| &x.0)
    }
    fn line(&self) -> usize {
        self.t.get(self.i).or(self.t.last()).map(|x| x.1).unwrap_or(1)
    }
    fn err(&self, what: &str) -> String {
        format!("line {}: expected {what}", self.line())
    }
    fn word(&mut self) -> Result<String, String> {
        match self.peek().cloned() {
            Some(T::Word(w)) => {
                self.i += 1;
                Ok(w)
            }
            _ => Err(self.err("a name")),
        }
    }
    fn is_word(&self, w: &str) -> bool {
        matches!(self.peek(), Some(T::Word(x)) if x == w)
    }
    fn is_sym(&self, s: char) -> bool {
        matches!(self.peek(), Some(T::Sym(x)) if *x == s)
    }
    fn sym(&mut self, s: char) -> Result<(), String> {
        if self.is_sym(s) {
            self.i += 1;
            Ok(())
        } else {
            Err(self.err(&format!("`{s}`")))
        }
    }
    /// Pass over everything to the `;` that ends a statement, or over a `{…}` block.
    fn skip_statement(&mut self) {
        let mut depth = 0;
        while let Some(t) = self.peek().cloned() {
            self.i += 1;
            match t {
                T::Sym('{') => depth += 1,
                T::Sym('}') => {
                    depth -= 1;
                    if depth <= 0 {
                        return;
                    }
                }
                T::Sym(';') if depth == 0 => return,
                _ => {}
            }
        }
    }

    /// An option's name: `json_name`, or an extension's, `(buf.validate.field).int32.gte`: the name
    /// that sets it, and the path below it.
    fn option_name(&mut self) -> Result<(String, Vec<String>), String> {
        if !self.is_sym('(') {
            return Ok((self.word()?, vec![]));
        }
        self.i += 1;
        // `(.dandori.v1.start)` names the extension from the root; the name is the same
        let ext = self.word()?.trim_start_matches('.').to_string();
        self.sym(')')?;
        let mut path = Vec::new();
        // `.int32.gte` is one word, which begins with a dot
        if let Some(T::Word(w)) = self.peek().cloned() {
            if let Some(rest) = w.strip_prefix('.') {
                self.i += 1;
                path = rest.split('.').map(String::from).collect();
            }
        }
        Ok((ext, path))
    }

    /// An option's value, as protobuf's text format writes it: a scalar, a `{ … }` message, a `[ … ]` list.
    fn option_value(&mut self) -> Result<Value, String> {
        match self.peek().cloned() {
            Some(T::Sym(open @ ('{' | '<'))) => {
                self.i += 1;
                self.option_fields(if open == '{' { '}' } else { '>' })
            }
            Some(T::Sym('[')) => {
                self.i += 1;
                let mut items = Vec::new();
                loop {
                    if self.is_sym(']') {
                        self.i += 1;
                        return Ok(Value::Array(items));
                    }
                    if self.is_sym(',') {
                        self.i += 1;
                        continue;
                    }
                    if self.peek().is_none() {
                        return Err(self.err("`]`"));
                    }
                    items.push(self.option_value()?);
                }
            }
            // strings written side by side are one
            Some(T::Str(s)) => {
                self.i += 1;
                let mut text = s;
                while let Some(T::Str(more)) = self.peek().cloned() {
                    self.i += 1;
                    text.push_str(&more);
                }
                Ok(Value::String(text))
            }
            Some(T::Num(n)) => {
                self.i += 1;
                Ok(number(&n))
            }
            Some(T::Word(w)) => {
                self.i += 1;
                Ok(match w.as_str() {
                    "true" | "True" => Value::Bool(true),
                    "false" | "False" => Value::Bool(false),
                    // the name of an enum's value: `IGNORE_IF_ZERO_VALUE`, `NO_SIDE_EFFECTS`
                    _ => Value::String(w),
                })
            }
            _ => Err(self.err("a value")),
        }
    }

    /// The fields of a message value, up to `close`: `name: value`, `name { … }`, `name: [ … ]`,
    /// the fields apart by `,` or `;` or by nothing. A field written again becomes a list.
    fn option_fields(&mut self, close: char) -> Result<Value, String> {
        let mut out = Map::new();
        loop {
            match self.peek().cloned() {
                None => return Err(self.err(&format!("`{close}`"))),
                Some(T::Sym(c)) if c == close => {
                    self.i += 1;
                    return Ok(Value::Object(out));
                }
                Some(T::Sym(',' | ';')) => self.i += 1,
                Some(T::Word(name)) => {
                    self.i += 1;
                    self.option_field(&mut out, name)?;
                }
                // an extension, or the type of an `Any`, in brackets: `[type.example.com/a.B] { … }`
                Some(T::Sym('[')) => {
                    self.i += 1;
                    let mut name = String::new();
                    loop {
                        match self.peek().cloned() {
                            None => return Err(self.err("`]`")),
                            Some(T::Sym(']')) => {
                                self.i += 1;
                                break;
                            }
                            Some(T::Word(w)) => name.push_str(&w),
                            Some(T::Sym(c)) => name.push(c),
                            Some(_) => {}
                        }
                        self.i += 1;
                    }
                    self.option_field(&mut out, name)?;
                }
                _ => return Err(self.err("a field's name")),
            }
        }
    }

    fn option_field(&mut self, out: &mut Map<String, Value>, name: String) -> Result<(), String> {
        if self.is_sym(':') {
            self.i += 1;
        }
        let v = self.option_value()?;
        match out.get_mut(&name) {
            None => {
                out.insert(name, v);
            }
            Some(Value::Array(a)) => match v {
                Value::Array(more) => a.extend(more),
                one => a.push(one),
            },
            Some(old) => {
                let first = old.take();
                let mut a = vec![first];
                match v {
                    Value::Array(more) => a.extend(more),
                    one => a.push(one),
                }
                *old = Value::Array(a);
            }
        }
        Ok(())
    }

    /// `name = value`, the inside of `[ … ]` on a field and of an `option` statement.
    fn one_option(&mut self) -> Result<((String, Vec<String>), Value), String> {
        let name = self.option_name()?;
        self.sym('=')?;
        Ok((name, self.option_value()?))
    }

    /// Pass over an option this reader cannot read, to the `,` that ends it or the `]` that ends the list.
    fn skip_option(&mut self) {
        let mut depth = 0;
        while let Some(t) = self.peek().cloned() {
            match t {
                T::Sym('(' | '[' | '{') => depth += 1,
                T::Sym(')' | '}') => depth -= 1,
                T::Sym(']') if depth == 0 => return,
                T::Sym(']') => depth -= 1,
                T::Sym(',') if depth == 0 => return,
                _ => {}
            }
            self.i += 1;
        }
    }

    /// `[a = 1, (buf.validate.field).int32.gte = 1, json_name = "x"]`: the options, as a tree keyed
    /// by the name of the option or of its extension. An option it cannot read is passed over.
    fn options_list(&mut self) -> Result<Map<String, Value>, String> {
        let mut tree = Map::new();
        if !self.is_sym('[') {
            return Ok(tree);
        }
        self.i += 1;
        loop {
            if self.is_sym(']') {
                self.i += 1;
                return Ok(tree);
            }
            if self.is_sym(',') {
                self.i += 1;
                continue;
            }
            if self.peek().is_none() {
                return Err(self.err("`]`"));
            }
            let at = self.i;
            match self.one_option() {
                Ok((name, v)) => put_option(&mut tree, name, v),
                Err(_) => {
                    self.i = at;
                    self.skip_option();
                }
            }
        }
    }

    /// An `option … ;` statement, after the word `option`; one it cannot read is passed over.
    fn option_statement(&mut self, into: &mut Map<String, Value>) {
        let at = self.i;
        match self.one_option() {
            Ok((name, v)) if self.is_sym(';') => {
                self.i += 1;
                put_option(into, name, v);
            }
            _ => {
                self.i = at;
                self.skip_statement();
            }
        }
    }
}

/// A number as an option writes it: a whole number as one, a fraction as a double; anything else is text.
fn number(text: &str) -> Value {
    let t = text.strip_prefix('+').unwrap_or(text);
    if let Ok(n) = t.parse::<i64>() {
        return Value::from(n);
    }
    if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        if let Ok(n) = i64::from_str_radix(h, 16) {
            return Value::from(n);
        }
    }
    if let Ok(n) = t.parse::<u64>() {
        return Value::from(n);
    }
    match t.parse::<f64>().ok().and_then(serde_json::Number::from_f64) {
        Some(f) => Value::Number(f),
        None => Value::String(text.to_string()),
    }
}

/// Put `v` in the tree where an option's name and path say: `(a).b.c = v` is `{"a": {"b": {"c": v}}}`,
/// laid over what the other options of the same extension wrote.
fn put_option(tree: &mut Map<String, Value>, (name, path): (String, Vec<String>), v: Value) {
    fn put(slot: &mut Value, path: &[String], v: Value) {
        match path.split_first() {
            None => lay(slot, v),
            Some((k, rest)) => {
                if !slot.is_object() {
                    *slot = Value::Object(Map::new());
                }
                let child = slot.as_object_mut().expect("an object").entry(k.clone()).or_insert(Value::Null);
                put(child, rest, v);
            }
        }
    }
    fn lay(slot: &mut Value, v: Value) {
        match (slot.as_object_mut(), v) {
            (Some(a), Value::Object(b)) => {
                for (k, x) in b {
                    match a.get_mut(&k) {
                        Some(s) => lay(s, x),
                        None => {
                            a.insert(k, x);
                        }
                    }
                }
            }
            (_, v) => *slot = v,
        }
    }
    put(tree.entry(name).or_insert(Value::Null), &path, v);
}

/// A message or an enum as declared, before the names of its fields' types are resolved.
struct Raw {
    fields: Vec<(PField, String)>,
}

struct RawMethod {
    name: String,
    input: String,
    output: String,
    streams: bool,
    options: Map<String, Value>,
}

struct RawService {
    name: String,
    methods: Vec<RawMethod>,
    options: Map<String, Value>,
    imports_options: bool,
}

/// What the files read so far have: messages not yet resolved, services, the files seen.
#[derive(Default)]
struct Reading {
    raws: BTreeMap<String, Raw>,
    services: Vec<RawService>,
    seen: Vec<PathBuf>,
}

pub fn load(path: &Path) -> Result<ProtoFile, String> {
    let mut f = ProtoFile::default();
    let mut rd = Reading::default();
    read(path, &mut f, &mut rd, true)?;
    resolve_all(f, rd)
}

/// A `.proto` given as text, read as if its file were `name`, in the current directory: for what
/// is tried without a file. The imports other than the well-known ones are read from the disk.
pub fn load_text(name: &str, text: &str) -> Result<ProtoFile, String> {
    let mut f = ProtoFile::default();
    let mut rd = Reading::default();
    read_text(Path::new(name), text, &mut f, &mut rd, true)?;
    resolve_all(f, rd)
}

fn resolve_all(mut f: ProtoFile, rd: Reading) -> Result<ProtoFile, String> {
    let Reading { raws, services, .. } = rd;
    // resolve every type name from the scope it is written in, the innermost first
    let known: Vec<String> = raws.keys().cloned().chain(f.enums.keys().cloned()).chain(WELL_KNOWN.iter().map(|s| s.to_string())).collect();
    // with an import that was not read, a name nothing has may be in it: it stays as written, a
    // type that is not known, and is told where a flow comes to it
    let lenient = !f.unread.is_empty();
    let resolve = |scope: &str, name: &str| -> Result<String, String> {
        let said = |e: String| if lenient { Ok(name.trim_start_matches('.').to_string()) } else { Err(e) };
        if let Some(abs) = name.strip_prefix('.') {
            return match known.iter().find(|k| *k == abs).cloned() {
                Some(k) => Ok(k),
                None => said(format!("there is no type `{name}`")),
            };
        }
        let mut s = scope.to_string();
        loop {
            let full = if s.is_empty() { name.to_string() } else { format!("{s}.{name}") };
            if known.contains(&full) {
                return Ok(full);
            }
            if s.is_empty() {
                return said(format!("there is no type `{name}` (in `{scope}`)"));
            }
            s = s.rsplit_once('.').map(|(a, _)| a.to_string()).unwrap_or_default();
        }
    };
    fn fix(t: &PType, scope: &str, resolve: &dyn Fn(&str, &str) -> Result<String, String>) -> Result<PType, String> {
        Ok(match t {
            PType::Scalar(s) => PType::Scalar(s.clone()),
            PType::Named(n) => PType::Named(resolve(scope, n)?),
            PType::Map(k, v) => PType::Map(Box::new(fix(k, scope, resolve)?), Box::new(fix(v, scope, resolve)?)),
        })
    }
    for (name, raw) in &raws {
        let mut fields = Vec::new();
        for (fl, scope) in &raw.fields {
            let mut fl = fl.clone();
            fl.ty = fix(&fl.ty, scope, &resolve)?;
            // a singular message field says whether it is set
            if let PType::Named(n) = &fl.ty {
                if !fl.repeated && !f.enums.contains_key(n) && n != "google.protobuf.NullValue" {
                    fl.presence = true;
                }
            }
            fields.push(fl);
        }
        f.messages.insert(name.clone(), fields);
    }
    for svc in services {
        let scope = svc.name.rsplit_once('.').map(|(a, _)| a.to_string()).unwrap_or_default();
        let mut ms = Vec::new();
        for m in svc.methods {
            ms.push(Method { name: m.name, input: resolve(&scope, &m.input)?, output: resolve(&scope, &m.output)?, streams: m.streams, options: m.options.into_iter().collect() });
        }
        f.services.push(Service { name: svc.name, methods: ms, options: svc.options.into_iter().collect(), imports_options: svc.imports_options });
    }
    Ok(f)
}

fn read(path: &Path, f: &mut ProtoFile, rd: &mut Reading, first: bool) -> Result<(), String> {
    let canon = crate::sources::canonical(path);
    if rd.seen.contains(&canon) {
        return Ok(());
    }
    rd.seen.push(canon);
    let src = crate::sources::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    read_text(path, &src, f, rd, first)
}

/// The directories an import is looked for in, in order. A file that sits where its package says
/// (`shop/v1/order.proto` for `package shop.v1`) is in a buf module whose root is above `shop/`,
/// and imports are named from that root; a file anywhere else imports from its own directory.
fn import_dirs(dir: &Path, package: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if !package.is_empty() {
        let at: PathBuf = package.split('.').collect();
        if dir.ends_with(&at) {
            if let Some(root) = dir.ancestors().nth(at.components().count()) {
                out.push(root.to_path_buf());
            }
        }
    }
    out.push(dir.to_path_buf());
    out
}

/// Read one file's text; `path` says where it is, for the messages and for the imports.
fn read_text(path: &Path, src: &str, f: &mut ProtoFile, rd: &mut Reading, first: bool) -> Result<(), String> {
    let mut p = P { t: tokens(src)?, i: 0 };
    let mut package = String::new();
    let mut imports = Vec::new();
    let first_service = rd.services.len();
    while let Some(t) = p.peek().cloned() {
        match t {
            T::Word(w) if w == "syntax" => {
                p.i += 1;
                p.sym('=')?;
                match p.peek().cloned() {
                    Some(T::Str(s)) if s == "proto3" => {}
                    Some(T::Str(s)) => return Err(format!("{}: `{s}` is not read; dandori reads proto3", path.display())),
                    _ => return Err(p.err("the syntax")),
                }
                p.skip_statement();
            }
            T::Word(w) if w == "edition" => return Err(format!("{}: editions are not read; dandori reads proto3", path.display())),
            T::Word(w) if w == "package" => {
                p.i += 1;
                package = p.word()?;
                p.sym(';')?;
            }
            T::Word(w) if w == "import" => {
                p.i += 1;
                if p.is_word("public") || p.is_word("weak") {
                    p.i += 1;
                }
                match p.peek().cloned() {
                    Some(T::Str(s)) => imports.push(s),
                    _ => return Err(p.err("the file imported")),
                }
                p.skip_statement();
            }
            T::Word(w) if w == "message" => {
                p.i += 1;
                message(&mut p, &package, f, &mut rd.raws)?;
            }
            T::Word(w) if w == "enum" => {
                p.i += 1;
                enumeration(&mut p, &package, f)?;
            }
            T::Word(w) if w == "service" => {
                p.i += 1;
                let name = p.word()?;
                let full = if package.is_empty() { name } else { format!("{package}.{name}") };
                p.sym('{')?;
                let mut svc = RawService { name: full, methods: Vec::new(), options: Map::new(), imports_options: false };
                while !p.is_sym('}') {
                    if p.is_word("rpc") {
                        p.i += 1;
                        let m = p.word()?;
                        p.sym('(')?;
                        let mut streams = false;
                        if p.is_word("stream") {
                            p.i += 1;
                            streams = true;
                        }
                        let input = p.word()?;
                        p.sym(')')?;
                        if !p.is_word("returns") {
                            return Err(p.err("`returns`"));
                        }
                        p.i += 1;
                        p.sym('(')?;
                        if p.is_word("stream") {
                            p.i += 1;
                            streams = true;
                        }
                        let output = p.word()?;
                        p.sym(')')?;
                        let mut options = Map::new();
                        if p.is_sym('{') {
                            p.i += 1;
                            loop {
                                if p.is_sym('}') {
                                    p.i += 1;
                                    break;
                                }
                                if p.is_word("option") {
                                    p.i += 1;
                                    p.option_statement(&mut options);
                                } else if p.peek().is_none() {
                                    return Err(p.err("`}`"));
                                } else {
                                    p.skip_statement();
                                }
                            }
                        } else {
                            p.sym(';')?;
                        }
                        svc.methods.push(RawMethod { name: m, input, output, streams, options });
                    } else if p.is_word("option") {
                        p.i += 1;
                        p.option_statement(&mut svc.options);
                    } else if p.peek().is_none() {
                        return Err(p.err("`}`"));
                    } else {
                        p.skip_statement();
                    }
                }
                p.i += 1;
                rd.services.push(svc);
            }
            T::Sym(';') => p.i += 1,
            _ => p.skip_statement(),
        }
    }
    if first {
        f.package = package.clone();
    }
    // the services this file wrote know whether it imports what a service's options need
    let uses_options = imports.iter().any(|i| i == OPTIONS_IMPORT);
    for svc in &mut rd.services[first_service..] {
        svc.imports_options = uses_options;
    }
    let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
    for imp in imports {
        if imp.starts_with("google/protobuf/") || imp == "buf/validate/validate.proto" {
            continue;
        }
        if imp == OPTIONS_IMPORT {
            let at = PathBuf::from(OPTIONS_IMPORT);
            if !rd.seen.contains(&at) {
                rd.seen.push(at.clone());
                read_text(&at, DANDORI_OPTIONS, f, rd, false)?;
            }
            continue;
        }
        // the file is where an import is looked for, or it is passed over; a file that is there and
        // cannot be read as a `.proto` is the trouble
        let dirs = import_dirs(&dir, &package);
        match dirs.iter().map(|d| d.join(&imp)).find(|at| crate::sources::read(at).is_ok()) {
            Some(at) => read(&at, f, rd, false)?,
            None => {
                if !f.unread.contains(&imp) {
                    f.unread.push(imp);
                }
            }
        }
    }
    Ok(())
}

fn message(p: &mut P, scope: &str, f: &mut ProtoFile, raws: &mut BTreeMap<String, Raw>) -> Result<(), String> {
    let name = p.word()?;
    let full = if scope.is_empty() { name } else { format!("{scope}.{name}") };
    p.sym('{')?;
    let mut fields = Vec::new();
    let mut oneof_depth = 0;
    loop {
        match p.peek().cloned() {
            None => return Err(p.err("`}`")),
            Some(T::Sym('}')) => {
                p.i += 1;
                if oneof_depth > 0 {
                    oneof_depth -= 1;
                    continue;
                }
                break;
            }
            Some(T::Sym(';')) => p.i += 1,
            Some(T::Word(w)) if w == "message" => {
                p.i += 1;
                message(p, &full, f, raws)?;
            }
            Some(T::Word(w)) if w == "enum" => {
                p.i += 1;
                enumeration(p, &full, f)?;
            }
            Some(T::Word(w)) if w == "oneof" => {
                p.i += 1;
                p.word()?;
                p.sym('{')?;
                oneof_depth += 1;
            }
            Some(T::Word(w)) if ["option", "reserved", "extensions", "extend"].contains(&w.as_str()) => p.skip_statement(),
            Some(T::Word(w)) => {
                p.i += 1;
                let (repeated, optional, ty) = match w.as_str() {
                    "repeated" => (true, false, p.word()?),
                    "optional" => (false, true, p.word()?),
                    "required" => (false, false, p.word()?),
                    _ => (false, false, w),
                };
                let ty = if ty == "map" {
                    p.sym('<')?;
                    let k = p.word()?;
                    p.sym(',')?;
                    let v = p.word()?;
                    p.sym('>')?;
                    PType::Map(Box::new(PType::Scalar(k)), Box::new(if SCALARS.contains(&v.as_str()) { PType::Scalar(v) } else { PType::Named(v) }))
                } else if SCALARS.contains(&ty.as_str()) {
                    PType::Scalar(ty)
                } else {
                    PType::Named(ty)
                };
                let fname = p.word()?;
                p.sym('=')?;
                match p.peek() {
                    Some(T::Num(_)) => p.i += 1,
                    _ => return Err(p.err("the field's number")),
                }
                let options = p.options_list()?;
                p.sym(';')?;
                let json = options.get("json_name").and_then(|v| v.as_str()).map(String::from).unwrap_or_else(|| json_name(&fname));
                let rules = options.get("buf.validate.field").cloned().unwrap_or(Value::Null);
                let is_map = matches!(ty, PType::Map(..));
                fields.push((PField { name: fname, json, ty, repeated: repeated && !is_map, presence: optional || oneof_depth > 0, rules }, full.clone()));
            }
            Some(_) => p.skip_statement(),
        }
    }
    raws.insert(full, Raw { fields });
    Ok(())
}

fn enumeration(p: &mut P, scope: &str, f: &mut ProtoFile) -> Result<(), String> {
    let name = p.word()?;
    let full = if scope.is_empty() { name } else { format!("{scope}.{name}") };
    p.sym('{')?;
    let mut values = Vec::new();
    loop {
        match p.peek().cloned() {
            None => return Err(p.err("`}`")),
            Some(T::Sym('}')) => {
                p.i += 1;
                break;
            }
            Some(T::Sym(';')) => p.i += 1,
            Some(T::Word(w)) if w == "option" || w == "reserved" => p.skip_statement(),
            Some(T::Word(w)) => {
                p.i += 1;
                p.sym('=')?;
                p.i += 1;
                p.options_list()?;
                p.sym(';')?;
                values.push(w);
            }
            Some(_) => p.skip_statement(),
        }
    }
    f.enums.insert(full, values);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn one(text: &str) -> ProtoFile {
        load_text("t.proto", text).unwrap_or_else(|e| panic!("{e}"))
    }

    fn field<'a>(f: &'a ProtoFile, msg: &str, name: &str) -> &'a PField {
        f.messages[msg].iter().find(|x| x.name == name).unwrap_or_else(|| panic!("no field {name} in {msg}"))
    }

    #[test]
    fn protovalidate_is_read_as_a_group_or_one_rule_at_a_time() {
        let f = one(
            r#"syntax = "proto3"; package a.v1;
            message M {
              int32 grouped = 1 [(buf.validate.field).int32 = {gte: 1, lte: 100}];
              int32 apart = 2 [(buf.validate.field).int32.gte = 1, (buf.validate.field).int32.lt = 50];
              int32 negative = 3 [(buf.validate.field).int32 = {gte: -5, lte: 5}, json_name = "neg"];
              int32 zero_ok = 4 [(buf.validate.field).int32.gt = 0, (buf.validate.field).ignore = IGNORE_IF_ZERO_VALUE];
              repeated int32 each = 5 [(buf.validate.field).repeated.items.int32 = {gte: 1, lte: 9}];
              optional string note = 6 [(buf.validate.field).required = true];
              string plain = 7;
            }"#,
        );
        assert_eq!(field(&f, "a.v1.M", "grouped").rules, json!({"int32": {"gte": 1, "lte": 100}}));
        assert_eq!(field(&f, "a.v1.M", "apart").rules, json!({"int32": {"gte": 1, "lt": 50}}));
        assert_eq!(field(&f, "a.v1.M", "negative").rules, json!({"int32": {"gte": -5, "lte": 5}}));
        assert_eq!(field(&f, "a.v1.M", "negative").json, "neg");
        assert_eq!(field(&f, "a.v1.M", "zero_ok").rules, json!({"int32": {"gt": 0}, "ignore": "IGNORE_IF_ZERO_VALUE"}));
        assert_eq!(field(&f, "a.v1.M", "each").rules, json!({"repeated": {"items": {"int32": {"gte": 1, "lte": 9}}}}));
        assert_eq!(field(&f, "a.v1.M", "note").rules, json!({"required": true}));
        assert_eq!(field(&f, "a.v1.M", "plain").rules, Value::Null);
    }

    #[test]
    fn an_option_it_cannot_read_is_passed_over_and_the_others_are_kept() {
        let f = one(
            r#"syntax = "proto3"; package a.v1;
            message M {
              string s = 1 [(buf.validate.field).string.(my.rule) = true, (buf.validate.field).string.min_len = 2, deprecated = true];
              int32 n = 2 [(buf.validate.field).int32 = {in: [1, 2, 3], gte: 1}];
            }"#,
        );
        let s = field(&f, "a.v1.M", "s");
        assert_eq!(s.rules, json!({"string": {"min_len": 2}}));
        assert_eq!(field(&f, "a.v1.M", "n").rules, json!({"int32": {"in": [1, 2, 3], "gte": 1}}));
    }

    #[test]
    fn a_service_and_its_methods_have_their_options() {
        let f = one(
            r#"syntax = "proto3"; package shop.v1;
            import "dandori/v1/options.proto";
            service S {
              option (dandori.v1.workflow) = {name: "fulfillment", version: 1};
              rpc Start(A) returns (B) {
                option (dandori.v1.start) = {fails: ["OutOfStock", "DeliveryFailed"]};
                option idempotency_level = NO_SIDE_EFFECTS;
              }
              rpc Again(A) returns (B) {
                option (.dandori.v1.start) = {fails: "One" fails: "Two"};
              }
              rpc Plain(A) returns (B);
              rpc Stream(stream A) returns (stream B) {}
            }
            message A {}
            message B {}"#,
        );
        let s = &f.services[0];
        assert_eq!(s.name, "shop.v1.S");
        assert!(s.imports_options);
        assert_eq!(s.options["dandori.v1.workflow"], json!({"name": "fulfillment", "version": 1}));
        let m = |n: &str| s.methods.iter().find(|m| m.name == n).unwrap();
        assert_eq!(m("Start").options["dandori.v1.start"], json!({"fails": ["OutOfStock", "DeliveryFailed"]}));
        assert_eq!(m("Start").options["idempotency_level"], json!("NO_SIDE_EFFECTS"));
        assert_eq!(strings_of(&m("Start").options["dandori.v1.start"]["fails"]), ["OutOfStock", "DeliveryFailed"]);
        // a name written by `(.dandori.v1.start)`, a field written again: the same as the list
        assert_eq!(m("Again").options["dandori.v1.start"], json!({"fails": ["One", "Two"]}));
        assert_eq!(strings_of(&json!("Only")), ["Only"]);
        assert!(m("Plain").options.is_empty());
        assert!(m("Stream").streams);
    }

    #[test]
    fn a_message_value_takes_its_fields_apart_by_a_comma_a_semicolon_or_nothing() {
        let f = one(
            r#"syntax = "proto3"; package a.v1;
            service S {
              option (x.y) = {name: "a" version: 1};
              option (x.z) = {name: "b"; version: 2; inner { deep: [1, 2] } tag: "a;b]c"};
              option (x.w) = { list: [ {k: 1}, {k: 2} ] };
            }"#,
        );
        let o = &f.services[0].options;
        assert_eq!(o["x.y"], json!({"name": "a", "version": 1}));
        assert_eq!(o["x.z"], json!({"name": "b", "version": 2, "inner": {"deep": [1, 2]}, "tag": "a;b]c"}));
        assert_eq!(o["x.w"], json!({"list": [{"k": 1}, {"k": 2}]}));
    }

    #[test]
    fn dandoris_options_are_known_without_their_file() {
        let f = one(r#"syntax = "proto3"; package shop.v1; import "dandori/v1/options.proto"; import "google/protobuf/timestamp.proto"; import "buf/validate/validate.proto"; message A { dandori.v1.Status status = 1; }"#);
        let status = &f.messages["dandori.v1.Status"];
        let at = status.iter().find(|x| x.name == "at").unwrap();
        assert!(at.presence, "`at` is optional");
        assert_eq!(at.ty, PType::Scalar("int32".into()));
        let cases = status.iter().find(|x| x.name == "cases").unwrap();
        assert_eq!(cases.ty, PType::Map(Box::new(PType::Scalar("string".into())), Box::new(PType::Named("google.protobuf.Value".into()))));
        let events = status.iter().find(|x| x.name == "events").unwrap();
        assert!(events.repeated);
        assert!(f.messages.contains_key("dandori.v1.StartOptions"));
        // the package that wrote them is the first file's
        assert_eq!(f.package, "shop.v1");
    }

    #[test]
    fn a_file_with_the_options_and_the_clutter_of_a_real_service_is_read() {
        let f = one(
            r#"syntax = "proto3";
            package acme.v1;
            import "google/protobuf/timestamp.proto";
            import "buf/validate/validate.proto";
            option go_package = "acme/v1;acmev1";
            option java_multiple_files = true;
            /* a block
               comment; with a semicolon */
            service Orders {
              option (some.api.default_host) = "orders.example.com";
              rpc Get(GetOrderRequest) returns (Order) {
                option (some.api.http) = { get: "/v1/{name=orders/*}" additional_bindings { get: "/v1/x" body: "*" } };
                option idempotency_level = NO_SIDE_EFFECTS;
              }
              // a method with its options on one line
              rpc Put(Order) returns (Order) { option deprecated = true; };
            }
            message GetOrderRequest { string name = 1 [(buf.validate.field).string = {min_len: 1, max_len: 63, pattern: "^orders/[a-z]+$"}]; }
            message Order {
              option (buf.validate.message).cel = { id: "x", message: "a;b]c", expression: "this.total > 0" };
              reserved 4, 5; reserved "old", "older";
              extensions 100 to 199;
              string name = 1 [(buf.validate.field).required = true, (some.api.field_behavior) = IDENTIFIER, json_name = "orderName"];
              int32 total = 2 [(buf.validate.field).int32 = {gte: 0, lte: 1000}, deprecated = true];
              repeated Line lines = 3 [(buf.validate.field).repeated = {min_items: 1, max_items: 50, items: {int32: {gte: 1}}}];
              google.protobuf.Timestamp created = 6;
              oneof payment { option (buf.validate.oneof).required = true; string card = 7; string bank = 8; }
              enum Kind { option allow_alias = true; KIND_UNSPECIFIED = 0; KIND_A = 1 [deprecated = true]; KIND_B = 1; }
              message Line { string sku = 1; }
            }"#,
        );
        let o = &f.messages["acme.v1.Order"];
        let get = |n: &str| o.iter().find(|x| x.name == n).unwrap_or_else(|| panic!("no field {n}"));
        assert_eq!(get("name").json, "orderName");
        assert_eq!(get("name").rules, json!({"required": true}));
        assert_eq!(get("total").rules, json!({"int32": {"gte": 0, "lte": 1000}}));
        // the rules on a list go as they were written, the items' rules among them
        assert_eq!(get("lines").rules, json!({"repeated": {"min_items": 1, "max_items": 50, "items": {"int32": {"gte": 1}}}}));
        assert!(get("card").presence && get("bank").presence);
        assert_eq!(f.enums["acme.v1.Order.Kind"], ["KIND_UNSPECIFIED", "KIND_A", "KIND_B"]);
        assert!(f.messages.contains_key("acme.v1.Order.Line"));
        let s = &f.services[0];
        assert_eq!(s.methods.len(), 2);
        let get_method = &s.methods[0];
        assert_eq!(get_method.options["some.api.http"]["additional_bindings"], json!({"get": "/v1/x", "body": "*"}));
        assert_eq!(get_method.options["idempotency_level"], json!("NO_SIDE_EFFECTS"));
        assert_eq!(s.options["some.api.default_host"], json!("orders.example.com"));
    }

    /// A directory of files for a test, named for it.
    fn scratch(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("dandori-proto-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for (path, text) in files {
            let p = dir.join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        }
        dir
    }

    #[test]
    fn an_import_is_named_from_the_root_of_the_module_when_the_file_sits_at_its_package() {
        let dir = scratch(
            "buf",
            &[
                ("shop/v1/order.proto", r#"syntax = "proto3"; package shop.v1; import "common/v1/money.proto"; message Order { common.v1.Money total = 1; }"#),
                ("common/v1/money.proto", r#"syntax = "proto3"; package common.v1; message Money { string currency = 1; int64 units = 2; }"#),
            ],
        );
        let f = load(&dir.join("shop/v1/order.proto")).unwrap_or_else(|e| panic!("{e}"));
        assert!(f.messages.contains_key("common.v1.Money"));
        assert_eq!(f.messages["shop.v1.Order"][0].ty, PType::Named("common.v1.Money".into()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_not_at_its_package_imports_from_its_own_directory() {
        let dir = scratch(
            "flat",
            &[
                ("specs/order.proto", r#"syntax = "proto3"; package shop.v1; import "money.proto"; message Order { Money total = 1; }"#),
                ("specs/money.proto", r#"syntax = "proto3"; package shop.v1; message Money { string currency = 1; }"#),
            ],
        );
        let f = load(&dir.join("specs/order.proto")).unwrap_or_else(|e| panic!("{e}"));
        assert!(f.messages.contains_key("shop.v1.Money"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_import_that_is_not_on_the_disk_is_passed_over_and_told() {
        let dir = scratch(
            "gone",
            &[(
                "specs/order.proto",
                r#"syntax = "proto3"; package shop.v1;
import "google/api/annotations.proto";
import "google/type/money.proto";
import "google/protobuf/timestamp.proto";
message Order { string id = 1; google.type.Money total = 2; repeated google.type.Money parts = 3; google.protobuf.Timestamp at = 4; }
service OrderService { rpc Place(Order) returns (Order) { option (google.api.http) = {post: "/v1/orders" body: "*"}; } }"#,
            )],
        );
        let f = load(&dir.join("specs/order.proto")).unwrap_or_else(|e| panic!("{e}"));
        // in the order they were met; the well-known file is known without its file, so it is not among them
        assert_eq!(f.unread, ["google/api/annotations.proto", "google/type/money.proto"]);
        // what is read stays as it is; a type of an import that was not read stays a name nothing has
        let order = &f.messages["shop.v1.Order"];
        assert_eq!(order[1].ty, PType::Named("google.type.Money".into()));
        assert!(!f.knows("google.type.Money") && f.knows("shop.v1.Order") && f.knows("google.protobuf.Timestamp"));
        assert_eq!(f.services[0].methods[0].input, "shop.v1.Order");
        // a name nothing has, with every import read, is still the trouble
        let all = scratch("typo", &[("specs/order.proto", r#"syntax = "proto3"; package shop.v1; message Order { Moneyy total = 1; }"#)]);
        let e = load(&all.join("specs/order.proto")).unwrap_err();
        assert!(e.contains("there is no type `Moneyy`"), "{e}");
        // a file that is there and is not a `.proto` is the trouble too
        let bad = scratch("bad", &[("specs/order.proto", r#"syntax = "proto3"; package shop.v1; import "money.proto"; message Order {}"#), ("specs/money.proto", "message {")]);
        assert!(load(&bad.join("specs/order.proto")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&all);
        let _ = std::fs::remove_dir_all(&bad);
    }
}
