//! The checks of security (DESIGN 16) on the example of the documents, `examples/webshop` and its
//! Japanese twin `examples/webshop.ja`, each with one change: what is meant and says so passes,
//! and what the mutants of `tests/mutants/W90…` do not show is held here — a relative server, the
//! loopback, `x-ritsu-plaintext`, a protocol whose name says nothing of encryption, `security: []`
//! on the document, on an operation and on a server, the empty requirement `{}`, a webhook, a
//! document no published language holds, an AsyncAPI operation with `security`, a key for tests,
//! and AWS's example key. The example itself passes with no warning (tests/contracts.rs).

mod common;

use ritsu_base::text::Lang;
use std::path::Path;

const EN: &str = "examples/webshop";
const JA: &str = "examples/webshop.ja";

/// The fake key of ritsu's DESIGN 16.10, joined here so that the source holds no key whole.
fn fake_key() -> String {
    ["AIzaSyD-ritsu-fake-", "key-for-tests-000000"].concat()
}

/// The example laid out in a temporary directory, each `(file, old, new)` replacing the first
/// `old` of the file with `new`; an empty `old` adds `new` at the end of the file (or writes it).
fn changed(example: &str, edits: &[(&str, &str, &str)]) -> common::TempDir {
    let dir = common::TempDir::new("security");
    common::copy_dir(Path::new(example), dir.path());
    for (file, old, new) in edits {
        let p = dir.path().join(file);
        let s = std::fs::read_to_string(&p).unwrap_or_default();
        let text = if old.is_empty() {
            s + new
        } else {
            assert!(s.contains(old), "{example}/{file} has no `{old}`");
            s.replacen(old, new, 1)
        };
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, text).unwrap();
    }
    dir
}

/// The codes `check` gives, and what it prints in English and in Japanese.
fn codes_of(dir: &common::TempDir) -> (Vec<String>, String) {
    let os = common::check_dir(dir.path());
    let codes = os.iter().flat_map(|o| o.diags.iter().map(|d| d.code.to_string())).collect();
    let text = [Lang::En, Lang::Ja].iter().map(|&l| os.iter().map(|o| sakai::check::render(o, l)).collect::<String>()).collect();
    (codes, text)
}

const PAYMENTS_API: &str = "payments/api/payments.yaml";
const PAYMENTS_EVENTS: &str = "payments/events/payments.yaml";
const ROOT_SECURITY: &str = "security:\n  - bearer: []\n";

/// SASL for the brokers of the events, so that a server with it says how a client proves who it is.
const SASL: &str = "  securitySchemes:\n    sasl:\n      type: scramSha256\n";

/// A change to the example: what it is, and its edits (`changed`'s, with the file `CONTEXT` for
/// the context file of Payments in either example).
type Case = (&'static str, Vec<(&'static str, String, String)>);

