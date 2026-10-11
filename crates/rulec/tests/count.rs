//! `count` (§15.58): the walk's summary, as a number the rest of the rule can use.
//!
//! A fold ends the walk with the answer; a count ends it with a number and hands the rule
//! back to the tables. That is the whole point: what turns "how many matched" into a class
//! is an ordinary table, and an ordinary table is checked. What is tested here is that the
//! number really is ordinary — a column, a range, a guard — and that every generated
//! language counts the same elements the reference evaluator does.

use std::path::PathBuf;
use std::process::Command;
use ritsu_testkit::{Need, TempDir, ready};
use ritsu_testkit::tmp::tmpdir_in;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn dir(tag: &str) -> (TempDir, PathBuf) {
    let t = TempDir::new(&format!("count-{tag}"));
    let d = t.path().to_path_buf();
    (t, d)
}

fn run(args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "ja")
        .current_dir(root())
        .args(args)
        .output()
        .expect("rulec を起動できない");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

/// `run`, with `tmp` as rulec's TMPDIR. `rulec test` asks swiftc for its version, and swiftc
/// leaves an empty directory in its TMPDIR almost every time; under the test's directory, it goes
/// with that.
fn run_tmp(tmp: &std::path::Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .env("RULEC_LANG", "ja")
        .env("TMPDIR", tmp)
        .current_dir(root())
        .args(args)
        .output()
        .expect("rulec を起動できない");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

fn write(d: &PathBuf, name: &str, body: &str) -> String {
    let p = d.join(name);
    std::fs::write(&p, body).unwrap();
    p.to_str().unwrap().to_string()
}

fn have(cmd: &str) -> bool {
    Command::new("sh")
        .args(["-c", &format!("command -v {cmd} >/dev/null 2>&1")])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn codes(out: &str) -> Vec<String> {
    out.lines()
        .filter_map(|l| rulec::json::parse(l).ok())
        .filter_map(|j| j.get("code").and_then(|c| c.as_str()).map(|s| s.to_string()))
        .collect()
}

/// Candidates matched one at a time, and the number of matches decides the answer.
const RULE: &str = r#"rule 納入先照合(supplier_match) v1
description "候補を一件ずつ照合し、一致した件数で結果を決める"

enum 照合結果(hit) = 一致(match) | 不一致(miss)
enum 判定(verdict) = 該当なし(none) | 一件(one) | 複数(many)

elements 候補(candidates)
  会社名一致(name_match) : bool
  住所一致(addr_match)   : bool

outputs
  結果(result) : 判定

table 候補判定(row_of)
policy unique
| 会社名一致 | 住所一致 | -> 照合(hit) : 照合結果 |
| true       | true     | 一致                    |
| true       | false    | 不一致                  |
| false      | -        | 不一致                  |

count 一致数(hits) over 候補 where 照合 = 一致  range >=0 <=100

table 結果判定(verdict_of)
policy unique
| 一致数 | -> 結果(result) : 判定 |
| 0      | 該当なし               |
| 1      | 一件                   |
| >=2    | 複数                   |
"#;

/// The examples a walk needs: a named sequence, and a cell naming one.
const EXAMPLES: &str = r#"
sequence 一件だけ一致(one)
| 会社名一致 | 住所一致 |
| true       | true     |
| true       | false    |

sequence 二件一致(two)
| 会社名一致 | 住所一致 |
| true       | true     |
| true       | true     |

sequence 空(none)
| 会社名一致 | 住所一致 |

examples
| 候補         | -> 結果  |
| 一件だけ一致 | 一件     |
| 二件一致     | 複数     |
| 空           | 該当なし |
"#;

#[test]
fn 数えた結果は表の列になる() {
    let (_tmp, d) = dir("ok");
    let p = write(&d, "r.rule", &format!("{RULE}{EXAMPLES}"));
    let (code, said, e) = run(&["check", &p]);
    assert_eq!(code, 0, "{said}{e}");
}

/// The range is the universe the completeness check quantifies over. Without it the check
/// would ask for a row covering a count of −1, so it is required rather than guessed.
#[test]
fn 範囲の無い数え上げはエラーにする() {
    let (_tmp, d) = dir("range");
    let p = write(&d, "r.rule", &RULE.replace("  range >=0 <=100", ""));
    let (code, said, _) = run(&["check", &p, "--format", "json"]);
    assert_eq!(code, 1);
    assert!(codes(&said).contains(&"E030".to_string()), "{said}");
}

/// Counting something there is one of per call could only ever answer 0 or 1, and an enum
/// column with no value says nothing about which elements are wanted.
#[test]
fn 数えられない列は指摘される() {
    for (what, src) in [
        ("入力", RULE.replace("where 照合 = 一致", "where 会社名一致 = true").replace(
            "elements 候補(candidates)\n  会社名一致(name_match) : bool",
            "inputs\n  会社名一致(name_match) : bool\n\nelements 候補(candidates)\n  住所照合(addr_check) : bool",
        )),
        ("値の無い列挙", RULE.replace("where 照合 = 一致", "where 照合")),
    ] {
        let (_tmp, d) = dir("bad");
        let p = write(&d, "r.rule", &src);
        let (code, said, _) = run(&["check", &p, "--format", "json"]);
        assert_eq!(code, 1, "{what}: {said}");
        assert!(codes(&said).contains(&"E029".to_string()), "{what}: {said}");
    }
}

#[test]
fn 書き方の間違いは指摘される() {
    let (_tmp, d) = dir("shape");
    let p = write(&d, "r.rule", &RULE.replace("over 候補 where", "where"));
    let (code, said, _) = run(&["check", &p, "--format", "json"]);
    assert_eq!(code, 1);
    assert!(codes(&said).contains(&"E028".to_string()), "{said}");
}

/// Two endings for one walk. A fold may stop partway, and what a count means on a walk that
/// stopped is not decided — so the rule is refused rather than answered.
#[test]
fn foldとcountは一緒に書けない() {
    let (_tmp, d) = dir("both");
    let src = format!(
        "{RULE}\nfold 照合 over 候補\n  一致 -> next\n  不一致 -> next\n  empty -> 該当なし\n  exhausted -> 該当なし\n"
    );
    let p = write(&d, "r.rule", &src);
    let (code, said, _) = run(&["check", &p, "--format", "json"]);
    assert_eq!(code, 1);
    assert!(codes(&said).contains(&"E031".to_string()), "{said}");
}

/// A count nothing reads is a walk for nothing.
#[test]
fn 使われない数え上げは注意される() {
    let (_tmp, d) = dir("unused");
    let src = RULE.replace("| 一致数 | -> 結果(result) : 判定 |\n| 0      | 該当なし               |\n| 1      | 一件                   |\n| >=2    | 複数                   |",
                           "| 会社名一致 | -> 結果(result) : 判定 |\n| -          | 該当なし               |");
    let p = write(&d, "r.rule", &src);
    let (_, said, _) = run(&["check", &p, "--format", "json"]);
    assert!(codes(&said).contains(&"W111".to_string()), "{said}");
}

/// The declared range caps the sequence: a longer one leaves the universe the proof was made
/// over, so the generated code refuses it at the door — and the vectors say the same.
#[test]
fn 上限を超えた並びはエラーになる() {
    if !ready(Need::Python, || have("python3"), "python3 が無いので飛ばした") {
        return;
    }
    let (_tmp, d) = dir("cap");
    let p = write(&d, "r.rule", &RULE.replace("<=100", "<=2"));
    let out = d.join("out");
    let (code, said, e) = run(&["gen", &p, "--out", out.to_str().unwrap()]);
    assert_eq!(code, 0, "{said}{e}");
    let py = std::fs::read_to_string(out.join("python/supplier_match.py")).unwrap();
    assert!(py.contains("if _len(candidates) > 2:"), "上限の検査が生成されていない:\n{py}");

    // Three elements: the rule has no answer, and the generated code says so rather than
    // counting past its own declaration.
    let call = out.join("call.py");
    std::fs::write(
        &call,
        "import sys, json\nsys.path.insert(0, \"python\")\nimport supplier_match as m\n\
         xs = [m.Element(True, True)] * 3\n\
         try:\n    m.supplier_match(xs)\n    print(\"answered\")\nexcept m.RuleInputError:\n    print(\"refused\")\n",
    )
    .unwrap();
    let o = Command::new("python3").current_dir(&out).arg(&call).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), "refused", "{:?}", o);
}

