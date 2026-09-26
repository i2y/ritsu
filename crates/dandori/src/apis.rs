//! What a task that calls an API is held to when the API's description is at hand (E016):
//! an OpenAPI document (JSON) for `http <method> <api> "<path>"`, an AWS API's Smithy model (the
//! JSON AST) for `aws <service>:<action>` when the model is used under the service's name, and a
//! `.proto` for `connect <api> "<Service>/<Method>"`. The task sends what the operation takes —
//! no parameter it does not know, every one it requires, of a type and in a range it takes —
//! and reads what the operation answers into its declared type; the errors it declares are
//! ones the operation answers with. The descriptions are read as far as the task's types go,
//! so a large one (Stripe's) costs only what is looked at.

use crate::model::*;
use crate::proto::{PType, ProtoFile};
use serde_json::{json, Map, Value};
use std::path::Path;

/// A difference, in English and in Japanese.
pub type Words = (String, String);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiKind {
    OpenApi,
    Smithy,
    Proto,
}

impl ApiKind {
    pub fn word(self) -> &'static str {
        match self {
            ApiKind::OpenApi => "openapi",
            ApiKind::Smithy => "smithy",
            ApiKind::Proto => "proto",
        }
    }
}

#[derive(Clone, Debug)]
pub enum ApiDoc {
    OpenApi(Value),
    Smithy(Value),
    Proto(ProtoFile),
}

#[derive(Clone, Debug)]
pub struct Api {
    pub name: String,
    pub doc: ApiDoc,
    /// where the API is: `url` under `use`, else an OpenAPI document's first server
    pub url: Option<String>,
}

pub fn load(kind: ApiKind, path: &Path) -> Result<ApiDoc, String> {
    match kind {
        ApiKind::Proto => crate::proto::load(path).map(ApiDoc::Proto),
        _ => {
            let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
            let v: Value = serde_json::from_str(&text).map_err(|e| format!("{} is not JSON ({e}); dandori reads an OpenAPI document or a Smithy model written as JSON", path.display()))?;
            if kind == ApiKind::OpenApi {
                if !v["openapi"].as_str().is_some_and(|s| s.starts_with('3')) {
                    return Err(format!("{} is not an OpenAPI 3 document", path.display()));
                }
                Ok(ApiDoc::OpenApi(v))
            } else {
                if !v["shapes"].is_object() {
                    return Err(format!("{} is not a Smithy model in the JSON AST", path.display()));
                }
                Ok(ApiDoc::Smithy(v))
            }
        }
    }
}

impl Api {
    /// The OpenAPI document's first server, without the slash at its end.
    pub fn base_url(&self) -> Option<String> {
        let u = match (&self.url, &self.doc) {
            (Some(u), _) => u.clone(),
            (None, ApiDoc::OpenApi(v)) => v["servers"][0]["url"].as_str()?.to_string(),
            _ => return None,
        };
        Some(u.trim_end_matches('/').to_string())
    }
}

/// A place in an API's description that a value goes to or comes from, read when it is looked at.
#[derive(Clone, Debug)]
enum Node {
    /// an OpenAPI schema
    Json(Value),
    /// a Smithy shape, by its id
    Shape(String),
    Proto(PType),
    ProtoList(PType),
    Any,
}

#[derive(Clone, Debug)]
enum Shape {
    Any,
    /// a string, one of `values` when they are given
    Str(Option<Vec<String>>),
    /// an integer; `text`: carried as a JSON string (protobuf's 64-bit integers)
    Int { text: bool, min: Option<i64>, max: Option<i64> },
    Float,
    Bool,
    Timestamp,
    Bytes,
    List(Node),
    Map(Node),
    Object(Vec<Member>),
    /// any of these (OpenAPI's anyOf and oneOf)
    Choice(Vec<Node>),
}

#[derive(Clone, Debug)]
struct Member {
    name: String,
    /// another name the API takes it by (protobuf: the field's own name beside its JSON key)
    alias: Option<String>,
    required: bool,
    /// the API may send null for it
    nullable: bool,
    /// the API may leave it out of an answer (not for a protobuf field without presence, which
    /// `connect` fills in with its zero value)
    optional_out: bool,
    node: Node,
}

/// One operation of an API: what it takes and answers, and how it fails.
pub struct Op<'a> {
    api: &'a Api,
    /// how a message names it: `POST /v1/payment_intents`, `Publish`, `StockService/Reserve`
    pub label: String,
    /// the query parameters and the path's, for OpenAPI; the body's, or the input's
    input: Node,
    query: Vec<Member>,
    output: Option<Node>,
    /// OpenAPI: the statuses it answers with ("4XX", "default" too); Smithy: its errors' names
    errors: Vec<String>,
    /// Smithy: the member that takes an idempotency token
    token: Option<String>,
    /// OpenAPI: the body is URL-encoded
    pub form: bool,
    /// protobuf: the full name of the service and the method, for the Connect path
    pub procedure: Option<String>,
    pub streams: bool,
}

