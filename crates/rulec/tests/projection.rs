//! The projection function, **run** (§15.125).
//!
//! `rulec test` compiles the generated module and runs its vectors, and the vectors are the
//! rule's own flat inputs — so the projection function is compiled and never called. A
//! generated function nothing runs is a generated function nothing checks, which is the
//! reason §15.46 keeps targets it cannot run out of the set altogether.
//!
//! So one object, built by hand against the contract the corpus rule cites, is pushed
//! through every language that generates a projection, and the answer is held to the one the
//! rule gives for the same case directly. A language whose toolchain is missing is skipped
//! and said so, the way `rulec test` does; Python is required, because a run with nothing in
//! it would pass while proving nothing.

use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn have(cmd: &str) -> bool {
    ["--version", "version"]
        .iter()
        .any(|a| Command::new(cmd).arg(a).output().map(|o| o.status.success()).unwrap_or(false))
}

/// The corpus rule that projects, generated once.
fn generated(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rulec-projection-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(root())
        .args(["gen", "tests/corpus/注文の送料.rule", "--out", &dir.to_string_lossy()])
        .output()
        .expect("rulec を起動できない");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));
    dir
}

/// One order: Okinawa, eleven lines, one of them chilled. The rule's own `examples` pin the
/// same case as `okinawa | true | 11 -> 2200円`, so what this holds the five projections to
/// is a number the reference evaluator already agreed with.
const WANT: &str = "2200";

fn lines_json() -> String {
    let mut v: Vec<String> = Vec::new();
    for i in 0..11 {
        let chilled = if i == 3 { "true" } else { "false" };
        v.push(format!("{{\"sku\":\"s{i}\",\"chilled\":{chilled},\"amount_jpy\":100}}"));
    }
    format!("[{}]", v.join(","))
}

fn order_json() -> String {
    format!("{{\"shipping\":{{\"zone\":\"okinawa\",\"postcode\":\"900-0001\"}},\"lines\":{}}}", lines_json())
}

/// Run one driver and hold what it prints to `WANT`.
fn check(lang: &str, cmd: &mut Command) {
    let o = cmd.output().unwrap_or_else(|e| panic!("{lang} を起動できない: {e}"));
    let out = String::from_utf8_lossy(&o.stdout).trim().to_string();
    assert!(
        o.status.success(),
        "{lang}: 射影が走らない:\n{}\n{}",
        out,
        String::from_utf8_lossy(&o.stderr)
    );
    assert_eq!(out, WANT, "{lang}: 射影の答えが違う");
}

