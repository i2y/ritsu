//! What a task that calls an API is held to when the API's description is at hand (E016):
//! an OpenAPI document (JSON) for `http <method> <api> "<path>"`, an AWS API's Smithy model (the
//! JSON AST) for `aws <service>:<action>` when the model is used under the service's name, and a
//! `.proto` for `connect <api> "<Service>/<Method>"`. The task sends what the operation takes —
//! no parameter it does not know, every one it requires, of a type and in a range it takes —
//! and reads what the operation answers into its declared type; the errors it declares are
//! ones the operation answers with. The descriptions are read as far as the task's types go,
//! so a large one (Stripe's) costs only what is looked at.

use crate::model::*;
use crate::proto::{PField, PType, ProtoFile};
use serde_json::{json, Map, Value};
use std::path::Path;
use std::sync::Arc;

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
    /// shared, since types made from it are looked up while the flow's own are being made
    Proto(Arc<ProtoFile>),
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
        ApiKind::Proto => crate::proto::load(path).map(|pf| ApiDoc::Proto(Arc::new(pf))),
        _ => {
            let text = crate::sources::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
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
    /// a protobuf type, with what the field it is the type of is held to
    Proto(PType, FieldRules),
    /// the type of a `repeated` field, which a list of it holds
    ProtoList(PType, FieldRules),
    Any,
}

/// What a protobuf field is held to by Protovalidate (`(buf.validate.field)`), as far as dandori
/// reads it, which is as far as rulec does in its check of a contract (its DESIGN 15.132): the
/// range of a whole number, and whether the field must be set. The other rules (the length of a
/// string, the count of a list, CEL) are read as not there, which only makes a field look wider.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FieldRules {
    /// the whole numbers the field lets through
    pub range: Option<Range>,
    /// the same for each item of a `repeated` field (`repeated.items`)
    pub items: Option<Range>,
    /// `required`: the field is always there
    pub required: bool,
}

const INT_KINDS: &[&str] = &["int32", "int64", "uint32", "uint64", "sint32", "sint64", "fixed32", "fixed64", "sfixed32", "sfixed64"];

/// The range the integer rules of one `(buf.validate.field)` tree let through, written as
/// `{"int32": {"gte": 1}}` under whichever of the ten kinds: `gte` and `gt` (one above) for the low
/// end, `lte` and `lt` (one below) for the high end, `const` for both. `ignore` with
/// `IGNORE_IF_ZERO_VALUE`, or what it was called before, lets 0 through whatever the rest says.
/// A low end above the high end means, to Protovalidate, what is outside them, which is not a
/// range; that, an empty range, and `IGNORE_ALWAYS` give None.
fn int_range(rules: &Value) -> Option<Range> {
    let obj = INT_KINDS.iter().find_map(|k| rules.get(*k).filter(|v| v.is_object()))?;
    if rules["ignore"].as_str() == Some("IGNORE_ALWAYS") {
        return None;
    }
    let n = |k: &str| obj.get(k).and_then(|v| v.as_i64());
    let (gt, gte, lt, lte) = (n("gt"), n("gte"), n("lt"), n("lte"));
    if let Some(c) = n("const") {
        return Some(Range::exactly(c));
    }
    if let (Some(a), Some(b)) = (gt.or(gte), lt.or(lte)) {
        if a > b {
            return None;
        }
    }
    let lo = match (gt, gte) {
        (Some(g), _) => Some(g.checked_add(1)?),
        (None, g) => g,
    };
    let hi = match (lt, lte) {
        (Some(l), _) => Some(l.checked_sub(1)?),
        (None, l) => l,
    };
    if lo.is_none() && hi.is_none() {
        return None;
    }
    let r = Range { lo, hi };
    if let (Some(lo), Some(hi)) = (r.lo, r.hi) {
        if lo > hi {
            return None;
        }
    }
    let zero_passes = matches!(rules["ignore"].as_str(), Some("IGNORE_IF_ZERO_VALUE" | "IGNORE_IF_UNPOPULATED" | "IGNORE_IF_DEFAULT_VALUE"));
    Some(if zero_passes { r.hull(&Range::exactly(0)) } else { r })
}

/// What Protovalidate holds the field to: see `FieldRules`.
pub fn field_rules(f: &PField) -> FieldRules {
    FieldRules {
        range: int_range(&f.rules),
        items: int_range(&f.rules["repeated"]["items"]),
        required: f.rules["required"].as_bool().unwrap_or(false),
    }
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
    /// protobuf: a type that no file read has, which an import that was not read may hold
    Unread(String),
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
    /// a field that says whether it is set is read into a type without `?` all the same: at the
    /// entry of a workflow that implements a service, the workflow says what it needs (DESIGN 1.14)
    presence_free: bool,
    /// what the messages call the value read, in English and in Japanese: the answer of an operation
    noun: (&'static str, &'static str),
}

