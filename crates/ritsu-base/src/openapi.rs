//! The operations of a contract written as an OpenAPI document (3.0, 3.1, 3.2) or an AsyncAPI
//! document (3.0, 3.1), read with [`crate::yaml`]: each operation with its `operationId` (or
//! AsyncAPI's key), its method and path (or its action and channel), its parameters, the fields of
//! its body with their types, ranges and enums, who may call it (`security`), and the status codes
//! it answers with. A language that holds an operation of a contract to something — sekisho's
//! `guards`, an action's `input` and the argument it reads a resource's id from — reads it here,
//! so that no two of them read an operation two ways.
//!
//! What a document means beyond its operations (where its schemas are, what a `$ref` lands on)
//! stays with the language that asks.

use crate::text::Text;
use crate::yaml::{self, Key, Node, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// The kind of a document.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    OpenApi,
    AsyncApi,
}

impl Kind {
    /// The word a reference writes it with (`openapi`, `asyncapi`).
    pub fn word(self) -> &'static str {
        match self {
            Kind::OpenApi => "openapi",
            Kind::AsyncApi => "asyncapi",
        }
    }

    /// The name people know it by.
    pub fn title(self) -> &'static str {
        match self {
            Kind::OpenApi => "OpenAPI",
            Kind::AsyncApi => "AsyncAPI",
        }
    }
}

/// A document, read for its operations.
#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    pub kind: Kind,
    /// The version of the specification it keeps to, as written (`3.1.0`).
    pub spec: String,
    /// `info.title` and `info.version`.
    pub title: String,
    pub version: String,
    /// Every operation, in the order written.
    pub operations: Vec<Operation>,
    /// The `$ref`s the reader could not follow: to a file it was not given, or to nothing. What
    /// sits behind one is not read (a parameter, a body, a schema), so a question about it cannot
    /// be answered from the document alone.
    pub unresolved: Vec<Unresolved>,
}

/// A `$ref` not followed, as written, and where it is: the file it is written in (the document, or
/// a file a `$ref` of it reached, as the loader was given it), its line and its column (from 1).
#[derive(Clone, Debug, PartialEq)]
pub struct Unresolved {
    pub file: String,
    pub written: String,
    pub line: usize,
    pub col: usize,
}

/// One operation.
#[derive(Clone, Debug, PartialEq)]
pub struct Operation {
    /// OpenAPI's `operationId`, empty when it has none; AsyncAPI's key under `operations`.
    pub id: String,
    /// OpenAPI: the method, in capitals (`POST`; a key of 3.2's `additionalOperations` as written).
    /// AsyncAPI: the action, `send` or `receive`.
    pub method: String,
    /// OpenAPI: the path as `paths` writes it (`/orders/{orderId}/refunds`), or the name of a
    /// webhook. AsyncAPI: the address of the channel, or its key when it has none.
    pub path: String,
    /// Where the operation is: a JSON Pointer from the document's root under `paths` (or `webhooks`,
    /// `operations`), as the document names it (`/paths/~1orders~1{orderId}~1refunds/post`), and the
    /// line and the column of its key where it is written (for a path item a `$ref` reaches, there).
    pub pointer: String,
    pub line: usize,
    pub col: usize,
    /// OpenAPI: under `webhooks`, not `paths`.
    pub webhook: bool,
    /// OpenAPI: the path item's parameters and the operation's own (the operation's in place of
    /// the path item's of the same name and place). AsyncAPI: the parameters of the channel's
    /// address.
    pub params: Vec<Param>,
    /// What the operation is sent: OpenAPI's request body, AsyncAPI's message payload. None when
    /// it has none.
    pub body: Option<Body>,
    /// Who may call it. OpenAPI: the operation's own `security`, else the document's. AsyncAPI: the
    /// operation's own (what a server asks is not read). None when neither writes one.
    pub security: Option<Security>,
    /// OpenAPI: the status codes of its responses, as written (`201`, `4XX`, `default`), each with
    /// its line. Empty for AsyncAPI.
    pub responses: Vec<(String, usize)>,
}

