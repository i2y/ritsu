//! Reading a `.proto` for the one thing a table depends on: which values an enum has (§15.59).
//!
//! The value set of an enum is usually not the rule's to decide. It is declared somewhere
//! else — a contract between systems, another file, often another repository — and it can
//! change without the rule changing. The tools that guard that contract guard its **shape**:
//! adding a value to an enum is a
//! compatible change, and on the wire it is. It is also the change that leaves a table with
//! no row for the new value, and the generated code answering from the default row for a tier
//! nobody priced. Nothing in the proto toolchain can see that, because the answer is not in
//! the proto.
//!
//! So a rule names where its enum comes from and `rulec check` holds the two together:
//!
//! ```text
//! import proto "api/v1/order.proto" MemberTier -> 会員区分
//! enum 会員区分(tier) = 一般(basic) | ゴールド(gold) | プラチナ(platinum) default
//! ```
//!
//! The `.proto` decides **which values exist**; the `.rule` decides **what they are called
//! here** and what each one costs. Neither can be derived from the other — a proto has no
//! Japanese in it, and a rule has no authority over the contract. What this module does is
//! read the set; holding the two together is `enums.rs`, which does it the same way for
//! every format.
//!
//! The file is read by ritsu's one reader of `.proto` files (`ritsu_proto`, ritsu's DESIGN
//! 4.10), the same one dandori and sakai read with. This module takes from what it reads what a
//! rule asks of a contract: the enums and their values (§15.59), the messages and their fields,
//! as far as a `from` path needs them (§15.125), and on a field, the Protovalidate rules that
//! bound which values pass (§15.132). The rules that relate fields to each other — CEL on a
//! message or a field, a `oneof` — are kept as text here and made into a condition by `cel` and
//! `projection` (§15.140). Services and every other option are passed over. The file is a
//! contract, not a program. A file the reader cannot read is not read in part: what a rule is
//! held to is not taken from half a contract (§15.166).


/// One value of an enum in a `.proto`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Value {
    /// As written, prefix and all: `MEMBER_TIER_GOLD`.
    pub name: String,
    pub number: i64,
}

/// One enum declared in a `.proto`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enum {
    pub name: String,
    pub values: Vec<Value>,
}

impl Enum {
    /// The ASCII aliases this enum asks a rule for, in declaration order.
    pub fn aliases(&self) -> Vec<String> {
        self.named().into_iter().map(|(a, _)| a).collect()
    }

    /// Each value a table answers for, with the alias it asks the rule to declare for it, in
    /// declaration order.
    ///
    /// Two conventions are read, and only two. The value names carry the enum's own name as a
    /// prefix (`MemberTier` → `MEMBER_TIER_`), which `buf lint` enforces and which is there to
    /// keep C++ scoping from colliding; it is not part of the value. And the zero value is
    /// proto3's "not set", so `MEMBER_TIER_UNSPECIFIED` is not a value a table answers for —
    /// the generated code refuses it at the door like any other input that is not a member.
    ///
    /// A zero value named anything else is **not** dropped. A file that puts a real value at 0
    /// is unusual, and quietly deciding it means nothing is the one thing this must not do.
    pub fn named(&self) -> Vec<(String, &Value)> {
        self.values.iter().filter(|v| !self.is_unset(v)).map(|v| (self.local(v).to_ascii_lowercase(), v)).collect()
    }

    /// The value proto3 reads as "not set", when the file has one: number 0, named
    /// `…_UNSPECIFIED`. A field left out of a message arrives as this value.
    pub fn unset(&self) -> Option<&Value> {
        self.values.iter().find(|v| self.is_unset(v))
    }

    fn is_unset(&self, v: &Value) -> bool {
        v.number == 0 && self.local(v).eq_ignore_ascii_case("unspecified")
    }

