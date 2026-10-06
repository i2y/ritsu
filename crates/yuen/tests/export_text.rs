//! The text of a `.req` stays inside the values of what `yuen export` writes (ritsu's DESIGN 9.2):
//! a requirement's text, a role's, a reason, the file's description, holding the characters that
//! end a line somewhere (`\r`, U+0085, U+2028, U+2029; a `.req` string cannot hold `\n`), the end
//! of a script (`</script>`), of a comment (`-->`), of a CDATA section (`]]>`) and YAML's document
//! marker (`---`), comes back whole from the value it is written in, and adds no element, no record
//! and no line. The same project with plain words is the yardstick: the two exports have the same
//! elements and records in the same order.

mod common;

use common::TempDir;
use std::path::Path;

/// What every string of the project holds, on top of its words.
const ODD: &str = "a\r b\u{85}c\u{2028}d\u{2029}e</script>f---g]]>h<!--i-->j";

/// The project, its strings ending with `tail`, its waivers approved by `yuen review`.
fn project(tail: &str) -> TempDir {
    let t = TempDir::new("export-text");
    t.write(
        "payment.req",
        format!(
            "requirements payment v1\ndescription \"A test material {tail}\"\n\nrole accounting \"Decides the rules of payment {tail}\"\n\nrequirement payment_day\n  text \"Closes on the 20th {tail}\"\n  owner accounting\n  decided 2026-10-03 by accounting \"decided for the example {tail}\"\n  not satisfied \"left out in this example {tail}\"\n  not verified \"left out in this example\"\n"
        ),
    );
    let r = common::yuen(t.path(), &["review", ".", "--all", "--by", "accounting", "--date", "2026-10-03"]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    let r = common::yuen(t.path(), &["check", "."]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    t
}

fn export(dir: &Path, args: &[&str]) -> String {
    let mut a = vec!["export"];
    a.extend_from_slice(args);
    a.push(".");
    let r = common::yuen(dir, &a);
    assert_eq!(r.code, 0, "{}", r.stderr);
    r.stdout
}

/// The elements of an XML document in order, as their tags (`SPEC-OBJECT`, `/SPEC-OBJECT`), read
/// with attribute values skipped whole: a `<` or a `>` inside a value is no tag.
fn tags(xml: &str) -> Vec<String> {
    let c: Vec<char> = xml.chars().collect();
    let (mut out, mut i) = (Vec::new(), 0);
    while i < c.len() {
        if c[i] != '<' {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        let mut quote: Option<char> = None;
        while j < c.len() {
            match (quote, c[j]) {
                (None, '"' | '\'') => quote = Some(c[j]),
                (Some(q), x) if x == q => quote = None,
                (None, '>') => break,
                _ => {}
            }
            j += 1;
        }
        let inside: String = c[i + 1..j].iter().collect();
        out.push(inside.split(|ch: char| ch.is_whitespace() || ch == '>').next().unwrap_or("").to_string());
        i = j + 1;
    }
    out
}

/// XML's escapes undone.
fn unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#13;", "\r").replace("&#10;", "\n").replace("&#9;", "\t").replace("&amp;", "&")
}

/// What a PROV-N document is outside its string literals, and the literals, unescaped.
fn provn_parts(n: &str) -> (String, Vec<String>) {
    let (mut outside, mut strings) = (String::new(), Vec::new());
    let mut it = n.chars();
    while let Some(ch) = it.next() {
        if ch != '"' {
            outside.push(ch);
            continue;
        }
        let mut s = String::new();
        while let Some(x) = it.next() {
            match x {
                '"' => break,
                '\\' => match it.next() {
                    Some('n') => s.push('\n'),
                    Some('r') => s.push('\r'),
                    Some('t') => s.push('\t'),
                    Some(y) => s.push(y),
                    None => {}
                },
                y => s.push(y),
            }
        }
        strings.push(s);
        outside.push_str("\"…\"");
    }
    (outside, strings)
}

#[test]
fn the_text_stays_inside_the_values_of_reqif_and_prov() {
    let odd = project(ODD);
    let plain = project("plain");
    let time = "2026-10-03T09:00:00+09:00";

    // ReqIF: the same elements, and every string whole in its value
    let (x, y) = (export(odd.path(), &["reqif", "--time", time]), export(plain.path(), &["reqif", "--time", time]));
    assert_eq!(tags(&x), tags(&y), "the strings add or remove no element");
    for raw in ["</script>", "<!--", "\r"] {
        assert!(!x.contains(raw), "{raw:?} is written as it is:\n{x}");
    }
    let back = unescape(&x);
    for s in [format!("Closes on the 20th {ODD}"), format!("decided for the example {ODD}"), format!("left out in this example {ODD}")] {
        assert!(back.contains(&s), "{s:?} does not come back whole from the ReqIF");
    }

    // PROV-N: the same records, line for line, and the strings only inside their literals
    let (n, m) = (export(odd.path(), &["prov"]), export(plain.path(), &["prov"]));
    let ((outside, strings), (plain_outside, _)) = (provn_parts(&n), provn_parts(&m));
    assert_eq!(n.lines().count(), m.lines().count(), "the strings add no line");
    let skeleton = |s: &str| s.lines().map(|l| l.split('(').next().unwrap_or("").to_string()).collect::<Vec<_>>();
    assert_eq!(skeleton(&outside), skeleton(&plain_outside), "the strings add or remove no record");
    for piece in ["</script>", "---", "]]>", "\r", "\u{85}", "\u{2028}", "\u{2029}"] {
        assert!(!outside.contains(piece), "{piece:?} is outside a string literal of the PROV-N");
    }
    assert!(strings.iter().any(|s| *s == format!("Closes on the 20th {ODD}")), "the text does not come back whole from the PROV-N");

    // PROV-JSON: one document, the text whole in one value
    let j: serde_json::Value = serde_json::from_str(&export(odd.path(), &["prov", "--format", "json"])).unwrap();
    let mut values = Vec::new();
    fn walk(v: &serde_json::Value, out: &mut Vec<String>) {
        match v {
            serde_json::Value::String(s) => out.push(s.clone()),
            serde_json::Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            serde_json::Value::Object(o) => o.values().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    walk(&j, &mut values);
    assert!(values.iter().any(|s| *s == format!("Closes on the 20th {ODD}")), "the text does not come back whole from the PROV-JSON");

    // and what reads them for real, when it is at hand: xmllint, and the Python `prov`
    if common::on_path("xmllint") {
        let f = odd.write("out.reqif", x.as_bytes());
        let o = std::process::Command::new("xmllint").arg("--noout").arg(&f).output().unwrap();
        assert!(o.status.success(), "xmllint: {}", String::from_utf8_lossy(&o.stderr));
        println!("compared: xmllint reads the ReqIF with the odd strings");
    }
    if let Some(py) = common::python("the PROV reading of odd strings") {
        let f = odd.write("out.provn", n.as_bytes());
        let script = "import sys\nfrom prov.model import ProvDocument\nd = ProvDocument.deserialize(sys.argv[1], format='provn', profile='strict')\nprint(len(list(d.get_records())))\n";
        let o = std::process::Command::new(&py).args(["-c", script]).arg(&f).output().unwrap();
        assert!(o.status.success(), "prov cannot read it:\n{}", String::from_utf8_lossy(&o.stderr));
        println!("compared: prov reads the PROV-N with the odd strings ({} records)", String::from_utf8_lossy(&o.stdout).trim());
    }
}
