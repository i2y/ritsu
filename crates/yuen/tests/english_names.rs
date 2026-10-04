//! Naming an artifact (DESIGN 2), in English: the twin of the second test of names.rs, with names
//! written in ASCII. The shared table of the form is names.rs's (it is ritsu-base's, and has the
//! lines of Japanese names that make it a test of Unicode).

use yuen::names::{self, Tool};

/// The shapes the table does not try: the path written from a `.req` in a directory, the
/// kinds a scope gathers, and what each tool takes.
#[test]
fn paths_from_a_directory_and_what_a_scope_gathers_in_english() {
    let w = |s: &str| {
        let toks = yuen::lex::naming_tokens(s).unwrap();
        names::read(&toks, s.chars().count() + 1).unwrap()
    };
    let (n, g) = names::resolve(&w("rulec \"../rules/shipping_fee.rule\" output shipping_fee"), "reqs", false).unwrap();
    assert_eq!(n.path, "rules/shipping_fee.rule");
    assert!(g.is_none());
    assert_eq!(n.text(), "rulec \"rules/shipping_fee.rule\" output shipping_fee");
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
    assert_eq!(names::resolve(&w("rulec \"x.rule\" \"output\" shipping_fee"), "", false).unwrap_err().code, "E012");
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

/// What one naming holds of another, written with ASCII names.
#[test]
fn containing_in_english() {
    let n = |s: &str| -> names::Name { names::parse_one(s).unwrap() };
    assert!(n("rulec \"a.rule\"").contains(&n("rulec \"a.rule\" output shipping_fee")));
    assert!(n("rulec \"a.rule\" enum Status").contains(&n("rulec \"a.rule\" enum Status value Open")));
    assert!(!n("rulec \"a.rule\"").contains(&n("rulec \"a.rule\"")));
    assert!(!n("rulec \"a.rule\"").contains(&n("koyomi \"a.rule\" date shipping_day")));
}
