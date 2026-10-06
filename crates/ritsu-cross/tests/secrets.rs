//! W901 of the contracts (DESIGN 16.3): a key written in a `.proto` of the project, or in a document
//! a language reads (a `.yaml` a flow's `use openapi` names, a `.json` a rule's `shape … jsonschema`
//! names), is said once, by ritsu, whatever reads it; a key whose line says `ritsu: test secret`, and
//! an AWS key ID that ends in `EXAMPLE`, are not. The project is laid out here, its keys put together
//! from pieces, so that no file of the repository holds them whole.

use ritsu_base::text::Lang;
use ritsu_project::{Joined, Project};
use ritsu_testkit::TempDir;

/// The fake key of the materials (DESIGN 16.10) and an AWS key ID of AWS's documents.
fn google() -> String {
    ["AIzaSyD-ritsu-fake-", "key-for-tests-000000"].concat()
}
fn aws_example() -> String {
    ["AKIA", "IOSFODNN7", "EXAMPLE"].concat()
}

/// A project of a `.proto`, a flow that calls an OpenAPI document (twice, from two flows), and a
/// rule that reads a JSON Schema.
fn project() -> TempDir {
    let t = TempDir::new("contract-keys");
    let k = google();
    t.write("maps.proto", format!("syntax = \"proto3\";\n\npackage maps.v1;\n\n// the Google Maps API key the service is called with: {k}\nmessage Place {{\n  string id = 1;\n}}\n"));
    t.write(
        "api/maps.yaml",
        format!(
            "openapi: 3.1.0\ninfo:\n  title: Maps\n  version: 1.0.0\nservers:\n  - url: https://maps.example.com/v1?key={k}\n  - url: https://test.maps.example.com/v1?key={k}   # ritsu: test secret\npaths: {{}}\n"
        ),
    );
    let flow = |name: &str| format!("workflow {name} v1\ndescription \"Looks a place up\"\n\nuse openapi maps from \"api/maps.yaml\"\n\ninputs\n  place : string\n\ntask find(place: string)\n  http GET \"https://maps.example.com/v1/places\"\n  idempotent\n\nflow\n  find(place: place)\n");
    t.write("lookup.flow", flow("lookup"));
    t.write("again.flow", flow("again"));
    t.write(
        "contracts/order.schema.json",
        format!("{{\n  \"$defs\": {{\n    \"Order\": {{\n      \"type\": \"object\",\n      \"examples\": [{{\"apiKey\": \"{k}\"}}, {{\"awsKey\": \"{}\"}}]\n    }}\n  }}\n}}\n", aws_example()),
    );
    t.write("order_fee.rule", "rule order_fee v1\n\nshape order = jsonschema \"contracts/order.schema.json\" \"#/$defs/Order\"\n\ninputs\n  total : number  range >=0 <=1000\n\noutputs\n  fee : number  round down(1)\n\ntable pick\npolicy unique\n| total | -> fee : number |\n| <500  | 100             |\n| >=500 | 0               |\n");
    t
}

#[test]
fn a_key_in_a_contract_is_said_once_by_ritsu() {
    let t = project();
    let root = t.path().to_string_lossy().to_string();
    let p = Project::load(std::slice::from_ref(&root), Some(&root)).expect("the project loads");
    let joined = Joined::new();
    assert_eq!(ritsu_cross::secrets::contracts(&p, &joined), ["api/maps.yaml", "contracts/order.schema.json", "maps.proto"]);
    for lang in [Lang::En, Lang::Ja] {
        let found = ritsu_cross::secrets::keys(&p, &joined, lang);
        let at: Vec<(String, Option<usize>)> = found.iter().map(|f| (f.file.clone().unwrap_or_default(), f.line)).collect();
        assert_eq!(
            at,
            [("api/maps.yaml".to_string(), Some(6)), ("contracts/order.schema.json".to_string(), Some(5)), ("maps.proto".to_string(), Some(5))],
            "the YAML document is said once although two flows name it; its line for tests, and the AWS example, are not said"
        );
        for f in &found {
            assert_eq!((f.code.as_str(), f.severity), ("W901", ritsu_base::diag::Severity::Warning));
            assert!(!f.text.contains(&google()), "{}", f.text);
            assert!(!f.json.compact().contains(&google()));
        }
        let note = |i: usize| found[i].text.clone();
        let (yaml, json, proto) = match lang {
            Lang::En => ("write `# ritsu: test secret` on the same line", "JSON has no comments", "write `// ritsu: test secret` on the same line"),
            Lang::Ja => ("同じ行に `# ritsu: test secret` と書いてください", "JSON にはコメントが書けません", "同じ行に `// ritsu: test secret` と書いてください"),
        };
        assert!(note(0).contains(yaml), "{}", note(0));
        assert!(note(1).contains(json), "{}", note(1));
        assert!(note(2).contains(proto), "{}", note(2));
    }
}

/// What the English and the Japanese text say of the `.proto`'s key, in full.
#[test]
fn the_text_of_a_key_in_a_proto() {
    let t = project();
    let root = t.path().to_string_lossy().to_string();
    let p = Project::load(std::slice::from_ref(&root), Some(&root)).unwrap();
    let joined = Joined::new();
    let shown = p.shown.path("maps.proto");
    let en = ritsu_cross::secrets::keys(&p, &joined, Lang::En).pop().unwrap().text;
    assert_eq!(
        en,
        format!(
            "warning[W901]: {shown}:5:56: A Google API key is written here (AIza…, 39 characters)\n  = A key in a file reaches everyone who can read the repository, its history and its builds. Keep it where the code runs (an environment variable, the platform's connection or secret store) and read it from there.\n  = If this key is real, revoke it with Google first: taking it out of the file leaves it in the history of the repository.\n  = If it is a value for tests, write `// ritsu: test secret` on the same line.\n"
        )
    );
    let ja = ritsu_cross::secrets::keys(&p, &joined, Lang::Ja).pop().unwrap().text;
    assert_eq!(
        ja,
        format!(
            "警告[W901]: {shown}:5:56: Google の API キーがここに書かれています（AIza…、39 文字）\n  = ファイルに書いた鍵は、リポジトリとその履歴とビルドを読めるすべての人に渡ります。鍵はコードが動くところ（環境変数、プラットフォームの接続やシークレットの置き場）に置き、そこから読んでください。\n  = 本物の鍵なら、まず Google で無効にしてください。ファイルから消しても、リポジトリの履歴には残ります。\n  = テスト用の値なら、同じ行に `// ritsu: test secret` と書いてください。\n"
        )
    );
}
