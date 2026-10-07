//! E102's third form (DESIGN §15.189, §15.195): a row that asks a derived or `define` column only
//! for values it never comes to, or only for values it never comes to while a table above writes
//! the value another column of the row asks for. Each rule of `tests/reach` is written in English,
//! with a Japanese version beside it, and `rulec check` says E102 once, with the values the column
//! can come to marked on its line; what it prints in English and in Japanese is the rule's golden
//! file in `tests/golden/reach` (`RULEC_BLESS=1 cargo test --test reach` writes them again).
//!
//! The row is reached by no input: every input inside the ranges goes through the reference
//! evaluator, and none of them makes the row fire. Deleting the row leaves the table complete, so
//! E101 never asks for what E102 called dead. A derive whose values fall between whole units — a
//! tenth of an amount — is read too, since its axis is cut on the step its values take (§15.190);
//! so is a `define`, and a derive over a product, a rounding or a table's output, whatever its
//! expression's interval holds (§15.195). What the form does not read — a row only a `constraint`
//! rules out, or one only the tables above do — passes as it did before.

use rulec::eval::Val;
use rulec::i18n::{self, Lang};
use rulec::num::Rat;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The rules: the English one, its Japanese version, and the row E102 calls dead in both.
const PAIRS: &[(&str, &str, usize)] = &[
    ("E102_past_a_derives_reach", "E102_導出の届かない行", 3),
    ("E102_earlier_rows_take_the_rest", "E102_残りは上の行が取る", 4),
    ("E102_reach_under_a_constraint", "E102_制約で狭まる導出", 1),
    ("E102_two_derives_together", "E102_二つの導出の組", 1),
    ("E102_past_a_tenth_of_the_amount", "E102_金額の一割が届かない行", 3),
    ("E102_past_a_defines_reach", "E102_定義の届かない行", 3),
    ("E102_past_what_a_table_above_allows", "E102_上の表が許さない行", 2),
];

/// A row that asks a boolean `define` for the truth value it never takes over the ranges of the
/// inputs (§15.196): the English rule, its Japanese version, and the row.
const TRUTH: &[(&str, &str, usize)] = &[("E102_asks_what_a_define_never_is", "E102_定義がとらない真偽を求める行", 2)];