/// How the messages of E016 call what an operation answers.
const ANSWER: (&str, &str) = ("the answer", "レスポンス");

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
    Ok(Op { api, label, input, query, output, errors: responses.keys().cloned().collect(), token: None, form, procedure: None, streams: false, presence_free: false, noun: ANSWER })
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
    Ok(Op { api, label: local(id).to_string(), input, query: vec![], output, errors, token, form: false, procedure: None, streams: false, presence_free: false, noun: ANSWER })
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
        input: Node::Proto(PType::Named(found.input.clone()), FieldRules::default()),
        query: vec![],
        output: Some(Node::Proto(PType::Named(found.output.clone()), FieldRules::default())),
        errors: vec![],
        token: None,
        form: false,
        procedure: Some(format!("{}/{}", service.name, found.name)),
        streams: found.streams,
        presence_free: false,
        noun: ANSWER,
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
            (Node::Proto(t, rules), ApiDoc::Proto(pf)) => proto_shape(pf, t, rules),
            (Node::ProtoList(t, rules), ApiDoc::Proto(_)) => Shape::List(Node::Proto(t.clone(), FieldRules { range: rules.items, ..Default::default() })),
            _ => Shape::Any,
        }
    }

    /// What the operation says of the task (E016): each difference, in both languages.
    pub fn check(&self, m: &Model, task: &TaskDef, path_params: &[String]) -> Vec<Words> {
        let mut out = Vec::new();
        let api = &self.api.name;
        let op = &self.label;
        if self.streams {
            out.push((format!("`{api}` {op} streams; `connect` calls a method that takes one message and answers one"), format!("`{api}` の {op} はストリームです。`connect` で呼べるのは、一つ受け取って一つ返すメソッドです")));
            return out;
        }
        // a message nothing read has: what the task sends or reads cannot be held to it
        if let Shape::Unread(u) = self.shape(&self.input) {
            out.push((
                format!("`{api}` {op} takes `{u}`, a type that is not known (a file it may be in was not read), so the parameters of `{}` cannot be held to it", task.name),
                format!("`{api}` の {op} が受け取る `{u}` が分かりません（それがあるはずのファイルを読めませんでした）。`{}` の引数をそれに合わせて確かめられません", task.name),
            ));
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
                out.push((format!("what `{api}` {op} answers is not `{}`: {en}", m.ty_name(t)), format!("`{api}` の {op} のレスポンスは `{}` に合いません。{ja}", m.ty_name(t))));
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
                            out.push((format!("`{api}` {op} does not answer with {st} (`{}`)", e.name), format!("`{api}` の {op} は {st} を返しません（`{}`）", e.name)));
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
            (_, Shape::Unread(u)) => Err(unknown_type(u)),
            (Ty::Str, Shape::Str(None)) | (Ty::Str, Shape::Bytes) => Ok(()),
            // protobuf's JSON takes a 64-bit integer as a string, which is how a flow has it from an answer
            (Ty::Str, Shape::Int { text: true, .. }) => Ok(()),
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
            (Shape::Unread(u), _) => Err(unknown_type(u)),
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
                let (en, ja) = self.noun;
                out.push(match other {
                    Some(o) => (format!("{en} names `{f}` `{}`", o.name), format!("{ja}では `{f}` は `{}` という名前です", o.name)),
                    None => (format!("{en} has no `{f}` (a field of `{}`)", rd.name), format!("{ja}に `{f}`（`{}` のフィールド）はありません", rd.name)),
                });
                continue;
            };
            // at a workflow's entry, a field that may be left out is the workflow's to need or not
            if (mb.nullable || (mb.optional_out && !self.presence_free)) && !matches!(ft, Ty::Opt(_) | Ty::Json) {
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
            (Node::Proto(PType::Named(e), _), ApiDoc::Proto(pf)) => pf.enums.get(e).and_then(|vs| vs.first().cloned()),
            _ => None,
        }
    }

    /// For `connect`: the zero values of the answer's fields that protobuf leaves out of JSON,
    /// and the same for the messages inside it: `{"f": {key: zero}, "m": {key: …}, "l": {key: …}}`.
    pub fn zeros(&self) -> Option<Value> {
        let (ApiDoc::Proto(pf), Some(Node::Proto(PType::Named(msg), _))) = (&self.api.doc, &self.output) else { return None };
        Some(zeros_of(pf, msg))
    }
}

/// The zero values of the fields of the message `msg` that protobuf leaves out of JSON when they
/// are at them, and the same for the messages inside it: `{"f": {key: zero}, "m": {key: …}, "l": {key: …}}`.
/// The names are the fields' JSON names. `fill` puts them back; it is what reading a message of
/// `msg` from JSON does.
pub fn zeros_of(pf: &ProtoFile, msg: &str) -> Value {
    zeros_in(pf, msg, &mut vec![])
}

