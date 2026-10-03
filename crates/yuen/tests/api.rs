//! `yuen api` (DESIGN 11, PLAN B.13): the JSON of the `period` fixture against its golden
//! file, the keys in the order DESIGN 11 writes them.

mod common;

use std::path::Path;

#[test]
fn the_api_of_the_period_fixture() {
    let r = common::yuen(Path::new("."), &["api", "tests/fixtures/period", "--root", "tests/fixtures/period"]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    let mut failures = Vec::new();
    common::golden("tests/golden/api/period.json", &r.stdout, &mut failures);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["yuen", "root", "files", "roles", "sources", "requirements", "artifacts", "scopes", "check"]);
    let req = &v["requirements"][2];
    let keys: Vec<&String> = req.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["name", "alias", "version", "file", "line", "text", "in_force", "owner", "replaces", "sha256", "from", "decided", "links", "waivers"]);
    assert_eq!(req["name"], "満了日_142条");
    assert_eq!(req["sha256"], "d4f2d2a67322df17");
    assert_eq!(req["from"][0]["status"], "ok");
    assert_eq!(req["links"][0]["artifact"]["text"], "file \"民法の期間.cal\"");
    assert_eq!(req["links"][0]["sha256"], "c9b94eecde23e6b5");
    assert_eq!(v["scopes"][0]["artifacts"], 1);
    assert_eq!(v["check"]["ok"], true);
}

#[test]
fn the_api_carries_the_marks_as_states() {
    let t = common::fixture("period");
    common::change_142(&t.path().join("period"));
    let r = common::yuen(t.path(), &["api", "period", "--root", "period"]);
    assert_eq!(r.code, 0, "marks do not stop the api");
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    let req = &v["requirements"][2];
    assert_eq!(req["from"][0]["status"], "up_changed");
    assert_eq!(req["links"][0]["status"], "up_changed");
    assert_eq!(req["waivers"][0]["status"], "up_changed");
    assert_eq!(v["requirements"][0]["from"][0]["status"], "ok");
    assert_eq!(v["check"]["ok"], false);
    assert_eq!(v["check"]["diagnostics"][0]["code"], "E302");
}

#[test]
fn no_api_for_a_project_whose_names_are_wrong() {
    let r = common::yuen(Path::new("."), &["api", "tests/mutants/E008_宣言されていない役割"]);
    assert_eq!(r.code, 1);
    assert!(r.stdout.is_empty());
    assert!(r.stderr.contains("error[E008]"), "{}", r.stderr);
}
