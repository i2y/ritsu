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
use crate::diag::Text;

/// A difference, in English and in Japanese.
pub type Words = Text;

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

/// The document of an API, or why it cannot be read, in both languages (what the system or the JSON
/// reader says is in its own words in both).
pub fn load(kind: ApiKind, path: &Path) -> Result<ApiDoc, Text> {
    match kind {
        ApiKind::Proto => crate::proto::load(path).map(|pf| ApiDoc::Proto(Arc::new(pf))),
        _ => {
            let shown = ritsu_base::paths::show(path);
            let text = crate::sources::read(path).map_err(|e| ritsu_base::tr!("{shown} を読めません: {e}", "cannot read {shown}: {e}"))?;
            let v: Value = serde_json::from_str(&text).map_err(|e| {
                ritsu_base::tr!(
                    "{shown} は JSON ではありません（{e}）。dandori が読むのは、JSON で書いた OpenAPI の文書か Smithy のモデルです",
                    "{shown} is not JSON ({e}); dandori reads an OpenAPI document or a Smithy model written as JSON"
                )
            })?;
            if kind == ApiKind::OpenApi {
                if !v["openapi"].as_str().is_some_and(|s| s.starts_with('3')) {
                    return Err(ritsu_base::tr!("{shown} は OpenAPI 3 の文書ではありません", "{shown} is not an OpenAPI 3 document"));
                }
                Ok(ApiDoc::OpenApi(v))
            } else {
                if !v["shapes"].is_object() {
                    return Err(ritsu_base::tr!("{shown} は、JSON AST で書いた Smithy のモデルではありません", "{shown} is not a Smithy model in the JSON AST"));
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
    /// a moment: OpenAPI's `date-time`, Smithy's `timestamp`, protobuf's `Timestamp`; `text`: the
    /// description's own rules make it a JSON string (OpenAPI's and protobuf's), which a `string`
    /// reads as well. A Smithy timestamp goes as its protocol and `@timestampFormat` say, which may
    /// be a number of seconds.
    Timestamp { text: bool },
    /// a day of the calendar, `2026-11-16`: OpenAPI's `date` (RFC 3339's full-date), which is a
    /// flow's `date` as it goes and as it comes
    Date,
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

/// A parameter of an OpenAPI operation (DESIGN 43): where it goes (`in`), and how it is written
/// there as the description says it (`style` and `explode` as written; none for the place's own).
#[derive(Clone, Debug)]
struct Param {
    member: Member,
    place: Place,
    style: Option<String>,
    explode: Option<bool>,
    /// the description says how it is written with `content` (a media type), not with `schema`
    content: bool,
}

/// Header parameters OpenAPI says are left out (3.1.1, Parameter Object): the request's own.
const HEADERS_LEFT_OUT: [&str; 3] = ["Accept", "Content-Type", "Authorization"];

/// The headers each language's HTTP client writes itself, which a parameter cannot set.
const HEADERS_OF_THE_CLIENT: [&str; 10] = ["Host", "Content-Length", "Transfer-Encoding", "Connection", "Keep-Alive", "Proxy-Connection", "TE", "Trailer", "Upgrade", "Expect"];

/// Whether `name` is a token of HTTP (RFC 9110's `token`), which a header's and a cookie's name is.
fn is_token(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
}

/// Whether a parameter's name in the flow is the name a description gives it at `place`: `-` and `_`
/// are one character on either side, and a header's name is the same in any case (a cookie's and
/// the others' are not).
pub fn same_name(place: Place, described: &str, param: &str) -> bool {
    let a = described.replace('-', "_");
    let b = param.replace('-', "_");
    if place == Place::Header {
        a.eq_ignore_ascii_case(&b)
    } else {
        a == b
    }
}

/// One operation of an API: what it takes and answers, and how it fails.
pub struct Op<'a> {
    api: &'a Api,
    /// how a message names it: `POST /v1/payment_intents`, `Publish`, `StockService/Reserve`
    pub label: String,
    /// the body's, or the input's
    input: Node,
    /// OpenAPI: the parameters of the path, the query, the headers and the cookies
    params: Vec<Param>,
    /// OpenAPI: the Encoding Objects of a URL-encoded body, by its properties
    encodings: Value,
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
    /// what the messages call the value read, in Japanese and in English: the answer of an operation
    noun: fn() -> Text,
}

/// How the messages of E016 call what an operation answers.
fn an_answer() -> Text {
    tr!("レスポンス", "the answer")
}

/// The OpenAPI operation at `method` and `path`.
pub fn openapi_op<'a>(api: &'a Api, method: &str, path: &str) -> Result<Op<'a>, Words> {
    let ApiDoc::OpenApi(doc) = &api.doc else { unreachable!("an OpenAPI document") };
    let label = format!("{method} {path}");
    let item = &doc["paths"][path];
    let op = &item[method.to_ascii_lowercase()];
    if !op.is_object() {
        let near: Vec<String> = doc["paths"].as_object().map(|p| p.keys().filter(|k| k.split('{').next() == path.split('{').next()).cloned().collect()).unwrap_or_default();
        let Text { en: hint_en, ja: hint_ja } = if near.is_empty() { Text::default() } else { tr!("（あるのは {}）", " (it has {})", near.join("・"); near.join(", ")) };
        return Err(tr!("`{}` に {label} はありません{hint_ja}", "`{}` has no {label}{hint_en}", api.name));
    }
    // the parameters of the path item and of the operation; the operation's take the place of the
    // path item's of the same name and place
    let mut params: Vec<Param> = Vec::new();
    for p in item["parameters"].as_array().into_iter().flatten().chain(op["parameters"].as_array().into_iter().flatten()) {
        let p = resolve(doc, p);
        let place = match p["in"].as_str() {
            Some("path") => Place::Path,
            Some("query") => Place::Query,
            Some("header") => Place::Header,
            Some("cookie") => Place::Cookie,
            _ => continue,
        };
        let name = p["name"].as_str().unwrap_or_default().to_string();
        let content = p.get("content").is_some_and(|c| c.is_object()) && p.get("schema").is_none();
        let param = Param {
            member: Member {
                name: name.clone(),
                alias: None,
                // a parameter of the path is always needed (OpenAPI: its `required` is true)
                required: place == Place::Path || p["required"].as_bool().unwrap_or(false),
                nullable: false,
                optional_out: true,
                node: Node::Json(p["schema"].clone()),
            },
            place,
            style: p["style"].as_str().map(String::from),
            explode: p["explode"].as_bool(),
            content,
        };
        let same = |q: &Param| q.place == place && if place == Place::Header { q.member.name.eq_ignore_ascii_case(&name) } else { q.member.name == name };
        match params.iter().position(same) {
            Some(i) => params[i] = param,
            None => params.push(param),
        }
    }
    let content = &resolve(doc, &op["requestBody"])["content"];
    let (input, form, encodings) = match (content.get("application/json"), content.get("application/x-www-form-urlencoded")) {
        (Some(c), _) => (Node::Json(c["schema"].clone()), false, Value::Null),
        (None, Some(c)) => (Node::Json(c["schema"].clone()), true, c["encoding"].clone()),
        _ => (Node::Any, false, Value::Null),
    };
    let responses = op["responses"].as_object().cloned().unwrap_or_default();
    let ok = responses.iter().find(|(s, _)| s.starts_with('2')).map(|(_, r)| resolve(doc, r));
    let output = ok.map(|r| match r["content"].get("application/json") {
        Some(c) => Node::Json(c["schema"].clone()),
        None => Node::Any,
    });
    Ok(Op { api, label, input, params, encodings, output, errors: responses.keys().cloned().collect(), token: None, form, procedure: None, streams: false, presence_free: false, noun: an_answer })
}

/// The Smithy operation that Step Functions names `action` (`publish` for `Publish`).
pub fn smithy_op<'a>(api: &'a Api, action: &str) -> Result<Op<'a>, Words> {
    let ApiDoc::Smithy(doc) = &api.doc else { unreachable!("a Smithy model") };
    let shapes = doc["shapes"].as_object().cloned().unwrap_or_default();
    let found = shapes.iter().find(|(id, s)| s["type"] == "operation" && local(id).eq_ignore_ascii_case(action));
    let Some((id, op)) = found else {
        return Err(tr!("`{}` に操作 `{action}` はありません", "`{}` has no operation `{action}`", api.name));
    };
    let input = op["input"]["target"].as_str().map(|t| Node::Shape(t.to_string())).unwrap_or(Node::Any);
    let output = op["output"]["target"].as_str().map(|t| Node::Shape(t.to_string()));
    let service_errors: Vec<Value> = shapes.values().filter(|s| s["type"] == "service").flat_map(|s| s["errors"].as_array().cloned().unwrap_or_default()).collect();
    let errors = op["errors"].as_array().into_iter().flatten().chain(service_errors.iter()).filter_map(|e| e["target"].as_str()).map(|t| local(t).to_string()).collect();
    let token = op["input"]["target"].as_str().and_then(|t| shapes.get(t)).and_then(|s| s["members"].as_object()).and_then(|ms| {
        ms.iter().find(|(_, m)| m["traits"].get("smithy.api#idempotencyToken").is_some()).map(|(n, _)| n.clone())
    });
    Ok(Op { api, label: local(id).to_string(), input, params: vec![], encodings: Value::Null, output, errors, token, form: false, procedure: None, streams: false, presence_free: false, noun: an_answer })
}

/// The protocols of AWS's Smithy models, by the forms a timestamp takes in them.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Protocol {
    /// awsJson1_0 and awsJson1_1: the input is a JSON body
    AwsJson,
    /// restJson1: a JSON body, and the members bound to the headers, the query and the path
    RestJson,
    /// restXml: an XML body, and the bound members
    RestXml,
    /// awsQuery and ec2Query: the input is a URL-encoded body
    Query,
    /// rpcv2Cbor: a CBOR body
    Cbor,
}

/// The protocol a Smithy model's service names by its trait. A service that names several is read
/// by the first of these.
fn smithy_protocol(doc: &Value) -> Option<Protocol> {
    let service = doc["shapes"].as_object()?.values().find(|s| s["type"] == "service")?;
    let traits = service["traits"].as_object()?;
    [
        ("aws.protocols#restJson1", Protocol::RestJson),
        ("aws.protocols#awsJson1_0", Protocol::AwsJson),
        ("aws.protocols#awsJson1_1", Protocol::AwsJson),
        ("aws.protocols#restXml", Protocol::RestXml),
        ("aws.protocols#awsQuery", Protocol::Query),
        ("aws.protocols#ec2Query", Protocol::Query),
        ("smithy.protocols#rpcv2Cbor", Protocol::Cbor),
    ]
    .into_iter()
    .find(|(t, _)| traits.contains_key(*t))
    .map(|(_, p)| p)
}

/// Where a member of an operation's input goes: the body, or the header, the query or the path a
/// protocol of HTTP bindings (restJson1, restXml) binds it to.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Bound {
    Body,
    Header,
    Query,
    Label,
}

