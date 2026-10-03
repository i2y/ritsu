//! SHA-256, the IDs of PLAN 0.3, and what a TigerBeetle client sends for every operation of
//! every scenario of tests/books (the golden `<book>.chains.json`).

mod common;
use chobo::ids::{self, hex, sha256};
use chobo::model::Unit;
use chobo::scenarios;
use common::*;
use serde_json::Value;

#[test]
fn sha256_gives_the_known_digests() {
    assert_eq!(hex(&sha256(b"")), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    assert_eq!(hex(&sha256(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    assert_eq!(
        hex(&sha256(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq")),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
    assert_eq!(hex(&sha256(&vec![b'a'; 1_000_000])), "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0");
}

/// The table of PLAN 0.3, worked out with Python's hashlib in stage A.
#[test]
fn ids_match_the_table() {
    let cases: &[(&[&str], &str)] = &[
        (&["account", "在庫", "", "在庫", "A-1"], "396213c27529b522708ee6da1d0d03f7"),
        (&["account", "inventory", "", "stock", "A-1"], "f0857e5a2049d01a7f4989e04ee4e37f"),
        (&["account", "inventory", "t-1", "stock", "A-1"], "9a98b9b990f5c3ea4640cd5755b4ce17"),
        (&["account", "inventory", "", "supplier"], "cafb18eaf6485ea880c3ef6350f4cdeb"),
        (&["transfer", "在庫", "", "引当", "hold", "o-1", "A-1", "0"], "594bbb54c47e8b03c55fac1644fbcffa"),
        (&["transfer", "在庫", "", "引当", "post", "o-1", "A-1", "0"], "0a75517949dcb9dec36a9a9d9b04a50c"),
        (&["sink", "在庫", "", "個"], "e132a4987f7c612337996fa93bc06f65"),
        (&["room", "f0857e5a2049d01a7f4989e04ee4e37f"], "2bd360017fee460bbca3ca67451e63dc"),
        (&["opening", "2bd360017fee460bbca3ca67451e63dc"], "c5af9cb7947230b39cfaa34fff20953b"),
        (&["account", "b", "", "a:b", "c"], "464fb5f164fc1b9fd9fc7ea1157c5aaf"),
        (&["account", "b", "", "a", "b:c"], "4f01a8c48b5a8baed7db4f9df67b300b"),
    ];
    for (parts, want) in cases {
        assert_eq!(ids::id_hex(ids::id(parts)), *want, "{parts:?}");
    }
    assert_eq!(ids::ledger(&Unit { name: "個".into(), scale: 0, line: 0, col: 0 }), 1653847866);
    assert_eq!(ids::ledger(&Unit { name: "USD".into(), scale: 2, line: 0, col: 0 }), 1838891465);
    assert_eq!(ids::code("在庫", "transfer", "引当"), 47038);
    assert_eq!(ids::code("在庫", "account", "在庫"), 48630);
    // the ID of a room account from the ID of its account, as the client works it out
    assert_eq!(ids::id_hex(ids::room_id(0xf0857e5a2049d01a7f4989e04ee4e37f)), "2bd360017fee460bbca3ca67451e63dc");
    assert_eq!(ids::id_hex(ids::opening_id(0x2bd360017fee460bbca3ca67451e63dc)), "c5af9cb7947230b39cfaa34fff20953b");
}

#[test]
fn json_strings_are_written_as_javascript_writes_them() {
    assert_eq!(ids::json_string("a\"b\\c\nd\u{1}é"), "\"a\\\"b\\\\c\\nd\\u0001é\"");
}

#[test]
fn every_operation_of_every_scenario_sends_what_the_golden_says() {
    for p in books_in("tests/books") {
        let (book, _) = chobo::model::load(&std::fs::read_to_string(&p).unwrap());
        let book = book.unwrap();
        let mut out = Vec::new();
        for s in scenarios::generate(&book) {
            out.push(serde_json::json!({"scenario": s.name, "operations": ids::scenario_chains(&book, "", &s)}));
        }
        golden(&p.with_file_name(format!("{}.chains.json", stem(&p))), &(serde_json::to_string_pretty(&Value::Array(out)).unwrap() + "\n"));
    }
}
