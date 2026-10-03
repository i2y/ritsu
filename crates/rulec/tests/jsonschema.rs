//! `import jsonschema` (§15.60): the same binding as `import proto`, for the other place a
//! value set is declared.
//!
//! What differs from a `.proto` is only how the enum is named and how its values map to
//! aliases. A schema holds hundreds of enums in one document, so one is named by a JSON
//! Pointer; and its values are the strings that go on the wire, so they are the aliases
//! exactly, with nothing taken off and no case folded. What happens after that — E032 when
//! the sets differ, E033 when nobody has decided what a value costs — is shared, and the
//! tests here check that it really is the same behaviour and not a second implementation.

use std::path::PathBuf;
use std::process::Command;

use rulec::jsonschema::{self, Found};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rulec-schema-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn run(args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(root())
        .env("RULEC_LANG", "ja")
        .args(args)
        .output()
        .expect("rulec を起動できない");
    let mut s = String::from_utf8_lossy(&o.stdout).into_owned();
    s.push_str(&String::from_utf8_lossy(&o.stderr));
    (o.status.code().unwrap_or(-1), s)
}

const DOC: &str = r#"{
  "openapi": "3.1.0",
  "components": {
    "schemas": {
      "MemberTier": { "type": "string", "enum": ["basic", "gold"] },
      "Order": {
        "type": "object",
        "properties": { "status": { "type": "string", "enum": ["open", "closed"] } }
      }
    }
  }
}"#;

fn rule(values: &str, rows: &str) -> String {
    format!(
        "rule 配送料金(delivery_fee) v1\n\n\
         import jsonschema \"api.json\" \"#/components/schemas/MemberTier\" -> 会員区分\n\
         enum 会員区分(tier) = {values}\n\n\
         inputs\n  会員(m) : 会員区分\n\n\
         outputs\n  送料(fee) : money[円, incl_tax]  round up(10円)\n\n\
         table 送料表(fee_table)\npolicy first\n\
         | 会員 | -> 送料(fee) : money[円, incl_tax] |\n{rows}"
    )
}

fn pair(tag: &str, doc: &str, rule: &str) -> String {
    let d = dir(tag);
    std::fs::write(d.join("api.json"), doc).unwrap();
    let p = d.join("配送料金.rule");
    std::fs::write(&p, rule).unwrap();
    p.to_string_lossy().into_owned()
}

fn values_at(doc: &str, ptr: &str) -> Vec<String> {
    let j = jsonschema::read(doc, "api.json").expect("JSON として読めない");
    match jsonschema::values(&j, ptr) {
        Found::Values(v) => v,
        _ => panic!("値が読めない: {ptr}"),
    }
}

#[test]
fn ポインタは三つの書き方を同じに読む() {
    for ptr in ["#/components/schemas/MemberTier", "/components/schemas/MemberTier", "components/schemas/MemberTier"] {
        assert_eq!(values_at(DOC, ptr), ["basic", "gold"], "{ptr}");
    }
}

#[test]
fn スキーマでも_その_enum_の配列でも指せる() {
    assert_eq!(values_at(DOC, "#/components/schemas/MemberTier/enum"), ["basic", "gold"]);
    // OpenAPI は列挙を property の中に直接書くことも多い。同じポインタで届く。
    assert_eq!(values_at(DOC, "#/components/schemas/Order/properties/status"), ["open", "closed"]);
}

#[test]
fn 値はそのまま別名になる() {
    // `.proto` の接頭辞落としのような変換はしない。スキーマにその慣習が無いので、変換すれば
    // 「そろって見えるのに違う文字列を送る」規則ができる。
    let doc = r#"{"$defs": {"T": {"enum": ["GOLD", "gold_plus"]}}}"#;
    assert_eq!(values_at(doc, "#/$defs/T"), ["GOLD", "gold_plus"]);
}

#[test]
fn 一致していれば何も言わない() {
    let p = pair("ok", DOC, &rule("一般(basic) | ゴールド(gold) default", "| 一般 | 800円 |\n| - | 400円 |\n"));
    let (code, out) = run(&["check", &p]);
    assert_eq!(code, 0, "{out}");
}

#[test]
fn スキーマに値が増えたら止まる() {
    let grown = DOC.replace(r#""enum": ["basic", "gold"]"#, r#""enum": ["basic", "gold", "platinum"]"#);
    let p = pair("grown", &grown, &rule("一般(basic) | ゴールド(gold) default", "| 一般 | 800円 |\n| - | 400円 |\n"));
    let (code, out) = run(&["check", &p]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("E032") && out.contains("platinum"), "{out}");
}

#[test]
fn 既定の行があっても_決めていない値は止まる() {
    let p = pair("undecided", DOC, &rule("一般(basic) | ゴールド(gold)", "| 一般 | 800円 |\n| - | 400円 |\n"));
    let (code, out) = run(&["check", &p]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("E033") && out.contains("ゴールド"), "{out}");
    assert!(!out.contains("W111"), "{out}");
}

#[test]
fn 届かないポインタは_そこにある鍵を並べる() {
    let r = rule("一般(basic) | ゴールド(gold) default", "| - | 400円 |\n")
        .replace("#/components/schemas/MemberTier", "#/components/schemas/MemberTiers");
    let p = pair("nopointer", DOC, &r);
    let (code, out) = run(&["check", &p]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("E013"), "{out}");
    assert!(out.contains("MemberTier") && out.contains("Order"), "そこにある鍵を並べる: {out}");
}

#[test]
fn 列挙でないものと_名前でない値は断る() {
    let r = rule("一般(basic) | ゴールド(gold) default", "| - | 400円 |\n")
        .replace("#/components/schemas/MemberTier", "#/components/schemas/Order");
    let p = pair("notenum", DOC, &r);
    let (code, out) = run(&["check", &p]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("E013") && out.contains("列挙ではありません"), "{out}");

    let nums = r#"{"$defs": {"T": {"enum": [1, 2]}}}"#;
    let r2 = rule("一般(basic) | ゴールド(gold) default", "| - | 400円 |\n")
        .replace("#/components/schemas/MemberTier", "#/$defs/T");
    let p2 = pair("numbers", nums, &r2);
    let (code, out) = run(&["check", &p2]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("E013") && out.contains("名前でない値"), "{out}");
}

#[test]
fn yamlは名前で断る() {
    let yaml = "openapi: 3.1.0\ncomponents:\n  schemas:\n    MemberTier:\n      enum: [basic, gold]\n";
    let d = dir("yaml");
    std::fs::write(d.join("api.yaml"), yaml).unwrap();
    let p = d.join("配送料金.rule");
    let r = rule("一般(basic) | ゴールド(gold) default", "| - | 400円 |\n").replace("api.json", "api.yaml");
    std::fs::write(&p, r).unwrap();
    let (code, out) = run(&["check", &p.to_string_lossy()]);
    assert_eq!(code, 1, "{out}");
    // 半分読めたふりをするより、読めないと名乗って JSON にするやり方を言うほうがよい。
    assert!(out.contains("YAML は読めません") && out.contains("openapi.json"), "{out}");
}
