//! The library rules (§15.40): transcriptions of published tables, held to the tables.
//!
//! Nothing inside rulec can say whether a transcription matches its source — that is the
//! first thing the README lists as not proved. What can be done is to transcribe the
//! source's own worked amounts a second time, as records, and replay the rule over them.
//! `tests/oracle/` carries the printed halves of two premium tables, one record per grade,
//! turned into the yen amounts the tables' own notes define; every record has to agree.

use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rulec(args: &[&str]) -> (i32, String, String) {
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

/// (rule, the records transcribed from its published table, how many records there are)
const ORACLES: &[(&str, &str, usize)] = &[
    ("tests/corpus/健康保険料.rule", "tests/oracle/健康保険料_東京_令和8年度.jsonl", 100),
    ("tests/corpus/厚生年金保険料.rule", "tests/oracle/厚生年金保険料_令和8年度.jsonl", 32),
];

/// Every grade of the printed table, both ways of settling the sen, agrees with the rule.
#[test]
fn 保険料額表の全等級と一致する() {
    for (rule, fx, records) in ORACLES {
        let (c, out, e) = rulec(&["fixtures", "lint", fx, rule, "--format", "json"]);
        assert_eq!(c, 0, "{rule}: {out}{e}");
        assert!(out.contains("\"problems\":[]"), "{rule}: 記録の形が規則と合わない: {out}");
        assert!(out.contains(&format!("\"records\":{records},")), "{rule}: 件数が違う: {out}");
        let (c, out, e) = rulec(&["replay", rule, "--fixtures", fx]);
        assert_eq!(c, 0, "{rule}: {out}{e}");
        assert!(out.contains("(100.000%)"), "{rule}: 印刷された表と食い違う:\n{out}");
    }
}

/// The committed records are what `tests/oracle/make.py` writes from the printed amounts.
/// A record edited by hand, or a table transcribed anew without regenerating, fails here.
#[test]
fn 記録は転記から作り直しても変わらない() {
    if !Command::new("python3").arg("--version").output().map(|o| o.status.success()).unwrap_or(false) {
        eprintln!("注意: python3 が無いので飛ばした");
        return;
    }
    let dir = std::env::temp_dir().join(format!("rulec-oracle-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(root().join("tests/oracle/make.py"), dir.join("make.py")).unwrap();
    let o = Command::new("python3").arg(dir.join("make.py")).output().expect("python3 を起動できない");
    assert!(
        o.status.success(),
        "make.py が失敗した。転記した数字が 標準報酬月額 × 料率 ÷ 2 と合っていない:\n{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    for (_, fx, _) in ORACLES {
        let name = std::path::Path::new(fx).file_name().unwrap();
        let committed = std::fs::read_to_string(root().join(fx)).unwrap();
        let fresh = std::fs::read_to_string(dir.join(name)).unwrap();
        assert_eq!(committed, fresh, "{fx}: `python3 tests/oracle/make.py` で作り直してください");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