/// Where a parameter goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    Path,
    Query,
    Header,
    Cookie,
    /// A parameter of an AsyncAPI channel's address (`orders/{orderId}`).
    Channel,
}

impl Place {
    /// As OpenAPI's `in` writes it; `channel` for AsyncAPI's.
    pub fn word(self) -> &'static str {
        match self {
            Place::Path => "path",
            Place::Query => "query",
            Place::Header => "header",
            Place::Cookie => "cookie",
            Place::Channel => "channel",
        }
    }
}

/// One parameter of an operation.
#[derive(Clone, Debug, PartialEq)]
pub struct Param {
    pub name: String,
    pub place: Place,
    /// A path parameter is always required.
    pub required: bool,
    pub schema: Schema,
    pub line: usize,
    pub col: usize,
}

/// What an operation is sent.
#[derive(Clone, Debug, PartialEq)]
pub struct Body {
    /// OpenAPI's `requestBody.required`; AsyncAPI's payload is always sent.
    pub required: bool,
    /// The media type the fields are read from: `application/json` first, else a form
    /// (`application/x-www-form-urlencoded`, `multipart/form-data`), else the first one written;
    /// AsyncAPI's message `contentType`, else the document's `defaultContentType`.
    pub media: String,
    /// The schema of the body itself.
    pub schema: Schema,
    /// The properties of the schema, when it is an object (through `allOf`), in the order written.
    pub fields: Vec<Field>,
    pub line: usize,
    pub col: usize,
}

/// One field of a body.
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub name: String,
    /// Listed in the object's `required`.
    pub required: bool,
    pub schema: Schema,
    pub line: usize,
    pub col: usize,
}

/// What a schema says of a value, as far as an operation's arguments are held to it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Schema {
    /// The JSON Schema type (`integer`, `number`, `string`, `boolean`, `array`, `object`): 3.0's
    /// `type`, or the one besides `null` of 3.1's list of types. None when none is written, or
    /// when the list has more than one besides `null`.
    pub ty: Option<String>,
    /// `null` is one of its values: 3.0's `nullable: true`, or `null` in 3.1's list of types or in
    /// its `enum`.
    pub nullable: bool,
    /// `format` (`int64`, `date`, `date-time`).
    pub format: Option<String>,
    /// The lowest and the highest value it takes, each with whether that value is itself left
    /// out: `minimum` (with 3.0's `exclusiveMinimum: true`), or 3.1's `exclusiveMinimum: <n>`.
    pub minimum: Option<Bound>,
    pub maximum: Option<Bound>,
    /// `enum` (or a `const`): each value as it goes on the wire, a string as itself and a number
    /// or a boolean as JSON writes it; `null` is left out (it sets `nullable`).
    pub values: Option<Vec<String>>,
    /// The mark that makes the value secret, when the schema has one (DESIGN 16.6, [`crate::marks`]):
    /// `x-data-classification` at `confidential` or `restricted`, `x-sensitive-data`, or `format:
    /// password`.
    pub mark: Option<crate::marks::SchemaMark>,
    /// Where the schema is written, after following the `$ref`s to it.
    pub line: usize,
    pub col: usize,
}

/// One end of a range.
#[derive(Clone, Debug, PartialEq)]
pub struct Bound {
    pub value: Number,
    /// The value itself is left out (`exclusiveMinimum`, `exclusiveMaximum`).
    pub exclusive: bool,
}

/// A number as a document writes it.
#[derive(Clone, Debug, PartialEq)]
pub enum Number {
    Int(i128),
    /// With a fraction or an exponent, as the digits were written.
    Decimal(String),
}

/// Who may call an operation, as the `security` that applies to it says.
#[derive(Clone, Debug, PartialEq)]
pub struct Security {
    /// Each requirement (any one of them is enough), each with the schemes it asks for (all of
    /// them). `security: []` is no requirement at all; a requirement `{}` asks for nothing.
    pub requirements: Vec<Vec<String>>,
    /// Written on the operation itself; false when the document's applies.
    pub own: bool,
    /// Where that `security` is written.
    pub line: usize,
    pub col: usize,
}

/// Why a text is not read as a document, and where.
#[derive(Clone, Debug, PartialEq)]
pub struct Problem {
    pub line: usize,
    pub col: usize,
    pub message: Text,
}

