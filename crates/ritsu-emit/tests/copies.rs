//! The tables rulec and dandori hold the names of their generated code to (`copies`), held to
//! `tests/golden/copies.txt`. The golden file was written while the tables were still compared,
//! word for word, with rulec's registry of backends and with the constants of dandori's source
//! (PLAN C.10, C.11). The two tools read the tables from here since, so a comparison with them
//! would compare the tables with themselves; a word added or taken away changes what they
//! generate and refuse, and shows here as a change of the golden first.

use ritsu_emit::copies;
use std::collections::BTreeSet;

/// Every table, a line each: the tool, the table, how many words, and the words in order.
fn rendered() -> String {
    let mut o = String::new();
    let mut line = |tool: &str, table: &str, ws: Vec<&'static str>| {
        let s: BTreeSet<&str> = ws.into_iter().collect();
        o.push_str(&format!("{tool} {table} ({}): {}\n", s.len(), s.into_iter().collect::<Vec<_>>().join(" ")));
    };
    for b in copies::rulec::BACKENDS {
        line("rulec", &format!("{} reserved", b.id), b.reserved.iter().collect());
        line("rulec", &format!("{} globals", b.id), b.globals.iter().collect());
        line("rulec", &format!("{} modules", b.id), b.modules.iter().collect());
    }
    line("dandori", "PY_RESERVED", copies::dandori::PY_RESERVED.iter().collect());
    line("dandori", "GO_RESERVED", copies::dandori::GO_RESERVED.iter().collect());
    line("dandori", "GO_EXPORTED", copies::dandori::GO_EXPORTED.to_vec());
    line("dandori", "TS_GLOBALS", copies::dandori::TS_GLOBALS.to_vec());
    o
}

/// The tables as the golden file has them: a word added or taken away changes what rulec and
/// dandori generate and refuse, so it is read here first.
#[test]
fn the_tables_are_the_golden() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    ritsu_testkit::golden(root.join("tests/golden/copies.txt"), &rendered());
}
