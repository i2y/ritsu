//! The elements of OpenAPI and AsyncAPI documents as references (DESIGN 6.2): where a reference
//! of `openapi` or `asyncapi` lands in a document, the reference of what a JSON Pointer names, and
//! the elements of a kind. The YAML and the JSON are read by [`crate::yaml`]; a `$ref` is a file and
//! a JSON Pointer, followed over files as sakai follows it (at most [`MAX_STEPS`] steps), so that
//! yuen and sakai find the same element for the same reference.
//!
//! What a kind is in a document:
//!
//! - `schema S`: `components/schemas/S`. `property P` under it is a property of its definition
//!   (its `properties`, else one of a schema of its `allOf`, in order); `value V` under it, a value
//!   of its `enum` (a string as it is, a number or a bool as JSON writes it, `null`).
//! - OpenAPI's `operation O`: the operation under `paths` or `webhooks` whose `operationId` is `O`,
//!   or, for one with none, whose method and path are `O` (`"POST /orders/{orderId}/refunds"`).
//! - AsyncAPI's `channel C` (`channels/C`) and `message M` under it (`channels/C/messages/M`),
//!   `message M` (`components/messages/M`), and `operation O` (`operations/O`).
//! - `pointer P`: what the JSON Pointer `P` names from the top of the file, for what has no kind
//!   of its own (a response, a server, a schema of a file that is a part of a document).

use crate::naming::{Name, Tool};
use crate::paths;
use crate::yaml::{self, Node, Value};

/// The documents a lookup reads: each file's top node, by its path from the root; None for a
/// file that is not there or does not read.
pub trait Documents {
    fn root(&self, file: &str) -> Option<&Node>;
}

/// Following `$ref`s takes at most this many steps, so that two documents that point at each other
/// cannot keep a lookup going.
pub const MAX_STEPS: usize = 64;

/// A key as a token of a JSON Pointer.
pub fn escape(key: &str) -> String {
    key.replace('~', "~0").replace('/', "~1")
}

