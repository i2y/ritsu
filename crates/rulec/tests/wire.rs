//! A rate travels as a count of steps (§10.2), and every place that crosses the boundary
//! says the same number.
//!
//! Money and quantities are integers in their own unit, so the conversion is the identity
//! and a mistake there is invisible. A rate is the only type whose stored value and wire
//! value differ, so it is the one that catches a place that forgot to convert. Each test
//! below is a different boundary: the JSON Schema, a witness in a diagnostic, the generated
//! entry guard, and a fixture read back in.

use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run(args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(root())
        .args(args)
        .output()
        .expect("rulec を起動できない");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned())
}

/// A rule whose only numeric input is a rate, with a hole in the middle of its range.
const HOLE: &str = "\
rule 率の穴(rate_hole) v1
description \"率の列に穴があり、証人が率になる\"

inputs
  割引率(rate) : rate[step 1%] range >=0% <=100%

outputs
  可否(ok) : bool

table 判定(judge)
policy first
| 割引率 | -> 可否(ok) : bool |
| <=10%  | true               |
| >=30%  | false              |

result 可否 = 可否
";

fn write_tmp(tag: &str, text: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rulec-wire-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("r.rule");
    std::fs::write(&p, text).unwrap();
    p
}

const COUPON: &str = "tests/corpus/クーポン一枚.rule";

#[test]
fn スキーマの上下限は刻みの個数で書かれる() {
    let (c, out) = run(&["schema", COUPON]);
    assert_eq!(c, 0, "{out}");
    // `range >=0% <=100%` with `step 1%` is 0..100 steps, not 0..1.
    assert!(
        out.contains("\"割引率\":{\"type\":\"integer\",\"description\":\"単位: 率（刻み単位の整数）\",\"minimum\":0,\"maximum\":100}"),
        "率の上下限がワイヤの単位になっていない:\n{out}"
    );
}

#[test]
fn 証人の率は書き戻せる形で出る() {
    let p = write_tmp("hole", HOLE);
    let p = p.to_string_lossy().to_string();
    let (_, text) = run(&["check", &p]);
    // Written back into a cell, so it is percent: 20%, not the stored 0.2.
    assert!(text.contains("当たらない例: 割引率 = 20%"), "証人が率になっていない:\n{text}");
    assert!(text.contains("`| 20% | true |`"), "書き戻せる行になっていない:\n{text}");

    let (_, js) = run(&["check", &p, "--format", "json"]);
    let first = js.lines().find(|l| l.contains("E101")).expect("E101 が無い");
    // In the witness it is the wire form, so it is a count of steps: 20 steps of 1%.
    assert!(first.contains("\"witness\":{\"inputs\":{\"割引率\":20}}"), "証人の JSON がワイヤの単位でない:\n{first}");
}

#[test]
fn 証人の行を貼ると穴が閉じる() {
    // The point of `fix.text`: pasting it has to actually remove that witness. With the
    // wrong unit it pastes `0.2%`, which closes nothing and the same witness comes back.
    let mut text = HOLE.to_string();
    let mut seen = Vec::new();
    for _ in 0..8 {
        let p = write_tmp("close", &text);
        let p = p.to_string_lossy().to_string();
        let (code, js) = run(&["check", &p, "--format", "json"]);
        if code == 0 {
            return;
        }
        let Some(line) = js.lines().find(|l| l.contains("E101")) else { return };
        let j = rulec::json::parse(line.trim()).expect("JSON として読めない");
        let row = j.get("fix").and_then(|f| f.get("text")).and_then(|t| t.as_str()).expect("fix.text が無い").to_string();
        assert!(!seen.contains(&row), "同じ行がまた出た。貼っても穴が閉じていない: {row}\n{seen:?}");
        seen.push(row.clone());
        // Insert the row above the `result` line.
        text = text.replace("\nresult ", &format!("{row}\n\nresult "));
    }
    panic!("8 回貼っても穴が閉じない: {seen:?}");
}

#[test]
fn 入口のガードは刻みの個数で比べる() {
    let dir = std::env::temp_dir().join(format!("rulec-wire-gen-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let out = dir.to_string_lossy().to_string();
    let (c, msg) = run(&["gen", COUPON, "--out", &out]);
    assert_eq!(c, 0, "{msg}");
    let py = std::fs::read_to_string(dir.join("python").join("coupon_step.py")).expect("Python が無い");
    let go =
        std::fs::read_to_string(dir.join("go").join("couponstep").join("coupon_step.go")).expect("Go が無い");
    // The generated code works in steps throughout, so the guard does too.
    assert!(py.contains("if not 0 <= rate <= 100:"), "Python のガードが刻みの個数でない:\n{py}");
    // The struct field comment has to agree with the guard, or a reader of the Go side is
    // told the field runs 0..1.
    assert!(go.contains("// 割引率 範囲 0..100"), "Go の欄の注記がガードと食い違う:\n{go}");
    assert!(go.contains("int64(in.Rate) < 0 || int64(in.Rate) > 100"), "Go のガードが刻みの個数でない:\n{go}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn 記録の率は刻みの個数として読まれる() {
    // `"割引率":10` is 10 steps, i.e. 10%. Read as the true value 10 it would be 1000%, far
    // outside `range <=100%`, and the record would be thrown out as malformed.
    let rec = "{\"in\":{\"商品合計\":10000,\"適用済割引\":0,\"種別\":\"率引き\",\
\"割引率\":10,\"額面\":0,\"同商品適用済\":false},\
\"observed\":{\"可否\":true,\"素割引\":1000}}\n";
    let dir = std::env::temp_dir().join(format!("rulec-wire-rec-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("one.jsonl");
    std::fs::write(&f, rec).unwrap();
    let f = f.to_string_lossy().to_string();

    let (c, out) = run(&["fixtures", "lint", &f, COUPON]);
    assert_eq!(c, 0, "10% の記録が形式検査を通らない:\n{out}");

    // 10% of 10000 is 1000, which is what the record says, so replay agrees.
    let (c, out) = run(&["replay", COUPON, "--fixtures", &f]);
    assert_eq!(c, 0, "{out}");
    assert!(out.contains("1 / 1") || out.contains("(100.000%)"), "10% が掛かっていない:\n{out}");
    let _ = std::fs::remove_dir_all(&dir);
}
