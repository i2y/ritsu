//! The worked example in docs/backends.md (DESIGN §15.19).
//!
//! That page tells a reader they can target a language rulec does not generate, keep the
//! evidence, and change nothing in this repository. The whole claim rests on one loop —
//! inventory, emit, adapt, `rulec verify` — so the page shows it run against SQL, a target
//! rulec has no backend for.
//!
//! A page showing something nobody has run would be worse than no page. These tests take the
//! SQL and the adapter out of the document itself, run them, and hold the two console blocks
//! to what really comes back: that the target agrees on every case, and that breaking one
//! threshold is caught with the case that shows it. If the wire changes, if a rate stops
//! travelling as a count of steps, if the vector suite grows, the document fails here.

use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn have(cmd: &str) -> bool {
    Command::new(cmd).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

fn doc() -> String {
    std::fs::read_to_string(root().join("docs/backends.md")).expect("docs/backends.md が読めない")
}

/// The first fenced block of `lang` in the document.
fn fence(body: &str, lang: &str) -> String {
    let open = format!("```{lang}\n");
    let start = body.find(&open).unwrap_or_else(|| panic!("```{lang} の囲みが無い"));
    let rest = &body[start + open.len()..];
    let end = rest.find("\n```").unwrap_or_else(|| panic!("```{lang} が閉じていない"));
    rest[..end].to_string()
}

/// Write the document's SQL and adapter into `dir`, optionally with the row-7 threshold
/// broken the way the second console block describes.
fn lay_out(dir: &std::path::Path, broken: bool) {
    let body = doc();
    let mut sql = fence(&body, "sql");
    if broken {
        // Article 7(2)(c) allows four hours; the document's mistake is three.
        let before = sql.clone();
        sql = sql.replace("AND delay <= 4 THEN 1", "AND delay <= 3 THEN 1");
        sql = sql.replace("AND delay >  4 THEN 2", "AND delay >  3 THEN 2");
        assert_ne!(sql, before, "壊す対象の行が文書から消えています");
    }
    std::fs::write(dir.join("ec261.sql"), sql).unwrap();
    std::fs::write(dir.join("adapter.py"), fence(&body, "python")).unwrap();
}

fn verify(dir: &std::path::Path) -> (i32, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(root())
        .env("RULEC_LANG", "en")
        .args([
            "verify",
            "tests/corpus/ec261.rule",
            "--adapter",
            "python3",
            dir.join("adapter.py").to_str().unwrap(),
        ])
        .output()
        .expect("rulec を起動できない");
    (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).into_owned())
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rulec-backends-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn 文書のクエリはec261と全件一致する() {
    if !have("python3") {
        eprintln!("skip: python3 が無い（sqlite3 は標準ライブラリなので他に要るものは無い）");
        return;
    }
    let dir = tmp("ok");
    lay_out(&dir, false);
    let (code, out) = verify(&dir);
    assert_eq!(code, 0, "一致しているなら終了コードは 0:\n{out}");
    // The document prints these three lines. The count is the vector suite's own size, so a
    // suite that grows has to be reflected on the page.
    assert!(out.contains("Compared 88 / matched 88 (100.000%)"), "文書の 88/88 と違う:\n{out}");
    assert!(out.contains("Counterpart: sqlite3/ec261.sql"), "impl 名が文書と違う:\n{out}");
    assert!(out.contains("No mismatches."), "不一致が出ている:\n{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn 閾値を一つ壊すとその入力ごと報告される() {
    if !have("python3") {
        eprintln!("skip: python3 が無い");
        return;
    }
    let dir = tmp("broken");
    lay_out(&dir, true);
    let (code, out) = verify(&dir);
    assert_eq!(code, 1, "不一致があるなら終了コードは 1:\n{out}");
    assert!(out.contains("Compared 88 / matched 87 (98.864%)"), "文書の 87/88 と違う:\n{out}");
    // The point of the section: the report names the rows and carries a case that shows it.
    for want in [
        "table band_of row 4",
        "table amount row 3",
        "table reduction row 7",
        "delay=4, distance=3501, intra_eu=false",
        "rule compensation=300 / legacy compensation=600",
    ] {
        assert!(out.contains(want), "文書が見せている「{want}」が出ていない:\n{out}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