    /// A value's name with the enum's prefix taken off, when what is left is a name of its own.
    ///
    /// It is not when it begins with a digit — `SIZE_60` would leave `60`, which no alias can
    /// be — or when it is another value's whole name, which would make two values one. The
    /// name then stays as written, prefix and all, which is also what protobuf-py does with it.
    fn local<'v>(&self, v: &'v Value) -> &'v str {
        let prefix = format!("{}_", upper_snake(&self.name));
        match v.name.strip_prefix(prefix.as_str()) {
            Some(rest) if rest.starts_with(|c: char| c.is_ascii_alphabetic()) && !self.values.iter().any(|o| o.name == rest) => rest,
            _ => &v.name,
        }
    }
}

/// One field of a message, as far as a path needs it: its name, the word that names its
/// type, and whether it is a collection (§15.125).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    /// The type as written — `string`, `int64`, or a message name, qualified or not.
    pub ty: String,
    pub repeated: bool,
    /// Written `optional`: proto3's explicit presence, where an unset field is absent rather
    /// than zero.
    pub optional: bool,
    /// What Protovalidate lets through here, as far as it was read (§15.132).
    pub rules: Rules,
    /// The `json_name` option, when the field sets one (§15.133).
    pub json_name: Option<String>,
    /// The `oneof` it is a member of. A member has presence of its own, like an `optional`
    /// field, and `optional` is set on it too (§15.140).
    pub oneof: Option<String>,
}

impl Field {
    /// The name protojson writes this field under: its `json_name`, or the lowerCamelCase of
    /// its name (§15.133). Readers of protojson accept the name as written too.
    pub fn json(&self) -> String {
        self.json_name.clone().unwrap_or_else(|| json_name(&self.name))
    }
}

/// protobuf's own rule for a field's JSON name: an underscore is dropped and the letter after
/// it is capitalised, so `weight_g` is `weightG` and `order_lines` is `orderLines`.
pub use ritsu_proto::json_name;

/// The Protovalidate rules on one field that decide which values pass: `(buf.validate.field)`
/// in the field's options, in either spelling — `.int64.gte = 1` or `.int64 = {gte: 1}`
/// (§15.132).
///
/// Only the rules that bound a value the way a rule's input is bounded are read: the integer
/// comparisons, the element count of a collection, the listed values of a string, and the CEL
/// expressions, which are kept as text for `cel` to read (§15.140). Anything else that could
/// narrow what passes — a predefined rule, a pattern — is recorded by name in `unread` and not
/// interpreted. Reading it as not there makes the field
/// look wider than it is, never narrower: the check that uses this may then speak where it
/// did not need to, and never stays quiet where it should have spoken.
///
/// The reader reads them (`ritsu_proto::validate`); this is its type, under the names rulec has
/// always used.
pub use ritsu_proto::validate::{IntRules, Rules};

/// The values an integer kind can hold on the wire.
pub fn int_bounds(ty: &str) -> Option<(i128, i128)> {
    match ty.rsplit('.').next().unwrap_or(ty) {
        "int32" | "sint32" | "sfixed32" => Some((i32::MIN as i128, i32::MAX as i128)),
        "uint32" | "fixed32" => Some((0, u32::MAX as i128)),
        "int64" | "sint64" | "sfixed64" => Some((i64::MIN as i128, i64::MAX as i128)),
        "uint64" | "fixed64" => Some((0, u64::MAX as i128)),
        _ => None,
    }
}

/// One message declared in a `.proto`, named as it is written. A nested message is listed
/// under its own short name as well, which is how a path names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub name: String,
    pub fields: Vec<Field>,
    /// What Protovalidate asks of the message as a whole (§15.140).
    pub rules: MsgRules,
}

/// The rules on a message rather than on one field: `(buf.validate.message)` in its options,
/// and the `oneof`s it declares.
pub use ritsu_proto::validate::{MsgRules, OneofRule as Oneof};

/// The kinds a proto scalar travels as. A rule's numbers are whole in their declared unit
/// (§2.1), so the floating kinds are named apart rather than folded in with the integers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scalar {
    Str,
    Int,
    Frac,
    Bool,
}

