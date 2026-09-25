//! Equivalence verification (§10). Acceptance for M2.
//!
//! In place of a legacy implementation, stand up an adapter that "just calls the generated Python".
//! Complete agreement is the correct picture; when a defect is deliberately put in, rulec reports
//! counts, clusters, and witnesses.

use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Pick the number out of `heading 123` in the output. Counts move with the generator's
/// circumstances, so the tests check the consistency of the breakdown rather than absolute values.
fn n_of(out: &str, head: &str) -> usize {
    out.split(head)
        .nth(1)
        .and_then(|r| r.split(|c: char| !c.is_ascii_digit()).find(|s| !s.is_empty()))
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}

fn have(cmd: &str) -> bool {
    ["--version", "version"]
        .iter()
        .any(|a| Command::new(cmd).arg(a).output().map(|o| o.status.success()).unwrap_or(false))
}

const RULE: &str = "tests/corpus/ゆうパック運賃.rule";

const ADAPTER: &str = r#"# 旧実装のふりをする。中身は生成 Python なので、素なら完全に一致する。
import json, sys, os
sys.path.insert(0, os.path.join(os.path.dirname(__file__), "python"))
import yupack_fee as legacy

sys.stdin.readline()
print(json.dumps({"ok": True, "impl": "legacy@fake-1"}), flush=True)

BUG = os.environ.get("BUG", "0")
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    req = json.loads(line)
    d = req["in"]
    got = legacy.yupack_fee(legacy.Prefecture(d["あて先"]), int(d["三辺合計"]), int(d["重量"]))
    if BUG == "1" and d["あて先"] == "沖縄県":
        got += 10
    if BUG == "2" and d["あて先"] == "沖縄県":
        # 旧実装が 1円 の細かさを持っていた場合。ずれは出力の刻み 10円 より必ず小さい。
        got -= 3 + int(d["重量"]) % 5
    if os.environ.get("BROKEN") == "1":
        print("運賃は " + str(got) + " 円", flush=True)
        continue
    # ESCAPE=1 writes the way json.dumps does unless told otherwise: every key and value in \u.
    print(json.dumps({"id": req["id"], "out": {"運賃": got}}, ensure_ascii=os.environ.get("ESCAPE") == "1"), flush=True)
"#;

