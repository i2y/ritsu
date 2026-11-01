//! The lists as the standards give them, and the names and literals made from them.

use ritsu_emit::words::{Words, go, postgresql, python, rust, typescript};
use ritsu_emit::{header, ident, lit};

/// No word twice in what a target is held to, and as many as the standards list.
#[test]
fn the_lists_are_the_standards() {
    for (name, w, n) in [("TypeScript", typescript::NAMES, 51), ("Python", python::NAMES, 39), ("Go", go::NAMES, 69), ("Rust", rust::NAMES, 52), ("PostgreSQL", postgresql::NAMES, 173)] {
        let all: Vec<&str> = w.iter().collect();
        let mut seen = std::collections::BTreeSet::new();
        assert!(all.iter().all(|x| seen.insert(*x)), "{name} has a word twice");
        assert_eq!(all.len(), n, "{name}");
    }
    assert_eq!(python::KEYWORDS.len(), 35);
    assert_eq!(go::KEYWORDS.len(), 25);
    assert!(typescript::NAMES.contains("implements") && !typescript::NAMES.contains("of"));
    assert!(go::NAMES.contains("complex128") && rust::NAMES.contains("Self") && postgresql::NAMES.contains("strict"));
    assert!(Words(&[&["a"], &["b"]]).contains("b"));
}

#[test]
fn names() {
    assert_eq!(ident::pascal("payment_at"), "PaymentAt");
    assert_eq!(ident::pascal("_a__b_"), "AB");
    assert_eq!(ident::go_package("payment_terms"), "paymentterms");
    assert_eq!((ident::go_exported("sku"), ident::go_exported("SKU"), ident::go_exported("引当"), ident::go_exported("ä")), ("Sku".into(), "SKU".into(), "X引当".into(), "Ä".into()));
    assert!(ident::is_ascii_ident("_a1") && !ident::is_ascii_ident("1a") && !ident::is_ascii_ident("在庫"));
    assert_eq!(ident::aside("class", |w| python::KEYWORDS.contains(&w)), "class_");
    assert_eq!(ident::aside("klass", |w| python::KEYWORDS.contains(&w)), "klass");
    assert_eq!(ident::unique(vec!["a".into(), "a".into(), "b".into(), "a".into()], &["b"]), ["a", "a_2", "b_2", "a_3"]);
    // Rust: a keyword as a raw identifier, a word no raw identifier writes moved aside
    assert_eq!((ident::rust_name("ref"), ident::rust_name("gen"), ident::rust_name("self"), ident::rust_name("Self"), ident::rust_name("_"), ident::rust_name("sku")), ("r#ref".into(), "r#gen".into(), "self_".into(), "Self_".into(), "__".into(), "sku".into()));
    assert_eq!((ident::rust_raw("crate"), ident::rust_raw("move")), (None, Some("r#move".into())));
    assert_eq!(ritsu_emit::package::rust_path("deps/pricing/rules/fee"), "crate::deps::pricing::rules::fee");
    assert_eq!(ritsu_emit::package::rust_path("glue/type"), "crate::glue::r#type");
}

/// The characters each target takes in a name (src/xid.rs): each table is ranges in order, apart,
/// of characters a name of ritsu's languages holds where the table is read; and names that part
/// the targets. kaname's tests/idents.rs holds the tables to the targets' tools.
#[test]
fn the_characters_of_names() {
    use ritsu_emit::xid;
    for (name, table, first) in [
        ("NOT_FIRST", xid::NOT_FIRST, true),
        ("NOT_AFTER", xid::NOT_AFTER, false),
        ("ID_NOT_FIRST", xid::ID_NOT_FIRST, true),
        ("ID_NOT_AFTER", xid::ID_NOT_AFTER, false),
        ("GO_NOT_FIRST", xid::GO_NOT_FIRST, true),
        ("GO_NOT_AFTER", xid::GO_NOT_AFTER, false),
    ] {
        assert!(table.iter().all(|&(lo, hi)| lo <= hi) && table.windows(2).all(|w| w[0].1 + 1 < w[1].0), "{name} is not ranges in order and apart");
        for &(lo, hi) in table {
            for u in lo..=hi {
                let c = char::from_u32(u).unwrap();
                assert!(c != '_' && if first { c.is_alphabetic() } else { c.is_alphanumeric() }, "{name} has U+{u:04X}, which a name of ritsu's languages does not hold there");
            }
        }
    }
    assert_eq!(xid::UNICODE, (17, 0, 0));
    // a name, and whether Rust, Python, TypeScript and Go take it
    for (n, takes) in [
        ("a_1", [true, true, true, true]),
        ("在庫", [true, true, true, true]),
        ("x\u{FF11}", [true, true, true, true]),  // a fullwidth digit, Nd
        ("m²", [false, false, false, false]),      // `²` is a number of No
        ("कीमत", [true, true, true, false]),       // `ी` is a vowel sign, Mc
        ("Ⅻ", [true, true, true, false]),          // a number of Nl
        ("\u{FF9E}", [false, false, true, true]), // the halfwidth voiced sound mark, Lm, not XID_Start
        ("\u{37A}", [false, false, true, true]),  // ͺ, Lm, neither XID_Start nor XID_Continue
        ("\u{345}a", [false, false, false, false]),
        ("a\u{345}", [true, true, true, false]),  // a combining letter, Mn
        ("\u{FF11}x", [false, false, false, false]),
        ("℃", [false, false, false, false]),
    ] {
        let got = [ident::rust_takes(n), ident::python_takes(n), ident::typescript_takes(n), ident::go_takes(n)];
        assert_eq!(got, takes, "{n:?} ({})", n.chars().map(|c| format!("U+{:04X}", c as u32)).collect::<Vec<_>>().join(" "));
    }
}

