//! 等価検証（§10）。M2 の受け入れ。
//!
//! 旧実装の代わりに「生成 Python を呼ぶだけ」のアダプタを立てる。完全に一致するのが
//! 正しい姿で、そこにわざと欠陥を入れると、rulec が件数・クラスタ・証人を出す。

use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
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

BUG = os.environ.get("BUG") == "1"
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    req = json.loads(line)
    d = req["in"]
    got = legacy.yupack_fee(legacy.Prefecture(d["あて先"]), int(d["三辺合計"]), int(d["重量"]))
    if BUG and d["あて先"] == "沖縄県":
        got += 10
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

fn verify(dir: &PathBuf, bug: bool) -> (i32, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rulec"));
    cmd.current_dir(root())
        .args(["verify", RULE, "--adapter", "python3"])
        .arg(dir.join("adapter.py"));
    if bug {
        cmd.env("BUG", "1");
    }
    let o = cmd.output().expect("rulec を起動できない");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned())
}

#[test]
fn 忠実なアダプタとは完全に一致する() {
    let Some(dir) = setup() else { return };
    let (code, out) = verify(&dir, false);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("一致 60 (100.000%)"), "{out}");
    assert!(out.contains("不一致はありません"), "{out}");
    assert!(out.contains("legacy@fake-1"), "旧実装の識別子を出す: {out}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn 旧実装の欠陥は件数と証人つきで出る() {
    let Some(dir) = setup() else { return };
    let (code, out) = verify(&dir, true);
    assert_eq!(code, 1, "不一致があれば 1 で終わる: {out}");
    assert!(out.contains("一致 53"), "{out}");
    assert!(out.contains("不一致 7 件"), "{out}");
    // §10.4: 発火行でクラスタし、件数・金額・証人を出す。
    assert!(out.contains("表 運賃表 行36"), "発火行でクラスタする: {out}");
    assert!(out.contains("差 -10"), "金額の差を出す: {out}");
    assert!(out.contains("沖縄県"), "証人を出す: {out}");
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