/// A `$ref`'s fragment as a JSON Pointer: `%XX` read as the byte it stands for.
pub fn fragment(f: &str) -> String {
    let bytes = f.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(b) = u8::from_str_radix(&f[i + 1..i + 3], 16)
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Whether a `$ref` is a URL (it starts with a scheme: `https:`, `urn:`).
fn is_url(uri: &str) -> bool {
    let mut cs = uri.chars();
    cs.next().is_some_and(|c| c.is_ascii_alphabetic()) && uri.split_once(':').is_some_and(|(s, _)| s.len() > 1 && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-')))
}

/// Where a `$ref` written in `from` lands: the file (from the root) and the JSON Pointer; Err with
/// the URL when it is one; Err(None) when the path leaves the root.
pub fn target(from: &str, written: &str) -> Result<(String, String), Option<String>> {
    let (uri, frag) = written.split_once('#').unwrap_or((written, ""));
    if is_url(uri) {
        return Err(Some(uri.to_string()));
    }
    let file = if uri.is_empty() { from.to_string() } else { paths::join(&paths::parent(from), uri).map_err(|_| None)? };
    Ok((file, fragment(frag)))
}

/// The keys under which a `$ref` is data, not a reference: an example (`example`, and an Example
/// Object's value) and an extension (`x-…`). A schema's `enum`, `const` and `default` hold data
/// too, but `default` is also the default response of OpenAPI, so they are read: a `$ref` in data
/// is a mapping with the key `$ref`, which their values hardly hold.
fn is_data(key: &str) -> bool {
    key == "example" || key.starts_with("x-")
}

/// The keys whose values are mappings of names (a property's, a response's, a channel's), not of
/// the keywords of the specifications: their keys are never data.
fn holds_names(key: &str) -> bool {
    matches!(
        key,
        "properties" | "patternProperties" | "$defs" | "definitions" | "dependentSchemas" | "schemas" | "responses" | "parameters" | "examples" | "requestBodies" | "headers" | "securitySchemes" | "links" | "callbacks" | "pathItems" | "paths" | "webhooks" | "channels" | "operations" | "messages" | "messageTraits" | "operationTraits" | "replies" | "replyAddresses" | "servers" | "serverVariables" | "variables" | "correlationIds" | "externalDocs" | "tags" | "mapping"
    )
}

/// Whether a mapping is a schema or a Reference Object itself, not a mapping of names: AsyncAPI's
/// message `headers` is a schema, where OpenAPI's `headers` names the headers.
fn schema_like(v: &Node) -> bool {
    v.as_map().is_some_and(|es| es.iter().any(|(k, _)| matches!(k.name.as_str(), "$ref" | "type" | "properties" | "allOf" | "oneOf" | "anyOf" | "items" | "enum" | "format" | "$schema")))
}

/// Every `$ref` of a node, with the JSON Pointer of the mapping it is in (`at` is the node's), and
/// the line and column of its key.
pub fn refs_in(node: &Node, at: &str, out: &mut Vec<(String, usize, usize, String)>) {
    refs_under(node, at, false, out);
}

/// `names`: the keys of this mapping are names (under `properties`, `responses` and the like).
fn refs_under(node: &Node, at: &str, names: bool, out: &mut Vec<(String, usize, usize, String)>) {
    match &node.value {
        Value::Map(es) => {
            for (k, v) in es {
                if !names
                    && k.name == "$ref"
                    && let Value::Str(s) = &v.value
                {
                    out.push((at.to_string(), k.line, k.col, s.clone()));
                    continue;
                }
                if !names && is_data(&k.name) {
                    continue;
                }
                // an example object's value is data
                if !names && k.name == "value" && es.iter().any(|(k2, _)| matches!(k2.name.as_str(), "summary" | "externalValue" | "dataValue" | "serializedValue")) {
                    continue;
                }
                let child_names = !names && holds_names(&k.name) && !(k.name == "headers" && schema_like(v));
                refs_under(v, &format!("{at}/{}", escape(&k.name)), child_names, out);
            }
        }
        Value::Seq(xs) => {
            for (i, x) in xs.iter().enumerate() {
                refs_under(x, &format!("{at}/{i}"), false, out);
            }
        }
        _ => {}
    }
}

/// The node a file and a JSON Pointer name, following the `$ref`s on the way (a channel that is a
/// `$ref` to another file, then a message under it), and where it is: the file and the pointer it
/// is at in the end.
pub fn locate<'a, D: Documents + ?Sized>(d: &'a D, file: &str, pointer: &str) -> Option<(String, String, &'a Node)> {
    locate_in(d, file, pointer, 0)
}

fn locate_in<'a, D: Documents + ?Sized>(d: &'a D, file: &str, pointer: &str, steps: usize) -> Option<(String, String, &'a Node)> {
    if steps > MAX_STEPS {
        return None;
    }
    let mut at: &Node = d.root(file)?;
    let mut cur_file = file.to_string();
    let mut cur_ptr = String::new();
    for token in yaml::pointer_tokens(pointer)? {
        // a `$ref` on the way: go on from where it lands
        if let Some(Value::Str(w)) = at.get("$ref").map(|n| &n.value) {
            let (f, p) = target(&cur_file, w).ok()?;
            let (f2, p2, n2) = locate_in(d, &f, &p, steps + 1)?;
            at = n2;
            cur_file = f2;
            cur_ptr = p2;
        }
        at = match &at.value {
            Value::Map(es) => &es.iter().find(|(k, _)| k.name == token)?.1,
            Value::Seq(xs) => xs.get(token.parse::<usize>().ok()?)?,
            _ => return None,
        };
        cur_ptr = format!("{cur_ptr}/{}", escape(&token));
    }
    Some((cur_file, cur_ptr, at))
}

/// Where the `$ref`s of a node lead in the end: the definition it stands for.
pub fn definition<'a, D: Documents + ?Sized>(d: &'a D, file: &str, pointer: &str, node: &'a Node) -> (String, String, &'a Node) {
    let (mut f, mut p, mut n) = (file.to_string(), pointer.to_string(), node);
    for _ in 0..MAX_STEPS {
        let Some(Value::Str(w)) = n.get("$ref").map(|x| &x.value) else { break };
        let Ok((tf, tp)) = target(&f, w) else { break };
        let Some((f2, p2, n2)) = locate(d, &tf, &tp) else { break };
        (f, p, n) = (f2, p2, n2);
    }
    (f, p, n)
}

