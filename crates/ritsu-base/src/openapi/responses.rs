//! What the operations of an OpenAPI document answer with, whole: each response's status, the
//! headers it must send, its media types and the schema of each, `$ref`s followed and loops kept,
//! for a language that holds what a service answered to the document (geas's `contract`). A
//! [`Document`](super::Document) reads what an operation is sent, as far as its arguments are held
//! to something; this is what comes back.

use super::{Bound, Kind, Loc, METHODS, Number, Problem, Reader, Unresolved, bounds, kind_of, number_at};
use crate::yaml::{self, Node, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// The keywords of a schema that say something of a value and are not read: an answer is not held
/// to them, and [`Responses::unheld`] says where they are written. `format` is not one of them,
/// since a document may use a format of its own, and what it says is for the language to read.
const NOT_READ: [&str; 13] = [
    "patternProperties",
    "propertyNames",
    "dependentRequired",
    "dependentSchemas",
    "dependencies",
    "if",
    "then",
    "else",
    "unevaluatedProperties",
    "unevaluatedItems",
    "contains",
    "minContains",
    "maxContains",
];

/// What a document says its operations answer with.
#[derive(Clone, Debug, PartialEq)]
pub struct Responses {
    /// Every operation under `paths`, in the order written.
    pub operations: Vec<Responding>,
    /// Every schema a response reaches. A [`Shape`] names another by its place here, so that a
    /// schema that holds itself is one entry.
    pub shapes: Vec<Shape>,
    /// The `$ref`s that could not be followed. What sits behind one is not read.
    pub unresolved: Vec<Unresolved>,
    /// The keywords no answer is held to, each where it is written.
    pub unheld: Vec<Unheld>,
}

/// A keyword of a schema that is not read, and where it is.
#[derive(Clone, Debug, PartialEq)]
pub struct Unheld {
    pub keyword: String,
    pub file: String,
    pub line: usize,
    pub col: usize,
}

/// One operation, as far as what it answers goes.
#[derive(Clone, Debug, PartialEq)]
pub struct Responding {
    /// In capitals (`GET`; a key of 3.2's `additionalOperations` as written).
    pub method: String,
    /// As `paths` writes it (`/orders/{orderId}`).
    pub path: String,
    /// The paths the `servers` that apply put in front of it (`/v1` of `https://example.com/v1`),
    /// the operation's own over its path item's over the document's. Only those that add something.
    pub bases: Vec<String>,
    pub responses: Vec<Response>,
}

/// One response of an operation.
#[derive(Clone, Debug, PartialEq)]
pub struct Response {
    /// As written: `201`, `4XX`, `default`.
    pub status: String,
    /// The headers it marks `required`, as written. `Content-Type` is not one (OpenAPI ignores it
    /// there).
    pub required_headers: Vec<String>,
    /// Its media types as written (`application/json`, `text/*`), each with its schema, a place in
    /// [`Responses::shapes`].
    pub media: Vec<(String, Option<usize>)>,
}

/// What a schema says of a value, in full. Every list says nothing when it is empty, and every
/// `Option` when it is None.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shape {
    /// `false` written as a schema: no value fits it.
    pub never: bool,
    /// `type`: one name, or 3.1's list. `null` among them stands for the null value.
    pub types: Vec<String>,
    /// 3.0's `nullable: true`.
    pub nullable: bool,
    /// `enum`, or a `const` as one value, each as JSON's text.
    pub values: Option<Vec<String>>,
    pub format: Option<String>,
    pub minimum: Option<Bound>,
    pub maximum: Option<Bound>,
    pub multiple_of: Option<Number>,
    pub min_length: Option<u64>,
    pub max_length: Option<u64>,
    pub pattern: Option<String>,
    pub items: Option<usize>,
    pub prefix_items: Vec<usize>,
    pub min_items: Option<u64>,
    pub max_items: Option<u64>,
    pub unique_items: bool,
    pub required: Vec<String>,
    pub properties: Vec<(String, usize)>,
    pub additional: Option<usize>,
    pub min_properties: Option<u64>,
    pub max_properties: Option<u64>,
    /// `allOf`, and what a `$ref` lands on.
    pub all_of: Vec<usize>,
    pub any_of: Vec<usize>,
    pub one_of: Vec<usize>,
    pub not: Option<usize>,
    /// `writeOnly: true`: the value is sent to the service, not by it.
    pub write_only: bool,
}