/// Every language counts the same elements the reference evaluator counts.
#[test]
fn 生成された数え上げは参照評価器と一致する() {
    if !ready(Need::Python, || have("python3"), "python3 が無いので飛ばした") {
        return;
    }
    let (_tmp, d) = dir("agree");
    let p = write(&d, "r.rule", &format!("{RULE}{EXAMPLES}"));
    let out = d.join("out");
    let (code, said, e) = run(&["gen", &p, "--out", out.to_str().unwrap()]);
    assert_eq!(code, 0, "{said}{e}");

    let (_, said, _) = run_tmp(&tmpdir_in(&out), &["test", out.to_str().unwrap(), "--format", "json"]);
    let j = rulec::json::parse(said.lines().next().expect("結果が無い")).unwrap();
    let rulec::json::Json::Arr(rs) = j.get("results").unwrap() else { panic!("{said}") };
    let mut ran = 0;
    for r in rs {
        if r.get("ran") != Some(&rulec::json::Json::Bool(true)) {
            continue;
        }
        ran += 1;
        assert_eq!(r.get("ok"), Some(&rulec::json::Json::Bool(true)), "{said}");
    }
    assert!(ran >= 1, "どの言語も走らなかった: {said}");
}

/// SQL gets no walk at all, and a count is a walk.
#[test]
fn sqlには生成しない() {
    let (_tmp, d) = dir("sql");
    let p = write(&d, "r.rule", RULE);
    let out = d.join("out");
    let (code, said, e) = run(&["gen", &p, "--out", out.to_str().unwrap()]);
    assert_eq!(code, 0, "{said}{e}");
    assert!(!out.join("sql").exists(), "SQL を書いてしまった");
    assert!(said.contains("SQL"), "生成しなかったことを言っていない:\n{said}");
}

