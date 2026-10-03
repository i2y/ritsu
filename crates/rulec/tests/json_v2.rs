//! Diagnostic JSON, version 2 (§11 principle 6).
//!
//! A diagnostic now says the same thing twice: in prose for a person, and as data for a
//! program. What is pinned here is the data half — and, above all, that **the data half does
//! not change with `--lang`**. A caller that reads `witness` and `fix` must get the same
//! bytes whichever language the prose is rendered in, or the structure is just prose again.
//!
//! Re-bake with `RULEC_BLESS=1 cargo test --test json_v2`.

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

/// The JSON lines for one code of one file, in one language.
fn lines_for(file: &str, code: &str, lang: &str) -> Vec<String> {
    let (_, out) = run(&["check", file, "--format", "json", "--show-shadow", "--lang", lang]);
    out.lines()
        .filter(|l| l.contains(&format!("\"code\":\"{code}\"")))
        .map(|l| l.to_string())
        .collect()
}

/// Pretty-print so a diff in the golden file is readable line by line.
fn pretty(l: &str) -> String {
    let j = rulec::json::parse(l).unwrap_or_else(|e| panic!("JSON として読めない: {e}\n{l}"));
    format!("{j:#?}\n")
}

fn snapshot(name: &str, lang: &str, got: &str) {
    let dir = match lang {
        "ja" => root().join("tests/golden/json"),
        _ => root().join("tests/golden/json/en"),
    };
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(format!("{name}.json"));
    if std::env::var("RULEC_BLESS").is_ok() || !p.exists() {
        std::fs::write(&p, got).unwrap();
        return;
    }
    let want = std::fs::read_to_string(&p).unwrap();
    assert_eq!(
        want, got,
        "JSON が変わりました: {name} ({lang})。意図した変更なら \
         RULEC_BLESS=1 cargo test --test json_v2 で焼き直してください"
    );
}

/// The keys whose value is prose, and so may differ between the two languages. Everything
/// else must be byte-identical.
fn strip_prose(l: &str) -> String {
    let j = rulec::json::parse(l).unwrap();
    let rulec::json::Json::Obj(m) = &j else { panic!("object でない") };
    let mut o = m.clone();
    o.remove("title");
    o.remove("notes");
    // A span's label is prose; its position is not.
    if let Some(rulec::json::Json::Arr(spans)) = o.get("spans") {
        let stripped: Vec<rulec::json::Json> = spans
            .iter()
            .map(|s| {
                let rulec::json::Json::Obj(sm) = s else { return s.clone() };
                let mut sm = sm.clone();
                sm.remove("label");
                rulec::json::Json::Obj(sm)
            })
            .collect();
        o.insert("spans".into(), rulec::json::Json::Arr(stripped));
    }
    format!("{:#?}", rulec::json::Json::Obj(o))
}

const CASES: &[(&str, &str, &str)] = &[
    ("E101", "tests/mutants/m_e101.rule", "E101"),
    ("E102", "tests/mutants/m_e102.rule", "E102"),
    ("E104", "tests/mutants/m_e104.rule", "E104"),
    ("E105", "tests/mutants/m_e105.rule", "E105"),
    ("E107", "tests/mutants/m_e107.rule", "E107"),
    ("E111", "tests/mutants/m_e111.rule", "E111"),
    ("E112", "tests/mutants/m_e112.rule", "E112"),
    ("W105", "tests/corpus/送料.rule", "W105"),
    ("W111", "tests/mutants/m_w111.rule", "W111"),
    ("W114", "tests/mutants/m_w114.rule", "W114"),
];

#[test]
fn 構造つきjsonを固定する() {
    for (name, file, code) in CASES {
        for lang in ["ja", "en"] {
            let ls = lines_for(file, code, lang);
            assert!(!ls.is_empty(), "{file} に {code} が出なかった");
            let body: String = ls.iter().map(|l| pretty(l)).collect();
            snapshot(name, lang, &body);
        }
    }
}

#[test]
fn 証人と直し方は言語で変わらない() {
    for (name, file, code) in CASES {
        let ja = lines_for(file, code, "ja");
        let en = lines_for(file, code, "en");
        assert_eq!(ja.len(), en.len(), "{name}: 件数が言語で違う");
        for (a, b) in ja.iter().zip(en.iter()) {
            assert_eq!(
                strip_prose(a),
                strip_prose(b),
                "{name}: 文面以外が言語で変わっている（witness と fix はデータです）"
            );
            // And the prose really is different, or the two-language claim is empty.
            let title = |l: &str| {
                rulec::json::parse(l).unwrap().get("title").unwrap().as_str().unwrap().to_string()
            };
            assert_ne!(title(a), title(b), "{name}: 日本語と英語の見出しが同じ");
        }
    }
}

#[test]
fn v1のフィールドは全部残っている() {
    // A reader written against v1 keeps working; `v` says who wrote the line.
    let (_, out) = run(&["check", "tests/mutants/m_e101.rule", "--format", "json"]);
    for l in out.lines().filter(|l| !l.trim().is_empty()) {
        let j = rulec::json::parse(l).unwrap();
        for k in ["severity", "code", "file", "line", "column", "title", "notes"] {
            assert!(j.get(k).is_some(), "v1 の {k} が消えている: {l}");
        }
        for k in ["v", "where", "spans", "witness", "rows", "fix"] {
            assert!(j.get(k).is_some(), "v2 の {k} が無い: {l}");
        }
        assert_eq!(j.get("v").unwrap().as_int(), Some(2));
    }
}

