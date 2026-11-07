//! `rulec run`: one input asked of the reference evaluator.
//!
//! The command is not a second interpreter. It reads the input through the door `vectors` states
//! the refusals of, and answers with the call the expected values of the vectors come from. So
//! the test is that it says what the vectors say: for every row of `rulec vectors`, `rulec run
//! --input -` on the row's `in` answers the row's `out`; for every refused input `gen` writes,
//! it refuses for the kind and the sentence the file gives. The materials have a machine, a
//! sequence read by a walk and by a count, `apply`, a `constraint`, a fold that contradicts
//! itself, and a plain table, English first with the Japanese versions of three of them.

use rulec::json::Json;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

const RULES: [&str; 11] = [
    "tests/corpus/member_tier_shipping.rule",
    "tests/corpus/order_lifecycle.rule",
    "tests/corpus/one_time_perk.rule",
    "tests/corpus/cart_shipping_fee.rule",
    "tests/corpus/earliest_due.rule",
    "tests/corpus/part_time_retirement_pay.rule",
    "tests/corpus/express_delivery_quote.rule",
    "tests/corpus/nationwide_freight.rule",
    "tests/corpus/注文の状態.rule",
    "tests/corpus/買物かごの送料.rule",
    "tests/corpus/全国運賃.rule",
];

fn rulec(args: &[&str], stdin: Option<&str>) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_rulec"));
    c.env("RULEC_LANG", "en").current_dir(root()).args(args).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = c.spawn().expect("cannot start rulec");
    let mut si = child.stdin.take().unwrap();
    if let Some(s) = stdin {
        si.write_all(s.as_bytes()).unwrap();
    }
    drop(si);
    let o = child.wait_with_output().unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

fn parse(s: &str) -> Json {
    rulec::json::parse(s.trim()).unwrap_or_else(|e| panic!("not JSON: {e}\n{s}"))
}

#[test]
fn run_answers_every_row_of_the_vectors() {
    let mut rows = 0;
    for rule in RULES {
        let (code, out, e) = rulec(&["vectors", rule], None);
        assert_eq!(code, 0, "{rule}: {e}");
        for line in out.lines() {
            let v = parse(line);
            let input = v.get("in").unwrap().compact();
            let (code, said, e) = rulec(&["run", rule, "--input", "-", "--format", "json"], Some(&input));
            assert_eq!(code, 0, "{rule}: {input}: {e}{said}");
            let got = parse(&said);
            assert_eq!(got.get("answer"), v.get("out"), "{rule}: {input}");
            assert_eq!(got.get("input"), v.get("in"), "{rule}: {input}");
            rows += 1;
        }
    }
    println!("compared: {rows} rows of the vectors of {} rules", RULES.len());
    assert!(rows > 100);
}

#[test]
fn run_refuses_what_the_door_refuses_for_the_same_reason() {
    let t = ritsu_testkit::TempDir::new("run-refused");
    let out = t.path().join("out");
    let o = out.to_string_lossy().into_owned();
    let mut args = vec!["gen"];
    args.extend(RULES);
    args.extend(["--out", o.as_str()]);
    args.extend(rulec::targets::UP_TO_0_40);
    let (code, said, e) = rulec(&args, None);
    assert_eq!(code, 0, "{said}{e}");
    let mut n = 0;
    let mut kinds = std::collections::BTreeSet::new();
    for (alias, rule) in aliases() {
        let file = out.join("vectors").join(format!("{alias}.refused.jsonl"));
        let Ok(body) = std::fs::read_to_string(&file) else { continue };
        for line in body.lines() {
            let v = parse(line);
            let input = v.get("in").unwrap().compact();
            let (code, said, e) = rulec(&["run", rule, "--input", "-", "--format", "json"], Some(&input));
            assert_eq!(code, 0, "{rule}: {input}: {e}");
            let got = parse(&said);
            assert_eq!(got.get("refused"), v.get("refused"), "{rule}: {input}");
            assert_eq!(got.get("error"), v.get("error"), "{rule}: {input}");
            assert!(got.get("answer").is_none());
            kinds.insert(v.get("refused").and_then(|k| k.as_str()).unwrap().to_string());
            n += 1;
        }
    }
    println!("compared: {n} refused inputs, kinds {kinds:?}");
    assert!(n > 20);
    assert!(kinds.contains("input") && kinds.contains("contradiction"), "{kinds:?}");
}

/// The alias of each rule (what the vectors file is named by), with its path.
fn aliases() -> Vec<(String, &'static str)> {
    RULES
        .iter()
        .map(|r| {
            let (code, out, e) = rulec(&["api", r], None);
            assert_eq!(code, 0, "{r}: {e}");
            let alias = parse(&out).get("rule").and_then(|x| x.as_str()).map(String::from);
            (alias.unwrap_or_else(|| Path::new(r).file_stem().unwrap().to_string_lossy().into_owned()), *r)
        })
        .collect()
}

#[test]
fn run_takes_name_value_and_says_what_it_cannot() {
    let rule = "tests/corpus/express_delivery_quote.rule";
    let (code, said, _) = rulec(&["run", rule, "weight=500", "express=false", "declared=0", "cover=0"], None);
    assert_eq!(code, 0, "{said}");
    assert!(said.contains(" = "), "{said}");
    // the same answer as the JSON object gives
    let (_, a, _) = rulec(&["run", rule, "--input", "-", "--format", "json"], Some(r#"{"weight":500,"express":false,"declared":0,"cover":0}"#));
    let (_, b, _) = rulec(&["run", rule, "weight=500", "express=false", "declared=0", "cover=0", "--format", "json"], None);
    assert_eq!(parse(&a), parse(&b));
    // a name that is no input, a word that is not `name=value`, an object that is not one
    assert_eq!(rulec(&["run", rule, "nope=1"], None).0, 2);
    assert_eq!(rulec(&["run", rule, "weight"], None).0, 2);
    assert_eq!(rulec(&["run", rule, "--input", "-"], Some("[1]")).0, 2);
    // a refusal of the door is an answer: 0, and it says why
    let (code, said, _) = rulec(&["run", rule, "weight=-1", "express=false", "declared=0", "cover=0"], None);
    assert_eq!(code, 0);
    assert!(said.starts_with("refused (input): weight is out of range"), "{said}");
    // a file that cannot be read
    let (code, _, _) = rulec(&["run", "tests/corpus/no_such_rule.rule", "a=1"], None);
    assert_eq!(code, 2);
}

#[test]
fn run_with_trace_says_the_rows_taken() {
    let rule = "tests/corpus/member_tier_shipping.rule";
    let (code, out, e) = rulec(&["vectors", rule], None);
    assert_eq!(code, 0, "{e}");
    let v = parse(out.lines().next().unwrap());
    let input = v.get("in").unwrap().compact();
    let (_, said, _) = rulec(&["run", rule, "--input", "-", "--format", "json", "--trace"], Some(&input));
    let got = parse(&said);
    let rows = got.get("rows").and_then(|r| r.as_arr()).expect("rows");
    assert!(!rows.is_empty());
    assert_eq!(got.get("trace").map(|t| t.compact()), v.get("trace").map(|t| t.compact()));
    let (_, plain, _) = rulec(&["run", rule, "--input", "-", "--format", "json"], Some(&input));
    assert!(parse(&plain).get("rows").is_none());
}
