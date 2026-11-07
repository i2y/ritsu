//! `fed by <naming>` (DESIGN 7.16), as every language reads it: where the two words stand in a line,
//! the text of a file with them taken off, what a naming can name to be a source of values, and what
//! a page says.

use ritsu_base::fed::{self, Problem};
use ritsu_base::naming::{ErrorKind, Tool};

/// What is cut from a line: the text before the form, and the naming.
fn cut(line: &str) -> Option<(&str, &str)> {
    fed::cut(line).map(|c| (&line[..c.at], c.text(line)))
}

#[test]
fn where_the_two_words_stand_in_a_line() {
    let line = "  food : number  range >=0 <=60  fed by rulec \"income.rule\" output food";
    assert_eq!(cut(line), Some(("  food : number  range >=0 <=60  ", "rulec \"income.rule\" output food")));
    // blanks between the words and a tab, and a comment after the naming
    assert_eq!(cut("  a : int\tfed  \t by rulec \"x.rule\" output y   # why"), Some(("  a : int\t", "rulec \"x.rule\" output y")));
    // nothing after the words: the naming is empty, for the language to say so
    assert_eq!(cut("  a : int  fed by"), Some(("  a : int  ", "")));
    assert_eq!(cut("  a : int  fed by  # nothing"), Some(("  a : int  ", "")));
    // a `#` in the naming's string is the string's
    assert_eq!(cut("  a : int  fed by rulec \"a#b.rule\" output y # why"), Some(("  a : int  ", "rulec \"a#b.rule\" output y")));
    // the words can be in any language's name for the rest of the line
    assert_eq!(cut("  量(amount) : int  fed by jsonschema \"注文.json\" def 注文 property 金額"), Some(("  量(amount) : int  ", "jsonschema \"注文.json\" def 注文 property 金額")));
}

#[test]
fn what_is_not_the_form() {
    for line in [
        // a name spelled like a word: both words are needed, each a word of its own
        "  fed : number  range >=0 <=9",
        "  by : number  range >=0 <=9",
        "  a : fed",
        "  a : int  fedby rulec \"x.rule\" output y",
        "  a : int  fed byte",
        "  a : int  fed\tbyte rulec",
        "  a : int  refed by rulec \"x.rule\" output y",
        "  a : int  fed, by rulec",
        // at the start of a line nothing has been declared to feed
        "fed by rulec \"x.rule\" output y",
        // in a string or a comment
        "description \"this is fed by rulec\"",
        "description \"a \\\"quoted\\\" fed by rulec\"",
        "  a : int  # fed by rulec \"x.rule\" output y",
        "# fed by rulec \"x.rule\" output y",
    ] {
        assert_eq!(fed::cut(line), None, "{line}");
    }
}

#[test]
fn a_file_without_the_form_is_the_bytes_of_one_that_never_had_it() {
    let with = "rule r v1\n\ninputs\n  a : int  range >=0 <=9  fed by rulec \"x.rule\" output y\n  b : int  range >=0 <=9\tfed by rulec \"x.rule\" output z   # why\n  c : int  fed by\n";
    let without = "rule r v1\n\ninputs\n  a : int  range >=0 <=9\n  b : int  range >=0 <=9 # why\n  c : int\n";
    assert_eq!(fed::without(with), without);
    assert_eq!(fed::without(without), without, "nothing to take off");
    assert!(matches!(fed::without(without), std::borrow::Cow::Borrowed(_)), "the text as it is, when there is nothing to take off");
    assert_eq!(fed::without("a : int  fed by rulec \"x.rule\" output y"), "a : int", "a last line with no newline keeps none");
    // the comments and strings that mention it stay
    let kept = "description \"fed by nothing\"\n# fed by nothing\n";
    assert_eq!(fed::without(kept), kept);
    assert_eq!(fed::hash(with), fed::hash(without), "what a file's meaning is named by does not read the form");
    assert_eq!(fed::hash(without), ritsu_base::sha256::hex(without.as_bytes()));
    assert_ne!(fed::hash(with), ritsu_base::sha256::hex(with.as_bytes()));
}