/// The kind a proto type word travels as, or `None` for a message or an enum. An enum is a
/// message-shaped name here and resolves to nothing, which is right: what a rule takes from
/// an enum is its value set (§15.59), and that is `import proto`'s business, not a path's.
pub fn scalar(ty: &str) -> Option<Scalar> {
    match ty.rsplit('.').next().unwrap_or(ty) {
        "string" => Some(Scalar::Str),
        "bool" => Some(Scalar::Bool),
        "double" | "float" => Some(Scalar::Frac),
        "int32" | "int64" | "uint32" | "uint64" | "sint32" | "sint64" | "fixed32" | "fixed64"
        | "sfixed32" | "sfixed64" => Some(Scalar::Int),
        // `bytes` is not a kind a rule's input can be, and saying so is better than calling
        // it a string: the two do not travel the same way.
        _ => None,
    }
}

/// Whether a message name written one way is the one written another. A `.proto` may name a
/// message bare, package-qualified or nested-qualified, and a `shape` line may name it any
/// of those ways; the last segment is what they always agree on.
pub fn same_message(declared: &str, wanted: &str) -> bool {
    let last = |s: &str| s.rsplit('.').next().unwrap_or(s).to_string();
    declared == wanted || last(declared) == last(wanted)
}

/// What rulec reads of one `.proto`: the package, the imports as written, every enum, and every
/// message with the fields a path can follow. A nested enum or message is listed under its own
/// short name, which is how a rule names it; the order is the file's.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct File {
    pub package: Option<String>,
    pub imports: Vec<String>,
    pub enums: Vec<Enum>,
    pub messages: Vec<Message>,
}

/// Read a `.proto` with ritsu's reader and take what a rule asks of it, or say where the file
/// does not read (§15.166). `path` names the file in what the reader says.
///
/// Of a message, a `map` field is passed over rather than guessed at, so a path into one gets
/// stuck instead of being waved through; so is a proto2 `required` field, which is not a field a
/// rule's path names. A member of a `oneof` has presence of its own, as an `optional` field has.
pub fn read(path: &str, src: &str) -> Result<File, ritsu_proto::ReadError> {
    use ritsu_proto::{Label, Type};
    let f = ritsu_proto::read(path, src)?;
    let short = |n: &str| n.rsplit('.').next().unwrap_or(n).to_string();
    Ok(File {
        package: (!f.package.is_empty()).then(|| f.package.clone()),
        imports: f.imports.iter().map(|i| i.path.clone()).collect(),
        enums: f.enums.iter().map(|e| Enum { name: short(&e.name), values: e.values.iter().map(|v| Value { name: v.name.clone(), number: v.number }).collect() }).collect(),
        messages: f
            .messages
            .iter()
            .map(|m| Message {
                name: short(&m.name),
                fields: m
                    .fields
                    .iter()
                    .filter(|x| !matches!(x.ty, Type::Map(..)) && x.label != Label::Required)
                    .map(|x| Field {
                        name: x.name.clone(),
                        ty: match &x.ty {
                            Type::Scalar(s) | Type::Named(s) => s.clone(),
                            Type::Map(..) => unreachable!("a map is passed over"),
                        },
                        repeated: x.label == Label::Repeated,
                        optional: x.label == Label::Optional || x.oneof.is_some(),
                        rules: x.rules.clone(),
                        json_name: x.options.iter().find_map(|o| match &o.value {
                            Some(ritsu_proto::Value::Str(s)) if o.name.split_whitespace().collect::<String>() == "json_name" => Some(s.clone()),
                            _ => None,
                        }),
                        oneof: x.oneof.clone(),
                    })
                    .collect(),
                rules: m.rules.clone(),
            })
            .collect(),
    })
}

