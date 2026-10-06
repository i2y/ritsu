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
            // each break is written `U+XXXX`: no `\`, no `\u`, the same in every target
            assert!(!l.contains(['\r', '\u{85}', '\u{2028}', '\u{2029}']) && l.contains("U+2028") && l.contains("U+000D"), "{path}: {line:?}");
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

/// A cited address, and the rule's own file name, may hold text a target reads as a Unicode
/// escape before it reads the comment the head writes them in: Java (JLS 3.3) translates a
/// `\uXXXX` anywhere in the source, so a `\u000a` would become a line break that ends the head's
/// comment, and the earlier `\u{XXXX}` the head wrote for a real break was a malformed escape that
/// stopped javac. The head now writes a break `U+XXXX` (no `\u`) and doubles the backslashes the
/// text holds, so the generated Java compiles and nothing leaves the head (ritsu's DESIGN 9.2).
#[test]
fn a_cited_address_does_not_break_the_java_head() {
    use ritsu_testkit::{Need, ready};
    // the rule's file name and the cited URL each hold a real break (U+2028), a CR and a NEL, and
    // the six characters `\`, `u`, `0`, `0`, `0`, `a`, which Java would read as a line break
    let esc = "\\u000a";
    let hazard = |s: &str| format!("{s}{MARK}1\u{2028}{MARK}2\r{MARK}3\u{85}{MARK}4{esc}{MARK}5");
    let t = TempDir::new("heads-java");
    let name = format!("{}.rule", hazard("postage"));
    std::fs::write(t.path().join("rates.txt"), "S60 990\n").unwrap();
    let url = format!("https://example.com/{}", hazard("rates"));
    let src = format!(
        "rule postage(postage) v1\n\nsource rates = file \"rates.txt\" url \"{url}\"\n\ninputs\n  a(a) : bool\n\noutputs\n  x(x) : bool\n\ntable t(t1)  @rates\n| a | -> x |\n| - | true |\n"
    );
    std::fs::write(t.path().join(&name), src).unwrap();
    rulec(t.path(), &["source", "pin", &name]);
    rulec(t.path(), &["check", &name]);
    rulec(t.path(), &["gen", &name, "--out", "gen"]);

    // the generated Java: the head names the file and the URL, and no line after the head is code
    // the head's text leaked into — every line that carries the mark is a comment.
    let java: Vec<(String, String)> = generated(&t.path().join("gen")).into_iter().filter(|(p, _)| p.ends_with(".java")).collect();
    assert!(!java.is_empty(), "rulec gen writes Java");
    for (path, text) in &java {
        for line in text.split('\n').filter(|l| l.contains(MARK)) {
            let l = line.trim_start();
            assert!(l.starts_with("//"), "{path}: the head's text is out of the comment: {line:?}");
            assert!(!l.contains(['\r', '\u{85}', '\u{2028}', '\u{2029}']), "{path}: a raw break is left: {line:?}");
            // the break is written U+XXXX, and the `\u000a` the text held is doubled so javac
            // reads no escape
            assert!(l.contains("U+2028") && l.contains("U+000D") && l.contains("\\\\u000a"), "{path}: {line:?}");
        }
    }
    // the module class is there, so the head did not swallow the class declaration
    assert!(java.iter().any(|(_, t)| t.contains("public final class Postage")), "the class is written");

    // javac reads a Unicode escape in a comment, so it is the one that proves the head is safe
    if !ready(Need::Java, || Command::new("javac").arg("-version").output().is_ok(), "javac が無いので Java の検査を飛ばした") {
        return;
    }
    let dir = t.path().join("gen/java");
    let mut args: Vec<String> = rulec::backend::JAVAC_FLAGS.iter().map(|s| s.to_string()).collect();
    args.push("Postage.java".into());
    args.push("PostageRunner.java".into());
    let o = Command::new("javac").current_dir(&dir).args(["-Xlint:all"]).args(&args).output().expect("javac を起動できない");
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(o.status.success(), "生成した Java が通らない:\n{err}");
    assert!(err.trim().is_empty(), "生成した Java が警告を出している:\n{err}");
}