#[test]
fn 射影は五つの言語で同じ答えを出す() {
    let dir = generated("run");
    let mut ran = 0;

    // --- Python
    assert!(have("python3"), "python3 が要ります（この試験は生成物を走らせます）");
    let script = format!(
        "import json, order_shipping as m\nprint(m.order_shipping_from(json.loads({:?})))\n",
        order_json()
    );
    check("Python", Command::new("python3").current_dir(dir.join("python")).args(["-B", "-c", &script]));
    ran += 1;

    // --- JavaScript, and with it the TypeScript it is stripped from.
    if have("node") {
        let script = format!(
            "import('./order_shipping.mjs').then(m => console.log(String(m.order_shipping_from(JSON.parse({:?})))));",
            order_json()
        );
        check("JavaScript", Command::new("node").current_dir(dir.join("javascript")).args(["--input-type=module", "-e", &script]));
        ran += 1;
    } else {
        eprintln!("注意: node が無いので JavaScript 側を飛ばした");
    }

    // --- Ruby
    if have("ruby") {
        let script = format!(
            "require 'json'\nrequire './order_shipping.rb'\nputs OrderShipping.order_shipping_from(JSON.parse({:?}))\n",
            order_json()
        );
        check("Ruby", Command::new("ruby").current_dir(dir.join("ruby")).args(["-e", &script]));
        ran += 1;
    } else {
        eprintln!("注意: ruby が無いので Ruby 側を飛ばした");
    }

    // --- PHP
    if have("php") {
        let script = format!(
            "require './order_shipping.php'; echo \\OrderShipping\\order_shipping_from(json_decode({}, true));",
            php_str(&order_json())
        );
        check("PHP", Command::new("php").current_dir(dir.join("php")).args(["-r", &script]));
        ran += 1;
    } else {
        eprintln!("注意: php が無いので PHP 側を飛ばした");
    }

    // --- TypeScript. Node reads a `.ts` by stripping the types, which is how `rulec test`
    // runs this target too (§8.3: the generated TypeScript is erasable syntax by design), so
    // the two declarations the projection adds have to be erasable as well.
    if have("node") {
        let script = format!(
            "import('./order_shipping.ts').then(m => console.log(String(m.order_shipping_from(JSON.parse({:?})))));",
            order_json()
        );
        check(
            "TypeScript",
            Command::new("node")
                .current_dir(dir.join("typescript"))
                .args(["--no-warnings", "--input-type=module", "-e", &script]),
        );
        ran += 1;
    }

    eprintln!("射影を走らせた言語: {ran}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A PHP single-quoted string for `php -r`.
fn php_str(s: &str) -> String {
    format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
}

/// The five that generate it, and only those five: a target that grew one without anyone
/// deciding to is a target whose caller's object this tool started naming (§15.125).
#[test]
fn 射影を出すのは五つの言語だけ() {
    let dir = generated("where");
    let has = |rel: &str, needle: &str| {
        std::fs::read_to_string(dir.join(rel)).map(|s| s.contains(needle)).unwrap_or(false)
    };
    for (rel, needle) in [
        ("python/order_shipping.py", "def order_shipping_from("),
        ("typescript/order_shipping.ts", "export function order_shipping_from("),
        ("javascript/order_shipping.mjs", "export function order_shipping_from("),
        ("ruby/order_shipping.rb", "def self.order_shipping_from("),
        ("php/order_shipping.php", "function order_shipping_from("),
    ] {
        assert!(has(rel, needle), "{rel} に射影関数がありません");
    }
    for rel in ["rust/order_shipping.rs", "go/ordershipping/order_shipping.go", "swift/order_shipping.swift", "java/OrderShipping.java", "sql/order_shipping.sql"] {
        assert!(!has(rel, "_from("), "{rel} に射影関数が出ています");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The paths are held to the contract, and the contract is read from beside the rule.
#[test]
fn 契約の欄が変われば止まる() {
    let dir = std::env::temp_dir().join(format!("rulec-projection-moved-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("contracts")).unwrap();
    let rule = std::fs::read_to_string(root().join("tests/corpus/注文の送料.rule")).unwrap();
    let schema = std::fs::read_to_string(root().join("tests/corpus/contracts/order.schema.json")).unwrap();
    std::fs::write(dir.join("r.rule"), &rule).unwrap();
    // The contract renames one field, which is the change nothing else in this tool can see.
    std::fs::write(dir.join("contracts/order.schema.json"), schema.replace("\"zone\"", "\"region\"")).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .args(["check", &dir.join("r.rule").to_string_lossy(), "--format", "json"])
        .output()
        .expect("rulec を起動できない");
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(out.contains("\"E121\""), "欄の名前が変わっても止まらない:\n{out}");
    assert_eq!(o.status.code(), Some(1), "E121 は error です");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A rule that projects nothing gets no projection function and no `_Obj` type: the five
/// emitters ask before writing anything.
#[test]
fn 射影の無い規則には何も出ない() {
    let dir = std::env::temp_dir().join(format!("rulec-projection-none-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(root())
        .args(["gen", "tests/corpus/送料.rule", "--out", &dir.to_string_lossy()])
        .output()
        .expect("rulec を起動できない");
    assert!(o.status.success());
    let names = |d: &Path| -> Vec<String> {
        std::fs::read_dir(d).map(|r| r.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect()).unwrap_or_default()
    };
    for lang in ["python", "typescript", "javascript", "ruby", "php"] {
        for n in names(&dir.join(lang)) {
            let Ok(s) = std::fs::read_to_string(dir.join(lang).join(&n)) else { continue };
            assert!(!s.contains("_from("), "{lang}/{n} に射影関数が出ています");
            assert!(!s.contains("_Obj"), "{lang}/{n} に射影の型が出ています");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}
