//! W901 (ritsu's DESIGN 16.3): a key written in a rule, in a string or in a comment, is said with
//! its kind, its prefix and its length, and never with the key nor its line; a key whose line says
//! `ritsu: test secret` is not said. What `rulec check` prints for each rule of `tests/secrets`, in
//! English and in Japanese, is its golden file in `tests/golden/secrets`; `RULEC_BLESS=1 cargo
//! test --test secrets` writes them again. Each rule holds one key that is said: the one whose
//! line has no mark.

use rulec::i18n::{self, Lang};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// What `rulec check <rel>` prints, in `lang`.
fn printed(rel: &str, lang: Lang) -> String {
    let src = std::fs::read_to_string(root().join(rel)).unwrap();
    let lines: Vec<String> = src.lines().map(String::from).collect();
    i18n::with(lang, || {
        let r = rulec::report(&src, rel);
        format!("{}{}", rulec::findings_text(&r.diags, &lines), rulec::check_tail(&r.shadow, &r.diags, rel, 0))
    })
}

#[test]
fn a_key_is_said_without_the_key_and_a_test_secret_is_not_said() {
    // the materials' one fake key, put together here so that this file does not hold it whole
    let key = ["AIzaSyD-ritsu-fake-", "key-for-tests-000000"].concat();
    let mut names: Vec<String> = std::fs::read_dir(root().join("tests/secrets")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    let mut failures = Vec::new();
    for name in &names {
        let rel = format!("tests/secrets/{name}");
        let stem = name.trim_end_matches(".rule");
        for (lang, tag) in [(Lang::En, "en"), (Lang::Ja, "ja")] {
            let out = printed(&rel, lang);
            if out.contains(&key) {
                failures.push(format!("{rel} ({tag}) prints the key"));
            }
            let said = out.matches("[W901]").count();
            if said != 1 {
                failures.push(format!("{rel} ({tag}) says W901 {said} times, not once"));
            }
            if let Err(e) = ritsu_testkit::golden::check(&root().join(format!("tests/golden/secrets/{stem}.{tag}.txt")), &out) {
                failures.push(e);
            }
        }
    }
    assert_eq!(names.len(), 6, "{names:?}");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The JSON gives the place of the key, which the text does not frame.
#[test]
fn the_json_gives_the_place() {
    let src = std::fs::read_to_string(root().join("tests/secrets/W901_test_secret.rule")).unwrap();
    let d = i18n::with(Lang::En, || rulec::report(&src, "W901_test_secret.rule").diags);
    let w: Vec<_> = d.iter().filter(|d| d.code == "W901").collect();
    assert_eq!(w.len(), 1);
    let json = rulec::diag::render_json(w[0], "W901_test_secret.rule");
    assert!(json.contains("\"line\":4,\"column\":53"), "{json}");
    assert!(json.contains("\"spans\":[]"), "{json}");
}

/// A key on a line that another diagnostic frames is masked in the frame (ritsu-base's
/// `secrets::mask`), and W901 still says it once.
#[test]
fn a_key_on_a_framed_line_is_masked() {
    let key = ["AIzaSyD-ritsu-fake-", "key-for-tests-000000"].concat();
    // two rows that overlap (E105), the second with a key in its comment
    let src = format!("rule t(t) v1\n\ninputs\n  b(b) : bool\n\noutputs\n  r(r) : bool\n\ntable x(x)\npolicy unique\n| b     | -> r(r) : bool |\n| -     | true           |\n| false | false          |   # read with {key}\n");
    let lines: Vec<String> = src.lines().map(String::from).collect();
    for lang in [Lang::En, Lang::Ja] {
        let out = i18n::with(lang, || {
            let r = rulec::report(&src, "t.rule");
            rulec::findings_text(&r.diags, &lines)
        });
        assert!(out.contains("[E105]"), "the two rows overlap:\n{out}");
        assert!(out.contains("# read with AIza…"), "{out}");
        assert!(!out.contains(&key), "{out}");
        assert_eq!(out.matches("[W901]").count(), 1, "{out}");
    }
}