/// The document a text holds: JSON when `file` ends in `.json`, else YAML. A `$ref` to another
/// file is not followed (it is listed in [`Document::unresolved`]).
pub fn read(file: &str, text: &str) -> Result<Document, Problem> {
    read_with(file, text, &|_| None)
}

/// The document a text holds, a `$ref` to another file followed through `load`, which is given the
/// file's path from the document's directory joined to `file`'s (`api/schemas.yaml` for
/// `schemas.yaml` in `api/orders.yaml`) and answers its text, or None when it cannot.
pub fn read_with(file: &str, text: &str, load: &dyn Fn(&str) -> Option<String>) -> Result<Document, Problem> {
    let root = yaml::read_file_text(file, text).map_err(|e| Problem { line: e.line, col: e.col, message: e.message })?;
    from_node(file, &root, load)
}

/// The document a node holds, read as [`read_with`] reads a text.
pub fn from_node(file: &str, root: &Node, load: &dyn Fn(&str) -> Option<String>) -> Result<Document, Problem> {
    let (kind, spec) = kind_of(root)?;
    let r = Reader { load, docs: RefCell::new(HashMap::new()), unresolved: RefCell::new(Vec::new()) };
    let top = Loc { file: file.to_string(), doc: Rc::new(root.clone()), ptr: String::new() };
    let info = |k: &str| root.get("info").and_then(|i| i.get(k)).and_then(scalar).unwrap_or_default();
    let operations = match kind {
        Kind::OpenApi => r.openapi_operations(&top),
        Kind::AsyncApi => r.asyncapi_operations(&top),
    };
    Ok(Document { kind, spec, title: info("title"), version: info("version"), operations, unresolved: r.unresolved.into_inner() })
}

/// The methods of a path item, as OpenAPI 3.2 lists them (`query` is 3.2's).
const METHODS: [&str; 9] = ["get", "put", "post", "delete", "options", "head", "patch", "trace", "query"];

/// How many `$ref`s in a row are followed before the chain is taken for a loop.
const MAX_REFS: usize = 32;

/// A scalar as text: a string as itself, a number or a boolean as written.
fn scalar(n: &Node) -> Option<String> {
    match &n.value {
        Value::Str(s) => Some(s.clone()),
        Value::Int(i) => Some(i.to_string()),
        Value::Float(f) => Some(f.clone()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// What the top of a document says it is, and the version of the specification.
fn kind_of(root: &Node) -> Result<(Kind, String), Problem> {
    let at = |n: &Node, message: Text| Problem { line: n.line, col: n.col, message };
    if root.as_map().is_none() {
        return Err(at(root, crate::tr!("いちばん上がマッピングではありません", "its top is not a mapping")));
    }
    if let Some(v) = root.get("openapi") {
        let s = scalar(v).unwrap_or_default();
        return if ["3.0", "3.1", "3.2"].iter().any(|p| s == *p || s.starts_with(&format!("{p}."))) {
            Ok((Kind::OpenApi, s))
        } else {
            Err(at(v, crate::tr!("OpenAPI {s} の文書は読めません。読めるのは 3.0、3.1、3.2 です", "an OpenAPI {s} document is not read: 3.0, 3.1 and 3.2 are")))
        };
    }
    if let Some(v) = root.get("swagger") {
        return Err(at(v, crate::tr!("Swagger 2.0 の文書は読めません。OpenAPI 3 に変換してください", "a Swagger 2.0 document is not read; convert it to OpenAPI 3")));
    }
    if let Some(v) = root.get("asyncapi") {
        let s = scalar(v).unwrap_or_default();
        return if ["3.0", "3.1"].iter().any(|p| s == *p || s.starts_with(&format!("{p}."))) {
            Ok((Kind::AsyncApi, s))
        } else {
            Err(at(v, crate::tr!("AsyncAPI {s} の文書は読めません。読めるのは 3.0 と 3.1 です", "an AsyncAPI {s} document is not read: 3.0 and 3.1 are")))
        };
    }
    Err(at(
        root,
        crate::tr!(
            "OpenAPI の文書でも AsyncAPI の文書でもありません（いちばん上に `openapi` も `asyncapi` もありません）",
            "it is neither an OpenAPI nor an AsyncAPI document (its top has no `openapi` and no `asyncapi`)"
        ),
    ))
}

/// A key as a token of a JSON Pointer: `~` as `~0`, `/` as `~1`.
fn escape(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

/// A fragment of a URI with its `%XX` read back (`%7Bid%7D` is `{id}`).
fn unpercent(s: &str) -> String {
    let b = s.as_bytes();
    let hex = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2]))
        {
            out.push(h * 16 + l);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

/// The file `rel` names, written in the document `file`: beside it, `.` and `..` folded.
fn beside(file: &str, rel: &str) -> String {
    let dir = match file.rfind('/') {
        Some(i) => &file[..i],
        None => "",
    };
    let rooted = dir.starts_with('/');
    let mut parts: Vec<&str> = dir.split('/').filter(|p| !p.is_empty() && *p != ".").collect();
    for p in rel.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                if parts.last().is_some_and(|l| *l != "..") {
                    parts.pop();
                } else if !rooted {
                    parts.push("..");
                }
            }
            _ => parts.push(p),
        }
    }
    let joined = parts.join("/");
    if rooted { format!("/{joined}") } else { joined }
}

