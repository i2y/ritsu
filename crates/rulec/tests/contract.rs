//! What a contract lets through, held against what an input takes (§15.132, §15.133).
//!
//! The ledger's examples and the mutants reach the JSON Schema side. These reach the `.proto`
//! side, where the defaults protojson leaves out decide what arrives: a field with no rule lets
//! 0 through, an unset message lets everything under it through unvalidated, and an
//! `optional` field is unset rather than zero.

use std::process::Command;
use ritsu_testkit::TempDir;

/// A rule and its contract, checked; the codes and titles that came out, in order. Each call
/// gets a directory of its own: the tests run side by side in one process.
fn check(tag: &str, rule: &str, file: &str, contract: &str) -> Vec<(String, String, String)> {
    let tmp = TempDir::new(&format!("contract-{tag}"));
    let dir = tmp.path();
    std::fs::write(dir.join("r.rule"), rule).unwrap();
    std::fs::write(dir.join(file), contract).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "ja")
        .current_dir(dir)
        .args(["check", "r.rule", "--format", "json", "--lang", "ja"])
        .output()
        .expect("rulec を起動できない");
    String::from_utf8_lossy(&o.stdout)
        .lines()
        .filter(|l| l.starts_with('{'))
        .map(|l| {
            let field = |k: &str| {
                let key = format!("\"{k}\":");
                l.find(&key).map(|i| {
                    let rest = &l[i + key.len()..];
                    if let Some(s) = rest.strip_prefix('"') {
                        s[..s.find("\",").or_else(|| s.find('"')).unwrap_or(s.len())].to_string()
                    } else {
                        rest[..rest.find([',', '}']).unwrap_or(rest.len())].to_string()
                    }
                })
                .unwrap_or_default()
            };
            (field("code"), field("title"), l.to_string())
        })
        // The minimal rules here name no enum value in a row; that warning is not the subject.
        .filter(|(c, _, _)| c != "W111")
        .collect()
}

fn codes(found: &[(String, String, String)]) -> Vec<&str> {
    found.iter().map(|(c, _, _)| c.as_str()).collect()
}

/// One input read from `order.proto`, `Order.<field>`, into a table that takes it.
fn rule(decl: &str) -> String {
    format!(
        "rule t(t) v1\n\nshape 注文(order) = proto \"order.proto\" shop.v1.Order\n\ninputs\n  {decl}\n\n\
         outputs\n  x(x) : bool\n\ntable 表(t1)\npolicy unique\n| a | -> x |\n| - | true |\n"
    )
}

fn proto(body: &str) -> String {
    format!("syntax = \"proto3\";\npackage shop.v1;\n\n{body}\n")
}

#[test]
fn 規則の無い数のフィールドは0を通す() {
    let f = check("zero", &rule("a(a) : mass[g]  range >=1g <=40kg  from 注文.weight_g"), "order.proto", &proto("message Order {\n  int64 weight_g = 1;\n}"));
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].1.contains("0 を通しますが"), "{f:?}");
    assert!(f[0].2.contains("\"inputs\":{\"a\":0}"), "例は 0: {f:?}");
    assert!(f[0].2.contains("[(buf.validate.field).int64 = {gte: 1, lte: 40000}]"), "{f:?}");
}

#[test]
fn 注釈の二通りの書き方はどちらもそろう() {
    for body in [
        "message Order {\n  int64 weight_g = 1 [(buf.validate.field).int64 = {gte: 1, lte: 40000}];\n}",
        "message Order {\n  int64 weight_g = 1 [\n    (buf.validate.field).int64.gte = 1,\n    (buf.validate.field).int64.lte = 40000\n  ];\n}",
    ] {
        let f = check("both", &rule("a(a) : mass[g]  range >=1g <=40kg  from 注文.weight_g"), "order.proto", &proto(body));
        assert!(codes(&f).is_empty(), "{body}: {f:?}");
    }
}

/// Protovalidate validates nothing under a message that is not set, and the value is read as
/// its default: the rule on the field is not what decides whether 0 arrives.
#[test]
fn 省略できるメッセージの下は既定値のまま届く() {
    let body = |req: &str| {
        format!(
            "message Order {{\n  Shipping shipping = 1{req};\n}}\n\nmessage Shipping {{\n  int64 size_cm = 1 [(buf.validate.field).int64 = {{gte: 1, lte: 160}}];\n}}"
        )
    };
    let r = rule("a(a) : length[cm]  range >=1cm <=160cm  from 注文.shipping.size_cm");
    let f = check("unset", &r, "order.proto", &proto(&body("")));
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].1.contains("`注文.shipping` を省略でき"), "{f:?}");
    assert!(f[0].2.contains("[(buf.validate.field).required = true]"), "{f:?}");
    let f = check("unset-req", &r, "order.proto", &proto(&body(" [(buf.validate.field).required = true]")));
    assert!(codes(&f).is_empty(), "required なら既定値は来ない: {f:?}");
}

#[test]
fn optionalのフィールドは無いとき既定値として読む() {
    let body = proto("message Order {\n  optional int64 coupon = 2 [(buf.validate.field).int64.gte = 1];\n}");
    let f = check("opt", &rule("a(a) : number  range >=1 <=100  from 注文.coupon"), "order.proto", &body);
    let c = codes(&f);
    assert!(c.contains(&"E122"), "{f:?}");
    assert!(f.iter().any(|(_, t, _)| t.contains("`注文.coupon` を省略でき")), "{f:?}");
    // The input that says it may be missing reads an unset field as none, and nothing arrives
    // that it does not take — but the upper end is still open in the contract.
    let f = check("opt-none", &rule("a(a) : number?  from 注文.coupon"), "order.proto", &body);
    assert!(!f.iter().any(|(_, t, _)| t.contains("省略でき")), "{f:?}");
}

#[test]
fn 読めるcelは範囲として読み_読めないものは名前を注に出す() {
    // `this <= 100` is read (§15.140): with `gte: 1` the contract lets 1 to 100 through.
    let body = proto("message Order {\n  int32 points = 1 [(buf.validate.field).int32.gte = 1, (buf.validate.field).cel = {id: \"x\", expression: \"this <= 100\"}];\n}");
    let f = check("cel", &rule("a(a) : number  range >=1 <=100  from 注文.points"), "order.proto", &body);
    assert!(codes(&f).is_empty(), "{f:?}");
    // A remainder is not: the upper end stays open, and the note names what was not read.
    let body = proto("message Order {\n  int32 points = 1 [(buf.validate.field).int32.gte = 1, (buf.validate.field).cel = {id: \"x\", expression: \"this % 7 != 3\"}];\n}");
    let f = check("cel-unread", &rule("a(a) : number  range >=1 <=100  from 注文.points"), "order.proto", &body);
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].2.contains("読んでいない規則があります（cel）"), "{f:?}");
}

#[test]
fn 列挙に無い値と_文字列の既定値() {
    let body = proto("message Order {\n  string zone = 1 [(buf.validate.field).string = {in: [\"honshu\", \"overseas\"]}];\n}");
    let r = "rule t(t) v1\n\nshape 注文(order) = proto \"order.proto\" shop.v1.Order\n\nenum 地域(zone) = honshu(honshu) | okinawa(okinawa)\n\n\
             inputs\n  a(a) : 地域  from 注文.zone\n\noutputs\n  x(x) : bool\n\ntable 表(t1)\npolicy unique\n| a | -> x |\n| - | true |\n";
    let f = check("enum", r, "order.proto", &body);
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].1.contains("overseas"), "{f:?}");
    assert!(f[0].2.contains("\"kind\":\"narrow_contract\"") && f[0].2.contains("[(buf.validate.field).string = {in: ["), "{f:?}");
}