/// What a text holds as [`Responses`]: JSON when `file` ends in `.json`, else YAML, a `$ref` to
/// another file followed through `load`, as [`read_with`](super::read_with) follows it.
pub fn responses(file: &str, text: &str, load: &dyn Fn(&str) -> Option<String>) -> Result<Responses, Problem> {
    let root = yaml::read_file_text(file, text).map_err(|e| Problem { line: e.line, col: e.col, message: e.message })?;
    let (kind, _) = kind_of(&root)?;
    if kind != Kind::OpenApi {
        return Err(Problem {
            line: root.line,
            col: root.col,
            message: crate::tr!("AsyncAPI の文書には、サービスが何を答えるかは書かれていません", "an AsyncAPI document does not say what a service answers"),
        });
    }
    let reader = Reader { load, docs: RefCell::new(HashMap::new()), unresolved: RefCell::new(Vec::new()) };
    let top = Loc { file: file.to_string(), doc: Rc::new(root), ptr: String::new() };
    let shapes = Shapes { reader: &reader, shapes: RefCell::new(Vec::new()), seen: RefCell::new(HashMap::new()), unheld: RefCell::new(Vec::new()) };
    let mut operations = Vec::new();
    let of_document = bases(&top).unwrap_or_default();
    for (key, item) in top.key("paths").map(|l| l.entries()).unwrap_or_default() {
        if !key.name.starts_with('/') {
            continue;
        }
        let Some(item) = reader.follow(item) else { continue };
        let of_path = bases(&item);
        let mut ops: Vec<(String, Loc)> = item.entries().into_iter().filter(|(k, _)| METHODS.contains(&k.name.as_str())).map(|(k, op)| (k.name.to_ascii_uppercase(), op)).collect();
        ops.extend(item.key("additionalOperations").map(|l| l.entries()).unwrap_or_default().into_iter().map(|(k, op)| (k.name.clone(), op)));
        for (method, op) in ops {
            let Some(op) = reader.follow(op) else { continue };
            let bases = bases(&op).or_else(|| of_path.clone()).unwrap_or_else(|| of_document.clone());
            let responses = op.key("responses").map(|r| r.entries()).unwrap_or_default().into_iter().filter(|(k, _)| !k.name.starts_with("x-")).filter_map(|(k, r)| shapes.response(&k.name, r)).collect();
            operations.push(Responding { method, path: key.name.clone(), bases, responses });
        }
    }
    let (all, unheld) = (shapes.shapes.into_inner(), shapes.unheld.into_inner());
    Ok(Responses { operations, shapes: all, unresolved: reader.unresolved.into_inner(), unheld })
}

/// What the `servers` at `l` put in front of a path; None when it has no `servers`.
fn bases(l: &Loc) -> Option<Vec<String>> {
    let servers = l.key("servers")?;
    let mut out: Vec<String> = Vec::new();
    for s in servers.items() {
        let base = base_of(&s.str("url").unwrap_or_default());
        if !base.is_empty() && !out.contains(&base) {
            out.push(base);
        }
    }
    Some(out)
}

/// The path of a server's URL, without its end's `/`: `/v1` of `https://example.com/v1/` and of
/// `./v1`, nothing of `http://localhost:8080`.
fn base_of(url: &str) -> String {
    let path = match url.split_once("://") {
        Some((_, rest)) => rest.find('/').map_or("", |i| &rest[i..]),
        None => url,
    };
    let path = path.trim_start_matches("./").trim_end_matches('/');
    match path {
        "" | "." => String::new(),
        p if p.starts_with('/') => p.to_string(),
        p => format!("/{p}"),
    }
}

/// What reads the schemas of the responses: each once, however many places reach it.
struct Shapes<'r, 'l> {
    reader: &'r Reader<'l>,
    shapes: RefCell<Vec<Shape>>,
    seen: RefCell<HashMap<(String, String), usize>>,
    unheld: RefCell<Vec<Unheld>>,
}

impl Shapes<'_, '_> {
    fn response(&self, status: &str, r: Loc) -> Option<Response> {
        let r = self.reader.follow(r)?;
        let mut required_headers = Vec::new();
        for (name, header) in r.key("headers").map(|h| h.entries()).unwrap_or_default() {
            let required = self.reader.follow(header).is_some_and(|h| h.node().get("required").is_some_and(|v| v.value == Value::Bool(true)));
            if required && !name.name.eq_ignore_ascii_case("content-type") {
                required_headers.push(name.name.clone());
            }
        }
        let media = r.key("content").map(|c| c.entries()).unwrap_or_default().into_iter().map(|(k, m)| (k.name.clone(), self.reader.follow(m).and_then(|m| m.key("schema")).map(|s| self.of(s)))).collect();
        Some(Response { status: status.to_string(), required_headers, media })
    }