/// A node of one of the documents read: the document's file and root, and a JSON Pointer to it.
#[derive(Clone)]
struct Loc {
    file: String,
    doc: Rc<Node>,
    ptr: String,
}

impl Loc {
    fn node(&self) -> &Node {
        self.doc.pointer(&self.ptr).unwrap_or(&self.doc)
    }

    /// The value of a key of the mapping here.
    fn key(&self, k: &str) -> Option<Loc> {
        let ptr = format!("{}/{}", self.ptr, escape(k));
        self.doc.pointer(&ptr).map(|_| Loc { file: self.file.clone(), doc: self.doc.clone(), ptr })
    }

    /// The items of the sequence here.
    fn items(&self) -> Vec<Loc> {
        let n = self.node().as_seq().map(|xs| xs.len()).unwrap_or(0);
        (0..n).map(|i| Loc { file: self.file.clone(), doc: self.doc.clone(), ptr: format!("{}/{i}", self.ptr) }).collect()
    }

    /// The keys of the mapping here, with the value of each.
    fn entries(&self) -> Vec<(Key, Loc)> {
        self.node().as_map().unwrap_or_default().iter().filter_map(|(k, _)| self.key(&k.name).map(|l| (k.clone(), l))).collect()
    }

    fn str(&self, k: &str) -> Option<String> {
        self.node().get(k).and_then(Node::as_str).map(str::to_string)
    }
}

/// What reads a document: the files a `$ref` reaches, each read once, and the `$ref`s it could
/// not follow.
struct Reader<'l> {
    load: &'l dyn Fn(&str) -> Option<String>,
    docs: RefCell<HashMap<String, Option<Rc<Node>>>>,
    unresolved: RefCell<Vec<Unresolved>>,
}

