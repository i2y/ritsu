//! The reader of `.proto` files (PLAN B.5): held to what buf reads from the same files, where
//! buf can read them, and to the rules of DESIGN 1.7 and 4.2 on real files.

mod common;

use sakai::proto::{self, Resolved, Type};
use serde_json::Value;
use std::path::Path;
use std::time::Duration;

/// Every `.proto` under `root`, as paths from it, without the known ones.
fn protos_under(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    sakai::paths::walk(root, ".", &[], &mut out);
    out.retain(|p| p.ends_with(".proto") && !proto::is_known(p));
    out
}

/// What sakai reads of a file, one line a fact, sorted.
fn sakai_reads(root: &Path, file: &str) -> Vec<String> {
    let (ps, issues) = proto::load(root, &protos_under(root), &[".".to_string()]);
    assert!(issues.is_empty(), "{issues:?}");
    let f = &ps.files[file];
    let mut out = vec![format!("package {}", f.package)];
    out.extend(f.imports.iter().map(|i| format!("import {}", i.path)));
    let ty = |scope: &str, t: &Type| -> String {
        let one = |n: &str| match ps.resolve(file, scope, n) {
            Resolved::Found(x) => x.full,
            Resolved::Known(x) => x,
            other => format!("{other:?}"),
        };
        match t {
            Type::Scalar(s) => s.clone(),
            Type::Named(n) => one(n),
            Type::Map(k, v) => format!("map<{k},{}>", match v.as_ref() {
                Type::Scalar(s) => s.clone(),
                Type::Named(n) => one(n),
                Type::Map(..) => unreachable!(),
            }),
        }
    };
    for m in &f.messages {
        out.push(format!("message {}", m.name));
        let scope = f.full(&m.name);
        for fl in &m.fields {
            let rep = if fl.label == proto::Label::Repeated { " repeated" } else { "" };
            out.push(format!("field {}.{} {}{rep} {}", m.name, fl.name, fl.number, ty(&scope, &fl.ty)));
        }
    }
    for e in &f.enums {
        out.push(format!("enum {}", e.name));
        out.extend(e.values.iter().map(|v| format!("value {}.{} {}", e.name, v.name, v.number)));
    }
    for s in &f.services {
        out.push(format!("service {}", s.name));
        for m in &s.methods {
            let full = |n: &str| match ps.resolve(file, &f.package, n) {
                Resolved::Found(x) => x.full,
                Resolved::Known(x) => x,
                other => format!("{other:?}"),
            };
            out.push(format!("method {}.{} {} {} {} {}", s.name, m.name, full(&m.input), full(&m.output), m.client_streaming, m.server_streaming));
        }
    }
    out.sort();
    out
}

