//! `sakai api` (PLAN B.11, DESIGN 9): the JSON of the maps of B, held to its golden files in
//! `tests/golden/api/`, with its keys in the order DESIGN 9 gives.

mod common;

#[test]
fn the_api_of_each_map_is_its_golden_file() {
    let mut failures = Vec::new();
    for name in ["基本", "パターン"] {
        let dir = common::variant(name, &[]);
        let os = common::check_dir(dir.path());
        let c = os.iter().find_map(|o| o.checked.as_ref()).unwrap();
        let text = serde_json::to_string_pretty(&sakai::api::api(c)).unwrap() + "\n";
        if let Some(f) = common::golden(&format!("tests/golden/api/{name}.json"), &text) {
            failures.push(f);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn the_keys_come_in_their_order() {
    let dir = common::variant("基本", &[]);
    let os = common::check_dir(dir.path());
    let v = sakai::api::api(os[0].checked.as_ref().unwrap());
    let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["sakai", "map", "covers", "except", "contexts", "relationships", "artifacts", "crossings", "not_checked"]);
    let a = &v["artifacts"][0];
    let keys: Vec<&String> = a.as_object().unwrap().keys().collect();
    assert_eq!(keys, ["name", "context", "by", "sha256"]);
    let n: Vec<&String> = a["name"].as_object().unwrap().keys().collect();
    assert_eq!(n, ["text", "tool", "path", "items"]);
    // Every artifact of the scope is there, with its owner.
    assert_eq!(v["artifacts"].as_array().unwrap().len(), 9);
    assert_eq!(v["crossings"].as_array().unwrap().len(), 3);
}

/// The command prints what the library gives, with the root from --root.
#[test]
fn the_command_prints_the_same() {
    let o = common::sakai(&["api", "tests/maps/基本/基本.ctx", "--root", "tests/maps/基本"]);
    let want = std::fs::read_to_string("tests/golden/api/基本.json").unwrap();
    assert_eq!(String::from_utf8_lossy(&o.stdout), want);
}