impl Reader<'_> {
    /// The node behind the `$ref`s at `l`, or `l` itself; None when one cannot be followed (it is
    /// listed as unresolved).
    fn follow(&self, l: Loc) -> Option<Loc> {
        let mut l = l;
        for _ in 0..MAX_REFS {
            let Some(r) = l.node().get("$ref") else { return Some(l) };
            let written = r.as_str().unwrap_or_default().to_string();
            match self.target(&l, &written) {
                Some(t) => l = t,
                None => {
                    self.unfollowed(Unresolved { file: l.file.clone(), written, line: r.line, col: r.col });
                    return None;
                }
            }
        }
        let r = l.node().get("$ref").map(|r| (r.line, r.col)).unwrap_or((l.node().line, l.node().col));
        self.unfollowed(Unresolved { file: l.file.clone(), written: l.str("$ref").unwrap_or_default(), line: r.0, col: r.1 });
        None
    }

    /// A `$ref` not followed, listed once however often it is come to.
    fn unfollowed(&self, u: Unresolved) {
        let mut un = self.unresolved.borrow_mut();
        if !un.contains(&u) {
            un.push(u);
        }
    }

    /// Where a `$ref` written at `from` lands: the same document for `#/…`, else the file beside
    /// it, read through the loader. A URL is not read.
    fn target(&self, from: &Loc, written: &str) -> Option<Loc> {
        let (f, frag) = written.split_once('#').unwrap_or((written, ""));
        let ptr = unpercent(frag);
        let (file, doc) = if f.is_empty() {
            (from.file.clone(), from.doc.clone())
        } else {
            if f.contains("://") {
                return None;
            }
            let path = beside(&from.file, &unpercent(f));
            let doc = self.doc(&path)?;
            (path, doc)
        };
        doc.pointer(&ptr)?;
        Some(Loc { file, doc, ptr })
    }

    /// A file a `$ref` reaches, read once.
    fn doc(&self, path: &str) -> Option<Rc<Node>> {
        if let Some(d) = self.docs.borrow().get(path) {
            return d.clone();
        }
        let d = (self.load)(path).and_then(|text| yaml::read_file_text(path, &text).ok()).map(Rc::new);
        self.docs.borrow_mut().insert(path.to_string(), d.clone());
        d
    }

    /// What a schema says of a value. A schema that cannot be read says nothing.
    fn schema(&self, l: Loc) -> Schema {
        let Some(l) = self.follow(l) else { return Schema::default() };
        let n = l.node();
        let mut s = Schema { line: n.line, col: n.col, ..Schema::default() };
        match n.get("type").map(|t| &t.value) {
            Some(Value::Str(t)) => s.ty = Some(t.clone()),
            Some(Value::Seq(ts)) => {
                let names: Vec<&str> = ts.iter().filter_map(Node::as_str).collect();
                s.nullable = names.contains(&"null");
                let rest: Vec<&str> = names.into_iter().filter(|t| *t != "null").collect();
                if rest.len() == 1 {
                    s.ty = Some(rest[0].to_string());
                }
            }
            _ => {}
        }
        if n.get("nullable").is_some_and(|v| v.value == Value::Bool(true)) {
            s.nullable = true;
        }
        s.format = l.str("format");
        // the marks of a secret, read as every language reads them (`marks`)
        let sensitive = n.get("x-sensitive-data").is_some_and(|v| v.value != Value::Bool(false));
        let classification = n.get("x-data-classification").map(|c| (c.get("category").and_then(Node::as_str).unwrap_or(""), c.get("sensitivity").and_then(Node::as_str)));
        s.mark = crate::marks::schema_mark(s.format.as_deref(), sensitive, classification);
        let number = |k: &str| -> Option<Number> {
            match &n.get(k)?.value {
                Value::Int(i) => Some(Number::Int(*i)),
                Value::Float(f) => Some(Number::Decimal(f.clone())),
                _ => None,
            }
        };
        let flag = |k: &str| n.get(k).is_some_and(|v| v.value == Value::Bool(true));
        s.minimum = number("minimum").map(|value| Bound { value, exclusive: flag("exclusiveMinimum") });
        s.maximum = number("maximum").map(|value| Bound { value, exclusive: flag("exclusiveMaximum") });
        // 3.1: the exclusive ends are numbers of their own; the tighter end holds
        if let Some(x) = number("exclusiveMinimum") {
            if s.minimum.as_ref().is_none_or(|m| !below(&x, &m.value)) {
                s.minimum = Some(Bound { value: x, exclusive: true });
            }
        }
        if let Some(x) = number("exclusiveMaximum") {
            if s.maximum.as_ref().is_none_or(|m| !below(&m.value, &x)) {
                s.maximum = Some(Bound { value: x, exclusive: true });
            }
        }
        let wire = |v: &Node| -> Option<String> {
            match &v.value {
                Value::Null => None,
                Value::Str(x) => Some(x.clone()),
                _ => Some(v.to_json_text()),
            }
        };
        if let Some(xs) = n.get("enum").and_then(Node::as_seq) {
            if xs.iter().any(|x| x.value == Value::Null) {
                s.nullable = true;
            }
            s.values = Some(xs.iter().filter_map(wire).collect());
        } else if let Some(c) = n.get("const") {
            match wire(c) {
                Some(v) => s.values = Some(vec![v]),
                None => s.nullable = true,
            }
        }
        // what an `allOf` says, where the schema itself says nothing
        if let Some(all) = l.key("allOf") {
            for m in all.items() {
                let m = self.schema(m);
                s.ty = s.ty.or(m.ty);
                s.format = s.format.or(m.format);
                s.minimum = s.minimum.or(m.minimum);
                s.maximum = s.maximum.or(m.maximum);
                s.values = s.values.or(m.values);
                s.mark = s.mark.or(m.mark);
            }
        }
        s
    }

    /// The properties of an object schema (through `allOf`), each marked required when any
    /// `required` on the way lists it.
    fn fields(&self, l: Loc) -> Vec<Field> {
        let mut out = Vec::new();
        let mut required = Vec::new();
        self.gather(l, &mut out, &mut required, 0);
        for f in &mut out {
            f.required = required.contains(&f.name);
        }
        out
    }

    fn gather(&self, l: Loc, out: &mut Vec<Field>, required: &mut Vec<String>, depth: usize) {
        if depth > MAX_REFS {
            return;
        }
        let Some(l) = self.follow(l) else { return };
        if let Some(rs) = l.node().get("required").and_then(Node::as_seq) {
            required.extend(rs.iter().filter_map(Node::as_str).map(str::to_string));
        }
        if let Some(all) = l.key("allOf") {
            for m in all.items() {
                self.gather(m, out, required, depth + 1);
            }
        }
        if let Some(props) = l.key("properties") {
            for (k, p) in props.entries() {
                if !out.iter().any(|f: &Field| f.name == k.name) {
                    out.push(Field { name: k.name.clone(), required: false, schema: self.schema(p), line: k.line, col: k.col });
                }
            }
        }
    }

    /// The `security` at `l`: each requirement with the schemes it names.
    fn security(&self, l: &Loc, own: bool) -> Security {
        let n = l.node();
        let requirements = l.items().into_iter().map(|r| r.node().as_map().unwrap_or_default().iter().map(|(k, _)| k.name.clone()).collect()).collect();
        Security { requirements, own, line: n.line, col: n.col }
    }

    /// The parameters at `l` (a sequence), each with its `$ref` followed.
    fn params(&self, l: Option<Loc>) -> Vec<Param> {
        let mut out = Vec::new();
        for p in l.map(|l| l.items()).unwrap_or_default() {
            let Some(p) = self.follow(p) else { continue };
            let place = match p.str("in").as_deref() {
                Some("path") => Place::Path,
                Some("query" | "querystring") => Place::Query,
                Some("header") => Place::Header,
                Some("cookie") => Place::Cookie,
                _ => continue,
            };
            let name = p.str("name").unwrap_or_default();
            let required = place == Place::Path || p.node().get("required").is_some_and(|v| v.value == Value::Bool(true));
            // the schema, or the one of its only media type
            let schema = match p.key("schema") {
                Some(s) => self.schema(s),
                None => p.key("content").and_then(|c| c.entries().into_iter().next()).and_then(|(_, m)| m.key("schema")).map(|s| self.schema(s)).unwrap_or_default(),
            };
            let n = p.node();
            out.push(Param { name, place, required, schema, line: n.line, col: n.col });
        }
        out
    }

    /// The operations under `paths` and `webhooks`.
    fn openapi_operations(&self, top: &Loc) -> Vec<Operation> {
        let mut out = Vec::new();
        let document = top.key("security").map(|l| self.security(&l, false));
        for (where_, webhook) in [("paths", false), ("webhooks", true)] {
            let Some(paths) = top.key(where_) else { continue };
            for (pk, item) in paths.entries() {
                let Some(item) = self.follow(item) else { continue };
                let base = format!("/{where_}/{}", escape(&pk.name));
                let shared = self.params(item.key("parameters"));
                let mut ops: Vec<(String, Key, Loc, String)> = Vec::new();
                for (mk, op) in item.entries() {
                    if METHODS.contains(&mk.name.as_str()) {
                        ops.push((mk.name.to_ascii_uppercase(), mk.clone(), op, format!("{base}/{}", mk.name)));
                    }
                }
                for (mk, op) in item.key("additionalOperations").map(|l| l.entries()).unwrap_or_default() {
                    let ptr = format!("{base}/additionalOperations/{}", escape(&mk.name));
                    ops.push((mk.name.clone(), mk, op, ptr));
                }
                for (method, mk, op, pointer) in ops {
                    let Some(op) = self.follow(op) else { continue };
                    let mut params = shared.clone();
                    for p in self.params(op.key("parameters")) {
                        match params.iter_mut().find(|q| q.name == p.name && q.place == p.place) {
                            Some(q) => *q = p,
                            None => params.push(p),
                        }
                    }
                    let body = op.key("requestBody").and_then(|b| self.follow(b)).map(|b| {
                        let content = b.key("content").map(|c| c.entries()).unwrap_or_default();
                        let pick = content
                            .iter()
                            .position(|(k, _)| k.name == "application/json")
                            .or_else(|| content.iter().position(|(k, _)| k.name.ends_with("+json") || k.name.ends_with("/json")))
                            .or_else(|| content.iter().position(|(k, _)| k.name == "application/x-www-form-urlencoded" || k.name == "multipart/form-data"))
                            .or((!content.is_empty()).then_some(0));
                        let (media, schema) = match pick {
                            Some(i) => (content[i].0.name.clone(), content[i].1.key("schema")),
                            None => (String::new(), None),
                        };
                        let n = b.node();
                        Body {
                            required: n.get("required").is_some_and(|v| v.value == Value::Bool(true)),
                            media,
                            schema: schema.clone().map(|s| self.schema(s)).unwrap_or_default(),
                            fields: schema.map(|s| self.fields(s)).unwrap_or_default(),
                            line: n.line,
                            col: n.col,
                        }
                    });
                    let security = match op.key("security") {
                        Some(l) => Some(self.security(&l, true)),
                        None => document.clone(),
                    };
                    let responses = op.key("responses").map(|r| r.entries().into_iter().map(|(k, _)| (k.name.clone(), k.line)).collect()).unwrap_or_default();
                    out.push(Operation {
                        id: op.str("operationId").unwrap_or_default(),
                        method,
                        path: pk.name.clone(),
                        pointer,
                        line: mk.line,
                        col: mk.col,
                        webhook,
                        params,
                        body,
                        security,
                        responses,
                    });
                }
            }
        }
        out
    }

    /// The operations under `operations`, each on its channel, sent the payloads of its messages.
    fn asyncapi_operations(&self, top: &Loc) -> Vec<Operation> {
        let mut out = Vec::new();
        let default_media = top.str("defaultContentType").unwrap_or_default();
        for (ok, op) in top.key("operations").map(|l| l.entries()).unwrap_or_default() {
            let Some(op) = self.follow(op) else { continue };
            let channel = op.key("channel").and_then(|c| self.follow(c));
            let key = channel.as_ref().and_then(|c| c.ptr.strip_prefix("/channels/").filter(|k| !k.contains('/')).map(|k| k.replace("~1", "/").replace("~0", "~")));
            let path = channel.as_ref().and_then(|c| c.str("address")).or(key).unwrap_or_default();
            let params = channel
                .as_ref()
                .and_then(|c| c.key("parameters"))
                .map(|ps| ps.entries())
                .unwrap_or_default()
                .into_iter()
                .filter_map(|(k, p)| {
                    let p = self.follow(p)?;
                    let n = p.node();
                    let values = n.get("enum").and_then(Node::as_seq).map(|xs| xs.iter().filter_map(scalar).collect());
                    Some(Param { name: k.name.clone(), place: Place::Channel, required: true, schema: Schema { ty: Some("string".into()), values, line: n.line, col: n.col, ..Schema::default() }, line: k.line, col: k.col })
                })
                .collect();
            // the operation's messages, or every message of its channel
            let messages: Vec<Loc> = match op.key("messages") {
                Some(ms) => ms.items(),
                None => channel.as_ref().and_then(|c| c.key("messages")).map(|ms| ms.entries().into_iter().map(|(_, m)| m).collect()).unwrap_or_default(),
            };
            let mut body: Option<Body> = None;
            for m in messages {
                let Some(m) = self.follow(m) else { continue };
                let Some(payload) = m.key("payload") else { continue };
                // a Multi Format Schema Object holds the schema under `schema`
                let payload = match self.follow(payload.clone()) {
                    Some(p) if p.node().get("schemaFormat").is_some() => p.key("schema").unwrap_or(p),
                    _ => payload,
                };
                let n = m.node();
                let one = Body { required: true, media: m.str("contentType").unwrap_or_else(|| default_media.clone()), schema: self.schema(payload.clone()), fields: self.fields(payload), line: n.line, col: n.col };
                body = Some(match body {
                    None => one,
                    Some(mut b) => {
                        // a field every message carries stays required only if each requires it
                        for f in &mut b.fields {
                            f.required &= one.fields.iter().any(|g| g.name == f.name && g.required);
                        }
                        for g in one.fields {
                            if !b.fields.iter().any(|f| f.name == g.name) {
                                b.fields.push(Field { required: false, ..g });
                            }
                        }
                        b
                    }
                });
            }
            let security = op.key("security").map(|l| {
                let n = l.node();
                let requirements = l
                    .items()
                    .into_iter()
                    .map(|s| match s.str("$ref") {
                        Some(r) => vec![r.rsplit('/').next().unwrap_or(&r).replace("~1", "/").replace("~0", "~")],
                        None => vec![s.str("type").unwrap_or_default()],
                    })
                    .collect();
                Security { requirements, own: true, line: n.line, col: n.col }
            });
            out.push(Operation {
                id: ok.name.clone(),
                method: op.str("action").unwrap_or_default(),
                path,
                pointer: format!("/operations/{}", escape(&ok.name)),
                line: ok.line,
                col: ok.col,
                webhook: false,
                params,
                body,
                security,
                responses: Vec::new(),
            });
        }
        out
    }
}

