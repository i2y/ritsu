//! Naming an artifact (DESIGN 2, PLAN B.3). The table `tests/fixtures/naming.tsv` is shared
//! with sakai: every line that gives JSON gives that JSON to the letter, and every line that
//! gives an error is an error here, with the code its reason stands for.

use yurai::names::{self, Name, Tool};

/// The code each reason of the table stands for (DESIGN 2.6). A reason not in this list must
/// still be an error.
fn code_of(reason: &str) -> Option<&'static str> {
    match reason {
        "unknown tool" | "a tool written as a string" => Some("E011"),
        "absolute path" | "outside the root" | "an empty path" => Some("E013"),
        "dandori has no kinds yet"
        | "value only right after enum"
        | "method only right after service"
        | "one child at most"
        | "chobo has no nested kinds"
        | "unknown kind for koyomi"
        | "a kind written as a string"
        | "a kind without a name" => Some("E012"),
        "a full-width space outside a string" => Some("E001"),
        r#"only \" and \\ are escapes"# => Some("E001"),
        _ => None,
    }
}

#[test]
fn every_line_of_the_shared_table() {
    let table = std::fs::read_to_string("tests/fixtures/naming.tsv").unwrap();
    let mut failures = Vec::new();
    let (mut ok, mut errors) = (0, 0);
    for (i, line) in table.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let Some((naming, want)) = line.split_once('\t') else {
            failures.push(format!("line {}: no tab", i + 1));
            continue;
        };
        let got = names::parse_one(naming);
        match (want.strip_prefix("ERROR: "), got) {
            (None, Ok(n)) => {
                let json = serde_json::to_string(&n.to_json()).unwrap();
                if json != want {
                    failures.push(format!("line {}: {naming}\n  want {want}\n  got  {json}", i + 1));
                }
                // The text, read again, is the same naming.
                match names::parse_one(&n.text()) {
                    Ok(again) if again == n => {}
                    other => failures.push(format!("line {}: the text {} reads back as {other:?}", i + 1, n.text())),
                }
                ok += 1;
            }
            (None, Err(e)) => failures.push(format!("line {}: {naming}: want {want}, got {} {}", i + 1, e.code, e.msg.en)),
            (Some(reason), Ok(n)) => failures.push(format!("line {}: {naming}: want an error ({reason}), got {}", i + 1, serde_json::to_string(&n.to_json()).unwrap())),
            (Some(reason), Err(e)) => {
                if let Some(code) = code_of(reason)
                    && code != e.code
                {
                    failures.push(format!("line {}: {naming}: {reason} is {code}, got {} {}", i + 1, e.code, e.msg.en));
                }
                errors += 1;
            }
        }
    }
    assert!(ok == 21 && errors == 15, "{ok} lines of JSON and {errors} of errors");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The shapes the table does not try: the path written from a `.req` in a directory, the
/// kinds a scope gathers, and what each tool takes.
#[test]
fn paths_from_a_directory_and_what_a_scope_gathers() {
    let w = |s: &str| {
        let toks = yurai::lex::naming_tokens(s).unwrap();
        names::read(&toks, s.chars().count() + 1).unwrap()
    };
    let (n, g) = names::resolve(&w("rulec \"../rules/送料.rule\" output 送料"), "reqs", false).unwrap();
    assert_eq!(n.path, "rules/送料.rule");
    assert!(g.is_none());
    assert_eq!(n.text(), "rulec \"rules/送料.rule\" output 送料");
    let (n, g) = names::resolve(&w("proto \"order.proto\" service OrderService method"), "", true).unwrap();
    assert_eq!(n.items, vec![("service".to_string(), "OrderService".to_string())]);
    assert_eq!(g.unwrap().text, "method");
    let e = names::resolve(&w("proto \"order.proto\" method"), "", true).unwrap_err();
    assert_eq!(e.code, "E012");
    let (n, g) = names::resolve(&w("file \"src/\""), "", true).unwrap();
    assert_eq!((n.path.as_str(), g), ("src", None));
    let (n, _) = names::resolve(&w("file \".\""), "", true).unwrap();
    assert_eq!(n.path, ".");
    assert_eq!(names::resolve(&w("file \"\""), "", false).unwrap_err().code, "E013");
    assert_eq!(names::resolve(&w("file src/app.py"), "", false).unwrap_err().code, "E013");
    assert_eq!(names::resolve(&w("file"), "", false).unwrap_err().code, "E013");
    assert_eq!(names::resolve(&w("rulec \"x.rule\" output"), "", false).unwrap_err().code, "E012");
    assert_eq!(names::resolve(&w("rulec \"x.rule\" \"output\" 送料"), "", false).unwrap_err().code, "E012");
    assert_eq!(names::resolve(&w("dir \"src\""), "", false).unwrap_err().code, "E011");
    assert_eq!(names::resolve(&w("\"rulec\" \"x.rule\""), "", false).unwrap_err().code, "E011");
    // Every tool reads every kind of the union (DESIGN 2.3), and nests only where it does.
    for t in Tool::ALL {
        for (k, children) in t.kinds() {
            let s = format!("{} \"x\" {k} a", t.word());
            assert!(names::parse_one(&s).is_ok(), "{s}");
            for c in *children {
                let s = format!("{} \"x\" {k} a {c} b", t.word());
                assert!(names::parse_one(&s).is_ok(), "{s}");
            }
        }
    }
}

#[test]
fn containing() {
    let n = |s: &str| -> Name { names::parse_one(s).unwrap() };
    assert!(n("rulec \"a.rule\"").contains(&n("rulec \"a.rule\" output x")));
    assert!(n("rulec \"a.rule\" enum E").contains(&n("rulec \"a.rule\" enum E value V")));
    assert!(n("proto \"o.proto\" service S").contains(&n("proto \"o.proto\" service S method M")));
    assert!(!n("proto \"o.proto\" service S").contains(&n("proto \"o.proto\" service T method M")));
    assert!(!n("rulec \"a.rule\"").contains(&n("rulec \"a.rule\"")));
    assert!(!n("rulec \"a.rule\"").contains(&n("koyomi \"a.rule\" date x")));
}
