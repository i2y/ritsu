//! What the completeness check (E101) no longer asks for (DESIGN §15.195): a value of a `define`
//! past what its expression comes to, and a value of a derive that a table above never lets come
//! with the value it writes into another column. Each rule of `tests/sieve` is written in English,
//! with a Japanese version beside it. Before §15.195 both asked for a row at a value no input
//! reaches; now they pass, and every input inside the ranges, run through the reference evaluator,
//! shows the value is never reached. The certificate says why with a leaf of its own, which the two
//! re-checkers hold it to (`tests/cert.rs`, `tests/lean.rs`).
//!
//! Where a gap does lie inside what the column comes to, E101 still names it, with a value the
//! column takes and an input that makes it.

use rulec::eval::Val;
use rulec::i18n::{self, Lang};
use rulec::num::Rat;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The rules: the English one, its Japanese version, and the leaf the certificate rests on.
const PAIRS: &[(&str, &str, &str)] = &[
    ("E101_past_a_defines_reach", "E101_定義の届かない値", r#"{"define_axis":"#),
    ("E101_past_what_a_table_above_allows", "E101_上の表が許さない値", r#"{"above_rows":"#),
];

fn source(stem: &str) -> (String, String) {
    let rel = format!("tests/sieve/{stem}.rule");
    let src = std::fs::read_to_string(root().join(&rel)).unwrap_or_else(|_| panic!("cannot read {rel}"));
    (rel, src)
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

/// Every input inside the declared ranges (whole units; the `constraint` lines kept), as the
/// reference evaluator takes it.
fn every_input(f: &rulec::ast::RuleFile, c: &rulec::types::Checked) -> Vec<BTreeMap<String, Val>> {
    let axes: Vec<(String, Vec<Val>)> = f
        .inputs
        .iter()
        .map(|i| {
            let (lo, hi) = c.ranges[&i.name.text];
            let (lo, hi) = (lo.unwrap().num, hi.unwrap().num);
            (i.name.text.clone(), (lo..=hi).map(|v| Val::Num(Rat::int(v))).collect())
        })
        .collect();
    let mut out = Vec::new();
    let mut at = vec![0usize; axes.len()];
    'walk: loop {
        let input: BTreeMap<String, Val> = axes.iter().zip(&at).map(|((n, vs), k)| (n.clone(), vs[*k].clone())).collect();
        if rulec::vectors::allowed(f, &input) {
            out.push(input);
        }
        for k in 0..at.len() {
            at[k] += 1;
            if at[k] < axes[k].1.len() {
                continue 'walk;
            }
            at[k] = 0;
        }
        break;
    }
    out
}

#[test]
fn a_value_no_input_reaches_is_no_gap() {
    let mut names: Vec<String> = std::fs::read_dir(root().join("tests/sieve")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    let mut listed: Vec<String> = PAIRS.iter().flat_map(|(en, ja, _)| [format!("{en}.rule"), format!("{ja}.rule")]).collect();
    listed.sort();
    assert_eq!(names, listed, "every rule of tests/sieve is in PAIRS, with its Japanese version");
    for (en, ja, _) in PAIRS {
        for stem in [en, ja] {
            let (rel, src) = source(stem);
            for lang in [Lang::En, Lang::Ja] {
                let ds = i18n::with(lang, || rulec::check_source(&src, &rel));
                assert!(!rulec::has_error(&ds), "{rel}: {:?}", ds.iter().map(|d| d.code).collect::<Vec<_>>());
            }
        }
        let (re, se) = source(en);
        let (rj, sj) = source(ja);
        assert_eq!(found(&se, &re), found(&sj, &rj), "{en} and {ja}");
    }
}

/// The values the check no longer asks a row for are reached by no input: the share never passes
/// the full score, and a small refund is never more than 50 pounds over the limit.
#[test]
fn what_is_no_longer_asked_for_no_input_reaches() {
    for (en, ja, _) in PAIRS {
        for stem in [en, ja] {
            let (rel, src) = source(stem);
            let (f, c) = rulec::prepare(&src, &rel).unwrap_or_else(|_| panic!("{rel} does not read"));
            let inputs = every_input(&f, &c);
            assert!(inputs.len() > 100, "{rel}: only {} inputs", inputs.len());
            for input in inputs {
                let (_, _, b) = i18n::with(Lang::En, || rulec::eval::run_all(&f, &c, input.clone().into_iter().collect::<HashMap<_, _>>()));
                let num = |alias: &str| -> Option<Rat> {
                    let name = if b.contains_key(alias) { alias.to_string() } else { f.items.iter().find_map(|it| match it {
                        rulec::ast::Item::Derived(d) if d.name.ascii.as_deref() == Some(alias) => Some(d.name.text.clone()),
                        rulec::ast::Item::Define(d) if d.name.ascii.as_deref() == Some(alias) => Some(d.name.text.clone()),
                        _ => None,
                    })? };
                    match b.get(&name) {
                        Some(Val::Num(v)) => Some(*v),
                        _ => None,
                    }
                };
                if stem.contains("define") || stem.contains("定義") {
                    let share = num("share").expect("the share is computed");
                    assert!(share.cmp_to(Rat::int(1)) != std::cmp::Ordering::Greater, "{rel}: {input:?} makes a share of {share}");
                } else {
                    let excess = num("excess").expect("the excess is computed");
                    let small = b.values().any(|v| matches!(v, Val::Enum(w) if w == "small" || w == "少額"));
                    assert!(!(small && excess.cmp_to(Rat::int(50)) == std::cmp::Ordering::Greater), "{rel}: {input:?} is small and {excess} over the limit");
                }
            }
        }
    }
}

/// The certificate says why with the leaf of §15.195.
#[test]
fn the_certificate_says_why_with_a_leaf_of_its_own() {
    for (en, ja, leaf) in PAIRS {
        for stem in [en, ja] {
            let rel = format!("tests/sieve/{stem}.rule");
            let o = std::process::Command::new(env!("CARGO_BIN_EXE_rulec")).current_dir(root()).args(["certificate", &rel]).output().unwrap();
            assert!(o.status.success(), "{rel}");
            let cert = String::from_utf8_lossy(&o.stdout);
            assert!(cert.contains(leaf), "{rel}: no {leaf} in the cover:\n{cert}");
        }
    }
}

/// A gap inside what the column comes to is still a gap, named by a value the column takes and the
/// input that makes it: the share of 50% when the middle row is left out, and a small refund just
/// over the limit when the row for it is.
#[test]
fn a_gap_inside_the_reach_is_still_named() {
    for (en, ja, _) in PAIRS {
        for (stem, lang) in [(en, Lang::En), (ja, Lang::Ja)] {
            let (rel, src) = source(stem);
            // the second row of the last table, left out
            let lines: Vec<&str> = src.lines().collect();
            let head = lines.iter().rposition(|l| l.starts_with("| ") && l.contains("->")).unwrap();
            let without = format!("{}\n", lines.iter().enumerate().filter(|(k, _)| *k != head + 2).map(|(_, l)| *l).collect::<Vec<_>>().join("\n"));
            let ds = i18n::with(lang, || rulec::check_source(&without, &rel));
            let gaps: Vec<_> = ds.iter().filter(|d| d.code == "E101").collect();
            assert_eq!(gaps.len(), 1, "{rel}: {ds:?}");
            let notes = gaps[0].notes.join("\n");
            let (example, input) = match (stem.contains("define") || stem.contains("定義"), lang) {
                (true, Lang::En) => ("An input that matches no row: share = 50%", "An input producing this example: score = 100"),
                (true, Lang::Ja) => ("当てはまらない例: 割合 = 50%", "この例を作る入力: 得点 = 100"),
                (false, Lang::En) => ("An input that matches no row: refund_size = small, excess = 1GBP", "An input producing this example: amount = 1, limit = 0"),
                (false, Lang::Ja) => ("当てはまらない例: 返金の大きさ = 少額, 超過額 = 1円", "この例を作る入力: 上限 = 0, 金額 = 1"),
            };
            assert!(notes.contains(example), "{rel}: {notes}");
            assert!(notes.contains(input), "{rel}: {notes}");
        }
    }
}

/// The example of a gap is a value the column takes: where the coordinate of the gap reaches below
/// what a `define` comes to, the example is the first value inside the reach, not the first value
/// of the coordinate (§15.195). A share between -10% and 10% is 0%, which a score of 0 makes.
#[test]
fn the_example_of_a_gap_is_a_value_the_column_takes() {
    let src = "rule v v1\n\nenum band = low | high\n\ninputs\n  score : number  range >=0 <=200\n\noutputs\n  grade : band\n\ndefine share : rate = score / 200\n\ntable t\npolicy unique\n| share  | -> grade : band |\n| <=-10% | low             |\n| >=10%  | high            |\n";
    let ds = i18n::with(Lang::En, || rulec::check_source(src, "v.rule"));
    let d = ds.iter().find(|d| d.code == "E101").unwrap_or_else(|| panic!("{ds:?}"));
    assert!(d.notes.iter().any(|n| n == "An input that matches no row: share = 0%"), "{:?}", d.notes);
    assert!(d.notes.iter().any(|n| n == "An input producing this example: score = 0"), "{:?}", d.notes);
}