fn zeros_in(pf: &ProtoFile, msg: &str, within: &mut Vec<String>) -> Value {
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
                let sub = zeros_in(pf, n, within);
                if sub.as_object().is_some_and(|o| !o.is_empty()) {
                    ls.insert(fl.json.clone(), sub);
                }
            }
        } else if let PType::Map(..) = fl.ty {
            f.insert(fl.json.clone(), json!({}));
        } else if let Some(n) = &message {
            let sub = zeros_in(pf, n, within);
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

/// The message as protobuf's JSON writes it: each field that `zeros` names left out when it holds
/// its zero value, in the message, in its messages and in the messages of its lists. What `fill`
/// reads back: `fill(&omit_zeros(v, z), z)` is `v` when `v` has every field `z` names.
pub fn omit_zeros(v: &Value, zeros: &Value) -> Value {
    let Some(o) = v.as_object() else { return v.clone() };
    let mut out = o.clone();
    for (k, z) in zeros["f"].as_object().into_iter().flatten() {
        if out.get(k) == Some(z) {
            out.remove(k);
        }
    }
    for (k, sub) in zeros["m"].as_object().into_iter().flatten() {
        if let Some(x) = out.get(k).filter(|x| x.is_object()).cloned() {
            out.insert(k.clone(), omit_zeros(&x, sub));
        }
    }
    for (k, sub) in zeros["l"].as_object().into_iter().flatten() {
        if let Some(a) = out.get(k).and_then(|x| x.as_array()).cloned() {
            out.insert(k.clone(), Value::Array(a.iter().map(|x| omit_zeros(x, sub)).collect()));
        }
    }
    Value::Object(out)
}

fn field_words(f: &str, (en, ja): Words) -> Words {
    (format!("in `{f}`, {en}"), format!("`{f}` で、{ja}"))
}

/// What differs where a `.proto` names a type that no file read has: nothing can be said of it
/// but that, and `json` takes whatever it is.
fn unknown_type(n: &str) -> Words {
    (
        format!("its type `{n}` is not known, since a file it may be in was not read; `json` takes it"),
        format!("型 `{n}` が分かりません。それがあるはずのファイルを読めませんでした。`json` にすれば受け渡せます"),
    )
}

/// The note for what a flow comes to of a `.proto` that imports files which were not read: which
/// they are, and that the types in them cannot be used. None when every import was read.
pub fn unread_note(pf: &ProtoFile) -> Option<Words> {
    let files: Vec<String> = pf.unread.iter().map(|f| format!("`{f}`")).collect();
    let (first, rest) = files.split_first()?;
    let (en_list, ja_list, en_them) = match rest.split_last() {
        None => (first.clone(), first.clone(), "it"),
        Some((last, middle)) => {
            let head = std::iter::once(first).chain(middle.iter()).map(|x| x.as_str()).collect::<Vec<_>>().join(", ");
            (format!("{head} and {last}"), files.join("・"), "them")
        }
    };
    Some((
        format!("{en_list} could not be read, so the types in {en_them} cannot be used"),
        format!("{ja_list} を読めなかったので、そこにある型は使えません"),
    ))
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
        Shape::Unread(n) => format!("`{n}`, a type that is not known"),
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
        Shape::Unread(n) => format!("分からない型 `{n}`"),
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

fn proto_shape(pf: &ProtoFile, t: &PType, rules: &FieldRules) -> Shape {
    let (min, max) = (rules.range.and_then(|r| r.lo), rules.range.and_then(|r| r.hi));
    match t {
        PType::Scalar(s) => match s.as_str() {
            "string" => Shape::Str(None),
            "bytes" => Shape::Bytes,
            "bool" => Shape::Bool,
            "double" | "float" => Shape::Float,
            "int64" | "uint64" | "sint64" | "fixed64" | "sfixed64" => Shape::Int { text: true, min, max },
            _ => Shape::Int { text: false, min, max },
        },
        PType::Map(_, v) => Shape::Map(Node::Proto((**v).clone(), FieldRules::default())),
        PType::Named(n) => match n.as_str() {
            "google.protobuf.Timestamp" => Shape::Timestamp,
            "google.protobuf.Duration" | "google.protobuf.FieldMask" | "google.protobuf.StringValue" => Shape::Str(None),
            "google.protobuf.Struct" | "google.protobuf.Value" | "google.protobuf.Any" | "google.protobuf.NullValue" => Shape::Any,
            "google.protobuf.ListValue" => Shape::List(Node::Any),
            "google.protobuf.Empty" => Shape::Object(vec![]),
            "google.protobuf.DoubleValue" | "google.protobuf.FloatValue" => Shape::Float,
            "google.protobuf.Int64Value" | "google.protobuf.UInt64Value" => Shape::Int { text: true, min, max },
            "google.protobuf.Int32Value" | "google.protobuf.UInt32Value" => Shape::Int { text: false, min, max },
            "google.protobuf.BoolValue" => Shape::Bool,
            "google.protobuf.BytesValue" => Shape::Bytes,
            _ => {
                if let Some(vs) = pf.enums.get(n) {
                    return Shape::Str(Some(vs.clone()));
                }
                if !pf.knows(n) {
                    return Shape::Unread(n.clone());
                }
                Shape::Object(
                    pf.messages
                        .get(n)
                        .into_iter()
                        .flatten()
                        .map(|fl| {
                            let rules = field_rules(fl);
                            Member {
                                name: fl.json.clone(),
                                alias: (fl.name != fl.json).then(|| fl.name.clone()),
                                required: rules.required,
                                nullable: false,
                                optional_out: fl.presence && !rules.required,
                                node: if fl.repeated { Node::ProtoList(fl.ty.clone(), rules) } else { Node::Proto(fl.ty.clone(), rules) },
                            }
                        })
                        .collect(),
                )
            }
        },
    }
}

// ---------------------------------------------------------------------------
// Types made from a `.proto` (DESIGN 1.12)

/// What a message's field, or a named message or enum, is to a flow: the one table by which a
/// `.proto` is made into types and by which a task is held to it (`proto_shape` above reads the
/// same things as shapes). `lower` turns it into a `Ty`.
#[derive(Clone, Debug, PartialEq)]
pub enum MadeTy {
    /// a 32-bit integer, in the range the field's rules let through
    Int(Option<Range>),
    Str,
    Bool,
    Timestamp,
    Json,
    /// a message, by its full name; `google.protobuf.Empty` too, which has no fields
    Message(String),
    Enum(String),
    List(Box<MadeTy>),
    Opt(Box<MadeTy>),
    /// a type no file read has, which an import that was not read may hold: no type can be made of it
    Unread(String),
}

/// A field of the record made from a message, by the key it has in JSON.
#[derive(Clone, Debug, PartialEq)]
pub struct MadeField {
    pub name: String,
    pub ty: MadeTy,
}

/// What a type of a `.proto` is, with `range` for a 32-bit integer. A 64-bit integer is a
/// `string`, for protobuf's JSON writes it as one; a `float` or a `double` is `json`, which has no
/// fraction to be checked; a wrapper is the type inside it.
fn made_elem(pf: &ProtoFile, t: &PType, range: Option<Range>) -> MadeTy {
    match t {
        PType::Scalar(s) => match s.as_str() {
            "string" | "bytes" => MadeTy::Str,
            "bool" => MadeTy::Bool,
            "double" | "float" => MadeTy::Json,
            "int64" | "uint64" | "sint64" | "fixed64" | "sfixed64" => MadeTy::Str,
            _ => MadeTy::Int(range),
        },
        PType::Map(..) => MadeTy::Json,
        PType::Named(n) => match n.as_str() {
            "google.protobuf.Timestamp" => MadeTy::Timestamp,
            "google.protobuf.Duration" | "google.protobuf.FieldMask" | "google.protobuf.StringValue" | "google.protobuf.BytesValue" => MadeTy::Str,
            "google.protobuf.Struct" | "google.protobuf.Value" | "google.protobuf.ListValue" | "google.protobuf.Any" | "google.protobuf.NullValue" => MadeTy::Json,
            "google.protobuf.Int32Value" | "google.protobuf.UInt32Value" => MadeTy::Int(range),
            "google.protobuf.Int64Value" | "google.protobuf.UInt64Value" => MadeTy::Str,
            "google.protobuf.DoubleValue" | "google.protobuf.FloatValue" => MadeTy::Json,
            "google.protobuf.BoolValue" => MadeTy::Bool,
            _ if pf.enums.contains_key(n) => MadeTy::Enum(n.clone()),
            _ if !pf.knows(n) => MadeTy::Unread(n.clone()),
            _ => MadeTy::Message(n.clone()),
        },
    }
}

/// What a type named in a `.proto` is when a flow names it (`warehouse.Stock`): as a field of it
/// would be, without its `?`.
pub fn made_named(pf: &ProtoFile, full: &str) -> MadeTy {
    made_elem(pf, &PType::Named(full.to_string()), None)
}

/// The fields of the record made from the message `msg`, named by its full name. A field that
/// says whether it is set is `T?`, unless it is `required` or its type is `json`, which has null
/// for not set; a `repeated` field is a list, and a `map` is `json`.
pub fn made_fields(pf: &ProtoFile, msg: &str) -> Vec<MadeField> {
    pf.messages
        .get(msg)
        .into_iter()
        .flatten()
        .map(|fl| {
            let rules = field_rules(fl);
            let ty = if fl.repeated {
                MadeTy::List(Box::new(made_elem(pf, &fl.ty, rules.items)))
            } else {
                let t = made_elem(pf, &fl.ty, rules.range);
                if fl.presence && !rules.required && t != MadeTy::Json {
                    MadeTy::Opt(Box::new(t))
                } else {
                    t
                }
            };
            MadeField { name: fl.json.clone(), ty }
        })
        .collect()
}

/// The values of the enum `en` (full name) as a flow has them, which are their names in the `.proto`.
/// The zero value is left out when its name says nothing was set: with the enum's own name
/// (`Stock` is `STOCK_`) taken off the front, it is `unspecified` in any case. A zero value with
/// another name is a value, and stays.
pub fn made_enum_values(pf: &ProtoFile, en: &str) -> Vec<String> {
    let values = pf.enums.get(en).cloned().unwrap_or_default();
    let simple = en.rsplit('.').next().unwrap_or(en);
    let prefix = format!("{}_", crate::rulec::upper_snake(simple));
    values.into_iter().enumerate().filter(|(i, v)| !(*i == 0 && v.strip_prefix(&prefix).unwrap_or(v).eq_ignore_ascii_case("unspecified"))).map(|(_, v)| v).collect()
}

/// What a task is held to, of a type made from the message or enum `full` (E016's two directions):
/// what is sent as `t` must be what the description takes, and what the description answers must be
/// readable as `t`. Empty when the type made agrees with its description.
pub fn made_fits(api: &Api, full: &str, m: &Model, t: &Ty) -> Vec<Words> {
    let ApiDoc::Proto(_) = &api.doc else { return vec![] };
    let node = Node::Proto(PType::Named(full.to_string()), FieldRules::default());
    let op = Op { api, label: full.to_string(), input: node.clone(), query: vec![], output: Some(node.clone()), errors: vec![], token: None, form: false, procedure: None, streams: false, presence_free: false, noun: ANSWER };
    let mut out = Vec::new();
    if let Err(w) = op.sends(m, t, None, &node, &mut vec![]) {
        out.push(w);
    }
    if let Err(w) = op.reads(&node, m, t, None, &mut vec![]) {
        out.push(w);
    }
    out
}

// ---------------------------------------------------------------------------
// The messages of a service a workflow implements (DESIGN 1.14)

/// A message of a `.proto`, looked at whole: what a method of a service takes or answers, which a
/// workflow that implements the service is held to (E017). Its fields are compared with the flow's
/// types by the table E016 uses, in the direction the values go.
pub struct MessageView<'a> {
    api: &'a Api,
    /// the message's full name: `shop.v1.FulfillRequest`
    pub name: String,
}

/// The message `msg` (its full name) of the `.proto` `api` read.
pub fn message<'a>(api: &'a Api, msg: &str) -> MessageView<'a> {
    MessageView { api, name: msg.to_string() }
}