#[test]
fn literals() {
    let s = "a\"b'c\\d\ne\r\tf\u{1}g\u{7f}h本";
    assert_eq!(lit::json(s), "\"a\\\"b'c\\\\d\\ne\\r\\tf\\u0001g\u{7f}h本\"");
    assert_eq!(lit::python(s), "'a\"b\\'c\\\\d\\ne\\r\\tf\\x01g\u{7f}h本'");
    assert_eq!(lit::go(s), "\"a\\\"b'c\\\\d\\ne\\r\\tf\\x01g\\x7fh本\"");
    assert_eq!(lit::rust(s), "\"a\\\"b'c\\\\d\\ne\\r\\tf\\u{1}g\\u{7f}h本\"");
    assert_eq!(lit::sql("本店's"), "'本店''s'");
    assert_eq!(lit::sql_ident("在\"庫"), "\"在\"\"庫\"");
}

#[test]
fn the_generated_line() {
    assert_eq!(header::generated("koyomi"), format!("Code generated by koyomi {}. DO NOT EDIT.", env!("CARGO_PKG_VERSION")));
    let s = header::Source { path: "dates/payment_terms.cal", kind: "dates", name: "payment_terms", version: "1", sha256: &ritsu_base::sha256::hex(b"x"), module: None };
    assert_eq!(s.line().get(ritsu_base::text::Lang::En), "Source: dates/payment_terms.cal (dates payment_terms v1, sha256:2d711642b726b044)");
    assert_eq!(s.line().get(ritsu_base::text::Lang::Ja), "もと: dates/payment_terms.cal（dates payment_terms v1、sha256:2d711642b726b044）");
    // a dependency's file, the canonical way, with its module's name and version (ritsu's DESIGN 17.8)
    let pricing = ritsu_emit::package::Module { name: "example.com/acme/pricing".to_string(), version: Some("1.1.0".to_string()) };
    let s = header::Source { path: "@pricing/rules/fee.rule", kind: "rule", name: "fee", version: "1", sha256: "0123456789abcdef", module: Some(&pricing) };
    assert_eq!(s.line().get(ritsu_base::text::Lang::En), "Source: @pricing/rules/fee.rule (rule fee v1, sha256:0123456789abcdef, module example.com/acme/pricing 1.1.0)");
    assert_eq!(s.line().get(ritsu_base::text::Lang::Ja), "もと: @pricing/rules/fee.rule（rule fee v1、sha256:0123456789abcdef、モジュール example.com/acme/pricing 1.1.0）");
    let local = ritsu_emit::package::Module { name: "lib".to_string(), version: None };
    let s = header::Source { module: Some(&local), ..s };
    assert_eq!(s.line().get(ritsu_base::text::Lang::En), "Source: @pricing/rules/fee.rule (rule fee v1, sha256:0123456789abcdef, module lib)");
    assert_eq!(header::file_name("rules/送料.rule"), "送料.rule");
    assert_eq!(header::Comment::Slashes.line("x"), "// x\n");
    assert_eq!(header::Comment::Hash.line("x"), "# x\n");
    assert_eq!(header::Comment::Dashes.line("x"), "-- x\n");
}

/// Text a source file holds never ends the comment it is written in (DESIGN 9.2). A file's name
/// with a line break in it, put in the head as it is, made the rest of the name a line of the
/// generated Python; a description of a flow with `\n` in it, a line of the TypeScript, the Go and
/// the Python, and a document of its own in the YAML for Argo.
#[test]
fn what_a_source_says_stays_in_its_comment() {
    let breaks = "a\nb\rc\u{85}d\u{2028}e\u{2029}f";
    // each break is written `U+XXXX`: no `\` and no `\u`, so it reads the same in every target and
    // is safe where a `\uXXXX` in a comment is read (Java, Scala 2)
    assert_eq!(header::one_line(breaks), "aU+000AbU+000DcU+0085dU+2028eU+2029f");
    assert_eq!(header::one_line("rules/送料.rule"), "rules/送料.rule");
    let s = header::Source { path: "rules/pickup\nprint('ran') #.rule", kind: "rule", name: "pickup", version: "1", sha256: "0123456789abcdef", module: None };
    assert_eq!(s.line().get(ritsu_base::text::Lang::En), "Source: rules/pickupU+000Aprint('ran') #.rule (rule pickup v1, sha256:0123456789abcdef)");
    assert_eq!(header::Comment::Hash.line("one\ntwo"), "# oneU+000Atwo\n");
    // a description of several lines: a comment on each, `\r\n` one break, an empty line a bare mark
    assert_eq!(header::Comment::Slashes.lines("order v1: An order.\nIt ships."), "// order v1: An order.\n// It ships.\n");
    assert_eq!(header::Comment::Hash.lines("a\r\nb\rc\u{85}d\u{2028}e\u{2029}f\n\ng"), "# a\n# b\n# c\n# d\n# e\n# f\n#\n# g\n");
    assert_eq!(header::Comment::Dashes.lines("one line"), "-- one line\n");
    for c in [header::Comment::Slashes, header::Comment::Hash, header::Comment::Dashes] {
        for line in c.lines(breaks).lines().chain(c.line(breaks).lines()) {
            assert!(line.starts_with(c.mark()), "{line:?} is not a comment");
            assert!(!line.contains(header::LINE_BREAKS), "{line:?} holds a line break");
        }
    }
}

