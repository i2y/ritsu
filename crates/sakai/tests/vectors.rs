//! `sakai vectors` (DESIGN 11.4): the values the mappings of the layers say, one line each, and the
//! example's own layer code held to them.
//!
//! The lines are held to a golden file and to the `.proto` they come from (every value of the enum, once,
//! with its number). `examples/shop/check_layers.py` reads them and calls the Python layers of the
//! example (`py/billing/acl/ordering/status.py`, `py/delivery/acl/inventory/packing.py`); it runs where
//! a Python is found, and is skipped where not. Three edits in a copy of the example, each a mistake a
//! layer's tests would be for (a value swapped in the code, the numbers swapped in the `.proto`, a value
//! swapped in the map), make it fail.

mod common;

use std::path::Path;

fn lines(args: &[&str]) -> (i32, String, String) {
    common::joined(args)
}

fn shop_vectors() -> String {
    let (code, out, err) = lines(&["vectors", "examples/shop/shop.ctx", "--root", "examples/shop"]);
    assert_eq!(code, 0, "{err}");
    out
}

#[test]
fn the_vectors_of_the_shop_are_the_golden_file() {
    let out = shop_vectors();
    if let Some(f) = common::golden("tests/golden/vectors/shop.jsonl", &out) {
        panic!("{f}");
    }
}

/// Every line is a JSON object, and the lines of an enum are its values: all of them, once each, in the
/// order of the `.proto`, with the numbers the `.proto` gives.
#[test]
fn every_value_of_every_mapped_enum_is_there_once_with_its_number() {
    let out = shop_vectors();
    let rows: Vec<serde_json::Value> = out.lines().map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("{e}: {l}"))).collect();
    for (file, en) in [("examples/shop/proto/warehouse/v1/stock.proto", "PackingStatus"), ("examples/shop/proto/shop/ordering/v1/order.proto", "OrderStatus")] {
        let text = std::fs::read_to_string(file).unwrap();
        let body = text.split(&format!("enum {en} {{")).nth(1).unwrap().split('}').next().unwrap();
        let want: Vec<(String, i64)> = body.lines().filter_map(|l| l.trim().strip_suffix(';')).filter_map(|l| l.split_once(" = ")).map(|(n, v)| (n.trim().to_string(), v.trim().parse().unwrap())).collect();
        let got: Vec<(String, i64)> = rows.iter().filter(|r| r["enum"] == en).map(|r| (r["value"].as_str().unwrap().to_string(), r["number"].as_i64().unwrap())).collect();
        assert_eq!(got, want, "{en}");
    }
    // each line says one of three things: where the value goes, that it is refused, or that the map says nothing of it
    for r in &rows {
        let says = ["to", "refuse", "unmapped"].iter().filter(|k| r.get(**k).is_some()).count();
        assert_eq!(says, 1, "{r}");
        for k in ["context", "upstream", "through", "layer", "enum", "value"] {
            assert!(r.get(k).is_some(), "{k}: {r}");
        }
    }
    // a refusal carries its reason; the value that says nothing is set is `unmapped`; the rule's answer is there
    let short = rows.iter().find(|r| r["value"] == "PACKING_STATUS_SHORT").unwrap();
    assert_eq!(short["refuse"], "A box with an item missing is not shipped. It goes back to ordering");
    let unset = rows.iter().find(|r| r["value"] == "PACKING_STATUS_UNSPECIFIED").unwrap();
    assert_eq!((unset["unmapped"].as_bool(), unset["number"].as_i64()), (Some(true), Some(0)));
    let paid = rows.iter().find(|r| r["value"] == "ORDER_STATUS_PAID").unwrap();
    assert_eq!(paid["to"], serde_json::json!({"enum": "order_status", "value": "paid", "rule": "billing/rules/billing_need.rule"}));
    assert_eq!(paid["answers"], serde_json::json!({"handling": "bill"}));
}

/// The twin with Japanese names says the same: the same values, numbers and fates, and the same answers.
#[test]
fn the_japanese_twin_says_the_same() {
    let en = shop_vectors();
    let (code, ja, err) = lines(&["vectors", "examples/shop.ja/通販.ctx", "--root", "examples/shop.ja"]);
    assert_eq!(code, 0, "{err}");
    let shape = |text: &str| -> Vec<(String, Option<i64>, String)> {
        text.lines()
            .map(|l| {
                let r: serde_json::Value = serde_json::from_str(l).unwrap();
                let fate = ["to", "refuse", "unmapped"].iter().find(|k| r.get(**k).is_some()).unwrap().to_string();
                (r["value"].as_str().unwrap().to_string(), r["number"].as_i64(), format!("{fate} {}", r.get("answers").map(|a| a.to_string()).unwrap_or_default()))
            })
            .collect()
    };
    let (a, b) = (shape(&en), shape(&ja));
    assert_eq!(a.len(), b.len());
    assert_eq!(a.iter().map(|x| (&x.0, x.1)).collect::<Vec<_>>(), b.iter().map(|x| (&x.0, x.1)).collect::<Vec<_>>());
    assert_eq!(a.iter().map(|x| x.2.split(' ').next().unwrap().to_string()).collect::<Vec<_>>(), b.iter().map(|x| x.2.split(' ').next().unwrap().to_string()).collect::<Vec<_>>());
}