/// What buf reads of the same file, in the same lines.
fn buf_reads(buf: &str, root: &Path, file: &str) -> Vec<String> {
    let r = common::run(std::process::Command::new(buf).args(["build", ".", "--path", file, "-o", "-#format=json"]).current_dir(root), Duration::from_secs(60));
    assert!(r.ok, "buf build {file}: {}", r.stderr);
    let image: Value = serde_json::from_str(&r.stdout).unwrap();
    let f = image["file"].as_array().unwrap().iter().find(|x| x["name"] == file).unwrap().clone();
    let pkg = f["package"].as_str().unwrap_or("").to_string();
    let mut out = vec![format!("package {pkg}")];
    for d in f["dependency"].as_array().into_iter().flatten() {
        out.push(format!("import {}", d.as_str().unwrap()));
    }
    // Map entries are nested messages to buf; to sakai they are a field's type.
    fn entries(prefix: &str, ms: &Value, into: &mut std::collections::HashMap<String, (Value, Value)>) {
        for m in ms.as_array().into_iter().flatten() {
            let name = format!("{prefix}.{}", m["name"].as_str().unwrap());
            if m["options"]["mapEntry"] == true {
                let fs = m["field"].as_array().unwrap();
                into.insert(name.clone(), (fs[0].clone(), fs[1].clone()));
            }
            entries(&name, &m["nestedType"], into);
        }
    }
    let mut maps = std::collections::HashMap::new();
    entries(&pkg, &f["messageType"], &mut maps);
    let one = |fl: &Value| -> String {
        match fl["type"].as_str().unwrap() {
            "TYPE_MESSAGE" | "TYPE_ENUM" => fl["typeName"].as_str().unwrap().trim_start_matches('.').to_string(),
            t => t.trim_start_matches("TYPE_").to_ascii_lowercase(),
        }
    };
    fn walk(prefix: &str, rel: &str, ms: &Value, maps: &std::collections::HashMap<String, (Value, Value)>, one: &dyn Fn(&Value) -> String, out: &mut Vec<String>) {
        for m in ms.as_array().into_iter().flatten() {
            if m["options"]["mapEntry"] == true {
                continue;
            }
            let short = m["name"].as_str().unwrap();
            let name = if rel.is_empty() { short.to_string() } else { format!("{rel}.{short}") };
            out.push(format!("message {name}"));
            for fl in m["field"].as_array().into_iter().flatten() {
                let tn = fl["typeName"].as_str().unwrap_or("").trim_start_matches('.').to_string();
                let (rep, ty) = if let Some((k, v)) = maps.get(&tn) { ("", format!("map<{},{}>", one(k), one(v))) } else { (if fl["label"] == "LABEL_REPEATED" { " repeated" } else { "" }, one(fl)) };
                out.push(format!("field {name}.{} {}{rep} {ty}", fl["name"].as_str().unwrap(), fl["number"]));
            }
            for e in m["enumType"].as_array().into_iter().flatten() {
                enum_lines(&format!("{name}.{}", e["name"].as_str().unwrap()), e, out);
            }
            walk(&format!("{prefix}.{short}"), &name, &m["nestedType"], maps, one, out);
        }
    }
    fn enum_lines(name: &str, e: &Value, out: &mut Vec<String>) {
        out.push(format!("enum {name}"));
        for v in e["value"].as_array().into_iter().flatten() {
            out.push(format!("value {name}.{} {}", v["name"].as_str().unwrap(), v["number"].as_i64().unwrap_or(0)));
        }
    }
    walk(&pkg, "", &f["messageType"], &maps, &one, &mut out);
    for e in f["enumType"].as_array().into_iter().flatten() {
        enum_lines(e["name"].as_str().unwrap(), e, &mut out);
    }
    for s in f["service"].as_array().into_iter().flatten() {
        let sn = s["name"].as_str().unwrap();
        out.push(format!("service {sn}"));
        for m in s["method"].as_array().into_iter().flatten() {
            out.push(format!(
                "method {sn}.{} {} {} {} {}",
                m["name"].as_str().unwrap(),
                m["inputType"].as_str().unwrap().trim_start_matches('.'),
                m["outputType"].as_str().unwrap().trim_start_matches('.'),
                m["clientStreaming"] == true,
                m["serverStreaming"] == true
            ));
        }
    }
    out.sort();
    out
}

#[test]
fn sakai_reads_what_buf_reads() {
    if !ritsu_testkit::need(ritsu_testkit::Need::Buf) {
        return;
    }
    let Some(buf) = common::program("BUF", "", "buf", &["--version"]) else {
        common::skip("buf is not installed (SAKAI_BUF or the PATH); the reader of .proto files is not compared with it");
        return;
    };
    let cases = [
        ("tests/fixtures/proto", "shop/ordering/v1/order.proto"),
        ("tests/fixtures/proto", "warehouse/v1/stock.proto"),
        ("tests/fixtures/proto", "shop/ordering/v1/fulfillment.proto"),
        ("tests/fixtures/proto-dandori", "warehouse.proto"),
    ];
    for (root, file) in cases {
        let root = Path::new(root);
        let ours = sakai_reads(root, file);
        let theirs = buf_reads(&buf, root, file);
        assert_eq!(ours, theirs, "{file}");
        println!("compared {file} with buf: {} facts", ours.len());
    }
}

