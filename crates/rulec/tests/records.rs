//! The records `fixtures lint`, `replay` and `diff` read (docs/formats.md, DESIGN §15.306).
//!
//! The generated code writes one record per call, and that line is the fixtures format: what the
//! record function writes, the three commands read. A rule that walks a sequence was the gap —
//! its records hold the sequence under `in`, and the reader knew the inputs alone, so every
//! record of such a rule was left out as a field the rule does not know.

use ritsu_testkit::TempDir;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rulec_in(cwd: &Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").current_dir(cwd).args(args).output().expect("rulec starts");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// A cart: a sequence with a `unique` field, a field of each kind, a `count` that caps its length
/// and a `sum` with a top.
const CART: &str = "\
rule cart v1

enum speed = normal | express

inputs
  ship : speed

elements lines
  sku      : string  unique
  quantity : number  range >=1 <=100
  amount   : money[JPY]  range >=0JPY <=100000JPY
  due      : date  range >=2026-01-01 <=2026-12-31
  gift     : bool

count wrapped over lines where gift  range >=0 <=3
sum total over lines of amount  range >=0JPY <=150000JPY

outputs
  fee : money[JPY]  round down(1JPY)

table fee_of
policy first
| ship    | wrapped | total      | -> fee |
| express | -       | -          | 800JPY |
| normal  | >=1     | -          | 300JPY |
| normal  | -       | >=10000JPY | 0JPY   |
| normal  | -       | -          | 500JPY |
";

fn line(sku: &str, quantity: &str, amount: &str, due: &str) -> String {
    format!("{{\"sku\":\"{sku}\",\"quantity\":{quantity},\"amount\":{amount},\"due\":\"{due}\",\"gift\":false}}")
}

fn record(lines: &str, fee: i64) -> String {
    format!("{{\"in\":{{\"lines\":{lines},\"ship\":\"normal\"}},\"observed\":{{\"fee\":{fee}}}}}")
}

fn cart(tag: &str, records: &[String]) -> (TempDir, PathBuf) {
    let tmp = TempDir::new(&format!("records-{tag}"));
    let dir = tmp.path().to_path_buf();
    std::fs::write(dir.join("cart.rule"), CART).unwrap();
    std::fs::write(dir.join("fx.jsonl"), records.iter().map(|r| format!("{r}\n")).collect::<String>()).unwrap();
    (tmp, dir)
}

/// Every rule of the corpus reads the records its own generated code writes: no problem of
/// form, every record compared, every one matched, no row moved. The expected file `gen` writes
/// beside the vectors is those records, and the 19 rules that walk a sequence read none of
/// theirs.
#[test]
fn every_rule_of_the_corpus_reads_the_records_its_generated_code_writes() {
    let mut names: Vec<String> = std::fs::read_dir(root().join("tests/corpus"))
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .filter(|n| n.ends_with(".rule"))
        .collect();
    names.sort();
    assert!(names.len() >= 150, "the corpus is here: {}", names.len());
    let (mut walked, mut compared) = (0, 0);
    for name in &names {
        let path = format!("tests/corpus/{name}");
        let src = std::fs::read_to_string(root().join(&path)).unwrap();
        rulec::link::with_set(&[path.clone()], || {
            let (f, c) = rulec::prepare(&src, &path).unwrap_or_else(|_| panic!("{name} passes check"));
            let vs = rulec::vectors::generate(&f, &c);
            let records: String = vs.iter().map(|v| format!("{}\n", rulec::vectors::expected_json(&f, &c, v))).collect();
            let m = rulec::fixtures::Manifest::default();
            let l = rulec::fixtures::load(&records, &f, &c, &m);
            let said: Vec<&str> = l.problems.iter().map(|p| p.what.as_str()).collect();
            assert!(l.problems.is_empty() && l.dropped == 0, "{name}: {said:?}");
            assert_eq!(l.records.len(), vs.len(), "{name}");
            let rep = rulec::replay::replay(&f, &c, &l, &m, "expected");
            assert_eq!((rep.total, rep.agreed), (vs.len(), vs.len()), "{name}");
            assert!(rep.mismatches.is_empty() && rep.moved.is_empty(), "{name}");
            walked += usize::from(!f.elements.is_empty());
            compared += vs.len();
        });
    }
    assert!(walked >= 19, "rules that walk a sequence are among them: {walked}");
    assert!(compared > 10_000, "{compared}");
}

/// A record's sequence is read as the wire has it, and compared: an answer that differs is a
/// mismatch whose witness holds the sequence as the array it is, not as one string.
#[test]
fn a_sequence_is_read_and_its_witness_is_the_array() {
    let two = format!("[{},{}]", line("a", "1", "1000", "2026-03-01"), line("b", "1", "9000.0", "2026-03-01"));
    let one = format!("[{}]", line("a", "1", "1000", "2026-03-01"));
    let (_tmp, dir) = cart("read", &[record(&two, 0), record(&one, 400), record("[]", 500)]);
    let (code, out, err) = rulec_in(&dir, &["fixtures", "lint", "fx.jsonl", "cart.rule"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("3 records") && out.contains("No format problems"), "{out}");
    let (code, out, err) = rulec_in(&dir, &["replay", "cart.rule", "--fixtures", "fx.jsonl"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.contains("Compared 3 / matched 2"), "{out}");
    assert!(out.contains("rule fee=500 / observed fee=400"), "{out}");
    let (_, json, _) = rulec_in(&dir, &["replay", "cart.rule", "--fixtures", "fx.jsonl", "--format", "json"]);
    assert!(
        json.contains(r#""in":{"lines":[{"amount":1000,"due":"2026-03-01","gift":false,"quantity":1,"sku":"a"}],"ship":"normal"}"#),
        "the witness holds the array: {json}"
    );
}

/// A sequence is held to what the generated code's door takes, each refusal by its own name:
/// not an array, an element that is no object, a field missing, unknown, of another type or
/// outside its range, more elements than the `count` allows, a `unique` field twice, a `sum`
/// past its top. A record with no sequence at all is missing a field.
#[test]
fn a_sequence_is_held_to_the_door() {
    let ok = |sku: &str| line(sku, "1", "1000", "2026-03-01");
    let recs = vec![
        record("\"none\"", 500),
        record("[1]", 500),
        record("[{\"sku\":\"a\",\"quantity\":1,\"amount\":1,\"due\":\"2026-03-01\"}]", 500),
        record(&format!("[{}]", ok("a").replace("\"gift\"", "\"colour\":\"red\",\"gift\"")), 500),
        record(&format!("[{}]", line("a", "\"1\"", "1000", "2026-03-01")), 500),
        record(&format!("[{}]", line("a", "0", "1000", "2026-03-01")), 500),
        record(&format!("[{}]", line("a", "1", "1000", "2027-01-01")), 500),
        record(&format!("[{},{},{},{}]", ok("a"), ok("b"), ok("c"), ok("d")), 500),
        record(&format!("[{},{}]", ok("a"), ok("a")), 500),
        record(&format!("[{},{}]", line("a", "1", "100000", "2026-03-01"), line("b", "1", "100000", "2026-03-01")), 0),
        "{\"in\":{\"ship\":\"normal\"},\"observed\":{\"fee\":500}}".to_string(),
    ];
    let (_tmp, dir) = cart("door", &recs);
    let (code, out, err) = rulec_in(&dir, &["fixtures", "lint", "fx.jsonl", "cart.rule"]);
    assert_eq!(code, 1, "{out}{err}");
    for want in [
        "`in.lines`: expected a sequence (an array), found string",
        "`in.lines`: an element is integer, not an object",
        "`in.lines[].gift` is missing",
        "an element of `in.lines` has fields the rule does not know: colour",
        "`in.lines[].quantity`: expected integer in the canonical unit, found string",
        "`in.lines[].quantity`: 0 is outside the declared range 1..100",
        "`in.lines[].due`: 2027-01-01 is outside the declared range 2026-01-01..2026-12-31",
        "`in.lines`: too many elements (at most 3)",
        "`in.lines`: two elements have the same sku",
        "`in.lines`: the sum total is past the top of its declared range",
        "Records excluded because a field was missing: 1",
        "10 problems",
    ] {
        assert!(out.contains(want), "{want}\n{out}");
    }
    let (_, json, _) = rulec_in(&dir, &["fixtures", "lint", "fx.jsonl", "cart.rule", "--format", "json"]);
    assert!(json.contains(r#""kind":"unknown_field","field":"lines[].colour""#), "{json}");
    assert!(json.contains(r#""kind":"bad_input","field":"lines[].quantity""#), "{json}");
    assert!(json.contains(r#""kind":"bad_input","field":"lines","#), "{json}");
    // Nothing is left to compare, and `replay` says so.
    let (code, out, _) = rulec_in(&dir, &["replay", "cart.rule", "--fixtures", "fx.jsonl"]);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("Not one record was compared"), "{out}");
}

/// `diff` hands each record's sequence to the new version, element by element: an answer that
/// moves is compared, and a sequence or a field of an element the new version does not take is
/// refused by its reason — `length` for more elements than its `count` allows.
#[test]
fn diff_hands_a_sequence_to_the_new_version() {
    let ok = |sku: &str, amount: &str| line(sku, "1", amount, "2026-03-01");
    let recs = vec![
        record(&format!("[{},{}]", ok("a", "1000"), ok("b", "8500")), 0),
        record(&format!("[{},{},{}]", ok("a", "1"), ok("b", "1"), ok("c", "1")), 0),
        record(&format!("[{}]", ok("a", "60000")), 0),
        record(&format!("[{}]", ok("a", "1000")), 0),
    ];
    let (_tmp, dir) = cart("diff", &recs);
    let v2 = CART
        .replace("rule cart v1", "rule cart v2")
        .replace("range >=0 <=3", "range >=0 <=2")
        .replace(">=10000JPY | 0JPY", ">=9000JPY  | 0JPY")
        .replace("<=100000JPY", "<=50000JPY");
    std::fs::write(dir.join("cart2.rule"), v2).unwrap();
    let (code, out, err) = rulec_in(&dir, &["diff", "cart.rule", "cart2.rule", "--fixtures", "fx.jsonl"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.contains("Compared 4 / matched 1"), "{out}");
    assert!(out.contains("table fee_of row 4→row 3"), "the answer that moved: {out}");
    assert!(out.contains("lines has more elements than the new version takes (at most 2)"), "{out}");
    assert!(out.contains("amount is outside the new version's range"), "{out}");
    let (_, json, _) = rulec_in(&dir, &["diff", "cart.rule", "cart2.rule", "--fixtures", "fx.jsonl", "--format", "json"]);
    assert!(json.contains(r#""kind":"length","field":"lines""#), "{json}");
    assert!(json.contains(r#""kind":"input_range","field":"amount""#), "{json}");
}

/// A byte order mark at the head of the file is no part of the first record, and a tag that is a
/// number names its record as the same digits in quotes do.
#[test]
fn a_byte_order_mark_and_a_number_for_a_tag_are_read() {
    let one = format!("[{}]", line("a", "1", "1000", "2026-03-01"));
    let rec = record(&one, 400).replacen("{\"in\"", "{\"tag\":1234567,\"in\"", 1);
    let (_tmp, dir) = cart("bom", &[]);
    std::fs::write(dir.join("fx.jsonl"), format!("\u{feff}{rec}\n")).unwrap();
    let (code, out, err) = rulec_in(&dir, &["replay", "cart.rule", "--fixtures", "fx.jsonl", "--format", "json"]);
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.contains("\"compared\":1") && out.contains("\"excluded\":{}"), "{out}");
    assert!(out.contains(r#""records":[{"line":1,"tag":"1234567"}]"#), "{out}");
}

/// A manifest that declares no `fills` is refused: a key spelled `fill` used to read as no
/// default at all, and only the counts said so.
#[test]
fn a_manifest_without_fills_is_refused() {
    let (_tmp, dir) = cart("manifest", &[record("[]", 500)]);
    for (text, want) in [("{\"fill\":{\"ship\":\"normal\"}}", "has no `fills`"), ("{\"fills\":[\"ship\"]}", "is array, not an object")] {
        std::fs::write(dir.join("m.json"), text).unwrap();
        let (code, out, err) = rulec_in(&dir, &["replay", "cart.rule", "--fixtures", "fx.jsonl", "--manifest", "m.json"]);
        assert_eq!(code, 2, "{out}{err}");
        assert!(err.contains(want), "{err}");
    }
    std::fs::write(dir.join("m.json"), "\u{feff}{\"rule\":\"cart\",\"fills\":{\"ship\":\"normal\"}}").unwrap();
    let (code, out, err) = rulec_in(&dir, &["replay", "cart.rule", "--fixtures", "fx.jsonl", "--manifest", "m.json"]);
    assert_eq!(code, 0, "{out}{err}");
}

/// The records are one rule's: a second rule on the line, or a directory that holds several, is
/// refused where it was dropped without a word.
#[test]
fn replay_takes_one_rule() {
    let (_tmp, dir) = cart("one", &[record("[]", 500)]);
    std::fs::write(dir.join("other.rule"), CART.replace("rule cart v1", "rule other v1")).unwrap();
    let (code, out, err) = rulec_in(&dir, &["replay", "cart.rule", "other.rule", "--fixtures", "fx.jsonl"]);
    assert_eq!(code, 2, "{out}{err}");
    assert!(err.contains("takes one rule (2 were given: cart.rule, other.rule)"), "{err}");
    let (code, _, err) = rulec_in(&dir, &["replay", ".", "--fixtures", "fx.jsonl"]);
    assert_eq!(code, 2, "{err}");
    let (code, _, err) = rulec_in(&dir, &["fixtures", "lint", "fx.jsonl", "cart.rule", "other.rule"]);
    assert_eq!(code, 2, "{err}");
}
