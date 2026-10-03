//! The tables copied from rulec and dandori (PLAN C.10) are theirs, word for word: rulec's
//! through its registry of backends, dandori's read from its source, where they are private.

use ritsu_emit::copies;
use ritsu_emit::words::Words;
use std::collections::BTreeSet;

fn set(ws: impl IntoIterator<Item = &'static str>) -> BTreeSet<&'static str> {
    ws.into_iter().collect()
}

#[test]
fn rulecs_tables_are_rulecs() {
    let ours = copies::rulec::BACKENDS;
    let theirs = rulec::backend::ALL;
    assert_eq!(ours.iter().map(|b| b.id).collect::<Vec<_>>(), theirs.iter().map(|b| b.id).collect::<Vec<_>>());
    for (o, t) in ours.iter().zip(theirs) {
        assert_eq!(set(o.reserved.iter()), set(t.reserved.iter().copied()), "{}: reserved", o.id);
        assert_eq!(set(o.globals.iter()), set(t.globals.iter().copied()), "{}: globals", o.id);
        assert_eq!(set(o.modules.iter()), set(t.modules.iter().copied()), "{}: modules", o.id);
    }
}

/// The words of `const <name>: &[&str] = &[…];` in a file of dandori's source.
fn dandori_const(file: &str, name: &str) -> BTreeSet<String> {
    let src = std::fs::read_to_string(format!("../dandori/src/{file}")).unwrap();
    let start = src.find(&format!("const {name}: &[&str] = &[")).unwrap_or_else(|| panic!("{file} has no {name}"));
    let body = &src[start..];
    let body = &body[body.find("= &[").unwrap() + 4..body.find("];").unwrap()];
    body.split(',').map(|w| w.trim()).filter(|w| !w.is_empty()).map(|w| w.trim_matches('"').to_string()).collect()
}

#[test]
fn dandoris_tables_are_dandoris() {
    let words = |w: Words| w.iter().map(String::from).collect::<BTreeSet<String>>();
    let list = |w: &[&str]| w.iter().map(|s| s.to_string()).collect::<BTreeSet<String>>();
    assert_eq!(words(copies::dandori::PY_RESERVED), dandori_const("temporal_py.rs", "PY_RESERVED"));
    assert_eq!(words(copies::dandori::GO_RESERVED), dandori_const("temporal_go.rs", "GO_RESERVED"));
    assert_eq!(list(copies::dandori::GO_EXPORTED), dandori_const("temporal_go.rs", "GO_EXPORTED"));
    assert_eq!(list(copies::dandori::TS_GLOBALS), dandori_const("temporal.rs", "TS_GLOBALS"));
}