/// Every element a file and a pointer reach: themselves, what their `$ref`s land on, and on from
/// there (a channel's messages and their payloads), as (file, pointer).
pub fn reach<D: Documents + ?Sized>(d: &D, file: &str, pointer: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut todo = vec![(file.to_string(), pointer.to_string())];
    while let Some((f, p)) = todo.pop() {
        if out.contains(&(f.clone(), p.clone())) || out.len() > 10_000 {
            continue;
        }
        out.push((f.clone(), p.clone()));
        let Some((lf, lp, node)) = locate(d, &f, &p) else { continue };
        if (lf.clone(), lp.clone()) != (f.clone(), p.clone()) {
            todo.push((lf.clone(), lp.clone()));
        }
        let mut rs = Vec::new();
        refs_in(node, &lp, &mut rs);
        for (_, _, _, w) in rs {
            if let Ok(t) = target(&lf, &w) {
                todo.push(t);
            }
        }
    }
    out
}

/// An operation of an OpenAPI document.
#[derive(Clone, Debug, PartialEq)]
pub struct Operation {
    /// Its `operationId`; empty when it has none.
    pub id: String,
    /// In capitals (`POST`).
    pub method: String,
    /// The key under `paths` (or the webhook's name).
    pub path: String,
    /// From the top of the file it is named in, through the path item.
    pub pointer: String,
    /// The path item's pointer: its `parameters` and `servers` are the operation's too.
    pub item: String,
    pub line: usize,
    pub webhook: bool,
}

impl Operation {
    /// The name of its references: its `operationId`, or with none its method and path
    /// (`"GET /orders/{id}"`).
    pub fn name(&self) -> String {
        if self.id.is_empty() { format!("{} {}", self.method, self.path) } else { self.id.clone() }
    }
}

/// The operations of an OpenAPI document: under `paths` and `webhooks`, each method of a path item
/// (and the `additionalOperations` of OpenAPI 3.2), in the order written.
pub fn operations<D: Documents + ?Sized>(d: &D, file: &str) -> Vec<Operation> {
    const METHODS: [&str; 9] = ["get", "put", "post", "delete", "options", "head", "patch", "trace", "query"];
    let mut out = Vec::new();
    let Some(top) = d.root(file) else { return out };
    for kind in ["paths", "webhooks"] {
        for (pk, item) in top.get(kind).and_then(Node::as_map).unwrap_or_default() {
            let base = format!("/{kind}/{}", escape(&pk.name));
            let (_, _, item) = definition(d, file, &base, item);
            let mut add = |method: &str, node: &Node, ptr: String| {
                let id = node.get("operationId").and_then(Node::as_str).unwrap_or("").to_string();
                out.push(Operation { id, method: method.to_ascii_uppercase(), path: pk.name.clone(), pointer: ptr, item: base.clone(), line: node.line, webhook: kind == "webhooks" });
            };
            for (mk, op) in item.as_map().unwrap_or_default() {
                if METHODS.contains(&mk.name.as_str()) {
                    add(&mk.name, op, format!("{base}/{}", mk.name));
                }
            }
            for (mk, op) in item.get("additionalOperations").and_then(Node::as_map).unwrap_or_default() {
                add(&mk.name, op, format!("{base}/additionalOperations/{}", escape(&mk.name)));
            }
        }
    }
    out
}

/// A value of an `enum` as its references write it: a string as it is, `null`, a number or a bool
/// as JSON writes it.
pub fn value_name(x: &Node) -> String {
    match &x.value {
        Value::Str(s) => s.clone(),
        Value::Null => "null".to_string(),
        _ => x.to_json_text(),
    }
}

/// What a reference of `openapi` or `asyncapi` with pairs names in a document: the file and the
/// JSON Pointer to read it at (from the top of that file; [`locate`] follows the `$ref`s on the
/// way). A property is where its schema's definition has it (in a schema of an `allOf`, perhaps
/// in another file); a value is the `enum` it is in.
#[derive(Clone, Debug, PartialEq)]
pub struct Element {
    pub file: String,
    pub pointer: String,
    /// `value V`: the value, as its references write it.
    pub value: Option<String>,
    /// `property P`: whether a schema it is in requires it.
    pub required: Option<bool>,
}

/// The definition of a schema of `components/schemas`, through its `$ref`s.
fn schema_def<'a, D: Documents + ?Sized>(d: &'a D, file: &str, name: &str) -> Option<(String, String, &'a Node)> {
    let (f, p, node) = locate(d, file, &format!("/components/schemas/{}", escape(name)))?;
    Some(definition(d, &f, &p, node))
}

