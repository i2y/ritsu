//! E043 for an input of the callee held to a range of its ordered enum (§15.252).
//!
//! `campaign : tier  range >=basic <=gold` lets platinum in nowhere: not at the callee's door, not
//! in the tables its check proves complete. An `apply` that bound a value that can be platinum to it
//! used to pass when a row of the callee took platinum too (`>=silver`), and the expanded rule then
//! answered platinum with that row, which the callee itself refuses; and to stop at E101 in a table
//! of the callee's, which this rule cannot add a row to, when no row did. E043 held a number to the
//! callee's range and read an enum's range not at all.
//!
//! The materials are in `tests/apply_enum`, English first, with a Japanese version beside each: two
//! callees (`campaign_credit.rule`, whose `>=silver` row takes platinum too, and
//! `campaign_points.rule`, with one row for each tier inside the range); `order_credits.rule`,
//! which passes a tier inside the range in each way a binding can — an input held to the same range,
//! a value, a table's output, a level read through `with`, a definition that names an input — and
//! passes; and mutants that each pass one value the range does not let in, every one of which passed
//! check or stopped at E101 before. What `rulec check` prints for a mutant, in English and in
//! Japanese, is its golden file in `tests/golden/apply_enum` (`RULEC_BLESS=1 cargo test --test
//! apply_enum` writes them again).
//!
//! The table's output in `order_credits.rule` writes every tier inside the range and goes to
//! `campaign_points.rule`, whose rows take only those. Where a table above writes some of an enum's
//! values and a row below takes others too, the row's box in the certificate leaves the others out,
//! and both rechecks read it so (§15.253, `tests/above_never.rs`); every rule of the tree is held
//! to both rechecks (`tests/cert.rs`, `tests/lean.rs`).

use ritsu_testkit::TempDir;
use rulec::i18n::{self, Lang};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The mutants: the English rule, its Japanese version, and what each English report says — the
/// value outside the range, as the example, or that what is passed is not known. Each passed check
/// before, but the second, which stopped at E101.
const MUTANTS: &[(&str, &str, &str, &str)] = &[
    // An input with no range, to the callee whose `>=silver` row takes platinum too
    ("E043_a_tier_with_no_range", "E043_範囲の無い段階", "For example campaign = platinum is outside range >=basic <=gold", "For example キャンペーン = プラチナ is outside range >=一般 <=ゴールド"),
    // The same, to the callee with no row for platinum: the check stopped at E101 before
    ("E043_no_row_past_the_range", "E043_範囲の外に当たる行が無い", "For example campaign = platinum is outside range >=basic <=gold of `campaign` in `campaign_points.rule`", "For example キャンペーン = プラチナ is outside range >=一般 <=ゴールド of `キャンペーン` in `キャンペーンのポイント.rule`"),
    // A value outside the range
    ("E043_a_literal_past_the_range", "E043_範囲の外のリテラル", "what is passed: platinum", "what is passed: プラチナ"),
    // The output of a table that writes platinum
    ("E043_a_table_that_writes_platinum", "E043_プラチナを書く表", "what is passed: one of silver, gold, platinum", "what is passed: one of シルバー, ゴールド, プラチナ"),
    // The later of two tiers inside the range: an expression, whose values check does not work out
    ("E043_the_later_of_two_tiers", "E043_二つの段階の後のほう", "The values of what is passed to `campaign` are not known", "The values of what is passed to `キャンペーン` are not known"),
    // A level read through `with` as platinum
    ("E043_with_reads_a_level_as_platinum", "E043_with_でプラチナに読み替える", "For example campaign = platinum is outside range >=basic <=gold", "For example キャンペーン = プラチナ is outside range >=一般 <=ゴールド"),
];

/// The rules that pass, each with its Japanese version: the two callees, and the rule that passes a
/// tier inside the range in every way a binding can.
const PASSING: [(&str, &str); 3] = [("campaign_credit", "キャンペーンの値引き"), ("campaign_points", "キャンペーンのポイント"), ("order_credits", "注文ごとの値引き")];

fn source(stem: &str) -> (String, String) {
    let rel = format!("tests/apply_enum/{stem}.rule");
    let src = std::fs::read_to_string(root().join(&rel)).unwrap_or_else(|_| panic!("cannot read {rel}"));
    (rel, src)
}

/// What `rulec check <rel>` prints, in `lang`.
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

fn errors(src: &str, rel: &str) -> Vec<&'static str> {
    i18n::with(Lang::En, || rulec::check_source(src, rel).iter().filter(|d| d.severity == rulec::diag::Severity::Error).map(|d| d.code).collect())
}