#[test]
fn 契約の外でしか当たらない行はw123() {
    let body = proto("message Order {\n  int64 weight_g = 1 [(buf.validate.field).int64 = {gte: 1, lte: 30000}];\n}");
    let r = "rule t(t) v1\n\nshape 注文(order) = proto \"order.proto\" shop.v1.Order\n\n\
             inputs\n  a(a) : mass[g]  range >=1g <=40kg  from 注文.weight_g\n\noutputs\n  x(x) : money[円]  round up(10円)\n\n\
             table 表(t1)\npolicy unique\n| a       | -> x   |\n| <=30kg  | 800円  |\n| >30kg   | 5000円 |\n";
    let f = check("w123", r, "order.proto", &body);
    assert_eq!(codes(&f), vec!["W123"], "{f:?}");
    assert!(f[0].2.contains("\"row\":2"), "{f:?}");
}

#[test]
fn 省略できるフィールドを必須の入力で読むとe122_省略できる入力なら通る() {
    let schema = "{\"$defs\":{\"Order\":{\"type\":\"object\",\"properties\":{\"memo\":{\"type\":\"string\",\"enum\":[\"a\",\"b\"]}}}}}";
    let r = |ty: &str| {
        format!(
            "rule t(t) v1\n\nshape 注文(order) = jsonschema \"order.json\" \"#/$defs/Order\"\n\nenum 種(kind) = a(a) | b(b)\n\n\
             inputs\n  a(a) : {ty}  from 注文.memo\n\noutputs\n  x(x) : bool\n\ntable 表(t1)\npolicy unique\n| a | -> x |\n| - | true |\n"
        )
    };
    let f = check("req", &r("種"), "order.json", schema);
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].1.contains("`注文.memo` を省略でき") && f[0].2.contains("\"kind\":\"narrow_contract\""), "{f:?}");
    let f = check("opt", &r("種?"), "order.json", schema);
    assert!(codes(&f).is_empty(), "{f:?}");
}

/// The default under an unset message is reported apart from what the field's own rules let
/// through: the two are fixed in two places, the rule on the field and `required` on the
/// message.
#[test]
fn 既定値と規則の緩さは別々に言う() {
    let body = proto("message Order {\n  Shipping shipping = 1;\n}\n\nmessage Shipping {\n  int64 size_cm = 1;\n}");
    let f = check("both-zero", &rule("a(a) : length[cm]  range >=1cm <=160cm  from 注文.shipping.size_cm"), "order.proto", &body);
    assert_eq!(codes(&f), vec!["E122", "E122"], "{f:?}");
    assert!(f[0].2.contains("[(buf.validate.field).int64 = {gte: 1, lte: 160}]"), "{f:?}");
    assert!(f[1].1.contains("`注文.shipping` を省略でき") && f[1].2.contains("[(buf.validate.field).required = true]"), "{f:?}");
}

/// A collection behind an unset message has no elements, whatever `min_items` says. How many
/// elements a `where` picks out can be none anyway, and that is said once.
#[test]
fn 省略できるメッセージの下の並びは0件() {
    let body = proto(
        "message Order {\n  Cart cart = 1;\n}\n\nmessage Cart {\n  repeated Line lines = 1 [(buf.validate.field).repeated = {min_items: 1, max_items: 50}];\n}\n\nmessage Line {\n  bool chilled = 1;\n}",
    );
    let f = check("count", &rule("a(a) : number  range >=1 <=50  from count 注文.cart.lines"), "order.proto", &body);
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].1.contains("`注文.cart` を省略でき") && f[0].1.contains("0 件"), "{f:?}");
    let f = check("count-where", &rule("a(a) : number  range >=1 <=50  from count 注文.cart.lines where chilled = true"), "order.proto", &body);
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].1.contains("`where` に当てはまる要素が 0 件"), "{f:?}");
}

/// A date travels as a string, and "" — how protojson leaves an unset string out — is not one.
/// `required` rules "" out on a field without presence; on an `optional` one it asks only that
/// the field be set, so what rules "" out there is a length.
#[test]
fn 日付の文字列は空を通さないこと() {
    let r = |ty: &str| rule(&format!("a(a) : {ty}  range >=2026-01-01 <=2026-12-31  from 注文.ships_on"));
    let date = |field: &str| proto(&format!("message Order {{\n  {field};\n}}"));
    let f = check("date", &r("date"), "order.proto", &date("string ships_on = 1"));
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].1.contains("\\\"\\\" は日付ではありません") && f[0].2.contains("[(buf.validate.field).required = true]"), "{f:?}");
    for ok in [
        "string ships_on = 1 [(buf.validate.field).required = true]",
        "string ships_on = 1 [(buf.validate.field).string.min_len = 10]",
        "string ships_on = 1 [(buf.validate.field).string = {len: 10}]",
    ] {
        let f = check("date-ok", &r("date"), "order.proto", &date(ok));
        assert!(codes(&f).is_empty(), "{ok}: {f:?}");
    }
    // Set to "" on purpose, an `optional` field passes `required`.
    let f = check("date-opt", &r("date?"), "order.proto", &date("optional string ships_on = 1 [(buf.validate.field).required = true]"));
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].2.contains("[(buf.validate.field).string.min_len = 1]"), "{f:?}");
    let f = check("date-opt-ok", &r("date?"), "order.proto", &date("optional string ships_on = 1 [(buf.validate.field).string.min_len = 1]"));
    assert!(codes(&f).is_empty(), "{f:?}");
    // A required input from an `optional` field: unset is "" as well.
    let f = check("date-unset", &r("date"), "order.proto", &date("optional string ships_on = 1 [(buf.validate.field).string.min_len = 1]"));
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].1.contains("`注文.ships_on` を省略でき"), "{f:?}");
}

/// What a `where` compares is what the contract carries: an enum as its value's name, and
/// nothing at all for `bytes` or a message.
#[test]
fn whereは要素のフィールドが運ぶもので比べる() {
    let body = proto(
        "enum Temp {\n  TEMP_AMBIENT = 0;\n  TEMP_FROZEN = 1;\n}\n\n\
         message Order {\n  repeated Line lines = 1 [(buf.validate.field).repeated.max_items = 20];\n}\n\n\
         message Line {\n  Temp temp = 1;\n  bytes blob = 2;\n  Money price = 3;\n}\n\nmessage Money {\n  int64 units = 1;\n}",
    );
    let w = |test: &str| rule(&format!("a(a) : bool  from any 注文.lines where {test}"));
    for ok in ["temp = TEMP_FROZEN", "temp = \"TEMP_FROZEN\"", "temp = TEMP_AMBIENT, TEMP_FROZEN"] {
        let f = check("where-enum", &w(ok), "order.proto", &body);
        assert!(codes(&f).is_empty(), "{ok}: {f:?}");
    }
    for (bad, says) in [("temp = 1", "string"), ("blob = \"x\"", "bytes"), ("price = 100", "object")] {
        let f = check("where-bad", &w(bad), "order.proto", &body);
        assert_eq!(codes(&f), vec!["E120"], "{bad}: {f:?}");
        assert!(f[0].2.contains(says), "{bad}: {f:?}");
    }
}

/// A contract that spells the enum's aliases, when what travels is the names: narrowing it to
/// none of the values would be no fix, so there is none, and the note says which side spells
/// what.
#[test]
fn 契約が別名で綴っているときは名前との違いを言う() {
    let body = proto("message Order {\n  string zone = 1 [(buf.validate.field).string = {in: [\"honshu\", \"okinawa\"]}];\n}");
    let r = "rule t(t) v1\n\nshape 注文(order) = proto \"order.proto\" shop.v1.Order\n\nenum 地域(zone) = 本州(honshu) | 沖縄(okinawa)\n\n\
             inputs\n  a(a) : 地域  from 注文.zone\n\noutputs\n  x(x) : bool\n\ntable 表(t1)\npolicy unique\n| a | -> x |\n| - | true |\n";
    let f = check("alias", r, "order.proto", &body);
    // And no row is reached, since nothing the contract lets through is a name.
    assert_eq!(codes(&f), vec!["E122", "W123"], "{f:?}");
    assert!(f[0].2.contains("\"fix\":{\"kind\":\"none\"}") && !f[0].2.contains("in: []"), "{f:?}");
    assert!(f[0].2.contains("列挙 地域 の別名です"), "{f:?}");
}

