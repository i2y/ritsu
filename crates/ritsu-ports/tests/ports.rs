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