/// What a rule writes in the body of the generated Java stays what it is (ritsu's DESIGN 9.2):
/// a table's name and a row's label in the comment over each branch, a string to match and a
/// string to give back, and an enum's value. rulec's lexer takes a `\` in a name and anything but
/// `"` in a string, so each can hold the six characters `\u000a` or `"`, which javac reads
/// before it reads the comment or the literal (JLS 3.3): a `\u000a` would end the comment, a
/// `"` would close the literal. The comments double their backslashes, the literals and the
/// record's keys were already escaped, and the generated Java compiles with no warning and gives
/// back each string as the rule wrote it.
#[test]
fn rule_strings_stay_what_they_are_in_the_java() {
    use ritsu_testkit::{Need, ready};
    let (u, q) = ("\\u000a", "\\u0022");
    let t = TempDir::new("heads-java-body");
    let src = format!(
        "rule label(label) v1\n\nenum k{u}ind(kind) = c{u}d{q}e(cd) | plain(plain)\n\ninputs\n  s(s) : string\n  k(k) : k{u}ind\n\noutputs\n  out(out) : string\n\ntable t{u}x{q}(t1)\npolicy first\n| s | k | -> out |\nr{u}1{q} | starts_with \"a{u}b{q}//c\u{2028}\" | c{u}d{q}e | \"o{u}p{q}q//r\u{2028}z\" |\n| - | - | \"plain\" |\n"
    );
    std::fs::write(t.path().join("label.rule"), src).unwrap();
    rulec(t.path(), &["check", "label.rule"]);
    rulec(t.path(), &["gen", "label.rule", "--out", "gen"]);
    let dir = t.path().join("gen/java");
    let java = std::fs::read_to_string(dir.join("Label.java")).unwrap();
    // every `\u` in the file is preceded by an odd number of `\`, so javac reads no escape
    let cs: Vec<char> = java.chars().collect();
    for i in 0..cs.len() {
        if cs[i] == '\\' && cs.get(i + 1) == Some(&'u') {
            let before = cs[..i].iter().rev().take_while(|c| **c == '\\').count();
            assert!(before % 2 == 1, "Label.java: a `\\u` at {i} that javac would read: {:?}", cs[i.saturating_sub(40)..(i + 10).min(cs.len())].iter().collect::<String>());
        }
    }
    if !ready(Need::Java, || Command::new("javac").arg("-version").output().is_ok(), "javac が無いので Java の検査を飛ばした") {
        return;
    }
    let mut args: Vec<String> = rulec::backend::JAVAC_FLAGS.iter().map(|s| s.to_string()).collect();
    args.extend(["Label.java".to_string(), "LabelRunner.java".to_string()]);
    let o = Command::new("javac").current_dir(&dir).args(["-Xlint:all"]).args(&args).output().expect("javac を起動できない");
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(o.status.success(), "生成した Java が通らない:\n{err}");
    assert!(err.trim().is_empty(), "生成した Java が警告を出している:\n{err}");
    // the runner gives back each string as the rule wrote it
    let input = format!("{{\"s\":\"a\\\\u000ab\\\\u0022//c\u{2028}tail\",\"k\":\"c\\\\u000ad\\\\u0022e\"}}\n");
    let mut child = Command::new("java")
        .current_dir(&dir)
        .args(["-cp", "classes", "LabelRunner"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("java を起動できない");
    std::io::Write::write_all(&mut child.stdin.take().unwrap(), input.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    let line = String::from_utf8_lossy(&out.stdout).to_string();
    let rec = rulec::json::parse(line.trim()).unwrap_or_else(|e| panic!("the record is not JSON: {e}: {line}"));
    let observed = rulec::json::members_of(&rec, "observed").unwrap();
    assert_eq!(observed["out"].as_str(), Some(format!("o{u}p{q}q//r\u{2028}z").as_str()), "{line}");
    let trace = rec.get("trace").and_then(|t| t.as_arr()).and_then(|a| a.first()).cloned().unwrap_or_else(|| panic!("no trace: {line}"));
    assert_eq!(trace.get("table").and_then(|v| v.as_str()), Some(format!("t{u}x{q}").as_str()), "{line}");
    assert_eq!(trace.get("label").and_then(|v| v.as_str()), Some(format!("r{u}1{q}").as_str()), "{line}");
}
