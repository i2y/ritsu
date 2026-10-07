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
//!
//! The same sieve after §15.196: a boolean `define` that always takes one truth value over the
//! ranges of the inputs is asked no row for the other, and the overlap checks (E105, W105, W114)
//! read the sieve as the completeness check does, so two rows that meet only where the rows of a
//! table above leave no values are no overlap. The certificate states such a pair apart, with the
//! rows of the table above that it rests on.

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

/// A boolean `define` that always takes one truth value (§15.196): the English rule and its Japanese
/// version.
const TRUTH: &[(&str, &str)] = &[("E101_a_define_that_is_always_true", "E101_いつも真になる定義")];

/// Two rows that meet only where the rows of a table above leave no values (§15.196): the English
/// rule, its Japanese version, the code the overlap was reported as before, and the column a table
/// above decides with the value its first row takes there, and what the second row asks of a
/// number, as (alias, bound): the two rows meet where the value is `small` and the number is past
/// the bound.
const OVERLAPS: &[(&str, &str, &str, &str, i64)] = &[
    ("E105_rows_that_meet_where_a_table_above_allows_nothing", "E105_上の表が許さない所で交わる行", "E105", "limit", 60),
    ("W105_rows_that_meet_where_a_table_above_allows_nothing", "W105_上の表が許さない所で交わる行", "W105", "excess", 50),
    ("W114_rows_that_meet_where_a_table_above_allows_nothing", "W114_上の表が許さない所で交わる行", "W114", "excess", 50),
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
    let mut listed: Vec<String> = PAIRS
        .iter()
        .map(|(en, ja, _)| (en, ja))
        .chain(TRUTH.iter().map(|(en, ja)| (en, ja)))
        .chain(OVERLAPS.iter().map(|(en, ja, ..)| (en, ja)))
        .flat_map(|(en, ja)| [format!("{en}.rule"), format!("{ja}.rule")])
        .collect();
    listed.sort();
    assert_eq!(names, listed, "every rule of tests/sieve is in PAIRS, TRUTH or OVERLAPS, with its Japanese version");
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

/// The value bound to a name, by the alias it is written with (`refund_size`, `excess`): the name
/// itself in the English rule, the name the alias is given to in the Japanese one.
fn bound(f: &rulec::ast::RuleFile, b: &HashMap<String, Val>, alias: &str) -> Option<Val> {
    if let Some(v) = b.get(alias) {
        return Some(v.clone());
    }
    let named = |n: &rulec::ast::Name| (n.ascii.as_deref() == Some(alias)).then(|| n.text.clone());
    let name = f.inputs.iter().find_map(|i| named(&i.name)).or_else(|| {
        f.items.iter().find_map(|it| match it {
            rulec::ast::Item::Derived(d) => named(&d.name),
            rulec::ast::Item::Define(d) => named(&d.name),
            rulec::ast::Item::Table(t) => t.outputs.iter().find_map(|o| named(&o.name)),
            _ => None,
        })
    })?;
    b.get(&name).cloned()
}

/// A boolean `define` that always takes one truth value over the ranges of the inputs: the check
/// asks no row for the other, the certificate says why with a leaf of its own, and every day the
/// input allows makes the define true (§15.196).
#[test]
fn a_define_that_is_always_true_is_asked_no_row_for_false() {
    for (en, ja) in TRUTH {
        for stem in [en, ja] {
            let (rel, src) = source(stem);
            for lang in [Lang::En, Lang::Ja] {
                let ds = i18n::with(lang, || rulec::check_source(&src, &rel));
                assert!(ds.is_empty(), "{rel}: {:?}", ds.iter().map(|d| d.code).collect::<Vec<_>>());
            }
            let o = std::process::Command::new(env!("CARGO_BIN_EXE_rulec")).current_dir(root()).args(["certificate", &rel]).output().unwrap();
            assert!(o.status.success(), "{rel}");
            assert!(String::from_utf8_lossy(&o.stdout).contains(r#"{"truth_axis":0}"#), "{rel}");
            let (f, c) = rulec::prepare(&src, &rel).unwrap_or_else(|_| panic!("{rel} does not read"));
            let input = &f.inputs[0].name.text;
            let (lo, hi) = c.ranges[input];
            let (lo, hi) = (lo.unwrap(), hi.unwrap());
            let mut days = 0;
            for y in 2024..=2026 {
                for m in 1..=12u32 {
                    let last = match m {
                        2 if y % 4 == 0 => 29,
                        2 => 28,
                        4 | 6 | 9 | 11 => 30,
                        _ => 31,
                    };
                    for d in 1..=last {
                        let at = rulec::types::date_ord(y, m, d);
                        if at.cmp_to(lo) == std::cmp::Ordering::Less || at.cmp_to(hi) == std::cmp::Ordering::Greater {
                            continue;
                        }
                        days += 1;
                        let (_, _, b) = i18n::with(Lang::En, || rulec::eval::run_all(&f, &c, HashMap::from([(input.clone(), Val::Date(y, m, d))])));
                        assert_eq!(bound(&f, &b, "reduced"), Some(Val::Bool(true)), "{rel}: {y}-{m}-{d}");
                    }
                }
            }
            assert!(days > 900, "{rel}: only {days} days");
        }
        let (re, se) = source(en);
        let (rj, sj) = source(ja);
        assert_eq!(found(&se, &re), found(&sj, &rj), "{en} and {ja}");
    }
}

/// A gap at the truth value the define does take is still a gap: with the row for `true` asking
/// for `false` instead, E101 names `true`.
#[test]
fn a_gap_at_the_truth_value_the_define_takes_is_still_named() {
    for (en, ja) in TRUTH {
        for (stem, lang, example) in [(en, Lang::En, "An input that matches no row: reduced = true"), (ja, Lang::Ja, "当てはまらない例: 軽減期間 = true")] {
            let (rel, src) = source(stem);
            let lines: Vec<&str> = src.lines().collect();
            // the table's header, the first line with an answer column (the examples come after it)
            let head = lines.iter().position(|l| l.starts_with("| ") && l.contains("->")).unwrap();
            let flipped = format!("{}\n", lines.iter().enumerate().map(|(k, l)| if k == head + 1 { l.replacen("true ", "false", 1) } else { l.to_string() }).collect::<Vec<_>>().join("\n"));
            let ds = i18n::with(lang, || rulec::check_source(&flipped, &rel));
            let gaps: Vec<_> = ds.iter().filter(|d| d.code == "E101").collect();
            assert_eq!(gaps.len(), 1, "{rel}: {ds:?}\n{flipped}");
            assert!(gaps[0].notes.iter().any(|n| n == example), "{rel}: {:?}", gaps[0].notes);
        }
    }
}

/// Two rows that meet only where the rows of a table above leave no values are no overlap: before
/// §15.196 the check reported the pair (E105 with an input it made up, W105, or W114), and now it
/// says nothing, and no input inside the ranges reaches both rows. A `unique` table's certificate
/// states the pair apart on the rows of the table above.
#[test]
fn rows_that_meet_only_where_no_input_reaches_do_not_overlap() {
    for (en, ja, code, alias, past) in OVERLAPS {
        for stem in [en, ja] {
            let (rel, src) = source(stem);
            for lang in [Lang::En, Lang::Ja] {
                let ds = i18n::with(lang, || rulec::check_source(&src, &rel));
                assert!(!ds.iter().any(|d| matches!(d.code, "E105" | "W105" | "W114") || d.severity == rulec::diag::Severity::Error), "{rel}: {:?}", ds.iter().map(|d| d.code).collect::<Vec<_>>());
            }
            if *code != "W105" {
                let o = std::process::Command::new(env!("CARGO_BIN_EXE_rulec")).current_dir(root()).args(["certificate", &rel]).output().unwrap();
                assert!(o.status.success(), "{rel}");
                assert!(String::from_utf8_lossy(&o.stdout).contains(r#""above_apart":[{"a":1,"b":2,"axis":0,"#), "{rel}");
            }
            let (f, c) = rulec::prepare(&src, &rel).unwrap_or_else(|_| panic!("{rel} does not read"));
            let inputs = every_input(&f, &c);
            assert!(inputs.len() > 1000, "{rel}: only {} inputs", inputs.len());
            for input in inputs {
                let (_, _, b) = i18n::with(Lang::En, || rulec::eval::run_all(&f, &c, input.clone().into_iter().collect::<HashMap<_, _>>()));
                let small = matches!(bound(&f, &b, "refund_size"), Some(Val::Enum(w)) if w == "small" || w == "少額");
                let beyond = match bound(&f, &b, alias) {
                    Some(Val::Num(v)) => v.cmp_to(Rat::int(*past as i128)) == std::cmp::Ordering::Greater,
                    other => panic!("{rel}: {alias} is {other:?}"),
                };
                assert!(!(small && beyond), "{rel}: {input:?} reaches both rows");
            }
        }
        let (re, se) = source(en);
        let (rj, sj) = source(ja);
        assert_eq!(found(&se, &re), found(&sj, &rj), "{en} and {ja}");
    }
}

/// A real overlap is still named: with the second row asking for a number that a small refund
/// does reach (40 in place of the bound), the pair is reported again as it was.
#[test]
fn a_real_overlap_is_still_named() {
    for (en, ja, code, _, past) in OVERLAPS {
        for stem in [en, ja] {
            let (rel, src) = source(stem);
            // the bound in the second row of the last table, the last place it is written
            let at = src.rfind(&format!(">{past}")).unwrap();
            let moved = format!("{}>40{}", &src[..at], &src[at + format!(">{past}").len()..]);
            assert_ne!(moved, src, "{rel}");
            let ds = i18n::with(Lang::En, || rulec::check_source(&moved, &rel));
            let want = if *code == "W105" { "W105" } else { "E105" };
            assert!(ds.iter().any(|d| d.code == want), "{rel}: {:?}", ds.iter().map(|d| d.code).collect::<Vec<_>>());
        }
    }
}