/// A field whose kind no input takes is named for what it is, rather than reported as a path
/// the contract does not have: an enum of the `.proto`, whose values travel as their names and
/// which a path does not hand to the rule's enum, and `bytes`.
#[test]
fn 入力にならない型のフィールドはe120で示す() {
    let body = proto("enum Zone {\n  ZONE_UNSPECIFIED = 0;\n  ZONE_HONSHU = 1;\n}\n\nmessage Order {\n  Zone zone = 1;\n  bytes blob = 2;\n}");
    let r = "rule t(t) v1\n\nshape 注文(order) = proto \"order.proto\" shop.v1.Order\n\nenum 地域(zone) = ZONE_HONSHU(honshu)\n\n\
             inputs\n  a(a) : 地域  from 注文.zone\n\noutputs\n  x(x) : bool\n\ntable 表(t1)\npolicy unique\n| a | -> x |\n| - | true |\n";
    let f = check("enum-leaf", r, "order.proto", &body);
    assert_eq!(codes(&f), vec!["E120"], "{f:?}");
    assert!(f[0].2.contains("enum Zone") && f[0].2.contains("`where` で比べることはできます"), "{f:?}");
    let f = check("bytes-leaf", &rule("a(a) : string  from 注文.blob"), "order.proto", &body);
    assert_eq!(codes(&f), vec!["E120"], "{f:?}");
    assert!(f[0].2.contains("契約では bytes"), "{f:?}");
}

// --- Across the fields (§15.140) ----------------------------------------------------------

/// A rule reading two values from `order.proto`, with `between` after the inputs and `table`
/// at the end.
fn pair(decls: &str, between: &str, table: &str) -> String {
    format!(
        "rule t(t) v1\n\nshape 注文(order) = proto \"order.proto\" shop.v1.Order\n\ninputs\n{decls}\n{between}\noutputs\n  x(x) : bool\n\n{table}"
    )
}

const WEIGHTS: &str = "  最小(min_g) : mass[g]  range >=1g <=30kg  from 注文.min_g\n  最大(max_g) : mass[g]  range >=1g <=30kg  from 注文.max_g\n";
const ONE_ROW: &str = "table 表(t1)\npolicy unique\n| 最小 | -> x |\n| -    | true |\n";

fn weights(rules: &str) -> String {
    proto(&format!(
        "message Order {{\n{rules}  int64 min_g = 1 [(buf.validate.field).int64 = {{gte: 1, lte: 30000}}];\n  int64 max_g = 2 [(buf.validate.field).int64 = {{gte: 1, lte: 30000}}];\n}}"
    ))
}

#[test]
fn 契約が約束しない制約はe123_約束すれば通る() {
    let r = pair(WEIGHTS, "constraint 最小 <= 最大\n", ONE_ROW);
    let f = check("e123", &r, "order.proto", &weights(""));
    assert_eq!(codes(&f), vec!["E123"], "{f:?}");
    let line = &f[0].2;
    assert!(line.contains("option (buf.validate.message).cel = {id: \\\"min_g_le_max_g\\\", expression: \\\"this.min_g <= this.max_g\\\"};"), "{line}");
    // The example breaks the constraint and passes the contract.
    let num = |k: &str| -> i64 {
        let key = format!("\"{k}\":");
        let i = line.find(&key).unwrap() + key.len();
        line[i..].split([',', '}']).next().unwrap().parse().unwrap()
    };
    assert!(num("最小") > num("最大"), "{line}");
    // The contract that promises it is quiet.
    let f = check("e123-ok", &r, "order.proto", &weights("  option (buf.validate.message).cel = {id: \"w\", expression: \"this.min_g <= this.max_g\"};\n"));
    assert!(codes(&f).is_empty(), "{f:?}");
}

#[test]
fn 契約が通さない組み合わせでしか当たらない行はw124() {
    let table = "table 表(t1)\npolicy first\n| 最小  | 最大   | -> x  |\n| >20kg | <=10kg | true  |\n| -     | -      | false |\n";
    let f = check("w124", &pair(WEIGHTS, "", table), "order.proto", &weights("  option (buf.validate.message).cel = {id: \"w\", expression: \"this.min_g <= this.max_g\"};\n"));
    assert!(codes(&f).contains(&"W124"), "{f:?}");
    let w = f.iter().find(|(c, _, _)| c == "W124").unwrap();
    assert!(w.2.contains("\"row\":1"), "{w:?}");
    // Without the relation, the row is reached.
    let f = check("w124-none", &pair(WEIGHTS, "", table), "order.proto", &weights(""));
    assert!(!codes(&f).contains(&"W124"), "{f:?}");
}

#[test]
fn メッセージのcelは一つのフィールドの範囲も狭める() {
    let body = proto("message Order {\n  option (buf.validate.message).cel_expression = \"this.w >= 1 && this.w <= 100\";\n  int64 w = 1;\n}");
    let f = check("msg-cel", &rule("a(a) : number  range >=1 <=100  from 注文.w"), "order.proto", &body);
    assert!(codes(&f).is_empty(), "{f:?}");
}

#[test]
fn oneofの二つが同時に来る行はw124() {
    let body = proto(
        "message Order {\n  oneof pay {\n    int64 card_jpy = 1 [(buf.validate.field).int64 = {gte: 1, lte: 1000000}];\n    int64 bank_jpy = 2 [(buf.validate.field).int64 = {gte: 1, lte: 1000000}];\n  }\n}",
    );
    let decls = "  カード(card) : money[円]  range >=0円 <=100万円  from 注文.card_jpy\n  振込(bank)   : money[円]  range >=0円 <=100万円  from 注文.bank_jpy\n";
    let table = "table 表(t1)\npolicy first\n| カード | 振込 | -> x  |\n| >0円   | >0円 | true  |\n| -      | -    | false |\n";
    let f = check("oneof", &pair(decls, "", table), "order.proto", &body);
    assert!(codes(&f).contains(&"W124"), "{f:?}");
    // A member of a oneof has presence of its own: unset, it reads as 0 without its rules.
    assert!(!codes(&f).contains(&"E122"), "{f:?}");
}

#[test]
fn optionalに付いたrequiredは0を通す() {
    let body = proto("message Order {\n  optional int64 n = 1 [(buf.validate.field).required = true, (buf.validate.field).int64.lte = 100];\n}");
    let f = check("opt-req", &rule("a(a) : number  range >=1 <=100  from 注文.n"), "order.proto", &body);
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].1.contains("0 を通しますが"), "{f:?}");
}

/// A rule reading from `order.json`, the schema at `#/$defs/Order`.
fn json_rule(decls: &str, between: &str, table: &str) -> String {
    format!(
        "rule t(t) v1\n\nshape 注文(order) = jsonschema \"order.json\" \"#/$defs/Order\"\n\ninputs\n{decls}\n{between}\noutputs\n  x(x) : bool\n\n{table}"
    )
}

const EXPRESS: &str = r##"{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$defs": {
    "Base": {
      "type": "object",
      "properties": { "weight_g": { "type": "integer", "minimum": 1, "maximum": 30000 } },
      "required": ["weight_g"]
    },
    "Order": {
      "allOf": [{ "$ref": "#/$defs/Base" }],
      "type": "object",
      "properties": { "express": { "type": "boolean" } },
      "required": ["express"],
      "if": { "properties": { "express": { "const": true } } },
      "then": { "properties": { "weight_g": { "maximum": 5000 } } }
    }
  }
}
"##;

#[test]
fn jsonschemaの組み合わせの中のパスを読み_ifとthenの関係でw124() {
    let decls = "  重さ(weight) : mass[g]  range >=1g <=30kg  from 注文.weight_g\n  急ぎ(express) : bool  from 注文.express\n";
    let table = "table 表(t1)\npolicy first\n| 急ぎ | 重さ | -> x  |\n| true | >5kg | true  |\n| -    | -    | false |\n";
    let f = check("json-if", &json_rule(decls, "", table), "order.json", EXPRESS);
    // `weight_g` is in `allOf`, `required` there too: no E121, no E122.
    assert_eq!(codes(&f), vec!["W124"], "{f:?}");
}