// --- Named like an output, and the stages of a walk (§15.208)

/// A count, a sum and a derive named like the output they answer are that output, and W111 says
/// nothing of them: it used to call each "never used". The English rule, and its Japanese version.
const NAMED_LIKE_OUTPUTS: &[(&str, &str)] = &[
    (
        "rule heavy_parcels v1\ndescription \"How many parcels are heavy. The count is named like the output it answers\"\n\nenum weight_class = light | heavy\n\nelements parcels\n  weight : number  range >=0 <=30\n\noutputs\n  heavy_parcels : number  round down(1)\n\ntable class_of\npolicy unique\n| weight | -> kind : weight_class |\n| <=10   | light                  |\n| >10    | heavy                  |\n\ncount heavy_parcels over parcels where kind = heavy  range >=0 <=50\n\nsequence mixed\n| weight |\n| 5      |\n| 12     |\n\nexamples\n| parcels | -> heavy_parcels |\n| mixed   | 1                |\n",
        "rule 重い荷物の数(heavy_parcels) v1\ndescription \"重い荷物の数。数え上げは、それが答える出力と同じ名前\"\n\nenum 重さの区分(weight_class) = 軽い(light) | 重い(heavy)\n\nelements 荷物(parcels)\n  重さ(weight) : number  range >=0 <=30\n\noutputs\n  重い荷物(heavy_parcels) : number  round down(1)\n\ntable 重さの表(class_of)\npolicy unique\n| 重さ | -> 種類(kind) : 重さの区分 |\n| <=10 | 軽い                       |\n| >10  | 重い                       |\n\ncount 重い荷物 over 荷物 where 種類 = 重い  range >=0 <=50\n\nsequence 混在(mixed)\n| 重さ |\n| 5    |\n| 12   |\n\nexamples\n| 荷物 | -> 重い荷物 |\n| 混在 | 1           |\n",
    ),
    (
        "rule order_total v1\ndescription \"The total of an order: the goods and the shipping. The derive is named like the output it answers\"\n\ninputs\n  goods    : number  range >=0 <=1000\n  shipping : number  range >=0 <=100\n\noutputs\n  total : number  round down(1)\n\nderive total : number = goods + shipping  range >=0 <=1100\n\nexamples\n| goods | shipping | -> total |\n| 300   | 40       | 340      |\n",
        "rule 注文の合計(order_total) v1\ndescription \"注文の合計。品物と送料。導出は、それが答える出力と同じ名前\"\n\ninputs\n  品物(goods)    : number  range >=0 <=1000\n  送料(shipping) : number  range >=0 <=100\n\noutputs\n  合計(total) : number  round down(1)\n\nderive 合計 : number = 品物 + 送料  range >=0 <=1100\n\nexamples\n| 品物 | 送料 | -> 合計 |\n| 300  | 40   | 340     |\n",
    ),
];

