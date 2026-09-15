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
| >=13%  | false              |

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
    // Written back into a cell, so it is percent, and on the declared step: one step past
    // 10% is 11%, not the stored 0.11 and not the midpoint 11.5%.
    assert!(text.contains("当てはまらない例: 割引率 = 11%"), "証人が率になっていない:\n{text}");
    assert!(text.contains("`| 11% | true |`"), "書き戻せる行になっていない:\n{text}");

    let (_, js) = run(&["check", &p, "--format", "json"]);
    let first = js.lines().find(|l| l.contains("E101")).expect("E101 が無い");
    // In the witness it is the wire form, so it is a count of steps: 20 steps of 1%.
    assert!(first.contains("\"witness\":{\"inputs\":{\"割引率\":11}}"), "証人の JSON がワイヤの単位でない:\n{first}");
}

#[test]
fn 証人の行を貼ると穴が閉じる() {
    // The point of `fix.text`: pasting it has to actually remove that witness. With the
    // wrong unit it pastes `0.11%`, which closes nothing and the same witness comes back.
    let mut text = HOLE.to_string();
    let mut seen = Vec::new();
    for _ in 0..6 {
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
    panic!("6 回貼っても穴が閉じない: {seen:?}");
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

/// A rule whose rate moves in tenths of a percent, which is what §2.1 asks for and what the
/// number scanner used to refuse outright.
const FINE: &str = "\
rule 細かい率(fine_rate) v1
description \"刻みが 1% より細かい率\"

inputs
  取引額(amount) : money[円, incl_tax] range >=0円 <=100万円
  手数料率(rate) : rate[step 0.1%]     range >=0% <=10%

outputs
  手数料(fee) : money[円, incl_tax] round up(1円)

define 率手数料(rate_fee) : money[円, incl_tax] = 取引額 × 手数料率

table 手数料表(fee_table)
policy unique
| 手数料率 | -> 手数料(fee) : money[円, incl_tax] |
| <=0.5%   | 0円                                  |
| >0.5%    | 率手数料                             |

result 手数料 = 手数料
";

#[test]
fn 一パーセントより細かい刻みが書ける() {
    let p = write_tmp("fine", FINE);
    let p = p.to_string_lossy().to_string();
    let (c, out) = run(&["check", &p]);
    assert_eq!(c, 0, "{out}");

    // 0.5% at a step of 0.1% is 5 steps, and the whole range is 100 of them.
    let (_, sc) = run(&["schema", &p]);
    assert!(sc.contains("\"手数料率\":{\"type\":\"integer\",\"description\":\"単位: 率（刻み単位の整数）\",\"minimum\":0,\"maximum\":100}"), "{sc}");

    // The vectors have to step on both sides of 0.5%, which they cannot do if every rate
    // under 100% collapses to the same value.
    let (c, cov) = run(&["coverage", &p]);
    assert_eq!(c, 0, "カバーが満たせていない:\n{cov}");
    let (_, vs) = run(&["vectors", &p]);
    for want in ["\"手数料率\":5", "\"手数料率\":6"] {
        assert!(vs.contains(want), "{want} を踏むベクタが無い:\n{vs}");
    }
}

#[test]
fn 小数は正確な有理数として読まれる() {
    // `3.6%` at a step of 0.1% is 36 steps. Truncating the literal to 3, or reading `.6` as
    // six tenths of a percent of something else, both land elsewhere.
    let rule = FINE.replace("<=0.5%", "<=3.6%").replace(">0.5%", ">3.6%");
    let p = write_tmp("exact", &rule);
    let p = p.to_string_lossy().to_string();
    let (c, out) = run(&["check", &p]);
    assert_eq!(c, 0, "{out}");

    let dir = std::env::temp_dir().join(format!("rulec-wire-exact-gen-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let o = dir.to_string_lossy().to_string();
    let (c, msg) = run(&["gen", &p, "--out", &o]);
    assert_eq!(c, 0, "{msg}");
    let py = std::fs::read_to_string(dir.join("python").join("fine_rate.py")).unwrap();
    assert!(py.contains("rate <= 36"), "3.6% が 36 刻みになっていない:\n{py}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn 刻みに載らない値は止まる() {
    // `0.05%` is half a step of 0.1%, so the runtime integer cannot hold it. Moving it to
    // the nearest step quietly would put a different boundary in the code than on the page.
    let rule = FINE.replace("<=0.5%", "<=0.05%").replace(">0.5%", ">0.05%");
    let p = write_tmp("offstep", &rule);
    let p = p.to_string_lossy().to_string();
    let (c, out) = run(&["check", &p]);
    assert_eq!(c, 1, "刻みの間の値が通ってしまう:\n{out}");
    assert!(out.contains("E114"), "{out}");
    assert!(out.contains("0.1%"), "直し方が刻みを名指ししていない:\n{out}");
}

#[test]
fn 出力のセルに式を書くと止まる() {
    // §3.2 allows one name or one value. The first word used to be taken and the rest
    // dropped, so `取引額 × 手数料率` generated code that never multiplied.
    let rule = FINE.replace("| >0.5%    | 率手数料                             |", "| >0.5%    | 取引額 × 手数料率 |");
    let p = write_tmp("expr", &rule);
    let p = p.to_string_lossy().to_string();
    let (c, out) = run(&["check", &p]);
    assert_eq!(c, 1, "式のセルが通ってしまう:\n{out}");
    assert!(out.contains("E014"), "{out}");
}

/// A count, a number of days, a score: numbers with no unit at all (§2.1). The only way a
/// unit disappears is dividing money by money, which is the "one point per 100 yen" case
/// §2.3 was written for and which had no type to land in until now.
const COUNT: &str = "\
rule 付与点数(pts_only) v1
description \"100 円につき 1 点\"

inputs
  税込金額(paid) : money[円, incl_tax] range >=0円 <=100万円

outputs
  点数(pts) : number round down(1)

define 基本点(base) : number = 税込金額 ÷ 100円

result 点数 = 基本点
";

#[test]
fn 単位のない数が書けて割り算が単位を消す() {
    let p = write_tmp("count", COUNT);
    let p = p.to_string_lossy().to_string();
    let (c, out) = run(&["check", &p]);
    assert_eq!(c, 0, "{out}");

    let dir = std::env::temp_dir().join(format!("rulec-wire-count-gen-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let o = dir.to_string_lossy().to_string();
    let (c, msg) = run(&["gen", &p, "--out", &o]);
    assert_eq!(c, 0, "{msg}");
    let py = std::fs::read_to_string(dir.join("python").join("pts_only.py")).unwrap();
    // A number is a plain int on both sides: branding it would shadow the language's own
    // `int` and protect nothing, since two counts of different things share the brand.
    assert!(py.contains("-> int:"), "点数 が素の int で返っていない:\n{py}");
    assert!(!py.contains("NewType(\"int\""), "int を NewType で覆っている:\n{py}");
    // Dividing by 100 does not divide the integer: it multiplies the scale, so 1050円
    // stays 1050 and the single rounding at the end turns it into 10 points.
    assert!(py.contains("base = paid"), "除算が整数を割ってしまっている:\n{py}");
    assert!(py.contains("// 100"), "スケールが戻されていない:\n{py}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn 単位のある数とない数は混ざらない() {
    // `number` is dimensionless, so it multiplies money — but it is not money, and putting
    // one where the other is expected has to stop.
    let bad = COUNT.replace("点数(pts) : number round down(1)", "点数(pts) : money[円, incl_tax] round down(1円)");
    let p = write_tmp("mix", &bad);
    let p = p.to_string_lossy().to_string();
    let (c, out) = run(&["check", &p]);
    assert_eq!(c, 1, "単位のない数が金額の位置に通ってしまう:\n{out}");
    assert!(out.contains("E103"), "{out}");
}

/// A rate written as a literal inside a `define`. No corpus rule has this shape: every rate
/// in the corpus is a column, so it reaches the generator as a *name*, and a name carries
/// whatever scale `types` recorded for it.
///
/// A literal used to be different. `3.6%` reduces to 9/250, and the generator stored the
/// reduced denominator — while `types` recorded 1000 for the name, the power of ten the
/// decimal asks for times the hundredth `%` stands for. So the value was written at one
/// scale and read back at another: a silent factor of four. Neither net that usually holds
/// catches it. `check` passes, because the reference evaluator is a separate path; and the
/// six languages agree with each other, because they share the generator's text. Only the
/// comparison against the evaluator says anything, and only if someone runs it.
///
/// It bites whenever the literal reduces at all — every rate sharing a factor with 100, so
/// `8%` and `50%` as much as `3.6%`. `7%` was fine, which is why a spot check could miss it.
const RATE_IN_DEFINE: &str = "\
rule 率の定義(rate_def) v1
description \"define の本体に率のリテラルを書く\"

inputs
  元金(src) : money[円, incl_tax] range >=0円 <=100万円

outputs
  結果(out) : money[円, incl_tax] round half_up(1円)

define 割合分(part) : money[円, incl_tax] = 元金 × 3.6%

result 結果 = 割合分

examples
| 元金    | -> 結果 |
| 10000円 | 360円   |
";

#[test]
fn defineの中の率のリテラルは名前と同じ尺度で保管される() {
    let p = write_tmp("ratedef", RATE_IN_DEFINE);
    let p = p.to_string_lossy().to_string();
    let (c, out) = run(&["check", &p]);
    assert_eq!(c, 0, "{out}");

    let dir = std::env::temp_dir().join(format!("rulec-wire-ratedef-gen-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let o = dir.to_string_lossy().to_string();
    let (c, msg) = run(&["gen", &p, "--out", &o]);
    assert_eq!(c, 0, "{msg}");
    let py = std::fs::read_to_string(dir.join("python").join("rate_def.py")).unwrap();
    // 36/1000, not the reduced 9/250 — because 1000 is what every later read assumes.
    assert!(py.contains("part = src * 36"), "率のリテラルが約分された尺度で保管されている:\n{py}");
    assert!(py.contains("1/1000 money"), "定義の単位が名前の尺度と食い違っている:\n{py}");
    assert!(py.contains("_round_half(raw, 1000) // 1000"), "尺度が戻されていない:\n{py}");
    // The example says 10000円 → 360円, and the evaluator agrees; the point of the test is
    // that the generated code agrees too. That is what the factor of four broke.
    let want = std::fs::read_to_string(dir.join("vectors").join("rate_def.expected.jsonl")).unwrap();
    assert!(want.contains("{\"結果\":360}"), "参照評価器の期待値が変わっている:\n{want}");
    let _ = std::fs::remove_dir_all(&dir);
}