/// A property of a schema's definition: its own `properties`, then those of the schemas of its
/// `allOf`, in order; with whether the schema it is in, or one that holds that one in `allOf`,
/// requires it.
fn property_in<D: Documents + ?Sized>(d: &D, file: &str, pointer: &str, def: &Node, name: &str, required: bool, depth: usize) -> Option<(String, String, bool)> {
    if depth > MAX_STEPS {
        return None;
    }
    let required = required || def.get("required").and_then(Node::as_seq).is_some_and(|xs| xs.iter().any(|x| x.as_str() == Some(name)));
    if def.get("properties").and_then(|ps| ps.get(name)).is_some() {
        return Some((file.to_string(), format!("{pointer}/properties/{}", escape(name)), required));
    }
    for (i, part) in def.get("allOf").and_then(Node::as_seq).unwrap_or_default().iter().enumerate() {
        let (f, p, n) = definition(d, file, &format!("{pointer}/allOf/{i}"), part);
        if let Some(found) = property_in(d, &f, &p, n, name, required, depth + 1) {
            return Some(found);
        }
    }
    None
}

/// The names of the properties of a schema's definition, its own first, then those of its
/// `allOf`, each once.
fn property_names<D: Documents + ?Sized>(d: &D, file: &str, pointer: &str, def: &Node, depth: usize, out: &mut Vec<String>) {
    if depth > MAX_STEPS {
        return;
    }
    for (k, _) in def.get("properties").and_then(Node::as_map).unwrap_or_default() {
        if !out.contains(&k.name) {
            out.push(k.name.clone());
        }
    }
    for (i, part) in def.get("allOf").and_then(Node::as_seq).unwrap_or_default().iter().enumerate() {
        let (f, p, n) = definition(d, file, &format!("{pointer}/allOf/{i}"), part);
        property_names(d, &f, &p, n, depth + 1, out);
    }
}

/// Where a reference of `openapi` or `asyncapi` lands; None when the file does not hold what it
/// names, and for a whole file (which is its bytes).
pub fn find<D: Documents + ?Sized>(d: &D, n: &Name) -> Option<Element> {
    let file = n.path.as_str();
    d.root(file)?;
    let pairs: Vec<(&str, &str)> = n.items.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let at = |pointer: String| -> Option<Element> {
        locate(d, file, &pointer)?;
        Some(Element { file: file.to_string(), pointer, value: None, required: None })
    };
    match (n.tool, pairs.as_slice()) {
        (Tool::Openapi | Tool::Asyncapi, [("schema", s)]) => at(format!("/components/schemas/{}", escape(s))),
        (Tool::Openapi | Tool::Asyncapi, [("schema", s), ("property", p)]) => {
            let (df, dp, def) = schema_def(d, file, s)?;
            let (pf, pp, required) = property_in(d, &df, &dp, def, p, false, 0)?;
            Some(Element { file: pf, pointer: pp, value: None, required: Some(required) })
        }
        (Tool::Openapi | Tool::Asyncapi, [("schema", s), ("value", v)]) => {
            let (df, dp, def) = schema_def(d, file, s)?;
            def.get("enum")?.as_seq()?.iter().find(|x| value_name(x) == *v)?;
            Some(Element { file: df, pointer: format!("{dp}/enum"), value: Some(v.to_string()), required: None })
        }
        (Tool::Openapi, [("operation", o)]) => operations(d, file).into_iter().find(|op| op.name() == *o).map(|op| Element { file: file.to_string(), pointer: op.pointer, value: None, required: None }),
        (Tool::Asyncapi, [("operation", o)]) => at(format!("/operations/{}", escape(o))),
        (Tool::Asyncapi, [("channel", c)]) => at(format!("/channels/{}", escape(c))),
        (Tool::Asyncapi, [("channel", c), ("message", m)]) => at(format!("/channels/{}/messages/{}", escape(c), escape(m))),
        (Tool::Asyncapi, [("message", m)]) => at(format!("/components/messages/{}", escape(m))),
        (Tool::Openapi | Tool::Asyncapi, [("pointer", p)]) if p.starts_with('/') => at(p.to_string()),
        _ => None,
    }
}