/// The form a timestamp takes where it goes when the model's `@timestampFormat` does not say:
/// `http-date` in a header, `date-time` in the query and the path, and in the body the protocol's
/// own, `epoch-seconds` in JSON and CBOR, `date-time` in XML and in a URL-encoded body. Under a
/// protocol dandori does not know, `timestamp`: the SDK that calls it knows.
fn timestamp_form(protocol: Option<Protocol>, place: Bound) -> &'static str {
    match (protocol, place) {
        (_, Bound::Header) => "http-date",
        (_, Bound::Query | Bound::Label) => "date-time",
        (Some(Protocol::AwsJson | Protocol::RestJson | Protocol::Cbor), Bound::Body) => "epoch-seconds",
        (Some(Protocol::RestXml | Protocol::Query), Bound::Body) => "date-time",
        (None, Bound::Body) => "timestamp",
    }
}

/// The timestamps inside a member of a Smithy model, each with its form: `"epoch-seconds"` for a
/// timestamp, `{"Time": …}` for those of a structure's members, `{"*": …}` for those of each item
/// of a list or each value of a map; none when it holds no timestamp. `seen`: the structures on the
/// way down, so that a structure that holds itself ends there.
fn smithy_times(doc: &Value, protocol: Option<Protocol>, member: &Value, place: Bound, seen: &mut Vec<String>) -> Option<Value> {
    let target = member["target"].as_str()?;
    let shape = &doc["shapes"][target];
    let kind = match target.strip_prefix("smithy.api#") {
        Some("Timestamp") => "timestamp",
        Some(_) => return None,
        None => shape["type"].as_str()?,
    };
    match kind {
        // the member's own @timestampFormat, else the shape's, else the protocol's for the place
        "timestamp" => {
            let said = |v: &Value| v["traits"]["smithy.api#timestampFormat"].as_str().map(String::from);
            Some(Value::String(said(member).or_else(|| said(shape)).unwrap_or_else(|| timestamp_form(protocol, place).to_string())))
        }
        "structure" | "union" => {
            if seen.iter().any(|s| s == target) {
                return None;
            }
            seen.push(target.to_string());
            let mut out = Map::new();
            for (name, m) in shape["members"].as_object().into_iter().flatten() {
                if let Some(v) = smithy_times(doc, protocol, m, Bound::Body, seen) {
                    out.insert(name.clone(), v);
                }
            }
            seen.pop();
            (!out.is_empty()).then_some(Value::Object(out))
        }
        "list" | "set" => smithy_times(doc, protocol, &shape["member"], place, seen).map(|v| json!({ "*": v })),
        "map" => smithy_times(doc, protocol, &shape["value"], Bound::Body, seen).map(|v| json!({ "*": v })),
        _ => None,
    }
}

impl Op<'_> {
    /// The timestamps in what a task sends this operation of a Smithy model, by the members the
    /// task gives (`params`), each with the form the model sends it in (`smithy_times`):
    /// `{"StartingPositionTimestamp": "epoch-seconds"}`. None when there is none, or the API is
    /// not a Smithy model. The default Transport in TypeScript hands them to the AWS SDK as Dates,
    /// which the SDK sends in that form (DESIGN 42).
    pub fn timestamps(&self, params: &[String]) -> Option<Value> {
        let ApiDoc::Smithy(doc) = &self.api.doc else { return None };
        let Node::Shape(input) = &self.input else { return None };
        let protocol = smithy_protocol(doc);
        let binds = matches!(protocol, Some(Protocol::RestJson | Protocol::RestXml));
        let mut out = Map::new();
        for (name, m) in doc["shapes"][input.as_str()]["members"].as_object().into_iter().flatten() {
            if !params.iter().any(|p| p == name) {
                continue;
            }
            let t = &m["traits"];
            let place = if !binds {
                Bound::Body
            } else if t.get("smithy.api#httpHeader").is_some() {
                Bound::Header
            } else if t.get("smithy.api#httpQuery").is_some() {
                Bound::Query
            } else if t.get("smithy.api#httpLabel").is_some() {
                Bound::Label
            } else {
                Bound::Body
            };
            if let Some(v) = smithy_times(doc, protocol, m, place, &mut vec![input.clone()]) {
                out.insert(name.clone(), v);
            }
        }
        (!out.is_empty()).then_some(Value::Object(out))
    }
}

/// The method of a `.proto`'s service, written `Service/Method`.
pub fn proto_op<'a>(api: &'a Api, method: &str) -> Result<Op<'a>, Words> {
    let ApiDoc::Proto(pf) = &api.doc else { unreachable!("a .proto") };
    let not = || tr!("`{}` にメソッド `{method}` はありません。`Service/Method` と書いてください", "`{}` has no method `{method}`; write it as `Service/Method`", api.name);
    let (svc, m) = method.split_once('/').ok_or_else(not)?;
    let service = pf.services.iter().find(|s| s.name == svc || s.name.rsplit('.').next() == Some(svc)).ok_or_else(not)?;
    let found = service.methods.iter().find(|x| x.name == m).ok_or_else(not)?;
    Ok(Op {
        api,
        label: format!("{}/{}", service.name.rsplit('.').next().unwrap_or(&service.name), found.name),
        input: Node::Proto(PType::Named(found.input.clone()), FieldRules::default()),
        params: vec![],
        encodings: Value::Null,
        output: Some(Node::Proto(PType::Named(found.output.clone()), FieldRules::default())),
        errors: vec![],
        token: None,
        form: false,
        procedure: Some(format!("{}/{}", service.name, found.name)),
        streams: found.streams,
        presence_free: false,
        noun: an_answer,
    })
}

fn local(id: &str) -> &str {
    id.rsplit('#').next().unwrap_or(id)
}

/// A property of an OpenAPI document that marks a value secret (ritsu's DESIGN 16.6): the place in
/// the parameter or the answer it marks (dotted; empty: the value itself), the mark, and where the
/// mark is, as a JSON Pointer.
#[derive(Clone, Debug, PartialEq)]
pub struct DocMark {
    pub path: String,
    pub mark: ritsu_base::marks::SchemaMark,
    pub pointer: String,
}

/// What the three keywords of a schema say of it (`format`, `x-sensitive-data`,
/// `x-data-classification`), as ritsu-base's `schema_mark` reads them. `writeOnly` is not one.
fn schema_mark(v: &Value) -> Option<ritsu_base::marks::SchemaMark> {
    let format = v.get("format").and_then(Value::as_str);
    let sensitive = v.get("x-sensitive-data").is_some_and(|x| *x != Value::Bool(false) && !x.is_null());
    let class = v.get("x-data-classification").map(|c| (c.get("category").and_then(Value::as_str).unwrap_or(""), c.get("sensitivity").and_then(Value::as_str)));
    ritsu_base::marks::schema_mark(format, sensitive, class)
}

/// A key as a JSON Pointer writes it (RFC 6901): `~` as `~0`, `/` as `~1`.
fn pointer_key(k: &str) -> String {
    k.replace('~', "~0").replace('/', "~1")
}

/// Every marked property under `schema` (at `pointer` in `doc`), followed down by the type `t` the
/// flow reads or writes there: a property that is marked marks the place as a whole, and the places
/// under it are not looked into.
#[allow(clippy::too_many_arguments)]
fn marks_in(doc: &Value, schema: &Value, pointer: &str, m: &Model, t: &Ty, path: &mut Vec<String>, seen: &mut Vec<RecordId>, out: &mut Vec<DocMark>) {
    let (mut s, mut at) = (schema.clone(), pointer.to_string());
    for _ in 0..16 {
        if let Some(mark) = schema_mark(&s) {
            out.push(DocMark { path: path.join("."), mark, pointer: at });
            return;
        }
        let Some(r) = s.get("$ref").and_then(|r| r.as_str()).and_then(|r| r.strip_prefix('#')).map(String::from) else { break };
        s = doc.pointer(&r).cloned().unwrap_or(Value::Null);
        at = r;
    }
    for key in ["anyOf", "oneOf", "allOf"] {
        for (i, b) in s[key].as_array().into_iter().flatten().enumerate() {
            marks_in(doc, b, &format!("{at}/{key}/{i}"), m, t, path, seen, out);
        }
    }
    match t {
        Ty::Opt(x) => marks_in(doc, &s, &at, m, x, path, seen, out),
        Ty::List(x) if s.get("items").is_some() => marks_in(doc, &s["items"], &format!("{at}/items"), m, x, path, seen, out),
        Ty::Record(r) if !seen.contains(r) => {
            seen.push(*r);
            for (f, ft) in &m.records[*r].fields {
                if let Some(p) = s["properties"].get(f) {
                    path.push(f.clone());
                    marks_in(doc, p, &format!("{at}/properties/{}", pointer_key(f)), m, ft, path, seen, out);
                    path.pop();
                }
            }
            seen.pop();
        }
        _ => {}
    }
}

/// The marks of a `.proto`'s method (`Service/Method`) on what a `connect` task gives it and reads
/// from it: the fields of its request and its response that `debug_redact` marks, followed down by the
/// task's types. A parameter is a field of the request by its name or its JSON name.
#[allow(clippy::type_complexity)]
pub fn proto_marks(api: &Api, method: &str, m: &Model, task: &TaskDef) -> (Vec<(String, String, crate::proto::Redact)>, Vec<(String, crate::proto::Redact)>) {
    let ApiDoc::Proto(pf) = &api.doc else { return (vec![], vec![]) };
    let Some((svc, mname)) = method.split_once('/') else { return (vec![], vec![]) };
    let Some(found) = pf.services.iter().find(|s| s.name == svc || s.name.rsplit('.').next() == Some(svc)).and_then(|s| s.methods.iter().find(|x| x.name == mname)) else { return (vec![], vec![]) };
    fn walk(pf: &ProtoFile, msg: &str, m: &Model, t: &Ty, path: &mut Vec<String>, seen: &mut Vec<RecordId>, out: &mut Vec<(String, crate::proto::Redact)>) {
        let Ty::Record(r) = t.inner() else {
            if let Ty::List(x) = t.inner() {
                walk(pf, msg, m, x, path, seen, out);
            }
            return;
        };
        if seen.contains(r) {
            return;
        }
        seen.push(*r);
        for (f, ft) in &m.records[*r].fields {
            let Some(pfl) = pf.messages.get(msg).into_iter().flatten().find(|x| x.json == *f || x.name == *f) else { continue };
            path.push(f.clone());
            match (&pfl.redact, &pfl.ty) {
                (Some(rd), _) => out.push((path.join("."), rd.clone())),
                (None, PType::Named(n)) if pf.messages.contains_key(n) => walk(pf, n, m, ft, path, seen, out),
                _ => {}
            }
            path.pop();
        }
        seen.pop();
    }
    let mut params = Vec::new();
    for (p, pt) in &task.params {
        let Some(pfl) = pf.messages.get(&found.input).into_iter().flatten().find(|x| x.json == *p || x.name == *p) else { continue };
        match (&pfl.redact, &pfl.ty) {
            (Some(rd), _) => params.push((p.clone(), String::new(), rd.clone())),
            (None, PType::Named(n)) if pf.messages.contains_key(n) => {
                let mut inner = Vec::new();
                walk(pf, n, m, pt, &mut vec![], &mut vec![], &mut inner);
                params.extend(inner.into_iter().map(|(path, rd)| (p.clone(), path, rd)));
            }
            _ => {}
        }
    }
    let mut answer = Vec::new();
    if let Some(t) = &task.result {
        walk(pf, &found.output, m, t, &mut vec![], &mut vec![], &mut answer);
    }
    (params, answer)
}