/// The generated module and the adapter beside it, in a directory of this test's own.
///
/// It used to be named after the process, and the four tests here share one — so they raced:
/// `remove_dir_all` at the start of one ran while another was reading `adapter.py`, and
/// `verify` exited 2 ("cannot read") instead of 1. It stayed green on a laptop for as long as
/// the timing held, and failed the first time CI ran the suite (DESIGN §15.94).
fn setup(tag: &str) -> Option<PathBuf> {
    if !have("python3") {
        eprintln!("注意: python3 が無いので等価検証を飛ばした");
        return None;
    }
    let dir = std::env::temp_dir().join(format!("rulec-verify-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.to_string_lossy().to_string();
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(root())
        .args(["gen", RULE, "--out", &out])
        .output()
        .unwrap();
    assert!(o.status.success());
    std::fs::write(dir.join("adapter.py"), ADAPTER).unwrap();
    Some(dir)
}

fn verify(dir: &PathBuf, bug: &str) -> (i32, String) {
    verify_env(dir, &[("BUG", bug)]).0
}

/// `verify` with the adapter's environment set, and what it wrote to stderr as well.
fn verify_env(dir: &PathBuf, env: &[(&str, &str)]) -> ((i32, String), String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rulec"));
    cmd.current_dir(root())
        .args(["verify", RULE, "--lang", "ja", "--adapter", "python3"])
        .arg(dir.join("adapter.py"));
    for (k, v) in env {
        cmd.env(k, v);
    }
    let o = cmd.output().expect("rulec を起動できない");
    (
        (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned()),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

#[test]
fn 忠実なアダプタとは完全に一致する() {
    let Some(dir) = setup("忠実なアダプタとは完全に一致する") else { return };
    let (code, out) = verify(&dir, "0");
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("(100.000%)"), "{out}");
    assert!(out.contains("不一致はありません"), "{out}");
    // The count itself moves with the generator, so it is not pinned. Only check that it is not
    // empty.
    assert!(n_of(&out, "照合 ") > 50, "ベクタが少なすぎる: {out}");
    assert!(out.contains("legacy@fake-1"), "旧実装の識別子を出す: {out}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn 旧実装の欠陥は件数と証人つきで出る() {
    let Some(dir) = setup("旧実装の欠陥は件数と証人つきで出る") else { return };
    let (code, out) = verify(&dir, "1");
    assert_eq!(code, 1, "不一致があれば 1 で終わる: {out}");
    let total = n_of(&out, "照合 ");
    let agreed = n_of(&out, "一致 ");
    let bad = n_of(&out, "影響 ");
    assert!(bad > 0 && agreed + bad == total, "内訳が合わない: {out}");
    // §10.4: cluster by firing row, and report counts, amounts, and witnesses.
    assert!(out.contains("表 運賃表 行36"), "発火行でクラスタする: {out}");
    // The amount is the cluster total, so it moves with the count. Check the sign, and the
    // difference of the witness's single record.
    assert!(out.contains("差 -"), "金額の差を出す: {out}");
    assert!(out.contains("運賃=1450 / 現行 運賃=1460"), "証人を出す: {out}");
    // A deviation of exactly one grid step is a value mismatch, not rounding. Do not tag it.
    assert!(!out.contains("丸め方の違い"), "刻みちょうどの差を丸めのせいにしている: {out}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// §10.4: a cluster made up solely of deviations smaller than the output grid gets tagged
/// "suspected rounding difference".
#[test]
fn 刻み未満のずれは丸め方の違いとして括られる() {
    let Some(dir) = setup("刻み未満のずれは丸め方の違いとして括られる") else { return };
    let (code, out) = verify(&dir, "2");
    assert_eq!(code, 1, "{out}");
    assert!(n_of(&out, "影響 ") > 0, "{out}");
    let tagged = out.matches("丸め方の違いの疑い").count();
    let clusters = out.lines().filter(|l| l.starts_with("  表 ")).count();
    assert!(clusters > 0, "クラスタが出ていない: {out}");
    assert_eq!(tagged, clusters, "全クラスタが刻み未満なので全部に付くはず: {out}");
    assert!(out.contains("出力の刻み 10円 未満"), "刻みを書いて根拠を示す: {out}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn 雛形とスキーマが出る() {
    let run = |args: &[&str]| -> String {
        let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
            .current_dir(root())
            .args(args)
            .output()
            .unwrap();
        assert!(o.status.success(), "{args:?}: {}", String::from_utf8_lossy(&o.stderr));
        String::from_utf8_lossy(&o.stdout).into_owned()
    };
    // The template must work as is (return the handshake, answer line by line).
    let py = run(&["adapter", RULE, "--template", "python"]);
    assert!(py.contains("\"ok\": True"), "握手を返す: {py}");
    assert!(py.contains("あて先"), "入力の名前を案内する: {py}");
    let go = run(&["adapter", RULE, "--template", "go"]);
    assert!(go.contains("package main") && go.contains("bufio"), "{go}");

    // The schema is the wire shape (canonical names, integers in canonical units).
    let sc = run(&["schema", RULE]);
    assert!(sc.contains("\"整数。単位は cm\""), "単位を書く: {sc}");
    assert!(sc.contains("\"minimum\":1"), "範囲を書く: {sc}");
    assert!(sc.contains("\"北海道\""), "列挙を書く: {sc}");
}

/// An answer is read as the JSON it is (§15.151). It used to be searched as text for `"運賃":`,
/// and a key written `"\u904b\u8cc3"` — Python's `json.dumps` unless told otherwise, and PHP's
/// `json_encode` — was never found: an adapter that answered every case right came out at 0%.
#[test]
fn エスケープしたjsonで答えるアダプタとも一致する() {
    let Some(dir) = setup("エスケープしたjsonで答えるアダプタとも一致する") else { return };
    let ((code, out), err) = verify_env(&dir, &[("BUG", "0"), ("ESCAPE", "1")]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("(100.000%)"), "{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A line that is not JSON stops the run and says which record, rather than being read as an
/// answer with no values in it.
#[test]
fn jsonでない答えは止まって何件目かを言う() {
    let Some(dir) = setup("jsonでない答えは止まって何件目かを言う") else { return };
    let ((code, out), err) = verify_env(&dir, &[("BUG", "0"), ("BROKEN", "1")]);
    assert_eq!(code, 2, "{out}{err}");
    assert!(err.contains("0 件目の答え") && err.contains("運賃は"), "{err}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// An optional output with no value is `null` on the wire, which is how the vectors write it;
/// it was compared as the text `null` against `none`, and never matched.
#[test]
fn 任意の出力のnullはnoneとして比べる() {
    if !have("python3") {
        eprintln!("注意: python3 が無いので等価検証を飛ばした");
        return;
    }
    let dir = std::env::temp_dir().join(format!("rulec-verify-{}-optional", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let rule = "\
rule 任意(opt) v1

enum 区分(kind) = A(a) | B(b)

inputs
  区分(kind) : 区分
  希望(pref) : 区分?

outputs
  割当(slot) : 区分?

table 割当表(slots)
policy unique
| 区分 | -> 割当 |
| A    | 希望    |
| B    | B       |
";
    let adapter = r#"import json, sys
sys.stdin.readline()
print(json.dumps({"ok": True, "impl": "optional"}), flush=True)
for line in sys.stdin:
    r = json.loads(line)
    d = r["in"]
    got = d["希望"] if d["区分"] == "A" else "B"
    print(json.dumps({"id": r["id"], "out": {"割当": got}}), flush=True)
"#;
    std::fs::write(dir.join("opt.rule"), rule).unwrap();
    std::fs::write(dir.join("adapter.py"), adapter).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(&dir)
        .args(["verify", "opt.rule", "--lang", "ja", "--adapter", "python3", "adapter.py"])
        .output()
        .unwrap();
    let out = String::from_utf8_lossy(&o.stdout).into_owned();
    assert_eq!(o.status.code(), Some(0), "{out}{}", String::from_utf8_lossy(&o.stderr));
    assert!(out.contains("(100.000%)"), "{out}");
    let _ = std::fs::remove_dir_all(&dir);
}
