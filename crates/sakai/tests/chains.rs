//! The mistakes other lines of a map hang on (ritsu's DESIGN 8.3): each directory of `tests/chains/`
//! is a small map with one, English first and its Japanese version beside it (`<name>.ja/`), and
//! what `sakai check` prints of its map, run in the directory, in English and in Japanese, is its
//! golden file beside the directory (`<name>.en.txt` and `<name>.ja.txt`, as dandori's
//! `tests/fixtures/chain_*.flow`). A mistake is said once, where it is; a name that is not there is
//! said where a file first writes it, with the lines of that file that write it too, and once in
//! each file that writes it; the lines that hang on a mistake say nothing; and the map still fails.
//! `RITSU_BLESS=1 cargo test -p sakai --test chains` writes the golden files again; read the diff.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

/// The diagnostics of a printed check, each without its place: no two are the same sentence of the
/// same file.
fn said_twice(text: &str) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    let mut twice = Vec::new();
    for l in text.lines().filter(|l| ["error[", "warning[", "エラー[", "警告["].iter().any(|h| l.starts_with(h))) {
        // `error[E007]: <file>:<line>:<column>: <sentence>`
        let mut parts = l.splitn(3, ": ");
        let (head, place, sentence) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""), parts.next().unwrap_or(""));
        let file = place.split(':').next().unwrap_or("");
        if !seen.insert(format!("{head} {file} {sentence}")) {
            twice.push(l.to_string());
        }
    }
    twice
}

#[test]
fn a_mistake_is_said_once_and_the_lines_that_hang_on_it_say_nothing() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/chains");
    let mut maps: Vec<String> =
        std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap()).filter(|e| e.path().is_dir()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    maps.sort();
    assert!(maps.len() >= 4, "{maps:?}");
    let mut wrong = Vec::new();
    for name in &maps {
        let at = dir.join(name);
        let map: Vec<String> = std::fs::read_dir(&at)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".ctx"))
            .collect();
        assert_eq!(map.len(), 1, "{name}: one map at the top, not {map:?}");
        for lang in ["en", "ja"] {
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_sakai"));
            cmd.args(["check", map[0].as_str(), "--lang", lang]).current_dir(&at).env("NO_COLOR", "1").env_remove("RITSU_LANG").env_remove("SAKAI_LANG");
            let ran = ritsu_testkit::run::finish(&mut cmd, Duration::from_secs(60), &format!("sakai check {name}/{}", map[0]));
            let text = ran.both();
            if ran.code != Some(1) {
                wrong.push(format!("{name} ({lang}): exit {:?}, not 1:\n{text}", ran.code));
            }
            for l in said_twice(&text) {
                wrong.push(format!("{name} ({lang}): said twice: {l}"));
            }
            if let Err(e) = ritsu_testkit::golden::check(&dir.join(format!("{name}.{lang}.txt")), &text) {
                wrong.push(e);
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n\n"));
}
