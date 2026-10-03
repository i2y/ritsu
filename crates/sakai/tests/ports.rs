//! ritsu's ports, as sakai answers them (ritsu's DESIGN 3.2, PLAN D.2): a context file's context
//! and terms as items, and the namings a `.ctx` writes as references — a short name of the
//! context's published language named in full, as `sakai api` names it.

use ritsu_ports::{Items, References};
use sakai::ports::Engine;
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/通販")
}

#[test]
fn a_context_and_its_terms_are_its_items() {
    let items = Engine.items(&root(), "contexts/在庫.ctx").unwrap();
    assert_eq!(items[0].naming.text(), "sakai \"contexts/在庫.ctx\" context 在庫");
    assert!(items[0].text.starts_with("context 在庫(inventory) v1"), "{}", items[0].text);
    let terms: Vec<&str> = items.iter().filter(|i| i.kind() == "term").map(|i| i.name()).collect();
    assert_eq!(terms, ["引当", "在庫切れ", "梱包の状態"]);
    let short = items.iter().find(|i| i.name() == "在庫切れ").unwrap();
    assert_eq!(short.text, "在庫切れ \"押さえようとした数が棚に無いこと\"\nmeans enum Stock value STOCK_SHORT");
    assert_eq!(short.lines.1, short.lines.0 + 1);
    // a map holds neither a context nor a term
    assert!(Engine.items(&root(), "通販.ctx").unwrap().is_empty());
}

/// What each term means, named in full: what `sakai api` says, term by term, for every context
/// the example's map reads.
#[test]
fn what_a_term_means_is_named_as_sakai_api_names_it() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_sakai")).current_dir(root()).args(["api", "通販.ctx", "--root", "."]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let api: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let mut means = 0;
    for c in api["contexts"].as_array().unwrap() {
        let file = c["file"].as_str().unwrap();
        let refs = Engine.references(&root(), file).unwrap();
        let ours: Vec<String> = refs.iter().filter(|r| r.how == "means").map(|r| r.target.text()).collect();
        let theirs: Vec<String> = c["terms"].as_array().unwrap().iter().flat_map(|t| t["means"].as_array().unwrap().iter().map(|m| m["text"].as_str().unwrap().to_string())).collect();
        assert_eq!(ours, theirs, "{file}");
        means += ours.len();
    }
    assert!(means >= 5, "{means}");
    let inv = Engine.references(&root(), "contexts/在庫.ctx").unwrap();
    let has = |how: &str, text: &str| inv.iter().any(|r| r.how == how && r.target.text() == text);
    assert!(has("owns", "file \"inventory\""), "{inv:?}");
    assert!(has("published language", "proto \"proto/warehouse/v1/stock.proto\""), "{inv:?}");
    assert!(has("open host service", "proto \"proto/warehouse/v1/stock.proto\" service StockService"), "{inv:?}");
    assert!(has("generated", "file \"py/warehouse/v1\""), "{inv:?}");
    let map = Engine.references(&root(), "通販.ctx").unwrap();
    assert!(map.iter().any(|r| r.how == "use context" && r.target.text() == "sakai \"contexts/在庫.ctx\""), "{map:?}");
    for r in inv.iter().chain(&map) {
        assert_eq!(ritsu_base::naming::parse_one(&r.target.text()).map(|n| n.text()).ok(), Some(r.target.text()));
    }
}