/// Whether `a` is below `b`. Decimals are compared by their value as far as an `f64` holds it.
fn below(a: &Number, b: &Number) -> bool {
    let f = |n: &Number| match n {
        Number::Int(i) => *i as f64,
        Number::Decimal(s) => s.parse::<f64>().unwrap_or(f64::NAN),
    };
    match (a, b) {
        (Number::Int(x), Number::Int(y)) => x < y,
        _ => f(a) < f(b),
    }
}

impl Document {
    /// The operation `key` names: an OpenAPI `operationId`, or the method and the path
    /// (`POST /orders/{orderId}/refunds`; the method in any case); the key of an AsyncAPI
    /// operation.
    pub fn operation(&self, key: &str) -> Option<&Operation> {
        self.operations.iter().find(|o| o.answers_to(key))
    }
}

impl Operation {
    /// What a reference names it by: its `operationId` (AsyncAPI's key), or with none its method
    /// and path (`GET /orders/{orderId}`).
    pub fn name(&self) -> String {
        if self.id.is_empty() { format!("{} {}", self.method, self.path) } else { self.id.clone() }
    }

    /// Whether `key` names it: its `operationId` or key, or its method and path.
    pub fn answers_to(&self, key: &str) -> bool {
        if !self.id.is_empty() && self.id == key {
            return true;
        }
        match key.split_once(' ') {
            Some((m, p)) => m.eq_ignore_ascii_case(&self.method) && p.trim() == self.path && !self.webhook,
            None => false,
        }
    }

    /// The parameter of this name, in any place.
    pub fn param(&self, name: &str) -> Option<&Param> {
        self.params.iter().find(|p| p.name == name)
    }

    /// The field of the body of this name.
    pub fn field(&self, name: &str) -> Option<&Field> {
        self.body.as_ref().and_then(|b| b.fields.iter().find(|f| f.name == name))
    }

    /// Whether its document says anyone may call it: `security: []`, or a requirement that asks
    /// for nothing (`{}`).
    pub fn open_to_anyone(&self) -> bool {
        self.security.as_ref().is_some_and(|s| s.requirements.is_empty() || s.requirements.iter().any(|r| r.is_empty()))
    }
}
