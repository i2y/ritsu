//! JSON without a dependency (DESIGN 4.9): read and written back the same, integers exact,
//! keys in their order, and the bytes serde_json writes, compact and pretty.

use ritsu_base::json::{self, Json};

#[test]
fn read_and_written_back_the_same() {
    for src in [
        r#"{"届け先":"北海道","x":"a\nb","n":-12,"f":1.50,"ok":true,"no":null,"list":[1,[2,{}],[]],"o":{"b":1,"a":2}}"#,
        r#"[]"#,
        r#"{}"#,
        r#""\"quoted\" and \\ back""#,
        r#"170141183460469231731687303715884105727"#,
    ] {
        let v = json::parse(src).unwrap_or_else(|e| panic!("{src}: {}", e.message.en));
        assert_eq!(v.compact(), src, "{src}");
        assert_eq!(json::parse(&v.pretty()).unwrap(), v, "{src}");
    }
}

#[test]
fn integers_are_exact_and_a_fraction_keeps_its_digits() {
    let v = json::parse(r#"{"big":9007199254740993,"neg":-170141183460469231731687303715884105728,"rate":0.1000}"#).unwrap();
    assert_eq!(v.get("big").and_then(Json::as_int), Some(9_007_199_254_740_993));
    assert_eq!(v.get("neg").and_then(Json::as_int), Some(i128::MIN));
    assert_eq!(v.get("rate"), Some(&Json::Frac("0.1000".into())));
    assert_eq!(v.get("rate").and_then(Json::as_int), None, "a fraction is not an integer");
}

#[test]
fn what_is_refused_and_where() {
    for (bad, at) in [(r#"{"a":1"#, 6), (r#"{"a":}"#, 5), (r#"{"a":1}x"#, 7), (r#"{"a":1e3}"#, 5), (r#"{"a":1,"a":2}"#, 7), ("[1,]", 3), (r#""a"#, 2), (r#""\x""#, 2), ("1.", 0), ("-", 0), ("\"a\u{1}b\"", 2)] {
        let e = json::parse(bad).expect_err(bad);
        assert_eq!(e.at, at, "{bad}: {}", e.message.en);
        assert!(!e.message.ja.is_empty() && !e.message.en.is_empty());
    }
    // What is wrong, for a tool that says it in words of its own (rulec).
    use json::Problem as P;
    for (bad, what) in [
        (r#"{"a":1}x"#, P::Extra),
        (r#"{"a" 1}"#, P::Expected(':')),
        ("", P::MissingValue),
        ("nul", P::Unreadable),
        ("1.", P::NoDigitAfterPoint),
        ("1e3", P::Exponent),
        ("1000000000000000000000000000000000000000", P::TooLarge),
        (r#""\u12""#, P::BadUnicodeEscape),
        (r#""\ud83d""#, P::LoneHighSurrogate),
        (r#""\ud83d\u0041""#, P::NotLowSurrogate),
        (r#""\x""#, P::UnknownEscape),
        ("\"a\u{1}b\"", P::ControlCharacter),
        (r#""a"#, P::Unclosed),
        ("[1 2]", P::CommaOrBracket),
        (r#"{"a":1 "b":2}"#, P::CommaOrBrace),
        (r#"{"a":1,"a":2}"#, P::DuplicateKey("a".into())),
    ] {
        assert_eq!(json::parse(bad).expect_err(bad).what, what, "{bad}");
    }
    assert_eq!(json::parse(&"[".repeat(json::MAX_DEPTH + 1)).expect_err("deep").what, P::TooDeep);
    let deep = "[".repeat(json::MAX_DEPTH + 1) + &"]".repeat(json::MAX_DEPTH + 1);
    assert!(json::parse(&deep).is_err(), "deeper than the limit");
    let ok = "[".repeat(json::MAX_DEPTH) + &"]".repeat(json::MAX_DEPTH);
    assert!(json::parse(&ok).is_ok(), "as deep as the limit");
}

#[test]
fn surrogate_pairs_and_escapes_are_read() {
    let v = json::parse(r#""\ud83d\ude00 \u00e9 \/ \b\f\r\t""#).unwrap();
    assert_eq!(v.as_str(), Some("\u{1F600} é / \u{8}\u{c}\r\t"));
    assert!(json::parse(r#""\ud83d""#).is_err(), "a high surrogate alone");
    assert!(json::parse(r#""\ud83d\u0041""#).is_err(), "a high surrogate with no low one");
}

#[test]
fn the_escapes_serde_json_writes() {
    let s = "\"\\\u{8}\u{c}\n\r\t\u{1}\u{1f}\u{7f}é日本";
    // serde_json writes `\b` and `\f` by name, other control characters as `\u00xx` (lowercase
    // hex), and passes DEL and every non-ASCII character through.
    assert_eq!(json::quote(s), "\"\\\"\\\\\\b\\f\\n\\r\\t\\u0001\\u001f\u{7f}é日本\"");
    assert_eq!(json::parse(&json::quote(s)).unwrap().as_str(), Some(s));
}

#[test]
fn compact_and_pretty_as_serde_json_writes_them() {
    let v = Json::obj([
        ("text", Json::str("yuen \"民法の期間.req\"")),
        ("n", Json::int(3)),
        ("items", Json::arr([Json::arr([Json::str("requirement"), Json::str("満了日_142条")])])),
        ("empty", Json::arr([])),
        ("none", Json::obj::<&str>([])),
        ("nested", Json::obj([("a", Json::Null), ("b", Json::Bool(false))])),
    ]);
    assert_eq!(
        v.compact(),
        r#"{"text":"yuen \"民法の期間.req\"","n":3,"items":[["requirement","満了日_142条"]],"empty":[],"none":{},"nested":{"a":null,"b":false}}"#
    );
    assert_eq!(
        v.pretty(),
        "{\n  \"text\": \"yuen \\\"民法の期間.req\\\"\",\n  \"n\": 3,\n  \"items\": [\n    [\n      \"requirement\",\n      \"満了日_142条\"\n    ]\n  ],\n  \"empty\": [],\n  \"none\": {},\n  \"nested\": {\n    \"a\": null,\n    \"b\": false\n  }\n}"
    );
    assert_eq!(format!("{}", Json::from(vec![1u64, 2])), "[1,2]");
}

#[test]
fn values_are_looked_into() {
    let v = json::parse(r#"{"a":{"b":[true,"x"]},"c":"d"}"#).unwrap();
    assert_eq!(v.get("a").and_then(|a| a.get("b")).and_then(Json::as_arr).map(|a| a.len()), Some(2));
    assert_eq!(v.get("c").and_then(Json::as_str), Some("d"));
    assert_eq!(v.as_obj().map(|o| o.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>()), Some(vec!["a", "c"]));
    assert_eq!(v.get("zzz"), None);
    assert!(Json::from(None::<String>).is_null());
    assert_eq!(Json::Bool(true).as_bool(), Some(true));
    assert_eq!(Json::Arr(vec![]).kind().en, "an array");
}