/// Why a file did not read, as the note of the diagnostic that names it: where, and what the
/// reader found there.
pub fn unreadable(e: &ritsu_proto::ReadError) -> String {
    let said = e.message("rulec");
    let what = if crate::i18n::ja() { said.ja } else { said.en };
    tr!(
        "`.proto` として読めません（{} 行目の {} 文字目）: {what}",
        "It does not read as a `.proto` (line {}, column {}): {what}",
        e.line,
        e.col
    )
}


/// `MemberTier` → `MEMBER_TIER`: an enum's name as the prefix its values carry.
///
/// The split is buf's, because `buf lint` is what asks for the prefix (`ENUM_VALUE_PREFIX`)
/// and protobuf-py splits the same way when it takes it off. A capital begins a word when the
/// letter before it is lower case or the one after it is, so `HTTPMethod` is `HTTP_METHOD` —
/// not `H_T_T_P_METHOD`, which buf refuses — and a digit begins none: `Tier2` is `TIER2`.
pub fn upper_snake(camel: &str) -> String {
    let b: Vec<char> = camel.chars().collect();
    let mut out = String::new();
    for (i, &ch) in b.iter().enumerate() {
        if ch == '_' {
            if !out.is_empty() && !out.ends_with('_') {
                out.push('_');
            }
            continue;
        }
        if i > 0 && ch.is_ascii_uppercase() && !out.ends_with('_') {
            let lower_before = b[i - 1].is_ascii_lowercase();
            let lower_after = b.get(i + 1).is_some_and(|n| !n.is_ascii_uppercase() && !n.is_ascii_digit() && *n != '_');
            if lower_before || lower_after {
                out.push('_');
            }
        }
        out.push(ch.to_ascii_uppercase());
    }
    while out.ends_with('_') {
        out.pop();
    }
    out
}

/// The BSR modules a `buf.yaml` declares under `deps`, by name: a label a ref carries is not
/// part of it (`buf.build/bufbuild/protovalidate:v1.0.0` is `buf.build/bufbuild/protovalidate`).
/// The block form and the flow form (`deps: [a, b]`) are both read; `version: v1` and `v2`
/// put `deps` at the top in the same way (§15.161).
pub fn buf_deps(yaml: &str) -> Vec<String> {
    ritsu_proto::buf::deps(yaml)
}

/// One module a `buf.lock` pins: its name, and the commit and digest it was resolved to.
pub use ritsu_proto::buf::Pin;

/// What a `buf.lock` pins, and the version of its shape. A `version: v1` lock names a module
/// by `remote`, `owner` and `repository` and digests it with shake256; a v2 module takes only
/// v2 pins (b5), so the version is what says whether these can be carried over (§15.161).
pub fn buf_lock(yaml: &str) -> (String, Vec<Pin>) {
    ritsu_proto::buf::lock(yaml)
}


#[cfg(test)]
mod tests {
    use super::*;

    const ORDER: &str = r#"
syntax = "proto3";
package shop.v1;

message Order {
  // The two spellings Protovalidate accepts, one per field.
  int64 weight_g = 1 [(buf.validate.field).int64 = {gte: 1, lte: 40000}];
  int64 total_jpy = 2 [
    (buf.validate.field).int64.gte = 0,  // a comment inside the options
    (buf.validate.field).int64.lte = 10000000
  ];
  // A list inside the options: the `]` of `[1, 2]` is not the end of them.
  repeated Line lines = 3 [(buf.validate.field).repeated = {min_items: 1, max_items: 50, items: {message: {required: true}}}];
  string zone = 4 [(buf.validate.field).string = {in: ["honshu", "hokkaido; okinawa"]}];
  int32 tier = 5 [(buf.validate.field).required = true, (buf.validate.field).ignore = IGNORE_IF_ZERO_VALUE, (buf.validate.field).cel = {id: "x", expression: "this > 0 && this != 7"}];
  string note = 6 [json_name = "memo", deprecated = true];
  sint32 delta = 7 [(buf.validate.field).sint32 = {gte: -5, lte: 0x10, not_in: [3, 4]}];
  int64 custom = 8 [(buf.validate.field).int64.(my.rule) = 3];
  optional int64 coupon = 9;
}

message Line {
  int64 amount = 1 [(buf.validate.field).int64 = {in: [100, 200]}];
}
"#;

