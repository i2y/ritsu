//! `fold` (§15.56): the verdicts of a per-element table, reduced to one answer.
//!
//! The walk is checkable because a complete and unique table puts every element on exactly
//! one verdict of a finite enum, so what the fold has to answer for is a string over a finite
//! alphabet. What is tested here is the four holes that string can have — and that the
//! table's own proof is untouched by any of it.

use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rulec-fold-{tag}-{}", std::process::id()));
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

/// A tariff sheet walked row by row: skip, halt, take, or hold the best so far.
const RULE: &str = r#"rule 全国運賃(freight) v1
description "運賃行を順に見て、一つの運賃に畳む"

enum 採用区分(verdict) = スキップ(skip) | 打ち切り(halt) | 確定(take) | 持ち越し(hold)
enum ゾーン区分(zone) = 近畿圏(kinki) | 遠隔地(remote)

elements 運賃行(freight_rows)
  行ゾーン(row_zone) : ゾーン区分
  閾値(threshold)    : money[円, incl_tax]  range >=0円 <=100万円
  行運賃(row_fee)    : money[円, incl_tax]  range >=0円 <=10万円

outputs
  運賃(fee) : money[円, incl_tax]  round up(1円)

table 行判定(row_of)
policy unique
| 行ゾーン | 閾値     | -> 採用(verdict) : 採用区分 |
| 近畿圏   | <=1000円 | 確定                        |
| 近畿圏   | >1000円  | 持ち越し                    |
| 遠隔地   | <=1000円 | スキップ                    |
| 遠隔地   | >1000円  | 打ち切り                    |

fold 採用 over 運賃行
  スキップ  -> next
  打ち切り  -> stop with 0円
  確定      -> take_unique 行運賃
  持ち越し  -> keep_max 行運賃 by 閾値
  empty     -> 0円
  exhausted -> held
"#;

fn write(d: &PathBuf, name: &str, body: &str) -> String {
    let p = d.join(name);
    std::fs::write(&p, body).unwrap();
    p.to_str().unwrap().to_string()
}

fn codes(out: &str) -> Vec<String> {
    out.lines()
        .filter_map(|l| rulec::json::parse(l).ok())
        .filter_map(|j| j.get("code").and_then(|c| c.as_str()).map(|s| s.to_string()))
        .collect()
}

#[test]
fn 要素ごとの表はいままでどおり検査される() {
    let d = dir("ok");
    let p = write(&d, "r.rule", RULE);
    let (code, out, e) = run(&["check", &p]);
    assert_eq!(code, 0, "{out}{e}");

    // The element's own table is proved the way any table is: take a row away and the gap
    // comes back with the element that falls through it.
    let holed = RULE.replace("| 遠隔地   | >1000円  | 打ち切り                    |\n", "");
    let p = write(&d, "holed.rule", &holed);
    let (code, out, _) = run(&["check", &p, "--format", "json"]);
    assert_eq!(code, 1, "{out}");
    assert!(codes(&out).contains(&"E101".to_string()), "{out}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn 空と終端の答えは宣言が要る() {
    let d = dir("answers");
    for (name, cut, want) in [
        ("empty", "  empty     -> 0円\n", "E022"),
        ("exhausted", "  exhausted -> held\n", "E023"),
    ] {
        let p = write(&d, &format!("{name}.rule"), &RULE.replace(cut, ""));
        let (code, out, _) = run(&["check", &p, "--format", "json"]);
        assert_eq!(code, 1, "{name}: {out}");
        assert!(codes(&out).contains(&want.to_string()), "{name}: {out}");
    }
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn 腕の無い判定は穴で_届かない腕は注意() {
    let d = dir("arms");
    // A verdict the table produces, with nothing to do about it.
    let p = write(&d, "hole.rule", &RULE.replace("  持ち越し  -> keep_max 行運賃 by 閾値\n", ""));
    let (code, out, _) = run(&["check", &p, "--format", "json"]);
    assert_eq!(code, 1, "{out}");
    assert!(codes(&out).contains(&"E024".to_string()), "{out}");
    assert!(out.contains("持ち越し"), "どの判定か言っていない: {out}");

    // An arm for a verdict no row can reach.
    let unreachable = RULE
        .replace("| 近畿圏   | >1000円  | 持ち越し                    |", "| 近畿圏   | >1000円  | スキップ                    |");
    let p = write(&d, "unreachable.rule", &unreachable);
    let (_, out, _) = run(&["check", &p, "--format", "json"]);
    assert!(codes(&out).contains(&"W115".to_string()), "{out}");
    let _ = std::fs::remove_dir_all(&d);
}

/// The one the proposal called the biggest: a bare `take` cannot be written, so the author
/// chooses between "only one may match" and "the first wins" when the line is first typed.
#[test]
fn takeは一意か先頭かを書かせる() {
    let d = dir("take");
    let p = write(&d, "bare.rule", &RULE.replace("take_unique 行運賃", "take 行運賃"));
    let (code, out, _) = run(&["check", &p, "--format", "json"]);
    assert_eq!(code, 1, "{out}");
    assert!(codes(&out).contains(&"E021".to_string()), "{out}");
    assert!(out.contains("take_unique") && out.contains("take_first"), "選択肢を挙げていない: {out}");

    // Both spellings are accepted, and they are different rules.
    for arm in ["take_unique 行運賃", "take_first 行運賃"] {
        let p = write(&d, "arm.rule", &RULE.replace("take_unique 行運賃", arm));
        let (code, out, e) = run(&["check", &p]);
        assert_eq!(code, 0, "{arm}: {out}{e}");
    }
    let _ = std::fs::remove_dir_all(&d);
}

/// What is not built yet is refused by name, not by silence (§15.56).
#[test]
fn 生成はまだできないと名指しで断る() {
    let d = dir("gen");
    let p = write(&d, "r.rule", RULE);
    let out = d.join("out");
    let (code, _, e) = run(&["gen", &p, "--out", out.to_str().unwrap()]);
    assert_eq!(code, 2, "{e}");
    assert!(e.contains("fold") || e.contains("畳み込み"), "{e}");
    assert!(!out.exists(), "断ったのに書き出している");

    let p = write(&d, "ex.rule", &format!("{RULE}\nexamples\n| 行ゾーン | 閾値 | 行運賃 | -> 運賃 |\n| 近畿圏 | 500円 | 800円 | 800円 |\n"));
    let (code, out, _) = run(&["check", &p, "--format", "json"]);
    assert_eq!(code, 1, "{out}");
    assert!(codes(&out).contains(&"E025".to_string()), "{out}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn 列は一つで_二本目は断る() {
    let d = dir("two");
    let p = write(&d, "two.rule", &RULE.replace("outputs\n", "elements 別の列(others)\n  m(m) : number  range >=0 <=9\n\noutputs\n"));
    let (code, out, _) = run(&["check", &p, "--format", "json"]);
    assert_eq!(code, 1, "{out}");
    assert!(codes(&out).contains(&"E020".to_string()), "{out}");
    let _ = std::fs::remove_dir_all(&d);
}