/// Each mutant stops at its one E043, and prints what its golden file holds in both languages.
#[test]
fn each_mutant_says_e043_of_the_value_outside_the_range() {
    let mut names: Vec<String> = std::fs::read_dir(root().join("tests/apply_enum")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    let mut listed: Vec<String> = MUTANTS
        .iter()
        .flat_map(|(en, ja, ..)| [format!("{en}.rule"), format!("{ja}.rule")])
        .chain(PASSING.iter().flat_map(|(en, ja)| [format!("{en}.rule"), format!("{ja}.rule")]))
        .collect();
    listed.sort();
    assert_eq!(names, listed, "every rule of tests/apply_enum is listed here, with its Japanese version");
    let mut failures = Vec::new();
    for (en, ja, said_en, said_ja) in MUTANTS {
        for (stem, said) in [(en, said_en), (ja, said_ja)] {
            let (rel, src) = source(stem);
            let errs = errors(&src, &rel);
            if errs != ["E043"] {
                failures.push(format!("{rel}: {errs:?}, not one E043"));
                continue;
            }
            let out = printed(&rel, &src, Lang::En);
            if !out.contains(said) {
                failures.push(format!("{rel} does not say `{said}`:\n{out}"));
            }
            for (lang, tag) in [(Lang::En, "en"), (Lang::Ja, "ja")] {
                let out = printed(&rel, &src, lang);
                if let Err(e) = ritsu_testkit::golden::check(&root().join(format!("tests/golden/apply_enum/{stem}.{tag}.txt")), &out) {
                    failures.push(e);
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The Japanese version is the same rule with other names: the same findings on the same lines.
#[test]
fn the_japanese_version_says_the_same_things_in_the_same_places() {
    for (en, ja) in MUTANTS.iter().map(|(en, ja, ..)| (*en, *ja)).chain(PASSING) {
        let (re, se) = source(en);
        let (rj, sj) = source(ja);
        assert_eq!(found(&se, &re), found(&sj, &rj), "{en} and {ja}");
    }
}

/// A binding that can pass only values inside the range passes, in each way it can be written: an
/// input held to the same range, a value, the output of a table whose rows write values inside it,
/// a level every value of which `with` reads as one inside it, and a definition that only names an
/// input held to it. The callees pass on their own, and the examples hold.
#[test]
fn values_inside_the_range_pass() {
    for (en, ja) in PASSING {
        for stem in [en, ja] {
            let (rel, src) = source(stem);
            let ds = i18n::with(Lang::En, || rulec::check_source(&src, &rel));
            assert!(ds.is_empty(), "{rel}: {:?}", ds.iter().map(|d| (d.code, d.title.clone())).collect::<Vec<_>>());
        }
    }
}

/// Nothing is made from a mutant: the generated code and ritsu's port read the rule `prepare` gives,
/// and E043 leaves none. Before, the expanded rule was prepared, and answered platinum.
#[test]
fn a_mutant_is_prepared_for_nothing() {
    for (en, ja, ..) in MUTANTS {
        for stem in [en, ja] {
            let (rel, src) = source(stem);
            let codes: Vec<&str> = match i18n::with(Lang::En, || rulec::prepare(&src, &rel)) {
                Ok(_) => panic!("{rel} was prepared"),
                Err(ds) => ds.iter().map(|d| d.code).collect(),
            };
            assert!(codes.contains(&"E043"), "{rel}: {codes:?}");
        }
    }
}

/// The output of another `apply` is a definition that names what the callee's table writes, and is
/// read as what that table writes: one that lifts a tier as far as gold passes, and one that lifts
/// platinum to platinum is E043 with platinum as the example.
#[test]
fn an_output_another_apply_brought_in_is_what_its_rows_write() {
    let lift = |top: &str| {
        format!(
            "rule lift_tier v1\ndescription \"The tier a member's own tier lifts a campaign to\"\n\nenum tier = basic < silver < gold < platinum\n\ninputs\n  member : tier\n\noutputs\n  lift : tier\n\ntable lift_by_member\npolicy unique\n| member   | -> lift  |\n| basic    | silver   |\n| silver   | gold     |\n| gold     | gold     |\n| platinum | {top} |\n"
        )
    };
    let callee = std::fs::read_to_string(root().join("tests/apply_enum/campaign_credit.rule")).unwrap();
    for (top, want) in [("gold    ", Vec::<&str>::new()), ("platinum", vec!["E043"])] {
        let tmp = TempDir::new("apply-enum-chain");
        std::fs::write(tmp.path().join("campaign_credit.rule"), &callee).unwrap();
        std::fs::write(tmp.path().join("lift_tier.rule"), lift(top)).unwrap();
        let meaning = |file: &str| rulec::link::pin::meaning(&std::fs::read_to_string(tmp.path().join(file)).unwrap());
        let src = format!(
            "rule chained_credit v1\ndescription \"The credit at the tier a member's own tier lifts a campaign to\"\n\nenum tier = basic < silver < gold < platinum\n\ninputs\n  member : tier\n\noutputs\n  credit : money[USD]  round down(1USD)\n\napply first = \"lift_tier.rule\" meaning:{}\n  member = member\n\napply campaign = \"campaign_credit.rule\" meaning:{}\n  campaign = lift\n",
            meaning("lift_tier.rule"),
            meaning("campaign_credit.rule")
        );
        let rel = tmp.path().join("chained_credit.rule").to_string_lossy().into_owned();
        let ds = i18n::with(Lang::En, || rulec::check_source(&src, &rel));
        let codes: Vec<&str> = ds.iter().map(|d| d.code).collect();
        assert_eq!(codes, want, "{:?}", ds.iter().map(|d| (d.code, d.title.clone(), d.notes.clone())).collect::<Vec<_>>());
        if !want.is_empty() {
            assert!(ds[0].notes[0].starts_with("For example campaign = platinum is outside range >=basic <=gold"), "{:?}", ds[0].notes);
        }
    }
}