    /// The place of the schema at `l`.
    fn of(&self, l: Loc) -> usize {
        let key = (l.file.clone(), l.ptr.clone());
        if let Some(&i) = self.seen.borrow().get(&key) {
            return i;
        }
        let i = {
            let mut shapes = self.shapes.borrow_mut();
            shapes.push(Shape::default());
            shapes.len() - 1
        };
        self.seen.borrow_mut().insert(key, i);
        let shape = self.read(&l);
        self.shapes.borrow_mut()[i] = shape;
        i
    }

    fn read(&self, l: &Loc) -> Shape {
        let n = l.node();
        let mut s = Shape::default();
        if n.value == Value::Bool(false) {
            s.never = true;
            return s;
        }
        if n.as_map().is_none() {
            return s;
        }
        // a `$ref` is one more thing the value has to fit: 3.1 reads what is written beside it too
        if let Some(r) = n.get("$ref") {
            let written = r.as_str().unwrap_or_default().to_string();
            match self.reader.target(l, &written) {
                Some(t) => s.all_of.push(self.of(t)),
                None => self.reader.unfollowed(Unresolved { file: l.file.clone(), written, line: r.line, col: r.col }),
            }
        }
        match n.get("type").map(|t| &t.value) {
            Some(Value::Str(t)) => s.types = vec![t.clone()],
            Some(Value::Seq(ts)) => s.types = ts.iter().filter_map(Node::as_str).map(str::to_string).collect(),
            _ => {}
        }
        let flag = |k: &str| n.get(k).is_some_and(|v| v.value == Value::Bool(true));
        let count = |k: &str| match n.get(k).map(|v| &v.value) {
            Some(Value::Int(i)) => u64::try_from(*i).ok(),
            _ => None,
        };
        s.nullable = flag("nullable");
        s.write_only = flag("writeOnly");
        s.unique_items = flag("uniqueItems");
        s.values = n.get("enum").and_then(Node::as_seq).map(|xs| xs.iter().map(Node::to_json_text).collect());
        if let Some(c) = n.get("const") {
            s.values = Some(vec![c.to_json_text()]);
        }
        s.format = l.str("format");
        (s.minimum, s.maximum) = bounds(n);
        s.multiple_of = number_at(n, "multipleOf");
        s.min_length = count("minLength");
        s.max_length = count("maxLength");
        s.pattern = l.str("pattern");
        s.min_items = count("minItems");
        s.max_items = count("maxItems");
        s.min_properties = count("minProperties");
        s.max_properties = count("maxProperties");
        s.required = n.get("required").and_then(Node::as_seq).map(|xs| xs.iter().filter_map(Node::as_str).map(str::to_string).collect()).unwrap_or_default();
        s.items = l.key("items").map(|x| self.of(x));
        s.additional = l.key("additionalProperties").map(|x| self.of(x));
        s.not = l.key("not").map(|x| self.of(x));
        s.properties = l.key("properties").map(|p| p.entries()).unwrap_or_default().into_iter().map(|(k, v)| (k.name.clone(), self.of(v))).collect();
        let list = |k: &str| -> Vec<usize> { l.key(k).map(|x| x.items()).unwrap_or_default().into_iter().map(|x| self.of(x)).collect() };
        s.prefix_items = list("prefixItems");
        s.all_of.extend(list("allOf"));
        s.any_of = list("anyOf");
        s.one_of = list("oneOf");
        for keyword in NOT_READ {
            if let Some((k, _)) = n.entry(keyword) {
                self.unheld.borrow_mut().push(Unheld { keyword: keyword.to_string(), file: l.file.clone(), line: k.line, col: k.col });
            }
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_server_puts_its_path_in_front() {
        for (url, base) in [("http://localhost:8080", ""), ("https://example.com/", ""), ("https://example.com/v1/", "/v1"), ("{scheme}://example.com/{version}", "/{version}"), ("/v1", "/v1"), ("./v1", "/v1"), ("v1", "/v1"), (".", ""), ("", "")] {
            assert_eq!(base_of(url), base, "{url}");
        }
    }
}
