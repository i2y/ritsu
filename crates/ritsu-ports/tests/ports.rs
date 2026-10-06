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

/// sekisho's port can be held as a trait object too, and the ports it reads with are one value.
#[allow(dead_code)]
fn held_gates(_: &dyn ritsu_ports::Gates, _: &ritsu_ports::GatePorts) {}

/// A language that answers only what every port of rules and of dates asks.
struct Bare;

impl Rules for Bare {
    fn facts(&self, rule: &std::path::Path) -> Result<ritsu_ports::RuleFacts, Vec<Said>> {
        Err(vec![Said::unreadable(&rule.display().to_string(), "bare")])
    }
    fn preconditions_hold(&self, _: &std::path::Path, _: &[(String, Option<i128>, Option<i128>)], _: Option<i128>) -> Result<Vec<(ritsu_ports::Precondition, ritsu_ports::Answer<ritsu_ports::Values>)>, Vec<Said>> {
        Ok(vec![])
    }
    fn output_values(&self, _: &std::path::Path, _: &str) -> Result<ritsu_ports::Found<ritsu_ports::OutputValues>, Vec<Said>> {
        Ok(ritsu_ports::Found::Undecided(ritsu_base::tr!("無い", "none")))
    }
    fn checked_over(&self, _: &std::path::Path, _: &str, _: &ritsu_ports::DaySet) -> Result<ritsu_ports::Answer<ritsu_base::text::Text>, Vec<Said>> {
        Ok(ritsu_ports::Answer::Holds)
    }
    fn eval(&self, _: &std::path::Path, _: &ritsu_ports::Values) -> Result<ritsu_ports::Values, ritsu_ports::RuleError> {
        Ok(vec![])
    }
    fn doc(&self, _: &std::path::Path, _: &str, _: bool, _: ritsu_base::text::Lang) -> Result<String, Vec<Said>> {
        Ok(String::new())
    }
}

impl Dates for Bare {
    fn facts(&self, file: &std::path::Path) -> Result<ritsu_ports::DateFacts, Vec<Said>> {
        Err(vec![Said::unreadable(&file.display().to_string(), "bare")])
    }
    fn values(&self, _: &std::path::Path, _: &str) -> Result<ritsu_ports::Found<ritsu_ports::DaySet>, Vec<Said>> {
        Ok(ritsu_ports::Found::Value(Default::default()))
    }
    fn days(&self, _: &std::path::Path, _: &str) -> Result<ritsu_ports::Found<(i64, i64)>, Vec<Said>> {
        Ok(ritsu_ports::Found::Value((0, 0)))
    }
    fn eval(&self, _: &std::path::Path, _: &[(String, i64)]) -> Result<Vec<(String, ritsu_ports::DateValue)>, Vec<Said>> {
        Ok(vec![])
    }
}

/// The questions added for sekisho have answers by default, so a language that implemented the
/// ports before them still does: the values of an output over ranges are undecided, and no
/// calendar or page is given — each saying so in both languages, of the file asked about.
#[test]
fn the_questions_added_for_sekisho_answer_by_default() {
    let p = std::path::Path::new("a.cal");
    match Bare.outputs_over(std::path::Path::new("a.rule"), "band", &[("amount".into(), Some(1), Some(50))]).unwrap() {
        ritsu_ports::Found::Undecided(t) => assert!(!t.ja.is_empty() && !t.en.is_empty() && t.ja != t.en, "{t:?}"),
        other => panic!("{other:?}"),
    }
    for said in [Bare.calendar(p).unwrap_err(), Dates::doc(&Bare, p, "a.cal", true, ritsu_base::text::Lang::Ja).unwrap_err()] {
        assert_eq!((said.len(), said[0].file.as_str(), said[0].code.as_str(), said[0].line), (1, "a.cal", "", None));
        assert!(!said[0].message.ja.is_empty() && said[0].message.ja != said[0].message.en, "{:?}", said[0].message);
    }
}