#[test]
fn nullを通すフィールドは省略できない入力ならe122() {
    let schema = "{\"$defs\":{\"Order\":{\"type\":\"object\",\"properties\":{\"n\":{\"type\":[\"integer\",\"null\"],\"minimum\":0,\"maximum\":10}},\"required\":[\"n\"]}}}";
    let one = "table 表(t1)\npolicy unique\n| a | -> x |\n| - | true |\n";
    let f = check("null", &json_rule("  a(a) : number  range >=0 <=10  from 注文.n\n", "", one), "order.json", schema);
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].1.contains("null を通しますが"), "{f:?}");
    assert!(f[0].2.contains("\\\"type\\\": \\\"integer\\\""), "{f:?}");
    let f = check("null-opt", &json_rule("  a(a) : number?  from 注文.n\n", "", one), "order.json", schema);
    assert!(codes(&f).is_empty(), "{f:?}");
}

#[test]
fn jsonschemaでは関係を書けないのでe123に契約側の直しは無い() {
    let schema = "{\"$defs\":{\"Order\":{\"type\":\"object\",\"properties\":{\"min_g\":{\"type\":\"integer\",\"minimum\":1,\"maximum\":30000},\"max_g\":{\"type\":\"integer\",\"minimum\":1,\"maximum\":30000}},\"required\":[\"min_g\",\"max_g\"]}}}";
    let decls = "  最小(min_g) : mass[g]  range >=1g <=30kg  from 注文.min_g\n  最大(max_g) : mass[g]  range >=1g <=30kg  from 注文.max_g\n";
    let f = check("json-e123", &json_rule(decls, "constraint 最小 <= 最大\n", ONE_ROW), "order.json", schema);
    assert_eq!(codes(&f), vec!["E123"], "{f:?}");
    assert!(f[0].2.contains("\"kind\":\"none\""), "{f:?}");
}

#[test]
fn メッセージのcelが列挙の値を絞る() {
    let body = proto(
        "message Order {\n  Shipping shipping = 1;\n}\n\nmessage Shipping {\n  option (buf.validate.message).cel_expression = \"this.zone in ['honshu', 'okinawa']\";\n  string zone = 1;\n}",
    );
    let r = "rule t(t) v1\n\nshape 注文(order) = proto \"order.proto\" shop.v1.Order\n\nenum 地域(zone) = honshu(honshu) | okinawa(okinawa)\n\n\
             inputs\n  a(a) : 地域  from 注文.shipping.zone\n\noutputs\n  x(x) : bool\n\ntable 表(t1)\npolicy unique\n| a | -> x |\n| - | true |\n";
    let f = check("enum-cel", r, "order.proto", &body);
    // The CEL names the values, so "any string" is not said; what is left is the "" an unset
    // `shipping` arrives as, said once.
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].1.contains("`注文.shipping` を省略でき"), "{f:?}");
}

/// A contract of an order whose lines carry a quantity and an amount, both always there: `extra`
/// goes into the collection's own keywords, and `amount` is the amount's.
fn lines_schema(extra: &str, amount: &str) -> String {
    format!(
        "{{\"$defs\":{{\"Order\":{{\"type\":\"object\",\"required\":[\"lines\"],\"properties\":{{\"lines\":{{\"type\":\"array\"{extra},\
         \"items\":{{\"type\":\"object\",\"required\":[\"qty\",\"amount\"],\"properties\":{{\"qty\":{{\"type\":\"integer\",\"minimum\":1,\"maximum\":9}},\
         \"amount\":{{\"type\":\"integer\"{amount}}}}}}}}}}}}}}}}}"
    )
}

/// A rule that reads `decl` and walks nothing: one input read from `order.json`.
fn schema_rule(decl: &str) -> String {
    format!(
        "rule t v1\n\nshape order = jsonschema \"order.json\" \"#/$defs/Order\"\n\ninputs\n  {decl}\n\n\
         outputs\n  x : bool\n\ntable t\npolicy unique\n| a | -> x |\n| - | true |\n"
    )
}

/// How far a total reaches is what the field's values times how many elements the contract lets
/// through come to (§15.223): ten lines of at most 100 come to 1000, which an input of at most
/// 500 refuses, and the narrowing offered is the length at which the total stays inside.
#[test]
fn 合計の届く先を要素の値と件数から決める() {
    let schema = lines_schema(",\"maxItems\":10", ",\"minimum\":0,\"maximum\":100");
    let f = check("sum-over", &schema_rule("a : money[USD]  range >=0USD <=500USD  from sum order.lines of amount"), "order.json", &schema);
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].1.contains("`order.lines` の `amount` の合計が 1000 になるのを通しますが"), "{f:?}");
    assert!(f[0].2.contains("\"witness\":{\"inputs\":{\"a\":1000}}"), "{f:?}");
    assert!(f[0].2.contains("\"fix\":{\"kind\":\"narrow_contract\",\"text\":\"\\\"maxItems\\\": 5\"}"), "{f:?}");
    // Inside the input's range, nothing is said.
    let f = check("sum-in", &schema_rule("a : money[USD]  range >=0USD <=1000USD  from sum order.lines of amount"), "order.json", &schema);
    assert!(codes(&f).is_empty(), "{f:?}");
    // No end on the length and none on the values: there is no narrowing of one keyword to offer.
    let f = check("sum-open", &schema_rule("a : money[USD]  range >=0USD <=1000USD  from sum order.lines of amount"), "order.json", &lines_schema("", ",\"minimum\":0"));
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].2.contains("\"fix\":{\"kind\":\"none\"}"), "{f:?}");
}

/// The largest or smallest value is one of the field's (§15.223): what the contract lets the field
/// be is held to the input, and the `empty` value is the input's own and is not the contract's.
#[test]
fn 最大と最小は要素のフィールドの値で比べる() {
    let schema = lines_schema(",\"maxItems\":10", ",\"minimum\":0,\"maximum\":100");
    let f = check("max-over", &schema_rule("a : money[USD]  range >=0USD <=50USD  from max order.lines of amount empty 0USD"), "order.json", &schema);
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].1.contains("`order.lines[].amount` に 51 を通しますが"), "{f:?}");
    assert!(f[0].2.contains("\"text\":\"\\\"minimum\\\": 0, \\\"maximum\\\": 50\""), "{f:?}");
    let f = check("min-in", &schema_rule("a : money[USD]  range >=0USD <=100USD  from min order.lines of amount empty 100USD"), "order.json", &schema);
    assert!(codes(&f).is_empty(), "{f:?}");
}

/// A field `sum`, `max` and `min` read is read in every element: one the contract lets an element
/// leave out, or lets be null, is one the function that reads it out fails on (§15.223).
#[test]
fn 合計の読むフィールドが無いことやnullを言う() {
    let schema = "{\"$defs\":{\"Order\":{\"type\":\"object\",\"required\":[\"lines\"],\"properties\":{\"lines\":{\"type\":\"array\",\"maxItems\":10,\
                  \"items\":{\"type\":\"object\",\"properties\":{\"amount\":{\"type\":[\"integer\",\"null\"],\"minimum\":0,\"maximum\":100}}}}}}}}";
    let f = check("sum-left-out", &schema_rule("a : money[USD]  range >=0USD <=1000USD  from sum order.lines of amount"), "order.json", schema);
    assert_eq!(codes(&f), vec!["E122", "E122"], "{f:?}");
    assert!(f[0].1.contains("契約では要素の `amount` を省略できますが、`from sum` はそれを必ず読みます"), "{f:?}");
    assert!(f[1].1.contains("`order.lines[].amount` に null を通しますが、`from sum` はそれを読めません"), "{f:?}");
}

