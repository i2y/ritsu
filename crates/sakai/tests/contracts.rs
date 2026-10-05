//! The OpenAPI and AsyncAPI documents (DESIGN 15), on the example of them: `examples/webshop`
//! (English names) and its Japanese twin `examples/webshop.ja`. Each passes check, and says what
//! crosses and how; the two say the same but for the names; their api and their CML are their
//! golden files; and Context Mapper 6.12.0's validator finds nothing in the CML. The diagnostics
//! of the documents are the mutants' (`tests/mutants/E108_…`, `W104_…`, `E210_…` and the others
//! on the example), and the pages of `doc` are `tests/doc.rs`'s.

mod common;

use ritsu_base::text::Lang;
use std::process::Command;
use std::time::Duration;

const EN: (&str, &str) = ("examples/webshop", "webshop.ctx");
const JA: (&str, &str) = ("examples/webshop.ja", "ネットショップ.ctx");

fn checked((example, map): (&str, &str)) -> sakai::check::Outcome {
    let ex = std::fs::canonicalize(example).unwrap();
    let o = sakai::check::check_map_with(&ex, map, &common::suite()).unwrap();
    assert!(!o.has_errors(), "{example}/{map}: {}", sakai::check::render(&o, Lang::En));
    o
}

#[test]
fn the_example_of_the_documents_passes_check() {
    for ((example, map), stem) in [(EN, "webshop"), (JA, "ネットショップ")] {
        let path = format!("{example}/{map}");
        let (code, out, err) = common::joined(&["check", &path]);
        assert_eq!((code, err.as_str()), (0, ""), "{out}");
        assert_eq!(out, format!("{path}: ok — 4 contexts, 5 relationships; 9 artifacts, each in one context; 7 crossings checked (openapi 1, asyncapi 5, dandori 1)\n"), "{stem}");
        let (_, out, _) = common::joined(&["check", &path, "--lang", "ja"]);
        assert_eq!(out, format!("{path}: ok — コンテキスト 4、関係 5。成果物 9 件は、どれも一つのコンテキストに属する。境界を越える参照 7 件を確かめた（openapi 1、asyncapi 5、dandori 1）\n"), "{stem}");
    }
}

/// What each crossing is: from which context to which, how (`$ref`, `send`, `receive`, `use
/// openapi`), and what allows it.
#[test]
fn each_crossing_of_the_documents_says_how_it_refers() {
    let o = checked(EN);
    let c = o.checked.as_ref().unwrap();
    let name = |i: usize| c.model.contexts[i].name.clone();
    let mut got: Vec<String> = c
        .crossings
        .iter()
        .map(|x| {
            let allowed = match &x.allowed {
                Some(sakai::refs::Allowed::Kernel(_)) => "shared kernel".to_string(),
                Some(sakai::refs::Allowed::Upstream(ri)) => c.model.contexts[x.from_ctx].rels[*ri].roles().iter().map(|r| r.word()).collect::<Vec<_>>().join(", "),
                Some(sakai::refs::Allowed::Partnership) => "partnership".to_string(),
                None => "none".to_string(),
            };
            let at = x.pointer.as_ref().map(|p| format!("#{p}")).unwrap_or_default();
            format!("{} -> {} {} {}{at} ({allowed})", name(x.from_ctx), name(x.to_ctx), x.kind.via(), x.to.trim_start_matches("examples/webshop/"))
        })
        .collect();
    got.sort();
    assert_eq!(
        got,
        [
            "Notifications -> Ordering receive ordering/events/ordering.yaml#/channels/orderCancelled (conformist)",
            "Notifications -> Ordering receive ordering/events/ordering.yaml#/channels/orderPlaced (conformist)",
            "Notifications -> Ordering use openapi ordering/api/ordering.json (conformist)",
            "Payments -> Ordering $ref common/money.yaml#/Money (shared kernel)",
            "Payments -> Ordering receive ordering/events/ordering.yaml#/channels/orderPlaced (conformist)",
            "Shipping -> Payments receive payments/events/payments.yaml#/channels/paymentFailed (anticorruption layer)",
            "Shipping -> Payments receive payments/events/payments.yaml#/channels/paymentSucceeded (anticorruption layer)",
        ]
    );
    // what a channel takes along: its message and the payload's schemas, the enum among them
    let succeeded = c.crossings.iter().find(|x| x.pointer.as_deref() == Some("/channels/paymentSucceeded")).unwrap();
    let reached: Vec<String> = succeeded.elements.iter().map(|(f, p)| format!("{}#{p}", f.trim_start_matches("examples/webshop/"))).collect();
    for want in ["payments/events/payments.yaml#/components/messages/PaymentResult", "payments/api/payments.yaml#/components/schemas/Charge", "payments/api/payments.yaml#/components/schemas/ChargeStatus", "common/money.yaml#/Money"] {
        assert!(reached.iter().any(|r| r == want), "{want} is not reached: {reached:?}");
    }
}

