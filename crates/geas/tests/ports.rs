//! ritsu's ports, as geas answers them (ritsu's DESIGN 3.2, PLAN D.2): the claims of a spec with
//! their lines and steps, the record `geas map` keeps, read where geas writes it, and a spec's
//! claims as items by ritsu's naming. Nothing is run.

use geas::ports::Engine;
use ritsu_ports::{Claims, Items};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn a_specs_claims_are_its_claims_with_their_steps() {
    let file = root().join("examples/greeter/greeter.geas");
    let src = std::fs::read_to_string(&file).unwrap();
    let spec = geas::parse::parse(&src).unwrap();
    let claims = Engine.claims(&file).unwrap();
    assert_eq!(claims.iter().map(|c| (c.name.clone(), c.line)).collect::<Vec<_>>(), spec.claims.iter().map(|c| (c.name.clone(), c.pos.line)).collect::<Vec<_>>());
    for (c, s) in claims.iter().zip(&spec.claims) {
        assert!(!c.steps.is_empty(), "{}", c.name);
        assert_eq!(c.steps.iter().filter(|l| l.starts_with("when ")).count(), s.steps.iter().filter(|x| matches!(x, geas::model::Step::When { .. })).count(), "{}", c.name);
        assert!(c.steps.iter().all(|l| !l.starts_with('#') && l.trim() == l.as_str()), "{:?}", c.steps);
    }
    // a spec that does not read says why
    let bad = Engine.claims(&root().join("tests/specs/E005-then-before-when.geas")).unwrap_err();
    assert!(bad.iter().any(|s| s.code == "E005"), "{bad:?}");
}

#[test]
fn the_record_is_read_where_geas_map_writes_it() {
    let dir = ritsu_testkit::TempDir::new("ports-record");
    let spec = dir.path().join("greeter.geas");
    std::fs::copy(root().join("examples/greeter/greeter.geas"), &spec).unwrap();
    assert_eq!(Engine.map_record(&spec).unwrap(), None, "no record yet");
    let text = std::fs::read_to_string(root().join("tests/golden/en/map/greeter.map.jsonl")).unwrap();
    std::fs::create_dir_all(dir.path().join(".geas")).unwrap();
    std::fs::write(dir.path().join(".geas/greeter.map.jsonl"), &text).unwrap();
    let got = Engine.map_record(&spec).unwrap().unwrap();
    let want = geas::map::Record::parse(&text).unwrap();
    assert_eq!(got.spec, want.spec);
    assert_eq!(got.claims.iter().map(|c| (c.name.clone(), c.status.clone(), c.targets.clone())).collect::<Vec<_>>(), want.claims.iter().map(|c| (c.name.clone(), c.status.clone(), c.targets.clone())).collect::<Vec<_>>());
    assert_eq!(got.files.iter().map(|f| f.path.clone()).collect::<Vec<_>>(), want.files.iter().map(|f| f.path.clone()).collect::<Vec<_>>());
    assert_eq!(got.ran.len(), want.ran.len());
    for (g, w) in got.ran.iter().zip(&want.ran) {
        assert_eq!(g.claim, want.claims[w.claim].name);
        let lines: std::collections::BTreeSet<u32> = g.lines.iter().flat_map(|(a, b)| (*a as u32)..=(*b as u32)).collect();
        assert_eq!(lines, w.lines, "{}", g.claim);
    }
    // a record that does not read says where
    std::fs::write(dir.path().join(".geas/greeter.map.jsonl"), "{not a record\n").unwrap();
    assert!(Engine.map_record(&spec).is_err());
}

#[test]
fn a_specs_claims_are_its_items() {
    let file = "examples/greeter/greeter.geas";
    let items = Engine.items(&root(), file).unwrap();
    let claims = Engine.claims(&root().join(file)).unwrap();
    assert_eq!(items.len(), claims.len());
    for (it, c) in items.iter().zip(&claims) {
        assert_eq!(it.kind(), "claim");
        assert_eq!(it.name(), c.name);
        assert_eq!(it.lines.0, c.line);
        assert!(it.text.starts_with("claim "), "{}", it.text);
        assert_eq!(it.text.lines().count(), 1 + c.steps.len(), "{}", it.text);
        assert_eq!(ritsu_base::naming::parse_one(&it.naming.text()).map(|x| x.text()).ok(), Some(it.naming.text()));
    }
    // every example spec reads, and its items are within its lines
    let mut all = Vec::new();
    ritsu_base::paths::walk(&root(), "examples", &[], &mut all);
    for f in all.iter().filter(|f| f.ends_with(".geas")) {
        let n = std::fs::read_to_string(root().join(f)).unwrap().lines().count();
        for it in Engine.items(&root(), f).unwrap() {
            assert!(it.lines.0 >= 1 && it.lines.0 <= it.lines.1 && it.lines.1 <= n, "{f}: {it:?}");
        }
    }
}