/// A map with an error has no vectors, and says what check says.
#[test]
fn a_map_that_does_not_pass_check_has_no_vectors() {
    let dir = common::TempDir::new("vectors-broken");
    common::copy_dir(Path::new("examples/shop"), dir.path());
    let ctx = dir.path().join("contexts/delivery.ctx");
    let s = std::fs::read_to_string(&ctx).unwrap();
    std::fs::write(&ctx, s.replacen("    PACKING_STATUS_SHORT   -> refuse \"A box with an item missing is not shipped. It goes back to ordering\"\n", "", 1)).unwrap();
    let (code, out, err) = lines(&["vectors", dir.path().join("shop.ctx").to_str().unwrap(), "--root", dir.path().to_str().unwrap()]);
    assert_eq!(code, 1, "{err}");
    assert!(out.is_empty());
    assert!(err.contains("E401"), "{err}");
}

fn python() -> Option<String> {
    if !ritsu_testkit::need(ritsu_testkit::Need::Python) {
        return None;
    }
    let ok = std::process::Command::new("python3").arg("--version").output().map(|o| o.status.success()).unwrap_or(false);
    if !ok {
        ritsu_testkit::skip("python3 is not on PATH");
        return None;
    }
    Some("python3".to_string())
}

/// `check_layers.py` in `dir` (a copy of the example, or the example) on `vectors`.
fn held(py: &str, dir: &Path, vectors: &str) -> ritsu_testkit::run::Ran {
    let mut cmd = std::process::Command::new(py);
    cmd.arg("-I").arg(dir.join("check_layers.py"));
    ritsu_testkit::run::run_with_input(&mut cmd, Some(vectors.as_bytes()), std::time::Duration::from_secs(60))
}

fn vectors_of(dir: &Path) -> String {
    let (code, out, err) = lines(&["vectors", dir.join("shop.ctx").to_str().unwrap(), "--root", dir.to_str().unwrap()]);
    assert_eq!(code, 0, "{err}");
    out
}

#[test]
fn the_python_layers_of_the_shop_hold_to_the_vectors() {
    let Some(py) = python() else { return };
    let ran = held(&py, Path::new("examples/shop"), &shop_vectors());
    assert!(ran.ok, "{}", ran.both());
    println!("compared: {}", ran.stdout.trim());
    assert!(ran.stdout.contains("7 held, 0 failed"), "{}", ran.stdout);
}

/// Three mistakes, each in a copy: a value swapped in the code of a layer, the numbers of two values
/// swapped in the `.proto` (the stand-in the code is given by is the old one), and a value swapped in
/// the map (the code is the old one). Each one makes the check fail, and says which value.
#[test]
fn a_changed_layer_a_changed_proto_and_a_changed_map_each_fail() {
    let Some(py) = python() else { return };
    let edits: [(&str, &str, &str, &str); 3] = [
        (
            "py/billing/acl/ordering/status.py",
            "(order_pb2.ORDER_STATUS_PAID, order_pb2.ORDER_STATUS_SHIPPED)",
            "(order_pb2.ORDER_STATUS_PAID,)",
            "ORDER_STATUS_SHIPPED",
        ),
        ("proto/warehouse/v1/stock.proto", "PACKING_STATUS_WAITING = 1;\n  PACKING_STATUS_PACKED = 2;", "PACKING_STATUS_WAITING = 2;\n  PACKING_STATUS_PACKED = 1;", "PACKING_STATUS_PACKED"),
        ("contexts/delivery.ctx", "PACKING_STATUS_PACKED  -> ship", "PACKING_STATUS_PACKED  -> wait", "PACKING_STATUS_PACKED"),
    ];
    for (file, old, new, value) in edits {
        let dir = common::TempDir::new("vectors-mutant");
        common::copy_dir(Path::new("examples/shop"), dir.path());
        let p = dir.path().join(file);
        let s = std::fs::read_to_string(&p).unwrap();
        assert!(s.contains(old), "{file}");
        std::fs::write(&p, s.replacen(old, new, 1)).unwrap();
        // the vectors of the copy (those of the example for an edit of the code, which they do not depend on)
        // against the code of the copy
        let v = if file.starts_with("py/") { shop_vectors() } else { vectors_of(dir.path()) };
        let ran = held(&py, dir.path(), &v);
        assert!(!ran.ok && ran.stdout.contains(value), "{file}: {}", ran.both());
        println!("mutant {file}: {}", ran.stdout.lines().next().unwrap_or(""));
    }
}
