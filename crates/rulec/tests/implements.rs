//! `implements proto "<file>" <Service>` (§15.274): a rule served as the Connect service a contract
//! declares, so the names and the numbers on the wire are the contract's.
//!
//! What is held here is the line's whole path through rulec. `check` holds the rule to the contract
//! (E013 where the line cannot be followed, E076 where the two do not agree); `gen` copies the
//! contract in place of the rule's own `.proto`; `rulec api`, the port's facts and the page say the
//! contract's names and numbers; and a rule that does not write the line is said and generated as
//! before. The service the generated Python stands up, driven over the contract's numbers, is in
//! `tests/connect.rs`, where the proto toolchain is.

use ritsu_ports::{References, Rules};
use ritsu_testkit::TempDir;
use rulec::json::Json;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// A contract numbered as no rule would number it: the request's fields out of order with
/// `reserved` numbers between them, the answer's fields from 1 up in another order, `trace` at 15,
/// and a value of the enum at 7. A second service beside it is not the one the rule names.
const CONTRACT: &str = include_str!("implements/api/shop/v1/member_shipping.proto");

const CONTRACT_AT: &str = "api/shop/v1/member_shipping.proto";

/// The rule that implements `MemberShippingService`. Its line 2 is the `implements` line, its inputs
/// are on lines 10 to 12 and its outputs on 15 to 17, which the findings below are said at.
const RULE: &str = include_str!("implements/rules/member_tier_shipping.rule");

const RULE_AT: &str = "rules/member_tier_shipping.rule";

const IMPLEMENTS_LINE: &str = "implements proto \"../api/shop/v1/member_shipping.proto\" MemberShippingService";

/// Writes `files` under a directory of its own.
fn tree(tag: &str, files: &[(&str, &str)]) -> (TempDir, PathBuf) {
    let t = TempDir::new(&format!("implements-{tag}"));
    let d = t.path().to_path_buf();
    for (at, text) in files {
        let p = d.join(at);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    (t, d)
}

/// The contract and the rule, either of them changed.
fn pair(tag: &str, contract: &str, rule: &str) -> (TempDir, PathBuf) {
    tree(tag, &[(CONTRACT_AT, contract), (RULE_AT, rule)])
}

/// rulec run in `dir`, in `lang`: the exit code and what it printed.
fn rulec(dir: &Path, lang: &str, args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).current_dir(dir).env("RULEC_LANG", lang).args(args).output().expect("rulec を起動できない");
    let mut s = String::from_utf8_lossy(&o.stdout).into_owned();
    s.push_str(&String::from_utf8_lossy(&o.stderr));
    (o.status.code().unwrap_or(-1), s)
}

fn api(dir: &Path, rule: &str) -> Json {
    let (code, out) = rulec(dir, "en", &["api", rule]);
    assert_eq!(code, 0, "{out}");
    rulec::json::parse(&out).expect("JSON でない")
}

fn items(j: Option<&Json>) -> Vec<Json> {
    match j {
        Some(Json::Arr(v)) => v.clone(),
        _ => Vec::new(),
    }
}

/// The finding `code` with `title`, said at `line` of the rule.
fn said_at(out: &str, lang: &str, code: &str, title: &str, line: usize) {
    let head = if lang == "ja" { "エラー" } else { "error" };
    let want = format!("{head}[{code}]: {title}\n  --> {RULE_AT}:{line}\n");
    assert!(out.contains(&want), "{want:?} が出ていません:\n{out}");
}

#[test]
fn コーパスの規則は何も言われない() {
    // No rule of the corpus writes the line, and every one of them passes as it did.
    let corpus: Vec<String> = std::fs::read_dir(root().join("tests/corpus"))
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "rule"))
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    assert!(corpus.len() > 100, "{}", corpus.len());
    let mut args = vec!["check"];
    args.extend(corpus.iter().map(String::as_str));
    let (code, out) = rulec(&root(), "en", &args);
    assert_eq!(code, 0, "{out}");
    assert!(!out.contains("E076") && !out.contains("implements"), "{out}");
}

/// The contract numbers every field otherwise than rulec's own `.proto` would, and the rule passes:
/// what may differ is the numbers, the order, the names of the service, the method and the messages,
/// `reserved`, and the service written with its package.
#[test]
fn 番号を違えて振った契約を実装する規則は通る() {
    let (_t, d) = pair("pass", CONTRACT, RULE);
    for lang in ["en", "ja"] {
        let (code, out) = rulec(&d, lang, &["check", RULE_AT]);
        assert_eq!(code, 0, "{out}");
        assert_eq!(out.trim(), format!("ok {RULE_AT}"), "{out}");
    }
    let full = RULE.replace("\" MemberShippingService", "\" shop.v1.MemberShippingService");
    let (_t, d) = pair("full-name", CONTRACT, &full);
    let (code, out) = rulec(&d, "en", &["check", RULE_AT]);
    assert_eq!(code, 0, "{out}");
}