// ── The fix must not lie ─────────────────────────────────────────────────
//
// A `fix.text` that does not remove the finding is worse than no fix at all: it reads as an
// instruction and costs a round trip to discover it was wrong. These two apply the text
// mechanically and require the code to be gone.

/// The source, the `fix.text`, and the line the finding points at.
fn fix_of(file: &str, code: &str) -> (String, String, usize) {
    let src = std::fs::read_to_string(root().join(file)).unwrap();
    let (_, out) = run(&["check", file, "--format", "json"]);
    let line = out
        .lines()
        .find(|l| l.contains(&format!("\"code\":\"{code}\"")))
        .unwrap_or_else(|| panic!("{file} に {code} が出なかった"));
    let j = rulec::json::parse(line).unwrap();
    let fix = j.get("fix").unwrap();
    let text = fix.get("text").and_then(|t| t.as_str()).unwrap_or_default().to_string();
    assert!(!text.is_empty(), "{code} の fix.text が空");
    let at = j.get("line").unwrap().as_int().unwrap() as usize;
    (src, text, at)
}

#[test]
fn e101のfixを貼り続けると穴が閉じる() {
    // One row closes the witness it was built for, and the next run names the next gap. What
    // "the fix does not lie" means here is that every application really removes the witness
    // it named and the loop ends — not that one paste finishes the job.
    let mut src = std::fs::read_to_string(root().join("tests/mutants/m_e101.rule")).unwrap();
    let dir = std::env::temp_dir().join(format!("rulec-fix-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("t.rule");
    let mut seen: Vec<String> = Vec::new();
    for round in 0..30 {
        std::fs::write(&p, &src).unwrap();
        let (_, out) = run(&["check", p.to_str().unwrap(), "--format", "json"]);
        let Some(line) = out.lines().find(|l| l.contains("\"code\":\"E101\"")) else {
            assert!(round > 0, "そもそも E101 が出ていない");
            let _ = std::fs::remove_dir_all(&dir);
            return;
        };
        let j = rulec::json::parse(line).unwrap();
        let w = format!("{:?}", j.get("witness").unwrap());
        assert!(!seen.contains(&w), "同じ証人が二度出た（進んでいない）: {w}");
        seen.push(w);
        let fix = j.get("fix").unwrap();
        assert_eq!(fix.get("kind").unwrap().as_str(), Some("add_row"));
        let row = fix.get("text").unwrap().as_str().unwrap().to_string();
        assert!(row.starts_with("| ") && row.ends_with(" |"), "貼れる行でない: {row}");
        let at = j.get("line").unwrap().as_int().unwrap() as usize;
        src = paste_row(&src, &row, at);
        // Pasting must never break the file.
        let codes: Vec<&str> =
            rulec::check_source(&src, "patched.rule").iter().map(|d| d.code).collect();
        assert!(!codes.iter().any(|c| c.starts_with("E0")), "fix の行が構文を壊した: {codes:?}\n{row}");
    }
    panic!("30 回貼っても E101 が閉じない");
}

/// Insert `row` at the end of the table whose header is on line `at` (1-based).
fn paste_row(src: &str, row: &str, at: usize) -> String {
    let mut lines: Vec<String> = src.lines().map(|s| s.to_string()).collect();
    let mut i = at; // the line after the header, 0-based
    while i < lines.len() && !lines[i].trim_start().starts_with('|') {
        i += 1;
    }
    while i < lines.len() && lines[i].trim_start().starts_with('|') {
        i += 1;
    }
    lines.insert(i, row.to_string());
    lines.join("\n") + "\n"
}

#[test]
fn e104のfixを宣言に足すと丸めが決まる() {
    let (src, text, n) = fix_of("tests/mutants/m_e104.rule", "E104");
    assert!(text.starts_with("round "), "丸めの宣言になっていない: {text}");
    // The output whose rounding is missing is the one E104 points at; appending the text to
    // that declaration line is exactly what the hint says to do.
    let mut lines: Vec<String> = src.lines().map(|s| s.to_string()).collect();
    lines[n - 1] = format!("{}  {text}", lines[n - 1].trim_end());
    let patched = lines.join("\n") + "\n";
    let codes: Vec<&str> =
        rulec::check_source(&patched, "patched.rule").iter().map(|d| d.code).collect();
    assert!(!codes.contains(&"E104"), "fix を足しても E104 が残る: {codes:?}\n{text}");
    assert!(!codes.iter().any(|c| c.starts_with("E0")), "fix が構文を壊した: {codes:?}");
}

#[test]
fn terseは三行に絞って最後に道案内を出す() {
    let (c, out) = run(&["check", "tests/mutants/m_e101.rule", "--terse", "--lang", "en"]);
    assert_eq!(c, 1);
    let blocks: Vec<&str> = out.lines().filter(|l| l.starts_with("error[")).collect();
    assert_eq!(blocks.len(), 1, "{out}");
    assert!(out.contains("  --> "), "位置が無い: {out}");
    assert!(out.contains("witness: "), "証人が無い: {out}");
    // No frame, no hints.
    assert!(!out.contains("^^^"), "枠が出ている: {out}");
    assert!(!out.contains("hint:"), "ヒントが出ている: {out}");
    // The pointer appears exactly once, at the end.
    assert_eq!(out.matches("details: rulec explain").count(), 1, "{out}");
    assert!(out.trim_end().ends_with("details: rulec explain <code>"), "{out}");
    // JSON is already the machine shape; combining the two is refused rather than guessed at.
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(root())
        .args(["check", "tests/mutants/m_e101.rule", "--terse", "--format", "json"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(2));
}