/// A sequence taken from a contract (§15.223): each field of its elements is held as an input is,
/// with no example of an input in what is said, since a field is none; its length to the cap
/// its `count`s put on it; and its total of a field to the upper end of a `sum` over it.
#[test]
fn 契約から取る並びの件数と合計と要素を突き合わせる() {
    let rule = |count: &str, sum: &str| {
        format!(
            "rule t v1\n\nshape order = jsonschema \"order.json\" \"#/$defs/Order\"\n\n\
             elements items from order.lines\n  qty    : number  range >=1 <=5  from qty\n  amount : money[USD]  range >=0USD <=100USD  from amount\n\n\
             outputs\n  x : bool\n\n\
             table b\npolicy unique\n| qty | -> big : bool |\n| >=3 | true          |\n| <3  | false         |\n\n\
             count n over items where big  range >=0 <={count}\n\
             sum s over items of amount  range >=0USD <={sum}USD\n\n\
             table t\npolicy unique\n| n | s | -> x |\n| - | - | true |\n"
        )
    };
    let schema = lines_schema(",\"maxItems\":30", ",\"minimum\":0,\"maximum\":100");
    let f = check("seq", &rule("20", "1000"), "order.json", &schema);
    let titles: Vec<&str> = f.iter().map(|(_, t, _)| t.as_str()).collect();
    assert_eq!(codes(&f), vec!["E122", "E122", "E122"], "{titles:?}");
    // The length: thirty lines, where the `count` takes twenty.
    assert!(titles[0].contains("契約は `order.lines` が 21 件でも通しますが、並び items は 20 件までしか受け付けません"), "{titles:?}");
    assert!(f[0].2.contains("\"text\":\"\\\"maxItems\\\": 20\""), "{f:?}");
    // The total: twenty lines of at most 100USD, where the `sum` takes 1000USD.
    assert!(titles[1].contains("契約は `order.lines` を 20 件まで通し、amount の合計は 2000USD に届きますが、s は 1000USD までしか受け付けません"), "{titles:?}");
    assert!(f[1].2.contains("\"text\":\"\\\"maxItems\\\": 10\""), "{f:?}");
    // A field of an element: the contract lets a quantity of up to 9, and the field takes 5.
    assert!(titles[2].contains("契約は `order.lines[].qty` に 6 を通しますが、規則はそれを受け付けません"), "{titles:?}");
    assert!(f[2].2.contains("\"witness\":{}"), "要素のフィールドは入力ではないので例は付かない: {f:?}");
    // Every cap held: nothing is said.
    let narrow = lines_schema(",\"maxItems\":10", ",\"minimum\":0,\"maximum\":100").replace("\"maximum\":9", "\"maximum\":5");
    let f = check("seq-ok", &rule("10", "1000"), "order.json", &narrow);
    assert!(codes(&f).is_empty(), "{f:?}");
}

/// A walk is held to the bottom of the range of a `sum` or a `count` where that is above zero
/// (§15.314), so a sequence taken from a contract is held to it as well: a collection the contract
/// lets through with no elements, or with a total under the bottom, is one the generated code
/// refuses once it has walked it. The least a total comes to is the fewest elements the contract
/// lets through, each at the least the contract lets its field be; which elements a `count` counts
/// is not read from a contract, so its bottom is one the contract does not promise. A range that
/// starts at zero says nothing, as before.
#[test]
fn a_sequence_taken_from_a_contract_is_held_to_the_bottoms_of_its_walk() {
    let rule = |count: &str, sum: &str| {
        format!(
            "rule t v1\n\nshape order = jsonschema \"order.json\" \"#/$defs/Order\"\n\n\
             elements items from order.lines\n  qty    : number  range >=1 <=9  from qty\n  amount : money[USD]  range >=0USD <=100USD  from amount\n\n\
             outputs\n  x : bool\n\n\
             table b\npolicy unique\n| qty | -> big : bool |\n| >=3 | true          |\n| <3  | false         |\n\n\
             count n over items where big  range {count} <=10\n\
             sum s over items of amount  range {sum} <=1000USD\n\n\
             table t\npolicy unique\n| n | s | -> x |\n| - | - | true |\n"
        )
    };
    let titles = |f: &[(String, String, String)]| f.iter().map(|(_, t, _)| t.clone()).collect::<Vec<_>>();
    // No least length and amounts from 0: an empty collection passes the contract, and its total is 0.
    let open = lines_schema(",\"maxItems\":10", ",\"minimum\":0,\"maximum\":100");
    let f = check("floor-sum-empty", &rule(">=0", ">=250USD"), "order.json", &open);
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].1.contains("契約は `order.lines` が 0 件でも通しますが、s は 250USD からしか受け付けません"), "{:?}", titles(&f));
    assert!(f[0].2.contains("\"fix\":{\"kind\":\"none\"}"), "{f:?}");
    // One line at the least, of 0 dollars at the least: no number of lines keeps the total up.
    let f = check("floor-sum-one", &rule(">=0", ">=250USD"), "order.json", &lines_schema(",\"minItems\":1,\"maxItems\":10", ",\"minimum\":0,\"maximum\":100"));
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].1.contains("契約は `order.lines` を 1 件から通し、amount の合計は 0USD のこともありますが、s は 250USD からしか受け付けません"), "{:?}", titles(&f));
    // Amounts from 50 dollars: five lines reach 250 dollars, and that is the narrowing offered.
    let fifty = |extra: &str| lines_schema(extra, ",\"minimum\":50,\"maximum\":100");
    let f = check("floor-sum-fifty", &rule(">=0", ">=250USD"), "order.json", &fifty(",\"maxItems\":10"));
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].2.contains("\"fix\":{\"kind\":\"narrow_contract\",\"text\":\"\\\"minItems\\\": 5, \\\"maxItems\\\": 10\"}"), "{f:?}");
    assert!(f[0].2.contains("一件の amount は 50USD から"), "{f:?}");
    // With it, the total is held; four lines are one short.
    let f = check("floor-sum-held", &rule(">=0", ">=250USD"), "order.json", &fifty(",\"minItems\":5,\"maxItems\":10"));
    assert!(codes(&f).is_empty(), "{f:?}");
    let f = check("floor-sum-four", &rule(">=0", ">=250USD"), "order.json", &fifty(",\"minItems\":4,\"maxItems\":10"));
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].1.contains("契約は `order.lines` を 4 件から通し、amount の合計は 200USD のこともありますが、s は 250USD からしか受け付けません"), "{:?}", titles(&f));
    // A count from 1: an empty collection has none, and a collection of five promises none either.
    let f = check("floor-count-empty", &rule(">=1", ">=0USD"), "order.json", &open);
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].1.contains("契約は `order.lines` が 0 件でも通しますが、n は数える要素が 1 件からしか受け付けません"), "{:?}", titles(&f));
    assert!(f[0].2.contains("\"fix\":{\"kind\":\"none\"}"), "{f:?}");
    let f = check("floor-count-five", &rule(">=1", ">=0USD"), "order.json", &fifty(",\"minItems\":5,\"maxItems\":10"));
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].1.contains("契約は、`order.lines` の要素のうちいくつが n に数えられるかを約束しませんが、n は 1 からしか受け付けません"), "{:?}", titles(&f));
    // A count from 2: a collection of one is short of it whichever lines count, and a collection
    // of two has as many lines as the count starts at, and promises none of them.
    let f = check("floor-count-short", &rule(">=2", ">=0USD"), "order.json", &fifty(",\"minItems\":1,\"maxItems\":10"));
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].1.contains("契約は `order.lines` が 1 件でも通しますが、n は数える要素が 2 件からしか受け付けません"), "{:?}", titles(&f));
    let f = check("floor-count-two", &rule(">=2", ">=0USD"), "order.json", &fifty(",\"minItems\":2,\"maxItems\":10"));
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].1.contains("契約は、`order.lines` の要素のうちいくつが n に数えられるかを約束しませんが、n は 2 からしか受け付けません"), "{:?}", titles(&f));
    // The same from a `.proto`, where the narrowing offered is the Protovalidate rule.
    let pb = |repeated: &str| {
        proto(&format!(
            "message Order {{\n  repeated Line lines = 1 [(buf.validate.field).repeated = {{{repeated}}}];\n}}\n\n\
             message Line {{\n  int64 qty = 1 [(buf.validate.field).int64 = {{gte: 1, lte: 9}}];\n  int64 amount = 2 [(buf.validate.field).int64 = {{gte: 50, lte: 100}}];\n}}"
        ))
    };
    let pb_rule = rule(">=0", ">=250USD").replace("jsonschema \"order.json\" \"#/$defs/Order\"", "proto \"order.proto\" shop.v1.Order");
    assert!(pb_rule.contains("shape order = proto"), "{pb_rule}");
    let f = check("floor-sum-proto", &pb_rule, "order.proto", &pb("max_items: 10"));
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].1.contains("契約は `order.lines` が 0 件でも通しますが、s は 250USD からしか受け付けません"), "{:?}", titles(&f));
    assert!(f[0].2.contains("[(buf.validate.field).repeated = {min_items: 5, max_items: 10}]"), "{f:?}");
    let f = check("floor-sum-proto-held", &pb_rule, "order.proto", &pb("min_items: 5, max_items: 10"));
    assert!(codes(&f).is_empty(), "{f:?}");
    // Ranges that start at zero: nothing is said.
    let f = check("floor-none", &rule(">=0", ">=0USD"), "order.json", &open);
    assert!(codes(&f).is_empty(), "{f:?}");
}