#[test]
fn 出力と同じ名前のまとめと導出は使われている() {
    for (en, ja) in NAMED_LIKE_OUTPUTS {
        for (src, lang) in [(en, rulec::i18n::Lang::En), (ja, rulec::i18n::Lang::Ja)] {
            let ds = rulec::i18n::with(lang, || rulec::check_source(src, "v.rule"));
            assert!(ds.is_empty(), "{:?}\n{src}", ds.iter().map(|d| (d.code, d.title.clone())).collect::<Vec<_>>());
        }
    }
    // the sum of tests/stages/parcel_charge.rule is named like its output too, and the rule
    // passes with nothing said (tests/fold.rs)
    let pc = std::fs::read_to_string(root().join("tests/stages/parcel_charge.rule")).unwrap();
    assert!(pc.contains("sum charge over parcels") && pc.contains("  charge : number"), "{pc}");
}

/// What a walk answers is one value for the whole sequence, and what is decided on one element is
/// gone once the walk moves on: an output that is a value of one element, and an item that reads
/// both kinds, are E031, each with what it reads named. The English rule of `tests/stages`, and its
/// Japanese version.
#[test]
fn 要素ごとの値と_たどった結果は混ぜられない() {
    use rulec::i18n::Lang;
    let en = std::fs::read_to_string(root().join("tests/stages/parcel_charge.rule")).unwrap();
    let ja = std::fs::read_to_string(root().join("tests/stages/荷物の料金.rule")).unwrap();
    let cases = [
        ("an output of one element", Lang::En, en.replace("  tier   : charge_tier\n", "  tier   : charge_tier\n  cost   : number  round down(1)\n"), "Output cost is a value of one element", "once the walk is over, it is not one value"),
        ("an output of one element (ja)", Lang::Ja, ja.replace("  区分(tier)   : 料金の区分\n", "  区分(tier)   : 料金の区分\n  費用(cost)   : number  round down(1)\n").replace("define 費用(cost) :", "define 費用 :"), "出力 費用 は、要素ごとの値です", "並びをたどり終えたあとには、一つの値に決まりません"),
        ("both kinds in one item", Lang::En, en.replace("define cost : number = weight * factor\n", "define cost : number = weight * factor\ndefine rest : number = charge - cost\n"), "One item cannot yet read both a value of one element and what the walk answers", "definition rest reads both cost, a value of one element, and charge, what the walk answers"),
        ("both kinds in one item (ja)", Lang::Ja, ja.replace("define 費用(cost) : number = 重さ * 係数\n", "define 費用(cost) : number = 重さ * 係数\ndefine ほかの荷物(rest) : number = 料金 - 費用\n"), "要素の値と、並びをたどった結果を、一つの項目で読むことはまだできません", "定義 ほかの荷物 は、要素の値 費用 と、たどった結果 料金 の両方を読んでいます"),
    ];
    for (what, lang, src, title, mark) in cases {
        assert_ne!(src, if lang == Lang::En { en.clone() } else { ja.clone() }, "{what}: the rule did not change");
        let ds = rulec::i18n::with(lang, || rulec::check_source(&src, "v.rule"));
        let d = ds.iter().find(|d| d.code == "E031").unwrap_or_else(|| panic!("{what}: {:?}", ds.iter().map(|d| (d.code, d.title.clone())).collect::<Vec<_>>()));
        assert_eq!(d.title, title, "{what}");
        assert!(d.marks.iter().any(|m| m.label == mark), "{what}: {:?}", d.marks);
    }
}