    fn messages(src: &str) -> Vec<Message> {
        read("t.proto", src).expect("the file reads").messages
    }

    fn enums(src: &str) -> Vec<Enum> {
        read("t.proto", src).expect("the file reads").enums
    }

    fn imports(src: &str) -> Vec<String> {
        read("t.proto", src).expect("the file reads").imports
    }

    fn field<'a>(ms: &'a [Message], m: &str, f: &str) -> &'a Field {
        ms.iter().find(|x| x.name == m).and_then(|x| x.fields.iter().find(|y| y.name == f)).unwrap()
    }

    #[test]
    fn 注釈の二通りの書き方をどちらも読む() {
        let ms = messages(ORDER);
        let w = &field(&ms, "Order", "weight_g").rules.int;
        assert_eq!((w.gte, w.lte), (Some(1), Some(40000)));
        let t = &field(&ms, "Order", "total_jpy").rules.int;
        assert_eq!((t.gte, t.lte), (Some(0), Some(10_000_000)));
    }

    #[test]
    fn 入れ子のリストの後ろのフィールドも読む() {
        let ms = messages(ORDER);
        let l = &field(&ms, "Order", "lines");
        assert!(l.repeated);
        assert_eq!((l.rules.min_items, l.rules.max_items), (Some(1), Some(50)));
        // Before the bracket was matched by nesting, `]` of an inner list ended the options
        // and the `}` after it ended the message: every field below was lost.
        assert_eq!(ms.iter().find(|m| m.name == "Order").unwrap().fields.len(), 9);
        assert_eq!(field(&ms, "Line", "amount").rules.int.in_, vec![100, 200]);
    }

    #[test]
    fn 文字列の中の区切りで文が切れない() {
        let ms = messages(ORDER);
        assert_eq!(field(&ms, "Order", "zone").rules.str_in, vec!["honshu".to_string(), "hokkaido; okinawa".to_string()]);
    }

    #[test]
    fn 読まなかった規則は名前を残す() {
        let ms = messages(ORDER);
        let r = &field(&ms, "Order", "tier").rules;
        assert!(r.required);
        assert_eq!(r.ignore.as_deref(), Some("IGNORE_IF_ZERO_VALUE"));
        // CEL is kept as text for `cel` to read (§15.140), and is no longer an unread rule.
        assert_eq!(r.cel, vec!["this > 0 && this != 7".to_string()]);
        assert!(r.unread.is_empty());
        assert_eq!(field(&ms, "Order", "custom").rules.unread, vec!["int64.(my.rule)".to_string()]);
    }

    const RULES: &str = r#"
syntax = "proto3";
package shop.v1;

message Quote {
  option (buf.validate.message).cel = {
    id: "weight_order",
    message: "min must not exceed max",
    expression: "this.min_weight <= this.max_weight"
  };
  option (buf.validate.message).cel_expression = "this.max_weight <= 30000";
  option (buf.validate.message).oneof = {fields: ["coupon", "points"], required: true};
  option deprecated = true;

  int64 min_weight = 1 [(buf.validate.field).cel_expression = "this >= 1"];
  int64 max_weight = 2 [(buf.validate.field) = {cel: [{id: "a", expression: "this >= 1"}, {id: "b", expression: "this <= 50000"}]}];
  int64 coupon = 3;
  int64 points = 4;
  oneof payment {
    option (buf.validate.oneof).required = true;
    Card card = 5;
    string bank = 6 [(buf.validate.field).string.min_len = 1];
  }
  string memo = 7;
}

message Card {
  string number = 1;
}