/// The marks of the OpenAPI operation at `method` and `path` on what `task` gives it and reads from
/// it: for each parameter, those of the parameter of the path, the query, a header or a cookie, or of
/// the body's property, it is sent as (`TaskDef::wire`); and those of the answer. Read as far as the
/// task's types go.
pub fn openapi_marks(api: &Api, method: &str, path: &str, m: &Model, task: &TaskDef) -> (Vec<(String, DocMark)>, Vec<DocMark>) {
    let ApiDoc::OpenApi(doc) = &api.doc else { return (vec![], vec![]) };
    let op_at = format!("/paths/{}/{}", pointer_key(path), method.to_ascii_lowercase());
    let item = &doc["paths"][path];
    let op = &item[method.to_ascii_lowercase()];
    let (mut params, mut answer) = (Vec::new(), Vec::new());
    // the parameters of the four places, behind their `$ref`
    let mut listed: Vec<(Value, String)> = Vec::new();
    for (list, at) in [(&item["parameters"], format!("/paths/{}/parameters", pointer_key(path))), (&op["parameters"], format!("{op_at}/parameters"))] {
        for (i, p) in list.as_array().into_iter().flatten().enumerate() {
            let (mut p, mut at) = (p.clone(), format!("{at}/{i}"));
            if let Some(r) = p.get("$ref").and_then(|r| r.as_str()).and_then(|r| r.strip_prefix('#')).map(String::from) {
                p = doc.pointer(&r).cloned().unwrap_or(Value::Null);
                at = r;
            }
            listed.push((p, at));
        }
    }
    // the body's schema, behind the request body's `$ref`
    let (mut body, mut body_at) = (op["requestBody"].clone(), format!("{op_at}/requestBody"));
    if let Some(r) = body.get("$ref").and_then(|r| r.as_str()).and_then(|r| r.strip_prefix('#')).map(String::from) {
        body = doc.pointer(&r).cloned().unwrap_or(Value::Null);
        body_at = r;
    }
    let body_schema = ["application/json", "application/x-www-form-urlencoded"].iter().find_map(|ct| body["content"].get(*ct).map(|c| (c["schema"].clone(), format!("{body_at}/content/{}/schema", pointer_key(ct)))));
    for (p, pt) in &task.params {
        let mut found = Vec::new();
        let w = task.wire.iter().find(|w| w.param == *p);
        if let Some(w) = w.filter(|w| w.place != Place::Body) {
            let same = |q: &Value| q["in"] == w.place.word() && q["name"].as_str().is_some_and(|n| if w.place == Place::Header { n.eq_ignore_ascii_case(&w.name) } else { n == w.name });
            // the operation's take the place of the path item's of the same name and place
            if let Some((q, at)) = listed.iter().rev().find(|(q, _)| same(q)) {
                marks_in(doc, &q["schema"], &format!("{at}/schema"), m, pt, &mut vec![], &mut vec![], &mut found);
            }
        } else if let Some((s, at)) = &body_schema {
            // the property of the body the parameter is, through the body's `$ref`, `allOf`, `anyOf` and `oneOf`
            let mut bodies = vec![(s.clone(), at.clone())];
            while let Some((s, at)) = bodies.pop() {
                let (s, at) = match s.get("$ref").and_then(|r| r.as_str()).and_then(|r| r.strip_prefix('#')).map(String::from) {
                    Some(r) => (doc.pointer(&r).cloned().unwrap_or(Value::Null), r),
                    None => (s, at),
                };
                for key in ["allOf", "anyOf", "oneOf"] {
                    for (i, b) in s[key].as_array().into_iter().flatten().enumerate() {
                        bodies.push((b.clone(), format!("{at}/{key}/{i}")));
                    }
                }
                if let Some(prop) = s["properties"].get(p.as_str()) {
                    marks_in(doc, prop, &format!("{at}/properties/{}", pointer_key(p)), m, pt, &mut vec![], &mut vec![], &mut found);
                }
            }
        }
        params.extend(found.into_iter().map(|d| (p.clone(), d)));
    }
    if let (Some(t), false) = (&task.result, task.callback) {
        let responses = op["responses"].as_object().cloned().unwrap_or_default();
        if let Some((status, r)) = responses.iter().find(|(s, _)| s.starts_with('2')) {
            let (mut r, mut at) = (r.clone(), format!("{op_at}/responses/{}", pointer_key(status)));
            if let Some(x) = r.get("$ref").and_then(|x| x.as_str()).and_then(|x| x.strip_prefix('#')).map(String::from) {
                r = doc.pointer(&x).cloned().unwrap_or(Value::Null);
                at = x;
            }
            if let Some(c) = r["content"].get("application/json") {
                marks_in(doc, &c["schema"], &format!("{at}/content/application~1json/schema"), m, t, &mut vec![], &mut vec![], &mut answer);
            }
        }
    }
    (params, answer)
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

    /// Where each parameter of `task` goes and how it is written there (DESIGN 43), as the
    /// operation's description says: a parameter the description has in the path, the query, a
    /// header or a cookie goes there by the description's name (`same_name`), a property of the body
    /// to the body; one the description has nowhere goes where it went before, to the URL's
    /// `{placeholder}` of its name, to the query of a `GET` or a `DELETE` and to the body of the other
    /// methods, where `check` says the operation does not take it. With what the description says
    /// cannot be sent as it says, in both languages (E016).
    pub fn placement(&self, m: &Model, task: &TaskDef, path_params: &[String]) -> (Vec<WireParam>, Vec<Words>) {
        let (api, op) = (&self.api.name, &self.label);
        let mut out = Vec::new();
        let mut said = Vec::new();
        let get = matches!(task.binding, Some(Binding::Http { ref method, .. }) if method == "GET" || method == "DELETE");
        let body_members = match self.shape(&self.input) {
            Shape::Object(ms) => ms,
            _ => vec![],
        };
        let left_out = |d: &Param| d.place == Place::Header && HEADERS_LEFT_OUT.iter().any(|h| h.eq_ignore_ascii_case(&d.member.name));
        // the description's parameters the flow's go to: which, and from which
        let mut taken: Vec<(usize, &String)> = Vec::new();
        for (p, t) in &task.params {
            // where the description has it: its parameters of the four places, or else the body's
            // property (a parameter the URL's path takes went there before, whatever the body had)
            let mut found: Vec<(Option<usize>, Place, String)> = self.params.iter().enumerate().filter(|(_, d)| !left_out(d) && same_name(d.place, &d.member.name, p)).map(|(i, d)| (Some(i), d.place, d.member.name.clone())).collect();
            if found.is_empty() && !get {
                if let Some(mb) = body_members.iter().find(|x| x.name == *p || x.alias.as_deref() == Some(p)) {
                    found.push((None, Place::Body, mb.name.clone()));
                }
            }
            if found.len() > 1 {
                let (ja, en): (Vec<String>, Vec<String>) = found.iter().map(|(_, place, name)| { let Text { ja, en } = crate::wire::place_words(*place, self.form); (format!("{ja}の `{name}`"), format!("`{name}` in {en}")) }).unzip();
                said.push(tr!("引数 `{p}` は、`{api}` の {op} の{} のどれにも当たるので、どこに送るかが決まりません。記述の名前のどれか一つにだけ当たる名前にしてください", "the parameter `{p}` matches {} of `{api}` {op}, so where it goes cannot be told; give it a name that matches only one of them", ja.join(" と"); en.join(" and ")));
            }
            let w = match found.first().cloned() {
                Some((Some(i), place, name)) => {
                    let d = &self.params[i];
                    if let Some((_, other)) = taken.iter().find(|(j, _)| *j == i) {
                        let Text { ja, en } = crate::wire::place_words(place, self.form);
                        said.push(tr!("引数 `{other}` と `{p}` が、どちらも `{api}` の {op} の{ja}の `{name}` に当たります", "the parameters `{other}` and `{p}` both go to `{name}` in {en} of `{api}` {op}"));
                    }
                    taken.push((i, p));
                    let style = match d.style.as_deref() {
                        None => Style::default_at(place),
                        Some(s) => match Style::read(s) {
                            Some(s) => s,
                            None => {
                                said.push(tr!("`{api}` の {op} の `{name}` の style `{s}` は、OpenAPI に無い書き方です", "`{api}` {op} writes `{name}` in the style `{s}`, which OpenAPI does not have"));
                                Style::default_at(place)
                            }
                        },
                    };
                    let w = WireParam { param: p.clone(), place, name: name.clone(), style, explode: d.explode.unwrap_or(style == Style::Form) };
                    if d.content {
                        said.push(tr!("`{api}` の {op} は、`{name}` の書き方を `content`（メディアタイプ）で言っています。dandori が書くのは `schema` と `style` で言う書き方です", "`{api}` {op} says how `{name}` is written with `content` (a media type); dandori writes what `schema` and `style` say"));
                    } else if matches!(place, Place::Header | Place::Cookie) && !is_token(&name) {
                        let Text { ja, en } = crate::wire::place_words(place, self.form);
                        said.push(tr!("`{name}` は{ja}の名前に書けません（HTTP の名前に使える字は、英数字と `!#$%&'*+-.^_`|~` です）", "`{name}` cannot be the name of {en} (HTTP's names are letters, digits and `!#$%&'*+-.^_`|~`)"));
                    } else if place == Place::Header && HEADERS_OF_THE_CLIENT.iter().any(|h| h.eq_ignore_ascii_case(&name)) {
                        said.push(tr!("ヘッダー `{name}` は、HTTP のクライアントが自分で書くので、引数からは送れません", "the header `{name}` is one the HTTP client writes itself, which a parameter cannot set"));
                    } else if place == Place::Header && name.eq_ignore_ascii_case("Cookie") {
                        said.push(tr!("クッキーは、ヘッダー `Cookie` でなく、記述の `in: cookie` の引数として送ります", "a cookie goes as a parameter that is `in: cookie`, not as the header `Cookie`"));
                    } else if place == Place::Header && name.eq_ignore_ascii_case("Idempotency-Key") && task.key {
                        said.push(tr!("ヘッダー `{name}` には、`key` の冪等キーを dandori が入れます。引数 `{p}` を外してください", "dandori puts the idempotency key of `key` in the header `{name}`; leave out the parameter `{p}`"));
                    } else if let Some(why) = crate::wire::unwritable(m, &w, t, self.form) {
                        said.push(self.param_words(p, &why));
                    } else if t.inner() == &Ty::Json && matches!(self.shape(&d.member.node), Shape::List(_) | Shape::Object(_) | Shape::Map(_)) {
                        said.push(self.param_words(p, &tr!("`json` の値は JSON の文字列で送るので、`{name}` のリストやオブジェクトの書き方になりません。リストかレコードの型にしてください", "a `json` value goes as its JSON text, which is not how the list or the object `{name}` is written; declare it a list or a record")));
                    }
                    w
                }
                Some((None, _, name)) if self.form => {
                    // a property of a URL-encoded body, written as its Encoding Object says
                    let e = &self.encodings[name.as_str()];
                    let style = match e["style"].as_str() {
                        None => Style::Form,
                        Some(s) => match Style::read(s) {
                            Some(s) => s,
                            None => {
                                said.push(tr!("`{api}` の {op} の `{name}` の style `{s}` は、OpenAPI に無い書き方です", "`{api}` {op} writes `{name}` in the style `{s}`, which OpenAPI does not have"));
                                Style::Form
                            }
                        },
                    };
                    let w = WireParam { param: p.clone(), place: Place::Body, name: name.clone(), style, explode: e["explode"].as_bool().unwrap_or(style == Style::Form) };
                    let plain_text = e["contentType"].as_str().map_or(true, |c| c.trim().starts_with("text/plain"));
                    if !plain_text && e.get("style").is_none() && e.get("explode").is_none() {
                        let c = e["contentType"].as_str().unwrap_or_default();
                        said.push(tr!("`{api}` の {op} は、URL エンコードの本文の `{name}` を `{c}` で書くと言っています。dandori がそこに書くのは、style と explode で言う書き方です", "`{api}` {op} says `{name}` of the URL-encoded body is written as `{c}`; dandori writes there what style and explode say"));
                    } else if let Some(why) = crate::wire::unwritable(m, &w, t, true) {
                        said.push(self.param_words(p, &why));
                    }
                    w
                }
                Some((None, _, name)) => WireParam::plain(p, Place::Body, &name),
                // not in the description: where it went before, which `check` says the operation does not take
                None => match path_params.iter().find(|ph| same_name(Place::Path, ph, p)) {
                    Some(ph) => WireParam::plain(p, Place::Path, ph),
                    None if get => WireParam::plain(p, Place::Query, p),
                    None => WireParam::plain(p, Place::Body, p),
                },
            };
            out.push(w);
        }
        (out, said)
    }

    /// What the operation says of the task (E016): each difference, in both languages.
    pub fn check(&self, m: &Model, task: &TaskDef) -> Vec<Words> {
        let mut out = Vec::new();
        let api = &self.api.name;
        let op = &self.label;
        if self.streams {
            out.push(tr!("`{api}` の {op} はストリームです。`connect` で呼べるのは、一つ受け取って一つ返すメソッドです", "`{api}` {op} streams; `connect` calls a method that takes one message and answers one"));
            return out;
        }
        // a message nothing read has: what the task sends or reads cannot be held to it
        if let Shape::Unread(u) = self.shape(&self.input) {
            out.push(tr!("`{api}` の {op} が受け取る `{u}` が分かりません（それがあるはずのファイルを読めませんでした）。dandori は、`{}` の引数がそれに合っているかを確かめられません", "`{api}` {op} takes `{u}`, a type that is not known (a file it may be in was not read), so the parameters of `{}` cannot be held to it", task.name));
            return out;
        }
        // what goes: the path's and the query's parameters, and the body's
        let body_members = match self.shape(&self.input) {
            Shape::Object(ms) => Some(ms),
            _ => None,
        };
        for (p, pt) in &task.params {
            if task.callback && p == "MessageBody" && self.label == "SendMessage" {
                // the token goes in the message, which Step Functions writes out as JSON text
                if !matches!(pt.inner(), Ty::Record(_) | Ty::Json | Ty::Str) {
                    out.push(self.param_words(p, &tr!("`{}` は文字列ではありません", "`{}` is not a string", m.ty_name(pt))));
                }
                continue;
            }
            // where it goes (DESIGN 43): a parameter of the description's, or a property of the body
            let w = task.wire.iter().find(|w| w.param == *p);
            let place = w.map_or(Place::Body, |w| w.place);
            let found = match w {
                Some(w) if w.place != Place::Body => self.params.iter().find(|d| d.place == w.place && d.member.name == w.name).map(|d| d.member.clone()),
                _ => body_members.as_ref().and_then(|ms| ms.iter().find(|x| x.name == *p || x.alias.as_deref() == Some(p)).cloned()),
            };
            let Some(mb) = found else {
                if place == Place::Body && body_members.is_none() && matches!(self.input, Node::Any) {
                    continue;
                }
                out.push(tr!("`{api}` の {op} は `{p}` を受け取りません", "`{api}` {op} takes no `{p}`"));
                continue;
            };
            if mb.required && matches!(pt, Ty::Opt(_)) {
                match place {
                    Place::Header | Place::Cookie => {
                        let Text { ja, en } = crate::wire::place_words(place, self.form);
                        out.push(tr!("`{api}` の {op} には{ja} `{}` が要りますが、引数 `{p}` は無いことがあります", "`{api}` {op} needs {en} `{}`, and the parameter `{p}` may be absent", mb.name; mb.name));
                    }
                    _ => out.push(tr!("`{api}` の {op} には `{p}` が要りますが、引数は無いことがあります", "`{api}` {op} needs `{p}`, and the parameter may be absent")),
                }
                continue;
            }
            if let Err(w) = self.sends(m, pt, task.param_ranges.get(p).copied(), &mb.node, &mut vec![]) {
                out.push(self.param_words(p, &w));
            }
        }
        // what it needs: each parameter of the four places that some parameter goes to (an
        // `Idempotency-Key` header the key of `key` fills), and each property of the body
        for d in self.params.iter().filter(|d| d.member.required) {
            if d.place == Place::Header && HEADERS_LEFT_OUT.iter().any(|h| h.eq_ignore_ascii_case(&d.member.name)) {
                continue;
            }
            let passed = task.wire.iter().any(|w| w.place == d.place && w.name == d.member.name) || (d.place == Place::Header && d.member.name.eq_ignore_ascii_case("Idempotency-Key") && task.key);
            if passed {
                continue;
            }
            match d.place {
                Place::Header if d.member.name.eq_ignore_ascii_case("Idempotency-Key") => {
                    let Text { ja, en } = crate::wire::place_words(d.place, self.form);
                    out.push(tr!("`{api}` の {op} には{ja} `{}` が要りますが、`{}` はそれを渡しません。タスクに `key` を書けば、dandori が冪等キーをそこに入れます", "`{api}` {op} needs {en} `{}`, which `{}` does not pass; write `key` on the task and dandori puts its idempotency key there", d.member.name, task.name));
                }
                Place::Header | Place::Cookie => {
                    let Text { ja, en } = crate::wire::place_words(d.place, self.form);
                    out.push(tr!("`{api}` の {op} には{ja} `{}` が要りますが、`{}` はそれを渡しません", "`{api}` {op} needs {en} `{}`, which `{}` does not pass", d.member.name, task.name));
                }
                _ => out.push(tr!("`{api}` の {op} には `{}` が要りますが、`{}` はそれを渡しません", "`{api}` {op} needs `{}`, which `{}` does not pass", d.member.name, task.name)),
            }
        }
        let givens: Vec<&String> = task.params.iter().map(|(p, _)| p).chain(task.key_param.iter()).collect();
        for w in body_members.iter().flatten().filter(|x| x.required) {
            if !givens.iter().any(|g| **g == w.name || w.alias.as_deref() == Some(g.as_str())) {
                out.push(tr!("`{api}` の {op} には `{}` が要りますが、`{}` はそれを渡しません", "`{api}` {op} needs `{}`, which `{}` does not pass", w.name, task.name));
            }
        }
        // what comes back: a callback's answer comes from whoever answers it, not from the API
        if let (Some(t), Some(o), false) = (&task.result, &self.output, task.callback) {
            // every field of the answer's record that differs, and one difference below it
            let found = match (t, self.shape(o)) {
                (Ty::Record(r), Shape::Object(ms)) => self.fields(&ms, m, *r, &mut vec![]),
                _ => self.reads(o, m, t, task.result_range, &mut vec![]).err().into_iter().collect(),
            };
            for Text { en, ja } in found {
                out.push(tr!("`{api}` の {op} のレスポンスは `{}` に合いません。{ja}", "what `{api}` {op} answers is not `{}`: {en}", m.ty_name(t)));
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
                            out.push(tr!("`{api}` の {op} は {st} を返しません（`{}`）", "`{api}` {op} does not answer with {st} (`{}`)", e.name));
                        }
                    }
                }
                ApiDoc::Smithy(_) => {
                    let x = e.exception.as_deref().unwrap_or(&e.name);
                    if !self.errors.iter().any(|y| y == x) {
                        out.push(tr!("`{api}` の {op} にエラー `{x}` はありません（エラーは {}）", "`{api}` {op} has no error `{x}` (its errors are {})", self.errors.join("・"); self.errors.join(", ")));
                    }
                }
                ApiDoc::Proto(_) => {}
            }
        }
        if let (ApiDoc::Smithy(_), Some(kp)) = (&self.api.doc, &task.key_param) {
            if self.token.as_deref() != Some(kp.as_str()) {
                let Text { en, ja } = match &self.token {
                    Some(t) => tr!("それは `{t}` です", "it is `{t}`"),
                    None => tr!("この操作は冪等トークンを受け取りません", "it takes none"),
                };
                out.push(tr!("`{kp}` は `{api}` の {op} の冪等トークンではありません。{ja}", "`{kp}` is not the idempotency token of `{api}` {op}: {en}"));
            }
        }
        out
    }

    fn param_words(&self, p: &str, Text { en, ja }: &Words) -> Words {
        tr!("引数 `{p}` は、`{}` の {} が受け取るものと合いません。{ja}", "the parameter `{p}` is not what `{}` {} takes: {en}", self.api.name, self.label)
    }

    /// Whether every value of `t` (its numbers in `rg`) is one the API takes at `n`.
    fn sends(&self, m: &Model, t: &Ty, rg: Option<Range>, n: &Node, seen: &mut Vec<RecordId>) -> Result<(), Words> {
        let s = self.shape(n);
        let differ = || tr!("`{}` で、{}ではありません", "`{}` is not {}", m.ty_name(t), describe_ja(&s); m.ty_name(t), describe(&s));
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
            // a day goes as its text, `2026-11-16`: what a date takes, and what any string does
            (Ty::Date, Shape::Date | Shape::Str(None)) => Ok(()),
            // a string goes to a date as it is, as it did before a date was read as one
            (Ty::Str, Shape::Date) => Ok(()),
            // protobuf's JSON takes a 64-bit integer as a string, which is how a flow has it from an answer
            (Ty::Str, Shape::Int { text: true, .. }) => Ok(()),
            (Ty::Str, Shape::Str(Some(vs))) => Err(match days_hint(vs) {
                // no enum of a flow has a day for a value; a date held to the days does
                Some(Text { en, ja }) => tr!("受け取るのは {} のどれかですが、`string` はどんな値にもなりえます。{ja}", "it takes one of {}, and `string` can be anything; {en}", vs.join("・"); vs.join(", ")),
                None => tr!("受け取るのは {} のどれかですが、`string` はどんな値にもなりえます。列挙にしてください", "`{}` takes one of {}, and `string` can be anything; give it an enum", vs.join("・"); m.ty_name(t), vs.join(", ")),
            }),
            // what the flow sends: the values of the enum its range lets in, every value without one
            (Ty::Enum(e), Shape::Str(vs)) => match (vs, m.values_in(*e, rg).into_iter().find(|v| vs.as_ref().is_some_and(|vs| !vs.contains(v)))) {
                (_, Some(v)) => Err(tr!("`{}` の `{v}` が、受け取る値（{}）にありません", "`{v}` of `{}` is not one of the values it takes ({})", m.enums[*e].name, vs.as_ref().unwrap().join("・"); m.enums[*e].name, vs.as_ref().unwrap().join(", "))),
                _ => Ok(()),
            },
            // the days an API lists take a day held to a range of them; a range with more days than
            // the list has one it does not list among its first few, which is where the search stops
            (Ty::Date, Shape::Str(Some(vs))) => match rg.and_then(|r| Some((r.lo?, r.hi?))) {
                Some((lo, hi)) => match (lo..=hi).map(crate::model::day_text).find(|d| !vs.contains(d)) {
                    Some(d) => Err(tr!("`{d}` のことがあり、受け取る値（{}）にありません", "it may be `{d}`, which is not one of the values it takes ({})", vs.join("・"); vs.join(", "))),
                    None => Ok(()),
                },
                None => Err(match days_hint(vs) {
                    Some(Text { en, ja }) => tr!("`date` で、{} のどれかではありません。{ja}", "`date` is not one of {}; {en}", vs.join("・"); vs.join(", ")),
                    None => differ(),
                }),
            },
            (Ty::Int | Ty::Num(_), Shape::Int { min, max, .. }) => within(m, t, rg, *min, *max),
            (Ty::Int | Ty::Num(_), Shape::Float) => Ok(()),
            (Ty::Bool, Shape::Bool) => Ok(()),
            (Ty::Timestamp, Shape::Timestamp { .. }) | (Ty::Timestamp, Shape::Str(None)) => Ok(()),
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
                        return Err(tr!("`{f}`（`{}` のフィールド）を受け取りません", "it takes no `{f}` (a field of `{}`)", rd.name));
                    };
                    self.sends(m, ft, rd.ranges.get(f).copied(), &mb.node, seen).map_err(|w| field_words(f, w))?;
                }
                for mb in ms.iter().filter(|x| x.required) {
                    match rd.fields.iter().find(|(f, _)| *f == mb.name || mb.alias.as_deref() == Some(f)) {
                        Some((_, Ty::Opt(_))) | None => return Err(tr!("`{}` が要りますが、`{}` には無いことがあります", "it needs `{}`, which `{}` may not have", mb.name, rd.name)),
                        _ => {}
                    }
                }
                seen.pop();
                Ok(())
            }
            // a day and a moment are different texts: `2026-11-16` is not `2026-11-16T09:00:00Z`
            (_, Shape::Date) => Err(tr!("`{}` で、日付ではありません。`date` にしてください", "`{}` is not a date; declare it `date`", m.ty_name(t))),
            (_, Shape::Timestamp { .. }) => Err(tr!("`{}` で、日時ではありません。`timestamp` にしてください", "`{}` is not a date and time; declare it `timestamp`", m.ty_name(t))),
            _ => Err(differ()),
        }
    }

    /// Whether every value the API answers at `n` is one of `t` (its numbers in `rg`).
    fn reads(&self, n: &Node, m: &Model, t: &Ty, rg: Option<Range>, seen: &mut Vec<RecordId>) -> Result<(), Words> {
        let s = self.shape(n);
        let differ = || tr!("{}で、`{}` ではありません", "{} is not `{}`", describe_ja(&s), m.ty_name(t); describe(&s), m.ty_name(t));
        match (&s, t) {
            (_, Ty::Json) => Ok(()),
            (_, Ty::Opt(x)) => self.reads(n, m, x, rg, seen),
            (Shape::Choice(ns), _) => {
                for x in ns {
                    self.reads(x, m, t, rg, &mut seen.clone())?;
                }
                Ok(())
            }
            (Shape::Any, _) => Err(tr!("どんな値にもなりえますが、`{}` はそうではありません。`json` にしてください", "it can be anything, which `{}` is not; declare it `json`", m.ty_name(t))),
            (Shape::Unread(u), _) => Err(unknown_type(u)),
            (Shape::Str(_), Ty::Str) | (Shape::Bytes, Ty::Str) | (Shape::Int { text: true, .. }, Ty::Str) => Ok(()),
            // a day comes as its text, and so does a moment a description writes as a string
            (Shape::Date, Ty::Str) | (Shape::Timestamp { text: true }, Ty::Str) => Ok(()),
            (Shape::Date, Ty::Date) => Ok(()),
            // the days or the moments an API lists: each one the type takes, in its range
            (Shape::Str(Some(vs)), Ty::Date | Ty::Timestamp) => {
                let fits = |v: &str, rg: Option<Range>| crate::render::value_fits(m, &Value::String(v.to_string()), t, rg);
                match vs.iter().find(|v| !fits(v.as_str(), rg)) {
                    Some(v) if fits(v.as_str(), None) => {
                        let w = rg.map(|r| m.range_text(t, &r)).unwrap_or_default();
                        Err(tr!("`{v}` のことがあり、範囲 `{w}` の外です", "it may be `{v}`, outside `{w}`"))
                    }
                    Some(v) => Err(tr!("`{v}` のことがあり、`{}` はそれを受け取りません", "it may be `{v}`, which `{}` does not take", m.ty_name(t))),
                    None => Ok(()),
                }
            }
            (Shape::Str(Some(vs)), Ty::Enum(e)) => {
                let zero = self.proto_zero(n);
                // what the flow takes: the values of the enum its range lets in, every value without one
                let takes = m.values_in(*e, rg);
                match vs.iter().find(|v| !takes.contains(v) && Some(v.as_str()) != zero.as_deref()) {
                    Some(v) if m.enums[*e].values.contains(v) => {
                        let w = ends_text(&takes);
                        Err(tr!("`{v}` のことがあり、範囲 `{w}` の外です", "it may be `{v}`, outside `{w}`"))
                    }
                    Some(v) => Err(tr!("`{v}` のことがありますが、`{}` にその値はありません", "it may be `{v}`, which `{}` has no value for", m.enums[*e].name)),
                    None => Ok(()),
                }
            }
            (Shape::Str(None), Ty::Enum(e)) => Err(tr!("どんな文字列でもありえますが、`{}` は自分の値しか取りません。`string` にしてください", "it is any string, and `{}` takes only its values; declare it `string`", m.enums[*e].name)),
            (Shape::Int { text: true, .. }, Ty::Int | Ty::Num(_)) => Err(tr!("64 ビットの整数で、protobuf の JSON では文字列で届きます。`string` にしてください", "a 64-bit integer comes as a string in protobuf's JSON; declare it `string`")),
            (Shape::Int { min, max, .. }, Ty::Int | Ty::Num(_)) => match rg {
                Some(want) if (min.is_some() || max.is_some()) && !(Range { lo: *min, hi: *max }).within(&want) => {
                    let (w, a) = (m.range_text(t, &want), m.range_text(t, &Range { lo: *min, hi: *max }));
                    Err(tr!("`{w}` の外の `{a}` になりえます", "it can be `{a}`, outside `{w}`"))
                }
                _ => Ok(()),
            },
            (Shape::Float, Ty::Int | Ty::Num(_)) => Err(tr!("小数のことがあり、dandori にはその型がありません。`json` にしてください", "it may have a fraction, which dandori has no type for; declare it `json`")),
            (Shape::Bool, Ty::Bool) | (Shape::Timestamp { .. }, Ty::Timestamp) => Ok(()),
            (Shape::List(x), Ty::List(y)) => self.reads(x, m, y, rg, seen),
            (Shape::Object(ms), Ty::Record(r)) => match self.fields(ms, m, *r, seen).into_iter().next() {
                Some(w) => Err(w),
                None => Ok(()),
            },
            (Shape::Map(_), Ty::Record(_)) => Err(differ()),
            (Shape::Date, _) => Err(tr!("日付で、`{}` ではありません。`date` にしてください", "a date is not `{}`; declare it `date`", m.ty_name(t))),
            (Shape::Timestamp { .. }, _) => Err(tr!("日時で、`{}` ではありません。`timestamp` にしてください", "a date and time is not `{}`; declare it `timestamp`", m.ty_name(t))),
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
                let Text { en, ja } = (self.noun)();
                out.push(match other {
                    Some(o) => tr!("{ja}では `{f}` は `{}` という名前です", "{en} names `{f}` `{}`", o.name),
                    None => tr!("{ja}に `{f}`（`{}` のフィールド）はありません", "{en} has no `{f}` (a field of `{}`)", rd.name),
                });
                continue;
            };
            // at a workflow's entry, a field that may be left out is the workflow's to need or not
            if (mb.nullable || (mb.optional_out && !self.presence_free)) && !matches!(ft, Ty::Opt(_) | Ty::Json) {
                let Text { en, ja } = if mb.nullable { tr!("は null のことがあります", "may be null") } else { tr!("は無いことがあります", "may be left out") };
                out.push(tr!("`{f}` {ja}。`{}?` にしてください", "`{f}` {en}; declare it `{}?`", m.ty_name(ft)));
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

    /// For `connect`: where the request holds a `google.type.Date` (`days_of`), to which a flow's
    /// `date` goes as protobuf's JSON writes it; None when it holds none.
    pub fn request_days(&self) -> Option<Value> {
        let (ApiDoc::Proto(pf), Node::Proto(PType::Named(msg), _)) = (&self.api.doc, &self.input) else { return None };
        let t = days_of(pf, msg);
        has_days(&t).then_some(t)
    }
}

/// The zero values of the fields of the message `msg` that protobuf leaves out of JSON when they
/// are at them, and the same for the messages inside it: `{"f": {key: zero}, "m": {key: …}, "l": {key: …}}`,
/// with `"d": [key]` for the fields that are `google.type.Date`s, one or a list of them.
/// The names are the fields' JSON names. `fill` puts them back and reads the dates; it is what
/// reading a message of `msg` from JSON does.
pub fn zeros_of(pf: &ProtoFile, msg: &str) -> Value {
    zeros_in(pf, msg, &mut vec![])
}

fn zeros_in(pf: &ProtoFile, msg: &str, within: &mut Vec<String>) -> Value {
    let mut f = Map::new();
    let mut ms = Map::new();
    let mut ls = Map::new();
    let mut ds = Vec::new();
    if within.iter().any(|x| x == msg) {
        return json!({});
    }
    within.push(msg.to_string());
    for fl in pf.messages.get(msg).into_iter().flatten() {
        // a date is read as one, whatever a file of it that was read says
        let date = matches!(&fl.ty, PType::Named(n) if n == crate::proto::DATE);
        if date {
            ds.push(json!(fl.json));
        }
        let message = match &fl.ty {
            PType::Named(n) if pf.messages.contains_key(n) && !date => Some(n.clone()),
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
    if !ds.is_empty() {
        out.insert("d".into(), Value::Array(ds));
    }
    Value::Object(out)
}

/// Where the message `msg` holds a `google.type.Date`, as `zeros_of` says it, with nothing else:
/// `{"d": [key], "m": {key: …}, "l": {key: …}}`, and `{}` when it holds none. What `wire_days`
/// writes a flow's dates by. A map's values are not looked into: a flow reads a `map` as `json`.
pub fn days_of(pf: &ProtoFile, msg: &str) -> Value {
    days_in(pf, msg, &mut vec![])
}

fn days_in(pf: &ProtoFile, msg: &str, within: &mut Vec<String>) -> Value {
    if within.iter().any(|x| x == msg) {
        return json!({});
    }
    within.push(msg.to_string());
    let (mut ds, mut ms, mut ls) = (Vec::new(), Map::new(), Map::new());
    for fl in pf.messages.get(msg).into_iter().flatten() {
        match &fl.ty {
            PType::Named(n) if n == crate::proto::DATE => ds.push(json!(fl.json)),
            PType::Named(n) if pf.messages.contains_key(n) => {
                let sub = days_in(pf, n, within);
                if has_days(&sub) {
                    if fl.repeated {
                        ls.insert(fl.json.clone(), sub);
                    } else {
                        ms.insert(fl.json.clone(), sub);
                    }
                }
            }
            _ => {}
        }
    }
    within.pop();
    let mut out = Map::new();
    if !ds.is_empty() {
        out.insert("d".into(), Value::Array(ds));
    }
    if !ms.is_empty() {
        out.insert("m".into(), Value::Object(ms));
    }
    if !ls.is_empty() {
        out.insert("l".into(), Value::Object(ls));
    }
    Value::Object(out)
}

/// Whether `z` (`zeros_of`, `days_of`) names a `google.type.Date`, here or in a message inside.
pub fn has_days(z: &Value) -> bool {
    z["d"].as_array().is_some_and(|d| !d.is_empty()) || ["m", "l"].iter().any(|k| z[*k].as_object().is_some_and(|s| s.values().any(has_days)))
}

/// `z` without what it says of dates: the zero values alone.
pub fn without_days(z: &Value) -> Value {
    let Some(o) = z.as_object() else { return z.clone() };
    let mut out = Map::new();
    for (k, v) in o {
        match k.as_str() {
            "d" => {}
            "m" | "l" => {
                out.insert(k.clone(), Value::Object(v.as_object().into_iter().flatten().map(|(f, s)| (f.clone(), without_days(s))).collect()));
            }
            _ => {
                out.insert(k.clone(), v.clone());
            }
        }
    }
    Value::Object(out)
}

/// A `google.type.Date` as protobuf's JSON writes it, `{"year": 2026, "month": 11, "day": 16}`, read
/// as a flow's `date`, `2026-11-16`; a list of them item by item. A date short of its year, its month
/// or its day (protobuf's JSON leaves a 0 out: a card's month, a birthday without its year), one with
/// a field of another name, a number that is not whole or not in its range (a year from 1 to 9999, a
/// month to 12, a day to 31), and what is not an object, is left as it is, for the check that reads
/// it to refuse (DESIGN 44).
pub fn day_read(v: &Value) -> Value {
    match v {
        Value::Array(a) => Value::Array(a.iter().map(day_read).collect()),
        Value::Object(o) => {
            let part = |k: &str, hi: i64| o.get(k).and_then(whole).filter(|n| (1..=hi).contains(n));
            match (part("year", 9999), part("month", 12), part("day", 31)) {
                (Some(y), Some(m), Some(d)) if o.keys().all(|k| matches!(k.as_str(), "year" | "month" | "day")) => json!(format!("{y:04}-{m:02}-{d:02}")),
                _ => v.clone(),
            }
        }
        _ => v.clone(),
    }
}

/// A JSON number that is a whole number (`16`, and `16.0` as JavaScript reads it).
fn whole(v: &Value) -> Option<i64> {
    v.as_i64().or_else(|| v.as_f64().filter(|f| f.fract() == 0.0 && f.abs() < 1e15).map(|f| f as i64))
}

/// A flow's `date`, `2026-11-16`, as protobuf's JSON writes a `google.type.Date`: `{"year": 2026,
/// "month": 11, "day": 16}`; a list of them item by item. What is not four, two and two digits
/// between hyphens is left as it is, for the service to refuse (DESIGN 44).
pub fn day_wire(v: &Value) -> Value {
    match v {
        Value::Array(a) => Value::Array(a.iter().map(day_wire).collect()),
        Value::String(s) => {
            let b = s.as_bytes();
            let digits = |r: std::ops::Range<usize>| s.get(r).filter(|t| t.bytes().all(|c| c.is_ascii_digit())).and_then(|t| t.parse::<i64>().ok());
            match (b.len() == 10 && b[4] == b'-' && b[7] == b'-', digits(0..4), digits(5..7), digits(8..10)) {
                (true, Some(y), Some(m), Some(d)) => json!({ "year": y, "month": m, "day": d }),
                _ => v.clone(),
            }
        }
        _ => v.clone(),
    }
}

/// `v` with the dates where `t` says (`days_of`, or `zeros_of`, whose zero values it passes over)
/// written as protobuf's JSON writes `google.type.Date` (`day_wire`), in it, in its messages and in
/// the messages of its lists: what a `connect` task sends. A field that is not there stays out.
pub fn wire_days(v: &Value, t: &Value) -> Value {
    let Some(o) = v.as_object() else { return v.clone() };
    let mut out = o.clone();
    for k in t["d"].as_array().into_iter().flatten().filter_map(Value::as_str) {
        if let Some(x) = out.get(k).cloned() {
            out.insert(k.to_string(), day_wire(&x));
        }
    }
    for (k, sub) in t["m"].as_object().into_iter().flatten() {
        if let Some(x) = out.get(k).filter(|x| x.is_object()).cloned() {
            out.insert(k.clone(), wire_days(&x, sub));
        }
    }
    for (k, sub) in t["l"].as_object().into_iter().flatten() {
        if let Some(a) = out.get(k).and_then(|x| x.as_array()).cloned() {
            out.insert(k.clone(), Value::Array(a.iter().map(|x| wire_days(x, sub)).collect()));
        }
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
/// leaves them out, and read the `google.type.Date`s it names as dates (`day_read`; a date that is
/// one already stays): in the answer, in its messages, and in the messages of its lists.
pub fn fill(v: &Value, zeros: &Value) -> Value {
    let Some(o) = v.as_object() else { return v.clone() };
    let mut out = o.clone();
    for (k, z) in zeros["f"].as_object().into_iter().flatten() {
        if out.get(k).is_none_or(|x| x.is_null()) {
            out.insert(k.clone(), z.clone());
        }
    }
    for k in zeros["d"].as_array().into_iter().flatten().filter_map(Value::as_str) {
        if let Some(x) = out.get(k).cloned() {
            out.insert(k.to_string(), day_read(&x));
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
/// A date is written as `{year, month, day}` (`day_wire`).
pub fn sparse(v: &Value, zeros: &Value) -> Value {
    let Some(o) = v.as_object() else { return v.clone() };
    let mut out = o.clone();
    for (k, z) in zeros["f"].as_object().into_iter().flatten() {
        if !z.is_array() && !z.is_object() {
            out.remove(k);
        }
    }
    for k in zeros["d"].as_array().into_iter().flatten().filter_map(Value::as_str) {
        if let Some(x) = out.get(k).cloned() {
            out.insert(k.to_string(), day_wire(&x));
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
/// its zero value, in the message, in its messages and in the messages of its lists, and a date as
/// `{year, month, day}` (`day_wire`). What `fill` reads back: `fill(&omit_zeros(v, z), z)` is `v`
/// when `v` has every field `z` names.
pub fn omit_zeros(v: &Value, zeros: &Value) -> Value {
    let Some(o) = v.as_object() else { return v.clone() };
    let mut out = o.clone();
    for (k, z) in zeros["f"].as_object().into_iter().flatten() {
        if out.get(k) == Some(z) {
            out.remove(k);
        }
    }
    for k in zeros["d"].as_array().into_iter().flatten().filter_map(Value::as_str) {
        if let Some(x) = out.get(k).cloned() {
            out.insert(k.to_string(), day_wire(&x));
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

fn field_words(f: &str, Text { en, ja }: Words) -> Words {
    tr!("`{f}` は、{ja}", "in `{f}`, {en}")
}

/// What differs where a `.proto` names a type that no file read has: nothing can be said of it
/// but that, and `json` takes whatever it is.
fn unknown_type(n: &str) -> Words {
    tr!("型が `{n}` ですが、それがあるはずのファイルを読めなかったので、その型が分かりません。`json` にすれば受け渡せます", "its type `{n}` is not known, since a file it may be in was not read; `json` takes it")
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
    Some(tr!("{ja_list} を読めなかったので、そこにある型は使えません", "{en_list} could not be read, so the types in {en_them} cannot be used"))
}

/// What to declare where an API lists the days it takes (`vs`, when each is a day): a `date` held
/// to them when they follow one another, else to a run of them.
fn days_hint(vs: &[String]) -> Option<Words> {
    let mut ds: Vec<i64> = vs.iter().map(|v| crate::computed::day_of(v)).collect::<Option<_>>()?;
    ds.sort_unstable();
    ds.dedup();
    let (lo, hi) = (*ds.first()?, *ds.last()?);
    Some(if hi - lo + 1 == ds.len() as i64 {
        let (a, b) = (crate::model::day_text(lo), crate::model::day_text(hi));
        tr!("`date  range >={a} <={b}` にしてください", "declare it `date  range >={a} <={b}`")
    } else {
        tr!("挙げた日のうち、続いている日だけを範囲にした `date` にしてください", "declare it `date` held to a range of days it lists, one after another")
    })
}

/// Whether the numbers a flow sends in `rg` (of the type `t`) are all in what the API takes, from
/// `min` to `max`. Both ranges are said in `t`'s unit: the API takes the whole numbers the flow
/// sends on the wire.
fn within(m: &Model, t: &Ty, rg: Option<Range>, min: Option<i64>, max: Option<i64>) -> Result<(), Words> {
    if min.is_none() && max.is_none() {
        return Ok(());
    }
    let want = Range { lo: min, hi: max };
    let w = m.range_text(t, &want);
    match rg {
        Some(h) if h.within(&want) => Ok(()),
        Some(h) => {
            let h = m.range_text(t, &h);
            Err(tr!("`{h}` で、受け取るのは `{w}` です", "it is `{h}`, and `{w}` is taken"))
        }
        None => Err(tr!("範囲が書かれていませんが、受け取るのは `{w}` です", "it has no range, and `{w}` is taken")),
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
        Shape::Timestamp { .. } => "a date and time".into(),
        Shape::Date => "a date".into(),
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
        Shape::Int { text: true, .. } => "文字列で受け渡す 64 ビットの整数".into(),
        Shape::Int { .. } => "整数".into(),
        Shape::Float => "数".into(),
        Shape::Bool => "真偽値".into(),
        Shape::Timestamp { .. } => "日時".into(),
        Shape::Date => "日付".into(),
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
        // a string that lists its values takes those, whatever its format says of them
        "string" if s["enum"].is_array() => Shape::Str(s["enum"].as_array().map(|vs| vs.iter().filter_map(|v| v.as_str().map(String::from)).collect())),
        "string" if s["format"] == "date-time" => Shape::Timestamp { text: true },
        "string" if s["format"] == "date" => Shape::Date,
        "string" => Shape::Str(None),
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
            "Timestamp" => Shape::Timestamp { text: false },
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
        "timestamp" => Shape::Timestamp { text: false },
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
            "google.protobuf.Timestamp" => Shape::Timestamp { text: true },
            // `{year, month, day}` in protobuf's JSON, which the generated code reads as a date and writes from one
            crate::proto::DATE => Shape::Date,
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
    /// `google.type.Date`, which protobuf's JSON writes as `{year, month, day}` (DESIGN 44)
    Date,
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
    /// the field is marked secret (`debug_redact`)
    pub redact: Option<crate::proto::Redact>,
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
            crate::proto::DATE => MadeTy::Date,
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
            MadeField { name: fl.json.clone(), ty, redact: fl.redact.clone() }
        })
        .collect()
}

/// `MemberTier` → `MEMBER_TIER`: an enum's name as buf's lint wants it in front of its values'
/// (`MEMBER_TIER_GOLD`). A capital that is not the first letter and does not follow an underscore
/// starts a new word.
pub fn upper_snake(camel: &str) -> String {
    let mut out = String::new();
    for (i, ch) in camel.chars().enumerate() {
        if ch.is_ascii_uppercase() && i > 0 && !out.ends_with('_') {
            out.push('_');
        }
        out.push(ch.to_ascii_uppercase());
    }
    out
}

/// The values of the enum `en` (full name) as a flow has them, which are their names in the `.proto`.
/// The zero value is left out when its name says nothing was set: with the enum's own name
/// (`Stock` is `STOCK_`) taken off the front, it is `unspecified` in any case. A zero value with
/// another name is a value, and stays.
pub fn made_enum_values(pf: &ProtoFile, en: &str) -> Vec<String> {
    let values = pf.enums.get(en).cloned().unwrap_or_default();
    let simple = en.rsplit('.').next().unwrap_or(en);
    let prefix = format!("{}_", upper_snake(simple));
    values.into_iter().enumerate().filter(|(i, v)| !(*i == 0 && v.strip_prefix(&prefix).unwrap_or(v).eq_ignore_ascii_case("unspecified"))).map(|(_, v)| v).collect()
}

/// What a task is held to, of a type made from the message or enum `full` (E016's two directions):
/// what is sent as `t` must be what the description takes, and what the description answers must be
/// readable as `t`. Empty when the type made agrees with its description.
pub fn made_fits(api: &Api, full: &str, m: &Model, t: &Ty) -> Vec<Words> {
    let ApiDoc::Proto(_) = &api.doc else { return vec![] };
    let node = Node::Proto(PType::Named(full.to_string()), FieldRules::default());
    let op = Op { api, label: full.to_string(), input: node.clone(), params: vec![], encodings: Value::Null, output: Some(node.clone()), errors: vec![], token: None, form: false, procedure: None, streams: false, presence_free: false, noun: an_answer };
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
fn a_message() -> Text {
    tr!("メッセージ", "the message")
}

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
        Op { api: self.api, label, input: node.clone(), params: vec![], encodings: Value::Null, output: Some(node), errors: vec![], token: None, form: false, procedure: None, streams: false, presence_free, noun: a_message }
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

    /// Whether the field `json` is a `google.type.Date`, or a message that holds one at any depth
    /// (`days_of`): a list of them too.
    pub fn holds_day(&self, json: &str) -> bool {
        let t = days_of(self.pf(), &self.name);
        t["d"].as_array().is_some_and(|d| d.iter().any(|x| x == json)) || ["m", "l"].iter().any(|k| t[*k].get(json).is_some())
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
            return Err(tr!("無いことがあります。`{}?` にしてください", "it may be left out; declare it `{}?`", m.ty_name(t)));
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
                None => out.push(tr!("`{}` がありません", "it has no `{}`", t.json)),
                Some(f) => {
                    let (a, b) = (field_text(self.pf(), f), field_text(other.pf(), t));
                    if a != b {
                        out.push(tr!("`{}` は `{a}` ですが、`{theirs_name}` では `{b}` です", "`{}` is `{a}`, and in `{theirs_name}` it is `{b}`", f.json));
                    }
                }
            }
        }
        for f in mine.iter().filter(|f| !theirs.iter().any(|t| t.json == f.json)) {
            out.push(tr!("`{theirs_name}` に無い `{}` があります", "it has `{}`, which `{theirs_name}` does not", f.json));
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
    fn upper_snake_is_bufs_spelling() {
        assert_eq!(upper_snake("Carrier"), "CARRIER");
        assert_eq!(upper_snake("MemberTier"), "MEMBER_TIER");
        assert_eq!(upper_snake("CaptureMethod"), "CAPTURE_METHOD");
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
        crate::proto::load_text("t.proto", text).unwrap_or_else(|e| panic!("{}", e.en))
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
    fn a_string_is_a_day_or_a_moment_as_its_format_says() {
        let doc = json!({"components": {"schemas": {"Day": {"type": "string", "format": "date"}}}});
        let shape = |s: Value| openapi_shape(&doc, &s);
        assert!(matches!(shape(json!({"type": "string", "format": "date"})), Shape::Date));
        assert!(matches!(shape(json!({"type": ["string", "null"], "format": "date"})), Shape::Date));
        assert!(matches!(shape(json!({"$ref": "#/components/schemas/Day"})), Shape::Date));
        assert!(matches!(shape(json!({"type": "string", "format": "date-time"})), Shape::Timestamp { text: true }));
        // the values it lists are the ones it takes, whatever its format says of them
        assert!(matches!(shape(json!({"type": "string", "format": "date", "enum": ["2026-12-28"]})), Shape::Str(Some(vs)) if vs == ["2026-12-28"]));
        // a format dandori has no type for is any string: a time of day, an address
        assert!(matches!(shape(json!({"type": "string", "format": "time"})), Shape::Str(None)));
        assert!(matches!(shape(json!({"type": "string", "format": "email"})), Shape::Str(None)));
        // protobuf's JSON writes a Timestamp as RFC 3339's text; a Smithy timestamp goes as its protocol says
        let pf = load(r#"syntax = "proto3"; package a.v1; import "google/protobuf/timestamp.proto"; message M { google.protobuf.Timestamp at = 1; }"#);
        assert!(matches!(proto_shape(&pf, &PType::Named("google.protobuf.Timestamp".into()), &FieldRules::default()), Shape::Timestamp { text: true }));
        assert!(matches!(smithy_shape(&json!({"shapes": {}}), "smithy.api#Timestamp"), Shape::Timestamp { text: false }));
        assert!(matches!(smithy_shape(&json!({"shapes": {"a#At": {"type": "timestamp"}}}), "a#At"), Shape::Timestamp { text: false }));
    }

    #[test]
    fn listed_days_are_sent_as_a_date_held_to_them() {
        let days = |vs: &[&str]| days_hint(&vs.iter().map(|v| v.to_string()).collect::<Vec<_>>()).map(|w| w.en);
        // days one after another, in any order and once each: the range of them
        assert_eq!(days(&["2026-12-29", "2026-12-28", "2026-12-28"]).as_deref(), Some("declare it `date  range >=2026-12-28 <=2026-12-29`"));
        assert_eq!(days(&["2026-12-31"]).as_deref(), Some("declare it `date  range >=2026-12-31 <=2026-12-31`"));
        // days with a gap: a run of them
        assert_eq!(days(&["2026-12-28", "2026-12-31"]).as_deref(), Some("declare it `date` held to a range of days it lists, one after another"));
        // a list with a value that is no day, or a day no month has, is no list of days
        assert_eq!(days(&["2026-12-28", "closed"]), None);
        assert_eq!(days(&["2026-02-30"]), None);
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

    /// A trip with `google.type.Date`s in it: one, a list, in a message, in a list of messages, and
    /// as a map's values, which a flow reads as `json`. `Plain` holds a message, and no date.
    const TRIP: &str = r#"syntax = "proto3"; package a.v1; import "google/type/date.proto";
        message Leg { google.type.Date day = 1; string note = 2; }
        message Trip { google.type.Date start = 1; repeated google.type.Date stops = 2; Leg first = 3; repeated Leg legs = 4; map<string, google.type.Date> by_name = 5; string id = 6; }
        message Note { string text = 1; }
        message Plain { string id = 1; Note none = 2; }
        service Trips { rpc Book(Trip) returns (Trip); rpc Name(Plain) returns (Plain); }"#;

    #[test]
    fn a_google_type_date_is_a_flows_date() {
        let pf = load(TRIP);
        let made = made_fields(&pf, "a.v1.Trip");
        let ty = |name: &str| made.iter().find(|f| f.name == name).unwrap_or_else(|| panic!("no field {name}")).ty.clone();
        assert_eq!(ty("start"), MadeTy::Opt(Box::new(MadeTy::Date)));
        assert_eq!(ty("stops"), MadeTy::List(Box::new(MadeTy::Date)));
        assert_eq!(ty("byName"), MadeTy::Json);
        assert!(matches!(proto_shape(&pf, &PType::Named(crate::proto::DATE.into()), &FieldRules::default()), Shape::Date));
        // where the dates are, and the zero values beside them; a map's values are not looked into
        let z = zeros_of(&pf, "a.v1.Trip");
        assert_eq!(z["d"], json!(["start", "stops"]));
        assert_eq!(z["m"]["first"], json!({"f": {"note": ""}, "d": ["day"]}));
        assert_eq!(z["l"]["legs"], json!({"f": {"note": ""}, "d": ["day"]}));
        let days = days_of(&pf, "a.v1.Trip");
        assert_eq!(days, json!({"d": ["start", "stops"], "m": {"first": {"d": ["day"]}}, "l": {"legs": {"d": ["day"]}}}));
        assert_eq!(days_of(&pf, "a.v1.Plain"), json!({}));
        assert!(has_days(&z) && !has_days(&zeros_of(&pf, "a.v1.Plain")));
        assert_eq!(without_days(&z), json!({"f": {"stops": [], "legs": [], "byName": {}, "id": ""}, "m": {"first": {"f": {"note": ""}}}, "l": {"legs": {"f": {"note": ""}}}}));
        // what the service writes reads as the flow's dates, and the flow's dates go out as it writes them
        let wire = json!({"start": {"year": 2026, "month": 11, "day": 16}, "stops": [{"year": 2026, "month": 1, "day": 2}], "first": {"day": {"year": 2026, "month": 12, "day": 31}}, "legs": [{"day": {"year": 1, "month": 1, "day": 1}, "note": "x"}], "id": "t"});
        let flow = json!({"start": "2026-11-16", "stops": ["2026-01-02"], "first": {"day": "2026-12-31", "note": ""}, "legs": [{"day": "0001-01-01", "note": "x"}], "byName": {}, "id": "t"});
        assert_eq!(fill(&wire, &z), flow);
        // the zero values left out, the dates as the service writes them
        assert_eq!(omit_zeros(&flow, &z), wire);
        assert_eq!(fill(&omit_zeros(&flow, &z), &z), flow);
        assert_eq!(wire_days(&flow, &days), json!({"start": {"year": 2026, "month": 11, "day": 16}, "stops": [{"year": 2026, "month": 1, "day": 2}], "first": {"day": {"year": 2026, "month": 12, "day": 31}, "note": ""}, "legs": [{"day": {"year": 1, "month": 1, "day": 1}, "note": "x"}], "byName": {}, "id": "t"}));
        // a date that is one already stays, and a field that is not there stays out
        assert_eq!(fill(&json!({"start": "2026-11-16"}), &z)["start"], json!("2026-11-16"));
        assert_eq!(wire_days(&json!({"id": "t"}), &days), json!({"id": "t"}));
        // what a `connect` task sends: where its request holds a date, and nothing for one that holds none
        let api = Api { name: "trips".into(), doc: ApiDoc::Proto(Arc::new(pf)), url: None };
        assert_eq!(proto_op(&api, "Trips/Book").unwrap().request_days(), Some(days));
        assert_eq!(proto_op(&api, "Trips/Name").unwrap().request_days(), None);
        let trip = message(&api, "a.v1.Trip");
        assert!(trip.holds_day("start") && trip.holds_day("stops") && trip.holds_day("first") && trip.holds_day("legs"));
        assert!(!trip.holds_day("by_name") && !trip.holds_day("byName") && !trip.holds_day("id"));
        assert!(!message(&api, "a.v1.Plain").holds_day("none"));
    }

    #[test]
    fn a_date_short_of_a_part_is_left_for_the_check_to_refuse() {
        let read = |v: Value| day_read(&v);
        assert_eq!(read(json!({"year": 2026, "month": 11, "day": 16})), json!("2026-11-16"));
        assert_eq!(read(json!({"year": 9999, "month": 12, "day": 31})), json!("9999-12-31"));
        // a whole number as JavaScript writes it, and the fields in another order
        assert_eq!(read(json!({"day": 16.0, "year": 2026.0, "month": 11})), json!("2026-11-16"));
        // protobuf's JSON leaves a 0 out: a card's month without its day, a birthday without its year, no day at all
        for partial in [json!({"year": 2026, "month": 11}), json!({"month": 12, "day": 25}), json!({"year": 2026}), json!({})] {
            assert_eq!(read(partial.clone()), partial);
        }
        // a 0 written out, a number out of its range or not whole, a field of another name, what is not a number
        for odd in [
            json!({"year": 0, "month": 11, "day": 16}),
            json!({"year": 2026, "month": 13, "day": 1}),
            json!({"year": 2026, "month": 11, "day": 32}),
            json!({"year": 10000, "month": 1, "day": 1}),
            json!({"year": 2026, "month": 11.5, "day": 1}),
            json!({"year": 2026, "month": 11, "day": 16, "era": 1}),
            json!({"year": "2026", "month": 11, "day": 16}),
            json!({"year": true, "month": 11, "day": 16}),
            json!("2026-11-16"),
            json!(null),
        ] {
            assert_eq!(read(odd.clone()), odd);
        }
        // a list item by item
        assert_eq!(read(json!([{"year": 2026, "month": 2, "day": 3}, {"month": 2}])), json!(["2026-02-03", {"month": 2}]));
        // a day a month does not have is the check's to refuse, as for any date
        assert_eq!(read(json!({"year": 2026, "month": 2, "day": 30})), json!("2026-02-30"));
        // and the other way: four, two and two digits between hyphens, and nothing else
        let wire = |v: Value| day_wire(&v);
        assert_eq!(wire(json!("2026-11-16")), json!({"year": 2026, "month": 11, "day": 16}));
        assert_eq!(wire(json!("0001-01-01")), json!({"year": 1, "month": 1, "day": 1}));
        for odd in [json!("2026-1-16"), json!("2026/11/16"), json!("2026-11-16T00:00:00Z"), json!("２０２６-11-16"), json!(16), json!(null)] {
            assert_eq!(wire(odd.clone()), odd);
        }
    }

    /// A Smithy model of one service under `protocol`, with the operation `Put` that takes `members`
    /// (`Entry` holds a timestamp and a list of itself; `Stamp` is a timestamp that says its form).
    fn smithy(protocol: &str, members: Value) -> Api {
        let doc = json!({"smithy": "2.0", "shapes": {
            "a#Service": {"type": "service", "operations": [{"target": "a#Put"}], "traits": {protocol: {}}},
            "a#Put": {"type": "operation", "input": {"target": "a#PutInput"}},
            "a#PutInput": {"type": "structure", "members": members},
            "a#Time": {"type": "timestamp"},
            "a#Stamp": {"type": "timestamp", "traits": {"smithy.api#timestampFormat": "http-date"}},
            "a#Entry": {"type": "structure", "members": {"At": {"target": "a#Time"}, "Note": {"target": "smithy.api#String"}, "More": {"target": "a#Entries"}}},
            "a#Entries": {"type": "list", "member": {"target": "a#Entry"}},
            "a#Times": {"type": "list", "member": {"target": "smithy.api#Timestamp"}},
            "a#ByName": {"type": "map", "key": {"target": "smithy.api#String"}, "value": {"target": "a#Time"}},
            "a#Plain": {"type": "structure", "members": {"Note": {"target": "smithy.api#String"}}}
        }});
        Api { name: "a".into(), doc: ApiDoc::Smithy(doc), url: None }
    }

    fn times(api: &Api, params: &[&str]) -> Option<Value> {
        smithy_op(api, "put").unwrap().timestamps(&params.iter().map(|p| p.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn a_timestamp_goes_in_the_form_its_model_says() {
        let members = json!({
            "At": {"target": "a#Time"},
            "Since": {"target": "a#Time", "traits": {"smithy.api#httpHeader": "X-Since"}},
            "From": {"target": "a#Time", "traits": {"smithy.api#httpQuery": "from"}},
            "Day": {"target": "a#Time", "traits": {"smithy.api#httpLabel": {}}},
            "Kept": {"target": "a#Time", "traits": {"smithy.api#timestampFormat": "date-time"}},
            "Stamp": {"target": "a#Stamp"},
            "Plain": {"target": "smithy.api#Timestamp"},
            "Note": {"target": "smithy.api#String"}
        });
        let all = ["At", "Since", "From", "Day", "Kept", "Stamp", "Plain", "Note"];
        // the body takes the protocol's form; the headers, the query and the path theirs, under a
        // protocol that binds members there; the member's or the shape's @timestampFormat wins
        assert_eq!(
            times(&smithy("aws.protocols#restJson1", members.clone()), &all),
            Some(json!({"At": "epoch-seconds", "Since": "http-date", "From": "date-time", "Day": "date-time", "Kept": "date-time", "Stamp": "http-date", "Plain": "epoch-seconds"}))
        );
        assert_eq!(
            times(&smithy("aws.protocols#restXml", members.clone()), &all),
            Some(json!({"At": "date-time", "Since": "http-date", "From": "date-time", "Day": "date-time", "Kept": "date-time", "Stamp": "http-date", "Plain": "date-time"}))
        );
        // a protocol without bindings sends every member in the body
        for (protocol, body) in [("aws.protocols#awsJson1_0", "epoch-seconds"), ("aws.protocols#awsJson1_1", "epoch-seconds"), ("aws.protocols#awsQuery", "date-time"), ("aws.protocols#ec2Query", "date-time"), ("smithy.protocols#rpcv2Cbor", "epoch-seconds")] {
            assert_eq!(
                times(&smithy(protocol, members.clone()), &all),
                Some(json!({"At": body, "Since": body, "From": body, "Day": body, "Kept": "date-time", "Stamp": "http-date", "Plain": body})),
                "{protocol}"
            );
        }
        // under a protocol dandori does not know, the SDK says
        assert_eq!(times(&smithy("a#another", members.clone()), &["At", "Kept"]), Some(json!({"At": "timestamp", "Kept": "date-time"})));
        // only the members the task gives
        assert_eq!(times(&smithy("aws.protocols#restJson1", members.clone()), &["From", "Note"]), Some(json!({"From": "date-time"})));
        assert_eq!(times(&smithy("aws.protocols#restJson1", members), &["Note"]), None);
    }

    #[test]
    fn the_timestamps_inside_a_member_are_found_where_they_are() {
        let members = json!({
            "Entries": {"target": "a#Entries"},
            "Entry": {"target": "a#Entry"},
            "Times": {"target": "a#Times", "traits": {"smithy.api#httpQuery": "t"}},
            "ByName": {"target": "a#ByName"},
            "Plain": {"target": "a#Plain"}
        });
        // in each item of a list (`*`), in a structure, and in a structure that holds itself, once;
        // in each value of a map; a list in the query takes the query's form
        assert_eq!(
            times(&smithy("aws.protocols#restJson1", members), &["Entries", "Entry", "Times", "ByName", "Plain"]),
            Some(json!({"Entries": {"*": {"At": "epoch-seconds"}}, "Entry": {"At": "epoch-seconds"}, "Times": {"*": "date-time"}, "ByName": {"*": "epoch-seconds"}}))
        );
        // a model other than Smithy's has none
        let api = Api { name: "o".into(), doc: ApiDoc::OpenApi(json!({"paths": {"/x": {"post": {"responses": {"200": {}}}}}})), url: None };
        assert_eq!(openapi_op(&api, "POST", "/x").unwrap().timestamps(&["x".into()]), None);
    }
}
