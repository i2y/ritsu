//! Walks over more than one sequence, and more than one walk (§15.219 to §15.221).
//!
//! A rule may declare any number of `elements`, and summarise each with any number of `count`,
//! `sum`, `max`, `min` and `fold` lines. Nothing says which walk comes first: an item of one
//! element that reads a summary goes on the walk after the one that finds it, and a value of the
//! call that reads one comes after that walk. What is held here is that the order is the one the
//! dependencies give, that a `max` and a `min` take the range their column and `empty` give them,
//! that the evaluator and every generated language answer alike, that the vectors reach what a
//! second walk asks for, that the certificate states the ranges it works out and the re-checker
//! holds it to them, and that the page lets a reader edit each sequence.

use ritsu_testkit::tmp::tmpdir_in;
use ritsu_testkit::{Need, TempDir, ready};
use rulec::ast::Item;
use rulec::eval::Val;
use rulec::num::Rat;
use rulec::types::Stage;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn dir(tag: &str) -> (TempDir, PathBuf) {
    let t = TempDir::new(&format!("walks-{tag}"));
    let d = t.path().to_path_buf();
    (t, d)
}

fn run(args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").current_dir(root()).args(args).output().expect("cannot start rulec");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// `run`, with `tmp` as rulec's TMPDIR (swiftc leaves a directory there).
fn run_tmp(tmp: &Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").env("TMPDIR", tmp).current_dir(root()).args(args).output().expect("cannot start rulec");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

fn write(d: &Path, name: &str, body: &str) -> String {
    let p = d.join(name);
    std::fs::write(&p, body).unwrap();
    p.to_str().unwrap().to_string()
}

fn have(cmd: &str) -> bool {
    Command::new("sh").args(["-c", &format!("command -v {cmd} >/dev/null 2>&1")]).status().map(|s| s.success()).unwrap_or(false)
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("cannot read {rel}: {e}"))
}

fn codes(src: &str) -> Vec<(String, String)> {
    rulec::i18n::with(rulec::i18n::Lang::En, || rulec::check_source(src, "w.rule").iter().map(|d| (d.code.to_string(), d.title.clone())).collect())
}

fn num(n: i128) -> Val {
    Val::Num(Rat::int(n))
}

/// Two folds and a count on one walk, a `max` of an ordered enum, and a table that reads a fold's
/// answer once the walk is over. The first fold stops at an element marked to stop; the count and
/// the other fold see every element all the same.
const MIXED: &str = r#"rule mixed_walk v1
description "Two folds and a count on one walk, a largest value of an ordered enum, and a fold's answer read by a table after the walk"

enum grade = bronze < silver < gold
enum pick = take | skip | halt
enum mark = flagged | clear
enum tier = high | low

elements items
  level     : grade
  price     : number  range >=0 <=100
  stop_here : bool

outputs
  first_price : number  round down(1)
  top_grade   : grade
  counted     : number  round down(1)
  verdict     : tier

table pick_of
policy unique
| stop_here | level  | -> choice : pick |
| true      | -      | halt             |
| false     | gold   | take             |
| false     | silver | take             |
| false     | bronze | skip             |

table mark_of
policy unique
| price | -> flag : mark |
| >=50  | flagged        |
| <50   | clear          |

# Stops at the first element marked to stop; the count beside it sees every element
fold choice over items -> first_price
  take      -> take_first price
  skip      -> next
  halt      -> stop
  empty     -> 0
  exhausted -> held

count counted over items where flag = flagged  range >=0 <=20

fold flag over items -> any_flagged : bool
  flagged   -> stop with true
  clear     -> next
  empty     -> false
  exhausted -> false

max top_grade over items of level empty bronze

table verdict_of
policy unique
| any_flagged | top_grade | -> verdict : tier |
| true        | -         | high              |
| false       | gold      | high              |
| false       | <gold     | low               |

examples
| items | -> first_price | top_grade | counted | verdict |
| s1    | 60             | gold      | 3       | high    |
| s2    | 0              | bronze    | 0       | low     |
| s0    | 0              | bronze    | 0       | low     |

sequence s1
| level  | price | stop_here |
| bronze | 10    | false     |
| silver | 60    | false     |
| gold   | 70    | true      |
| gold   | 80    | false     |

sequence s2
| level  | price | stop_here |
| bronze | 10    | false     |

sequence s0
| level | price | stop_here |
"#;

/// The order of the walks comes from what reads what (§15.219): the total and the highest price on
/// the first walk over the lines, the payments on a walk of their own, the line set against the
/// highest price and the fold that takes the discount on a second walk over the lines, and the
/// balance once both are over. Nothing in the rule says so.
#[test]
fn 歩きの順は読むものから決まる() {
    for rel in ["tests/corpus/order_settlement.rule", "tests/corpus/注文の精算.rule"] {
        let src = read(rel);
        let (f, _) = rulec::prepare(&src, rel).unwrap_or_else(|ds| panic!("{rel}: {ds:?}"));
        let st = rulec::types::stages(&f);
        // a name by its alias: the Japanese rule's names are its own
        let mut names: Vec<&rulec::ast::Name> = f.outputs.iter().map(|o| &o.name).collect();
        for it in &f.items {
            match it {
                Item::Agg(d) => names.push(&d.name),
                Item::Define(d) => names.push(&d.name),
                Item::Derived(d) => names.push(&d.name),
                Item::Table(t) => names.extend(t.outputs.iter().map(|o| &o.name)),
                Item::Fold(_) => {}
            }
        }
        let at = |alias: &str| -> Stage {
            let n = names.iter().find(|n| n.text == alias || n.ascii.as_deref() == Some(alias)).unwrap_or_else(|| panic!("{rel}: no {alias}")).text.clone();
            let i = f.items.iter().position(|it| rulec::types::item_binds(&f, it).contains(&n)).unwrap_or_else(|| panic!("{rel}: no item binds {n}"));
            st.at(i)
        };
        assert_eq!(at("subtotal"), Stage::Each { seq: 0, walk: 1 }, "{rel}");
        assert_eq!(at("top_price"), Stage::Each { seq: 0, walk: 1 }, "{rel}");
        assert_eq!(at("paid"), Stage::Each { seq: 1, walk: 1 }, "{rel}");
        assert_eq!(at("gap"), Stage::Each { seq: 0, walk: 2 }, "{rel}");
        assert_eq!(at("pick"), Stage::Each { seq: 0, walk: 2 }, "{rel}");
        assert_eq!(at("discount"), Stage::Each { seq: 0, walk: 2 }, "{rel}");
        assert_eq!(at("balance"), Stage::Once(2), "{rel}");
        assert_eq!(st.depth, 2, "{rel}");
    }
}

/// A `stop` ends the reading of its own fold and no other: the count and the second fold on the
/// same walk see the elements after it (§15.219). And a `max` of an ordered enum answers the
/// value furthest along the order.
#[test]
fn 止まった畳み込みはほかの集計を止めない() {
    let (f, c) = rulec::prepare(MIXED, "mixed.rule").unwrap_or_else(|ds| panic!("{ds:?}"));
    let item = |level: &str, price: i128, stop: bool| {
        BTreeMap::from([("level".to_string(), Val::Enum(level.to_string())), ("price".to_string(), num(price)), ("stop_here".to_string(), Val::Bool(stop))])
    };
    let inputs = HashMap::from([(
        "items".to_string(),
        Val::Seq(vec![item("bronze", 10, false), item("silver", 20, true), item("gold", 90, false), item("gold", 55, false)]),
    )]);
    let (outs, _, _) = rulec::eval::run_all(&f, &c, inputs);
    let got: BTreeMap<String, Option<Val>> = outs.into_iter().collect();
    // the first fold stops on the silver element, which took nothing: it answers the empty-handed 0
    assert_eq!(got["first_price"], Some(num(0)), "{got:?}");
    // the count goes on past the stop to both elements of 50 and over
    assert_eq!(got["counted"], Some(num(2)), "{got:?}");
    // the max sees the gold elements after the stop
    assert_eq!(got["top_grade"], Some(Val::Enum("gold".to_string())), "{got:?}");
    assert_eq!(got["verdict"], Some(Val::Enum("high".to_string())), "{got:?}");
}

/// A `max` or a `min` takes the hull of its column's range and its `empty` value as its range
/// (§15.220). A range written on it has to hold that hull (E112, with the range that would), and an
/// enum's takes none (E069). And `empty` is required (E022).
#[test]
fn 最大と最小の範囲は列と空の値から決まる() {
    let base = "rule m v1\n\nelements xs\n  k : number  range >=0 <=10\n\noutputs\n  r : number  round down(1)\n\n";
    // the hull of 0..10 and -1
    let src = format!("{base}min r over xs of k empty -1\n");
    let (f, c) = rulec::prepare(&src, "m.rule").unwrap_or_else(|ds| panic!("{ds:?}"));
    assert_eq!(c.ranges.get("r").copied(), Some((Some(Rat::int(-1)), Some(Rat::int(10)))));
    assert!(f.aggs().any(|d| d.kind == rulec::ast::AggKind::Min));
    // a written range that leaves out the `empty` value
    let ds = codes(&format!("{base}min r over xs of k empty -1  range >=0 <=10\n"));
    assert!(ds.iter().any(|(c, t)| c == "E112" && t == "The range of a `min` does not contain the values it can actually take"), "{ds:?}");
    let ds = rulec::i18n::with(rulec::i18n::Lang::En, || rulec::check_source(&format!("{base}min r over xs of k empty -1  range >=0 <=10\n"), "m.rule"));
    let fix = ds.iter().find(|d| d.code == "E112").and_then(|d| d.fix.text.clone());
    assert_eq!(fix.as_deref(), Some("range >=-1 <=10"));
    // a written range that holds it is the universe, as a declared one is
    let (_, c) = rulec::prepare(&format!("{base}min r over xs of k empty -1  range >=-5 <=20\n"), "m.rule").unwrap();
    assert_eq!(c.ranges.get("r").copied(), Some((Some(Rat::int(-5)), Some(Rat::int(20)))));
    // no `empty`
    let ds = codes(&format!("{base}max r over xs of k\n"));
    assert!(ds.iter().any(|(c, _)| c == "E022"), "{ds:?}");
    // an enum's
    let en = "rule m v1\n\nenum g = a < b < c\n\nelements xs\n  k : g\n\noutputs\n  r : g\n\nmax r over xs of k empty a  range >=0 <=1\n";
    let ds = codes(en);
    assert!(ds.iter().any(|(c, _)| c == "E069"), "{ds:?}");
    // an enum with no order has no largest value
    let ds = codes(&en.replace("a < b < c", "a | b | c").replace("  range >=0 <=1", ""));
    assert!(ds.iter().any(|(c, _)| c == "E029" || c == "E069"), "{ds:?}");
}

/// What a fold answers, named with `->`, is a value the items below read, and its range is the
/// hull of what the fold answers with (§15.219): `balance` reads `discount`, so it is bounded, and
/// its int64 claim is made. Without a name for an answer that is no output, or with two places
/// deciding one name, it is E021.
#[test]
fn 名前の付いた畳み込みの答えは後ろの項目が読める() {
    let src = read("tests/corpus/order_settlement.rule");
    let (_, c) = rulec::prepare(&src, "order_settlement.rule").unwrap();
    // `top_off` is half of a tenth of a line's total: 0 to 2,500; `empty` is 0
    assert_eq!(c.ranges.get("discount").copied(), Some((Some(Rat::int(0)), Some(Rat::int(2500)))));
    assert_eq!(c.ranges.get("balance").copied(), Some((Some(Rat::int(-27500)), Some(Rat::int(25000)))));
    // a name that is no output, with no type
    let bad = src.replace("fold pick over lines -> discount", "fold pick over lines -> taken");
    let ds = codes(&bad);
    assert!(ds.iter().any(|(c, t)| c == "E021" && t == "`taken` is not given a type"), "{ds:?}");
    // no name: the first output, which a `sum` decides already
    let bad = src.replace("fold pick over lines -> discount", "fold pick over lines");
    let ds = codes(&bad);
    assert!(ds.iter().any(|(c, t)| c == "E021" && t == "Two places decide `subtotal`"), "{ds:?}");
}

/// A sequence nothing walks is E020, and one item reading the elements of two sequences is E031,
/// by name and in both languages (§15.219).
#[test]
fn たどらない並びと二つの並びを読む項目は止める() {
    let src = read("tests/corpus/order_settlement.rule");
    // the payments declared, and nothing that walks them: the line, the output and the examples gone
    let body = &src[..src.find("\nexamples\n").unwrap()];
    let unwalked = body.replace("sum paid over payments of amount  range >=0USD <=25_000USD\n", "").replace("  paid     : money[USD]  round down(1USD)\n", "").replace(" - paid\n", "\n");
    assert_ne!(unwalked, body);
    let ds = codes(&unwalked);
    assert!(ds.iter().any(|(c, t)| c == "E020" && t == "Nothing walks the sequence payments"), "{ds:?}");
    let both = src.replace("= quantity * price\n", "= quantity * price + amount\n");
    let ds = codes(&both);
    assert!(ds.iter().any(|(c, t)| c == "E031" && t == "One item reads the elements of two sequences"), "{ds:?}");
    let ja = read("tests/corpus/注文の精算.rule").replace("= 数量 × 単価\n", "= 数量 × 単価 + 支払額\n");
    let ds = rulec::i18n::with(rulec::i18n::Lang::Ja, || rulec::check_source(&ja, "w.rule"));
    assert!(ds.iter().any(|d| d.code == "E031" && d.title == "一つの項目が、二つの並びの要素を読んでいます"), "{:?}", ds.iter().map(|d| (d.code, d.title.clone())).collect::<Vec<_>>());
}

/// The rule with two folds and a count on one walk answers in every language as the evaluator does,
/// each fold reading the walk as far as it goes and the count to the end. The corpus's rules of two
/// sequences and second walks are run in every language by `tests/threeway.rs`.
#[test]
fn 一つの歩きの二つの畳み込みと数え上げは全言語で一致する() {
    if !ready(Need::Python, || have("python3"), "python3 is not installed") {
        return;
    }
    let (_tmp, d) = dir("mixed");
    let p = write(&d, "mixed.rule", MIXED);
    let out = d.join("out");
    let (code, said, e) = run(&["gen", &p, "--out", out.to_str().unwrap()]);
    assert_eq!(code, 0, "{said}{e}");
    let (_, said, _) = run_tmp(&tmpdir_in(&out), &["test", out.to_str().unwrap(), "--format", "json"]);
    let j = rulec::json::parse(said.lines().next().expect("no result")).unwrap();
    let rulec::json::Json::Arr(rs) = j.get("results").unwrap() else { panic!("{said}") };
    let mut ran = 0;
    for r in rs {
        if r.get("ran") != Some(&rulec::json::Json::Bool(true)) {
            continue;
        }
        ran += 1;
        assert_eq!(r.get("ok"), Some(&rulec::json::Json::Bool(true)), "{said}");
    }
    assert!(ran >= 1, "no language ran: {said}");
}

/// A table on the second walk is reached at its boundaries by sequences whose first walk lands on
/// them (§15.221): the suite of `rate_adjustment` covers every obligation, and without its sequences
/// of two adjustments — where what the first walk finds depends on both — the audit says which
/// boundary pairs of the second walk's table are left.
#[test]
fn 二回目の歩きの表の境界は一回目の答えが境界に来る並びで覆われる() {
    rulec::i18n::set(rulec::i18n::Lang::En);
    let rel = "tests/corpus/rate_adjustment.rule";
    let src = read(rel);
    let (f, c) = rulec::prepare(&src, rel).unwrap();
    let vs = rulec::vectors::generate(&f, &c);
    let a = rulec::coverage::audit(&f, &c, rel, &vs, &[]);
    assert!(a.ok(), "{rel}");
    let two = |v: &rulec::vectors::Vector| matches!(v.input.get("adjustments"), Some(Val::Seq(s)) if s.len() == 2);
    assert!(vs.iter().any(two), "{rel}: no sequence of two adjustments");
    let fewer: Vec<rulec::vectors::Vector> = vs.iter().filter(|v| !two(v)).cloned().collect();
    let b = rulec::coverage::audit(&f, &c, rel, &fewer, &[]);
    assert!(!b.ok(), "{rel}: the audit is green without the sequences that reach the second walk's boundaries");
    assert!(b.tally[rulec::coverage::BOUND].0 < b.tally[rulec::coverage::BOUND].1, "{rel}: {:?}", b.tally);
}

/// Feed one certificate to `tools/recheck.py`, with the rule it is about.
fn recheck(cert: &str, rule: &str) -> (i32, String) {
    use std::io::Write;
    let mut p = Command::new("python3")
        .current_dir(root())
        .args(["tools/recheck.py", "--rule", rule])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("cannot start python3");
    p.stdin.as_mut().unwrap().write_all(cert.as_bytes()).unwrap();
    let o = p.wait_with_output().unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr))
}

