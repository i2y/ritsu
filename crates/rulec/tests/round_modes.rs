//! An output's `round` names one of the five rounding modes of §7.3: `down`, `up`, `half_up`,
//! `half_down` and `half_even` (§15.341). The parser takes any word after `round`, and only the check
//! reads it there. A word none of the five used to pass without a word: the checks that read the grid
//! read the output as one with no rounding, the generated code rounded down in most languages, and
//! NumPy's failed when run. `check` now stops at it with E154, which names the word, the five, and
//! the mode the word is nearest, where one is.
//!
//! The materials are in `tests/round_modes`, English first, with a Japanese version beside each: the
//! start of a mode (`half`), a mode in capitals (`DOWN`), a common name of one (`truncate`,
//! `切り捨て`) and a word near none (`sideways`, `端数処理`). What `rulec check` prints for each is
//! its golden file in `tests/golden/round_modes` (`RULEC_BLESS=1 cargo test --test round_modes`
//! writes them again). Written with each of the five modes instead, each passes the check without a
//! word, and is generated for every target and extra.

use ritsu_testkit::TempDir;
use rulec::i18n::{self, Lang};
use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// One rule for each kind of word, the English first and the Japanese version beside it, with the
/// mode E154 names as the nearest, if one.
const WORDS: &[(&str, &str, Option<&str>)] = &[
    ("mode_written_half", "halfという丸め方", Some("half_up")),
    ("mode_in_capitals", "大文字の丸め方", Some("down")),
    ("mode_by_a_common_name", "よく使う名前の丸め方", Some("down")),
    ("mode_near_none", "どれにも近くない丸め方", None),
];

/// The five modes of §7.3.
const MODES: [&str; 5] = ["down", "up", "half_up", "half_down", "half_even"];

/// The path of a material from the crate's root, and its text.
fn source(stem: &str) -> (String, String) {
    let rel = format!("tests/round_modes/{stem}.rule");
    let src = std::fs::read_to_string(root().join(&rel)).unwrap_or_else(|_| panic!("cannot read {rel}"));
    (rel, src)
}

/// What `rulec check <rel>` prints, in `lang`, and what it found.
fn printed(rel: &str, src: &str, lang: Lang) -> (String, Vec<rulec::diag::Diag>) {
    let lines: Vec<String> = src.lines().map(String::from).collect();
    i18n::with(lang, || {
        let r = rulec::report(src, rel);
        let text = format!("{}{}", rulec::findings_text(&r.diags, &lines), rulec::check_tail(&r.shadow, &r.diags, rel, 0));
        (text, r.diags)
    })
}

/// The word a material writes after `round`.
fn word(src: &str) -> &str {
    let (_, after) = src.split_once(" round ").expect("the material rounds its output");
    after.split_once('(').expect("the mode is followed by its grid").0
}

/// Every rule here is listed above, with its Japanese version.
#[test]
fn every_rule_here_is_one_of_the_words() {
    let mut names: Vec<String> = std::fs::read_dir(root().join("tests/round_modes"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".rule"))
        .collect();
    names.sort();
    let mut listed: Vec<String> = WORDS.iter().flat_map(|(en, ja, _)| [format!("{en}.rule"), format!("{ja}.rule")]).collect();
    listed.sort();
    assert_eq!(names, listed);
}

