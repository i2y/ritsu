//! Naming (DESIGN 6.2): every line of `tests/fixtures/naming.tsv` — the table yuen and sakai
//! were held to, now this crate's — gives its JSON byte for byte, or is refused for the reason
//! the table gives. Until yuen and sakai name through this crate (PLAN C.7, C.8), their copies of
//! the table are held to be this one.

use ritsu_base::naming::{self, ErrorKind, Name, Tool, Word};
use std::path::Path;

/// What each reason of the table is, as an error of this crate.
fn reason(why: &str) -> ErrorKind {
    match why {
        "file has no kinds" => ErrorKind::NoKinds(Tool::File),
        "value only right after enum" => ErrorKind::ChildFirst { kind: "value".into(), parent: "enum" },
        "nothing under task" => ErrorKind::NothingUnder("task".into()),
        "only field under record" => ErrorKind::WrongChild { kind: "task".into(), parent: "record".into(), allowed: &["field"] },
        "method only right after service" => ErrorKind::ChildFirst { kind: "method".into(), parent: "service" },
        "one child at most" => ErrorKind::TooManyPairs("method".into()),
        "nothing under account" => ErrorKind::NothingUnder("account".into()),
        "operation only right after transfer" => ErrorKind::ChildFirst { kind: "operation".into(), parent: "transfer" },
        "unknown kind for koyomi" => ErrorKind::UnknownKind { tool: Tool::Koyomi, kind: "alias".into() },
        "unknown tool" => ErrorKind::UnknownTool("excel".into()),
        "absolute path" => ErrorKind::AbsolutePath("/etc/hosts".into()),
        "outside the root" => ErrorKind::OutsideRoot("../outside.txt".into()),
        "a full-width space outside a string" => ErrorKind::FullWidthSpace,
        "only \\\" and \\\\ are escapes" => ErrorKind::BadEscape("n".into()),
        "a kind written as a string" => ErrorKind::QuotedKind("date".into()),
        "a tool written as a string" => ErrorKind::QuotedTool("koyomi".into()),
        "a kind without a name" => ErrorKind::MissingName("output".into()),
        "an empty path" => ErrorKind::EmptyPath,
        "property only right after schema" => ErrorKind::ChildFirst { kind: "property".into(), parent: "schema" },
        "nothing under operation" => ErrorKind::NothingUnder("operation".into()),
        "unknown kind for openapi" => ErrorKind::UnknownKind { tool: Tool::Openapi, kind: "enum".into() },
        "only message under channel" => ErrorKind::WrongChild { kind: "operation".into(), parent: "channel".into(), allowed: &["message"] },
        "one child at most (value)" => ErrorKind::TooManyPairs("value".into()),
        "cedar has no nested kinds" => ErrorKind::NoNesting(Tool::Cedar),
        other => panic!("the reason `{other}` is new to this test; say which error it is"),
    }
}

fn table() -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/naming.tsv")).unwrap()
}