/// The ranges `check` works out rather than reads (§15.220): a `max`'s and a `min`'s from the
/// column's and the `empty` value, and the answer of a fold the items below read from the values it
/// answers with. The certificate lists both, the re-checker holds the ranges to them, and a range
/// cut short, an `empty` value moved, or an answer left out is refused.
#[test]
fn 証明書は最大と最小と畳み込みの答えの範囲を述べ_再検査が確かめる() {
    if !ready(Need::Python, || have("python3"), "python3 is not installed") {
        return;
    }
    let rule = "tests/corpus/order_settlement.rule";
    let (c, cert, e) = run(&["certificate", rule]);
    assert_eq!(c, 0, "{e}");
    let j = rulec::json::parse(&cert).unwrap();
    let ex = j.get("extremes").and_then(|x| x.as_arr()).expect("no `extremes`");
    assert_eq!(ex.len(), 1);
    assert_eq!(ex[0].get("name").and_then(|x| x.as_str()), Some("top_price"));
    assert_eq!(ex[0].get("kind").and_then(|x| x.as_str()), Some("max"));
    assert_eq!(ex[0].get("empty").and_then(|x| x.as_str()), Some("0"));
    let fa = j.get("fold_answers").and_then(|x| x.as_arr()).expect("no `fold_answers`");
    assert_eq!(fa[0].get("name").and_then(|x| x.as_str()), Some("discount"));
    let (code, said) = recheck(&cert, rule);
    assert_eq!(code, 0, "{said}");
    assert!(said.contains("extremes: 1 `max` and `min` answers keep to their column's range and their `empty` value"), "{said}");
    assert!(said.contains("fold answers: 1 ranges hold every value their fold answers with"), "{said}");
    for (what, forged, says) in [
        ("the max's range cut short", cert.replace(r#""top_price":["0","500"]"#, r#""top_price":["0","400"]"#), "FAILED extremes"),
        ("the empty value moved", cert.replace(r#""of":"price","empty":"0""#, r#""of":"price","empty":"-5""#), "FAILED extremes"),
        ("the fold's range cut short", cert.replace(r#""discount":["0","2500"]"#, r#""discount":["0","2000"]"#), "FAILED fold answers"),
        ("an answer left out", cert.replace(r#"{"name":"top_off"}"#, r#"{"num":"0USD","value":"0","type":"money[USD]"}"#), ""),
    ] {
        assert_ne!(forged, cert, "{what}: the certificate's shape changed, and nothing was forged");
        let (code, said) = recheck(&forged, rule);
        if says.is_empty() {
            // the answer the fold takes left out narrows nothing the range is held to: still true
            assert_eq!(code, 0, "{what}: {said}");
            continue;
        }
        assert_eq!(code, 1, "{what}: a forged certificate passed\n{said}");
        assert!(said.contains(says), "{what}: {said}");
    }
    // a rule with neither writes neither, so no other certificate changes
    let (_, cert, _) = run(&["certificate", "tests/corpus/cart_shipping_fee.rule"]);
    assert!(!cert.contains("\"extremes\"") && !cert.contains("\"fold_answers\""), "{cert}");
}

/// The Lean checker holds the two ranges to what they are worked out from too (§15.220, §15.233):
/// `extreme_mem_stated` and `foldAnswer_mem_stated` are what a pass settles, so nothing about them is
/// left stated, and the forgeries `tools/recheck.py` refuses above are refused there as well.
#[test]
fn leanの検査器も二つの範囲を確かめる() {
    let bin = root().join("../../proofs/.lake/build/bin/rulec-recheck");
    if !ready(Need::Lean, || bin.exists(), "ritsu's proofs/ is not built (lake build there)") {
        return;
    }
    use std::io::Write;
    let lean = |cert: &str, rule: &str| -> (i32, String) {
        let mut p = Command::new(&bin).current_dir(root()).args(["--rule", rule]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
        p.stdin.as_mut().unwrap().write_all(cert.as_bytes()).unwrap();
        let o = p.wait_with_output().unwrap();
        (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned())
    };
    for (rule, want) in [
        ("tests/corpus/order_settlement.rule", vec!["extremes: 1 `max` and `min` answers keep to their column's range and their `empty` value", "fold answers: 1 ranges hold every value their fold answers with"]),
        ("tests/corpus/rate_adjustment.rule", vec!["extremes: 2 `max` and `min` answers keep to their column's range and their `empty` value"]),
        ("tests/corpus/earliest_due.rule", vec!["extremes: 1 `max` and `min` answers keep to their column's range and their `empty` value"]),
        ("tests/stages/best_offer.rule", vec!["fold answers: 1 ranges hold every value their fold answers with"]),
    ] {
        let (_, cert, _) = run(&["certificate", rule]);
        let (code, said) = lean(&cert, rule);
        assert_eq!(code, 0, "{rule}: {said}");
        for w in want {
            assert!(said.contains(w), "{rule}: {said}");
        }
        assert!(said.contains("OK: every claim this program states was proved"), "{rule}: {said}");
    }
    let rule = "tests/corpus/order_settlement.rule";
    let (_, cert, _) = run(&["certificate", rule]);
    for (what, forged, says) in [
        ("the max's range cut short", cert.replace(r#""top_price":["0","500"]"#, r#""top_price":["0","400"]"#), "top_price: a `max` of price answers from 0 to 500 with its `empty` value, and the certificate states the range [0, 400]"),
        ("the empty value moved", cert.replace(r#""of":"price","empty":"0""#, r#""of":"price","empty":"-5""#), "top_price: a `max` of price answers from -5 to 500"),
        ("the fold's range cut short", cert.replace(r#""discount":["0","2500"]"#, r#""discount":["0","2000"]"#), "discount: the fold answers with values from 0 to 2500, and the certificate states the range [0, 2000]"),
        ("an answer the fold cannot read", cert.replace(r#"{"name":"top_off"}"#, r#"{"name":"held"}"#), "discount: one of the values the fold answers with has no interval this program can work out"),
        ("a `max` no walk leaves behind", cert.replace(r#""walks":["subtotal","top_price","paid","discount"]"#, r#""walks":["subtotal","paid","discount"]"#), "top_price: a `max` is a value a walk leaves behind, and `walks` does not name it"),
    ] {
        assert_ne!(forged, cert, "{what}: the certificate's shape changed, and nothing was forged");
        let (code, said) = lean(&forged, rule);
        assert_eq!(code, 1, "{what}: a forged certificate passed\n{said}");
        assert!(said.contains(says), "{what}: {said}");
    }
}

/// A DOM small enough to run the panel the page carries (the one `tests/fold.rs` drives).
const DOM: &str = r##"
class El {
  constructor(tag) {
    this.tagName = tag.toUpperCase();
    this.children = [];
    this.listeners = {};
    this.classList = { add() {}, remove() {}, contains: () => false };
    this.style = {};
    this.value = "";
    this.checked = false;
    this._text = "";
    this.name = "";
  }
  get textContent() { return this._text; }
  set textContent(v) { this._text = String(v); }
  appendChild(c) { this.children.push(c); c.parentNode = this; return c; }
  remove() { const p = this.parentNode; if (p) p.children = p.children.filter((x) => x !== this); }
  addEventListener(k, f) { (this.listeners[k] ||= []).push(f); }
  click() { for (const f of this.listeners.click || []) f(); }
  querySelectorAll() { return []; }
  get elements() {
    const o = {};
    const walk = (n) => { if (n.name) o[n.name] = n; n.children.forEach(walk); };
    this.children.forEach(walk);
    return o;
  }
}
const ids = {};
for (const id of ["try-form", "try-rows", "try-buttons", "try-result", "try-record"]) {
  ids["#" + id] = new El(id === "try-form" ? "form" : "div");
}
globalThis.document = { createElement: (t) => new El(t), querySelector: (s) => ids[s] ?? null, querySelectorAll: () => [] };
globalThis.CSS = { escape: (s) => s };
globalThis.history = { replaceState() {} };
globalThis.location = { hash: "" };
globalThis.window = { addEventListener() {} };
globalThis.window.parent = globalThis.window;
export { ids };
"##;

/// Driving the panel of a rule with two sequences: a caption, a table and an "add" button for each,
/// three lines and two payments filled in, Compute, then the second example.
const DRIVE: &str = r##"
const kids = ids["#try-rows"].children;
console.log(JSON.stringify(kids.map((k) => k.tagName + ":" + k.textContent)));
const lines = kids[1].children[1];
const pays = kids[4].children[1];
kids[2].click(); kids[2].click();
kids[5].click();
const set = (body, r, vals) => { const tds = body.children[r].children; vals.forEach((v, i) => { tds[i].children[0].value = v; }); };
set(lines, 0, ["2", "30"]); set(lines, 1, ["1", "120"]); set(lines, 2, ["4", "15"]);
set(pays, 0, ["50"]); set(pays, 1, ["50"]);
ids["#try-buttons"].children[0].click();
console.log(JSON.stringify({ result: ids["#try-result"].textContent, record: ids["#try-record"].textContent }));
ids["#try-buttons"].children[2].click();
console.log(JSON.stringify({ lines: lines.children.length, pays: pays.children.length, result: ids["#try-result"].textContent }));
"##;

/// The page of a rule that walks two sequences has an editor for each, passes them in the order the
/// function takes them, and an example fills both (§15.219). The page of a rule that walks one, or
/// none, keeps the script it had.
#[test]
fn 二つの並びのページは並びごとに編集して走る() {
    if !ready(Need::Node, || have("node"), "node is not installed") {
        return;
    }
    let (_tmp, d) = dir("page");
    let out = d.join("out");
    let (code, said, e) = run(&["gen", "tests/corpus/order_settlement.rule", "--out", out.to_str().unwrap()]);
    assert_eq!(code, 0, "{said}{e}");
    let html = std::fs::read_to_string(out.join("javascript/order_settlement_page.html")).unwrap();
    let head = html.find("<script type=\"module\">").unwrap() + "<script type=\"module\">".len();
    let body = &html[head..head + html[head..].find("</script>").unwrap()];
    write(&d, "dom.mjs", DOM);
    let p = write(&d, "page.mjs", &format!("import {{ ids }} from \"./dom.mjs\";\n{body}\n{DRIVE}"));
    let o = Command::new("node").arg(&p).output().unwrap();
    let said = String::from_utf8_lossy(&o.stdout).into_owned();
    assert!(o.status.success(), "{said}{}", String::from_utf8_lossy(&o.stderr));
    let mut ls = said.lines();
    assert_eq!(ls.next(), Some(r#"["DIV:lines","TABLE:","BUTTON:Add a row","DIV:payments","TABLE:","BUTTON:Add a row"]"#), "{said}");
    let run1 = rulec::json::parse(ls.next().unwrap()).unwrap();
    assert_eq!(run1.get("result").and_then(|x| x.as_str()), Some("subtotal = 240USD   discount = 12USD   paid = 100USD   balance = 128USD"), "{said}");
    let rec = run1.get("record").and_then(|x| x.as_str()).unwrap();
    assert!(rec.contains(r#""payments":[{"amount":50},{"amount":50}]"#), "{rec}");
    // the second example: three lines, no payments
    let ex = rulec::json::parse(ls.next().unwrap()).unwrap();
    assert_eq!((ex.get("lines").and_then(|x| x.as_int()), ex.get("pays").and_then(|x| x.as_int())), (Some(3), Some(0)), "{said}");
    assert!(ex.get("result").and_then(|x| x.as_str()).unwrap().contains("balance = 228USD"), "{said}");
    // one sequence: the page keeps the script that knows one list
    let (_, _, _) = run(&["gen", "tests/corpus/cart_shipping_fee.rule", "--out", out.to_str().unwrap()]);
    let one = std::fs::read_to_string(out.join("javascript/cart_shipping_fee_page.html")).unwrap();
    assert!(one.contains("if (RULE.elements) {") && !one.contains("const lists = "), "the page of one sequence changed its script");
}

/// The document says what each summary is in one sentence, and the order of the walks when there
/// is more than one (§15.219), in English and in Japanese.
#[test]
fn 文書は集計を一文で言い_歩きの順を言う() {
    let (_, en, _) = run(&["doc", "tests/corpus/order_settlement.rule"]);
    for want in [
        "## What the walks find",
        "| subtotal | the total of line_total over lines |",
        "| top_price | the largest price of lines, 0USD when there is none |",
        "The rule walks its sequences in this order: the first walk over lines finds subtotal, top_price; the first walk over payments finds paid; the second walk over lines finds discount, reading what the walks before it found.",
    ] {
        assert!(en.contains(want), "`{want}` is not in:\n{en}");
    }
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "ja").current_dir(root()).args(["doc", "tests/corpus/注文の精算.rule"]).output().unwrap();
    let ja = String::from_utf8_lossy(&o.stdout).into_owned();
    for want in [
        "## 並びをたどって求める値",
        "| 最高単価 | 明細 の 単価 の最大。無ければ 0USD |",
        "この規則は、並びを次の順にたどります。明細 を一回目にたどって 小計、最高単価 を求め、支払い を一回目にたどって 支払済 を求め、明細 を二回目にたどって、前の歩きで求めた値を読みながら 値引き を求めます。",
    ] {
        assert!(ja.contains(want), "`{want}` is not in:\n{ja}");
    }
}

/// `rulec diff` holds two versions to the summaries as well as the rows: a `count` that tests
/// another value answers differently for one sequence (it used to be called the same). A rule that
/// walks two sequences, takes a `max` or a `min`, or walks twice cannot be cut into cells, and is
/// said to be so unless nothing that decides an answer changed (§15.219).
#[test]
fn 版の比べは集計も比べ_切り分けられない歩きはそう言う() {
    let (_tmp, d) = dir("diff");
    let old = read("tests/corpus/supplier_matching.rule");
    let new = old.replace("rule supplier_matching v1", "rule supplier_matching v2").replace("where kind = same ", "where kind = diff ");
    let (a, b) = (write(&d, "old.rule", &old), write(&d, "new.rule", &new));
    let (code, said, e) = run(&["diff", &a, &b]);
    // 1: there is an impact
    assert_eq!(code, 1, "{said}{e}");
    assert!(!said.contains("nothing that decides an answer changed"), "{said}");
    assert!(said.contains(" differ"), "{said}");

    let old = read("tests/corpus/earliest_due.rule");
    let a = write(&d, "e1.rule", &old);
    let same = write(&d, "e2.rule", &old.replace("rule earliest_due v1", "rule earliest_due v2"));
    let (_, said, _) = run(&["diff", &a, &same]);
    assert!(said.contains("nothing that decides an answer changed"), "{said}");
    let moved = write(&d, "e3.rule", &old.replace("rule earliest_due v1", "rule earliest_due v2").replace("empty 2027-12-31", "empty 2027-12-30"));
    let (_, said, _) = run(&["diff", &a, &moved]);
    assert!(said.contains("takes a largest or a smallest value, or walks a sequence twice"), "{said}");
}

/// SQL and NumPy write no rule that walks (§15.56), two sequences or a second walk included, and
/// say so.
#[test]
fn sqlとnumpyは二つの並びの規則も生成しない() {
    let (_tmp, d) = dir("sql");
    let out = d.join("out");
    let (code, said, e) = run(&["gen", "tests/corpus/rate_adjustment.rule", "--out", out.to_str().unwrap()]);
    assert_eq!(code, 0, "{said}{e}");
    assert!(!out.join("sql").exists() || std::fs::read_dir(out.join("sql")).unwrap().next().is_none(), "SQL was written for a walk");
    assert!(!out.join("numpy").exists() || std::fs::read_dir(out.join("numpy")).unwrap().next().is_none(), "NumPy was written for a walk");
}

/// The same row, in a rule whose walk has a `max` beside the count: the pool of a walk with more
/// than one kind of summary holds the count to the row's cell too, out of the sequences that reach
/// each of its values.
#[test]
fn 数えた列と入力の境界が同じ行にあれば_歩きの答えをそろえて境界の対を作る() {
    let src = r#"rule walk_fee(walk_fee) v1
description "The handling fee of an order: waived when two or more lines are in stock and the subtotal is 100 dollars or more; the largest quantity is reported too"

inputs
  subtotal(subtotal) : money[USD]  range >=0USD <=1000USD

elements lines(lines)
  in_stock(in_stock) : bool
  qty(qty)           : number  range >=0 <=10

outputs
  fee(fee)         : money[USD]  round down(1USD)
  largest(largest) : number  round down(1)

count stocked over lines where in_stock = true  range >=0 <=10
max largest over lines of qty empty 0

table fee_of
policy first
| stocked | subtotal  | -> fee : money[USD] |
| >=2     | >=100USD  | 0USD                |
| -       | -         | 5USD                |
"#;
    let (f, c) = rulec::prepare(src, "walk_fee.rule").unwrap_or_else(|ds| panic!("{ds:?}"));
    let vs = rulec::vectors::generate(&f, &c);
    let a = rulec::coverage::audit(&f, &c, "walk_fee.rule", &vs, &[]);
    assert!(a.ok(), "{:?}", a.missing.iter().map(|m| &m.what).collect::<Vec<_>>());
    let outside = |v: &rulec::vectors::Vector| {
        matches!(v.input.get("lines"), Some(Val::Seq(s)) if s.len() == 2) && v.input.get("subtotal") == Some(&Val::Num(Rat::int(99)))
    };
    assert!(vs.iter().any(outside), "no vector with two stocked lines and 99 dollars");
}

/// A fold that answers the number of the first member who qualifies, or 0 when none does: its
/// answer is 1 to 4 or 0 (§15.236). The table below it reads the answer with a row for `0` and one
/// for `>=1`, which covers every value the fold can answer with.
const FIRST_MEMBER: &str = r#"rule first_member v1
description "The first member who qualifies, or 0 when none does"

enum verdict = takes | skips

elements members
  member : number  range >=1 <=4
  score  : number  range >=0 <=10

outputs
  found : bool

table judge
policy unique
| score | -> verdict : verdict |
| >=5   | takes                |
| <5    | skips                |

fold verdict over members -> first : number
  takes     -> take_first member
  skips     -> next
  empty     -> 0
  exhausted -> held

table found_of
policy unique
| first | -> found : bool |
| >=1   | true           |
| 0     | false          |

sequence one
| member | score |
| 2      | 7     |

examples
| members | -> found |
| one     | true     |
"#;

/// The Japanese version of `FIRST_MEMBER`.
const FIRST_MEMBER_JA: &str = r#"rule 最初の該当者(first_member_ja) v1
description "条件に合う最初の人の番号。誰も合わなければ 0"

enum 判定(verdict) = 取る(takes) | 飛ばす(skips)

elements 会員(members)
  番号(member) : number  range >=1 <=4
  点数(score)  : number  range >=0 <=10

outputs
  見つかった(found) : bool

table 判定表(judge)
policy unique
| 点数 | -> 判定 : 判定 |
| >=5  | 取る           |
| <5   | 飛ばす         |

fold 判定 over 会員 -> 最初(first) : number
  取る      -> take_first 番号
  飛ばす    -> next
  empty     -> 0
  exhausted -> held

table 見つかったか(found_of)
policy unique
| 最初 | -> 見つかった : bool |
| >=1  | true                 |
| 0    | false                |
"#;

/// The latest date of a year's lines, read by a table with a row for each half of the year.
const LATEST_LINE: &str = r#"rule latest_line v1
description "Whether the latest line of an order ships in the second half of the year"

elements lines
  ships : date  range >=2026-01-01 <=2026-12-31

outputs
  late : bool

max latest over lines of ships empty 2026-01-01

table late_of
policy unique
| latest                    | -> late : bool |
| >=2026-07-01              | true           |
| >=2026-01-01 <=2026-06-30 | false          |
"#;

/// A table that reads what a walk answers is checked over the range the check worked out for it,
/// as a table that reads an input is checked over the input's declared range (§15.236): a fold's
/// answer over the hull of the values it answers with, as reference §6.2 says, and the latest of
/// the dates a `max` reads over its column's range and its `empty` date. Before, the axis of a
/// fold's answer and of a date's `max` ran over every number, and the table was asked for a row
/// of −1 and of the day before the year. Both re-checkers hold the axis to the range the
/// certificate states.
#[test]
fn 歩きの答えを読む表は_その範囲の上で検査される() {
    for (name, src) in [("first_member.rule", FIRST_MEMBER), ("最初の該当者.rule", FIRST_MEMBER_JA), ("latest_line.rule", LATEST_LINE)] {
        let ds = codes(src);
        assert!(ds.iter().all(|(c, _)| !c.starts_with('E')), "{name}: {ds:?}");
    }
    // The two re-checkers hold each axis to the range the certificate states for it.
    let (d_tmp, d) = dir("walk-answers");
    let python = ready(Need::Python, || have("python3"), "python3 is not installed");
    let bin = root().join("../../proofs/.lake/build/bin/rulec-recheck");
    let lean = ready(Need::Lean, || bin.exists(), "ritsu's proofs/ is not built (lake build there)");
    for (name, src, column, range, forged) in [
        ("first_member.rule", FIRST_MEMBER, "first", r#""first":["0","4"]"#, r#""first":["0","5"]"#),
        ("latest_line.rule", LATEST_LINE, "latest", r#""latest":["20454","20818"]"#, r#""latest":["20454","20819"]"#),
    ] {
        let path = write(&d, name, src);
        let (c, cert, e) = run(&["certificate", &path]);
        assert_eq!(c, 0, "{name}: {e}");
        let j = rulec::json::parse(&cert).unwrap();
        let walks: Vec<&str> = j.get("walks").and_then(|x| x.as_arr()).unwrap().iter().filter_map(|x| x.as_str()).collect();
        assert!(walks.contains(&column), "{name}: {walks:?}");
        assert!(cert.contains(range), "{name}: the range is not stated as {range}:\n{cert}");
        let axis = j.get("tables").and_then(|x| x.as_arr()).unwrap().iter().flat_map(|t| t.get("axes").and_then(|x| x.as_arr()).unwrap().iter()).find(|a| a.get("column").and_then(|x| x.as_str()) == Some(column)).expect("no axis");
        assert_eq!(axis.get("kind").and_then(|x| x.as_str()), Some("walk"), "{name}");
        let forged = cert.replace(range, forged);
        assert_ne!(forged, cert);
        if python {
            let (code, said) = recheck(&cert, &path);
            assert_eq!(code, 0, "{name}: {said}");
            let (code, said) = recheck(&forged, &path);
            assert_eq!(code, 1, "{name}: a forged range passed\n{said}");
            assert!(said.contains(&format!("{column} runs from")) && said.contains("and its range is"), "{name}: {said}");
        }
        if lean {
            use std::io::Write;
            let run_lean = |cert: &str| {
                let mut p = Command::new(&bin).current_dir(root()).args(["--rule", &path]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
                p.stdin.as_mut().unwrap().write_all(cert.as_bytes()).unwrap();
                let o = p.wait_with_output().unwrap();
                (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned())
            };
            let (code, said) = run_lean(&cert);
            assert_eq!(code, 0, "{name}: {said}");
            let (code, said) = run_lean(&forged);
            assert_eq!(code, 1, "{name}: a forged range passed\n{said}");
            assert!(said.contains(&format!("{column} does not run the whole of the range it states")), "{name}: {said}");
        }
    }
    drop(d_tmp);
}
