//! What a contract lets through, held against what an input takes (§15.132, §15.133).
//!
//! The ledger's examples and the mutants reach the JSON Schema side. These reach the `.proto`
//! side, where the defaults protojson leaves out decide what arrives: a field with no rule lets
//! 0 through, an unset message lets everything under it through unvalidated, and an
//! `optional` field is unset rather than zero.

use std::path::PathBuf;
use std::process::Command;

/// A rule and its contract, checked; the codes and titles that came out, in order. Each call
/// gets a directory of its own: the tests run side by side in one process.
fn check(tag: &str, rule: &str, file: &str, contract: &str) -> Vec<(String, String, String)> {
    static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rulec-contract-{tag}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("r.rule"), rule).unwrap();
    std::fs::write(dir.join(file), contract).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(&dir)
        .args(["check", "r.rule", "--format", "json", "--lang", "ja"])
        .output()
        .expect("rulec を起動できない");
    let _ = std::fs::remove_dir_all(PathBuf::from(&dir));
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
fn 読まない規則は名前を注に出す() {
    let body = proto("message Order {\n  int32 points = 1 [(buf.validate.field).int32.gte = 1, (buf.validate.field).cel = {id: \"x\", expression: \"this <= 100\"}];\n}");
    let f = check("cel", &rule("a(a) : number  range >=1 <=100  from 注文.points"), "order.proto", &body);
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
fn 入力にならない型のフィールドはe120で名指す() {
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