/// The OpenAPI operation at `method` and `path`.
pub fn openapi_op<'a>(api: &'a Api, method: &str, path: &str) -> Result<Op<'a>, Words> {
    let ApiDoc::OpenApi(doc) = &api.doc else { unreachable!("an OpenAPI document") };
    let label = format!("{method} {path}");
    let item = &doc["paths"][path];
    let op = &item[method.to_ascii_lowercase()];
    if !op.is_object() {
        let near: Vec<String> = doc["paths"].as_object().map(|p| p.keys().filter(|k| k.split('{').next() == path.split('{').next()).cloned().collect()).unwrap_or_default();
        let (hint_en, hint_ja) = if near.is_empty() { (String::new(), String::new()) } else { (format!(" (it has {})", near.join(", ")), format!("（あるのは {}）", near.join("・"))) };
        return Err((format!("`{}` has no {label}{hint_en}", api.name), format!("`{}` に {label} はありません{hint_ja}", api.name)));
    }
    let mut query = Vec::new();
    let params = item["parameters"].as_array().into_iter().flatten().chain(op["parameters"].as_array().into_iter().flatten());
    for p in params {
        let p = resolve(doc, p);
        if matches!(p["in"].as_str(), Some("query") | Some("path")) {
            query.push(Member {
                name: p["name"].as_str().unwrap_or_default().to_string(),
                alias: None,
                required: p["required"].as_bool().unwrap_or(false),
                nullable: false,
                optional_out: true,
                node: Node::Json(p["schema"].clone()),
            });
        }
    }
    let content = &resolve(doc, &op["requestBody"])["content"];
    let (input, form) = match (content.get("application/json"), content.get("application/x-www-form-urlencoded")) {
        (Some(c), _) => (Node::Json(c["schema"].clone()), false),
        (None, Some(c)) => (Node::Json(c["schema"].clone()), true),
        _ => (Node::Any, false),
    };
    let responses = op["responses"].as_object().cloned().unwrap_or_default();
    let ok = responses.iter().find(|(s, _)| s.starts_with('2')).map(|(_, r)| resolve(doc, r));
    let output = ok.map(|r| match r["content"].get("application/json") {
        Some(c) => Node::Json(c["schema"].clone()),
        None => Node::Any,
    });
    Ok(Op { api, label, input, query, output, errors: responses.keys().cloned().collect(), token: None, form, procedure: None, streams: false })
}

/// The Smithy operation that Step Functions names `action` (`publish` for `Publish`).
pub fn smithy_op<'a>(api: &'a Api, action: &str) -> Result<Op<'a>, Words> {
    let ApiDoc::Smithy(doc) = &api.doc else { unreachable!("a Smithy model") };
    let shapes = doc["shapes"].as_object().cloned().unwrap_or_default();
    let found = shapes.iter().find(|(id, s)| s["type"] == "operation" && local(id).eq_ignore_ascii_case(action));
    let Some((id, op)) = found else {
        return Err((format!("`{}` has no operation `{action}`", api.name), format!("`{}` に操作 `{action}` はありません", api.name)));
    };
    let input = op["input"]["target"].as_str().map(|t| Node::Shape(t.to_string())).unwrap_or(Node::Any);
    let output = op["output"]["target"].as_str().map(|t| Node::Shape(t.to_string()));
    let service_errors: Vec<Value> = shapes.values().filter(|s| s["type"] == "service").flat_map(|s| s["errors"].as_array().cloned().unwrap_or_default()).collect();
    let errors = op["errors"].as_array().into_iter().flatten().chain(service_errors.iter()).filter_map(|e| e["target"].as_str()).map(|t| local(t).to_string()).collect();
    let token = op["input"]["target"].as_str().and_then(|t| shapes.get(t)).and_then(|s| s["members"].as_object()).and_then(|ms| {
        ms.iter().find(|(_, m)| m["traits"].get("smithy.api#idempotencyToken").is_some()).map(|(n, _)| n.clone())
    });
    Ok(Op { api, label: local(id).to_string(), input, query: vec![], output, errors, token, form: false, procedure: None, streams: false })
}

/// The method of a `.proto`'s service, written `Service/Method`.
pub fn proto_op<'a>(api: &'a Api, method: &str) -> Result<Op<'a>, Words> {
    let ApiDoc::Proto(pf) = &api.doc else { unreachable!("a .proto") };
    let not = || (format!("`{}` has no method `{method}`; write it as `Service/Method`", api.name), format!("`{}` にメソッド `{method}` はありません。`Service/Method` と書きます", api.name));
    let (svc, m) = method.split_once('/').ok_or_else(not)?;
    let service = pf.services.iter().find(|s| s.name == svc || s.name.rsplit('.').next() == Some(svc)).ok_or_else(not)?;
    let found = service.methods.iter().find(|x| x.name == m).ok_or_else(not)?;
    Ok(Op {
        api,
        label: format!("{}/{}", service.name.rsplit('.').next().unwrap_or(&service.name), found.name),
        input: Node::Proto(PType::Named(found.input.clone())),
        query: vec![],
        output: Some(Node::Proto(PType::Named(found.output.clone()))),
        errors: vec![],
        token: None,
        form: false,
        procedure: Some(format!("{}/{}", service.name, found.name)),
        streams: found.streams,
    })
}

fn local(id: &str) -> &str {
    id.rsplit('#').next().unwrap_or(id)
}

/// An OpenAPI object behind its `$ref`, if it has one (a reference within the document).
fn resolve(doc: &Value, v: &Value) -> Value {
    let mut v = v.clone();
    for _ in 0..16 {
        let Some(r) = v.get("$ref").and_then(|r| r.as_str()) else { return v };
        let Some(pointer) = r.strip_prefix('#') else { return Value::Null };
        v = doc.pointer(pointer).cloned().unwrap_or(Value::Null);
    }
    v
}