/// Each word none of the five is said once, with E154, in both versions: the English and the
/// Japanese wording are their golden files, and the note names the mode the word is nearest, or
/// none.
#[test]
fn each_word_none_of_the_five_is_said_once_with_the_nearest() {
    let mut failures = Vec::new();
    for (en, ja, near) in WORDS {
        for (stem, lang, tag) in [(*en, Lang::En, "en"), (*ja, Lang::Ja, "ja")] {
            let (rel, src) = source(stem);
            let (text, ds) = printed(&rel, &src, lang);
            if let Err(e) = ritsu_testkit::golden::check(&root().join(format!("tests/golden/round_modes/{stem}.{tag}.txt")), &text) {
                failures.push(e);
            }
            let codes: Vec<&str> = ds.iter().map(|d| d.code).collect();
            if codes != ["E154"] {
                failures.push(format!("{rel}: {codes:?}, not E154 alone\n{text}"));
                continue;
            }
            let w = word(&src);
            let title = if tag == "ja" { format!("`{w}` という丸め方はありません") } else { format!("There is no rounding mode `{w}`") };
            if ds[0].title != title {
                failures.push(format!("{rel}: the title is {:?}, not {title:?}", ds[0].title));
            }
            let hints: Vec<&String> = ds[0].notes.iter().filter(|n| n.starts_with("Did you mean") || n.ends_with("のことですか。")).collect();
            let want: Vec<String> = near.iter().map(|m| if tag == "ja" { format!("`{m}` のことですか。") } else { format!("Did you mean `{m}`?") }).collect();
            if hints.iter().map(|h| h.as_str()).ne(want.iter().map(|w| w.as_str())) {
                failures.push(format!("{rel}: the hint is {hints:?}, not {want:?}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// `rulec gen` writes nothing for a word none of the five, asked for every target and extra: it says
/// the file does not pass the check and exits 1, where it used to write code that rounded down, or
/// failed when run.
#[test]
fn gen_writes_nothing_for_a_word_none_of_the_five() {
    let tmp = TempDir::new("round-modes-gen");
    for (en, ja, _) in WORDS {
        for stem in [*en, *ja] {
            let (rel, _) = source(stem);
            let out = tmp.path().join(stem);
            let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
                .current_dir(root())
                .args(rulec::targets::up_to_0_40(&["gen", &rel, "--out", out.to_str().unwrap()]))
                .env("RULEC_LANG", "en")
                .output()
                .unwrap();
            let err = String::from_utf8_lossy(&o.stderr);
            assert_eq!(o.status.code(), Some(1), "{rel}: {err}");
            assert!(err.contains("does not pass check"), "{rel}: {err}");
            assert!(!out.exists() || std::fs::read_dir(&out).unwrap().next().is_none(), "{rel}: gen wrote files");
        }
    }
}

/// Written with each of the five modes instead, each material passes the check without a word, in
/// both languages, and `rulec gen` writes every target and extra.
#[test]
fn the_five_modes_pass_and_are_generated_for_every_target() {
    let tmp = TempDir::new("round-modes-five");
    for (en, ja, _) in WORDS {
        for (stem, lang) in [(*en, Lang::En), (*ja, Lang::Ja)] {
            let (rel, src) = source(stem);
            let w = word(&src);
            for mode in MODES {
                let written = src.replacen(&format!(" round {w}("), &format!(" round {mode}("), 1);
                let (text, ds) = printed(&rel, &written, lang);
                assert!(ds.is_empty(), "{rel} with {mode}:\n{text}");
                // Generated once a kind of word: the materials differ only in the word and the
                // description.
                if stem != WORDS[0].0 && stem != WORDS[0].1 {
                    continue;
                }
                let dir = tmp.path().join(format!("{stem}-{mode}"));
                std::fs::create_dir_all(&dir).unwrap();
                let file = dir.join("rule.rule");
                std::fs::write(&file, &written).unwrap();
                let out = dir.join("out");
                let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
                    .args(rulec::targets::up_to_0_40(&["gen", file.to_str().unwrap(), "--out", out.to_str().unwrap()]))
                    .env("RULEC_LANG", "en")
                    .output()
                    .unwrap();
                assert_eq!(o.status.code(), Some(0), "{rel} with {mode}: {}", String::from_utf8_lossy(&o.stderr));
                for t in ["python", "numpy", "typescript", "rust", "go", "java", "sql"] {
                    assert!(out.join(t).is_dir(), "{rel} with {mode}: nothing written for {t}");
                }
            }
        }
    }
}