/// A comment of a target that reads `\uXXXX` before the comment (Java, Scala 2): a line break is
/// `U+XXXX`, which holds no `\` and no `\u`, and the backslashes the text itself holds are doubled,
/// so no `u` follows an even run of `\` and no escape (`\u000a` → a break; `\u{…}` → a stop) begins.
#[test]
fn a_comment_safe_where_unicode_escapes_are_read() {
    // the six characters `\`, `u`, `0`, `0`, `0`, `a`: a line break to Java, inert once doubled
    let cited = "https://example.com/x\\u000a\u{2028}y";
    let line = header::for_unicode_comment(&header::one_line(cited));
    assert_eq!(line, "https://example.com/x\\\\u000aU+2028y");
    assert!(!line.contains('\u{2028}'), "no raw line break is left");
    // a `\` that is followed by `u` begins an escape only when an even number of `\` stands before
    // it (Java 3.3); every such `\` here has an odd count before it, so none begins one
    let cs: Vec<char> = line.chars().collect();
    for i in 0..cs.len() {
        if cs[i] == '\\' && cs.get(i + 1) == Some(&'u') {
            let before = cs[..i].iter().rev().take_while(|x| **x == '\\').count();
            assert!(before % 2 == 1, "the `\\` before the `u` at {i} has {before} backslashes before it (even), and would begin an escape");
        }
    }
    // doubling leaves the `u` after each run of `\` preceded by an odd count
    assert_eq!(header::for_unicode_comment("a\\ub\\\\uc"), "a\\\\ub\\\\\\\\uc");
    // text with no backslash is unchanged
    assert_eq!(header::for_unicode_comment("rules/送料.rule"), "rules/送料.rule");
}

/// The world and the package a Wasm module's `.wit` declares (src/wasm.rs): a name's words joined by
/// `-`, a keyword of WIT `%`-escaped, a version of one number as a semver; and the three functions of
/// the canonical ABI, with what `call` answers. koyomi's and kaname's tests/wasm.rs hold the worlds
/// to wasm-tools and the modules to a host and to wasmtime.
#[test]
fn the_world_and_the_abi_of_a_wasm_module() {
    use ritsu_base::text::Lang;
    use ritsu_emit::wasm;
    let worlds = ["payment_terms", "close_20th", "a__b", "trailing_", "Fee", "record", "error_context", "records"].map(wasm::world);
    assert_eq!(worlds, ["payment-terms", "close-20th", "a-b", "trailing", "fee", "%record", "%error-context", "records"]);
    assert_eq!(["1", "007", "123456789012345678901"].map(wasm::semver), [Some("1.0.0".into()), Some("7.0.0".into()), None]);
    assert_eq!((wasm::package("koyomi", "payment_terms", Some("1.0.0")), wasm::package("kaname", "use", None)), ("koyomi:payment-terms@1.0.0".into(), "kaname:%use".into()));
    assert_eq!(wasm::component_line("fees"), "wasm-tools component embed fees.wit fees.wasm -o fees.embedded.wasm && wasm-tools component new fees.embedded.wasm -o fees.component.wasm");
    let mut seen = std::collections::BTreeSet::new();
    assert!(wasm::WIT_KEYWORDS.iter().all(|k| seen.insert(*k) && k.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')));
    for lang in [Lang::En, Lang::Ja] {
        let abi = wasm::abi(lang, "answer(text)", "refusal()");
        for f in ["pub unsafe extern \"C\" fn cabi_realloc(", "pub unsafe extern \"C\" fn call(", "pub unsafe extern \"C\" fn cabi_post_call("] {
            assert_eq!(abi.matches(f).count(), 1, "{f}");
        }
        assert_eq!(abi.matches("#[unsafe(no_mangle)]\n").count(), 3);
        assert!(abi.contains("        Ok(text) => answer(text),\n        Err(_) => refusal(),\n"), "{abi}");
        assert_eq!(abi.lines().filter(|l| l.starts_with("/// ")).count(), 3, "{abi}");
    }
}