/// One element adds no more to a total than the contract lets its field be, where that is under
/// the top of what the rule takes of it (§15.314). Ten lines of up to 50 dollars come to 500, which
/// the `sum` takes, though the rule takes a line of 100. Counted by the rule's range alone, the
/// total was said to reach 1000 dollars, and a contract that kept the rule's range was E122.
#[test]
fn the_top_of_a_total_counts_what_the_contract_lets_one_element_be() {
    let rule = "rule t v1\n\nshape order = jsonschema \"order.json\" \"#/$defs/Order\"\n\n\
                elements items from order.lines\n  qty    : number  range >=1 <=9  from qty\n  amount : money[USD]  range >=0USD <=100USD  from amount\n\n\
                outputs\n  x : bool\n\n\
                table b\npolicy unique\n| qty | -> big : bool |\n| >=3 | true          |\n| <3  | false         |\n\n\
                count n over items where big  range >=0 <=10\n\
                sum s over items of amount  range >=0USD <=500USD\n\n\
                table t\npolicy unique\n| n | s | -> x |\n| - | - | true |\n";
    let titles = |f: &[(String, String, String)]| f.iter().map(|(_, t, _)| t.clone()).collect::<Vec<_>>();
    let f = check("top-narrow", rule, "order.json", &lines_schema(",\"maxItems\":10", ",\"minimum\":0,\"maximum\":50"));
    assert!(codes(&f).is_empty(), "{f:?}");
    // One dollar more a line passes the top, and the narrowing offered counts lines of 51 dollars.
    let f = check("top-narrow-51", rule, "order.json", &lines_schema(",\"maxItems\":10", ",\"minimum\":0,\"maximum\":51"));
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].1.contains("契約は `order.lines` を 10 件まで通し、amount の合計は 510USD に届きますが、s は 500USD までしか受け付けません"), "{:?}", titles(&f));
    assert!(f[0].2.contains("一件の amount は 51USD まで"), "{f:?}");
    assert!(f[0].2.contains("\"fix\":{\"kind\":\"narrow_contract\",\"text\":\"\\\"maxItems\\\": 9\"}"), "{f:?}");
    // A contract as wide as the rule is counted at the rule's top.
    let f = check("top-wide", rule, "order.json", &lines_schema(",\"maxItems\":10", ",\"minimum\":0,\"maximum\":100"));
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].1.contains("amount の合計は 1000USD に届きますが、s は 500USD までしか受け付けません"), "{:?}", titles(&f));
    assert!(f[0].2.contains("\"text\":\"\\\"maxItems\\\": 5\"}"), "{f:?}");
}

/// A `unique` field of a sequence taken from a contract (§15.314): the door refuses two elements
/// that share a value of it, and nothing read from a contract keeps one field apart between
/// elements — `uniqueItems` compares whole elements — so two lines with one quantity pass the
/// contract and were refused with `check` saying nothing. It is E122 with no narrowing to offer,
/// and a collection of one element at most, which cannot hold two alike, says nothing. The same
/// from a `.proto`.
#[test]
fn a_unique_field_of_a_sequence_taken_from_a_contract_is_one_the_contract_does_not_promise() {
    let rule = |cap: &str| {
        format!(
            "rule t v1\n\nshape order = jsonschema \"order.json\" \"#/$defs/Order\"\n\n\
             elements items from order.lines\n  qty    : number  range >=1 <=9  unique  from qty\n  amount : money[USD]  range >=0USD <=100USD  from amount\n\n\
             outputs\n  x : bool\n\n\
             table b\npolicy unique\n| qty | -> big : bool |\n| >=3 | true          |\n| <3  | false         |\n\n\
             count n over items where big  range >=0 <={cap}\n\
             sum s over items of amount  range >=0USD <=1000USD\n\n\
             table t\npolicy unique\n| n | s | -> x |\n| - | - | true |\n"
        )
    };
    let amount = ",\"minimum\":0,\"maximum\":100";
    let f = check("unique-two", &rule("10"), "order.json", &lines_schema(",\"maxItems\":10", amount));
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].1.contains("契約は `order.lines` に qty が同じ要素が二つあっても通しますが、並び items は qty が同じ要素を二つ受け付けません"), "{f:?}");
    assert!(f[0].2.contains("契約には、要素どうしで `order.lines[].qty` が違うという決まりがありません"), "{f:?}");
    // The sentence the door says, with the two elements as `i` and `j` (§15.324).
    assert!(f[0].2.contains("items の要素に qty が同じものが二つあります（items[i] と items[j]）") && f[0].2.contains("`uniqueItems` は要素を丸ごと比べる"), "{f:?}");
    assert!(f[0].2.contains("\"fix\":{\"kind\":\"none\"}") && f[0].2.contains("qty の `unique` を外して"), "{f:?}");
    // `uniqueItems` does not promise it: two lines of one quantity and two amounts are two items.
    let f = check("unique-items", &rule("10"), "order.json", &lines_schema(",\"maxItems\":10,\"uniqueItems\":true", amount));
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    // Two lines can be alike; one cannot, whether the contract or the rule's `count` holds it to one.
    let f = check("unique-two-lines", &rule("10"), "order.json", &lines_schema(",\"maxItems\":2", amount));
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    let f = check("unique-one-line", &rule("10"), "order.json", &lines_schema(",\"maxItems\":1", amount));
    assert!(codes(&f).is_empty(), "{f:?}");
    let f = check("unique-one-counted", &rule("1"), "order.json", &lines_schema(",\"maxItems\":1", amount));
    assert!(codes(&f).is_empty(), "{f:?}");
    // A field that is not `unique` says nothing.
    let f = check("unique-none", &rule("10").replace("  unique  from qty", "  from qty"), "order.json", &lines_schema(",\"maxItems\":10", amount));
    assert!(codes(&f).is_empty(), "{f:?}");
    // From a `.proto`: the same, without the sentence about `uniqueItems`.
    let pb = proto(
        "message Order {\n  repeated Line lines = 1 [(buf.validate.field).repeated = {max_items: 10}];\n}\n\n\
         message Line {\n  int64 qty = 1 [(buf.validate.field).int64 = {gte: 1, lte: 9}];\n  int64 amount = 2 [(buf.validate.field).int64 = {gte: 0, lte: 100}];\n}",
    );
    let pb_rule = rule("10").replace("jsonschema \"order.json\" \"#/$defs/Order\"", "proto \"order.proto\" shop.v1.Order");
    assert!(pb_rule.contains("shape order = proto"), "{pb_rule}");
    let f = check("unique-proto", &pb_rule, "order.proto", &pb);
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].1.contains("qty が同じ要素を二つ受け付けません") && !f[0].2.contains("uniqueItems"), "{f:?}");
}