impl<'a> Op<'a> {
    fn shape(&self, n: &Node) -> Shape {
        match (n, &self.api.doc) {
            (Node::Any, _) => Shape::Any,
            (Node::Json(s), ApiDoc::OpenApi(doc)) => openapi_shape(doc, s),
            (Node::Shape(id), ApiDoc::Smithy(doc)) => smithy_shape(doc, id),
            (Node::Proto(t), ApiDoc::Proto(pf)) => proto_shape(pf, t),
            (Node::ProtoList(t), ApiDoc::Proto(_)) => Shape::List(Node::Proto(t.clone())),
            _ => Shape::Any,
        }
    }

    /// What the operation says of the task (E016): each difference, in both languages.
    pub fn check(&self, m: &Model, task: &TaskDef, path_params: &[String]) -> Vec<Words> {
        let mut out = Vec::new();
        let api = &self.api.name;
        let op = &self.label;
        if self.streams {
            out.push((format!("`{api}` {op} streams; `connect` calls a method that takes one message and answers one"), format!("`{api}` の {op} はストリームです。`connect` で呼べるのは、一つ受け取って一つ答えるメソッドです")));
            return out;
        }
        // what goes: the path's and the query's parameters, and the body's
        let body_members = match self.shape(&self.input) {
            Shape::Object(ms) => Some(ms),
            _ => None,
        };
        let get = matches!(task.binding, Some(Binding::Http { ref method, .. }) if method == "GET" || method == "DELETE");
        for (p, pt) in &task.params {
            if task.callback && p == "MessageBody" && self.label == "SendMessage" {
                // the token goes in the message, which Step Functions writes out as JSON text
                if !matches!(pt.inner(), Ty::Record(_) | Ty::Json | Ty::Str) {
                    out.push(self.param_words(p, &(format!("`{}` is not a string", m.ty_name(pt)), format!("`{}` は文字列ではありません", m.ty_name(pt)))));
                }
                continue;
            }
            let in_query = path_params.contains(p) || get;
            let found = if in_query { self.query.iter().find(|q| q.name == *p).cloned() } else { body_members.as_ref().and_then(|ms| ms.iter().find(|x| x.name == *p || x.alias.as_deref() == Some(p)).cloned()) };
            let Some(mb) = found else {
                if !in_query && body_members.is_none() && matches!(self.input, Node::Any) {
                    continue;
                }
                out.push((format!("`{api}` {op} takes no `{p}`"), format!("`{api}` の {op} は `{p}` を受け取りません")));
                continue;
            };
            if mb.required && matches!(pt, Ty::Opt(_)) {
                out.push((format!("`{api}` {op} needs `{p}`, and the parameter may be absent"), format!("`{api}` の {op} には `{p}` が要りますが、引数は無いことがあります")));
                continue;
            }
            if let Err(w) = self.sends(m, pt, task.param_ranges.get(p).copied(), &mb.node, &mut vec![]) {
                out.push(self.param_words(p, &w));
            }
        }
        let givens: Vec<&String> = task.params.iter().map(|(p, _)| p).chain(task.key_param.iter()).collect();
        let wanted: Vec<Member> = self.query.iter().filter(|q| q.required).cloned().chain(body_members.iter().flatten().filter(|x| x.required).cloned()).collect();
        for w in wanted {
            if !givens.iter().any(|g| **g == w.name || w.alias.as_deref() == Some(g.as_str())) {
                out.push((format!("`{api}` {op} needs `{}`, which `{}` does not pass", w.name, task.name), format!("`{api}` の {op} には `{}` が要りますが、`{}` はそれを渡しません", w.name, task.name)));
            }
        }
        // what comes back: a callback's answer comes from whoever answers it, not from the API
        if let (Some(t), Some(o), false) = (&task.result, &self.output, task.callback) {
            // every field of the answer's record that differs, and one difference below it
            let found = match (t, self.shape(o)) {
                (Ty::Record(r), Shape::Object(ms)) => self.fields(&ms, m, *r, &mut vec![]),
                _ => self.reads(o, m, t, task.result_range, &mut vec![]).err().into_iter().collect(),
            };
            for (en, ja) in found {
                out.push((format!("what `{api}` {op} answers is not `{}`: {en}", m.ty_name(t)), format!("`{api}` の {op} の答えは `{}` に合いません。{ja}", m.ty_name(t))));
            }
        }
        // the errors it answers with, and the idempotency token
        for e in &task.errors {
            match &self.api.doc {
                ApiDoc::OpenApi(_) => {
                    if let Some(st) = e.status {
                        let s = st.to_string();
                        let class = format!("{}XX", &s[..1]);
                        if !self.errors.iter().any(|x| *x == s || x.eq_ignore_ascii_case(&class) || x == "default") {
                            out.push((format!("`{api}` {op} does not answer with {st} (`{}`)", e.name), format!("`{api}` の {op} は {st} で答えません（`{}`）", e.name)));
                        }
                    }
                }
                ApiDoc::Smithy(_) => {
                    let x = e.exception.as_deref().unwrap_or(&e.name);
                    if !self.errors.iter().any(|y| y == x) {
                        out.push((format!("`{api}` {op} has no error `{x}` (its errors are {})", self.errors.join(", ")), format!("`{api}` の {op} にエラー `{x}` はありません（エラーは {}）", self.errors.join("・"))));
                    }
                }
                ApiDoc::Proto(_) => {}
            }
        }
        if let (ApiDoc::Smithy(_), Some(kp)) = (&self.api.doc, &task.key_param) {
            if self.token.as_deref() != Some(kp.as_str()) {
                let (en, ja) = match &self.token {
                    Some(t) => (format!("it is `{t}`"), format!("それは `{t}` です")),
                    None => ("it takes none".to_string(), "この操作は冪等トークンを受け取りません".to_string()),
                };
                out.push((format!("`{kp}` is not the idempotency token of `{api}` {op}: {en}"), format!("`{kp}` は `{api}` の {op} の冪等トークンではありません。{ja}")));
            }
        }
        out
    }

