//! The `.proto` files a flow reads (proto3): the package, the messages and enums with their
//! fields, and the services' methods, which a `connect` task is held to. Of the options it keeps
//! what dandori uses: a field's `json_name` and its Protovalidate rules (`buf.validate.field`), and
//! the options of services and methods (dandori's own marks, `dandori.v1.workflow` and the rest).
//!
//! Each file is read by ritsu's one reader of `.proto` files (`ritsu_proto`, ritsu's DESIGN 4.10),
//! and the names of types are resolved by protobuf's rule over the files a file can see: itself,
//! what it imports, and what those pass on with `import public` (`ritsu_proto::Protos`). What is
//! dandori's own is what it makes of that: it reads proto3 only, it finds a file through its
//! sources (the disk, through ritsu_base::fs, which ritsu's playground holds in memory), and with an
//! import that was not read it keeps a name it cannot resolve as written.
//!
//! Google's well-known types, `buf/validate/validate.proto` and dandori's
//! `dandori/v1/options.proto` are known without their files; any other import is read from the
//! root of the importing file's module when the file sits where its package says (buf's layout),
//! else from the file's directory. An import that is on the disk in neither place is passed over
//! and told in `ProtoFile::unread` (a `.proto` imports `google/api/annotations.proto` for options,
//! and the types a flow uses are often not in it): what such a file holds is not known, and a type
//! of it that a message or a method names stays a name no message or enum has.

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
    /// the field is marked secret: `debug_redact`, written on it or on the enum value of a custom
    /// option it is given (ritsu's DESIGN 16.6)
    pub redact: Option<Redact>,
}