/// What a field of a message says of being set, beside whether it may be left out.
pub struct FieldFacts {
    pub required: bool,
    /// `repeated`, or a `map`
    pub list: bool,
}

/// How the messages of E017 call the value a message is read into.
const MESSAGE: (&str, &str) = ("the message", "メッセージ");

impl<'a> MessageView<'a> {
    /// The message's name without its package, as a message names it: `FulfillRequest`.
    pub fn simple(&self) -> &str {
        self.name.rsplit('.').next().unwrap_or(&self.name)
    }

    fn pf(&self) -> &ProtoFile {
        match &self.api.doc {
            ApiDoc::Proto(pf) => pf,
            _ => unreachable!("a .proto"),
        }
    }

    /// Whether a file read has the message (a well-known type is known too).
    pub fn known(&self) -> bool {
        self.pf().knows(&self.name)
    }

    fn op(&self, presence_free: bool) -> Op<'a> {
        let node = Node::Proto(PType::Named(self.name.clone()), FieldRules::default());
        let label = self.simple().to_string();
        Op { api: self.api, label, input: node.clone(), query: vec![], output: Some(node), errors: vec![], token: None, form: false, procedure: None, streams: false, presence_free, noun: MESSAGE }
    }

    fn members(&self, op: &Op) -> Vec<Member> {
        match op.shape(&Node::Proto(PType::Named(self.name.clone()), FieldRules::default())) {
            Shape::Object(ms) => ms,
            _ => vec![],
        }
    }

    /// The fields by their JSON names, in order, and whether each may be left out: it says whether
    /// it is set (`optional`, a message, a member of a `oneof`) and is not `required`.
    pub fn fields(&self) -> Vec<(String, bool)> {
        self.members(&self.op(false)).into_iter().map(|mb| (mb.name, mb.optional_out)).collect()
    }

    /// What the field `json` says of being set: whether it is `required`, and whether it is a list
    /// or a map, which say nothing of it.
    pub fn field(&self, json: &str) -> Option<FieldFacts> {
        let f = self.pf().messages.get(&self.name)?.iter().find(|f| f.json == json)?;
        Some(FieldFacts { required: field_rules(f).required, list: f.repeated || matches!(f.ty, PType::Map(..)) })
    }

    /// Whether every value the field `json` has is one of `t` (in `rg`): what comes in. With
    /// `presence_free`, a field that may be left out is read into a type without `?` all the same,
    /// in the fields of the messages inside it too.
    pub fn field_reads(&self, json: &str, m: &Model, t: &Ty, rg: Option<Range>, presence_free: bool) -> Result<(), Words> {
        let op = self.op(presence_free);
        let Some(mb) = self.members(&op).into_iter().find(|x| x.name == json) else { return Ok(()) };
        if mb.optional_out && !presence_free && !matches!(t, Ty::Opt(_) | Ty::Json) {
            return Err((format!("it may be left out; declare it `{}?`", m.ty_name(t)), format!("無いことがあります。`{}?` にしてください", m.ty_name(t))));
        }
        op.reads(&mb.node, m, t, rg, &mut vec![])
    }

    /// Whether every value of `t` (in `rg`) is one the field `json` takes: what goes out.
    pub fn field_sends(&self, json: &str, m: &Model, t: &Ty, rg: Option<Range>) -> Result<(), Words> {
        let op = self.op(false);
        let Some(mb) = self.members(&op).into_iter().find(|x| x.name == json) else { return Ok(()) };
        op.sends(m, t, rg, &mb.node, &mut vec![])
    }

    /// What differs when the whole message is read into `t` (in `rg`): for a record, one difference
    /// for each field that differs; with `presence_free`, as `field_reads` says.
    pub fn reads(&self, m: &Model, t: &Ty, rg: Option<Range>, presence_free: bool) -> Vec<Words> {
        let op = self.op(presence_free);
        let node = Node::Proto(PType::Named(self.name.clone()), FieldRules::default());
        match (t, op.shape(&node)) {
            (Ty::Record(r), Shape::Object(ms)) => op.fields(&ms, m, *r, &mut vec![]),
            _ => op.reads(&node, m, t, rg, &mut vec![]).err().into_iter().collect(),
        }
    }

    /// What differs between the fields of this message and those of `other`, by their JSON names:
    /// a field one has and the other does not, and the type of one they both have, as a `.proto`
    /// writes it (`optional int32`, `map<string, google.protobuf.Value>`, `repeated string`).
    pub fn same_fields(&self, other: &MessageView) -> Vec<Words> {
        let mine = self.pf().messages.get(&self.name).cloned().unwrap_or_default();
        let theirs = other.pf().messages.get(&other.name).cloned().unwrap_or_default();
        let theirs_name = &other.name;
        let mut out = Vec::new();
        for t in &theirs {
            match mine.iter().find(|f| f.json == t.json) {
                None => out.push((format!("it has no `{}`", t.json), format!("`{}` がありません", t.json))),
                Some(f) => {
                    let (a, b) = (field_text(self.pf(), f), field_text(other.pf(), t));
                    if a != b {
                        out.push((format!("`{}` is `{a}`, and in `{theirs_name}` it is `{b}`", f.json), format!("`{}` は `{a}` ですが、`{theirs_name}` では `{b}` です", f.json)));
                    }
                }
            }
        }
        for f in mine.iter().filter(|f| !theirs.iter().any(|t| t.json == f.json)) {
            out.push((format!("it has `{}`, which `{theirs_name}` does not", f.json), format!("`{theirs_name}` に無い `{}` があります", f.json)));
        }
        out
    }
}

