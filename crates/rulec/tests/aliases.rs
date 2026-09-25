//! The names an alias cannot take because a target language already has them (W121, §15.103).
//!
//! The rule's alias names more than a function: it is the file of the Python module, the Go
//! package and the Ruby module. Measured before §15.149, a rule aliased `time` generated
//! Python that imported itself in place of the standard `time`, a Go module that made every
//! `import "time"` ambiguous, and a Ruby `module Time` that stops because `Time` is a class —
//! and `check` said nothing about any of them.

fn notes(src: &str) -> Vec<String> {
    rulec::check_source(src, "a.rule")
        .into_iter()
        .filter(|d| d.code == "W121")
        .flat_map(|d| d.notes.into_iter())
        .collect()
}

fn rule(alias: &str) -> String {
    format!(
        "rule t({alias}) v1\n\ninputs\n  a(a) : bool\n\noutputs\n  x(x) : bool\n\n\
         table u(u)\npolicy unique\n| a     | -> x(x) : bool |\n| true  | true           |\n| false | false          |\n"
    )
}

/// The languages a note names, from its head.
fn named(notes: &[String]) -> Vec<String> {
    notes
        .iter()
        .filter_map(|n| n.split_once(": ").map(|(h, _)| h.to_string()))
        .flat_map(|h| h.split(", ").map(|s| s.to_string()).collect::<Vec<_>>())
        .collect()
}

#[test]
fn 規則の別名は標準ライブラリのモジュールの名前と比べる() {
    rulec::i18n::set(rulec::i18n::Lang::En);
    for (alias, want) in [
        ("time", &["Python", "Go", "Ruby"][..]),
        ("types", &["Python"][..]),
        ("signal", &["Python", "Ruby"][..]),
        ("str_conv", &["Go"][..]),
        ("date_time", &["Ruby"][..]),
    ] {
        let ns = notes(&rule(alias));
        let langs = named(&ns);
        for w in want {
            assert!(langs.iter().any(|l| l == w), "{alias}: {w} が名指しされない: {ns:?}");
        }
    }
    // An alias no standard library has is quiet.
    assert!(notes(&rule("order_state")).is_empty());
}

/// Only the rule's alias names a module. An enum's becomes a type inside it, so the module
/// names are not held against it.
#[test]
fn 列挙の別名はモジュールの名前と比べない() {
    rulec::i18n::set(rulec::i18n::Lang::En);
    let src = "rule t(fee_check) v1\n\nenum k(types) = p(p) | q(q)\n\ninputs\n  a(a) : k\n\noutputs\n  x(x) : bool\n\n\
               table u(u)\npolicy unique\n| a | -> x(x) : bool |\n| p | true           |\n| q | false          |\n";
    let ns = notes(src);
    assert!(!ns.iter().any(|n| n.contains("module")), "{ns:?}");
}

/// A class the generated Java imports is taken at the top of its file: `List` beside
/// `import java.util.List` does not compile.
#[test]
fn javaが読み込むクラスの名前も比べる() {
    rulec::i18n::set(rulec::i18n::Lang::En);
    let langs = named(&notes(&rule("list")));
    assert!(langs.iter().any(|l| l == "Java"), "{langs:?}");
}