#[test]
fn what_a_naming_can_name() {
    let holds = [
        "rulec \"x.rule\" output y",
        "chobo \"x.book\" account a",
        "koyomi \"x.cal\" date d",
        "proto \"x.proto\" message M field f",
        "openapi \"x.yaml\" schema S property p",
        "jsonschema \"x.json\" def D",
        "jsonschema \"x.json\" def D property p",
        "jsonschema \"x.json\" pointer /$defs/D/properties/p",
        "dandori \"x.flow\" output o",
        "dandori \"x.flow\" record R field f",
        "kaname \"x.kn\" record R field f",
        // a place that took the value first
        "rulec \"x.rule\" input a",
        "koyomi \"x.cal\" input i",
        "dandori \"x.flow\" input i",
        "sekisho \"x.gate\" principal P attribute a",
        "sekisho \"x.gate\" resource R attribute a",
        "sekisho \"x.gate\" action A input i",
        // a path out of the directory, and a dependency's, are for the language to join
        "rulec \"../x.rule\" output y",
        "rulec \"@pricing/rules/x.rule\" output y",
        "rulec \"x.rule\" output 送料",
    ];
    for text in holds {
        let n = fed::read(text).unwrap_or_else(|e| panic!("{text}: {}", e.text().en));
        assert!(fed::holds_a_value(&n), "{text}");
        assert_eq!(n.text(), text, "the path is as written until a language joins it");
    }
    // what names no value: each says what can be named
    for (text, says) in [
        ("rulec \"x.rule\"", "Of rulec, what can be named is `rulec \"…\" output …`, `rulec \"…\" input …`."),
        ("rulec \"x.rule\" table t", "Of rulec, what can be named is"),
        ("chobo \"x.book\" transfer t", "Of chobo, what can be named is `chobo \"…\" account …`."),
        ("koyomi \"x.cal\" claim c", "Of koyomi, what can be named is `koyomi \"…\" date …`, `koyomi \"…\" input …`."),
        ("proto \"x.proto\" message M", "Of proto, what can be named is"),
        ("openapi \"x.yaml\" schema S", "Of openapi, what can be named is"),
        ("openapi \"x.yaml\" operation o", "Of openapi, what can be named is"),
        ("jsonschema \"x.json\"", "Of jsonschema, what can be named is"),
        ("sakai \"x.ctx\" context C", "Nothing of sakai can be named"),
        ("geas \"x.geas\" claim c", "Nothing of geas can be named"),
        ("yuen \"x.req\" requirement r", "Nothing of yuen can be named"),
        ("sekisho \"x.gate\" action a", "Of sekisho, what can be named is `sekisho \"…\" principal … attribute …`, `sekisho \"…\" resource … attribute …`, `sekisho \"…\" action … input …`."),
        ("cedar \"x.cedar\" policy p", "Nothing of cedar can be named"),
        ("asyncapi \"x.yaml\" schema S property p", "Nothing of asyncapi can be named"),
        ("file \"x.txt\"", "Nothing of file can be named"),
    ] {
        let e = fed::read(text).unwrap_err();
        assert!(matches!(e, Problem::NoValue(_)), "{text}: {e:?}");
        assert!(e.text().en.contains(says), "{text}: {}", e.text().en);
        assert!(!e.text().ja.is_empty());
        assert_eq!(e.at(), 0);
    }
    // what does not read as a naming: why, from the one reader of namings
    for (text, kind) in [
        ("", ErrorKind::Missing),
        ("rulec x.rule output y", ErrorKind::UnquotedPath("x.rule".into())),
        ("excel \"x.xlsx\"", ErrorKind::UnknownTool("excel".into())),
        ("rulec \"\" output y", ErrorKind::EmptyPath),
        ("rulec \"/etc/x.rule\" output y", ErrorKind::AbsolutePath("/etc/x.rule".into())),
        ("rulec \"x.rule\" output", ErrorKind::MissingName("output".into())),
    ] {
        match fed::read(text) {
            Err(Problem::Naming(e)) => assert_eq!(e.kind, kind, "{text}"),
            other => panic!("{text}: {other:?}"),
        }
    }
    // every tool has its answer, and the ones with shapes hold them under the kinds naming knows
    for t in Tool::ALL {
        for shape in fed::shapes(t) {
            let (first, rest) = shape.split_first().unwrap();
            let kinds = t.kinds();
            let (_, children) = kinds.iter().find(|(k, _)| k == first).unwrap_or_else(|| panic!("{}: {first}", t.word()));
            assert!(rest.iter().all(|k| children.contains(k)), "{}: {shape:?}", t.word());
        }
    }
}

#[test]
fn from_the_directory_of_the_file() {
    let root = std::path::Path::new(".");
    let n = fed::read("rulec \"../income.rule\" output food").unwrap();
    assert_eq!(fed::from_root(root, "rules/deep/r.rule", &n).unwrap().path, "rules/income.rule");
    assert_eq!(fed::from_root(root, "r.rule", &fed::read("rulec \"income.rule\" output food").unwrap()).unwrap().path, "income.rule");
    assert!(fed::from_root(root, "r.rule", &n).is_err(), "out of the root");
}

/// Every language's page says the same: where it comes from, what is checked and what is not.
#[test]
fn what_a_page_says() {
    let t = fed::page_note("rulec \"x.rule\" output y");
    assert_eq!(t.en, "Comes from `rulec \"x.rule\" output y` (`ritsu check` holds the range of its values; the hand-written code that carries the value is not checked)");
    assert_eq!(t.ja, "出どころ: `rulec \"x.rule\" output y`（値の範囲は `ritsu check` が確かめます。値を運ぶ手書きのコードは確かめません）");
}
