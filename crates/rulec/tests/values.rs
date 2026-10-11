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
    let py = rulec::codegen::Gen::new(&f, &c, &src, "").python();
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

/// A word in an output cell, or in an example's expected value, is read in the type of its column
/// (§15.208): `gold` in the column of `outcome` is the value, though an output of numbers is called
/// `gold` too. It used to be read as that output and refused with E103. The English rule, and its
/// Japanese version.
const VALUE_LIKE_A_NAME: [&str; 2] = [
    "rule treasure v1\ndescription \"What a roll of the dice finds, and how much gold comes with it\"\n\nenum outcome = found_gold | gold | nothing\n\ninputs\n  roll : number  range >=0 <=3\n\noutputs\n  result_kind : outcome\n  gold        : number  round down(1)\n\ntable t\npolicy unique\n| roll | -> result_kind | gold |\n| 0    | gold          | 25   |\n| 1    | found_gold    | 50   |\n| >=2  | nothing       | 0    |\n\nexamples\n| roll | -> result_kind | gold |\n| 0    | gold           | 25   |\n| 2    | nothing        | 0    |\n",
    "rule 宝(treasure) v1\ndescription \"さいころの目で見つかるものと、ついてくる金の量\"\n\nenum 結果(outcome) = 金鉱(found_gold) | 金(gold) | 何も無し(nothing)\n\ninputs\n  目(roll) : number  range >=0 <=3\n\noutputs\n  種類(result_kind) : 結果\n  金(gold)          : number  round down(1)\n\ntable 表(t)\npolicy unique\n| 目   | -> 種類 | 金 |\n| 0    | 金      | 25 |\n| 1    | 金鉱    | 50 |\n| >=2  | 何も無し | 0  |\n\nexamples\n| 目 | -> 種類 | 金 |\n| 0  | 金      | 25 |\n| 2  | 何も無し | 0  |\n",
];

#[test]
fn 出力のセルの語は列の型で読む() {
    use rulec::eval::Val;
    for (src, lang) in VALUE_LIKE_A_NAME.iter().zip([rulec::i18n::Lang::En, rulec::i18n::Lang::Ja]) {
        let ds = rulec::i18n::with(lang, || rulec::check_source(src, "a.rule"));
        assert!(ds.is_empty(), "{:?}\n{src}", ds.iter().map(|d| (d.code, d.title.clone())).collect::<Vec<_>>());
        // the evaluator answers the value, and the number beside it
        let (f, c) = rulec::prepare(src, "a.rule").unwrap();
        let roll = f.inputs[0].name.text.clone();
        let (outs, _, _) = rulec::eval::run_all(&f, &c, std::collections::HashMap::from([(roll, Val::Num(rulec::num::Rat::int(0)))]));
        let gold = if lang == rulec::i18n::Lang::En { "gold" } else { "金" };
        assert_eq!(outs[0].1, Some(Val::Enum(gold.into())), "{outs:?}");
        assert_eq!(outs[1].1, Some(Val::Num(rulec::num::Rat::int(25))), "{outs:?}");
    }
    // and the generated code writes the enum's member, not the output
    let t = ritsu_testkit::TempDir::new("values-words");
    let p = t.path().join("treasure.rule");
    std::fs::write(&p, VALUE_LIKE_A_NAME[0]).unwrap();
    let out = t.path().join("out");
    let o = std::process::Command::new(env!("CARGO_BIN_EXE_rulec")).args(["gen", p.to_str().unwrap(), "--out", out.to_str().unwrap()]).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let py = std::fs::read_to_string(out.join("python/treasure.py")).unwrap();
    assert!(py.contains("result_kind = Outcome.GOLD"), "{py}");
}

/// Where the column's enum has the value and a name of that same enum is spelled the same, the
/// word reads either way, and the writer is asked to rename one (E075), in the table's output cell
/// and in a fold's answer alike. The English rule, and its Japanese version.
#[test]
fn 同じ列挙の値と名前に当たる語は_e075() {
    let cases = [
        (rulec::i18n::Lang::En, "rule carry_stage v1\n\nenum stage = fresh | previous | done\n\ninputs\n  previous : stage\n  close    : bool\n\noutputs\n  next_stage : stage\n\ntable decide\npolicy unique\n| close | -> next_stage : stage |\n| true  | done                 |\n| false | previous             |\n", "`previous` is both a value of enum stage and the name of an input"),
        (rulec::i18n::Lang::Ja, "rule 段階の引き継ぎ(carry_stage) v1\n\nenum 段階(stage) = 新規(fresh) | 前回(previous) | 終了(done)\n\ninputs\n  前回(previous) : 段階\n  閉じる(close)  : bool\n\noutputs\n  次の段階(next_stage) : 段階\n\ntable 決め方(decide)\npolicy unique\n| 閉じる | -> 次の段階 : 段階 |\n| true   | 終了              |\n| false  | 前回              |\n", "`前回` は、列挙 段階 の値でも、入力の名前でもあります"),
        (rulec::i18n::Lang::En, "rule plan_choice v1\n\nenum plan = basic | pro | current\nenum verdict = take | skip\n\ninputs\n  current : plan\n\nelements offers\n  offered : plan\n  ok      : bool\n\noutputs\n  chosen : plan\n\ntable judge\npolicy unique\n| ok    | -> verdict : verdict |\n| true  | take                 |\n| false | skip                 |\n\nfold verdict over offers\n  take      -> take_first offered\n  skip      -> next\n  empty     -> current\n  exhausted -> held\n", "`current` is both a value of enum plan and the name of an input"),
        (rulec::i18n::Lang::Ja, "rule プランの選択(plan_choice) v1\n\nenum プラン(plan) = 基本(basic) | 上位(pro) | 現在のプラン(current)\nenum 判定(verdict) = 採る(take) | 飛ばす(skip)\n\ninputs\n  現在のプラン(current) : プラン\n\nelements 提案(offers)\n  提案のプラン(offered) : プラン\n  可(ok)                : bool\n\noutputs\n  選んだプラン(chosen) : プラン\n\ntable 判定の表(judge)\npolicy unique\n| 可    | -> 判定 : 判定 |\n| true  | 採る           |\n| false | 飛ばす         |\n\nfold 判定 over 提案\n  採る      -> take_first 提案のプラン\n  飛ばす    -> next\n  empty     -> 現在のプラン\n  exhausted -> held\n", "`現在のプラン` は、列挙 プラン の値でも、入力の名前でもあります"),
    ];
    for (lang, src, title) in cases {
        let ds = rulec::i18n::with(lang, || rulec::check_source(src, "a.rule"));
        let e075: Vec<_> = ds.iter().filter(|d| d.code == "E075").collect();
        assert_eq!(e075.len(), 1, "{:?}\n{src}", ds.iter().map(|d| (d.code, d.title.clone())).collect::<Vec<_>>());
        assert_eq!(e075[0].title, title);
        // the value is not then said to appear nowhere
        let word = title.split('`').nth(1).unwrap();
        for d in ds.iter().filter(|d| d.code == "W111") {
            assert!(!d.notes.iter().any(|n| n.split([' ', '/', ':']).any(|w| w == word)), "{word}: {:?}", d.notes);
        }
    }
}