/// CML without its comments and strings.
fn bare(cml: &str) -> String {
    let mut out = String::new();
    let mut rest = cml;
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix("//") {
            rest = r.split_once('\n').map(|x| x.1).unwrap_or("");
            out.push('\n');
        } else if let Some(r) = rest.strip_prefix("/*") {
            rest = r.split_once("*/").map(|x| x.1).unwrap_or("");
        } else if let Some(r) = rest.strip_prefix('"') {
            rest = r.split_once('"').map(|x| x.1).unwrap_or("");
            out.push_str("\"\"");
        } else {
            let c = rest.chars().next().unwrap();
            out.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    out
}

/// The two maps are one map in two languages: every crossing goes the same way between the same
/// contexts (read by their aliases), and the CML is the same but for its comments and strings.
#[test]
fn the_two_maps_of_the_documents_say_the_same_but_for_the_names() {
    let (en, ja) = (checked(EN), checked(JA));
    let (en, ja) = (en.checked.as_ref().unwrap(), ja.checked.as_ref().unwrap());
    let crossings = |c: &sakai::check::Checked| {
        let mut v: Vec<String> = c.crossings.iter().map(|x| format!("{} -> {} {} {:?} {:?}", c.model.contexts[x.from_ctx].alias, c.model.contexts[x.to_ctx].alias, x.kind.via(), x.allowed, x.pointer)).collect();
        v.sort();
        v
    };
    assert_eq!(crossings(en).len(), 7);
    assert_eq!(crossings(en), crossings(ja));
    for lang in [Lang::En, Lang::Ja] {
        let (a, b) = (sakai::cml::render(en, EN.1, lang), sakai::cml::render(ja, JA.1, lang));
        assert_ne!(a, b, "the comments and strings differ");
        assert_eq!(bare(&a), bare(&b), "the CML of the two maps, without comments and strings ({lang:?})");
    }
}

#[test]
fn the_api_of_the_example_of_the_documents_is_its_golden_file() {
    let mut failures = Vec::new();
    for ((example, map), stem) in [(EN, "webshop"), (JA, "ネットショップ")] {
        let ex = std::fs::canonicalize(example).unwrap();
        let o = sakai::check::check_map_with(&ex, map, &common::suite()).unwrap();
        let text = serde_json::to_string_pretty(&sakai::api::api(o.checked.as_ref().unwrap())).unwrap() + "\n";
        if let Some(f) = common::golden(&format!("tests/golden/api/{stem}.json"), &text) {
            failures.push(f);
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn cml((example, map): (&str, &str), lang: Lang) -> String {
    let o = checked((example, map));
    sakai::cml::render(o.checked.as_ref().unwrap(), map, lang)
}

#[test]
fn the_cml_of_the_example_of_the_documents_is_its_golden_file() {
    let mut failures = Vec::new();
    for (map, stem) in [(EN, "webshop"), (JA, "ネットショップ")] {
        for (lang, tag) in [(Lang::En, ""), (Lang::Ja, ".ja")] {
            if let Some(x) = common::golden(&format!("tests/golden/cml/{stem}{tag}.cml"), &cml(map, lang)) {
                failures.push(x);
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Context Mapper's validator, with every check, finds nothing in the CML of either map, in
/// either language (`tests/cml.rs` runs it on the shop's, and shows it at work).
#[test]
fn context_mapper_finds_nothing_wrong_in_the_cml_of_the_documents() {
    if !common::linters() {
        return;
    }
    let (Some(java), Some(javac), Some(lib)) = (common::java("java"), common::java("javac"), common::cml_lib()) else {
        common::skip("Java or the Context Mapper CLI is not there (SAKAI_JAVA and SAKAI_JAVAC, or JAVA_HOME; SAKAI_CML_LIB, or tools/cml: tools/cml/fetch.sh)");
        return;
    };
    let dir = common::TempDir::new("context-mapper-webshop");
    let classes = dir.path().join("classes");
    let cp = format!("{}/*", lib.display());
    let limit = Duration::from_secs(180);
    let b = common::run(Command::new(&javac).arg("-cp").arg(&cp).arg("-d").arg(&classes).arg("tools/cml/Validate.java"), limit);
    assert!(b.ok, "javac: {}", b.stderr);
    let mut files = Vec::new();
    for (map, stem) in [(EN, "webshop"), (JA, "webshop-ja-names")] {
        for (lang, tag) in [(Lang::En, "en"), (Lang::Ja, "ja")] {
            let f = format!("{stem}.{tag}.cml");
            dir.write(&f, &cml(map, lang));
            files.push(f);
        }
    }
    for f in &files {
        let r = common::run(Command::new(&java).arg("-cp").arg(format!("{cp}:{}", classes.display())).arg("Validate").arg(f).current_dir(dir.path()), limit);
        assert!(r.ok && r.stdout.trim().is_empty(), "{f}: {}{}", r.stdout, r.stderr);
    }
    eprintln!("compared: Context Mapper 6.12.0 finds nothing in the {} CML files of the webshop", files.len());
}

/// sakai's crate alone joins no other language: the example holds a workflow, so it says E104,
/// and the documents themselves need no language but sakai's.
#[test]
fn the_documents_need_no_other_language() {
    let o = common::sakai(&["check", "examples/webshop/webshop.ctx"]);
    let out = String::from_utf8_lossy(&o.stdout);
    assert_eq!(o.status.code(), Some(2), "{out}");
    assert!(out.contains("error[E104]") && out.contains("dandori"), "{out}");
    assert!(!out.contains("E108") && !out.contains("E2"), "{out}");
}

/// The English example laid out in a temporary directory, each `(file, old, new)` replacing the
/// first `old` of the file with `new` (a file with no `old` is written whole).
fn changed(edits: &[(&str, &str, &str)]) -> common::TempDir {
    let dir = common::TempDir::new("webshop");
    common::copy_dir(std::path::Path::new(EN.0), dir.path());
    for (file, old, new) in edits {
        let p = dir.path().join(file);
        let text = if old.is_empty() {
            new.to_string()
        } else {
            let s = std::fs::read_to_string(&p).unwrap();
            assert!(s.contains(old), "{file} has no `{old}`");
            s.replacen(old, new, 1)
        };
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, text).unwrap();
    }
    dir
}

/// The codes `check` gives, and what it prints, in English.
fn codes_of(dir: &common::TempDir) -> (Vec<String>, String) {
    let os = common::check_dir(dir.path());
    let codes = os.iter().flat_map(|o| o.diags.iter().map(|d| d.code.to_string())).collect();
    (codes, os.iter().map(|o| sakai::check::render(o, Lang::En)).collect())
}

/// What else the documents are held to, each with one change to the example: a document no
/// context owns, documents mixed with a `.proto` in one published language, a `$ref` to a file
/// that is not there and to nothing in a file, a published language `through` does not list, and
/// the elements a context file names in a document.
#[test]
fn what_else_the_documents_are_held_to() {
    // a document is an artifact: one no context owns is E101
    let d = changed(&[("elsewhere/api.yaml", "", "openapi: 3.1.0\ninfo:\n  title: Elsewhere\n  version: 1.0.0\n")]);
    let (codes, text) = codes_of(&d);
    assert_eq!(codes, ["E101"], "{text}");
    assert!(text.contains("No context owns elsewhere/api.yaml"), "{text}");
    // documents and a `.proto` in one published language
    let d = changed(&[("contexts/shipping.ctx", "  openapi \"../shipping/api/shipping.yaml\"\n", "  openapi \"../shipping/api/shipping.yaml\"\n  proto \"../shipping/api/shipping.proto\"\n")]);
    let (codes, text) = codes_of(&d);
    assert!(codes.contains(&"E004".to_string()), "{text}");
    // a `$ref` to a file that is not there, and one to nothing in a file
    let d = changed(&[("payments/api/payments.yaml", "'../../common/money.yaml#/Money'", "'../../common/coins.yaml#/Money'")]);
    let (codes, text) = codes_of(&d);
    assert_eq!(codes, ["E108"], "{text}");
    assert!(text.contains("points at common/coins.yaml, which is not there"), "{text}");
    let d = changed(&[("payments/api/payments.yaml", "'../../common/money.yaml#/Money'", "'../../common/money.yaml#/Coins'")]);
    let (codes, text) = codes_of(&d);
    assert_eq!(codes, ["E108"], "{text}");
    assert!(text.contains("points at nothing (common/money.yaml has no #/Coins)"), "{text}");
    // Shipping goes through a package it does not list
    let d = changed(&[("contexts/shipping.ctx", "  through payments.v1\n", "  through payments.v2\n")]);
    let (codes, _) = codes_of(&d);
    assert!(codes.contains(&"E312".to_string()) && codes.contains(&"E203".to_string()), "{codes:?}");
    // the elements a context file names: a term that means an operation, and one that means a
    // value of an enum; and a name that is not there
    let d = changed(&[(
        "contexts/payments.ctx",
        "    means enum ChargeStatus\n",
        "    means enum ChargeStatus\n  refund \"Giving the money back\"\n    means enum ChargeStatus value refunded\n  charging \"Asking for the money\"\n    means operation createCharge\n",
    )]);
    let os = common::check_dir(d.path());
    assert!(!os.iter().any(|o| o.has_errors()), "{}", os.iter().map(|o| sakai::check::render(o, Lang::En)).collect::<String>());
    let c = os.iter().find_map(|o| o.checked.as_ref()).unwrap();
    let means: Vec<String> = c.elements.found.values().map(sakai::elements::display).filter(|m| m.contains("refunded") || m.contains("charges")).collect();
    assert_eq!(means, ["examples/webshop/payments/api/payments.yaml#/components/schemas/ChargeStatus value refunded", "examples/webshop/payments/api/payments.yaml#/paths/~1charges/post"].map(|s| s.trim_start_matches("examples/webshop/").to_string()), "{means:?}");
    let d = changed(&[("contexts/payments.ctx", "    means schema Charge\n", "    means schema Refund\n")]);
    let (codes, text) = codes_of(&d);
    assert_eq!(codes, ["E007"], "{text}");
}

/// A rule's `import jsonschema` of an enum of another context's OpenAPI document is a crossing
/// (DESIGN 15.5), held to the relationships as the rest: Payments, a conformist of Ordering, may
/// take Ordering's `OrderStatus`; Shipping, with no relationship to Ordering, may not.
#[test]
fn a_rule_that_takes_an_enum_of_a_document_crosses_into_it() {
    let rule = |depth: &str| {
        format!(
            "rule hold_fee v1\n\nimport jsonschema \"{depth}ordering/api/ordering.json\" \"#/components/schemas/OrderStatus\" -> order_status\nenum order_status = placed | paid | shipped | cancelled\n\ninputs\n  status : order_status\n\noutputs\n  fee : money[JPY, incl_tax]  round up(10JPY)\n\ntable fees\npolicy first\n| status | -> fee : money[JPY, incl_tax] |\n| placed | 0JPY |\n| paid | 100JPY |\n| shipped | 100JPY |\n| cancelled | 0JPY |\n"
        )
    };
    let r = rule("../../");
    let d = changed(&[("payments/rules/hold_fee.rule", "", &r)]);
    let os = common::check_dir(d.path());
    let text: String = os.iter().map(|o| sakai::check::render(o, Lang::En)).collect();
    assert!(text.contains("8 crossings checked (openapi 1, asyncapi 5, rulec 1, dandori 1)"), "{text}");
    let c = os.iter().find_map(|o| o.checked.as_ref()).unwrap();
    assert!(c.crossings.iter().any(|x| x.kind.via() == "import jsonschema" && x.to.ends_with("ordering/api/ordering.json")), "{text}");
    let d = changed(&[("shipping/rules/hold_fee.rule", "", &r)]);
    let (codes, text) = codes_of(&d);
    assert_eq!(codes, ["E201"], "{text}");
    assert!(text.contains("refers to ordering/api/ordering.json of Ordering (import jsonschema)"), "{text}");
}