/// How a field of a message is marked secret (ritsu's DESIGN 16.6): the file of the field, as dandori
/// reaches it, its line, and the mark as written; for a custom option, the value and where the value
/// is marked `debug_redact = true`.
#[derive(Clone, Debug, PartialEq)]
pub struct Redact {
    pub file: PathBuf,
    pub line: usize,
    /// `debug_redact = true`, or the option with its value: `(acme.v1.sensitivity) = PERSONAL`
    pub mark: String,
    pub by: Option<(String, PathBuf, usize)>,
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
pub use ritsu_proto::WELL_KNOWN;

/// The key protobuf's JSON gives a field: its name in lowerCamelCase.
pub use ritsu_proto::json_name;

/// The files read so far, by the name `ritsu_proto::Protos` knows each by, and the imports that
/// were not read.
#[derive(Default)]
struct Reading {
    ps: ritsu_proto::Protos,
    /// the file each name stands for, by its canonical path, so that a file is read once
    seen: Vec<(PathBuf, String)>,
    unread: Vec<String>,
    /// each file's path as dandori reached it, by the name the file is known by
    reached: BTreeMap<String, PathBuf>,
}

pub fn load(path: &Path) -> Result<ProtoFile, String> {
    let mut rd = Reading::default();
    let first = read(path, None, &mut rd)?;
    resolve_all(&first, rd)
}

/// A `.proto` given as text, read as if its file were `name`, in the current directory: for what
/// is tried without a file. The imports other than the well-known ones are read from the disk.
pub fn load_text(name: &str, text: &str) -> Result<ProtoFile, String> {
    let mut rd = Reading::default();
    let first = read(Path::new(name), Some(text), &mut rd)?;
    resolve_all(&first, rd)
}

/// Read one file (from its text when it is given) and every file it imports, each before the
/// files it imports and those in the order written: the name the file is known by.
fn read(path: &Path, text: Option<&str>, rd: &mut Reading) -> Result<String, String> {
    let canon = if text.is_some() { path.to_path_buf() } else { crate::sources::canonical(path) };
    if let Some((_, name)) = rd.seen.iter().find(|(c, _)| *c == canon) {
        return Ok(name.clone());
    }
    let name = canon.to_string_lossy().to_string();
    rd.seen.push((canon, name.clone()));
    rd.reached.insert(name.clone(), path.to_path_buf());
    let src = match text {
        Some(t) => t.to_string(),
        None => crate::sources::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?,
    };
    let f = ritsu_proto::read(&name, &src).map_err(|e| format!("{}:{}:{}: {}", path.display(), e.line, e.col, e.message("dandori").en))?;
    // dandori reads proto3; a file that says nothing of its syntax is read as such
    if f.syntax_line.is_some() && f.syntax != "proto3" {
        return Err(if f.syntax.starts_with("edition") {
            format!("{}: editions are not read; dandori reads proto3", path.display())
        } else {
            format!("{}: `{}` is not read; dandori reads proto3", path.display(), f.syntax)
        });
    }
    let imports = f.imports.clone();
    let package = f.package.clone();
    rd.ps.add(f);
    let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
    let mut at = Vec::new();
    for imp in &imports {
        let imp = &imp.path;
        if imp.starts_with("google/protobuf/") || imp == "buf/validate/validate.proto" {
            at.push(None);
            continue;
        }
        if imp == OPTIONS_IMPORT {
            at.push(Some(read(Path::new(OPTIONS_IMPORT), Some(DANDORI_OPTIONS), rd)?));
            continue;
        }
        // the file is where an import is looked for, or it is passed over; a file that is there and
        // cannot be read as a `.proto` is the trouble
        match import_dirs(&dir, &package).iter().map(|d| d.join(imp)).find(|p| crate::sources::read(p).is_ok()) {
            Some(p) => at.push(Some(read(&p, None, rd)?)),
            None => {
                if !rd.unread.contains(imp) {
                    rd.unread.push(imp.clone());
                }
                rd.ps.unread.insert(name.clone());
                at.push(None);
            }
        }
    }
    rd.ps.imports.insert(name.clone(), at);
    Ok(name)
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

/// The options of an element as dandori reads them: a tree under each option's name.
fn options(opts: &[ritsu_proto::Opt]) -> BTreeMap<String, Value> {
    match ritsu_proto::value::tree(opts) {
        ritsu_base::json::Json::Obj(kv) => kv.iter().map(|(k, v)| (k.clone(), json(v))).collect(),
        _ => BTreeMap::new(),
    }
}

/// JSON as serde_json holds it: a whole number as one, a fraction as a double.
fn json(j: &ritsu_base::json::Json) -> Value {
    use ritsu_base::json::Json;
    match j {
        Json::Null => Value::Null,
        Json::Bool(b) => Value::Bool(*b),
        Json::Int(n) => match (i64::try_from(*n), u64::try_from(*n)) {
            (Ok(i), _) => Value::from(i),
            (_, Ok(u)) => Value::from(u),
            _ => Value::String(n.to_string()),
        },
        Json::Frac(t) => t.parse::<f64>().ok().and_then(serde_json::Number::from_f64).map(Value::Number).unwrap_or_else(|| Value::String(t.clone())),
        Json::Str(s) => Value::String(s.clone()),
        Json::Arr(a) => Value::Array(a.iter().map(json).collect()),
        Json::Obj(kv) => Value::Object(kv.iter().map(|(k, v)| (k.clone(), json(v))).collect::<Map<String, Value>>()),
    }
}

/// Every message, enum and service of the files read, with the names of their types resolved
/// from the file that writes them. With an import that was not read, a name nothing has may be in
/// it: it stays as written, a type that is not known, and is told where a flow comes to it.
fn resolve_all(first: &str, rd: Reading) -> Result<ProtoFile, String> {
    use ritsu_proto::{Label, Resolved, Type};
    let Reading { ps, unread, reached, .. } = rd;
    let reached_as = |name: &str| reached.get(name).cloned().unwrap_or_else(|| PathBuf::from(name));
    let lenient = !unread.is_empty();
    let resolve = |file: &str, scope: &str, name: &str| -> Result<String, String> {
        match ps.resolve(file, scope, name) {
            Resolved::Found(s) => Ok(s.full),
            Resolved::Known(k) if WELL_KNOWN.contains(&k.as_str()) => Ok(k),
            _ if lenient => Ok(name.trim_start_matches('.').to_string()),
            _ if name.starts_with('.') => Err(format!("there is no type `{name}`")),
            _ => Err(format!("there is no type `{name}` (in `{scope}`)")),
        }
    };
    let mut f = ProtoFile { package: ps.files[first].package.clone(), unread, ..ProtoFile::default() };
    for file in &ps.order {
        let pf = &ps.files[file];
        for e in &pf.enums {
            f.enums.insert(pf.full(&e.name), e.values.iter().map(|v| v.name.clone()).collect());
        }
    }
    // every message by its full name; a name two files declare is the one read last
    let mut messages: BTreeMap<String, (&str, &ritsu_proto::Message)> = BTreeMap::new();
    for file in &ps.order {
        let pf = &ps.files[file];
        for m in &pf.messages {
            messages.insert(pf.full(&m.name), (file.as_str(), m));
        }
    }
    for (scope, (file, m)) in messages {
        {
            let mut fields = Vec::new();
            for x in &m.fields {
                let named = |n: &str| resolve(file, &scope, n);
                let ty = match &x.ty {
                    Type::Scalar(s) => PType::Scalar(s.clone()),
                    Type::Named(n) => PType::Named(named(n)?),
                    Type::Map(k, v) => PType::Map(
                        Box::new(PType::Scalar(k.clone())),
                        Box::new(match v.as_ref() {
                            Type::Named(n) => PType::Named(named(n)?),
                            Type::Scalar(s) | Type::Map(s, _) => PType::Scalar(s.clone()),
                        }),
                    ),
                };
                let repeated = x.label == Label::Repeated && !matches!(x.ty, Type::Map(..));
                // a singular message field says whether it is set
                let message = matches!(&ty, PType::Named(n) if !repeated && !f.enums.contains_key(n) && n != "google.protobuf.NullValue");
                let rules = options(&x.options).remove("buf.validate.field").unwrap_or(Value::Null);
                let redact = ps.redaction(file, x).map(|r| match r {
                    ritsu_proto::Redaction::Direct { line } => Redact { file: reached_as(file), line, mark: "debug_redact = true".into(), by: None },
                    ritsu_proto::Redaction::ByOption { option, value, file: vf, line: vl } => Redact { file: reached_as(file), line: x.line, mark: format!("{option} = {value}"), by: Some((value, reached_as(&vf), vl)) },
                });
                fields.push(PField { name: x.name.clone(), json: x.json(), ty, repeated, presence: x.label == Label::Optional || x.oneof.is_some() || message, rules, redact });
            }
            f.messages.insert(scope, fields);
        }
    }
    for file in &ps.order {
        let pf = &ps.files[file];
        let imports_options = pf.imports.iter().any(|i| i.path == OPTIONS_IMPORT);
        for s in &pf.services {
            let mut methods = Vec::new();
            for m in &s.methods {
                methods.push(Method {
                    name: m.name.clone(),
                    input: resolve(file, &pf.package, &m.input)?,
                    output: resolve(file, &pf.package, &m.output)?,
                    streams: m.client_streaming || m.server_streaming,
                    options: options(&m.options),
                });
            }
            f.services.push(Service { name: pf.full(&s.name), methods, options: options(&s.options), imports_options });
        }
    }
    Ok(f)
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

    /// A directory of files for a test, named for it, removed when the test is done with it.
    fn scratch(name: &str, files: &[(&str, &str)]) -> ritsu_testkit::TempDir {
        let dir = ritsu_testkit::TempDir::new(&format!("proto-{name}"));
        for (path, text) in files {
            dir.write(path, text);
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
        let f = load(&dir.path().join("shop/v1/order.proto")).unwrap_or_else(|e| panic!("{e}"));
        assert!(f.messages.contains_key("common.v1.Money"));
        assert_eq!(f.messages["shop.v1.Order"][0].ty, PType::Named("common.v1.Money".into()));
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
        let f = load(&dir.path().join("specs/order.proto")).unwrap_or_else(|e| panic!("{e}"));
        assert!(f.messages.contains_key("shop.v1.Money"));
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
        let f = load(&dir.path().join("specs/order.proto")).unwrap_or_else(|e| panic!("{e}"));
        // in the order they were met; the well-known file is known without its file, so it is not among them
        assert_eq!(f.unread, ["google/api/annotations.proto", "google/type/money.proto"]);
        // what is read stays as it is; a type of an import that was not read stays a name nothing has
        let order = &f.messages["shop.v1.Order"];
        assert_eq!(order[1].ty, PType::Named("google.type.Money".into()));
        assert!(!f.knows("google.type.Money") && f.knows("shop.v1.Order") && f.knows("google.protobuf.Timestamp"));
        assert_eq!(f.services[0].methods[0].input, "shop.v1.Order");
        // a name nothing has, with every import read, is still the trouble
        let all = scratch("typo", &[("specs/order.proto", r#"syntax = "proto3"; package shop.v1; message Order { Moneyy total = 1; }"#)]);
        let e = load(&all.path().join("specs/order.proto")).unwrap_err();
        assert!(e.contains("there is no type `Moneyy`"), "{e}");
        // a file that is there and is not a `.proto` is the trouble too
        let bad = scratch("bad", &[("specs/order.proto", r#"syntax = "proto3"; package shop.v1; import "money.proto"; message Order {}"#), ("specs/money.proto", "message {")]);
        assert!(load(&bad.path().join("specs/order.proto")).is_err());
    }
}
