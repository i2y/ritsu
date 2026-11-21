//! The names an alias cannot take because a target language already has them (W121, §15.103).
//!
//! The rule's alias names more than a function: it is the file of the Python module, the Go
//! package and the Ruby module. Measured before §15.149, a rule aliased `time` generated
//! Python that imported itself in place of the standard `time`, a Go module that made every
//! `import "time"` ambiguous, and a Ruby `module Time` that stops because `Time` is a class —
//! and `check` said nothing about any of them.
//!
//! A builtin the generated code reaches another way where a name of the rule's hides it is not said
//! (§15.336), and each language's names are compared as it writes them: the rule's function in
//! PascalCase in Go, a value's alias as it stands, in the same case, in the languages that write it
//! so.

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
            assert!(langs.iter().any(|l| l == w), "{alias}: {w} が挙がらない: {ns:?}");
        }
    }
    // An alias no standard library has is quiet.
    assert!(notes(&rule("order_state")).is_empty());
}

/// The targets the warning keeps from being written are the languages it names (`Diag::targets`).
/// Python's module and Go's package meet the library's only where `rulec gen` writes them beside
/// the language's own, by the alias alone; `ritsu gen` writes them in a package of rules, by its
/// path (`Diag::beside`). Ruby's module is a constant at the top of any file, which `Time` meets
/// wherever it is.
#[test]
fn モジュールとぶつかるのが並べ方のためだけの出力先を分けて持つ() {
    rulec::i18n::set(rulec::i18n::Lang::En);
    for (alias, targets, beside) in [
        ("time", &["python", "ruby", "go"][..], &["python", "go"][..]),
        ("str_conv", &["go"][..], &["go"][..]),
        ("date_time", &["ruby"][..], &[][..]),
    ] {
        let ds: Vec<_> = rulec::check_source(&rule(alias), "a.rule").into_iter().filter(|d| d.code == "W121").collect();
        assert_eq!(ds.len(), 1, "{alias}: {:?}", ds.iter().map(|d| &d.title).collect::<Vec<_>>());
        assert_eq!(ds[0].targets, targets, "{alias}");
        assert_eq!(ds[0].beside, beside, "{alias}");
    }
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

/// A class of `java.lang`, which the generated Java reads by its simple name, is taken in the whole
/// package, where the rule's class of that name hides it: said, with the note of the package. A
/// class a file imports by a single import (`List` beside `import java.util.List`) is not: the
/// generated code then writes the JDK's class by its full name and leaves the import out (§15.336).
#[test]
fn javaが読み込むクラスの名前も比べる() {
    rulec::i18n::set(rulec::i18n::Lang::En);
    let ns = notes(&rule("string"));
    assert!(named(&ns).iter().any(|l| l == "Java"), "{ns:?}");
    assert!(ns.iter().any(|n| n.starts_with("Java: the rule's class `String` hides `java.lang`'s `String` in the whole package")), "{ns:?}");
    for alias in ["list", "map", "array_list", "linked_hash_map", "buffered_reader", "input_stream_reader", "print_stream", "standard_charsets"] {
        let langs = named(&notes(&rule(alias)));
        assert!(!langs.iter().any(|l| l == "Java"), "{alias}: {langs:?}");
    }
}

/// The W121s of `src`, each with its title and the targets it keeps from being written.
fn said(src: &str) -> Vec<(String, Vec<&'static str>)> {
    rulec::check_source(src, "a.rule").into_iter().filter(|d| d.code == "W121").map(|d| (d.title, d.targets)).collect()
}

/// A rule whose input `a` has the alias `alias`.
fn input(alias: &str) -> String {
    format!(
        "rule t(t) v1\n\ninputs\n  a({alias}) : bool\n\noutputs\n  x(x) : bool\n\n\
         table u(u)\npolicy unique\n| a     | -> x(x) : bool |\n| true  | true           |\n| false | false          |\n"
    )
}

/// A rule of one enum, declared as `decl` (`k(count)`, `sort`) and read as `ty`.
fn enumerated(decl: &str, ty: &str) -> String {
    format!(
        "rule t(t) v1\n\nenum {decl} = p(p) | q(q)\n\ninputs\n  a(a) : {ty}\n\noutputs\n  x(x) : bool\n\n\
         table u(u)\npolicy unique\n| a | -> x(x) : bool |\n| p | true           |\n| q | false          |\n"
    )
}

/// A value's alias is a parameter or a local, which a language that writes a name as it stands
/// cannot take where it is one of its keywords in the same case (`type` in Rust, `True` in Python).
/// Swift writes a keyword in backticks, Java with `_` after it, and PHP a value as a variable, so an
/// alias that is a keyword only there, or one only in other letters' case, is said nowhere.
#[test]
fn a_value_alias_is_held_to_the_keywords_of_the_languages_that_write_it_as_it_stands() {
    rulec::i18n::set(rulec::i18n::Lang::En);
    for alias in ["int", "float", "TRUE", "Class", "NULL", "Return", "echo", "extension"] {
        let ws = said(&input(alias));
        assert!(ws.is_empty(), "{alias}: {ws:?}");
    }
    for (alias, target) in [("type", "rust"), ("class", "python"), ("fn", "rust"), ("def", "ruby"), ("True", "python")] {
        let ws = said(&input(alias));
        assert!(ws.iter().any(|(_, ts)| ts.contains(&target)), "{alias}: {target} is not named: {ws:?}");
    }
}

/// The rule's function, by its alias or by its name, is at the top of the module, where it hides a
/// builtin of its spelling. The generated code reaches most such builtins another way (Python's
/// `len` and `sum`, PHP's `count`, Ruby's `format`), and those are said nowhere; one it reads as it
/// is (`undefined` in TypeScript and JavaScript) is said there. A keyword PHP writes its function
/// with `_` after (`list_`) is said too, and keeps no target from being written (§15.344).
#[test]
fn a_rule_is_said_to_hide_only_a_builtin_the_generated_code_reads_as_it_is() {
    rulec::i18n::set(rulec::i18n::Lang::En);
    for alias in ["len", "sum", "count", "format"] {
        let ws = said(&rule(alias));
        assert!(ws.is_empty(), "{alias}: {ws:?}");
    }
    let named_len = rule("t").replacen("rule t(t) v1", "rule len v1", 1);
    assert!(said(&named_len).is_empty(), "{:?}", said(&named_len));
    let ws = said(&rule("undefined"));
    assert_eq!(ws.iter().map(|(_, ts)| ts.clone()).collect::<Vec<_>>(), [vec!["typescript", "javascript"]], "{ws:?}");
    let ws = said(&rule("list"));
    assert_eq!(ws, [("The alias `list` is a keyword as written in PHP, so the generated code there writes it with `_` after it".to_string(), Vec::<&str>::new())], "{ws:?}");
    let ns = notes(&rule("list"));
    assert!(ns.iter().any(|n| n == "PHP: the rule's function is `list_` and its namespace `List_`, the names a caller reads it by."), "{ns:?}");
}

/// PHP keeps its classes apart from its functions: an enum's type, by its alias or by its name, meets
/// no builtin function (`Count`, `Sort`), and only PHP's keywords, which name no class: PHP writes
/// the class with `_` after one (`List_`), and the code builds, so that is said with no target
/// (§15.344).
#[test]
fn a_php_type_is_held_to_the_keywords_alone() {
    rulec::i18n::set(rulec::i18n::Lang::En);
    for (decl, ty) in [("k(count)", "k"), ("sort", "sort")] {
        let ws = said(&enumerated(decl, ty));
        assert!(ws.is_empty(), "{decl}: {ws:?}");
    }
    for (decl, ty, title) in [
        ("k(list)", "k", "The alias `list` is a keyword as written in PHP, so the generated code there writes it with `_` after it"),
        ("list", "list", "`list` is a keyword as written in PHP, so the generated code there writes it with `_` after it"),
    ] {
        let ws = said(&enumerated(decl, ty));
        assert_eq!(ws, [(title.to_string(), Vec::<&str>::new())], "{decl}: {ws:?}");
        let ns = notes(&enumerated(decl, ty));
        assert!(ns.iter().any(|n| n == "PHP: the enum's class is `List_`, the name a caller reads it by."), "{decl}: {ns:?}");
    }
}

/// A rule of one enum whose values are declared as `values` (`p(self) | q(q)`), read in rows `p` and
/// `q`.
fn valued(values: &str) -> String {
    enumerated("k(k)", "k").replacen("enum k(k) = p(p) | q(q)", &format!("enum k(k) = {values}"), 1)
}

/// The rule's alias and an enum's are held to the keywords as each language spells them, as the
/// names with no alias are (§15.265): an enum's type in PascalCase, so `float` is Java's `Float`,
/// which is no keyword, and `self` is `Self`, Rust's and Swift's; the rule's Go package in lower case
/// without its `_`, so `fall_through` is `fallthrough`. Ruby defines the rule's function on its
/// module, which a keyword can name. PHP writes the function and the namespace with `_` after one,
/// and Java the method, and the code builds: that is said apart, with no target (§15.344).
#[test]
fn the_rule_and_an_enum_are_held_to_the_keywords_as_each_language_spells_them() {
    rulec::i18n::set(rulec::i18n::Lang::En);
    let targets = |ws: &[(String, Vec<&'static str>)]| ws.iter().map(|(_, ts)| ts.clone()).collect::<Vec<_>>();
    // Java writes the type `Float`, which no keyword is; PHP writes `Float_`, and says so (§15.344).
    let ws = said(&enumerated("k(float)", "k"));
    assert_eq!(ws, [("The alias `float` is a keyword as written in PHP, so the generated code there writes it with `_` after it".to_string(), Vec::<&str>::new())], "{ws:?}");
    // PHP writes the class `Self_`, which stops nothing (§15.344).
    let ws = said(&enumerated("k(self)", "k"));
    assert_eq!(targets(&ws), [vec![], vec!["rust", "swift", "wasm"]], "{ws:?}");
    let ws = said(&rule("fall_through"));
    assert_eq!(targets(&ws), [vec!["go"]], "{ws:?}");
    let ws = said(&rule("for"));
    assert_eq!(ws.len(), 2, "{ws:?}");
    assert_eq!(ws[0], ("The alias `for` is a keyword as written in PHP and Java, so the generated code there writes it with `_` after it".to_string(), Vec::<&str>::new()), "{ws:?}");
    for t in ["python", "typescript", "javascript", "rust", "go", "swift", "wasm"] {
        assert!(ws[1].1.contains(&t), "for: {t} is not named: {ws:?}");
    }
    for t in ["ruby", "php", "java"] {
        assert!(!ws[1].1.contains(&t), "for: {t} is named: {ws:?}");
    }
    let ns = notes(&rule("for"));
    assert!(ns.iter().any(|n| n == "Java: the method of the rule's class `For` is `for_`, the name a caller calls it by."), "{ns:?}");
}

/// A value of an enum is written in each language's own case, which is a keyword only where Rust
/// writes its variant `Self` and Ruby its constant `BEGIN` or `END`: that is said once, by its alias
/// as by its name, and an alias that is a keyword only as written (`long`, `class`) stops nothing.
/// PHP writes the case `CLASS_`, which W121 says with no target (§15.344).
/// A table's alias is no identifier of its own anywhere, and no keyword is held against it.
#[test]
fn a_value_of_an_enum_and_a_table_are_held_to_the_keywords_as_they_are_written() {
    rulec::i18n::set(rulec::i18n::Lang::En);
    let targets = |ws: &[(String, Vec<&'static str>)]| ws.iter().map(|(_, ts)| ts.clone()).collect::<Vec<_>>();
    let ws = said(&valued("p(long) | q(class)"));
    assert_eq!(ws, [("The alias `class` is a keyword as written in PHP, so the generated code there writes it with `_` after it".to_string(), Vec::<&str>::new())], "{ws:?}");
    // PHP writes the case `SELF_`, which stops nothing (§15.344).
    let ws = said(&valued("p(self) | q(q)"));
    assert_eq!(targets(&ws), [vec![], vec!["rust", "wasm"]], "{ws:?}");
    let ws = said(&valued("p(begin) | q(end)"));
    assert_eq!(targets(&ws), [vec!["ruby"], vec!["ruby"]], "{ws:?}");
    let ws = said(&rule("t").replacen("table u(u)", "table u(for)", 1));
    assert!(ws.is_empty(), "{ws:?}");
}
