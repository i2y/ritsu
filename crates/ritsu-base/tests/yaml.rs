//! The reader of YAML and JSON (DESIGN 4.17). It reads the part of YAML 1.2 that goes to JSON and
//! back, and stops at the edge of it; the YAML test suite holds it to that: on every case it reads
//! the value the suite gives, or does not read the case, and it reads no case the suite says is no
//! YAML. The cases are `tests/fixtures/yaml-test-suite.json`, gathered from the suite's release
//! data-2022-01-17 (MIT, `tests/fixtures/yaml-test-suite.LICENSE`): the test runs with no network.

use ritsu_base::yaml::{Node, Value, read_json, read_yaml};

/// The same value: numbers by what they are worth (the suite writes `1.0` for some floats).
fn same(a: &Node, b: &Node) -> bool {
    let num = |n: &Node| match &n.value {
        Value::Int(i) => *i as f64,
        Value::Float(f) => f.parse::<f64>().unwrap_or(f64::NAN),
        _ => f64::NAN,
    };
    match (&a.value, &b.value) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Str(x), Value::Str(y)) => x == y,
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Int(_) | Value::Float(_), Value::Int(_) | Value::Float(_)) => num(a) == num(b),
        (Value::Seq(x), Value::Seq(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same(p, q)),
        (Value::Map(x), Value::Map(y)) => x.len() == y.len() && x.iter().zip(y).all(|((k1, v1), (k2, v2))| k1.name == k2.name && same(v1, v2)),
        _ => false,
    }
}

struct Case {
    id: String,
    name: String,
    yaml: String,
    json: Option<String>,
    error: bool,
}

fn cases() -> Vec<Case> {
    let text = std::fs::read_to_string("tests/fixtures/yaml-test-suite.json").unwrap();
    let all = read_json(&text).unwrap();
    all.as_seq()
        .unwrap()
        .iter()
        .map(|c| Case {
            id: c.get("id").and_then(Node::as_str).unwrap().to_string(),
            name: c.get("name").and_then(Node::as_str).unwrap().to_string(),
            yaml: c.get("yaml").and_then(Node::as_str).unwrap().to_string(),
            json: c.get("json").and_then(Node::as_str).map(str::to_string),
            error: c.get("error").is_some_and(|e| e.value == Value::Bool(true)),
        })
        .collect()
}

#[test]
fn every_case_of_the_yaml_test_suite_reads_as_the_suite_says_or_is_not_read() {
    let cases = cases();
    let (mut read_same, mut not_read, mut errors) = (0, 0, 0);
    let mut unjudged: Vec<String> = Vec::new();
    let mut why: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new();
    let mut failures: Vec<String> = Vec::new();
    for c in &cases {
        let got = read_yaml(&c.yaml);
        if c.error {
            match got {
                Ok(v) => failures.push(format!("{} ({}): the suite says it is no YAML, and it reads as {}", c.id, c.name, v.to_json_text())),
                Err(_) => errors += 1,
            }
            continue;
        }
        match (got, &c.json) {
            (Err(e), _) => {
                not_read += 1;
                why.entry(e.message.en.clone()).or_default().push(c.id.clone());
            }
            (Ok(v), Some(j)) => match read_json(j) {
                Ok(want) if same(&v, &want) => read_same += 1,
                Ok(want) => failures.push(format!("{} ({}): reads as {}, and the suite gives {}", c.id, c.name, v.to_json_text(), want.to_json_text())),
                Err(_) => failures.push(format!("{} ({}): the suite gives more than one document, and one reads as {}", c.id, c.name, v.to_json_text())),
            },
            (Ok(v), None) => unjudged.push(format!("{} ({}): {}", c.id, c.name, v.to_json_text())),
        }
    }
    println!(
        "yaml-test-suite data-2022-01-17: {} cases; {read_same} read as the suite's JSON, {not_read} not read, {errors} of {} that are no YAML not read, {} read with no JSON to compare",
        cases.len(),
        cases.iter().filter(|c| c.error).count(),
        unjudged.len()
    );
    for (w, ids) in &why {
        println!("  not read, {} cases: {w} ({})", ids.len(), ids.join(", "));
    }
    for u in &unjudged {
        println!("  read with no JSON to compare: {u}");
    }
    assert!(failures.is_empty(), "{} of the cases:\n{}", failures.len(), failures.join("\n"));
    assert!(unjudged.is_empty(), "cases read that the suite gives no JSON for:\n{}", unjudged.join("\n"));
}