    fn param_words(&self, p: &str, (en, ja): &Words) -> Words {
        (format!("the parameter `{p}` is not what `{}` {} takes: {en}", self.api.name, self.label), format!("引数 `{p}` は、`{}` の {} が受け取るものと合いません。{ja}", self.api.name, self.label))
    }

    /// Whether every value of `t` (its numbers in `rg`) is one the API takes at `n`.
    fn sends(&self, m: &Model, t: &Ty, rg: Option<Range>, n: &Node, seen: &mut Vec<RecordId>) -> Result<(), Words> {
        let s = self.shape(n);
        let differ = || (format!("`{}` is not {}", m.ty_name(t), describe(&s)), format!("`{}` は{}ではありません", m.ty_name(t), describe_ja(&s)));
        match (t, &s) {
            (_, Shape::Any) => Ok(()),
            // the flow sends null for an absent value, which the APIs take as not given
            (Ty::Opt(x), _) => self.sends(m, x, rg, n, seen),
            (_, Shape::Choice(ns)) => {
                if ns.iter().any(|x| self.sends(m, t, rg, x, &mut seen.clone()).is_ok()) {
                    Ok(())
                } else {
                    Err(differ())
                }
            }
            (Ty::Json, _) => Ok(()),
            (Ty::Str, Shape::Str(None)) | (Ty::Str, Shape::Bytes) => Ok(()),
            (Ty::Str, Shape::Str(Some(vs))) => Err((format!("`{}` takes one of {}, and `string` can be anything; give it an enum", m.ty_name(t), vs.join(", ")), format!("受け取るのは {} のどれかで、`string` は何でもありえます。列挙にしてください", vs.join("・")))),
            (Ty::Enum(e), Shape::Str(vs)) => match (vs, m.enums[*e].values.iter().find(|v| vs.as_ref().is_some_and(|vs| !vs.contains(v)))) {
                (_, Some(v)) => Err((format!("`{v}` of `{}` is not one of the values it takes ({})", m.enums[*e].name, vs.as_ref().unwrap().join(", ")), format!("`{}` の `{v}` は、受け取る値（{}）にありません", m.enums[*e].name, vs.as_ref().unwrap().join("・")))),
                _ => Ok(()),
            },
            (Ty::Int | Ty::Num(_), Shape::Int { min, max, .. }) => within(rg, *min, *max),
            (Ty::Int | Ty::Num(_), Shape::Float) => Ok(()),
            (Ty::Bool, Shape::Bool) => Ok(()),
            (Ty::Timestamp, Shape::Timestamp) | (Ty::Timestamp, Shape::Str(None)) => Ok(()),
            (Ty::List(x), Shape::List(y)) => self.sends(m, x, rg, y, seen),
            (Ty::Record(r), Shape::Map(y)) => {
                for (f, ft) in &m.records[*r].fields {
                    self.sends(m, ft, m.field_range(*r, f), y, seen).map_err(|w| field_words(f, w))?;
                }
                Ok(())
            }
            (Ty::Record(r), Shape::Object(ms)) => {
                if seen.contains(r) {
                    return Ok(());
                }
                seen.push(*r);
                let rd = &m.records[*r];
                for (f, ft) in &rd.fields {
                    let Some(mb) = ms.iter().find(|x| x.name == *f || x.alias.as_deref() == Some(f)) else {
                        return Err((format!("it takes no `{f}` (a field of `{}`)", rd.name), format!("`{f}`（`{}` のフィールド）は受け取りません", rd.name)));
                    };
                    self.sends(m, ft, rd.ranges.get(f).copied(), &mb.node, seen).map_err(|w| field_words(f, w))?;
                }
                for mb in ms.iter().filter(|x| x.required) {
                    match rd.fields.iter().find(|(f, _)| *f == mb.name || mb.alias.as_deref() == Some(f)) {
                        Some((_, Ty::Opt(_))) | None => return Err((format!("it needs `{}`, which `{}` may not have", mb.name, rd.name), format!("`{}` が要りますが、`{}` には無いことがあります", mb.name, rd.name))),
                        _ => {}
                    }
                }
                seen.pop();
                Ok(())
            }
            _ => Err(differ()),
        }
    }

