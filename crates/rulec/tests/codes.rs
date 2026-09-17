//! The ledger of diagnostic codes (§11) is the single source.
//!
//! Three properties are pinned, and together they close the way a ledger normally rots:
//! a code the checker emits but the ledger has forgotten, an example that no longer
//! produces its code, and a checked-in `docs/codes*.md` that has drifted from the tool.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn ledger_codes() -> BTreeSet<&'static str> {
    rulec::codes::ledger().iter().map(|e| e.code).collect()
}

/// Every code any file under `dir` produces.
fn codes_seen_in(dir: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let d = root().join(dir);
    for e in std::fs::read_dir(&d).unwrap_or_else(|_| panic!("読めない: {dir}")).flatten() {
        let p = e.path();
        if p.extension().is_none_or(|x| x != "rule") {
            continue;
        }
        let src = std::fs::read_to_string(&p).unwrap();
        let rel = format!("{dir}/{}", p.file_name().unwrap().to_string_lossy());
        for d in rulec::check_source(&src, &rel) {
            out.insert(d.code.to_string());
        }
    }
    out
}

#[test]
fn 出しうるコードは全部台帳にある() {
    let have = ledger_codes();
    let mut seen = codes_seen_in("tests/mutants");
    seen.extend(codes_seen_in("tests/corpus"));
    // The golden snapshots are named after the code they pin, so they are a second list of
    // codes the tool really prints.
    for e in std::fs::read_dir(root().join("tests/golden")).unwrap().flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        if let Some(stem) = n.strip_suffix(".txt") {
            seen.insert(stem.split('-').next().unwrap().to_string());
        }
    }
    let missing: Vec<&String> = seen.iter().filter(|c| !have.contains(c.as_str())).collect();
    assert!(missing.is_empty(), "台帳に無いコードが出ている: {missing:?}");
}

#[test]
fn 台帳の例は本当にそのコードを出す() {
    // An example that has rotted is worse than none: the prose around it still reads well.
    for e in rulec::codes::ledger() {
        let budget = e.budget.unwrap_or(rulec::region::DEFAULT_BUDGET);
        let ds = rulec::report_with(e.example, "explain.rule", budget).diags;
        let codes: Vec<&str> = ds.iter().map(|d| d.code).collect();
        assert!(
            codes.contains(&e.code),
            "{} の例が {} を出さない（出たのは {:?}）:\n{}",
            e.code,
            e.code,
            codes,
            e.example
        );
    }
}

#[test]
fn 台帳は重複せず_関係するコードも台帳にある() {
    let all = rulec::codes::ledger();
    let mut seen = BTreeSet::new();
    for e in &all {
        assert!(seen.insert(e.code), "台帳に {} が二度ある", e.code);
        assert!(!e.title.is_empty() && !e.when.is_empty() && !e.fix.is_empty(), "{} の欄が空", e.code);
        for r in e.related {
            assert!(all.iter().any(|x| x.code == *r), "{} が台帳に無い {r} を指している", e.code);
            assert_ne!(r, &e.code, "{} が自分自身を指している", e.code);
        }
    }
    // Every code that has a golden snapshot, and every code in the DESIGN ledger, is here;
    // `出しうるコードは全部台帳にある` covers the first. There are no vacant numbers left.
    assert_eq!(all.len(), 45, "台帳の件数が変わった: {}", all.len());
}

fn run(args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(root())
        .args(args)
        .output()
        .expect("rulec を起動できない");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned())
}

/// `docs/codes*.md` is a generated file: checked in, and held to the current output.
/// Re-bake with `cargo run -- explain --all --format markdown --lang en > docs/codes.md`
/// (and `--lang ja > docs/codes.ja.md`).
#[test]
fn チェックインされた文書はいまの出力と同じ() {
    for (lang, file) in [("en", "docs/codes.md"), ("ja", "docs/codes.ja.md")] {
        let (c, got) = run(&["explain", "--all", "--format", "markdown", "--lang", lang]);
        assert_eq!(c, 0);
        let p = root().join(file);
        assert!(Path::new(&p).exists(), "{file} がありません。生成して入れてください");
        let want = std::fs::read_to_string(&p).unwrap();
        assert_eq!(
            want, got,
            "{file} が古いか手で編集されています。\
             `cargo run -- explain --all --format markdown --lang {lang} > {file}` で焼き直してください"
        );
    }
}

