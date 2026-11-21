//! Every error says the file it is in, the errors of the syntax among them (§15.318).
//!
//! A line the parser could not read was framed with its number and no `--> file:line` above it:
//! `rulec check a.rule b.rule` then printed two frames, and which file to open for which was left
//! unsaid. The errors of single characters, which the lexer finds, named their file all along.
//! Both kinds are checked here, for one file and for two, in English and in Japanese, as text,
//! as `--terse` and as JSON; and no rule of `tests/mutants` is left with a finding that has
//! marks and no place.

use rulec::i18n::{self, Lang};
use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// A line that starts with no word: the parser's E004, at line 9.
const NO_WORD: &str = "rule a v1\n\ninputs\n  x : number  range >=1 <=3\n\noutputs\n  y : number  round down(1)\n\n| stray line\ntable t\npolicy unique\n| x | -> y |\n| - | 1    |\n";

/// A character no token starts with: the lexer's E002, at line 12.
const BAD_CHARACTER: &str = "rule b v1\n\ninputs\n  x : number  range >=1 <=3\n\noutputs\n  y : number  round down(1)\n\ntable t\npolicy unique\n| x | -> y |\n| - | 1 $ |\n";

fn rulec_in(dir: &std::path::Path, args: &[&str]) -> (i32, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env_remove("RULEC_LANG").current_dir(dir).args(args).output().expect("cannot start rulec");
    (o.status.code().unwrap_or(-1), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)))
}

fn two_files(name: &str) -> ritsu_testkit::TempDir {
    let t = ritsu_testkit::TempDir::new(name);
    std::fs::write(t.path().join("a.rule"), NO_WORD).unwrap();
    std::fs::write(t.path().join("b.rule"), BAD_CHARACTER).unwrap();
    t
}

/// The line under each heading of a finding, in the order the findings are printed.
fn places(out: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = out.lines().collect();
    lines
        .iter()
        .enumerate()
        .filter(|(_, l)| l.starts_with("error[") || l.starts_with("エラー["))
        .map(|(i, l)| (l.split(':').next().unwrap_or("").to_string(), lines.get(i + 1).copied().unwrap_or("").to_string()))
        .collect()
}

#[test]
fn an_error_of_the_syntax_names_its_file_and_line() {
    let t = two_files("syntax-place");
    for (lang, head) in [("en", "error"), ("ja", "エラー")] {
        // one file: the parser's error, then the lexer's
        let (code, out) = rulec_in(t.path(), &["check", "a.rule", "--lang", lang]);
        assert_eq!(code, 1, "{out}");
        assert_eq!(places(&out), [(format!("{head}[E004]"), "  --> a.rule:9".to_string())], "{out}");
        let (_, out) = rulec_in(t.path(), &["check", "b.rule", "--lang", lang]);
        assert_eq!(places(&out), [(format!("{head}[E002]"), "  --> b.rule:12".to_string())], "{out}");
        // two files in one run: each frame under its own file
        for extra in [&[][..], &["--terse"][..]] {
            let mut args = vec!["check", "a.rule", "b.rule", "--lang", lang];
            args.extend(extra);
            let (code, out) = rulec_in(t.path(), &args);
            assert_eq!(code, 1, "{out}");
            assert_eq!(places(&out), [(format!("{head}[E004]"), "  --> a.rule:9".to_string()), (format!("{head}[E002]"), "  --> b.rule:12".to_string())], "{out}");
        }
    }
    // the frame is the one the other errors have
    let (_, out) = rulec_in(t.path(), &["check", "a.rule", "--lang", "en"]);
    assert!(out.starts_with("error[E004]: The line does not start with a word\n  --> a.rule:9\n  |\n9 | | stray line\n  | ^^^^^^^^^^^^\n  |\n"), "{out}");
}

#[test]
fn the_json_of_an_error_of_the_syntax_names_the_same_place() {
    let t = two_files("syntax-place-json");
    for lang in ["en", "ja"] {
        let (code, out) = rulec_in(t.path(), &["check", "a.rule", "b.rule", "--format", "json", "--lang", lang]);
        assert_eq!(code, 1, "{out}");
        let found: Vec<ritsu_base::json::Json> = out.lines().filter(|l| l.starts_with('{')).map(|l| ritsu_base::json::parse(l).unwrap()).collect();
        let at: Vec<(String, String, i128, String, i128)> = found
            .iter()
            .map(|d| {
                let s = |j: &ritsu_base::json::Json, k: &str| j.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
                let n = |j: &ritsu_base::json::Json, k: &str| j.get(k).and_then(|x| x.as_int()).unwrap_or(-1);
                let w = d.get("where").unwrap();
                (s(d, "code"), s(d, "file"), n(d, "line"), s(w, "file"), n(w, "line"))
            })
            .collect();
        assert_eq!(at, [("E004".to_string(), "a.rule".to_string(), 9, "a.rule".to_string(), 9), ("E002".to_string(), "b.rule".to_string(), 12, "b.rule".to_string(), 12)], "{out}");
    }
}

/// No finding of a deliberately broken rule that marks a line is without the `-->` that names
/// its file: the syntax errors are most of what `tests/mutants` holds below `m_e03`.
#[test]
fn no_finding_with_a_mark_is_without_its_place() {
    let dir = root().join("tests/mutants");
    let mut seen = 0;
    let mut names: Vec<String> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".rule")).collect();
    names.sort();
    for name in names {
        let rel = format!("tests/mutants/{name}");
        let src = std::fs::read_to_string(root().join(&rel)).unwrap();
        for lang in [Lang::En, Lang::Ja] {
            for d in i18n::with(lang, || rulec::check_source(&src, &rel)) {
                seen += 1;
                // a contribution's finding may be at a line of the rule it adds to: any file, but one
                assert!(d.marks.is_empty() || d.where_.contains(".rule:"), "{rel}: {} {} has marks and the place {:?}", d.code, d.title, d.where_);
            }
        }
    }
    assert!(seen >= 500, "only {seen} findings were looked at");
}