    /// Whether every value the API answers at `n` is one of `t` (its numbers in `rg`).
    fn reads(&self, n: &Node, m: &Model, t: &Ty, rg: Option<Range>, seen: &mut Vec<RecordId>) -> Result<(), Words> {
        let s = self.shape(n);
        let differ = || (format!("{} is not `{}`", describe(&s), m.ty_name(t)), format!("{}は `{}` ではありません", describe_ja(&s), m.ty_name(t)));
        match (&s, t) {
            (_, Ty::Json) => Ok(()),
            (_, Ty::Opt(x)) => self.reads(n, m, x, rg, seen),
            (Shape::Choice(ns), _) => {
                for x in ns {
                    self.reads(x, m, t, rg, &mut seen.clone())?;
                }
                Ok(())
            }
            (Shape::Any, _) => Err((format!("it can be anything, which `{}` is not; declare it `json`", m.ty_name(t)), format!("何でもありえますが、`{}` はそうではありません。`json` にしてください", m.ty_name(t)))),
            (Shape::Str(_), Ty::Str) | (Shape::Bytes, Ty::Str) | (Shape::Int { text: true, .. }, Ty::Str) => Ok(()),
            (Shape::Str(Some(vs)), Ty::Enum(e)) => {
                let zero = self.proto_zero(n);
                match vs.iter().find(|v| !m.enums[*e].values.contains(v) && Some(v.as_str()) != zero.as_deref()) {
                    Some(v) => Err((format!("it may be `{v}`, which `{}` has no value for", m.enums[*e].name), format!("`{v}` のことがありますが、`{}` にその値はありません", m.enums[*e].name))),
                    None => Ok(()),
                }
            }
            (Shape::Str(None), Ty::Enum(e)) => Err((format!("it is any string, and `{}` takes only its values; declare it `string`", m.enums[*e].name), format!("どんな文字列でもありえますが、`{}` は自分の値しか取りません。`string` にしてください", m.enums[*e].name))),
            (Shape::Int { text: true, .. }, Ty::Int | Ty::Num(_)) => Err((
                "a 64-bit integer comes as a string in protobuf's JSON; declare it `string`".into(),
                "64 ビットの整数は、protobuf の JSON では文字列で来ます。`string` にしてください".into(),
            )),
            (Shape::Int { min, max, .. }, Ty::Int | Ty::Num(_)) => match rg {
                Some(want) if (min.is_some() || max.is_some()) && !(Range { lo: *min, hi: *max }).within(&want) => Err((
                    format!("it can be `{}`, outside `{}`", Range { lo: *min, hi: *max }.show(), want.show()),
                    format!("`{}` の外の `{}` になりえます", want.show(), Range { lo: *min, hi: *max }.show()),
                )),
                _ => Ok(()),
            },
            (Shape::Float, Ty::Int | Ty::Num(_)) => Err(("it may have a fraction, which dandori has no type for; declare it `json`".into(), "小数のことがあり、dandori にはその型がありません。`json` にしてください".into())),
            (Shape::Bool, Ty::Bool) | (Shape::Timestamp, Ty::Timestamp) => Ok(()),
            (Shape::List(x), Ty::List(y)) => self.reads(x, m, y, rg, seen),
            (Shape::Object(ms), Ty::Record(r)) => match self.fields(ms, m, *r, seen).into_iter().next() {
                Some(w) => Err(w),
                None => Ok(()),
            },
            (Shape::Map(_), Ty::Record(_)) => Err(differ()),
            _ => Err(differ()),
        }
    }

    /// What differs between the members an answer's object has and the fields of the record
    /// `r` it is read into: one difference for each field.
    fn fields(&self, ms: &[Member], m: &Model, r: RecordId, seen: &mut Vec<RecordId>) -> Vec<Words> {
        if seen.contains(&r) {
            return vec![];
        }
        seen.push(r);
        let rd = &m.records[r];
        let mut out = Vec::new();
        for (f, ft) in &rd.fields {
            let Some(mb) = ms.iter().find(|x| x.name == *f) else {
                let other = ms.iter().find(|x| x.alias.as_deref() == Some(f));
                out.push(match other {
                    Some(o) => (format!("the answer names `{f}` `{}`", o.name), format!("答えでは `{f}` は `{}` という名前です", o.name)),
                    None => (format!("the answer has no `{f}` (a field of `{}`)", rd.name), format!("答えに `{f}`（`{}` のフィールド）はありません", rd.name)),
                });
                continue;
            };
            if (mb.optional_out || mb.nullable) && !matches!(ft, Ty::Opt(_) | Ty::Json) {
                let (en, ja) = if mb.nullable { ("may be null", "は null のことがあります") } else { ("may be left out", "は無いことがあります") };
                out.push((format!("`{f}` {en}; declare it `{}?`", m.ty_name(ft)), format!("`{f}` {ja}。`{}?` にしてください", m.ty_name(ft))));
                continue;
            }
            if let Err(w) = self.reads(&mb.node, m, ft, rd.ranges.get(f).copied(), seen) {
                out.push(field_words(f, w));
            }
        }
        seen.pop();
        out
    }

    /// The zero value of a protobuf enum at `n`: what a field without presence reads as when it
    /// is not set, which a flow's enum need not have (the answer's check refuses it, if it comes).
    fn proto_zero(&self, n: &Node) -> Option<String> {
        match (n, &self.api.doc) {
            (Node::Proto(PType::Named(e)), ApiDoc::Proto(pf)) => pf.enums.get(e).and_then(|vs| vs.first().cloned()),
            _ => None,
        }
    }