#[test]
fn a_file_that_imports_buf_validate_reads_without_its_import() {
    let root = Path::new("tests/fixtures/proto");
    let (ps, issues) = proto::load(root, &protos_under(root), &[".".to_string()]);
    assert!(issues.is_empty(), "{issues:?}");
    let f = &ps.files["shop/delivery/v1/shipment.proto"];
    assert_eq!(f.package, "shop.delivery.v1");
    assert_eq!(f.message("CreateShipmentRequest").unwrap().fields.len(), 4);
    assert!(!ps.unread.contains("shop/delivery/v1/shipment.proto"));
    // fulfillment.proto names the warehouse's types and Google's.
    let ff = "shop/ordering/v1/fulfillment.proto";
    let Resolved::Found(rr) = ps.resolve(ff, "shop.ordering.v1.FulfillResponse", "warehouse.v1.ReserveResponse") else { panic!() };
    assert_eq!((rr.full.as_str(), rr.file.as_str()), ("warehouse.v1.ReserveResponse", "warehouse/v1/stock.proto"));
    assert_eq!(ps.resolve(ff, "shop.ordering.v1.FulfillmentOrder", "google.protobuf.Value"), Resolved::Known("google.protobuf.Value".into()));
    // What a field of fulfillment.proto reaches crosses into stock.proto.
    let Resolved::Found(fr) = ps.resolve(ff, "shop.ordering.v1", "FulfillResponse") else { panic!() };
    let reached: Vec<String> = ps.reach(&[fr]).into_iter().map(|s| s.full).collect();
    assert_eq!(reached, vec!["shop.ordering.v1.FulfillResponse", "warehouse.v1.ReserveResponse", "warehouse.v1.Stock"]);
}

#[test]
fn the_value_zero_that_says_nothing_is_set_on_real_files() {
    let read = |p: &str| proto::read(p, &std::fs::read_to_string(p).unwrap()).unwrap();
    let order = read("tests/fixtures/proto/shop/ordering/v1/order.proto");
    let e = order.enumeration("OrderStatus").unwrap();
    assert!(proto::is_unset(e, &e.values[0]));
    assert!(!proto::is_unset(e, &e.values[1]));
    let stock = read("tests/fixtures/proto/warehouse/v1/stock.proto");
    let e = stock.enumeration("Stock").unwrap();
    assert!(proto::is_unset(e, &e.values[0]));
    let dandori = read("tests/fixtures/proto-dandori/warehouse.proto");
    let e = dandori.enumeration("Stock").unwrap();
    assert_eq!(e.values[0].name, "unspecified");
    assert!(proto::is_unset(e, &e.values[0]));
    let ship = read("tests/fixtures/proto/shop/delivery/v1/shipment.proto");
    let e = ship.enumeration("Handling").unwrap();
    assert_eq!(e.values[0].name, "HANDLING_STANDARD");
    assert!(!proto::is_unset(e, &e.values[0]), "HANDLING_STANDARD = 0 is a value like the rest");
}

#[test]
fn an_import_that_is_not_there_is_told() {
    let dir = common::TempDir::new("import");
    dir.write("a/v1/a.proto", "syntax = \"proto3\";\npackage a.v1;\nimport \"google/api/annotations.proto\";\nimport \"b/v1/b.proto\";\nmessage A { b.v1.B b = 1; c.v1.C c = 2; }\n");
    let (ps, issues) = proto::load(dir.path(), &["a/v1/a.proto".to_string()], &[]);
    assert_eq!(issues.len(), 2, "{issues:?}");
    assert!(matches!(&issues[0], proto::Issue::NotFound { import, .. } if import.path == "google/api/annotations.proto" && import.line == 3));
    assert_eq!(ps.resolve("a/v1/a.proto", "a.v1.A", "c.v1.C"), Resolved::Unknown);
}