message Old {
  option (buf.validate.message).disabled = true;
  int64 n = 1 [(buf.validate.field).int64.gte = 1];
  oneof pick {
    int64 a = 2;
    int64 b = 3;
  }
}
"#;

    #[test]
    fn メッセージの規則とoneofを読む() {
        let ms = messages(RULES);
        let q = ms.iter().find(|m| m.name == "Quote").unwrap();
        assert_eq!(q.rules.cel, vec!["this.min_weight <= this.max_weight".to_string(), "this.max_weight <= 30000".to_string()]);
        assert_eq!(field(&ms, "Quote", "min_weight").rules.cel, vec!["this >= 1".to_string()]);
        assert_eq!(field(&ms, "Quote", "max_weight").rules.cel, vec!["this >= 1".to_string(), "this <= 50000".to_string()]);
        // The members of a oneof are fields of the message, with presence of their own.
        let bank = field(&ms, "Quote", "bank");
        assert!(bank.optional && bank.oneof.as_deref() == Some("payment"));
        assert_eq!(bank.rules.str_min_len, Some(1));
        assert!(field(&ms, "Quote", "card").oneof.is_some());
        assert_eq!(field(&ms, "Quote", "memo").oneof, None);
        assert_eq!(q.fields.len(), 7);
        assert_eq!(
            q.rules.oneofs,
            vec![
                Oneof { fields: vec!["coupon".into(), "points".into()], required: true },
                Oneof { fields: vec!["card".into(), "bank".into()], required: true },
            ]
        );
    }

    #[test]
    fn 検証を切ったメッセージの規則は信じない() {
        let ms = messages(RULES);
        let old = ms.iter().find(|m| m.name == "Old").unwrap();
        assert!(old.rules.disabled);
        assert_eq!(field(&ms, "Old", "n").rules.int.gte, None);
        // The oneof itself is the wire's, and stays.
        assert_eq!(old.rules.oneofs, vec![Oneof { fields: vec!["a".into(), "b".into()], required: false }]);
    }

    #[test]
    fn 検証でない選択肢は読み飛ばす() {
        let ms = messages(ORDER);
        assert_eq!(field(&ms, "Order", "note").rules, Rules::default());
        let c = field(&ms, "Order", "coupon");
        assert!(c.optional && c.rules == Rules::default());
    }

    #[test]
    fn protojson_の名前() {
        let ms = messages(ORDER);
        assert_eq!(field(&ms, "Order", "weight_g").json(), "weightG");
        assert_eq!(field(&ms, "Order", "note").json(), "memo");
        assert_eq!(json_name("order_lines"), "orderLines");
        assert_eq!(json_name("zone"), "zone");
        assert_eq!(json_name("a1_b"), "a1B");
    }

    #[test]
    fn 負の数と十六進と除外の一覧() {
        let ms = messages(ORDER);
        let d = &field(&ms, "Order", "delta").rules.int;
        assert_eq!((d.gte, d.lte, d.not_in.clone()), (Some(-5), Some(16), vec![3, 4]));
    }

    #[test]
    fn 列挙の読み取りは変わらない() {
        let src = "enum MemberTier { MEMBER_TIER_UNSPECIFIED = 0; MEMBER_TIER_GOLD = 1 [(foo) = {a: [1, 2]}]; MEMBER_TIER_BASIC = 2; }";
        let es = enums(src);
        assert_eq!(es[0].aliases(), vec!["gold".to_string(), "basic".to_string()]);
    }

    #[test]
    fn 接頭辞は_buf_と同じに切る() {
        for (name, want) in [
            ("MemberTier", "MEMBER_TIER"),
            ("HTTPMethod", "HTTP_METHOD"),
            ("ABTest", "AB_TEST"),
            ("IPv4Kind", "I_PV4_KIND"),
            ("Tier2", "TIER2"),
            ("MemberTierV2", "MEMBER_TIER_V2"),
            ("Size60cm", "SIZE60CM"),
            ("Member_Tier", "MEMBER_TIER"),
            ("Carrier", "CARRIER"),
        ] {
            assert_eq!(upper_snake(name), want, "{name}");
        }
        let es = enums("enum HTTPMethod { HTTP_METHOD_UNSPECIFIED = 0; HTTP_METHOD_GET = 1; HTTP_METHOD_POST = 2; }");
        assert_eq!(es[0].aliases(), vec!["get".to_string(), "post".to_string()]);
        assert_eq!(es[0].unset().map(|v| v.name.as_str()), Some("HTTP_METHOD_UNSPECIFIED"));
    }

    #[test]
    fn 数字で始まる残りは接頭辞ごと残す() {
        let es = enums("enum Size { SIZE_UNSPECIFIED = 0; SIZE_60 = 1; SIZE_80 = 2; SIZE_LARGE = 3; }");
        assert_eq!(es[0].aliases(), vec!["size_60".to_string(), "size_80".to_string(), "large".to_string()]);
        // A remainder that is another value's whole name stays whole, so two values stay two.
        let es = enums("enum Color { COLOR_UNSPECIFIED = 0; COLOR_RED = 1; RED = 2; }");
        assert_eq!(es[0].aliases(), vec!["color_red".to_string(), "red".to_string()]);
        // A zero value that is a value of its own is not "not set".
        let es = enums("enum Status { STATUS_ACTIVE = 0; STATUS_CLOSED = 1; }");
        assert_eq!(es[0].aliases(), vec!["active".to_string(), "closed".to_string()]);
        assert!(es[0].unset().is_none());
    }

    #[test]
    fn buf_の依存とピンを読む() {
        let v2 = "version: v2\nmodules:\n  - path: proto\ndeps:\n  - buf.build/bufbuild/protovalidate:v1.0.0\n  - \"buf.build/googleapis/googleapis\"  # types\nlint:\n  use:\n    - STANDARD\n";
        assert_eq!(buf_deps(v2), ["buf.build/bufbuild/protovalidate", "buf.build/googleapis/googleapis"]);
        assert_eq!(buf_deps("version: v1\ndeps: [buf.build/bufbuild/protovalidate, buf.build/acme/x]\n"), ["buf.build/bufbuild/protovalidate", "buf.build/acme/x"]);
        assert!(buf_deps("version: v2\nlint:\n  use:\n    - STANDARD\n").is_empty());
        let lock = "# Generated by buf. DO NOT EDIT.\nversion: v2\ndeps:\n  - name: buf.build/bufbuild/protovalidate\n    commit: 511051f7f4374c3ca873b53ae68a9288\n    digest: b5:a4a2\n  - name: buf.build/googleapis/googleapis\n    commit: 61b203b9a9164be9a834f58c37be6f62\n    digest: b5:7811\n";
        let (v, pins) = buf_lock(lock);
        assert_eq!(v, "v2");
        assert_eq!(pins.len(), 2);
        assert_eq!(pins[0], Pin { name: "buf.build/bufbuild/protovalidate".into(), commit: "511051f7f4374c3ca873b53ae68a9288".into(), digest: "b5:a4a2".into() });
        let old = "version: v1\ndeps:\n  - remote: buf.build\n    owner: bufbuild\n    repository: protovalidate\n    commit: 511051f7f4374c3ca873b53ae68a9288\n    digest: shake256:b911\n";
        let (v, pins) = buf_lock(old);
        assert_eq!((v.as_str(), pins[0].name.as_str(), pins[0].digest.as_str()), ("v1", "buf.build/bufbuild/protovalidate", "shake256:b911"));
    }

    #[test]
    fn import_を読む() {
        let src = "syntax = \"proto3\";\npackage shop.v1;\nimport \"buf/validate/validate.proto\";\n\
                   import public 'shop/v1/common.proto';\nimport weak \"google/protobuf/struct.proto\";\n\
                   // import \"commented.proto\";\nmessage Imported { int64 importance = 1; imported.Foo x = 2; }\n";
        assert_eq!(imports(src), vec!["buf/validate/validate.proto", "shop/v1/common.proto", "google/protobuf/struct.proto"]);
    }
}