/// A field's type as a `.proto` writes it: `optional int32`, `repeated string`, `map<string, google.protobuf.Value>`.
fn field_text(pf: &ProtoFile, f: &PField) -> String {
    fn ty(t: &PType) -> String {
        match t {
            PType::Scalar(s) | PType::Named(s) => s.clone(),
            PType::Map(k, v) => format!("map<{}, {}>", ty(k), ty(v)),
        }
    }
    let t = ty(&f.ty);
    let message = matches!(&f.ty, PType::Named(n) if !pf.enums.contains_key(n));
    if f.repeated {
        format!("repeated {t}")
    } else if f.presence && !message {
        format!("optional {t}")
    } else {
        t
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn range(lo: Option<i64>, hi: Option<i64>) -> Option<Range> {
        Some(Range { lo, hi })
    }

    #[test]
    fn a_range_is_what_the_integer_rules_let_through() {
        assert_eq!(int_range(&json!({"int32": {"gte": 1, "lte": 100}})), range(Some(1), Some(100)));
        // gt is one above, lt one below
        assert_eq!(int_range(&json!({"int32": {"gt": 0, "lt": 10}})), range(Some(1), Some(9)));
        // either end alone, under any of the ten kinds
        assert_eq!(int_range(&json!({"uint32": {"lte": 5}})), range(None, Some(5)));
        assert_eq!(int_range(&json!({"sfixed64": {"gte": -3}})), range(Some(-3), None));
        assert_eq!(int_range(&json!({"int32": {"const": 7}})), range(Some(7), Some(7)));
        // a field ignored at its zero value lets 0 through, whatever the ends say
        assert_eq!(int_range(&json!({"int32": {"gt": 0}, "ignore": "IGNORE_IF_ZERO_VALUE"})), range(Some(0), None));
        assert_eq!(int_range(&json!({"int32": {"gte": 5, "lte": 9}, "ignore": "IGNORE_IF_UNPOPULATED"})), range(Some(0), Some(9)));
        // what is outside two ends, an empty range, a field that is ignored, and no rule at all are not ranges
        assert_eq!(int_range(&json!({"int32": {"gt": 10, "lt": 5}})), None);
        assert_eq!(int_range(&json!({"int32": {"gt": 5, "lt": 6}})), None);
        assert_eq!(int_range(&json!({"int32": {"gte": 1}, "ignore": "IGNORE_ALWAYS"})), None);
        assert_eq!(int_range(&json!({"string": {"min_len": 1}})), None);
        assert_eq!(int_range(&Value::Null), None);
        // the rules of a string or a list say nothing of a number
        assert_eq!(int_range(&json!({"int32": {"in": [1, 2]}})), None);
    }

    fn load(text: &str) -> ProtoFile {
        crate::proto::load_text("t.proto", text).unwrap_or_else(|e| panic!("{e}"))
    }

    #[test]
    fn the_zero_value_of_an_enum_is_left_out_when_its_name_says_nothing_was_set() {
        let pf = load(
            r#"syntax = "proto3"; package a.v1;
            enum Stock { unspecified = 0; secured = 1; short = 2; }
            enum MemberTier { MEMBER_TIER_UNSPECIFIED = 0; MEMBER_TIER_GOLD = 1; }
            enum Mode { ACTIVE = 0; PAUSED = 1; }
            enum State { STATE_UNKNOWN = 0; STATE_OPEN = 1; }
            enum Counted { COUNTED_unspecified = 0; OTHER = 1; }
            enum Bare { UNSPECIFIED = 0; ONE = 1; }
            message Order { enum Status { STATUS_UNSPECIFIED = 0; STATUS_PAID = 1; } }"#,
        );
        let values = |e: &str| made_enum_values(&pf, e);
        assert_eq!(values("a.v1.Stock"), ["secured", "short"]);
        assert_eq!(values("a.v1.MemberTier"), ["MEMBER_TIER_GOLD"]);
        // a zero value with another name is a value
        assert_eq!(values("a.v1.Mode"), ["ACTIVE", "PAUSED"]);
        assert_eq!(values("a.v1.State"), ["STATE_UNKNOWN", "STATE_OPEN"]);
        // the prefix is the enum's own name in capitals, and any case of `unspecified` counts
        assert_eq!(values("a.v1.Counted"), ["OTHER"]);
        assert_eq!(values("a.v1.Bare"), ["ONE"]);
        // a nested enum's prefix is its own name, not its message's
        assert_eq!(values("a.v1.Order.Status"), ["STATUS_PAID"]);
    }

    #[test]
    fn every_kind_of_field_is_what_the_design_says() {
        let pf = load(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/protos/types.proto")).unwrap());
        let fields = made_fields(&pf, "types.v1.Everything");
        let ty = |name: &str| fields.iter().find(|f| f.name == name).unwrap_or_else(|| panic!("no field {name}")).ty.clone();
        let opt = |t: MadeTy| MadeTy::Opt(Box::new(t));
        let list = |t: MadeTy| MadeTy::List(Box::new(t));
        assert_eq!(ty("name"), MadeTy::Str);
        assert_eq!(ty("blob"), MadeTy::Str);
        assert_eq!(ty("flag"), MadeTy::Bool);
        assert_eq!(ty("count"), MadeTy::Int(None));
        assert_eq!(ty("small"), MadeTy::Int(None));
        // a 64-bit integer is a string, and a float or a double is json
        assert_eq!(ty("big"), MadeTy::Str);
        assert_eq!(ty("bigger"), MadeTy::Str);
        assert_eq!(ty("ratio"), MadeTy::Json);
        assert_eq!(ty("weight"), MadeTy::Json);
        assert_eq!(ty("status"), MadeTy::Enum("types.v1.Status".into()));
        assert_eq!(ty("mode"), MadeTy::Enum("types.v1.Mode".into()));
        // what says whether it is set is `T?`, unless it is `required`
        assert_eq!(ty("line"), opt(MadeTy::Message("types.v1.Line".into())));
        assert_eq!(ty("requiredLine"), MadeTy::Message("types.v1.Line".into()));
        assert_eq!(ty("lines"), list(MadeTy::Message("types.v1.Line".into())));
        assert_eq!(ty("tags"), MadeTy::Json);
        assert_eq!(ty("byName"), MadeTy::Json);
        assert_eq!(ty("note"), opt(MadeTy::Str));
        assert_eq!(ty("amount"), opt(MadeTy::Int(Some(Range { lo: Some(1), hi: None }))));
        assert_eq!(ty("maybeStatus"), opt(MadeTy::Enum("types.v1.Status".into())));
        assert_eq!(ty("requiredNote"), MadeTy::Str);
        // a member of a oneof says whether it is set
        assert_eq!(ty("byId"), opt(MadeTy::Str));
        assert_eq!(ty("byNumber"), opt(MadeTy::Int(None)));
        // well-known types
        assert_eq!(ty("at"), opt(MadeTy::Timestamp));
        assert_eq!(ty("span"), opt(MadeTy::Str));
        assert_eq!(ty("extra"), MadeTy::Json);
        assert_eq!(ty("value"), MadeTy::Json);
        assert_eq!(ty("values"), MadeTy::Json);
        assert_eq!(ty("anyThing"), MadeTy::Json);
        assert_eq!(ty("nothing"), opt(MadeTy::Message("google.protobuf.Empty".into())));
        assert_eq!(ty("wrapped"), opt(MadeTy::Int(None)));
        assert_eq!(ty("wrappedBig"), opt(MadeTy::Str));
        assert_eq!(ty("wrappedDouble"), MadeTy::Json);
        assert_eq!(ty("wrappedText"), opt(MadeTy::Str));
        assert_eq!(ty("times"), list(MadeTy::Timestamp));
        assert_eq!(ty("nested"), opt(MadeTy::Message("types.v1.Everything.Nested".into())));
        // the ranges of Line, and of the items of a list of numbers
        let line = made_fields(&pf, "types.v1.Line");
        let l = |name: &str| line.iter().find(|f| f.name == name).unwrap().ty.clone();
        assert_eq!(l("quantity"), MadeTy::Int(range(Some(1), Some(99))));
        assert_eq!(l("discount"), MadeTy::Int(range(Some(0), Some(50))));
        assert_eq!(l("offset"), MadeTy::Int(range(Some(0), None)));
        assert_eq!(l("outside"), MadeTy::Int(None));
        assert_eq!(l("free"), MadeTy::Int(None));
        assert_eq!(l("marks"), list(MadeTy::Int(range(Some(1), Some(5)))));
        assert_eq!(l("fixed"), MadeTy::Int(range(Some(7), Some(7))));
        assert_eq!(l("total"), MadeTy::Str);
    }

    #[test]
    fn the_zero_values_are_those_of_the_fields_json_leaves_out() {
        let pf = load(r#"syntax = "proto3"; package a.v1; message M { string s = 1; optional string o = 2; int32 n = 3; M inner = 4; repeated M more = 5; }"#);
        let z = zeros_of(&pf, "a.v1.M");
        assert_eq!(z["f"], json!({"s": "", "n": 0, "more": []}));
        assert_eq!(fill(&json!({"o": "x"}), &z), json!({"o": "x", "s": "", "n": 0, "more": []}));
    }

    #[test]
    fn what_is_left_out_at_its_zero_value_is_filled_back_in() {
        let pf = load(
            r#"syntax = "proto3"; package a.v1;
            message Line { string sku = 1; int32 quantity = 2; bool gift = 3; }
            message Order { string id = 1; int32 amount = 2; repeated Line lines = 3; Line first = 4; optional string note = 5; map<string, string> tags = 6; }"#,
        );
        let z = zeros_of(&pf, "a.v1.Order");
        // every field at its zero value, in the message, in its message and in the messages of its list
        let full = json!({"id": "", "amount": 0, "lines": [{"sku": "", "quantity": 0, "gift": false}, {"sku": "a", "quantity": 2, "gift": true}], "first": {"sku": "", "quantity": 3, "gift": false}, "note": "", "tags": {}});
        let sparse = omit_zeros(&full, &z);
        assert_eq!(sparse, json!({"lines": [{}, {"sku": "a", "quantity": 2, "gift": true}], "first": {"quantity": 3}, "note": ""}));
        assert_eq!(fill(&sparse, &z), full);
        // an empty list is left out, and filled back in empty
        let empty = json!({"id": "x", "amount": 1, "lines": [], "note": null, "tags": {"a": "b"}});
        assert_eq!(omit_zeros(&empty, &z), json!({"id": "x", "amount": 1, "note": null, "tags": {"a": "b"}}));
        assert_eq!(fill(&omit_zeros(&empty, &z), &z), empty);
    }
}
