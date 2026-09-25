//! What an enum value may be called (§15.150).
//!
//! A value is read in the enum of the place it is written, so two enums may both have
//! `automatic` — Stripe's `capture_method` and `confirmation_method` do. Before, the checker
//! looked the name up in whichever enum came first and refused the other column's cells, and
//! the generators wrote every `automatic` as the last enum's member. A value never starts a
//! line either, so it is held to the eleven reserved words its own position reads as something
//! else, not to all of them: a state may be called `initial`, `held` or `final`.

fn codes(src: &str) -> Vec<String> {
    rulec::check_source(src, "a.rule").into_iter().map(|d| d.code.to_string()).collect()
}

fn titles(src: &str, code: &str) -> Vec<String> {
    rulec::check_source(src, "a.rule").into_iter().filter(|d| d.code == code).map(|d| d.title).collect()
}

/// Two enums with the same two values, a table reading both, and an output of the second.
fn shared(extra: &str, rows: &str) -> String {
    format!(
        "rule t v1

enum capture = automatic | manual
enum confirmation = automatic | manual
{extra}
inputs
  cap  : capture
  conf : confirmation

outputs
  how : confirmation

table u
policy unique
{rows}"
    )
}

const ROWS: &str = "| cap       | conf      | -> how    |
| automatic | automatic | automatic |
| automatic | manual    | manual    |
| manual    | -         | manual    |
";

#[test]
fn 二つの列挙に同じ名前の値があってよい() {
    assert_eq!(codes(&shared("", ROWS)), Vec::<String>::new());
}

#[test]
fn 同じ名前の値は列の列挙で生成される() {
    let src = shared("", ROWS);
    let (f, c) = rulec::prepare(&src, "a.rule").unwrap_or_else(|d| panic!("{:?}", d.iter().map(|d| d.code).collect::<Vec<_>>()));
    let py = rulec::codegen::Gen::new(&f, &c, &src).python();
    assert!(py.contains("cap == Capture.AUTOMATIC and conf == Confirmation.AUTOMATIC"), "{py}");
    assert!(py.contains("how = Confirmation.AUTOMATIC"), "{py}");
}

/// A value of one enum in a column of another passed in silence when it was an output cell,
/// and the generated code returned a value of the wrong type.
#[test]
fn 出力のセルに別の列挙の値を書くと_e103() {
    let src = "rule t v1

enum color = red | green
enum size = small | large

inputs
  c : color

outputs
  s : size

table u
policy unique
| c     | -> s  |
| red   | small |
| green | red   |
";
    let t = titles(src, "E103");
    assert_eq!(t.len(), 1, "{t:?}");
    assert!(t[0].contains("`red`") && t[0].contains("color"), "{t:?}");
}

/// A value two enums share leaves the other enum's value unnamed.
#[test]
fn 使われていない値は列挙ごとに数える() {
    let rows = "| cap       | conf      | -> how    |
| automatic | -         | manual    |
| manual    | -         | manual    |
";
    let ws = rulec::check_source(&shared("", rows), "a.rule");
    let w: Vec<_> = ws.iter().filter(|d| d.code == "W111").collect();
    assert_eq!(w.len(), 1, "{:?}", ws.iter().map(|d| (d.code, d.title.clone())).collect::<Vec<_>>());
    assert!(w[0].title.contains("confirmation"), "{}", w[0].title);
}

/// A group of values two enums share belongs to the enum of the first column it is written in,
/// and a column of the other enum is then refused rather than read as the first one's.
#[test]
fn 共有する値の群は最初に使った列の列挙になる() {
    let first = "| cap | conf | -> how    |
| -   | auto | automatic |
| -   | man  | manual    |
";
    // Read as confirmation's values: only capture's go unnamed.
    let ds = rulec::check_source(&shared("group auto = automatic\ngroup man = manual\n", first), "a.rule");
    let got: Vec<(&str, String)> = ds.iter().map(|d| (d.code, d.title.clone())).collect();
    assert!(got.iter().all(|(c, _)| *c == "W111"), "{got:?}");
    assert!(got.len() == 1 && got[0].1.contains("capture"), "{got:?}");
    let both = "| cap  | conf | -> how    |
| auto | auto | automatic |
| auto | man  | manual    |
| man  | -    | manual    |
";
    let t = titles(&shared("group auto = automatic\ngroup man = manual\n", both), "E103");
    assert!(!t.is_empty() && t.iter().all(|x| x.contains("`auto`") || x.contains("`man`")), "{t:?}");
}

#[test]
fn 群に二つの列挙の値を混ぜると_e103() {
    let rows = "| cap   | conf | -> how |
| mixed | -    | manual |
| -     | -    | manual |
";
    let src = format!(
        "rule t v1

enum capture = automatic | manual
enum confirmation = prompt | later

group mixed = automatic, later

inputs
  cap  : capture
  conf : confirmation

outputs
  how : capture

table u
policy first
{rows}"
    );
    let t = titles(&src, "E103");
    assert!(t.iter().any(|x| x.contains("mixed")), "{t:?}");
}

/// A state may be called like a line of the language: it never starts one.
#[test]
fn 値は予約語でもよい() {
    let src = "rule t v1

enum stage = initial | held | review | final
enum signal = next | stop | over | where | result | table

inputs
  stage  : stage
  signal : signal

outputs
  next_stage : stage

table step
policy unique
| stage   | signal               | -> next_stage |
| initial | next                 | held          |
| initial | not: next            | stage         |
| held    | next, over           | review        |
| held    | stop                 | final         |
| held    | where, result, table | stage         |
| review  | next                 | final         |
| review  | not: next            | stage         |
| final   | -                    | stage         |

machine flow over step
  carry   stage -> next_stage
  initial initial
  final   final
  never   initial after held
";
    assert_eq!(codes(src), Vec::<String>::new());
}

/// The eleven words a value's own position reads as something else stay out of reach.
#[test]
fn 値の位置で別の意味になる語は値にできない() {
    for w in rulec::kw::VALUE_RESERVED {
        let src = format!(
            "rule t v1\n\nenum k = a | {w}\n\ninputs\n  x : k\n\noutputs\n  y : bool\n\n\
             table u\npolicy unique\n| x | -> y  |\n| - | true |\n"
        );
        assert!(codes(&src).contains(&"E009".to_string()), "{w}: {:?}", codes(&src));
    }
    // A name is still held to all of them.
    let src = "rule t v1\n\ninputs\n  held : bool\n\noutputs\n  y : bool\n\n\
               table u\npolicy unique\n| held | -> y  |\n| -    | true |\n";
    assert!(codes(src).contains(&"E009".to_string()), "{:?}", codes(src));
}
