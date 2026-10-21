//! E121 for a `from` path that goes on through a collection of the contract (§15.254).
//!
//! A path reads one value, and a collection holds any number of elements, each with its own: the
//! path does not say which element's value the input takes. E121 used to say only that the path
//! resolved as far as the collection, "which has no fields under it", though every element had
//! the field the path named. It now says that the path goes on through a collection, names every
//! collection along it, and says what can be written instead: the ways a rule reads a collection —
//! `any`, `all`, `count`, `sum`, `max` and `min`, or the elements walked as a sequence — where the
//! rest of the path reads a value in one element, and a contract that holds one element as one
//! object.
//!
//! The materials are in `tests/collection_path`, English first, with a Japanese version beside
//! each: a JSON Schema and a `.proto` of an order and its lines, mutants that each go on through a
//! collection in one way, and a rule that reads the lines the ways the message names, and passes.
//! What `rulec check` prints for a mutant, in English and in Japanese, is its golden file in
//! `tests/golden/collection_path` (`RULEC_BLESS=1 cargo test --test collection_path` writes them
//! again).

use rulec::i18n::{self, Lang};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The mutants: the English rule, its Japanese version, and what the English report says.
const MUTANTS: &[(&str, &str, &str)] = &[
    // A field of each line, which a quantifier or a sequence reads
    ("E121_a_field_of_each_line", "E121_明細ごとの区分", "`from any order.lines where category = <value>` is true when some element has that value"),
    // A number of each line, which `sum`, `max` and `min` read too
    ("E121_a_number_of_each_line", "E121_明細ごとの数量", "`from sum order.lines of quantity`"),
    // Two steps in each line: a sequence reads them, a `where` does not
    ("E121_a_path_in_each_line", "E121_明細の中のパス", "write `elements <name> from order.lines` with a field `from unit.kind`"),
    // A collection inside each line too
    ("E121_a_collection_in_each_line", "E121_明細の中の並び", "Inside the elements, `order.lines[].parts` is a collection too."),
    // A field no line has
    ("E121_a_field_no_line_has", "E121_明細に無いフィールド", "In an element it resolved as far as `order.lines[]`. The fields there: category, parts, quantity, unit"),
    // A `repeated` field of a `.proto`
    ("E121_a_repeated_field", "E121_repeated_のフィールド", "make `lines` a field that is not `repeated` in the contract"),
    // The collection a `count` walks, through another
    ("E121_a_count_through_a_collection", "E121_並びを通る件数", "`count` walks one collection, and the path to it does not go on through another."),
    // The collection a sequence is taken from, through another
    ("E121_a_sequence_through_a_collection", "E121_並びを通る並び", "`elements` walks one collection, and the path to it does not go on through another."),
    // A field of an element, through a collection inside the element
    ("E121_a_collection_in_an_element", "E121_要素の中の並び", "the path does not say which one the field takes"),
];

/// The rule that reads the lines the ways E121 names, and its Japanese version.
const PASSING: [(&str, &str); 1] = [("order_lines_read", "明細を読む規則")];

/// The contracts beside them.
const CONTRACTS: [&str; 2] = ["orders.proto", "orders.schema.json"];

fn source(stem: &str) -> (String, String) {
    let rel = format!("tests/collection_path/{stem}.rule");
    let src = std::fs::read_to_string(root().join(&rel)).unwrap_or_else(|_| panic!("cannot read {rel}"));
    (rel, src)
}

/// What `rulec check <rel>` prints, in `lang`. A test runs in the crate's directory, where `rel`
/// leads to the rule, and the contract is found beside it.
fn printed(rel: &str, src: &str, lang: Lang) -> String {
    let lines: Vec<String> = src.lines().map(String::from).collect();
    i18n::with(lang, || {
        let r = rulec::report(src, rel);
        format!("{}{}", rulec::findings_text(&r.diags, &lines), rulec::check_tail(&r.shadow, &r.diags, rel, 0))
    })
}

/// The findings of a rule's text, as (code, line where it is reported).
fn found(src: &str, rel: &str) -> Vec<(String, usize)> {
    i18n::with(Lang::En, || {
        rulec::check_source(src, rel)
            .iter()
            .map(|d| (d.code.to_string(), d.where_.strip_prefix(&format!("{rel}:")).and_then(|t| t.split(' ').next()).and_then(|n| n.parse().ok()).unwrap_or(0)))
            .collect()
    })
}

/// Each mutant stops at its one E121, says what the case asks for, and prints what its golden
/// file holds in both languages. The Japanese version says the same things in the same places.
#[test]
fn each_mutant_says_the_path_goes_on_through_a_collection() {
    let mut names: Vec<String> = std::fs::read_dir(root().join("tests/collection_path")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    let mut listed: Vec<String> = MUTANTS
        .iter()
        .flat_map(|(en, ja, _)| [format!("{en}.rule"), format!("{ja}.rule")])
        .chain(PASSING.iter().flat_map(|(en, ja)| [format!("{en}.rule"), format!("{ja}.rule")]))
        .chain(CONTRACTS.iter().map(|c| c.to_string()))
        .collect();
    listed.sort();
    assert_eq!(names, listed, "every file of tests/collection_path is listed here, a rule with its Japanese version");
    let mut failures = Vec::new();
    for (en, ja, said) in MUTANTS {
        let (re, se) = source(en);
        let (rj, sj) = source(ja);
        let codes: Vec<String> = found(&se, &re).into_iter().map(|(c, _)| c).collect();
        if codes != ["E121"] {
            failures.push(format!("{re}: {codes:?}, not one E121"));
        }
        if found(&se, &re) != found(&sj, &rj) {
            failures.push(format!("{re} and {rj} do not say the same things in the same places"));
        }
        let out = printed(&re, &se, Lang::En);
        if !out.contains("goes on through the collection") || !out.contains(said) {
            failures.push(format!("{re} does not say `{said}`:\n{out}"));
        }
        for stem in [en, ja] {
            let (rel, src) = source(stem);
            for (lang, tag) in [(Lang::En, "en"), (Lang::Ja, "ja")] {
                let out = printed(&rel, &src, lang);
                if let Err(e) = ritsu_testkit::golden::check(&root().join(format!("tests/golden/collection_path/{stem}.{tag}.txt")), &out) {
                    failures.push(e);
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The ways the message names read the lines: a rule that asks of them, counts them, adds up and
/// compares a number of theirs, and walks them with a field two steps into each, passes.
#[test]
fn the_ways_the_message_names_read_the_collection() {
    for (en, ja) in PASSING {
        for stem in [en, ja] {
            let (rel, src) = source(stem);
            let ds = i18n::with(Lang::En, || rulec::check_source(&src, &rel));
            assert!(ds.is_empty(), "{rel}: {:?}", ds.iter().map(|d| (d.code, d.title.clone())).collect::<Vec<_>>());
        }
    }
}