/// The weights of a pick over a sequence taken from a contract (§15.314): the walk adds them up in
/// an int64 and refuses the call the moment the total would pass it, so the longest collection the
/// contract lets through, each weight at its largest, has to fit. Three reviewers of up to four
/// quintillion each pass the contract and were refused with `check` saying nothing; it is E122,
/// and the narrowing offered is the length the total fits at. Two fit; so does a weight the
/// contract holds lower than the rule does; and a collection with no top has no top on its total.
#[test]
fn the_weights_of_a_pick_over_a_sequence_taken_from_a_contract_have_to_fit() {
    let rule = "rule t v1\n\nenum presence = away | in_office\n\nshape team = jsonschema \"team.json\" \"#/$defs/Team\"\n\n\
                elements reviewers from team.reviewers\n  reviewer_id : number  range >=1 <=50  from id\n  present     : bool  from present\n\
                \x20 capacity    : number  range >=0 <=4_000_000_000_000_000_000  from capacity\n\n\
                inputs\n  roll : number  range >=0 <=9999  random\n\n\
                outputs\n  reviewer : number  round down(1)\n\n\
                table in_today\npolicy unique\n| present | -> presence : presence |\n| true    | in_office              |\n| false   | away                   |\n\n\
                fold presence over reviewers\n  away      -> next\n  in_office -> take_weighted reviewer_id by capacity\n  draw      -> roll\n  empty     -> 0\n  exhausted -> held\n";
    let team = |extra: &str, top: &str| {
        format!(
            "{{\"$defs\":{{\"Team\":{{\"type\":\"object\",\"required\":[\"reviewers\"],\"properties\":{{\"reviewers\":{{\"type\":\"array\"{extra},\
             \"items\":{{\"type\":\"object\",\"required\":[\"id\",\"present\",\"capacity\"],\"properties\":{{\"id\":{{\"type\":\"integer\",\"minimum\":1,\"maximum\":50}},\
             \"present\":{{\"type\":\"boolean\"}},\"capacity\":{{\"type\":\"integer\",\"minimum\":0,\"maximum\":{top}}}}}}}}}}}}}}}}}"
        )
    };
    let wide = "4000000000000000000";
    let f = check("weights-three", rule, "team.json", &team(",\"maxItems\":3", wide));
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(
        f[0].1.contains("契約は `team.reviewers` を 3 件まで通し、重みの合計は 12_000_000_000_000_000_000 に届きますが、並び reviewers は、重みの合計が int64 に収まる並びしか受け付けません"),
        "{f:?}"
    );
    assert!(f[0].2.contains("一件の重みは 4_000_000_000_000_000_000 まで") && f[0].2.contains("reviewers の重みの合計が int64 に収まりません"), "{f:?}");
    assert!(f[0].2.contains("\"fix\":{\"kind\":\"narrow_contract\",\"text\":\"\\\"maxItems\\\": 2\"}"), "{f:?}");
    // Two reviewers at the most fit, as the fix says.
    let f = check("weights-two", rule, "team.json", &team(",\"maxItems\":2", wide));
    assert!(codes(&f).is_empty(), "{f:?}");
    // Seven reviewers of 1,317,624,576,693,539,401 at the most come to 2^63 − 1, the most an int64
    // holds: the total fits. One more each, and it is seven past.
    let f = check("weights-edge", rule, "team.json", &team(",\"maxItems\":7", "1317624576693539401"));
    assert!(codes(&f).is_empty(), "{f:?}");
    let f = check("weights-past", rule, "team.json", &team(",\"maxItems\":7", "1317624576693539402"));
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].1.contains("重みの合計は 9_223_372_036_854_775_814 に届きます") && f[0].2.contains("\\\"maxItems\\\": 6\""), "{f:?}");
    // A capacity the contract holds to 100 is counted at 100: ten reviewers come to 1000.
    let f = check("weights-narrow", rule, "team.json", &team(",\"maxItems\":10", "100"));
    assert!(codes(&f).is_empty(), "{f:?}");
    // No top on the collection: no top on the total, and the length offered is counted at 100.
    let f = check("weights-open", rule, "team.json", &team("", "100"));
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].1.contains("契約は `team.reviewers` の件数に上限を置いていないので、重みの合計にも上限がありません"), "{f:?}");
    assert!(f[0].2.contains("\\\"maxItems\\\": 92233720368547758\""), "{f:?}");
}

/// The range a `define` is accepted in (§15.313), where the value is worked out from what a contract
/// gives: the generated code refuses a call whose value falls outside, and an amount of 1,000 the
/// contract lets through has a fee of 3% past the 20 the rule accepts, with `check` saying
/// nothing. It is E122, with a request that takes the value outside as the example and no
/// annotation to offer; a contract that stops at 699, whose fee is 20, says nothing. The same of a
/// value worked out on each element from its fields, and nothing of one worked out from an input
/// no contract gives.
#[test]
fn the_range_a_define_is_accepted_in_is_held_to_what_the_contract_lets_through() {
    let rule = "rule t v1\n\nshape order = jsonschema \"order.json\" \"#/$defs/Order\"\n\n\
                inputs\n  amount : number  range >=1 <=1000  from order.amount\n\n\
                define fee : number = down(amount * 3%, 1)  range >=0 <=20\n\n\
                outputs\n  fee : number  round down(1)\n";
    let schema = |top: i64| format!("{{\"$defs\":{{\"Order\":{{\"type\":\"object\",\"required\":[\"amount\"],\"properties\":{{\"amount\":{{\"type\":\"integer\",\"minimum\":1,\"maximum\":{top}}}}}}}}}}}");
    let f = check("define-input", rule, "order.json", &schema(1000));
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].1.contains("契約が通す値で、規則の計算する fee が、受け付ける範囲 >=0 <=20 の外になります"), "{f:?}");
    assert!(f[0].2.contains("たとえば amount = 701 のとき。") && f[0].2.contains("`order.amount` から計算します") && f[0].2.contains("fee が範囲の外です"), "{f:?}");
    assert!(f[0].2.contains("\"fix\":{\"kind\":\"none\"}") && f[0].2.contains("契約で `order.amount` の範囲を狭めてください"), "{f:?}");
    // 3% of 699 is 20, rounded down; of 700, 21.
    let f = check("define-input-held", rule, "order.json", &schema(699));
    assert!(codes(&f).is_empty(), "{f:?}");
    let f = check("define-input-700", rule, "order.json", &schema(700));
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].2.contains("たとえば amount = 700 のとき。"), "{f:?}");
    // A value neither shown to stay inside nor found outside is said too, a contract being read no
    // narrower than it is: the difference of the amount and itself is 0, which the arithmetic of
    // intervals does not show.
    let f = check("define-undecided", &rule.replace("down(amount * 3%, 1)  range >=0 <=20", "amount - amount  range >=0 <=0"), "order.json", &schema(1000));
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].1.contains("契約が通す値で、規則の計算する fee が、受け付ける範囲 >=0 <=0 に収まると示せません"), "{f:?}");
    assert!(f[0].2.contains("\"fix\":{\"kind\":\"none\"}"), "{f:?}");
    // An amount the caller gives beside the object is the caller's to keep: nothing is said here.
    let beside = "rule t v1\n\nshape order = jsonschema \"order.json\" \"#/$defs/Order\"\n\n\
                  inputs\n  amount : number  range >=1 <=1000\n  seats  : number  range >=1 <=1000  from order.amount\n\n\
                  define fee : number = down(amount * 3%, 1)  range >=0 <=20\n\n\
                  outputs\n  fee : number  round down(1)\n  n   : number  round down(1)\n\ndefine n : number = seats\n";
    let f = check("define-beside", beside, "order.json", &schema(1000));
    assert!(codes(&f).is_empty(), "{f:?}");
    // On each element: a line of up to 9 at up to 100 comes to 900, and one is accepted up to 500.
    let lines = "rule t v1\n\nshape order = jsonschema \"order.json\" \"#/$defs/Order\"\n\n\
                 elements items from order.lines\n  qty    : number  range >=1 <=9  from qty\n  amount : number  range >=0 <=100  from amount\n\n\
                 define subtotal : number = qty * amount  range >=0 <=500\n\n\
                 sum total over items of subtotal  range >=0 <=9000\n\n\
                 outputs\n  x : number  round down(1)\n\ndefine x : number = total\n";
    let f = check("define-lines", lines, "order.json", &lines_schema(",\"maxItems\":10", ",\"minimum\":0,\"maximum\":100"));
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].1.contains("規則の計算する subtotal が、受け付ける範囲 >=0 <=500 の外になります"), "{f:?}");
    assert!(f[0].2.contains("`order.lines[].qty`、`order.lines[].amount` から計算します") && f[0].2.contains("たとえば items = [qty "), "{f:?}");
    // Lines of up to 55 come to 495 at the most.
    let f = check("define-lines-held", lines, "order.json", &lines_schema(",\"maxItems\":10", ",\"minimum\":0,\"maximum\":55"));
    assert!(codes(&f).is_empty(), "{f:?}");
}