fn source(stem: &str) -> (String, String) {
    let rel = format!("tests/reach/{stem}.rule");
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

#[test]
fn a_row_past_a_derives_reach_is_e102_with_what_the_derive_comes_to() {
    let mut names: Vec<String> = std::fs::read_dir(root().join("tests/reach")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    let mut listed: Vec<String> = PAIRS.iter().chain(TRUTH).flat_map(|(en, ja, _)| [format!("{en}.rule"), format!("{ja}.rule")]).collect();
    listed.sort();
    assert_eq!(names, listed, "every rule of tests/reach is in PAIRS or TRUTH, with its Japanese version");
    let mut failures = Vec::new();
    for (en, ja, row) in PAIRS {
        for stem in [en, ja] {
            let (rel, src) = source(stem);
            let ds = i18n::with(Lang::En, || rulec::check_source(&src, &rel));
            let errors: Vec<_> = ds.iter().filter(|d| d.severity == rulec::diag::Severity::Error).collect();
            if errors.len() != 1 || errors[0].code != "E102" {
                failures.push(format!("{rel}: {:?}, not one E102", errors.iter().map(|d| d.code).collect::<Vec<_>>()));
                continue;
            }
            if !errors[0].title.contains(&format!("row {row} ")) {
                failures.push(format!("{rel}: E102 is about {:?}, not row {row}", errors[0].title));
            }
            for (lang, tag) in [(Lang::En, "en"), (Lang::Ja, "ja")] {
                let out = printed(&rel, &src, lang);
                // the third form: the derive's reach, marked and said
                let (mark, said) = if lang == Lang::En { ("the reachable interval is", "can only come to") } else { ("実際に取りうる値は", "が取りうる値は") };
                if !out.contains(mark) || !out.contains(said) {
                    failures.push(format!("{rel} ({tag}) does not say what the derive comes to:\n{out}"));
                }
                if let Err(e) = ritsu_testkit::golden::check(&root().join(format!("tests/golden/reach/{stem}.{tag}.txt")), &out) {
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
    for (en, ja, _) in PAIRS {
        let (re, se) = source(en);
        let (rj, sj) = source(ja);
        assert_eq!(found(&se, &re), found(&sj, &rj), "{en} and {ja}");
    }
}

/// Every input inside the declared ranges (whole units; the `constraint` lines kept) goes through
/// the reference evaluator, and the row E102 calls dead fires for none of them, while the rows
/// around it fire for some.
#[test]
fn no_input_reaches_the_row() {
    for (en, ja, row) in PAIRS {
        for stem in [en, ja] {
            let (rel, src) = source(stem);
            let (f, c) = rulec::prepare(&src, &rel).unwrap_or_else(|_| panic!("{rel} does not read"));
            // the last table, which holds the row (a table above, where there is one, comes first)
            let table = f
                .items
                .iter()
                .filter_map(|it| match it {
                    rulec::ast::Item::Table(t) => t.name.as_ref().map(|n| n.text.clone()),
                    _ => None,
                })
                .last()
                .unwrap();
            let axes: Vec<(String, Vec<Val>)> = f
                .inputs
                .iter()
                .map(|i| {
                    let (lo, hi) = c.ranges[&i.name.text];
                    let (lo, hi) = (lo.unwrap().num, hi.unwrap().num);
                    (i.name.text.clone(), (lo..=hi).map(|v| Val::Num(Rat::int(v))).collect())
                })
                .collect();
            let (dead, mut fired_rows) = i18n::with(Lang::En, || (rulec::eval::row_tag(&table, *row), std::collections::BTreeSet::new()));
            let mut at = vec![0usize; axes.len()];
            let mut walked = 0usize;
            'walk: loop {
                let input: BTreeMap<String, Val> = axes.iter().zip(&at).map(|((n, vs), k)| (n.clone(), vs[*k].clone())).collect();
                if rulec::vectors::allowed(&f, &input) {
                    walked += 1;
                    let (_, fired, _) = i18n::with(Lang::En, || rulec::eval::run_all(&f, &c, input.clone().into_iter().collect::<HashMap<_, _>>()));
                    assert!(!fired.contains(&dead), "{rel}: {dead} fires for {input:?}");
                    fired_rows.extend(fired);
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
            assert!(walked > 100, "{rel}: only {walked} inputs");
            assert!(fired_rows.len() >= 2, "{rel}: the other rows fire too: {fired_rows:?}");
        }
    }
}

/// Deleting the row leaves the rule passing: the box E102 called dead is one E101 never asks a row
/// for, so the two never tell the writer opposite things.
#[test]
fn deleting_the_row_leaves_no_gap() {
    for (en, ja, row) in PAIRS {
        for stem in [en, ja] {
            let (rel, src) = source(stem);
            // the row's line: the n-th line of the last table after its header
            let lines: Vec<&str> = src.lines().collect();
            let head = lines.iter().rposition(|l| l.starts_with("| ") && l.contains("->")).unwrap();
            let kept: Vec<&str> = lines.iter().enumerate().filter(|(k, _)| *k != head + row).map(|(_, l)| *l).collect();
            let without = format!("{}\n", kept.join("\n"));
            let codes: Vec<String> = found(&without, &rel).into_iter().map(|(c, _)| c).collect();
            assert!(codes.iter().all(|c| !c.starts_with('E')), "{rel} without row {row}: {codes:?}\n{without}");
        }
    }
}

/// The rows of another table that take precedence take the rest, and a `constraint` that does not
/// bear on the derive is said apart from it.
#[test]
fn what_takes_the_rest_of_the_row_is_said() {
    let overrides = "rule v v1\n\nenum band = small | big | huge\n\ninputs\n  amount : money[GBP]  range >=1GBP <=100GBP\n  limit  : money[GBP]  range >=0GBP <=100GBP\n\noutputs\n  out : band\n\nderive excess : money[GBP] = amount - limit  range >=-1000GBP <=1000GBP\n\ntable base\npolicy unique\n| excess  | -> out : band |\n| <=50GBP | small         |\n| >50GBP  | big           |\n\ntable special\noverrides base\npolicy unique\n| excess          | -> out : band |\n| >50GBP <=200GBP | huge          |\n";
    let unrelated = "rule v v1\n\nenum band = small | big | huge\n\ninputs\n  amount : money[GBP]  range >=1GBP <=100GBP\n  limit  : money[GBP]  range >=0GBP <=100GBP\n  x      : number  range >=0 <=10\n  y      : number  range >=0 <=10\n\nconstraint x <= y\n\noutputs\n  out : band\n\nderive excess : money[GBP] = amount - limit  range >=-1000GBP <=1000GBP\n\ntable t\npolicy first\n| excess   | x  | y   | -> out : band |\n| <=0GBP   | -  | -   | small         |\n| <=100GBP | -  | -   | big           |\n| -        | >5 | <=5 | huge          |\n| -        | -  | -   | huge          |\n";
    let notes = |src: &str, lang: Lang| -> String {
        i18n::with(lang, || {
            let ds = rulec::check_source(src, "v.rule");
            let d: Vec<_> = ds.iter().filter(|d| d.code == "E102").collect();
            assert_eq!(d.len(), 1, "{ds:?}");
            d[0].notes.join("\n")
        })
    };
    let en = notes(overrides, Lang::En);
    assert!(en.contains("`excess` can only come to >=-99GBP <=100GBP. The rows of table special, which take precedence, take first every one of those values that meets this row's conditions."), "{en}");
    let ja = notes(overrides, Lang::Ja);
    assert!(ja.contains("その中でこの行の条件に当てはまる値は、優先する 表 special の行がすべて先に取ります。"), "{ja}");
    let en = notes(unrelated, Lang::En);
    assert!(en.contains("Over the ranges of the inputs, `excess` can only come to >=-99GBP <=100GBP. Because of `policy first`"), "{en}");
    assert!(en.contains("The combinations of inputs that `constraint x <= y` rules out never come either."), "{en}");
    let ja = notes(unrelated, Lang::Ja);
    assert!(ja.contains("`excess` が取りうる値は、入力の範囲から計算すると >=-99GBP <=100GBP です。"), "{ja}");
    assert!(ja.contains("`constraint x <= y` が許さない入力の組み合わせも来ません。"), "{ja}");
}

/// koyomi's days take part too (`range from koyomi`, with koyomi joined as `ritsu rulec` joins it):
/// the rows above take every payment day but those of a gap that holds none, and the rest of the
/// row lies past what the derive comes to. The days are said in a note of their own.
#[test]
fn the_days_of_a_koyomi_date_are_said_when_they_take_part() {
    let t = ritsu_testkit::TempDir::new("reach-days");
    std::fs::copy(root().join("tests/days/payment_terms.cal"), t.path().join("payment_terms.cal")).unwrap();
    let src = "rule v v1\n\nenum band = early | late | mid\n\ninputs\n  pay_day : date  range from koyomi \"payment_terms.cal\" date payment\n  amount  : money[GBP]  range >=1GBP <=100GBP\n  limit   : money[GBP]  range >=0GBP <=100GBP\n\noutputs\n  out : band\n\nderive excess : money[GBP] = amount - limit  range >=-1000GBP <=1000GBP\n\ntable t\npolicy first\n| pay_day      | excess    | -> out : band |\n| <=2026-06-30 | -         | early         |\n| >=2026-07-10 | -         | late          |\n| -            | >=-200GBP | mid           |\n";
    let path = t.path().join("v.rule").to_string_lossy().into_owned();
    std::fs::write(&path, src).unwrap();
    let koyomi: rulec::days::Port = std::sync::Arc::new(koyomi::ports::Engine);
    let notes = |lang: Lang| -> String {
        rulec::days::with(Some(koyomi.clone()), || {
            i18n::with(lang, || {
                let ds = rulec::check_source(src, &path);
                let d: Vec<_> = ds.iter().filter(|d| d.code == "E102").collect();
                assert_eq!(d.len(), 1, "{ds:?}");
                assert!(d[0].title.contains(if lang == Lang::En { "row 3" } else { "行3" }), "{}", d[0].title);
                d[0].notes.join("\n")
            })
        })
    };
    let en = notes(Lang::En);
    assert!(en.contains("`excess` can only come to >=-99GBP <=100GBP. Because of `policy first`, the earlier rows take first"), "{en}");
    assert!(en.contains("pay_day takes only the days koyomi \"payment_terms.cal\" date payment comes to."), "{en}");
    let ja = notes(Lang::Ja);
    assert!(ja.contains("pay_day は koyomi \"payment_terms.cal\" date payment がとる日だけです。"), "{ja}");
}

/// What the third form does not read passes as it did before, though the row in each is reached
/// by no input: a row only a `constraint` rules out, and one the tables above rule out together
/// with a `constraint` and no derived or `define` value. It stops a rule only where such a value
/// takes part (§15.189, §15.195); E101 reads the same, so it never asks for the rows back.
#[test]
fn what_the_third_form_does_not_read_passes_as_before() {
    let head = "rule v v1\n\nenum band = small | big | huge\nenum size = low | high\n\ninputs\n  amount : money[GBP]  range >=1GBP <=100GBP\n  limit  : money[GBP]  range >=0GBP <=100GBP\n  k      : bool  contract_only\n\noutputs\n  out : band\n\n";
    let cases = [
        // a row only a `constraint` rules out, with no derive in it
        ("a row only a constraint rules out", "constraint limit <= amount\n\ntable t\npolicy unique\n| amount  | limit   | -> out : band |\n| >50GBP  | -       | small         |\n| <=50GBP | <=50GBP | big           |\n| <=50GBP | >50GBP  | huge          |\n"),
        // a value decided above and a `constraint`: `low` is written only for an amount up to 50
        // pounds, and the limit never passes the amount
        ("a row the table above and a constraint rule out", "constraint limit <= amount\n\ntable size_of\npolicy unique\n| amount  | -> s : size |\n| <=50GBP | low         |\n| >50GBP  | high        |\n\ntable t\npolicy unique\n| s    | limit   | -> out : band |\n| low  | <=50GBP | small         |\n| low  | >50GBP  | huge          |\n| high | -       | big           |\n"),
    ];
    for (what, body) in cases {
        let src = format!("{head}{body}");
        let codes: Vec<String> = found(&src, "v.rule").into_iter().map(|(c, _)| c).collect();
        assert!(!codes.iter().any(|c| c.starts_with('E')), "{what}: {codes:?}\n{src}");
        // and the row is reached by no input all the same: its value is never the answer
        let (f, c) = rulec::prepare(&src, "v.rule").unwrap();
        let mut answers = std::collections::BTreeSet::new();
        for a in 1..=100 {
            for l in 0..=100 {
                for k in [true, false] {
                    let input: BTreeMap<String, Val> = [("amount", Val::Num(Rat::int(a))), ("limit", Val::Num(Rat::int(l))), ("k", Val::Bool(k))].into_iter().map(|(n, v)| (n.to_string(), v)).collect();
                    if !rulec::vectors::allowed(&f, &input) {
                        continue;
                    }
                    let (outs, _, _) = rulec::eval::run_all(&f, &c, input.into_iter().collect());
                    if let Some((_, Some(v))) = outs.iter().find(|(n, _)| n == "out") {
                        answers.insert(format!("{v:?}"));
                    }
                }
            }
        }
        assert!(!answers.iter().any(|a| a.contains("huge")), "{what}: the last row is reached: {answers:?}");
    }
}

/// What the third form reads since §15.195, which passed before though the row in each is reached
/// by no input: a `define` in a column, a derive that reads a table's output, a derive of a
/// product. Each comes to the interval its expression is forced into, and the row asks only past
/// it.
#[test]
fn a_define_and_a_derive_of_any_shape_are_read() {
    let head = "rule v v1\n\nenum band = small | big | huge\n\ninputs\n  amount : money[GBP]  range >=1GBP <=100GBP\n  limit  : money[GBP]  range >=0GBP <=100GBP\n  k      : bool  contract_only\n\noutputs\n  out : band\n\n";
    let cases = [
        ("a define in a column", "derive total : money[GBP] = amount + limit  range >=0GBP <=1000GBP\ndefine double : money[GBP] = total * 2\n\ntable t\npolicy unique\n| double           | -> out : band |\n| <=200GBP         | small         |\n| >200GBP <=400GBP | big           |\n| >400GBP          | huge          |\n", "`double` can only come to >=2GBP <=400GBP. None of those values meets this row's conditions.", "`double` が取りうる値は、入力の範囲から計算すると >=2GBP <=400GBP です。"),
        ("a derive that reads a table's output", "table fee_of\npolicy unique\n| k     | -> fee : money[GBP] |\n| true  | 10GBP               |\n| false | 20GBP               |\n\nderive net : money[GBP] = amount - fee  range >=-1000GBP <=1000GBP\n\ntable t\npolicy unique\n| net            | -> out : band |\n| <=0GBP         | small         |\n| >0GBP <=200GBP | big           |\n| >200GBP        | huge          |\n", "Over the ranges of the inputs and the values the tables above produce, `net` can only come to >=-19GBP <=90GBP.", "`net` が取りうる値は、入力の範囲と、上流の表が出す値から計算すると >=-19GBP <=90GBP です。"),
        ("a derive of a product", "derive area : money[GBP] = amount * 3  range >=0GBP <=1000GBP\n\ntable t\npolicy unique\n| area             | -> out : band |\n| <=100GBP         | small         |\n| >100GBP <=300GBP | big           |\n| >300GBP          | huge          |\n", "`area` can only come to >=3GBP <=300GBP.", "`area` が取りうる値は、入力の範囲から計算すると >=3GBP <=300GBP です。"),
    ];
    for (what, body, en, ja) in cases {
        let src = format!("{head}{body}");
        let notes = |lang: Lang| -> String {
            i18n::with(lang, || {
                let ds = rulec::check_source(&src, "v.rule");
                let d: Vec<_> = ds.iter().filter(|d| d.code == "E102").collect();
                assert_eq!(d.len(), 1, "{what}: {ds:?}");
                assert!(d[0].title.contains(if lang == Lang::En { "row 3" } else { "行3" }), "{what}: {}", d[0].title);
                d[0].notes.join("\n")
            })
        };
        let got = notes(Lang::En);
        assert!(got.contains(en), "{what}: {got}");
        let got = notes(Lang::Ja);
        assert!(got.contains(ja), "{what}: {got}");
    }
}

/// The JSON of the third form carries both places — the row, and the derive's expression with what
/// it comes to — and the data half is the same in both languages.
#[test]
fn the_json_marks_the_row_and_the_derive_the_same_in_both_languages() {
    let (rel, src) = source("E102_past_a_derives_reach");
    let json = |lang: Lang| -> rulec::json::Json {
        i18n::with(lang, || {
            let ds = rulec::check_source(&src, &rel);
            let d = ds.iter().find(|d| d.code == "E102").unwrap();
            rulec::json::parse(&rulec::diag::render_json(d, &rel)).unwrap()
        })
    };
    let (en, ja) = (json(Lang::En), json(Lang::Ja));
    let place = |j: &rulec::json::Json| -> Vec<(i128, i128, i128)> {
        j.get("spans").and_then(|s| s.as_arr()).unwrap().iter().map(|s| (s.get("line").and_then(|x| x.as_int()).unwrap(), s.get("column").and_then(|x| x.as_int()).unwrap(), s.get("length").and_then(|x| x.as_int()).unwrap())).collect()
    };
    assert_eq!(place(&en), vec![(20, 1, 38), (13, 30, 14)]);
    assert_eq!(place(&en), place(&ja));
    for k in ["where", "rows", "fix", "witness"] {
        assert_eq!(format!("{:?}", en.get(k)), format!("{:?}", ja.get(k)), "{k}");
    }
}

/// A row that asks a boolean `define` for the truth value it never takes is E102's third form
/// (§15.196): the define's line is marked with the value it always takes, the note says so, and
/// deleting the row leaves the rule passing. The same findings on the same lines in both versions.
#[test]
fn a_row_that_asks_what_a_define_never_is_is_e102() {
    let mut failures = Vec::new();
    for (en, ja, row) in TRUTH {
        for stem in [en, ja] {
            let (rel, src) = source(stem);
            let ds = i18n::with(Lang::En, || rulec::check_source(&src, &rel));
            let errors: Vec<_> = ds.iter().filter(|d| d.severity == rulec::diag::Severity::Error).collect();
            if errors.len() != 1 || errors[0].code != "E102" || !errors[0].title.contains(&format!("row {row} ")) {
                failures.push(format!("{rel}: {:?}, not one E102 on row {row}", errors.iter().map(|d| (d.code, d.title.clone())).collect::<Vec<_>>()));
                continue;
            }
            for (lang, tag) in [(Lang::En, "en"), (Lang::Ja, "ja")] {
                let out = printed(&rel, &src, lang);
                let (mark, said) = if lang == Lang::En {
                    ("always true over the ranges of the inputs", "is always true, and this row asks for it to be false.")
                } else {
                    ("入力の範囲では、いつも true です", "はいつも true です。この行は false を求めています。")
                };
                if !out.contains(mark) || !out.contains(said) {
                    failures.push(format!("{rel} ({tag}) does not say what the define always is:\n{out}"));
                }
                if let Err(e) = ritsu_testkit::golden::check(&root().join(format!("tests/golden/reach/{stem}.{tag}.txt")), &out) {
                    failures.push(e);
                }
            }
            // deleting the row: the table's header is the first line with an answer column
            let lines: Vec<&str> = src.lines().collect();
            let head = lines.iter().position(|l| l.starts_with("| ") && l.contains("->")).unwrap();
            let kept: Vec<&str> = lines.iter().enumerate().filter(|(k, _)| *k != head + row).map(|(_, l)| *l).collect();
            let without = format!("{}\n", kept.join("\n"));
            let codes: Vec<String> = found(&without, &rel).into_iter().map(|(c, _)| c).collect();
            if codes.iter().any(|c| c.starts_with('E')) {
                failures.push(format!("{rel} without row {row}: {codes:?}"));
            }
        }
        let (re, se) = source(en);
        let (rj, sj) = source(ja);
        if found(&se, &re) != found(&sj, &rj) {
            failures.push(format!("{en} and {ja} say different things"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
