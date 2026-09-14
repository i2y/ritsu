//! The documents must not promise what the tool does not do.
//!
//! `AGENTS.md` and `docs/` are the only thing an agent reads before driving rulec, so a
//! command that does not exist or a link that goes nowhere is a real failure, not a typo.

use std::collections::BTreeSet;
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

fn docs() -> Vec<(String, String)> {
    let mut out = vec![
        ("AGENTS.md".to_string(), read("AGENTS.md")),
        ("README.md".to_string(), read("README.md")),
    ];
    for e in std::fs::read_dir(root().join("docs")).unwrap().flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| x == "md") {
            let rel = format!("docs/{}", p.file_name().unwrap().to_string_lossy());
            let body = std::fs::read_to_string(&p).unwrap();
            out.push((rel, body));
        }
    }
    out
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(root().join(rel)).unwrap_or_else(|_| panic!("読めない: {rel}"))
}

fn subcommands() -> BTreeSet<String> {
    let (c, out) = run(&["--help", "--lang", "en"]);
    assert_eq!(c, 0);
    out.lines()
        .filter_map(|l| l.strip_prefix("  rulec "))
        .filter_map(|l| l.split_whitespace().next())
        .map(|s| s.to_string())
        .collect()
}

#[test]
fn 文書が名指しするコマンドは実在する() {
    let have = subcommands();
    // `help`, and the two flags that stand in for a command name, are not in the list.
    let extra = ["help", "--help", "--version", "-h", "-V"];
    for (name, body) in docs() {
        for (i, l) in body.lines().enumerate() {
            // Only where the text is showing a command line: inside backticks, after a
            // shell prompt, or in a CI step. `rulec` in a sentence is prose.
            for lead in ["`rulec ", "$ rulec ", "run: rulec "] {
                let mut rest = l;
                while let Some(k) = rest.find(lead) {
                    rest = &rest[k + lead.len()..];
                    let Some(w) = rest.split(|c: char| c.is_whitespace() || c == '`').next() else {
                        continue;
                    };
                    if w.is_empty() || !w.chars().all(|c| c.is_ascii_lowercase() || c == '-') {
                        continue;
                    }
                    assert!(
                        have.contains(w) || extra.contains(&w),
                        "{name}:{}: `rulec {w}` というコマンドは無い",
                        i + 1
                    );
                }
            }
        }
    }
}

#[test]
fn 文書の相対リンクは実在する() {
    for (name, body) in docs() {
        let dir = if name.starts_with("docs/") { "docs" } else { "." };
        let mut rest = body.as_str();
        while let Some(k) = rest.find("](") {
            rest = &rest[k + 2..];
            let Some(end) = rest.find(')') else { break };
            let target = &rest[..end];
            if target.starts_with("http") || target.starts_with('#') {
                continue;
            }
            // Strip an in-page anchor.
            let path = target.split('#').next().unwrap();
            if path.is_empty() {
                continue;
            }
            let full = root().join(dir).join(path);
            assert!(full.exists(), "{name}: リンク先が無い: {target}");
        }
    }
}

/// The one worked example in AGENTS.md is the shape an agent will copy, so it has to be the
/// shape the tool really emits.
#[test]
fn agentsの実演はいまの出力と一致する() {
    let body = read("AGENTS.md");
    let want = body
        .lines()
        .find(|l| l.starts_with("{\"table\":\"運賃表\""))
        .expect("実演の出力行が無い");
    let (_, out) = run(&["check", "tests/mutants/m_e101.rule", "--format", "json", "--lang", "en"]);
    let line = out.lines().find(|l| l.contains("\"code\":\"E101\"")).expect("E101 が無い");
    let j = rulec::json::parse(line).unwrap();
    let got = format!(
        "{{\"table\":{},\"witness\":{},\"fix\":{}}}",
        rulec::json::quote(j.get("where").unwrap().get("table").unwrap().as_str().unwrap()),
        to_json(j.get("witness").unwrap().get("inputs").unwrap()),
        to_json(j.get("fix").unwrap()),
    );
    assert_eq!(want, got, "AGENTS.md の実演が実際の出力と食い違う");
}

/// Re-serialise a parsed value in the compact form the document shows.
fn to_json(j: &rulec::json::Json) -> String {
    match j {
        rulec::json::Json::Obj(m) => {
            let parts: Vec<String> = m
                .iter()
                .map(|(k, v)| format!("{}:{}", rulec::json::quote(k), to_json(v)))
                .collect();
            format!("{{{}}}", parts.join(","))
        }
        rulec::json::Json::Arr(a) => {
            format!("[{}]", a.iter().map(to_json).collect::<Vec<_>>().join(","))
        }
        rulec::json::Json::Str(s) => rulec::json::quote(s),
        other => format!("{other}"),
    }
}

/// The loop AGENTS.md teaches has to be the loop the CI section of the README runs.
#[test]
fn agentsとreadmeのciが同じ行を言う() {
    let agents = read("AGENTS.md");
    let readme = read("README.md");
    for line in [
        "rulec fmt --check rules/",
        "rulec check rules/ --diff-base origin/main",
        "rulec gen rules/ --out generated/ --check",
        "rulec coverage rules/",
        "rulec test generated/",
    ] {
        assert!(agents.contains(line), "AGENTS.md の CI に `{line}` が無い");
        assert!(readme.contains(line), "README の CI に `{line}` が無い");
    }
}

/// The set of target languages, held to `src/backend.rs` wherever a document states it.
///
/// Adding Ruby meant editing twenty-odd places by hand, and the only thing that caught an
/// omission was the flow diagram's `--verify`; the prose had no such check and would have
/// gone stale silently (§15.20). This is that check for the prose: every document that
/// names the set has to name all of it, and nothing that is not in it.
#[test]
fn 文書が並べる対象言語はレジストリと同じ() {
    let all = rulec::backend::ALL;
    let names: Vec<&str> = all.iter().map(|b| b.name).collect();
    // The two spellings the documents use, and the Japanese one.
    let en = rulec::backend::names_en();
    let ja = rulec::backend::names_ja();
    let en_comma = names.join(", ");

    // A document naming three or more of them in a row is stating the set.
    for (name, body) in docs() {
        for (i, line) in body.lines().enumerate() {
            let hits = names.iter().filter(|n| line.contains(**n)).count();
            if hits < 3 {
                continue;
            }
            assert!(
                line.contains(&en) || line.contains(&ja) || line.contains(&en_comma),
                "{name}:{}: 対象言語の並びがレジストリと違います。\n  行: {}\n  期待: 「{en}」か「{ja}」",
                i + 1,
                line.trim()
            );
        }
    }
}

/// Every id the registry declares has a directory of its own under what `gen` writes, and
/// nothing else does. A backend added without its files, or files left behind by one that
/// was removed, both fail here.
#[test]
fn genが書く言語のディレクトリはレジストリと同じ() {
    let dir = std::env::temp_dir().join(format!("rulec-docs-langs-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (code, out) = run(&["gen", "tests/corpus/ec261.rule", "--out", dir.to_str().unwrap()]);
    assert_eq!(code, 0, "{out}");
    let mut got: Vec<String> = std::fs::read_dir(&dir)
        .expect("出力が無い")
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n != "vectors")
        .collect();
    got.sort();
    let mut want: Vec<String> = rulec::backend::ids().iter().map(|s| s.to_string()).collect();
    want.sort();
    assert_eq!(got, want, "gen が書くディレクトリとレジストリが食い違います");
    let _ = std::fs::remove_dir_all(&dir);
}
