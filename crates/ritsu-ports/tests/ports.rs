//! The ports are traits a receiving language holds as `&dyn` (DESIGN 3.2): the language that knows
//! implements them, and the one that asks never names its crate. Every implementation is tested
//! in the crate that gives it (rulec, koyomi, chobo, geas, yuen, sakai, `tests/ports.rs`).

use ritsu_base::naming::{Name, Tool};
use ritsu_ports::{Books, Claims, Dates, Item, Items, Ledger, References, Rules, Said};

/// Each port can be held as a trait object: a receiving language takes `&dyn Rules`, a runner a
/// `Box<dyn Ledger>`.
#[allow(dead_code)]
fn held(_: &dyn Rules, _: &dyn Dates, _: &dyn Books, _: &dyn Claims, _: &dyn Items, _: &dyn References, _: Box<dyn Ledger>) {}

#[test]
fn an_item_is_named_by_its_last_pair() {
    let it = Item { naming: Name::file(Tool::Rulec, "rules/a.rule").with("enum", "区分").with("value", "一般"), lines: (3, 3), text: "一般(basic)".into() };
    assert_eq!((it.kind(), it.name()), ("value", "一般"));
    let file = Item { naming: Name::file(Tool::File, "a.txt"), lines: (1, 1), text: String::new() };
    assert_eq!((file.kind(), file.name()), ("", ""));
}

#[test]
fn what_a_language_says_keeps_both_languages() {
    let s = Said::unreadable("rules/a.rule", "No such file");
    assert_eq!(s.code, "");
    assert!(s.message.ja.contains("rules/a.rule") && s.message.en.starts_with("cannot read"), "{:?}", s.message);
    let d: ritsu_base::diag::Diag = ritsu_base::diag::Diag::error("E101", "a.cal", 3, 5, ritsu_base::tr!("穴があります", "there is a hole"));
    let s = Said::of(&d);
    assert_eq!((s.code.as_str(), s.file.as_str(), s.line), ("E101", "a.cal", Some(3)));
    assert_eq!(s.message.en, "there is a hole");
}

/// A language that holds one table and one enum with a value in every file, and counts how often
/// it is asked.
struct Two {
    asked: std::cell::Cell<usize>,
}

impl Items for Two {
    fn items(&self, _root: &std::path::Path, file: &str) -> Result<Vec<Item>, Vec<Said>> {
        self.asked.set(self.asked.get() + 1);
        if file == "bad.rule" {
            return Err(vec![Said::unreadable(file, "no")]);
        }
        let n = Name::file(Tool::Rulec, file);
        Ok(vec![
            Item { naming: n.clone().with("table", "送料表"), lines: (3, 6), text: "| a |".into() },
            Item { naming: n.clone().with("enum", "区分"), lines: (1, 1), text: "enum 区分 = 一般".into() },
            Item { naming: n.with("enum", "区分").with("value", "一般"), lines: (1, 1), text: "一般".into() },
        ])
    }
}

/// The index asks a file's language once in a run, finds a thing by its naming, and says what
/// is beside a thing that is not there (ritsu's DESIGN 6.4).
#[test]
fn the_index_finds_a_naming_and_asks_each_file_once() {
    use ritsu_ports::{Index, Lookup};
    let two = std::rc::Rc::new(Two { asked: std::cell::Cell::new(0) });
    let index = Index::new().with_items(Tool::Rulec, two.clone());
    let root = std::path::Path::new("/project");
    let file = Name::file(Tool::Rulec, "a.rule");
    assert!(matches!(index.find(root, &file), Lookup::Found(None)));
    match index.find(root, &file.clone().with("table", "送料表")) {
        Lookup::Found(Some(i)) => assert_eq!(i.lines, (3, 6)),
        other => panic!("{other:?}"),
    }
    match index.find(root, &file.clone().with("table", "手数料表")) {
        Lookup::Missing(same) => assert_eq!(same.iter().map(|i| i.name()).collect::<Vec<_>>(), ["送料表"]),
        other => panic!("{other:?}"),
    }
    // a value is looked for under its own enum only
    match index.find(root, &file.clone().with("enum", "区分").with("value", "特別")) {
        Lookup::Missing(same) => assert_eq!(same.iter().map(|i| i.name()).collect::<Vec<_>>(), ["一般"]),
        other => panic!("{other:?}"),
    }
    assert_eq!(two.asked.get(), 1, "one file, asked once");
    assert!(matches!(index.find(root, &Name::file(Tool::Rulec, "bad.rule")), Lookup::Refused(_)));
    assert!(matches!(index.find(root, &Name::file(Tool::Koyomi, "a.cal")), Lookup::NotJoined));
    assert!(index.reads_items(Tool::Rulec) && !index.reads_items(Tool::Koyomi) && !index.reads_references(Tool::Rulec));
}