/// What passes: each change, in the English and the Japanese example, gives no code.
#[test]
fn what_is_meant_and_says_so_passes() {
    let key = fake_key();
    let aws = ["AKIA", "IOSFODNN7", "EXAMPLE"].concat();
    let cases: Vec<Case> = vec![
        ("a relative server says no scheme", vec![(PAYMENTS_API, String::new(), "servers:\n  - url: /v1\n".into())]),
        ("https", vec![(PAYMENTS_API, String::new(), "servers:\n  - url: https://payments.example.com/v1\n".into())]),
        (
            "the loopback",
            vec![(PAYMENTS_API, String::new(), "servers:\n  - url: http://localhost:8080/v1\n  - url: http://127.0.0.1:8080/v1\n  - url: 'http://[::1]:8080/v1'\n  - url: ws://api.localhost/v1\n".into())],
        ),
        (
            "x-ritsu-plaintext",
            vec![(PAYMENTS_API, String::new(), "servers:\n  - url: http://payments.internal/v1\n    x-ritsu-plaintext: \"The service mesh encrypts every connection inside the cluster\"\n".into())],
        ),
        (
            "a protocol whose name says nothing of encryption",
            vec![(PAYMENTS_EVENTS, String::new(), "servers:\n  production:\n    host: nats.internal:4222\n    protocol: nats\n    security: []\n".into())],
        ),
        ("a broker on the loopback", vec![(PAYMENTS_EVENTS, String::new(), "servers:\n  local:\n    host: localhost:9092\n    protocol: kafka\n    security: []\n".into())]),
        (
            "x-ritsu-plaintext on a broker",
            vec![(PAYMENTS_EVENTS, String::new(), format!("{SASL}servers:\n  production:\n    host: broker.internal:9092\n    protocol: kafka\n    x-ritsu-plaintext: \"Only inside the cluster network, which the mesh encrypts\"\n    security:\n      - $ref: '#/components/securitySchemes/sasl'\n"))],
        ),
        ("`security: []` on the document", vec![(PAYMENTS_API, ROOT_SECURITY.into(), "security: []\n".into())]),
        ("the empty requirement", vec![(PAYMENTS_API, ROOT_SECURITY.into(), "security:\n  - {}\n  - bearer: []\n".into())]),
        (
            "`security: []` on each operation",
            vec![
                (PAYMENTS_API, ROOT_SECURITY.into(), String::new()),
                (PAYMENTS_API, "      operationId: createCharge\n".into(), "      operationId: createCharge\n      security: []\n".into()),
                (PAYMENTS_API, "      operationId: getCharge\n".into(), "      operationId: getCharge\n      security: []\n".into()),
            ],
        ),
        (
            "a webhook without `security`",
            vec![
                (PAYMENTS_API, ROOT_SECURITY.into(), "webhooks:\n  chargeChanged:\n    post:\n      operationId: chargeChanged\n      responses:\n        '200':\n          description: Received\n".into()),
                (PAYMENTS_API, "      operationId: createCharge\n".into(), "      operationId: createCharge\n      security:\n        - bearer: []\n".into()),
                (PAYMENTS_API, "      operationId: getCharge\n".into(), "      operationId: getCharge\n      security:\n        - bearer: []\n".into()),
            ],
        ),
        (
            "a document no published language holds",
            vec![("payments/internal/admin.yaml", String::new(), "openapi: 3.1.0\ninfo:\n  title: Payments admin\n  version: 1.0.0\npaths:\n  /refunds:\n    post:\n      operationId: refund\n      responses:\n        '204':\n          description: Refunded\n".into())],
        ),
        (
            "AsyncAPI operations with `security`",
            vec![
                (PAYMENTS_EVENTS, String::new(), format!("{SASL}servers:\n  production:\n    host: broker.example.com:9093\n    protocol: kafka-secure\n")),
                (PAYMENTS_EVENTS, "  sendPaymentSucceeded:\n    action: send\n".into(), "  sendPaymentSucceeded:\n    action: send\n    security:\n      - $ref: '#/components/securitySchemes/sasl'\n".into()),
                (PAYMENTS_EVENTS, "  sendPaymentFailed:\n    action: send\n".into(), "  sendPaymentFailed:\n    action: send\n    security:\n      - $ref: '#/components/securitySchemes/sasl'\n".into()),
            ],
        ),
        ("`security: []` on a broker", vec![(PAYMENTS_EVENTS, String::new(), "servers:\n  production:\n    host: broker.example.com:9093\n    protocol: kafka-secure\n    security: []\n".into())]),
        ("a key for tests", vec![("CONTEXT", String::new(), format!("# the maps key of the staging site: {key}   # ritsu: test secret\n"))]),
        ("AWS's example key", vec![("CONTEXT", String::new(), format!("# an access key ID from AWS's documents: {aws}\n"))]),
    ];
    for (example, context) in [(EN, "contexts/payments.ctx"), (JA, "contexts/決済.ctx")] {
        for (what, edits) in &cases {
            let edits: Vec<(&str, &str, &str)> = edits.iter().map(|(f, o, n)| (if *f == "CONTEXT" { context } else { *f }, o.as_str(), n.as_str())).collect();
            let d = changed(example, &edits);
            let (codes, text) = codes_of(&d);
            assert!(codes.is_empty(), "{example}, {what}: {codes:?}\n{text}");
        }
    }
}

/// What is said and how: a variable's value from its `enum`, and an `x-ritsu-plaintext` with no
/// reason, each a W902 with the note that says why.
#[test]
fn a_server_variable_and_a_reason_left_empty() {
    for example in [EN, JA] {
        let d = changed(example, &[(PAYMENTS_API, "", "servers:\n  - url: '{scheme}://payments.example.com/v1'\n    variables:\n      scheme:\n        default: https\n        enum: [https, http]\n")]);
        let (codes, text) = codes_of(&d);
        assert_eq!(codes, ["W902"], "{text}");
        assert!(text.contains("With the server variable `scheme` at `http`, the URL is http://payments.example.com/v1."), "{text}");
        assert!(text.contains("サーバー変数 `scheme` が `http` のとき、URL は http://payments.example.com/v1 です。"), "{text}");
        let d = changed(example, &[(PAYMENTS_API, "", "servers:\n  - url: http://payments.internal/v1\n    x-ritsu-plaintext: \"\"\n")]);
        let (codes, text) = codes_of(&d);
        assert_eq!(codes, ["W902"], "{text}");
        assert!(text.contains("`x-ritsu-plaintext` takes the reason the connection is safe another way, as a string that is not empty."), "{text}");
    }
}

/// A key is said wherever the map's files hold it, in a map that does not read too: the check
/// stops at the first stage, and the key is still told.
#[test]
fn a_key_is_told_in_a_map_that_does_not_read() {
    let key = fake_key();
    let d = changed(EN, &[("webshop.ctx", "covers \".\"\n", &format!("covers \".\"\nnot a section  # {key}\n"))]);
    let (codes, text) = codes_of(&d);
    assert!(codes.contains(&"W901".to_string()) && codes.iter().any(|c| c.starts_with('E')), "{codes:?}\n{text}");
    // never the key itself, in the text or in the JSON
    assert!(!text.contains(&key), "{text}");
    let os = common::check_dir(d.path());
    let json: String = os.iter().map(|o| sakai::check::to_json(o, Lang::En).to_string()).collect();
    assert!(json.contains("\"W901\"") && !json.contains(&key), "{json}");
}