/// From a `.proto`, a field of an element left unset arrives as its default, and a field with no
/// rule lets 0 through (§15.223, §15.133); a `repeated` with no `max_items` puts no end on a total,
/// and the narrowing offered is the Protovalidate rule that would.
#[test]
fn protoの並びと合計も既定値まで突き合わせる() {
    let body = "message Order {\n  repeated Line lines = 1;\n}\n\nmessage Line {\n  int64 qty = 1;\n  int64 amount = 2 [(buf.validate.field).int64 = {gte: 0, lte: 100}];\n}";
    let rule = "rule t v1\n\nshape 注文(order) = proto \"order.proto\" shop.v1.Order\n\n\
                inputs\n  a : money[USD]  range >=0USD <=1000USD  from sum 注文.lines of amount\n\n\
                elements items from 注文.lines\n  qty : number  range >=1 <=5  from qty\n\n\
                outputs\n  x : bool\n\n\
                table b\npolicy unique\n| qty | -> big : bool |\n| >=3 | true          |\n| <3  | false         |\n\n\
                count n over items where big  range >=0 <=10\n\n\
                table t\npolicy unique\n| a | n | -> x |\n| - | - | true |\n";
    let f = check("pb-seq", rule, "order.proto", &proto(body));
    let titles: Vec<&str> = f.iter().map(|(_, t, _)| t.as_str()).collect();
    assert_eq!(codes(&f), vec!["E122", "E122", "E122"], "{titles:?}");
    assert!(titles[0].contains("`注文.lines` の `amount` の合計が 1100 になるのを通しますが"), "{titles:?}");
    assert!(f[0].2.contains("\"text\":\"[(buf.validate.field).repeated = {max_items: 10}]\""), "{f:?}");
    assert!(titles[1].contains("契約は `注文.lines` が 11 件でも通しますが、並び items は 10 件までしか受け付けません"), "{titles:?}");
    assert!(titles[2].contains("契約は `注文.lines[].qty` に 0 を通しますが"), "{titles:?}");
    assert!(f[2].2.contains("[(buf.validate.field).int64 = {gte: 1, lte: 5}]"), "{f:?}");
}

/// An ordered enum input with a range takes only the values inside it, and that is what the
/// contract is held to (§15.251). A contract that lets platinum through where the range stops at
/// gold is E122, with the contract narrowed to the range as the fix; one that lets through only
/// the range's values says nothing. The input without a range, read from a field that lets every
/// value through, says nothing either. The range was not read here, so the first contract was
/// quiet and the door refused platinum. English first, then the Japanese version, one against a
/// JSON Schema and one against a `.proto`.
#[test]
fn an_enum_input_with_a_range_is_held_to_the_values_inside_it() {
    let rule = |shape: &str, en: &str, member: &str, campaign: &str, range: &str| {
        format!(
            "rule t(t) v1\n\nshape {shape}\n\nenum {en}\n\n\
             inputs\n  {member}\n  {campaign}  range {range}  from order.campaign\n\noutputs\n  x(x) : bool\n\n\
             table t1(t1)\npolicy unique\n| a | b | -> x |\n| - | - | true |\n"
        )
    };
    let quote = |vs: &[&str]| vs.iter().map(|v| format!("\"{v}\"")).collect::<Vec<_>>().join(", ");

    // English, against a JSON Schema.
    let en = rule(
        "order(order) = jsonschema \"order.json\" \"#/$defs/Order\"",
        "tier(tier) = basic(basic) < silver(silver) < gold(gold) < platinum(platinum)",
        "a(a) : tier  from order.tier",
        "b(b) : tier",
        ">=basic <=gold",
    );
    let schema = |campaign: &[&str]| {
        format!(
            "{{\"$defs\":{{\"Order\":{{\"type\":\"object\",\"properties\":{{\"tier\":{{\"type\":\"string\",\"enum\":[{}]}},\"campaign\":{{\"type\":\"string\",\"enum\":[{}]}}}},\"required\":[\"tier\",\"campaign\"]}}}}}}",
            quote(&["basic", "silver", "gold", "platinum"]).replace(", ", ","),
            quote(campaign).replace(", ", ","),
        )
    };
    let f = check("enum-range-en", &en, "order.json", &schema(&["basic", "silver", "gold", "platinum"]));
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].1.contains("`order.campaign`") && f[0].1.contains("platinum") && f[0].1.contains("受け付けません"), "{f:?}");
    assert!(f[0].2.contains(r#""witness":{"inputs":{"b":"platinum"}}"#), "{f:?}");
    assert!(f[0].2.contains(r#""fix":{"kind":"narrow_contract","text":"\"enum\": [\"basic\", \"silver\", \"gold\"]"}"#), "{f:?}");
    let f = check("enum-range-en-narrow", &en, "order.json", &schema(&["basic", "silver", "gold"]));
    assert!(codes(&f).is_empty(), "{f:?}");
    // One that lets through only a value past the range leaves nothing to narrow it to, and the
    // value is the enum's own name, not an alias the contract spells it by.
    let f = check("enum-range-en-past", &en, "order.json", &schema(&["platinum"]));
    assert_eq!(codes(&f).first(), Some(&"E122"), "{f:?}");
    assert!(f[0].2.contains(r#""kind":"none""#) && f[0].2.contains("範囲に入る値は一つもありません") && !f[0].2.contains("別名"), "{f:?}");

    // The Japanese version, against a `.proto` whose strings carry the names.
    let ja = rule(
        "注文(order) = proto \"order.proto\" shop.v1.Order",
        "段階(tier) = 一般(basic) < シルバー(silver) < ゴールド(gold) < プラチナ(platinum)",
        "a(a) : 段階  from 注文.tier",
        "b(b) : 段階",
        ">=一般 <=ゴールド",
    )
    .replace("from order.campaign", "from 注文.campaign");
    let body = |campaign: &[&str]| {
        proto(&format!(
            "message Order {{\n  string tier = 1 [(buf.validate.field).string = {{in: [{}]}}];\n  string campaign = 2 [(buf.validate.field).string = {{in: [{}]}}];\n}}",
            quote(&["一般", "シルバー", "ゴールド", "プラチナ"]),
            quote(campaign),
        ))
    };
    let f = check("enum-range-ja", &ja, "order.proto", &body(&["一般", "シルバー", "ゴールド", "プラチナ"]));
    assert_eq!(codes(&f), vec!["E122"], "{f:?}");
    assert!(f[0].1.contains("`注文.campaign`") && f[0].1.contains("プラチナ"), "{f:?}");
    assert!(f[0].2.contains(r#""fix":{"kind":"narrow_contract","text":"[(buf.validate.field).string = {in: [\"一般\", \"シルバー\", \"ゴールド\"]}]"}"#), "{f:?}");
    let f = check("enum-range-ja-narrow", &ja, "order.proto", &body(&["一般", "シルバー", "ゴールド"]));
    assert!(codes(&f).is_empty(), "{f:?}");
}
