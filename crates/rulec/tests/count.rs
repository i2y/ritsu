//! `count` (§15.58): the walk's summary, as a number the rest of the rule can use.
//!
//! A fold ends the walk with the answer; a count ends it with a number and hands the rule
//! back to the tables. That is the whole point: what turns "how many matched" into a class
//! is an ordinary table, and an ordinary table is checked. What is tested here is that the
//! number really is ordinary — a column, a range, a guard — and that every generated
//! language counts the same elements the reference evaluator does.

use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rulec-count-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn run(args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
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
    let d = dir("ok");
    let p = write(&d, "r.rule", &format!("{RULE}{EXAMPLES}"));
    let (code, said, e) = run(&["check", &p]);
    assert_eq!(code, 0, "{said}{e}");
    let _ = std::fs::remove_dir_all(&d);
}

/// The range is the universe the completeness check quantifies over. Without it the check
/// would ask for a row covering a count of −1, so it is required rather than guessed.
#[test]
fn 範囲の無い数え上げは断る() {
    let d = dir("range");
    let p = write(&d, "r.rule", &RULE.replace("  range >=0 <=100", ""));
    let (code, said, _) = run(&["check", &p, "--format", "json"]);
    assert_eq!(code, 1);
    assert!(codes(&said).contains(&"E030".to_string()), "{said}");
    let _ = std::fs::remove_dir_all(&d);
}

/// Counting something there is one of per call could only ever answer 0 or 1, and an enum
/// column with no value says nothing about which elements are wanted.
#[test]
fn 数えられない列は名指しされる() {
    for (what, src) in [
        ("入力", RULE.replace("where 照合 = 一致", "where 会社名一致 = true").replace(
            "elements 候補(candidates)\n  会社名一致(name_match) : bool",
            "inputs\n  会社名一致(name_match) : bool\n\nelements 候補(candidates)\n  住所照合(addr_check) : bool",
        )),
        ("値の無い列挙", RULE.replace("where 照合 = 一致", "where 照合")),
    ] {
        let d = dir("bad");
        let p = write(&d, "r.rule", &src);
        let (code, said, _) = run(&["check", &p, "--format", "json"]);
        assert_eq!(code, 1, "{what}: {said}");
        assert!(codes(&said).contains(&"E029".to_string()), "{what}: {said}");
        let _ = std::fs::remove_dir_all(&d);
    }
}

#[test]
fn 書き方の間違いは名指しされる() {
    let d = dir("shape");
    let p = write(&d, "r.rule", &RULE.replace("over 候補 where", "where"));
    let (code, said, _) = run(&["check", &p, "--format", "json"]);
    assert_eq!(code, 1);
    assert!(codes(&said).contains(&"E028".to_string()), "{said}");
    let _ = std::fs::remove_dir_all(&d);
}

/// Two endings for one walk. A fold may stop partway, and what a count means on a walk that
/// stopped is not decided — so the rule is refused rather than answered.
#[test]
fn foldとcountは一緒に書けない() {
    let d = dir("both");
    let src = format!(
        "{RULE}\nfold 照合 over 候補\n  一致 -> next\n  不一致 -> next\n  empty -> 該当なし\n  exhausted -> 該当なし\n"
    );
    let p = write(&d, "r.rule", &src);
    let (code, said, _) = run(&["check", &p, "--format", "json"]);
    assert_eq!(code, 1);
    assert!(codes(&said).contains(&"E031".to_string()), "{said}");
    let _ = std::fs::remove_dir_all(&d);
}

/// A count nothing reads is a walk for nothing.
#[test]
fn 使われない数え上げは注意される() {
    let d = dir("unused");
    let src = RULE.replace("| 一致数 | -> 結果(result) : 判定 |\n| 0      | 該当なし               |\n| 1      | 一件                   |\n| >=2    | 複数                   |",
                           "| 会社名一致 | -> 結果(result) : 判定 |\n| -          | 該当なし               |");
    let p = write(&d, "r.rule", &src);
    let (_, said, _) = run(&["check", &p, "--format", "json"]);
    assert!(codes(&said).contains(&"W111".to_string()), "{said}");
    let _ = std::fs::remove_dir_all(&d);
}

/// The declared range caps the sequence: a longer one leaves the universe the proof was made
/// over, so the generated code refuses it at the door — and the vectors say the same.
#[test]
fn 上限を超えた並びは断られる() {
    if !have("python3") {
        eprintln!("注意: python3 が無いので飛ばした");
        return;
    }
    let d = dir("cap");
    let p = write(&d, "r.rule", &RULE.replace("<=100", "<=2"));
    let out = d.join("out");
    let (code, said, e) = run(&["gen", &p, "--out", out.to_str().unwrap()]);
    assert_eq!(code, 0, "{said}{e}");
    let py = std::fs::read_to_string(out.join("python/supplier_match.py")).unwrap();
    assert!(py.contains("if len(candidates) > 2:"), "上限の検査が生成されていない:\n{py}");

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
    let _ = std::fs::remove_dir_all(&d);
}

/// Every language counts the same elements the reference evaluator counts.
#[test]
fn 生成された数え上げは参照評価器と一致する() {
    if !have("python3") {
        eprintln!("注意: python3 が無いので飛ばした");
        return;
    }
    let d = dir("agree");
    let p = write(&d, "r.rule", &format!("{RULE}{EXAMPLES}"));
    let out = d.join("out");
    let (code, said, e) = run(&["gen", &p, "--out", out.to_str().unwrap()]);
    assert_eq!(code, 0, "{said}{e}");

    let (_, said, _) = run(&["test", out.to_str().unwrap(), "--format", "json"]);
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
    let _ = std::fs::remove_dir_all(&d);
}

/// SQL gets no walk at all, and a count is a walk.
#[test]
fn sqlには生成しない() {
    let d = dir("sql");
    let p = write(&d, "r.rule", RULE);
    let out = d.join("out");
    let (code, said, e) = run(&["gen", &p, "--out", out.to_str().unwrap()]);
    assert_eq!(code, 0, "{said}{e}");
    assert!(!out.join("sql").exists(), "SQL を書いてしまった");
    assert!(said.contains("SQL"), "断ったことを言っていない:\n{said}");
    let _ = std::fs::remove_dir_all(&d);
}