    /// For `connect`: the zero values of the answer's fields that protobuf leaves out of JSON,
    /// and the same for the messages inside it: `{"f": {key: zero}, "m": {key: …}, "l": {key: …}}`.
    pub fn zeros(&self) -> Option<Value> {
        let (ApiDoc::Proto(pf), Some(Node::Proto(PType::Named(msg)))) = (&self.api.doc, &self.output) else { return None };
        Some(zeros_of(pf, msg, &mut vec![]))
    }
}

fn zeros_of(pf: &ProtoFile, msg: &str, within: &mut Vec<String>) -> Value {
    let mut f = Map::new();
    let mut ms = Map::new();
    let mut ls = Map::new();
    if within.iter().any(|x| x == msg) {
        return json!({});
    }
    within.push(msg.to_string());
    for fl in pf.messages.get(msg).into_iter().flatten() {
        let message = match &fl.ty {
            PType::Named(n) if pf.messages.contains_key(n) => Some(n.clone()),
            _ => None,
        };
        if fl.repeated {
            f.insert(fl.json.clone(), json!([]));
            if let Some(n) = &message {
                let sub = zeros_of(pf, n, within);
                if sub.as_object().is_some_and(|o| !o.is_empty()) {
                    ls.insert(fl.json.clone(), sub);
                }
            }
        } else if let PType::Map(..) = fl.ty {
            f.insert(fl.json.clone(), json!({}));
        } else if let Some(n) = &message {
            let sub = zeros_of(pf, n, within);
            if sub.as_object().is_some_and(|o| !o.is_empty()) {
                ms.insert(fl.json.clone(), sub);
            }
        } else if !fl.presence {
            f.insert(fl.json.clone(), zero(pf, &fl.ty));
        }
    }
    within.pop();
    let mut out = Map::new();
    if !f.is_empty() {
        out.insert("f".into(), Value::Object(f));
    }
    if !ms.is_empty() {
        out.insert("m".into(), Value::Object(ms));
    }
    if !ls.is_empty() {
        out.insert("l".into(), Value::Object(ls));
    }
    Value::Object(out)
}

/// A protobuf type's zero value, as its JSON has it.
fn zero(pf: &ProtoFile, t: &PType) -> Value {
    match t {
        PType::Scalar(s) => match s.as_str() {
            "string" | "bytes" => json!(""),
            "bool" => json!(false),
            "int64" | "uint64" | "sint64" | "fixed64" | "sfixed64" => json!("0"),
            _ => json!(0),
        },
        PType::Named(n) => pf.enums.get(n).and_then(|vs| vs.first()).map(|v| json!(v)).unwrap_or(Value::Null),
        PType::Map(..) => json!({}),
    }
}

/// Fill in the zero values `zeros` names (`Op::zeros`), as protobuf reads a message whose JSON
/// leaves them out: in the answer, in its messages, and in the messages of its lists.
pub fn fill(v: &Value, zeros: &Value) -> Value {
    let Some(o) = v.as_object() else { return v.clone() };
    let mut out = o.clone();
    for (k, z) in zeros["f"].as_object().into_iter().flatten() {
        if out.get(k).is_none_or(|x| x.is_null()) {
            out.insert(k.clone(), z.clone());
        }
    }
    for (k, sub) in zeros["m"].as_object().into_iter().flatten() {
        if let Some(x) = out.get(k).filter(|x| x.is_object()).cloned() {
            out.insert(k.clone(), fill(&x, sub));
        }
    }
    for (k, sub) in zeros["l"].as_object().into_iter().flatten() {
        if let Some(a) = out.get(k).and_then(|x| x.as_array()).cloned() {
            out.insert(k.clone(), Value::Array(a.iter().map(|x| fill(x, sub)).collect()));
        }
    }
    Value::Object(out)
}

/// The answer as a Connect server sends it when every field that `zeros` names is at its zero
/// value: those fields left out of the JSON, in the answer, in its messages and in the messages
/// of its lists. The lists and the maps stay, so that there are messages inside to leave fields out of.
pub fn sparse(v: &Value, zeros: &Value) -> Value {
    let Some(o) = v.as_object() else { return v.clone() };
    let mut out = o.clone();
    for (k, z) in zeros["f"].as_object().into_iter().flatten() {
        if !z.is_array() && !z.is_object() {
            out.remove(k);
        }
    }
    for (k, sub) in zeros["m"].as_object().into_iter().flatten() {
        if let Some(x) = out.get(k).filter(|x| x.is_object()).cloned() {
            out.insert(k.clone(), sparse(&x, sub));
        }
    }
    for (k, sub) in zeros["l"].as_object().into_iter().flatten() {
        if let Some(a) = out.get(k).and_then(|x| x.as_array()).cloned() {
            out.insert(k.clone(), Value::Array(a.iter().map(|x| sparse(x, sub)).collect()));
        }
    }
    Value::Object(out)
}

fn field_words(f: &str, (en, ja): Words) -> Words {
    (format!("in `{f}`, {en}"), format!("`{f}` で、{ja}"))
}

fn within(rg: Option<Range>, min: Option<i64>, max: Option<i64>) -> Result<(), Words> {
    if min.is_none() && max.is_none() {
        return Ok(());
    }
    let want = Range { lo: min, hi: max };
    match rg {
        Some(h) if h.within(&want) => Ok(()),
        Some(h) => Err((format!("it is `{}`, and `{}` is taken", h.show(), want.show()), format!("`{}` で、受け取るのは `{}` です", h.show(), want.show()))),
        None => Err((format!("it has no range, and `{}` is taken", want.show()), format!("範囲が書かれていませんが、受け取るのは `{}` です", want.show()))),
    }
}

