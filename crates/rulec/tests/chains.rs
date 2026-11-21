//! The mistakes other lines of a file hang on (ritsu's DESIGN 8.3): each file of `tests/chains/`
//! holds one or two, English first and its Japanese version beside it, and what `rulec check` prints
//! of it, in English and in Japanese, is its golden file there (`<name>.en.txt` and `<name>.ja.txt`
//! for `<name>.rule`, as dandori's `tests/fixtures/chain_*.flow`). A mistake is said once, where it
//! is; a name that is not there is said where it is first written, with the lines that write it too;
//! the lines that hang on a mistake say nothing; and the file still fails. `RITSU_BLESS=1 cargo
//! test -p rulec --test chains` writes the golden files again; read the diff.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

/// The diagnostics of a printed check: no two are the same sentence (rulec's head line has no place,
/// which its `-->` line under it gives).
fn said_twice(text: &str) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    let mut twice = Vec::new();
    for l in text.lines().filter(|l| ["error[", "warning[", "エラー[", "警告["].iter().any(|h| l.starts_with(h))) {
        if !seen.insert(l.to_string()) {
            twice.push(l.to_string());
        }
    }
    twice
}

#[test]
fn a_mistake_is_said_once_and_the_lines_that_hang_on_it_say_nothing() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/chains");
    let mut files: Vec<String> =
        std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".rule")).collect();
    files.sort();
    assert!(files.len() >= 4, "{files:?}");
    let mut wrong = Vec::new();
    for file in &files {
        let stem = file.strip_suffix(".rule").unwrap();
        for lang in ["en", "ja"] {
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_rulec"));
            cmd.args(["check", file.as_str(), "--lang", lang]).current_dir(&dir).env("NO_COLOR", "1").env_remove("RITSU_LANG").env_remove("RULEC_LANG");
            let ran = ritsu_testkit::run::finish(&mut cmd, Duration::from_secs(60), &format!("rulec check {file}"));
            let text = ran.both();
            if ran.code != Some(1) {
                wrong.push(format!("{file} ({lang}): exit {:?}, not 1:\n{text}", ran.code));
            }
            for l in said_twice(&text) {
                wrong.push(format!("{file} ({lang}): said twice: {l}"));
            }
            if let Err(e) = ritsu_testkit::golden::check(&dir.join(format!("{stem}.{lang}.txt")), &text) {
                wrong.push(e);
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n\n"));
}
