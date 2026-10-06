//! What a `.proto` holds, as the reader gives it: every element with its line, every option as
//! written, and what Protovalidate says of a field and of a message.

use crate::validate::{MsgRules, Rules};
use crate::value::Value;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProtoFile {
    /// From the root.
    pub path: String,
    /// `proto2` (when the file does not say), `proto3`, or `edition 2023`.
    pub syntax: String,
    /// The line of the `syntax` or `edition` statement, when the file has one.
    pub syntax_line: Option<usize>,
    pub package: String,
    pub imports: Vec<Import>,
    /// The file's own options (`go_package`, …).
    pub options: Vec<Opt>,
    /// Every message, nested ones too (`Order.Line`), in the order they start.
    pub messages: Vec<Message>,
    /// Every enum, nested ones too (`Order.Status`), in the order they start.
    pub enums: Vec<Enum>,
    pub services: Vec<Service>,
    /// The fields of every `extend` (custom options among them), at the top of the file and
    /// inside messages, in the order written.
    pub extensions: Vec<Extension>,
}

/// A field of an `extend` (`extend google.protobuf.FieldOptions { Sensitivity sensitivity =
/// 50001; }`): a custom option, when what it extends is one of `descriptor.proto`'s options.
#[derive(Clone, Debug, PartialEq)]
pub struct Extension {
    /// What it extends, as written: "google.protobuf.FieldOptions".
    pub extendee: String,
    /// From the package, as a message's name is: `sensitivity`, or `Holder.sensitivity` for one
    /// declared in the message `Holder`.
    pub name: String,
    pub ty: Type,
    pub number: i64,
    pub line: usize,
}

/// How a field is marked to be redacted (DESIGN 16.6): protobuf's `debug_redact`, which C++'s
/// protobuf (from v30) keeps out of the debug formats, and which ritsu reads as the contract
/// saying the value is secret.
#[derive(Clone, Debug, PartialEq)]
pub enum Redaction {
    /// `[debug_redact = true]` on the field: the field's line.
    Direct { line: usize },
    /// A custom option of the field (`[(acme.v1.sensitivity) = PERSONAL]`) whose value is an enum
    /// value marked `[debug_redact = true]`: the option as written, the value, and where the
    /// value is (its file, from the root, and its line).
    ByOption { option: String, value: String, file: String, line: usize },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Import {
    /// As written.
    pub path: String,
    pub line: usize,
    pub col: usize,
    pub public: bool,
    pub weak: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Message {
    /// From the package: `Order`, `Order.Line`.
    pub name: String,
    pub line: usize,
    /// The fields in the order written, the members of a `oneof` among them.
    pub fields: Vec<Field>,
    /// The message's own `option` statements.
    pub options: Vec<Opt>,
    /// The `oneof`s it declares.
    pub oneofs: Vec<Oneof>,
    /// What Protovalidate asks of the message as a whole.
    pub rules: MsgRules,
}

/// A `oneof` as declared: its members and its own options (`(buf.validate.oneof).required`).
#[derive(Clone, Debug, PartialEq)]
pub struct Oneof {
    pub name: String,
    pub line: usize,
    pub fields: Vec<String>,
    pub options: Vec<Opt>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Label {
    None,
    Optional,
    Required,
    Repeated,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Type {
    /// `string`, `int32`, …
    Scalar(String),
    /// A message or an enum, as written (`warehouse.v1.ReserveResponse`, `Line`).
    Named(String),
    /// `map<K, V>`: the key is a scalar.
    Map(String, Box<Type>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub name: String,
    pub number: i64,
    pub label: Label,
    pub ty: Type,
    /// The `json_name` option (the last, when it is written twice).
    pub json_name: Option<String>,
    /// The `oneof` it is a member of.
    pub oneof: Option<String>,
    pub line: usize,
    /// The options in its `[…]`, in the order written.
    pub options: Vec<Opt>,
    /// What Protovalidate lets through here (`(buf.validate.field)`).
    pub rules: Rules,
}

impl Field {
    /// The name protobuf's JSON writes the field under: its `json_name`, or its name in
    /// lowerCamelCase ([`json_name`]).
    pub fn json(&self) -> String {
        self.json_name.clone().unwrap_or_else(|| json_name(&self.name))
    }

    /// Whether the field says if it is set (protobuf's explicit presence): written `optional`, a
    /// member of a `oneof`, or a singular field of a message type. `is_enum` says whether a
    /// named type is an enum (which the file alone does not know).
    pub fn has_presence(&self, is_enum: impl Fn(&str) -> bool) -> bool {
        self.label == Label::Optional
            || self.oneof.is_some()
            || matches!(&self.ty, Type::Named(n) if self.label != Label::Repeated && !is_enum(n))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Enum {
    pub name: String,
    pub line: usize,
    pub values: Vec<EnumValue>,
    /// The enum's own `option` statements (`allow_alias`).
    pub options: Vec<Opt>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnumValue {
    pub name: String,
    pub number: i64,
    pub line: usize,
    pub options: Vec<Opt>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Service {
    pub name: String,
    pub line: usize,
    pub methods: Vec<Method>,
    pub options: Vec<Opt>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Method {
    pub name: String,
    pub input: String,
    pub output: String,
    pub client_streaming: bool,
    pub server_streaming: bool,
    pub line: usize,
    pub options: Vec<Opt>,
}

/// One option: `(dandori.v1.workflow) = {name: "引当と発送", version: 1}`, or `json_name = "注文"`
/// in a field's `[…]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Opt {
    /// As written: `(dandori.v1.workflow)`, `(buf.validate.field).int64.gte`, `json_name`.
    pub name: String,
    /// The value as written: `{name: "引当と発送", version: 1}`.
    pub text: String,
    /// The value read as protobuf's text format, or None when it does not read as one.
    pub value: Option<Value>,
}

impl ProtoFile {
    pub fn message(&self, name: &str) -> Option<&Message> {
        self.messages.iter().find(|m| m.name == name)
    }

    pub fn enumeration(&self, name: &str) -> Option<&Enum> {
        self.enums.iter().find(|e| e.name == name)
    }

    pub fn service(&self, name: &str) -> Option<&Service> {
        self.services.iter().find(|s| s.name == name)
    }

    /// `warehouse.v1.Stock` for `Stock`.
    pub fn full(&self, name: &str) -> String {
        if self.package.is_empty() { name.to_string() } else { format!("{}.{name}", self.package) }
    }
}

/// protobuf's rule for a field's JSON name: an underscore is dropped and the letter after it is
/// capitalised, so `weight_g` is `weightG` and `order_lines` is `orderLines`.
pub fn json_name(field: &str) -> String {
    let mut out = String::new();
    let mut up = false;
    for c in field.chars() {
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