fn describe(s: &Shape) -> String {
    match s {
        Shape::Any => "anything".into(),
        Shape::Str(Some(vs)) => format!("one of {}", vs.join(", ")),
        Shape::Str(None) => "a string".into(),
        Shape::Int { text: true, .. } => "a 64-bit integer, as a string".into(),
        Shape::Int { .. } => "an integer".into(),
        Shape::Float => "a number".into(),
        Shape::Bool => "true or false".into(),
        Shape::Timestamp => "a date and time".into(),
        Shape::Bytes => "bytes".into(),
        Shape::List(_) => "a list".into(),
        Shape::Map(_) => "a map".into(),
        Shape::Object(_) => "an object".into(),
        Shape::Choice(_) => "one of several shapes".into(),
    }
}

fn describe_ja(s: &Shape) -> String {
    match s {
        Shape::Any => "何でもよい値".into(),
        Shape::Str(Some(vs)) => format!("{} のどれか", vs.join("・")),
        Shape::Str(None) => "文字列".into(),
        Shape::Int { text: true, .. } => "文字列で運ぶ 64 ビットの整数".into(),
        Shape::Int { .. } => "整数".into(),
        Shape::Float => "数".into(),
        Shape::Bool => "真偽".into(),
        Shape::Timestamp => "日時".into(),
        Shape::Bytes => "バイト列".into(),
        Shape::List(_) => "リスト".into(),
        Shape::Map(_) => "マップ".into(),
        Shape::Object(_) => "オブジェクト".into(),
        Shape::Choice(_) => "いくつかの形のどれか".into(),
    }
}

fn openapi_shape(doc: &Value, schema: &Value) -> Shape {
    let s = resolve(doc, schema);
    // Stripe's expandable fields: an id unless the request asks to expand them
    if let (Some(any), true) = (s["anyOf"].as_array(), s.get("x-expansionResources").is_some()) {
        if let Some(id) = any.iter().find(|x| x["type"] == "string") {
            return openapi_shape(doc, id);
        }
    }
    if let Some(any) = s["anyOf"].as_array().or(s["oneOf"].as_array()) {
        let branches: Vec<Node> = any.iter().filter(|x| resolve(doc, x)["type"] != "null").map(|x| Node::Json(x.clone())).collect();
        return if branches.len() == 1 { openapi_shape(doc, match &branches[0] { Node::Json(v) => v, _ => unreachable!() }) } else { Shape::Choice(branches) };
    }
    if let Some(all) = s["allOf"].as_array() {
        if all.len() == 1 {
            return openapi_shape(doc, &all[0]);
        }
        let mut members = Vec::new();
        for x in all {
            if let Shape::Object(ms) = openapi_shape(doc, x) {
                members.extend(ms);
            }
        }
        return Shape::Object(members);
    }
    let ty = match &s["type"] {
        Value::Array(ts) => ts.iter().filter_map(|t| t.as_str()).find(|t| *t != "null").unwrap_or("").to_string(),
        t => t.as_str().unwrap_or("").to_string(),
    };
    match ty.as_str() {
        "string" if s["format"] == "date-time" => Shape::Timestamp,
        "string" => Shape::Str(s["enum"].as_array().map(|vs| vs.iter().filter_map(|v| v.as_str().map(String::from)).collect())),
        "integer" => Shape::Int { text: false, min: s["minimum"].as_i64(), max: s["maximum"].as_i64() },
        "number" => Shape::Float,
        "boolean" => Shape::Bool,
        "array" => Shape::List(Node::Json(s["items"].clone())),
        "object" | "" if s["properties"].is_object() => {
            let required: Vec<&str> = s["required"].as_array().map(|r| r.iter().filter_map(|x| x.as_str()).collect()).unwrap_or_default();
            let props = s["properties"].as_object().cloned().unwrap_or_default();
            Shape::Object(
                props
                    .iter()
                    .map(|(k, v)| {
                        let pv = resolve(doc, v);
                        let nullable = pv["nullable"] == true || pv["type"].as_array().is_some_and(|ts| ts.iter().any(|t| t == "null")) || pv["anyOf"].as_array().is_some_and(|a| a.iter().any(|x| x["type"] == "null"));
                        Member { name: k.clone(), alias: None, required: required.contains(&k.as_str()), nullable, optional_out: !required.contains(&k.as_str()), node: Node::Json(v.clone()) }
                    })
                    .collect(),
            )
        }
        "object" => match &s["additionalProperties"] {
            Value::Object(_) => Shape::Map(Node::Json(s["additionalProperties"].clone())),
            _ => Shape::Map(Node::Any),
        },
        _ => Shape::Any,
    }
}