#[test]
fn every_line_of_the_table_gives_its_json_or_is_refused_for_its_reason() {
    let mut failures = Vec::new();
    let (mut ok, mut refused) = (0, 0);
    for (i, line) in table().lines().enumerate() {
        let (name, want) = line.split_once('\t').unwrap_or_else(|| panic!("line {} has no tab", i + 1));
        let got = naming::parse_one(name);
        if let Some(why) = want.strip_prefix("ERROR:") {
            let kind = reason(why.trim());
            match got {
                Ok(n) => failures.push(format!("line {}: `{name}` should be refused ({why}), and gave {}", i + 1, n.to_json())),
                Err(e) if e.kind == kind => refused += 1,
                Err(e) => failures.push(format!("line {}: `{name}` should be refused because{why}, and was refused because {:?}", i + 1, e.kind)),
            }
            continue;
        }
        match got {
            Ok(n) => {
                let json = n.to_json().compact();
                if json != want {
                    failures.push(format!("line {}: `{name}`\n  want {want}\n  got  {json}", i + 1));
                } else {
                    ok += 1;
                }
                assert_eq!(naming::parse_one(&n.text()).unwrap(), n, "line {}: the text reads back as the same naming", i + 1);
            }
            Err(e) => failures.push(format!("line {}: `{name}` was refused: {}", i + 1, e.text().en)),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert_eq!((ok, refused), (41, 25), "the table has 41 namings and 25 refusals");
}

#[test]
fn the_table_is_the_one_yuen_and_sakai_hold() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    for copy in ["yuen/tests/fixtures/naming.tsv", "sakai/tests/fixtures/naming.tsv"] {
        let p = root.join(copy);
        // A crate whose copy is gone names through this one already (PLAN C.7, C.8).
        if let Ok(theirs) = std::fs::read_to_string(&p) {
            assert_eq!(theirs, table(), "{copy} is not this crate's table");
        }
    }
    let t = table();
    for tool in Tool::ALL {
        assert!(t.lines().any(|l| l.starts_with(&format!("{} ", tool.word()))), "{} is in the table", tool.word());
    }
    assert!(!t.lines().any(|l| l.starts_with("dir ")), "dir is not a tool");
}

#[test]
fn where_an_error_is_and_what_it_says() {
    let e = naming::parse_one("koyomi \"a.cal\" alias x").unwrap_err();
    assert_eq!(e.at, 15, "the character of `alias`");
    assert_eq!(e.text().en, "`alias` is not a kind of koyomi; the kinds of koyomi are input, date, claim and source");
    assert_eq!(e.text().ja, "koyomi に `alias` という種類はありません。koyomi の種類は input、date、claim、source です");
    let e = naming::parse_one("excel \"a.xlsx\"").unwrap_err();
    assert!(e.text().en.starts_with("`excel` is not a tool; a naming starts with one of rulec, dandori, koyomi, chobo, geas, proto, openapi, asyncapi, cedar, file, yuen, sakai"), "{}", e.text().en);
    assert_eq!(naming::parse_one("").unwrap_err().kind, ErrorKind::Missing);
    assert_eq!(naming::parse_one("rulec").unwrap_err().kind, ErrorKind::MissingPath);
    assert_eq!(naming::parse_one("rulec x.rule").unwrap_err().kind, ErrorKind::UnquotedPath("x.rule".into()));
    assert_eq!(naming::parse_one("rulec \"x.rule\" table t input i").unwrap_err().kind, ErrorKind::NothingUnder("table".into()));
    assert_eq!(
        naming::parse_one("proto \"x.proto\" service S field f").unwrap_err().kind,
        ErrorKind::ChildFirst { kind: "field".into(), parent: "message" }
    );
    assert_eq!(
        naming::parse_one("proto \"x.proto\" service S enum E").unwrap_err().kind,
        ErrorKind::WrongChild { kind: "enum".into(), parent: "service".into(), allowed: &["method"] }
    );
    assert_eq!(naming::parse_one("file \"a\" # c").unwrap_err().kind, ErrorKind::Hash);
    assert_eq!(naming::parse_one("file \"a").unwrap_err().kind, ErrorKind::UnclosedString);
}

#[test]
fn a_path_is_from_the_directory_of_the_file_it_is_written_in() {
    assert_eq!(naming::parse("rulec \"../rules/送料.rule\"", "flows").unwrap().path, "rules/送料.rule");
    assert_eq!(naming::parse("file \".\"", "a/b").unwrap().path, "a/b");
    assert_eq!(naming::parse("file \"../..\"", "a/b").unwrap().path, ".");
    assert_eq!(naming::parse("file \"../../..\"", "a/b").unwrap_err().kind, ErrorKind::OutsideRoot("../../..".into()));
}

#[test]
fn a_scope_gathers_one_kind_after_its_pairs() {
    let ws = naming::words("rulec \"rules\" table").unwrap();
    let w = naming::written(ws, 19).unwrap();
    let (n, g) = naming::resolve(&w, "", true).unwrap();
    assert_eq!(n, Name::file(Tool::Rulec, "rules"));
    assert_eq!(g, Some(Word { text: "table".into(), at: 14, quoted: false }));
    let ws = naming::words("rulec \"rules\" bogus").unwrap();
    assert_eq!(naming::resolve(&naming::written(ws, 19).unwrap(), "", true).unwrap_err().kind, ErrorKind::UnknownKind { tool: Tool::Rulec, kind: "bogus".into() });
}

#[test]
fn what_contains_what() {
    let file = Name::file(Tool::Proto, "a.proto");
    let e = file.clone().with("enum", "E");
    let v = e.clone().with("value", "V");
    assert!(e.contains(&v) && file.contains(&v) && file.contains(&e));
    assert!(!v.contains(&e) && !e.contains(&e), "a naming does not contain itself");
    assert!(e.is_or_contains(&e) && e.is_or_contains(&v));
    assert!(!Name::file(Tool::Proto, "b.proto").is_or_contains(&v));
    assert_eq!(v.whole_file(), file);
    assert_eq!(v.kind(), Some("value"));
    assert_eq!(file.kind(), None);
    assert_eq!(Name::file(Tool::Geas, "a.geas").with("claim", "say \"hi\" #1").text(), "geas \"a.geas\" claim \"say \\\"hi\\\" #1\"");
    assert!(naming::is_word("40営業日以内") && naming::is_word("Order.Line"));
    assert!(!naming::is_word("rejects an empty name") && !naming::is_word("a#b") && !naming::is_word(""));
    assert_eq!(Tool::from_word("yuen"), Some(Tool::Yuen));
    assert_eq!(Tool::Yuen.extension(), Some("req"));
    assert_eq!(Tool::from_word("yurai"), None, "the tool is yuen now");
}

/// OpenAPI, AsyncAPI and Cedar (DESIGN 6.2, item 4): a document's elements by the keys the
/// specifications give them, a policy by its id, a schema's declarations by their names. sekisho
/// is no tool of a naming yet.
#[test]
fn the_standard_formats_and_their_kinds() {
    let e = naming::parse_one("openapi \"a.yaml\" enum Status").unwrap_err();
    assert_eq!(e.text().en, "`enum` is not a kind of openapi; the kinds of openapi are schema, operation and pointer");
    assert_eq!(e.text().ja, "openapi に `enum` という種類はありません。openapi の種類は schema、operation、pointer です");
    // a message is a channel's or the components'
    let m = naming::parse_one("asyncapi \"e.yaml\" message OrderPlaced").unwrap();
    let cm = naming::parse_one("asyncapi \"e.yaml\" channel orderPlaced message OrderPlaced").unwrap();
    assert_ne!(m, cm);
    assert!(naming::parse_one("asyncapi \"e.yaml\" channel orderPlaced").unwrap().contains(&cm));
    assert_eq!(
        naming::parse_one("openapi \"a.yaml\" pointer /components/responses/NotFound").unwrap().text(),
        "openapi \"a.yaml\" pointer /components/responses/NotFound",
        "a JSON Pointer is a word"
    );
    assert_eq!(naming::parse_one("cedar \"s.cedarschema\" entity User").unwrap().kind(), Some("entity"));
    for t in [Tool::Openapi, Tool::Asyncapi, Tool::Cedar] {
        assert_eq!(t.extension(), None, "{} files are not told by one extension", t.word());
        assert_eq!(Tool::from_word(t.word()), Some(t));
    }
    assert_eq!(Tool::Cedar.extensions(), &[".cedar", ".cedarschema", ".cedarschema.json"]);
    assert_eq!(Tool::Openapi.extensions(), Tool::Asyncapi.extensions());
    assert_eq!(Tool::from_word("sekisho"), None, "sekisho is not a tool of a naming yet");
    assert!(!Tool::ALL.contains(&Tool::Sekisho));
}