/// The reference of what a file and a JSON Pointer name in a document of `tool` (`openapi` or
/// `asyncapi`): the element of a kind whose place it is, else `pointer`; the file itself for the
/// empty pointer.
pub fn reference<D: Documents + ?Sized>(d: &D, tool: Tool, file: &str, pointer: &str) -> Name {
    let n = Name::file(tool, file.to_string());
    let Some(tokens) = yaml::pointer_tokens(pointer) else { return n.with("pointer", pointer) };
    let t: Vec<&str> = tokens.iter().map(String::as_str).collect();
    match (tool, t.as_slice()) {
        (_, []) => n,
        (Tool::Openapi | Tool::Asyncapi, ["components", "schemas", s]) => n.with("schema", *s),
        (Tool::Openapi | Tool::Asyncapi, ["components", "schemas", s, "properties", p]) => n.with("schema", *s).with("property", *p),
        (Tool::Asyncapi, ["channels", c]) => n.with("channel", *c),
        (Tool::Asyncapi, ["channels", c, "messages", m]) => n.with("channel", *c).with("message", *m),
        (Tool::Asyncapi, ["components", "messages", m]) => n.with("message", *m),
        (Tool::Asyncapi, ["operations", o]) => n.with("operation", *o),
        (Tool::Openapi, _) => match operations(d, file).into_iter().find(|op| op.pointer == pointer) {
            Some(op) => n.with("operation", op.name()),
            None => n.with("pointer", pointer),
        },
        _ => n.with("pointer", pointer),
    }
}

/// Every element of `kind` in the file `n` names, under its pairs (`schema Order` for `property`),
/// as references, in the order written.
pub fn gather<D: Documents + ?Sized>(d: &D, n: &Name, kind: &str) -> Vec<Name> {
    let file = Name::file(n.tool, n.path.clone());
    let Some(top) = d.root(&n.path) else { return vec![] };
    let keys = |pointer: &str| -> Vec<String> {
        let _ = top;
        locate(d, &n.path, pointer).map(|(f, p, node)| definition(d, &f, &p, node).2).and_then(Node::as_map).unwrap_or_default().iter().map(|(k, _)| k.name.clone()).collect()
    };
    let pairs: Vec<(&str, &str)> = n.items.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    match (n.tool, kind, pairs.as_slice()) {
        (Tool::Openapi | Tool::Asyncapi, "schema", []) => keys("/components/schemas").into_iter().map(|s| file.clone().with("schema", s)).collect(),
        (Tool::Openapi | Tool::Asyncapi, "property", [("schema", s)]) => {
            let Some((df, dp, def)) = schema_def(d, &n.path, s) else { return vec![] };
            let mut names = Vec::new();
            property_names(d, &df, &dp, def, 0, &mut names);
            names.into_iter().map(|p| file.clone().with("schema", *s).with("property", p)).collect()
        }
        (Tool::Openapi | Tool::Asyncapi, "value", [("schema", s)]) => {
            let Some((_, _, def)) = schema_def(d, &n.path, s) else { return vec![] };
            def.get("enum").and_then(Node::as_seq).unwrap_or_default().iter().map(|x| file.clone().with("schema", *s).with("value", value_name(x))).collect()
        }
        (Tool::Openapi, "operation", []) => operations(d, &n.path).into_iter().map(|op| file.clone().with("operation", op.name())).collect(),
        (Tool::Asyncapi, "operation", []) => keys("/operations").into_iter().map(|o| file.clone().with("operation", o)).collect(),
        (Tool::Asyncapi, "channel", []) => keys("/channels").into_iter().map(|c| file.clone().with("channel", c)).collect(),
        (Tool::Asyncapi, "message", [("channel", c)]) => keys(&format!("/channels/{}/messages", escape(c))).into_iter().map(|m| file.clone().with("channel", *c).with("message", m)).collect(),
        (Tool::Asyncapi, "message", []) => keys("/components/messages").into_iter().map(|m| file.clone().with("message", m)).collect(),
        _ => vec![],
    }
}