#[test]
fn 知らないコードは2で止まる() {
    let (c, _) = run(&["explain", "E999"]);
    assert_eq!(c, 2);
    let (c, out) = run(&["explain", "e101"]);
    assert_eq!(c, 0, "コードの大小文字は問わない");
    assert!(out.contains("E101"), "{out}");
}

#[test]
fn jsonは鍵が英語で読み戻せる() {
    let (c, out) = run(&["explain", "--all", "--format", "json", "--lang", "ja"]);
    assert_eq!(c, 0);
    let lines: Vec<&str> = out.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(lines.len(), rulec::codes::ledger().len(), "一行一件でない");
    for l in &lines {
        let j = rulec::json::parse(l).unwrap_or_else(|e| panic!("JSON として読めない: {e}\n{l}"));
        for k in ["code", "severity", "title", "when", "fix", "example", "related", "lang"] {
            assert!(j.get(k).is_some(), "{k} が無い: {l}");
        }
        assert_eq!(j.get("lang").unwrap().as_str(), Some("ja"));
    }
    // The keys are the stable API; only the prose follows `--lang`.
    let (_, en) = run(&["explain", "--all", "--format", "json", "--lang", "en"]);
    let key_of = |s: &str| -> Vec<String> {
        s.lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| rulec::json::parse(l).unwrap().get("code").unwrap().as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(key_of(&out), key_of(&en));
}

/// The codes a command's `--help` names must be codes the ledger carries. Without this the
/// CLI table and the ledger drift apart, which is the exact failure this step removes.
#[test]
fn helpが名指しするコードは台帳にある() {
    let have = ledger_codes();
    let (_, help) = run(&["--help", "--lang", "en"]);
    let cmds: Vec<String> = help
        .lines()
        .filter_map(|l| l.strip_prefix("  rulec "))
        .filter_map(|l| l.split_whitespace().next())
        .map(|s| s.to_string())
        .collect();
    let mut named = 0usize;
    for c in &cmds {
        let (_, page) = run(&[c, "--help", "--lang", "en"]);
        let Some(tail) = page.split("Diagnostics it can print").nth(1).and_then(|t| t.split(":\n").nth(1))
        else {
            continue;
        };
        for code in tail.split_whitespace() {
            assert!(have.contains(code), "`rulec {c} --help` が台帳に無い {code} を名指ししている");
            named += 1;
        }
    }
    assert!(named >= 30, "どの help もコードを名指ししていない");
}

// ── The reference (docs/reference.md) ────────────────────────────────────

/// The reserved-word table in the reference must be exactly `kw::RESERVED`. Both a word the
/// parser reserves but the document omits, and a word the document claims but the parser
/// does not reserve, go red — the same shape as the README's keyword test.
#[test]
fn 参照文書の予約語表はkwと一致する() {
    let doc = std::fs::read_to_string(root().join("docs/reference.md")).unwrap();
    let i = doc.find("<!-- RESERVED -->").expect("予約語表の印が無い");
    let j = doc.find("<!-- /RESERVED -->").expect("予約語表の閉じ印が無い");
    let mut listed: Vec<String> = Vec::new();
    for w in doc[i..j].split('`').skip(1).step_by(2) {
        listed.push(w.to_string());
    }
    listed.sort();
    listed.dedup();
    let mut want: Vec<String> = rulec::kw::RESERVED.iter().map(|s| s.to_string()).collect();
    want.sort();
    want.dedup();
    assert_eq!(listed, want, "docs/reference.md の予約語表と src/kw.rs が食い違う");
}

/// Every rule of the corpus has to be describable by the reference, so the reference has to
/// name every section word the corpus actually uses.
#[test]
fn 参照文書は行頭の語を全部説明している() {
    let doc = std::fs::read_to_string(root().join("docs/reference.md")).unwrap();
    for w in rulec::kw::LINE_HEAD.iter().chain([&rulec::kw::RULE]) {
        assert!(doc.contains(&format!("`{w}`")), "docs/reference.md に `{w}` の説明が無い");
    }
}