fn smithy_shape(doc: &Value, id: &str) -> Shape {
    let prelude = |name: &str| -> Option<Shape> {
        Some(match name {
            "String" => Shape::Str(None),
            "Integer" | "PrimitiveInteger" | "Long" | "PrimitiveLong" | "Short" | "PrimitiveShort" | "Byte" | "PrimitiveByte" | "BigInteger" => Shape::Int { text: false, min: None, max: None },
            "Float" | "PrimitiveFloat" | "Double" | "PrimitiveDouble" | "BigDecimal" => Shape::Float,
            "Boolean" | "PrimitiveBoolean" => Shape::Bool,
            "Timestamp" => Shape::Timestamp,
            "Blob" => Shape::Bytes,
            "Document" => Shape::Any,
            _ => return None,
        })
    };
    if let Some(name) = id.strip_prefix("smithy.api#") {
        return prelude(name).unwrap_or(Shape::Any);
    }
    let s = &doc["shapes"][id];
    let range = |s: &Value| (s["traits"]["smithy.api#range"]["min"].as_i64(), s["traits"]["smithy.api#range"]["max"].as_i64());
    match s["type"].as_str().unwrap_or("") {
        "string" => Shape::Str(s["traits"]["smithy.api#enum"].as_array().map(|vs| vs.iter().filter_map(|v| v["value"].as_str().map(String::from)).collect())),
        "enum" => Shape::Str(Some(
            s["members"].as_object().map(|ms| ms.iter().map(|(n, m)| m["traits"]["smithy.api#enumValue"].as_str().unwrap_or(n).to_string()).collect()).unwrap_or_default(),
        )),
        "integer" | "long" | "short" | "byte" | "bigInteger" | "intEnum" => {
            let (min, max) = range(s);
            Shape::Int { text: false, min, max }
        }
        "float" | "double" | "bigDecimal" => Shape::Float,
        "boolean" => Shape::Bool,
        "timestamp" => Shape::Timestamp,
        "blob" => Shape::Bytes,
        "document" => Shape::Any,
        "list" | "set" => Shape::List(Node::Shape(s["member"]["target"].as_str().unwrap_or_default().to_string())),
        "map" => Shape::Map(Node::Shape(s["value"]["target"].as_str().unwrap_or_default().to_string())),
        "structure" | "union" => {
            let union = s["type"] == "union";
            Shape::Object(
                s["members"]
                    .as_object()
                    .map(|ms| {
                        ms.iter()
                            .map(|(n, mb)| {
                                let required = !union && mb["traits"].get("smithy.api#required").is_some();
                                // AWS models rarely say an answer's member is always there; the answer's check tells
                                Member { name: n.clone(), alias: None, required, nullable: false, optional_out: false, node: Node::Shape(mb["target"].as_str().unwrap_or_default().to_string()) }
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            )
        }
        _ => Shape::Any,
    }
}

fn proto_shape(pf: &ProtoFile, t: &PType) -> Shape {
    match t {
        PType::Scalar(s) => match s.as_str() {
            "string" => Shape::Str(None),
            "bytes" => Shape::Bytes,
            "bool" => Shape::Bool,
            "double" | "float" => Shape::Float,
            "int64" | "uint64" | "sint64" | "fixed64" | "sfixed64" => Shape::Int { text: true, min: None, max: None },
            _ => Shape::Int { text: false, min: None, max: None },
        },
        PType::Map(_, v) => Shape::Map(Node::Proto((**v).clone())),
        PType::Named(n) => match n.as_str() {
            "google.protobuf.Timestamp" => Shape::Timestamp,
            "google.protobuf.Duration" | "google.protobuf.FieldMask" | "google.protobuf.StringValue" => Shape::Str(None),
            "google.protobuf.Struct" | "google.protobuf.Value" | "google.protobuf.Any" => Shape::Any,
            "google.protobuf.ListValue" => Shape::List(Node::Any),
            "google.protobuf.Empty" => Shape::Object(vec![]),
            "google.protobuf.DoubleValue" | "google.protobuf.FloatValue" => Shape::Float,
            "google.protobuf.Int64Value" | "google.protobuf.UInt64Value" => Shape::Int { text: true, min: None, max: None },
            "google.protobuf.Int32Value" | "google.protobuf.UInt32Value" => Shape::Int { text: false, min: None, max: None },
            "google.protobuf.BoolValue" => Shape::Bool,
            "google.protobuf.BytesValue" => Shape::Bytes,
            _ => {
                if let Some(vs) = pf.enums.get(n) {
                    return Shape::Str(Some(vs.clone()));
                }
                Shape::Object(
                    pf.messages
                        .get(n)
                        .into_iter()
                        .flatten()
                        .map(|fl| Member {
                            name: fl.json.clone(),
                            alias: (fl.name != fl.json).then(|| fl.name.clone()),
                            required: false,
                            nullable: false,
                            optional_out: fl.presence,
                            node: if fl.repeated { Node::ProtoList(fl.ty.clone()) } else { Node::Proto(fl.ty.clone()) },
                        })
                        .collect(),
                )
            }
        },
    }
}

/// The HTTP status a Connect error code comes back with (connectrpc.com/docs/protocol, "Error Codes").
pub fn connect_status(code: &str) -> Option<u16> {
    Some(match code {
        "canceled" => 499,
        "unknown" | "internal" | "data_loss" => 500,
        "invalid_argument" | "failed_precondition" | "out_of_range" => 400,
        "deadline_exceeded" => 504,
        "not_found" => 404,
        "already_exists" | "aborted" => 409,
        "permission_denied" => 403,
        "resource_exhausted" => 429,
        "unimplemented" => 501,
        "unavailable" => 503,
        "unauthenticated" => 401,
        _ => return None,
    })
}
