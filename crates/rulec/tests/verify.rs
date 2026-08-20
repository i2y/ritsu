//! 等価検証（§10）。M2 の受け入れ。
//!
//! 旧実装の代わりに「生成 Python を呼ぶだけ」のアダプタを立てる。完全に一致するのが
//! 正しい姿で、そこにわざと欠陥を入れると、rulec が件数・クラスタ・証人を出す。

use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// 出力から `見出し 123` の数を拾う。件数は生成器の都合で動くので、
/// テストは絶対値ではなく内訳の整合を見る。
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
        # 旧実装が 1円 の精度を持っていた場合。ずれは出力格子 10円 より必ず小さい。
        got -= 3 + int(d["重量"]) % 5
    print(json.dumps({"id": req["id"], "out": {"運賃": got}}, ensure_ascii=False), flush=True)
"#;

fn setup() -> Option<PathBuf> {
    if !have("python3") {
        eprintln!("注意: python3 が無いので等価検証を飛ばした");
        return None;
    }
    let dir = std::env::temp_dir().join(format!("rulec-verify-{}", std::process::id()));
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
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rulec"));
    cmd.current_dir(root())
        .args(["verify", RULE, "--adapter", "python3"])
        .arg(dir.join("adapter.py"));
    cmd.env("BUG", bug);
    let o = cmd.output().expect("rulec を起動できない");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned())
}

#[test]
fn 忠実なアダプタとは完全に一致する() {
    let Some(dir) = setup() else { return };
    let (code, out) = verify(&dir, "0");
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("(100.000%)"), "{out}");
    assert!(out.contains("不一致はありません"), "{out}");
    // 件数そのものは生成器の都合で動くので刻まない。空でないことだけ見る。
    assert!(n_of(&out, "照合 ") > 50, "ベクタが少なすぎる: {out}");
    assert!(out.contains("legacy@fake-1"), "旧実装の識別子を出す: {out}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn 旧実装の欠陥は件数と証人つきで出る() {
    let Some(dir) = setup() else { return };
    let (code, out) = verify(&dir, "1");
    assert_eq!(code, 1, "不一致があれば 1 で終わる: {out}");
    let total = n_of(&out, "照合 ");
    let agreed = n_of(&out, "一致 ");
    let bad = n_of(&out, "影響 ");
    assert!(bad > 0 && agreed + bad == total, "内訳が合わない: {out}");
    // §10.4: 発火行でクラスタし、件数・金額・証人を出す。
    assert!(out.contains("表 運賃表 行36"), "発火行でクラスタする: {out}");
    // 金額はクラスタの合計なので件数で動く。符号と、証人の一件分の差を見る。
    assert!(out.contains("差 -"), "金額の差を出す: {out}");
    assert!(out.contains("運賃=1450 / 旧 運賃=1460"), "証人を出す: {out}");
    // ちょうど格子一つ分のずれは値の食い違いであって丸めではない。括ってはいけない。
    assert!(!out.contains("丸め差異"), "格子ちょうどの差を丸めのせいにしている: {out}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// §10.4: 出力格子より小さいずれだけで固まったクラスタには「丸め差異の疑い」が付く。
#[test]
fn 格子未満のずれは丸め差異として括られる() {
    let Some(dir) = setup() else { return };
    let (code, out) = verify(&dir, "2");
    assert_eq!(code, 1, "{out}");
    assert!(n_of(&out, "影響 ") > 0, "{out}");
    let tagged = out.matches("丸め差異の疑い").count();
    let clusters = out.lines().filter(|l| l.starts_with("  表 ")).count();
    assert!(clusters > 0, "クラスタが出ていない: {out}");
    assert_eq!(tagged, clusters, "全クラスタが格子未満なので全部に付くはず: {out}");
    assert!(out.contains("出力格子 10円 未満"), "格子を書いて根拠を示す: {out}");
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
    // 雛形はそのまま動く形であること（握手を返し、行ごとに答える）。
    let py = run(&["adapter", RULE, "--template", "python"]);
    assert!(py.contains("\"ok\": True"), "握手を返す: {py}");
    assert!(py.contains("あて先"), "入力の名前を案内する: {py}");
    let go = run(&["adapter", RULE, "--template", "go"]);
    assert!(go.contains("package main") && go.contains("bufio"), "{go}");

    // スキーマはワイヤの形（正準名と正準単位の整数）。
    let sc = run(&["schema", RULE]);
    assert!(sc.contains("\"単位: cm\""), "単位を書く: {sc}");
    assert!(sc.contains("\"minimum\":1"), "範囲を書く: {sc}");
    assert!(sc.contains("\"北海道\""), "列挙を書く: {sc}");
}