/// What the top mapping of a document says it is: `openapi` (or Swagger's `swagger`), `asyncapi`;
/// None for anything else (a part of a document, a file of settings).
pub fn kind_of(top: &Node) -> Option<Tool> {
    if top.get("openapi").is_some() || top.get("swagger").is_some() {
        Some(Tool::Openapi)
    } else if top.get("asyncapi").is_some() {
        Some(Tool::Asyncapi)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    struct Docs(BTreeMap<String, Node>);

    impl Documents for Docs {
        fn root(&self, file: &str) -> Option<&Node> {
            self.0.get(file)
        }
    }

    fn docs(files: &[(&str, &str)]) -> Docs {
        Docs(files.iter().map(|(f, t)| (f.to_string(), yaml::read_file_text(f, t).unwrap())).collect())
    }

    #[test]
    fn where_a_ref_lands() {
        assert_eq!(target("a/b.yaml", "#/components/schemas/X"), Ok(("a/b.yaml".into(), "/components/schemas/X".into())));
        assert_eq!(target("a/b.yaml", "../c.yaml#/channels/x"), Ok(("c.yaml".into(), "/channels/x".into())));
        assert_eq!(target("a/b.yaml", "https://example.com/x.yaml#/a"), Err(Some("https://example.com/x.yaml".into())));
        assert_eq!(target("a/b.yaml", "c.yaml#/paths/~1orders%7Bid%7D"), Ok(("a/c.yaml".into(), "/paths/~1orders{id}".into())));
    }

    #[test]
    fn a_reference_and_its_place_go_both_ways() {
        let d = docs(&[
            (
                "api.yaml",
                "openapi: 3.1.0\npaths:\n  /orders/{id}:\n    post:\n      operationId: refund\n    get: {}\ncomponents:\n  schemas:\n    Order:\n      allOf:\n        - $ref: 'common.yaml#/Base'\n        - properties:\n            status: {$ref: '#/components/schemas/Status'}\n          required: [status]\n    Status:\n      enum: [paid, 2, null]\n",
            ),
            ("common.yaml", "Base:\n  required: [id]\n  properties:\n    id: {type: string}\n"),
            ("events.yaml", "asyncapi: 3.0.0\nchannels:\n  placed:\n    messages:\n      placed: {$ref: '#/components/messages/Placed'}\noperations:\n  send: {action: send}\ncomponents:\n  messages:\n    Placed: {}\n"),
        ]);
        let api = Name::file(Tool::Openapi, "api.yaml");
        for (n, ptr) in [
            (api.clone().with("operation", "refund"), "/paths/~1orders~1{id}/post"),
            (api.clone().with("operation", "GET /orders/{id}"), "/paths/~1orders~1{id}/get"),
            (api.clone().with("schema", "Order"), "/components/schemas/Order"),
            (api.clone().with("pointer", "/paths"), "/paths"),
        ] {
            assert_eq!(find(&d, &n).unwrap().pointer, ptr);
            assert_eq!(reference(&d, Tool::Openapi, "api.yaml", ptr), n);
        }
        assert!(find(&d, &api.clone().with("operation", "POST /orders/{id}")).is_none(), "an operation with an operationId is named by it");
        // a property of a schema of `allOf`, required by the schema it is in, or by one around it
        let id = find(&d, &api.clone().with("schema", "Order").with("property", "id")).unwrap();
        assert_eq!((id.file.as_str(), id.pointer.as_str(), id.required), ("common.yaml", "/Base/properties/id", Some(true)));
        assert_eq!(find(&d, &api.clone().with("schema", "Order").with("property", "status")).unwrap().required, Some(true));
        assert_eq!(gather(&d, &api.clone().with("schema", "Order"), "property"), [api.clone().with("schema", "Order").with("property", "id"), api.clone().with("schema", "Order").with("property", "status")]);
        assert_eq!(gather(&d, &api.clone().with("schema", "Status"), "value").iter().map(|n| n.items[1].1.clone()).collect::<Vec<_>>(), ["paid", "2", "null"]);
        assert_eq!(find(&d, &api.clone().with("schema", "Status").with("value", "2")).unwrap().pointer, "/components/schemas/Status/enum");
        assert!(find(&d, &api.clone().with("schema", "Status").with("value", "unpaid")).is_none());
        let ev = Name::file(Tool::Asyncapi, "events.yaml");
        for (n, ptr) in [
            (ev.clone().with("channel", "placed"), "/channels/placed"),
            (ev.clone().with("channel", "placed").with("message", "placed"), "/channels/placed/messages/placed"),
            (ev.clone().with("message", "Placed"), "/components/messages/Placed"),
            (ev.clone().with("operation", "send"), "/operations/send"),
        ] {
            assert_eq!(find(&d, &n).unwrap().pointer, ptr);
            assert_eq!(reference(&d, Tool::Asyncapi, "events.yaml", ptr), n);
        }
        // what has no kind is named by its pointer; a part of a document too
        assert_eq!(reference(&d, Tool::Openapi, "common.yaml", "/Base").text(), "openapi \"common.yaml\" pointer /Base");
        assert_eq!(reference(&d, Tool::Asyncapi, "events.yaml", "/components/schemas/X/items").text(), "asyncapi \"events.yaml\" pointer /components/schemas/X/items");
        assert_eq!(kind_of(d.root("events.yaml").unwrap()), Some(Tool::Asyncapi));
        assert_eq!(kind_of(d.root("common.yaml").unwrap()), None);
    }
}
