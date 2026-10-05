//! What a rule names stays where the generated code writes it (ritsu's DESIGN 9.2).
//!
//! The head of every file `rulec gen` writes names what the rule is made from: the rule's file,
//! and each document it cites, with the document's path and address. They are text of the rule's,
//! and may hold a character some target ends a line at (U+2028 ends a line of TypeScript, `\r`
//! one of Python), which would end the comment and make the rest a line of the code; the head
//! writes each as its escape. The page `rulec gen` writes beside the code embeds the JavaScript,
//! head and all, in a `<script>` element, which a `</script` in an address would end, the rest
//! read as HTML; the page writes it `<\/script`.

use ritsu_testkit::TempDir;
use std::path::Path;
use std::process::Command;

/// What every line of the address that matters carries, to be found in the generated files.
const MARK: &str = "MARKED";

/// `rulec` in `dir`, in English.
fn rulec(dir: &Path, args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").current_dir(dir).args(args).output().unwrap();
    let said = String::from_utf8_lossy(&out.stdout).into_owned() + &String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "rulec {args:?}: {said}");
    said
}

/// A rule that cites a document at `url`, pinned, in `dir`.
fn rule_citing(dir: &Path, url: &str) {
    std::fs::write(dir.join("rates.txt"), "S60 990\n").unwrap();
    let src = format!(
        "rule postage(postage) v1\n\nsource rates = file \"rates.txt\" url \"{url}\"\n\ninputs\n  a(a) : bool\n\noutputs\n  x(x) : bool\n\ntable t(t1)  @rates\n| a | -> x |\n| - | true |\n"
    );
    std::fs::write(dir.join("postage.rule"), src).unwrap();
    rulec(dir, &["source", "pin", "postage.rule"]);
    rulec(dir, &["check", "postage.rule"]);
}

/// Every file under `dir`, with its text.
fn generated(dir: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            if e.path().is_dir() {
                todo.push(e.path());
            } else if let Ok(text) = std::fs::read_to_string(e.path()) {
                out.push((e.path().strip_prefix(dir).unwrap().display().to_string(), text));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn an_address_with_line_breaks_stays_in_the_head() {
    let t = TempDir::new("heads-breaks");
    rule_citing(t.path(), &format!("https://example.com/rates\u{2028}{MARK} after a line separator\r{MARK} after a CR\u{85}{MARK} after a NEL"));
    rulec(t.path(), &["gen", "postage.rule", "--out", "gen"]);
    let mut heads = 0;
    for (path, text) in generated(&t.path().join("gen")) {
        for line in text.split('\n').filter(|l| l.contains(MARK)) {
            if path.ends_with(".html") && line.starts_with("<p>") {
                continue; // the page's text, where a break is a character like any other
            }
            let l = line.trim_start();
            assert!(["//", "#", "--"].iter().any(|m| l.starts_with(m)), "{path}: the address is out of the head: {line:?}");
            assert!(!l.contains(['\r', '\u{85}', '\u{2028}', '\u{2029}']) && l.contains("\\u{2028}") && l.contains("\\r"), "{path}: {line:?}");
            heads += 1;
        }
    }
    assert!(heads >= 12, "the head of a file of each target names the address: {heads}");
}

#[test]
fn an_address_does_not_end_the_script_of_the_page() {
    let t = TempDir::new("heads-script");
    rule_citing(t.path(), "https://example.com/rates?</script><img src=x onerror=alert(1)><!--");
    rulec(t.path(), &["gen", "postage.rule", "--out", "gen"]);
    let pages: Vec<(String, String)> = generated(&t.path().join("gen")).into_iter().filter(|(p, _)| p.ends_with("_page.html")).collect();
    assert!(!pages.is_empty(), "rulec gen writes a page");
    for (path, page) in pages {
        let script = page.split("<script type=\"module\">").nth(1).unwrap_or_else(|| panic!("{path} has no script"));
        let (inside, after) = script.split_once("</script>").unwrap();
        assert_eq!(after, "\n</body>\n</html>\n", "{path}: the script ends before its end");
        assert!(!inside.to_ascii_lowercase().contains("</script") && !inside.contains("<!--"), "{path}");
        assert!(inside.contains("https://example.com/rates?<\\/script><img src=x onerror=alert(1)><\\!--"), "{path}: the address, escaped");
    }
}
