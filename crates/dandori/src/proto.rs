//! A reader of `.proto` files (proto3): the package, the messages and enums with their fields,
//! and the services' methods, which a `connect` task is held to. Options are passed over but
//! `json_name`. Google's well-known types are known without their files; any other import is
//! read from the importing file's directory.

use std::collections::BTreeMap;
use std::path::Path;

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
}

#[derive(Clone, Debug)]
pub struct Method {
    pub name: String,
    pub input: String,
    pub output: String,
    pub streams: bool,
}

#[derive(Clone, Debug)]
pub struct Service {
    /// the full name, as a Connect path has it: `warehouse.v1.StockService`
    pub name: String,
    pub methods: Vec<Method>,
}

#[derive(Clone, Debug, Default)]
pub struct ProtoFile {
    pub package: String,
    pub messages: BTreeMap<String, Vec<PField>>,
    /// the values by name, in order; the first is the zero value
    pub enums: BTreeMap<String, Vec<String>>,
    pub services: Vec<Service>,
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
    /// `[a = 1, json_name = "x"]`: the json_name, if it is there.
    fn field_options(&mut self) -> Result<Option<String>, String> {
        let mut json = None;
        if self.is_sym('[') {
            self.i += 1;
            let mut depth = 1;
            while depth > 0 {
                match self.peek().cloned() {
                    None => return Err(self.err("`]`")),
                    Some(T::Sym('[')) => depth += 1,
                    Some(T::Sym(']')) => depth -= 1,
                    Some(T::Word(w)) if w == "json_name" && depth == 1 => {
                        self.i += 1;
                        self.sym('=')?;
                        if let Some(T::Str(s)) = self.peek().cloned() {
                            json = Some(s);
                        }
                        continue;
                    }
                    _ => {}
                }
                self.i += 1;
            }
        }
        Ok(json)
    }
}

/// A message or an enum as declared, before the names of its fields' types are resolved.
struct Raw {
    fields: Vec<(PField, String)>,
}

pub fn load(path: &Path) -> Result<ProtoFile, String> {
    let mut f = ProtoFile::default();
    let mut raws: BTreeMap<String, Raw> = BTreeMap::new();
    let mut raw_services: Vec<(String, Vec<(String, String, String, bool)>)> = Vec::new();
    let mut seen = Vec::new();
    read(path, &mut f, &mut raws, &mut raw_services, &mut seen, true)?;
    // resolve every type name from the scope it is written in, the innermost first
    let known: Vec<String> = raws.keys().cloned().chain(f.enums.keys().cloned()).chain(WELL_KNOWN.iter().map(|s| s.to_string())).collect();
    let resolve = |scope: &str, name: &str| -> Result<String, String> {
        if let Some(abs) = name.strip_prefix('.') {
            return known.iter().find(|k| *k == abs).cloned().ok_or_else(|| format!("there is no type `{name}`"));
        }
        let mut s = scope.to_string();
        loop {
            let full = if s.is_empty() { name.to_string() } else { format!("{s}.{name}") };
            if known.contains(&full) {
                return Ok(full);
            }
            if s.is_empty() {
                return Err(format!("there is no type `{name}` (in `{scope}`)"));
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
    for (name, methods) in raw_services {
        let scope = name.rsplit_once('.').map(|(a, _)| a.to_string()).unwrap_or_default();
        let mut ms = Vec::new();
        for (m, i, o, streams) in methods {
            ms.push(Method { name: m, input: resolve(&scope, &i)?, output: resolve(&scope, &o)?, streams });
        }
        f.services.push(Service { name, methods: ms });
    }
    Ok(f)
}

type RawServices = Vec<(String, Vec<(String, String, String, bool)>)>;

fn read(path: &Path, f: &mut ProtoFile, raws: &mut BTreeMap<String, Raw>, services: &mut RawServices, seen: &mut Vec<std::path::PathBuf>, first: bool) -> Result<(), String> {
    let canon = crate::sources::canonical(path);
    if seen.contains(&canon) {
        return Ok(());
    }
    seen.push(canon);
    let src = crate::sources::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let mut p = P { t: tokens(&src)?, i: 0 };
    let mut package = String::new();
    let mut imports = Vec::new();
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
                message(&mut p, &package, f, raws)?;
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
                let mut methods = Vec::new();
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
                        if p.is_sym('{') {
                            p.skip_statement();
                        } else {
                            p.sym(';')?;
                        }
                        methods.push((m, input, output, streams));
                    } else if p.peek().is_none() {
                        return Err(p.err("`}`"));
                    } else {
                        p.skip_statement();
                    }
                }
                p.i += 1;
                services.push((full, methods));
            }
            T::Sym(';') => p.i += 1,
            _ => p.skip_statement(),
        }
    }
    if first {
        f.package = package.clone();
    }
    let dir = path.parent().unwrap_or(Path::new("."));
    for imp in imports {
        if imp.starts_with("google/protobuf/") {
            continue;
        }
        read(&dir.join(&imp), f, raws, services, seen, false)?;
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
                let json = p.field_options()?.unwrap_or_else(|| json_name(&fname));
                p.sym(';')?;
                let is_map = matches!(ty, PType::Map(..));
                fields.push((PField { name: fname, json, ty, repeated: repeated && !is_map, presence: optional || oneof_depth > 0 }, full.clone()));
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
                p.field_options()?;
                p.sym(';')?;
                values.push(w);
            }
            Some(_) => p.skip_statement(),
        }
    }
    f.enums.insert(full, values);
    Ok(())
}