/// `gen` copies the contract byte for byte to where its package puts it and writes no `.proto` of
/// the rule's own; the service and the runner speak the contract's service, method and messages.
#[test]
fn gen_は契約を写し_自前の_proto_を書かない() {
    let (_t, d) = pair("gen", CONTRACT, RULE);
    let (code, out) = rulec(&d, "en", &["gen", RULE_AT, "--out", "out"]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(std::fs::read_to_string(d.join("out/proto/shop/v1/member_shipping.proto")).unwrap(), CONTRACT);
    assert!(!d.join("out/proto/rulec").exists(), "規則の .proto を書いた: {out}");
    assert!(!out.contains("proto/rulec/"), "{out}");
    let svc = std::fs::read_to_string(d.join("out/python/member_tier_shipping_service.py")).unwrap();
    for want in [
        "from stubs.shop.v1 import member_shipping_pb as pb",
        "from stubs.shop.v1.member_shipping_connect import (",
        "MemberShippingService,",
        "def quote(",
        "pb.QuoteRequest",
        "pb.QuoteResponse",
        "pb.FiredRow(",
    ] {
        assert!(svc.contains(want), "{want} が無い:\n{svc}");
    }
    assert!(!svc.contains("Decide") && !svc.contains("DecideRequest"), "{svc}");
    let runner = std::fs::read_to_string(d.join("out/python/member_tier_shipping_connect_runner.py")).unwrap();
    for want in ["MemberShippingServiceClientSync", "client.quote(", "pb.QuoteRequest("] {
        assert!(runner.contains(want), "{want} が無い:\n{runner}");
    }
}

/// What `rulec api` says of the service is the contract's: its path, package, service, method and
/// messages, and every number as ritsu-proto reads it out of the contract. `contract` says which
/// file it is.
#[test]
fn api_の名前と番号は契約のもの() {
    let (_t, d) = pair("api", CONTRACT, RULE);
    let a = api(&d, RULE_AT);
    let c = a.get("connect").expect("connect が無い");
    let s = |k: &str| c.get(k).and_then(|x| x.as_str()).map(str::to_string);
    assert_eq!(s("path").as_deref(), Some("/shop.v1.MemberShippingService/Quote"));
    assert_eq!(s("package").as_deref(), Some("shop.v1"));
    assert_eq!(s("service").as_deref(), Some("MemberShippingService"));
    assert_eq!(s("method").as_deref(), Some("Quote"));
    assert_eq!(s("request").as_deref(), Some("QuoteRequest"));
    assert_eq!(s("response").as_deref(), Some("QuoteResponse"));
    assert_eq!(s("proto").as_deref(), Some("proto/shop/v1/member_shipping.proto"));
    assert_eq!(s("idempotency_level").as_deref(), Some("NO_SIDE_EFFECTS"));
    let contract = c.get("contract").expect("contract が無い");
    assert_eq!(contract.get("file").and_then(|x| x.as_str()), Some("../api/shop/v1/member_shipping.proto"));
    assert_eq!(contract.get("proto").and_then(|x| x.as_str()), Some("proto/shop/v1/member_shipping.proto"));
    assert_eq!(c.get("python").and_then(|p| p.get("client")).and_then(|x| x.as_str()), Some("MemberShippingServiceClientSync"));
    // every number, against the contract read on its own
    let pf = ritsu_proto::read(CONTRACT_AT, CONTRACT).expect("the contract reads");
    let number = |msg: &str, field: &str| pf.message(msg).and_then(|m| m.fields.iter().find(|f| f.name == field)).map(|f| f.number as i128);
    let mut seen = 0;
    for (key, msg) in [("request_fields", "QuoteRequest"), ("response_fields", "QuoteResponse")] {
        for f in items(c.get(key)) {
            let field = f.get("field").and_then(|x| x.as_str()).unwrap().to_string();
            assert_eq!(f.get("number").and_then(|x| x.as_int()), number(msg, &field), "{key} {field}");
            seen += 1;
        }
    }
    assert_eq!(seen, 6);
    // the enum's values, by the contract's numbers
    let tier = items(c.get("enums")).into_iter().find(|e| e.get("alias").and_then(|x| x.as_str()) == Some("Tier")).expect("Tier が無い");
    let values: Vec<(String, i128)> = items(tier.get("values"))
        .iter()
        .map(|v| (v.get("alias").and_then(|x| x.as_str()).unwrap_or_default().to_string(), v.get("number").and_then(|x| x.as_int()).unwrap_or(-1)))
        .collect();
    assert!(values.contains(&("TIER_PLATINUM".to_string(), 7)), "{values:?}");
}

/// A request whose sequence is a message nested in it, numbered on its own, and an answer whose rows
/// are a message of another file in another package: the contract's files are each copied where
/// their package puts them, and the numbers are each message's own.
#[test]
fn 入れ子のメッセージと別のファイルのメッセージを読む() {
    let rows = include_str!("implements/api/shop/common/v1/rows.proto");
    let cart = include_str!("implements/api/shop/v1/cart.proto");
    let rule = include_str!("implements/rules/cart_fee.rule");
    let (_t, d) = tree("nested", &[("api/shop/common/v1/rows.proto", rows), ("api/shop/v1/cart.proto", cart), ("rules/cart_fee.rule", rule)]);
    let (code, out) = rulec(&d, "en", &["check", "rules/cart_fee.rule"]);
    assert_eq!(code, 0, "{out}");
    let c = api(&d, "rules/cart_fee.rule").get("connect").cloned().expect("connect が無い");
    assert_eq!(c.get("path").and_then(|x| x.as_str()), Some("/shop.v1.CartService/Price"));
    let field = |key: &str, name: &str| items(c.get(key)).into_iter().find(|f| f.get("field").and_then(|x| x.as_str()) == Some(name)).expect(name);
    let lines = field("request_fields", "lines");
    assert_eq!(lines.get("number").and_then(|x| x.as_int()), Some(3));
    assert_eq!(lines.get("type").and_then(|x| x.as_str()), Some("repeated CartRequest.Line"));
    assert_eq!(field("request_fields", "member").get("number").and_then(|x| x.as_int()), Some(1));
    assert_eq!(field("element_fields", "price").get("number").and_then(|x| x.as_int()), Some(2));
    assert_eq!(field("response_fields", "fee").get("number").and_then(|x| x.as_int()), Some(4));
    let (code, out) = rulec(&d, "en", &["gen", "rules/cart_fee.rule", "--out", "out"]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(std::fs::read_to_string(d.join("out/proto/shop/v1/cart.proto")).unwrap(), cart);
    assert_eq!(std::fs::read_to_string(d.join("out/proto/shop/common/v1/rows.proto")).unwrap(), rows);
    assert!(!d.join("out/proto/rulec").exists());
    let svc = std::fs::read_to_string(d.join("out/python/cart_fee_service.py")).unwrap();
    assert!(svc.contains("from stubs.shop.common.v1 import rows_pb as shop_common_v1_rows_pb"), "{svc}");
    assert!(svc.contains("shop_common_v1_rows_pb.MatchedRow("), "{svc}");
    let runner = std::fs::read_to_string(d.join("out/python/cart_fee_connect_runner.py")).unwrap();
    assert!(runner.contains("pb.CartRequest.Line(price="), "{runner}");
}

/// The way over: the `.proto` rulec wrote for the rule is copied out as the contract, the rule reads
/// its enum from it and writes the line. The path, the numbers and the names stay as they were, so
/// a caller meets nothing new; `rulec api` gains `contract` and nothing else, and `gen` writes the
/// same `.proto`, now as a copy of the contract. Half way, the enum read from the copy and the line
/// not yet written, `gen` stops rather than let one of the two files at that path replace the other.
#[test]
fn 生成した_proto_を写して契約にすると_パスも番号も変わらない() {
    let own = RULE.replace(&format!("{IMPLEMENTS_LINE}\n"), "").replace("import proto \"../api/shop/v1/member_shipping.proto\" Tier -> tier\n", "");
    let (_t, d) = tree("migrate", &[(RULE_AT, own.as_str())]);
    let before = api(&d, RULE_AT).get("connect").cloned().unwrap();
    let (code, out) = rulec(&d, "en", &["gen", RULE_AT, "--out", "before"]);
    assert_eq!(code, 0, "{out}");
    let at = "rulec/member_tier_shipping/v1/member_tier_shipping.proto";
    let generated = std::fs::read_to_string(d.join("before/proto").join(at)).unwrap();
    std::fs::create_dir_all(d.join("api/rulec/member_tier_shipping/v1")).unwrap();
    std::fs::write(d.join("api").join(at), &generated).unwrap();

    // half way: the enum is read from the copy, and the line is not written yet
    let file = format!("../api/{at}");
    let half = own.replace("enum tier = basic < silver < gold < platinum\n", &format!("enum tier = basic < silver < gold < platinum\nimport proto \"{file}\" Tier -> tier\n"));
    std::fs::write(d.join(RULE_AT), &half).unwrap();
    let (code, out) = rulec(&d, "en", &["check", RULE_AT]);
    assert_eq!(code, 0, "{out}");
    let (code, out) = rulec(&d, "en", &["gen", RULE_AT, "--out", "half"]);
    assert_eq!(code, 1, "{out}");
    // the contract is named where it was found, from where gen was run
    assert!(out.contains(&format!("error: `api/{at}` would be copied to `proto/{at}`, the path of the `.proto` generated from `{RULE_AT}`, and the two differ, so nothing is generated.")), "{out}");
    assert!(out.contains("write `implements proto \"<file>\" <Service>` in"), "{out}");
    let (code, out) = rulec(&d, "ja", &["gen", RULE_AT, "--out", "half"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains(&format!("`api/{at}` を写す先の `proto/{at}` は、`{RULE_AT}` から生成する `.proto` と同じパスで、二つは中身が違うので、生成しません。")), "{out}");

    // the line written
    let after = half.replace("rule member_tier_shipping v1\n", &format!("rule member_tier_shipping v1\nimplements proto \"{file}\" MemberTierShippingService\n"));
    std::fs::write(d.join(RULE_AT), &after).unwrap();
    let (code, out) = rulec(&d, "en", &["check", RULE_AT]);
    assert_eq!(code, 0, "{out}");
    let mut now = api(&d, RULE_AT).get("connect").cloned().unwrap();
    let Json::Obj(kv) = &mut now else { panic!("connect is not an object") };
    let contract = kv.iter().position(|(k, _)| k == "contract").expect("contract が無い");
    let (_, c) = kv.remove(contract);
    assert_eq!(c.get("proto").and_then(|x| x.as_str()), Some(format!("proto/{at}").as_str()));
    // the enum, now read from the contract, says so as every enum read with `import proto` does
    for (k, v) in kv.iter_mut() {
        if k == "enums"
            && let Json::Arr(es) = v
            && let Some(Json::Obj(e)) = es.first_mut()
            && let Some((_, from)) = e.iter_mut().find(|(k, _)| k == "contract")
        {
            assert_eq!(*from, c, "列挙の契約と implements の契約が違う");
            *from = Json::Null;
        }
    }
    assert_eq!(now, before, "contract のほかに変わったものがある");
    let (code, out) = rulec(&d, "en", &["gen", RULE_AT, "--out", "after"]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(std::fs::read_to_string(d.join("after/proto").join(at)).unwrap(), generated);
}

/// The rule as it was before the line: its own `.proto`, and the enum declared in the rule.
fn before_the_line() -> String {
    RULE.replace(&format!("{IMPLEMENTS_LINE}\n"), "").replace("import proto \"../api/shop/v1/member_shipping.proto\" Tier -> tier\n", "")
}

/// What `gen --format json` lists under `left`.
fn left(out: &str) -> Vec<String> {
    let j = rulec::json::parse(out.trim()).unwrap_or_else(|e| panic!("JSON でない: {e}\n{out}"));
    items(j.get("left")).iter().map(|v| v.as_str().expect("left のパスが文字列でない").to_string()).collect()
}

/// The `.proto` an earlier `gen` wrote for the rule.
const OWN_AT: &str = "out/proto/rulec/member_tier_shipping/v1/member_tier_shipping.proto";

/// A rule that moved to a contract at another path leaves the `.proto` `gen` wrote for it before.
/// `gen` deletes nothing in its output directory, so it names the file as generated no more and
/// says it can be deleted: in the text and under `left` of `--format json`, with or without
/// `--check`, and the exit code is what it would be without the file (§15.275).
#[test]
fn 別のパスの契約へ移った規則では前の_proto_が挙がる() {
    let (_t, d) = tree("left", &[(RULE_AT, before_the_line().as_str()), (CONTRACT_AT, CONTRACT)]);
    let (code, out) = rulec(&d, "en", &["gen", RULE_AT, "--out", "out"]);
    assert_eq!(code, 0, "{out}");
    let before = std::fs::read_to_string(d.join(OWN_AT)).unwrap();
    assert!(!out.contains("generated no more"), "{out}");

    std::fs::write(d.join(RULE_AT), RULE).unwrap();
    let (code, out) = rulec(&d, "en", &["gen", RULE_AT, "--out", "out"]);
    assert_eq!(code, 0, "{out}");
    let said = format!(
        "generated no more, and still there: {OWN_AT}\nnote: `{RULE_AT}` implements a contract's service (`implements`), so gen writes no `.proto` of the rule's own; gen deletes nothing in the output directory, and this file can be deleted\n"
    );
    assert!(out.ends_with(&said), "{out}");
    assert!(out.contains("generated: out/proto/shop/v1/member_shipping.proto\n"), "{out}");
    assert_eq!(std::fs::read_to_string(d.join(OWN_AT)).unwrap(), before, "gen が前の .proto を書き換えた");
    let (code, out) = rulec(&d, "en", &["gen", RULE_AT, "--out", "out", "--format", "json"]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(left(&out), [OWN_AT]);
    assert_eq!(items(rulec::json::parse(out.trim()).unwrap().get("written")).len(), 0, "{out}");

    // under `--check` too, and the run passes: what it writes is up to date
    let (code, out) = rulec(&d, "en", &["gen", RULE_AT, "--out", "out", "--check"]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(out, said);
    let (code, out) = rulec(&d, "en", &["gen", RULE_AT, "--out", "out", "--check", "--format", "json"]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(left(&out), [OWN_AT]);
    // and one stale file still fails it, the left one named beside it
    let svc = d.join("out/python/member_tier_shipping_service.py");
    std::fs::write(&svc, "# edited\n").unwrap();
    let (code, out) = rulec(&d, "en", &["gen", RULE_AT, "--out", "out", "--check", "--format", "json"]);
    assert_eq!(code, 1, "{out}");
    assert_eq!(left(&out), [OWN_AT]);
    assert_eq!(items(rulec::json::parse(out.trim()).unwrap().get("stale")).len(), 1, "{out}");

    // in Japanese (the generated code is written again, in Japanese)
    let (code, out) = rulec(&d, "ja", &["gen", RULE_AT, "--out", "out"]);
    assert_eq!(code, 0, "{out}");
    let said = format!("もう生成しないファイルが残っています: {OWN_AT}\n注記: `{RULE_AT}` は契約のサービスを実装する（`implements`）ので、gen は規則の `.proto` をもう生成しません。gen は出力先のファイルを消さないので、このファイルは消してかまいません\n");
    assert!(out.ends_with(&said), "{out}");

    // deleted, it is named no more
    std::fs::remove_file(d.join(OWN_AT)).unwrap();
    let (code, out) = rulec(&d, "en", &["gen", RULE_AT, "--out", "out", "--format", "json"]);
    assert_eq!(code, 0, "{out}");
    assert!(left(&out).is_empty(), "{out}");
}

/// A rule whose contract is a copy of the `.proto` `gen` wrote for it, at the same path, leaves
/// nothing: the copy is written where the `.proto` was (§15.274, §15.275).
#[test]
fn 生成した_proto_をコピーして同じパスへ移った規則では何も挙がらない() {
    let own = before_the_line();
    let (_t, d) = tree("same-path", &[(RULE_AT, own.as_str())]);
    let (code, out) = rulec(&d, "en", &["gen", RULE_AT, "--out", "out"]);
    assert_eq!(code, 0, "{out}");
    let at = "rulec/member_tier_shipping/v1/member_tier_shipping.proto";
    std::fs::create_dir_all(d.join("api/rulec/member_tier_shipping/v1")).unwrap();
    std::fs::copy(d.join(OWN_AT), d.join("api").join(at)).unwrap();
    let file = format!("../api/{at}");
    let after = own
        .replace("enum tier = basic < silver < gold < platinum\n", &format!("enum tier = basic < silver < gold < platinum\nimport proto \"{file}\" Tier -> tier\n"))
        .replace("rule member_tier_shipping v1\n", &format!("rule member_tier_shipping v1\nimplements proto \"{file}\" MemberTierShippingService\n"));
    std::fs::write(d.join(RULE_AT), &after).unwrap();
    for lang in ["en", "ja"] {
        let (code, out) = rulec(&d, lang, &["gen", RULE_AT, "--out", "out"]);
        assert_eq!(code, 0, "{out}");
        assert!(!out.contains("generated no more") && !out.contains("もう生成しない"), "{out}");
    }
    let (code, out) = rulec(&d, "en", &["gen", RULE_AT, "--out", "out", "--format", "json"]);
    assert_eq!(code, 0, "{out}");
    assert!(left(&out).is_empty(), "{out}");
    let (code, out) = rulec(&d, "en", &["gen", RULE_AT, "--out", "out", "--check", "--format", "json"]);
    assert_eq!(code, 0, "{out}");
    assert!(left(&out).is_empty(), "{out}");
}

/// Only a file whose first line is rulec's head is taken as left by `gen`: one a person put at the
/// path, or another generator wrote, is not named (§15.275).
#[test]
fn 頭の行の無いファイルは挙がらない() {
    let (_t, d) = pair("by-hand", CONTRACT, RULE);
    std::fs::create_dir_all(d.join(OWN_AT).parent().unwrap()).unwrap();
    let body = "syntax = \"proto3\";\n\npackage rulec.member_tier_shipping.v1;\n";
    for head in ["", "// Kept by hand. DO NOT EDIT.\n", "// Code generated by protoc-gen-x 1.0.0. DO NOT EDIT.\n", "\n// Code generated by rulec 0.38.0. DO NOT EDIT.\n"] {
        std::fs::write(d.join(OWN_AT), format!("{head}{body}")).unwrap();
        let (code, out) = rulec(&d, "en", &["gen", RULE_AT, "--out", "out"]);
        assert_eq!(code, 0, "{out}");
        assert!(!out.contains("generated no more"), "{head:?}: {out}");
        let (code, out) = rulec(&d, "en", &["gen", RULE_AT, "--out", "out", "--format", "json"]);
        assert_eq!(code, 0, "{out}");
        assert!(left(&out).is_empty(), "{head:?}: {out}");
    }
    // the same file with rulec's head, of any version, is
    std::fs::write(d.join(OWN_AT), format!("// Code generated by rulec 0.38.0. DO NOT EDIT.\n{body}")).unwrap();
    let (code, out) = rulec(&d, "en", &["gen", RULE_AT, "--out", "out", "--format", "json"]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(left(&out), [OWN_AT]);
}

#[test]
fn 行を辿れなければ_e013() {
    // the file is not there
    let (_t, d) = pair("missing", CONTRACT, &RULE.replace(IMPLEMENTS_LINE, "implements proto \"../api/shop/v1/nope.proto\" MemberShippingService"));
    let (code, out) = rulec(&d, "en", &["check", RULE_AT]);
    assert_eq!(code, 1, "{out}");
    said_at(&out, "en", "E013", "Cannot read `../api/shop/v1/nope.proto`", 2);
    assert!(out.contains("The path is followed from the directory of the rule file (looked for: "), "{out}");
    let (_, out) = rulec(&d, "ja", &["check", RULE_AT]);
    said_at(&out, "ja", "E013", "`../api/shop/v1/nope.proto` を読めません", 2);
    assert!(out.contains("パスは規則ファイルのある場所からたどります（探した先: "), "{out}");

    // the file has no such service, and the ones it has are named
    let (_t, d) = pair("service", CONTRACT, &RULE.replace("\" MemberShippingService", "\" ShippingService"));
    let (code, out) = rulec(&d, "en", &["check", RULE_AT]);
    assert_eq!(code, 1, "{out}");
    said_at(&out, "en", "E013", "`../api/shop/v1/member_shipping.proto` has no service `ShippingService`", 2);
    assert!(out.contains("The services in that file: MemberShippingService, PlainService"), "{out}");
    let (_, out) = rulec(&d, "ja", &["check", RULE_AT]);
    said_at(&out, "ja", "E013", "`../api/shop/v1/member_shipping.proto` にサービス `ShippingService` はありません", 2);
    assert!(out.contains("そのファイルにあるサービス: MemberShippingService, PlainService"), "{out}");

    // the line is not the shape, and what follows the service is marked
    for (tag, line) in [
        ("no-proto", "implements \"../api/shop/v1/member_shipping.proto\" MemberShippingService"),
        ("no-service", "implements proto \"../api/shop/v1/member_shipping.proto\""),
        ("method", "implements proto \"../api/shop/v1/member_shipping.proto\" MemberShippingService Quote"),
    ] {
        let (_t, d) = pair(tag, CONTRACT, &RULE.replace(IMPLEMENTS_LINE, line));
        let (code, out) = rulec(&d, "en", &["check", RULE_AT]);
        assert_eq!(code, 1, "{out}");
        said_at(&out, "en", "E013", "The shape of the `implements` line is wrong", 2);
        assert!(out.contains("The shape is `implements proto \"<file>\" <Service>`"), "{out}");
        let (_, ja) = rulec(&d, "ja", &["check", RULE_AT]);
        said_at(&ja, "ja", "E013", "`implements` の行の形が違います", 2);
        if tag == "method" {
            assert!(out.contains("nothing follows the service") && ja.contains("サービスの名前の後ろには何も書けません"), "{out}{ja}");
        }
    }

    // two lines: the second is said, at the first
    let two = RULE.replace(&format!("{IMPLEMENTS_LINE}\n"), &format!("{IMPLEMENTS_LINE}\nimplements proto \"../api/shop/v1/member_shipping.proto\" PlainService\n"));
    let (_t, d) = pair("two", CONTRACT, &two);
    let (code, out) = rulec(&d, "en", &["check", RULE_AT]);
    assert_eq!(code, 1, "{out}");
    said_at(&out, "en", "E013", "There are two `implements` lines", 3);
    assert!(out.contains("the first is on line 2"), "{out}");
    let (_, out) = rulec(&d, "ja", &["check", RULE_AT]);
    said_at(&out, "ja", "E013", "`implements` の行が二行あります", 3);
    assert!(out.contains("2 行目に一行目があります"), "{out}");
}

/// One case of E076: the contract or the rule changed one way, and the finding each language says,
/// at a line of the rule, with a note that names the place in the contract.
struct Off {
    tag: &'static str,
    contract: (&'static str, &'static str),
    rule: (&'static str, &'static str),
    line: usize,
    en: &'static str,
    ja: &'static str,
    note_en: &'static str,
    note_ja: &'static str,
}

const OFF: &[Off] = &[
    // a value of the rule's the contract does not have, said at the value
    Off {
        tag: "missing",
        contract: ("  optional int64 subtotal = 5;\n", ""),
        rule: ("", ""),
        line: 12,
        en: "The input `subtotal` is not in the contract's `QuoteRequest`",
        ja: "入力 `subtotal` が、契約の `QuoteRequest` にありません",
        note_en: "Add `optional int64 subtotal = <number>;` to the contract's `QuoteRequest`, with a number no field of `QuoteRequest` has ever used.",
        note_ja: "契約の `QuoteRequest` に `optional int64 subtotal = <番号>;` を足してください。",
    },
    // a field the rule does not have, said at the line
    Off {
        tag: "extra",
        contract: ("  optional Tier campaign = 9;\n", "  optional Tier campaign = 9;\n  optional string coupon_code = 11;\n"),
        rule: ("", ""),
        line: 2,
        en: "The contract's `QuoteRequest` has `coupon_code`, which the rule does not",
        ja: "契約の `QuoteRequest` の `coupon_code` が、規則にありません",
        note_en: "`optional string coupon_code = 11;` is on line 25 of ../api/shop/v1/member_shipping.proto.",
        note_ja: "`optional string coupon_code = 11;` は ../api/shop/v1/member_shipping.proto の 25 行目にあります。",
    },
    Off {
        tag: "type",
        contract: ("optional int64 subtotal = 5;", "optional string subtotal = 5;"),
        rule: ("", ""),
        line: 12,
        en: "`subtotal` has another type in the contract",
        ja: "`subtotal` の型が、契約と違います",
        note_en: "A rule's service has it as `optional int64`. The contract's `QuoteRequest` has `optional string subtotal = 5;` (line 23 of ../api/shop/v1/member_shipping.proto).",
        note_ja: "規則のサービスでは `optional int64` です。契約の `QuoteRequest` には `optional string subtotal = 5;` とあります（../api/shop/v1/member_shipping.proto の 23 行目）。",
    },
    Off {
        tag: "presence",
        contract: ("optional int64 subtotal = 5;", "int64 subtotal = 5;"),
        rule: ("", ""),
        line: 12,
        en: "Whether `subtotal` is `optional` does not agree with the contract",
        ja: "`subtotal` の `optional` が、契約と合っていません",
        note_en: "A field of the request needs `optional`",
        note_ja: "要求のフィールドには `optional` が要ります。",
    },
    Off {
        tag: "presence-answer",
        contract: ("  bool lifted = 7;", "  optional bool lifted = 7;"),
        rule: ("", ""),
        line: 17,
        en: "Whether `lifted` is `optional` does not agree with the contract",
        ja: "`lifted` の `optional` が、契約と合っていません",
        note_en: "`lifted` always has a value, so the field of the answer is not `optional`.",
        note_ja: "`lifted` にはいつも値があるので、答えのフィールドに `optional` は付けません。",
    },
    // the rule's own enum, said once, at the first value that carries it
    Off {
        tag: "enum",
        contract: ("", ""),
        rule: ("import proto \"../api/shop/v1/member_shipping.proto\" Tier -> tier\n", "# the enum is the rule's own\n"),
        line: 10,
        en: "The enum `tier` is not read from a contract",
        ja: "列挙 `tier` は、契約から読んだものではありません",
        note_en: "Declare the enum in the contract's `.proto`, and read it with `import proto \"<file>\" <Enum> -> tier`.",
        note_ja: "列挙を契約の `.proto` に書き、`import proto \"<ファイル>\" <列挙> -> tier` で読んでください。",
    },
    Off {
        tag: "no-trace",
        contract: ("  repeated FiredRow trace = 15;\n", ""),
        rule: ("", ""),
        line: 2,
        en: "The contract's `QuoteResponse` has no `trace`",
        ja: "契約の `QuoteResponse` に `trace` がありません",
        note_en: "`QuoteResponse` is on line 27 of ../api/shop/v1/member_shipping.proto.",
        note_ja: "`QuoteResponse` は ../api/shop/v1/member_shipping.proto の 27 行目にあります。",
    },
    Off {
        tag: "trace-row",
        contract: ("  int32 row = 1;", "  int64 row = 1;"),
        rule: ("", ""),
        line: 2,
        en: "The `trace` of the contract's `QuoteResponse` is not shaped as a rule's",
        ja: "契約の `QuoteResponse` の `trace` の形が違います",
        note_en: "`FiredRow` has `row` as `int64 row = 1;` (line 15 of ../api/shop/v1/member_shipping.proto).",
        note_ja: "`FiredRow` の `row` は `int64 row = 1;` です（../api/shop/v1/member_shipping.proto の 15 行目）。",
    },
    Off {
        tag: "service",
        contract: ("    option idempotency_level = NO_SIDE_EFFECTS;\n", ""),
        rule: ("", ""),
        line: 2,
        en: "The contract's service `MemberShippingService` is not shaped as a rule's service",
        ja: "契約のサービス `MemberShippingService` は、規則のサービスの形ではありません",
        note_en: "`Quote` does not say `option idempotency_level = NO_SIDE_EFFECTS;`.",
        note_ja: "`Quote` に `option idempotency_level = NO_SIDE_EFFECTS;` がありません。",
    },
    Off {
        tag: "json-name",
        contract: ("optional int64 subtotal = 5;", "optional int64 subtotal = 5 [json_name = \"sub\"];"),
        rule: ("", ""),
        line: 12,
        en: "The contract's `QuoteRequest` writes `subtotal` in a way a rule's service does not",
        ja: "契約の `QuoteRequest` の `subtotal` は、規則のサービスが読まない書き方です",
        note_en: "It sets `json_name = \"sub\"`.",
        note_ja: "`json_name = \"sub\"` を付けています。",
    },
    Off {
        tag: "proto2",
        contract: ("syntax = \"proto3\";", "syntax = \"proto2\";"),
        rule: ("", ""),
        line: 2,
        en: "The contract `../api/shop/v1/member_shipping.proto` is not proto3",
        ja: "契約 `../api/shop/v1/member_shipping.proto` は proto3 ではありません",
        note_en: "`../api/shop/v1/member_shipping.proto` says `proto2`.",
        note_ja: "`../api/shop/v1/member_shipping.proto` には `proto2` と書かれています。",
    },
];

#[test]
fn 契約と合わなければ_e076() {
    for off in OFF {
        let contract = if off.contract.0.is_empty() { CONTRACT.to_string() } else { CONTRACT.replacen(off.contract.0, off.contract.1, 1) };
        let rule = if off.rule.0.is_empty() { RULE.to_string() } else { RULE.replacen(off.rule.0, off.rule.1, 1) };
        assert!(contract != CONTRACT || rule != RULE, "{}: nothing changed", off.tag);
        let (_t, d) = pair(off.tag, &contract, &rule);
        let (code, out) = rulec(&d, "en", &["check", RULE_AT]);
        assert_eq!(code, 1, "{}: {out}", off.tag);
        said_at(&out, "en", "E076", off.en, off.line);
        assert!(out.contains(off.note_en), "{}: {:?} が無い:\n{out}", off.tag, off.note_en);
        assert_eq!(out.matches("[E076]").count(), 1, "{}: ずれ一つに一つの診断:\n{out}", off.tag);
        let (code, out) = rulec(&d, "ja", &["check", RULE_AT]);
        assert_eq!(code, 1, "{}: {out}", off.tag);
        said_at(&out, "ja", "E076", off.ja, off.line);
        assert!(out.contains(off.note_ja), "{}: {:?} が無い:\n{out}", off.tag, off.note_ja);
    }
}

/// A rule whose own types do not hold is not held to the contract: what its values are on the wire
/// is not known yet, and E076 would only repeat the first finding.
#[test]
fn 型の通らない規則は契約と比べない() {
    let broken = RULE.replace("define lifted : bool = campaign > member", "define lifted : bool = campaign + member");
    let (_t, d) = pair("types", &CONTRACT.replacen("  optional int64 subtotal = 5;\n", "", 1), &broken);
    let (code, out) = rulec(&d, "en", &["check", RULE_AT]);
    assert_eq!(code, 1, "{out}");
    assert!(!out.contains("E076"), "{out}");
}

#[test]
fn enums_のファイルと寄与のファイルには書けない() {
    let (_t, d) = tree("enums", &[(CONTRACT_AT, CONTRACT), ("rules/tiers.rule", "enums tiers v1\nimplements proto \"../api/shop/v1/member_shipping.proto\" MemberShippingService\n\nenum tier = basic | gold\n")]);
    let (code, out) = rulec(&d, "en", &["check", "rules/tiers.rule"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("error[E066]: An `enums` file cannot hold `implements`\n  --> rules/tiers.rule:2"), "{out}");

    // The shape of the function is the rule's, so the line is said before anything is added.
    let campaign = "contribution campaign(campaign) v1\nto \"member_tier_shipping.rule\" signature:0000000000000000\n\
                    implements proto \"../api/shop/v1/member_shipping.proto\" MemberShippingService\n\ninputs\n  north : bool\n";
    let (_t, d) = tree("contribution", &[(CONTRACT_AT, CONTRACT), (RULE_AT, RULE), ("rules/campaign.rule", campaign)]);
    let (code, out) = rulec(&d, "en", &["check", "rules/campaign.rule"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("error[E067]: A contribution file cannot hold `implements`\n  --> rules/campaign.rule:3"), "{out}");
}

/// The word is not reserved: a line that declares a name `implements` is a declaration, as before.
#[test]
fn implements_という名前の入力は通る() {
    let rule = "rule t(t) v1\n\ninputs\n  implements : bool\n\noutputs\n  r : bool\n\ntable j\n| implements | -> r |\n| true | false |\n| false | true |\n";
    let (_t, d) = tree("word", &[("t.rule", rule)]);
    let (code, out) = rulec(&d, "en", &["check", "t.rule"]);
    assert_eq!(code, 0, "{out}");
}

#[test]
fn fmt_は行をそのまま残す() {
    let messy = RULE.replace("| silver    | >=50USD  |", "| silver | >=50USD |");
    let (_t, d) = pair("fmt", CONTRACT, &messy);
    let (code, out) = rulec(&d, "en", &["fmt", RULE_AT]);
    assert_eq!(code, 0, "{out}");
    assert_eq!(std::fs::read_to_string(d.join(RULE_AT)).unwrap(), RULE, "fmt の結果が元の規則と違う");
}

/// The page for people says that the inputs and outputs are a contract's, and that `check` holds
/// them to it.
#[test]
fn 承認者のページが契約を言う() {
    let (_t, d) = pair("doc", CONTRACT, RULE);
    let (code, out) = rulec(&d, "en", &["doc", RULE_AT]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("The inputs and outputs are those of the service `MemberShippingService` in `../api/shop/v1/member_shipping.proto`, and a Connect call uses that contract's names and numbers. `rulec check` holds the two together (E076 when they differ)."), "{out}");
    let (code, out) = rulec(&d, "ja", &["doc", RULE_AT]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("入力と出力は `../api/shop/v1/member_shipping.proto` のサービス `MemberShippingService` に合わせてあり、Connect で呼ぶときの名前と番号はその契約のものです。"), "{out}");
    // a rule that does not write the line says nothing of it
    let (_t, d) = tree("doc-own", &[(RULE_AT, RULE.replace(&format!("{IMPLEMENTS_LINE}\n"), "").replace("import proto \"../api/shop/v1/member_shipping.proto\" Tier -> tier\n", "").as_str())]);
    let (code, out) = rulec(&d, "en", &["doc", RULE_AT]);
    assert_eq!(code, 0, "{out}");
    assert!(!out.contains("a Connect call uses that contract's"), "{out}");
}

/// What the ports read: the contract is a file the rule names, so a module that holds it is used,
/// and the Connect path dandori, sekisho and sakai call is the contract's.
#[test]
fn 口の参照とパスは契約のもの() {
    let (_t, d) = pair("ports", CONTRACT, RULE);
    let e = rulec::ports::Engine::new();
    let refs = e.references(&d, RULE_AT).unwrap();
    let r = refs.iter().find(|r| r.how == "implements").unwrap_or_else(|| panic!("{refs:?}"));
    assert_eq!(r.line, 2);
    assert_eq!(r.target.text(), "proto \"api/shop/v1/member_shipping.proto\" service MemberShippingService");
    let facts = e.facts(&d.join(RULE_AT)).unwrap_or_else(|s| panic!("{s:?}"));
    let c = facts.connect.expect("connect が無い");
    assert_eq!(c.path, "/shop.v1.MemberShippingService/Quote");
    let names: Vec<&str> = c.request.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, ["member", "campaign", "subtotal"]);
}
