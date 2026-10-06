//! The port of maps (ritsu's DESIGN 16.8, 16.9), as sakai answers it: a map's contexts and
//! relationships, and the context each file of the project belongs to, decided as the check
//! decides who owns what. The answers for the examples `shop` and `webshop` (and their Japanese
//! twins) are golden files in `tests/golden/maps/`; `SAKAI_BLESS=1 cargo test` writes them again.
//! A context file is no map, and a map whose names or owners do not pass says why.

mod common;

use ritsu_ports::Maps;
use sakai::ports::Engine;
use std::path::Path;

/// What the port says of a map, and of every file under its example.
fn answers(example: &str, map: &str) -> String {
    let root = std::fs::canonicalize(example).unwrap();
    let m = Engine.map(&root, map).unwrap().unwrap();
    let mut out = format!("map {}\ncontexts {}\n", m.file, m.contexts.join(", "));
    for r in &m.relationships {
        out.push_str(&format!("  {} {} {}{}  ({}:{})\n", r.from, r.words, r.to, if r.separate { " [separate]" } else { "" }, r.file, r.line));
    }
    let mut files = Vec::new();
    ritsu_base::paths::walk(&root, ".", &[], &mut files);
    files.sort();
    out.push_str("files\n");
    for f in files {
        let c = Engine.context_of(&root, map, &f).unwrap();
        out.push_str(&format!("  {f} -> {}\n", c.as_deref().unwrap_or("-")));
    }
    out
}

#[test]
fn the_maps_of_the_examples_and_where_their_files_belong() {
    let mut failures = Vec::new();
    for (example, map, stem) in [
        ("examples/shop", "shop.ctx", "shop"),
        ("examples/shop.ja", "通販.ctx", "通販"),
        ("examples/webshop", "webshop.ctx", "webshop"),
        ("examples/webshop.ja", "ネットショップ.ctx", "ネットショップ"),
    ] {
        if let Some(f) = common::golden(&format!("tests/golden/maps/{stem}.txt"), &answers(example, map)) {
            failures.push(f);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_context_file_is_no_map_and_a_map_that_does_not_pass_says_why() {
    let root = std::fs::canonicalize("examples/webshop").unwrap();
    assert_eq!(Engine.map(&root, "contexts/payments.ctx").unwrap(), None);
    assert_eq!(Engine.context_of(&root, "contexts/payments.ctx", "payments/api/payments.yaml").unwrap(), None);
    // a file no context owns: E101, said as the check says it
    let d = common::TempDir::new("maps");
    common::copy_dir(Path::new("examples/webshop"), d.path());
    std::fs::create_dir_all(d.path().join("elsewhere")).unwrap();
    std::fs::write(d.path().join("elsewhere/api.yaml"), "openapi: 3.1.0\ninfo:\n  title: Elsewhere\n  version: 1.0.0\n").unwrap();
    let said = Engine.map(d.path(), "webshop.ctx").unwrap_err();
    assert_eq!(said.iter().map(|s| (s.code.as_str(), s.file.as_str())).collect::<Vec<_>>(), [("E101", "elsewhere/api.yaml")]);
    assert!(Engine.context_of(d.path(), "webshop.ctx", "payments/api/payments.yaml").is_err());
    // a map that does not read
    std::fs::write(d.path().join("webshop.ctx"), "map Webshop(webshop) v1\nuse context \"contexts/nowhere.ctx\"\ncovers \".\"\n").unwrap();
    let said = Engine.map(d.path(), "webshop.ctx").unwrap_err();
    assert!(!said.is_empty() && said.iter().all(|s| s.code.starts_with('E')), "{said:?}");
    // a file that is not there
    assert!(Engine.map(d.path(), "no-such.ctx").is_err());
}

/// A file outside the map's scope belongs to no context, even under a directory a context owns.
#[test]
fn a_file_outside_the_scope_is_in_no_context() {
    let d = common::TempDir::new("maps-scope");
    common::copy_dir(Path::new("examples/webshop"), d.path());
    let map = std::fs::read_to_string(d.path().join("webshop.ctx")).unwrap();
    std::fs::write(d.path().join("webshop.ctx"), map.replace("covers \".\"\n", "covers \".\"\nexcept \"payments/events\"\n")).unwrap();
    assert_eq!(Engine.context_of(d.path(), "webshop.ctx", "payments/api/payments.yaml").unwrap().as_deref(), Some("Payments"));
    assert_eq!(Engine.context_of(d.path(), "webshop.ctx", "payments/events/payments.yaml").unwrap(), None);
}
