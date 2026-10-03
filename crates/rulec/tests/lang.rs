//! The output language switch (§11 principle 7): `--lang` beats `RULEC_LANG`,
//! `RULEC_LANG` beats the default, and the default is English. Every surface
//! that prints prose — diagnostics, reports, the rendered document, generated
//! code — must follow the same switch.

use std::process::Command;

fn run(args: &[&str], env: &[(&str, &str)]) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_rulec"));
    c.current_dir(env!("CARGO_MANIFEST_DIR")).args(args);
    // `.cargo/config.toml` pins RULEC_LANG=ja for the suite; clear it so the
    // fallback order can be observed.
    c.env_remove("RULEC_LANG");
    for (k, v) in env {
        c.env(k, v);
    }
    let o = c.output().expect("cannot start rulec");
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

fn has_japanese(s: &str) -> bool {
    s.chars().any(|c| ('\u{3040}'..='\u{30ff}').contains(&c) || ('\u{4e00}'..='\u{9fff}').contains(&c))
}

/// The first line of the first diagnostic: `error[E101]: <title>`.
fn title(out: &str) -> String {
    out.lines().find(|l| l.starts_with("error[") || l.starts_with("warning[")).unwrap_or("").to_string()
}

const MUTANT: &str = "tests/mutants/m_e101.rule";

#[test]
fn default_is_english() {
    let (c, out, _) = run(&["check", MUTANT], &[]);
    assert_eq!(c, 1);
    let t = title(&out);
    assert!(t.starts_with("error[E101]:") && !has_japanese(&t), "{out}");
}

#[test]
fn env_selects_english() {
    let (c, out, _) = run(&["check", MUTANT], &[("RULEC_LANG", "en")]);
    assert_eq!(c, 1);
    let t = title(&out);
    assert!(t.starts_with("error[E101]:") && !has_japanese(&t), "{out}");
}

/// Japanese is one setting away — the single knob the approver's side needs.
#[test]
fn env_selects_japanese() {
    let (c, out, _) = run(&["check", MUTANT], &[("RULEC_LANG", "ja")]);
    assert_eq!(c, 1);
    assert!(title(&out).contains("完全性の欠落"), "{out}");
}

#[test]
fn flag_beats_env() {
    let (_, out, _) = run(&["check", MUTANT, "--lang", "ja"], &[("RULEC_LANG", "en")]);
    assert!(title(&out).contains("完全性の欠落"), "{out}");
    let (_, out, _) = run(&["--lang=en", "check", MUTANT], &[("RULEC_LANG", "ja")]);
    assert!(!has_japanese(&title(&out)), "{out}");
}

#[test]
fn unknown_language_is_refused() {
    let (c, _, e) = run(&["check", MUTANT, "--lang", "fr"], &[]);
    assert_eq!(c, 2, "{e}");
    assert!(e.contains("--lang"), "{e}");
}

#[test]
fn codes_do_not_depend_on_language() {
    let (_, ja, _) = run(&["check", "tests/corpus", "--format", "json"], &[]);
    let (_, en, _) = run(&["check", "tests/corpus", "--format", "json", "--lang", "en"], &[]);
    let codes = |s: &str| -> Vec<String> {
        s.lines().filter_map(|l| l.split("\"code\":\"").nth(1).map(|r| r.split('"').next().unwrap().to_string())).collect()
    };
    assert_eq!(codes(&ja), codes(&en), "the codes are the stable API; only the prose may differ");
    assert!(!en.lines().any(|l| l.contains("\"title\":\"") && has_japanese(l.split("\"title\":\"").nth(1).unwrap().split("\",\"").next().unwrap())), "{en}");
}

#[test]
fn document_and_reports_follow_the_switch() {
    let (c, doc, e) = run(&["doc", "tests/corpus/ゆうパック運賃.rule", "--lang", "en"], &[]);
    assert_eq!(c, 0, "{e}");
    let headings: Vec<&str> = doc.lines().filter(|l| l.starts_with("## ")).collect();
    assert!(!headings.is_empty());
    for h in &headings {
        // Table names quoted from the rule are Japanese; the heading words are not.
        let words: String = h.chars().filter(|c| c.is_ascii_alphabetic() || *c == ' ').collect();
        assert!(words.trim().len() >= 4, "English heading expected: {h}");
    }
    let (c, cov, _) = run(&["coverage", "tests/corpus/送料.rule", "--lang", "en"], &[]);
    assert_eq!(c, 0, "row coverage must still be recognised in English: {cov}");
    // The first line is the file name (Japanese by design); the report below it is prose.
    for l in cov.lines().skip(1) {
        assert!(!has_japanese(l), "{cov}");
    }
}

#[test]
fn generated_code_prose_follows_the_switch() {
    let dir = std::env::temp_dir().join(format!("rulec-lang-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let out = dir.to_string_lossy().to_string();
    let (c, _, e) = run(&["gen", "tests/corpus/ゆうパック運賃.rule", "--out", &out, "--lang", "en"], &[]);
    assert_eq!(c, 0, "{e}");
    let py = std::fs::read_to_string(dir.join("python").join("yupack_fee.py")).unwrap();
    // Comments quote cells (Japanese by design); the docstring and the runtime
    // error text are prose and must be English here.
    let doc_line = py.lines().find(|l| l.trim_start().starts_with("\"\"\"")).unwrap_or("");
    assert!(!has_japanese(doc_line), "{doc_line}");
    // The same rule generated in Japanese differs only in prose, and each
    // language is deterministic: `gen --check` must pass in the language it was
    // generated with.
    let (c, o, _) = run(&["gen", "tests/corpus/ゆうパック運賃.rule", "--out", &out, "--check", "--lang", "en"], &[]);
    assert_eq!(c, 0, "{o}");
    let (c, _, _) = run(&["gen", "tests/corpus/ゆうパック運賃.rule", "--out", &out, "--check", "--lang", "ja"], &[]);
    assert_eq!(c, 1, "a Japanese --check against English output must report a difference");
    let _ = std::fs::remove_dir_all(&dir);
}