#[test]
fn values_know_where_they_were_written() {
    let doc = read_yaml("openapi: 3.1.0\ncomponents:\n  schemas:\n    Status:\n      enum: [placed, \"paid\", 200, true, null]\n").unwrap();
    let e = doc.pointer("/components/schemas/Status/enum").unwrap();
    assert_eq!((e.line, e.col), (5, 13));
    let xs = e.as_seq().unwrap();
    assert_eq!(xs.iter().map(|x| (x.line, x.col)).collect::<Vec<_>>(), [(5, 14), (5, 22), (5, 30), (5, 35), (5, 41)]);
    assert_eq!(e.to_json_text(), r#"["placed","paid",200,true,null]"#);
    let (k, _) = doc.pointer("/components/schemas").unwrap().entry("Status").unwrap();
    assert_eq!((k.line, k.col), (4, 5));
    // a pointer's `~1` is `/`
    let paths = read_yaml("paths:\n  /orders/{id}:\n    get:\n      operationId: getOrder\n").unwrap();
    assert_eq!(paths.pointer("/paths/~1orders~1{id}/get/operationId").and_then(Node::as_str), Some("getOrder"));
}

#[test]
fn a_plain_scalar_is_typed_by_the_core_schema_and_a_key_is_the_string_written() {
    let v = read_yaml("a: yes\nb: on\nc: 0x1F\nd: 0o17\ne: 1e3\nf: ~\n200: ok\ntrue: t\n'q': !!str 12\n").unwrap();
    assert_eq!(v.to_json_text(), r#"{"a":"yes","b":"on","c":31,"d":15,"e":1e3,"f":null,"200":"ok","true":"t","q":"12"}"#);
}

#[test]
fn what_goes_beyond_json_is_not_read_and_said_by_name() {
    let said = |src: &str| {
        let e = read_yaml(src).unwrap_err();
        (e.line, e.col, e.message.en, e.message.ja)
    };
    assert_eq!(said("a: 1\n---\nb: 2\n").2, "a second document; one document is read");
    assert_eq!(said("a: !!binary aGk=\n").2, "the tag `!!binary` is not read; the tags read are those of JSON's types (`!!str`, `!!int`, `!!float`, `!!bool`, `!!null`, `!!seq`, `!!map`) and `!`");
    assert_eq!(said("? a\n: b\n").2, "a key written with `?` is not read");
    assert_eq!(said("a: 1\na: 2\n").0, 2);
    assert_eq!(said("a: 1\na: 2\n").3, "キー `a` が二度あります");
    assert_eq!(said("x: .inf\n").2, "`.inf` is no value of JSON (`.inf` and `.nan` are not read)");
    assert_eq!(said("%YAML 1.1\n---\na: 1\n").3, "YAML 1.1 の文書です。読むのは YAML 1.2 の文書です");
    assert_eq!(said("a:\n\tb: 1\n").2, "a tab in the indentation; YAML indents with spaces");
    assert_eq!(said("a: &x [*x]\n").2, "no value with the anchor `&x` before the alias `*x`");
    // an alias is a copy of what its anchor is on
    assert_eq!(read_yaml("a: &x {k: v}\nb: *x\n").unwrap().to_json_text(), r#"{"a":{"k":"v"},"b":{"k":"v"}}"#);
}

#[test]
fn json_is_read_with_its_places_and_a_key_written_twice_is_not() {
    let v = read_json("{\n  \"openapi\": \"3.2.0\",\n  \"n\": [1, 2.5, -3e2]\n}").unwrap();
    let (k, _) = v.entry("n").unwrap();
    assert_eq!((k.line, k.col), (3, 3));
    assert_eq!(v.get("n").unwrap().to_json_text(), "[1,2.5,-3e2]");
    let e = read_json("{\"a\": 1, \"a\": 2}").unwrap_err();
    assert_eq!(e.message.en, "the key `a` is written twice");
}
