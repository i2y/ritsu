//! The input behind a gap on a column the linear model does not tie to the inputs (DESIGN §15.194).
//!
//! E101 on a column the rule computes names a value no caller sends, and beside it the input that
//! produces it (§15.190). That input was worked out from the table's linear model, which holds the
//! derives that add, subtract and multiply by constants and nothing else, so a gap on a `min`, a
//! rounding, a `define` or what a table above gives came with no input at all. Where the inputs
//! behind the columns are few — at most 2^14 combinations, the bound of the other exhaustive looks
//! (§15.153) — every one of them is walked, and the first one the reference evaluator confirms is
//! named.
//!
//! `tests/mutants/m_e101ratio.rule` and `m_e101ratioja.rule` are the corpus rule `rating_grade` and
//! its Japanese version with the lowest band written `<50%` where the catch-all was: the column is
//! a `define`, the share of the full score, and the input behind 50% is found among 14,641 scores.

use ritsu_testkit::{Need, TempDir, ready};
use rulec::eval::Val;
use rulec::i18n::{self, Lang};
use rulec::num::Rat;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn source(rel: &str) -> String {
    std::fs::read_to_string(root().join(rel)).unwrap_or_else(|_| panic!("cannot read {rel}"))
}

fn have_python() -> bool {
    Command::new("python3").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// What the note says before the input, in each language.
const EN: &str = "An input producing this example: ";
const JA: &str = "この例を作る入力: ";

/// The band on the lower of the amount and a tenth of it, written in whole pounds as if the value
/// came in whole pounds: 1.1GBP to 1.9GBP falls in neither row. `min` is not in the linear model.
/// The English rule, and its Japanese version.
const LOWEST: &str = "rule lowest_band v1\n\ninputs\n  amount : money[GBP]  range >=1GBP <=100GBP\n\noutputs\n  small : bool\n\nderive low : money[GBP] = min(amount, amount * 10%)  range >=0GBP <=100GBP\n\ntable band\npolicy unique\n| low    | -> small : bool |\n| <=1GBP | true            |\n| >=2GBP | false           |\n";
const LOWEST_JA: &str = "rule 少額の区分(lowest_band) v1\n\ninputs\n  金額(amount) : money[円]  range >=1円 <=100円\n\noutputs\n  少額(small) : bool\n\nderive 低い方(low) : money[円] = min(金額, 金額 × 10%)  range >=0円 <=100円\n\ntable 区分(band)\npolicy unique\n| 低い方 | -> 少額(small) : bool |\n| <=1円  | true                  |\n| >=2円  | false                 |\n";

/// Each version of the band: its text, a file name, and the names it gives the amount and the
/// lower value, and its currency.
const LOWEST_PAIR: [(&str, &str, &str, &str, &str); 2] =
    [(LOWEST, "lowest_band.rule", "amount", "low", "GBP"), (LOWEST_JA, "少額の区分.rule", "金額", "低い方", "円")];

/// The mutants: the file, the names of the four scores in the order the rule declares them, and
/// the name of the share.
const RATIO: [(&str, [&str; 4], &str); 2] = [
    ("tests/mutants/m_e101ratio.rule", ["quality", "delivery", "price", "support"], "ratio"),
    ("tests/mutants/m_e101ratioja.rule", ["品質", "納期", "価格", "対応"], "達成率"),
];

/// The errors a rule's text gets, in `lang`.
fn errors(src: &str, rel: &str, lang: Lang) -> Vec<rulec::diag::Diag> {
    i18n::with(lang, || rulec::check_source(src, rel).into_iter().filter(|d| d.severity == rulec::diag::Severity::Error).collect())
}

/// The one error a rule's text gets, which is E101, in `lang`, as JSON too.
fn the_error(src: &str, rel: &str, lang: Lang) -> (rulec::diag::Diag, rulec::json::Json) {
    let errors = errors(src, rel, lang);
    assert_eq!(errors.iter().map(|d| d.code).collect::<Vec<_>>(), ["E101"], "{rel}");
    let json = i18n::with(lang, || rulec::json::parse(&rulec::diag::render_json(&errors[0], rel)).unwrap());
    (errors.into_iter().next().unwrap(), json)
}

/// The input a note hands over, if it hands one over.
fn behind<'a>(d: &'a rulec::diag::Diag, said: &str) -> Option<&'a str> {
    d.notes.iter().find_map(|n| n.strip_prefix(said))
}

/// `name = value, …`, as whole numbers, or the name of an enum value.
fn input_of(note: &str) -> BTreeMap<String, String> {
    note.split(", ")
        .map(|p| {
            let (n, v) = p.split_once(" = ").unwrap_or_else(|| panic!("not `name = value`: {p}"));
            (n.to_string(), v.to_string())
        })
        .collect()
}

fn num(v: &str) -> i128 {
    v.parse().unwrap_or_else(|_| panic!("not a whole number: {v}"))
}

/// What the reference evaluator gives for whole-number inputs: the rows that took them, and every
/// value it worked out.
fn evaluate(src: &str, rel: &str, input: &[(&str, i128)]) -> (Vec<String>, HashMap<String, Val>) {
    let (f, c) = rulec::prepare(src, rel).expect("the rule reads");
    let env: HashMap<String, Val> = input.iter().map(|(n, v)| (n.to_string(), Val::Num(Rat::int(*v)))).collect();
    let (outs, fired, b) = rulec::eval::run_all(&f, &c, env);
    if fired.is_empty() {
        assert!(outs.iter().all(|(_, v)| v.is_none()), "{rel}: an answer came with no row: {outs:?}");
    }
    (fired, b)
}

/// The generated Python asked each call in `calls` (`name(args)`), one line each: the answer, or
/// that it stopped. The code is made from the rule without the check, as `rulec gen` would before
/// it refused a rule with a gap.
fn ask_python(tmp: &TempDir, tag: &str, src: &str, rel: &str, module: &str, calls: &[String]) -> String {
    let (f, c) = rulec::prepare(src, rel).expect("the rule reads");
    let dir = tmp.path().join(tag);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(format!("{module}.py")), rulec::codegen::Gen::new(&f, &c, src, rel).python()).unwrap();
    let asks: Vec<String> = calls.iter().map(|call| format!("ask({call:?}, lambda: m.{call})")).collect();
    let script = format!(
        "import sys\nsys.path.insert(0, {dir:?})\nimport {module} as m\ndef ask(call, f):\n    try:\n        print(call, f())\n    except AssertionError as e:\n        print(call, 'stopped:', e)\n{}\n",
        asks.join("\n"),
        dir = dir.to_string_lossy()
    );
    let o = Command::new("python3").arg("-c").arg(script).output().expect("python3 runs");
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).into_owned()
}

const STOPPED: &str = "stopped: unreachable: completeness was statically checked by rulec";

/// The gap on `min(amount, amount * 10%)` is E101 with the value in it, the input behind it and the
/// row to add, in the shape a gap on a linear derive has them (`tests/steps.rs`): the value as the
/// number it is in the JSON witness, the input in the note, the row in cells the column takes. The
/// Japanese version says the same on the same line.
#[test]
fn a_gap_on_a_min_names_the_input_behind_it() {
    let mut lines = Vec::new();
    for (src, rel, amount, low, unit) in LOWEST_PAIR {
        for (lang, said) in [(Lang::En, EN), (Lang::Ja, JA)] {
            let (d, json) = the_error(src, rel, lang);
            lines.push(d.where_.clone());
            let witness = json.get("witness").and_then(|w| w.get("inputs")).and_then(|w| w.get(low)).and_then(|v| v.as_str());
            assert_eq!(witness, Some("1.1"), "{rel}: {}", json.compact());
            let input = behind(&d, said).unwrap_or_else(|| panic!("{rel}: no input behind the example: {:?}", d.notes));
            assert_eq!(input_of(input), BTreeMap::from([(amount.to_string(), "11".to_string())]), "{rel}");
            let notes: Vec<&str> = json.get("notes").and_then(|n| n.as_arr()).unwrap().iter().filter_map(|n| n.as_str()).collect();
            assert!(notes.contains(&format!("{said}{input}").as_str()), "{rel}: the JSON's notes: {notes:?}");
            let fix = json.get("fix").and_then(|f| f.get("text")).and_then(|t| t.as_str());
            assert_eq!(fix, Some(format!("| >1{unit} <2{unit} | true |").as_str()), "{rel}");
        }
    }
    // on the table's heading, line 11 of both
    assert!(lines.iter().all(|l| l.contains(".rule:11 ")), "{lines:?}");
}

/// The input E101 names behind the `min` is one the generated code stops on: the reference
/// evaluator works the lower value out at 1.1 and takes no row, and the code made without the check
/// raises its `unreachable` assertion there, while answering on either side.
#[test]
fn the_generated_code_stops_on_the_input_behind_a_min() {
    if !ready(Need::Python, have_python, "python3 is not here") {
        return;
    }
    let tmp = TempDir::new("behind-min");
    for (k, (src, rel, amount, low, _)) in LOWEST_PAIR.into_iter().enumerate() {
        let (d, _) = the_error(src, rel, Lang::En);
        let input = input_of(behind(&d, EN).unwrap());
        let at = num(&input[amount]);
        let (fired, b) = evaluate(src, rel, &[(amount, at)]);
        assert!(fired.is_empty(), "{rel}: a row took it: {fired:?}");
        assert_eq!(b.get(low), Some(&Val::Num(Rat::new(11, 10))), "{rel}");
        let said = ask_python(&tmp, &format!("{k}"), src, rel, "lowest_band", &[10, at, 20].map(|a| format!("lowest_band({a})")));
        assert_eq!(said, format!("lowest_band(10) True\nlowest_band({at}) {STOPPED}\nlowest_band(20) False\n"), "{rel}");
    }
}

/// The gap of the mutants is on a `define`, which the linear model does not hold, behind a derive
/// of four scores. The walk names an input that makes the share exactly the 50% the example shows
/// and that no row takes, and both versions name the same scores.
#[test]
fn a_gap_on_a_define_names_the_input_behind_it() {
    let mut named = Vec::new();
    for (rel, scores, share) in RATIO {
        let src = source(rel);
        for (lang, said, example) in [(Lang::En, EN, "An input that matches no row: "), (Lang::Ja, JA, "当てはまらない例: ")] {
            let (d, _) = the_error(&src, rel, lang);
            assert!(d.notes.iter().any(|n| *n == format!("{example}{share} = 50%")), "{rel}: {:?}", d.notes);
            let input = input_of(behind(&d, said).unwrap_or_else(|| panic!("{rel}: no input behind the example: {:?}", d.notes)));
            assert_eq!(input.len(), 4, "{rel}: {input:?}");
            let at: Vec<i128> = scores.iter().map(|s| num(&input[*s])).collect();
            let given: Vec<(&str, i128)> = scores.iter().copied().zip(at.iter().copied()).collect();
            let (fired, b) = evaluate(&src, rel, &given);
            assert!(fired.is_empty(), "{rel}: a row took {given:?}: {fired:?}");
            assert_eq!(b.get(share), Some(&Val::Num(Rat::new(1, 2))), "{rel}: {given:?}");
            named.push(at);
        }
    }
    assert!(named.windows(2).all(|w| w[0] == w[1]), "the versions named different scores: {named:?}");
}

/// The input named is the first the walk meets, so it is the same on every run and in both
/// versions: the walk goes through the inputs in the order the rule declares them, the first
/// moving fastest, each from the bottom of its range up — the order `grid::exhaust_input` shares
/// with the other exhaustive looks.
#[test]
fn the_walk_names_the_first_input_it_meets() {
    let (rel, scores, _) = RATIO[0];
    let src = source(rel);
    let first = (0..=10i128)
        .flat_map(|s| (0..=10i128).flat_map(move |p| (0..=10i128).flat_map(move |d| (0..=10i128).map(move |q| [q, d, p, s]))))
        .find(|x| 2 * x[0] + x[1] + x[2] + x[3] == 25)
        .unwrap();
    assert_eq!(first, [10, 5, 0, 0]);
    for _ in 0..3 {
        let (d, _) = the_error(&src, rel, Lang::En);
        let input = input_of(behind(&d, EN).unwrap());
        assert_eq!(scores.map(|s| num(&input[s])), first, "{rel}");
    }
}

/// The input behind the mutants' gap is one the generated code stops on, and one score less or more
/// moves it into a band: the code answers there.
#[test]
fn the_generated_code_stops_on_the_input_behind_a_define() {
    if !ready(Need::Python, have_python, "python3 is not here") {
        return;
    }
    let tmp = TempDir::new("behind-define");
    // The function and the enum each version's code has.
    for (k, ((rel, scores, _), (module, grade))) in RATIO.into_iter().zip([("rating_grade", "Rank"), ("rank", "Grade")]).enumerate() {
        let src = source(rel);
        let (d, _) = the_error(&src, rel, Lang::En);
        let input = input_of(behind(&d, EN).unwrap());
        let [q, dl, p, s] = scores.map(|n| num(&input[n]));
        let call = |dl: i128| format!("{module}(quality={q}, delivery={dl}, price={p}, support={s})");
        let said = ask_python(&tmp, &format!("{k}"), &src, rel, module, &[call(dl - 1), call(dl), call(dl + 5)]);
        assert_eq!(said, format!("{} {grade}.C\n{} {STOPPED}\n{} {grade}.B\n", call(dl - 1), call(dl), call(dl + 5)), "{rel}");
    }
}

/// What the reference evaluator gives for inputs of any type: the rows that took them, and every
/// value it worked out.
fn evaluate_vals(src: &str, rel: &str, input: &[(&str, Val)]) -> (Vec<String>, HashMap<String, Val>) {
    let (f, c) = rulec::prepare(src, rel).expect("the rule reads");
    let env: HashMap<String, Val> = input.iter().map(|(n, v)| (n.to_string(), v.clone())).collect();
    let (_, fired, b) = rulec::eval::run_all(&f, &c, env);
    (fired, b)
}

/// A gap on what a table above gives is a gap on a value no caller sends either: `size` comes from
/// the girth. The English rule the README shows (`m_e101en.rule`, the parcel tariff with its
/// overseas small parcels left out) and the Japanese rule the golden holds (`m_e101.rule`, Japan
/// Post's tariff with 山梨県 left out of its group) both name the input behind the example. The
/// reference evaluator gives the column the example's value there and takes no row, and the code
/// made without the check stops on it and answers for a girth on either side.
#[test]
fn a_gap_on_a_table_above_names_the_input_behind_it() {
    let en_rel = "tests/mutants/m_e101en.rule";
    let src = source(en_rel);
    let (d, _) = the_error(&src, en_rel, Lang::En);
    assert!(d.notes.iter().any(|n| n == "An input that matches no row: dest = overseas, size = small, weight = 1lb"), "{:?}", d.notes);
    assert_eq!(behind(&d, EN), Some("dest = overseas, girth = 23, signature = true, weight = 1"));
    let (fired, b) = evaluate_vals(
        &src,
        en_rel,
        &[("dest", Val::Enum("overseas".into())), ("girth", Val::Num(Rat::int(23))), ("signature", Val::Bool(true)), ("weight", Val::Num(Rat::int(1)))],
    );
    assert_eq!(b.get("size"), Some(&Val::Enum("small".into())));
    assert!(!fired.iter().any(|r| r.contains("base_rate")), "a row of base_rate took it: {fired:?}");

    let ja_rel = "tests/mutants/m_e101.rule";
    let ja = source(ja_rel);
    let (d, _) = the_error(&ja, ja_rel, Lang::Ja);
    assert!(d.notes.iter().any(|n| n == "当てはまらない例: あて先 = 山梨県, サイズ = S60"), "{:?}", d.notes);
    assert_eq!(behind(&d, JA), Some("あて先 = 山梨県, 三辺合計 = 1, 重量 = 1"));
    let (fired, b) = evaluate_vals(&ja, ja_rel, &[("あて先", Val::Enum("山梨県".into())), ("三辺合計", Val::Num(Rat::int(1))), ("重量", Val::Num(Rat::int(1)))]);
    assert_eq!(b.get("サイズ"), Some(&Val::Enum("S60".into())));
    assert!(!fired.iter().any(|r| r.contains("運賃表")), "a row of 運賃表 took it: {fired:?}");

    if !ready(Need::Python, have_python, "python3 is not here") {
        return;
    }
    let tmp = TempDir::new("behind-above");
    let call = |girth: i128| format!("parcel_rate(1, {girth}, m.Zone.OVERSEAS, True)");
    let said = ask_python(&tmp, "parcel", &src, en_rel, "parcel_rate", &[call(22), call(23), call(61)]);
    let lines: Vec<&str> = said.lines().collect();
    assert_eq!(lines.len(), 3, "{said}");
    assert_eq!(lines[1], format!("{} {STOPPED}", call(23)), "{said}");
    assert!(!lines[0].contains("stopped") && !lines[2].contains("stopped"), "{said}");
}

/// An input the table reads as a column of its own is not walked: it stays at the value the example
/// shows, and only the inputs behind the computed column move.
#[test]
fn an_input_the_table_reads_keeps_the_example_value() {
    let src = "rule tiered_band v1\n\nenum level = regular | gold\n\ninputs\n  tier   : level\n  amount : money[GBP]  range >=1GBP <=100GBP\n\noutputs\n  small : bool\n\nderive low : money[GBP] = min(amount, amount * 10%)  range >=0GBP <=100GBP\n\ntable band\npolicy unique\n| tier    | low    | -> small : bool |\n| gold    | -      | true            |\n| regular | <=1GBP | true            |\n| regular | >=2GBP | false           |\n";
    let (d, _) = the_error(src, "tiered_band.rule", Lang::En);
    assert!(d.notes.iter().any(|n| n == "An input that matches no row: tier = regular, low = 1.1GBP"), "{:?}", d.notes);
    assert_eq!(behind(&d, EN), Some("amount = 11, tier = regular"));
}

/// A `constraint` holds of the input named. Walking the cap from the bottom, the first cap that lets
/// the tenth of 11GBP through (2GBP) is below the amount, which the constraint does not allow, and
/// the walk goes on to the first cap that holds both.
#[test]
fn the_input_named_keeps_the_constraints() {
    let src = "rule capped_band v1\n\ninputs\n  amount : money[GBP]  range >=1GBP <=100GBP\n  cap    : money[GBP]  range >=0GBP <=100GBP\n\nconstraint amount <= cap\n\noutputs\n  small : bool\n\nderive low : money[GBP] = min(amount * 10%, cap)  range >=0GBP <=100GBP\n\ntable band\npolicy unique\n| low    | -> small : bool |\n| <=1GBP | true            |\n| >=2GBP | false           |\n";
    let (d, _) = the_error(src, "capped_band.rule", Lang::En);
    assert_eq!(behind(&d, EN), Some("amount = 11, cap = 11"));
    let (fired, b) = evaluate(src, "capped_band.rule", &[("amount", 11), ("cap", 11)]);
    assert!(fired.is_empty(), "{fired:?}");
    assert_eq!(b.get("low"), Some(&Val::Num(Rat::new(11, 10))));
}

/// The walk has a budget, 2^14 combinations. Two inputs of 128 values each are walked, and the
/// larger of them names the input behind 11; one value more on one of them is past the budget, and
/// E101 stands as it did, with no input named.
#[test]
fn past_the_budget_no_input_is_named() {
    let rule = |top: u32| {
        format!(
            "rule larger v1\n\ninputs\n  a : number  range >=0 <=127\n  b : number  range >=0 <={top}\n\noutputs\n  high : bool\n\nderive peak : number = max(a, b)  range >=0 <={top}\n\ntable t\npolicy unique\n| peak | -> high : bool |\n| <=10 | false          |\n| >=12 | true           |\n"
        )
    };
    let (d, _) = the_error(&rule(127), "larger.rule", Lang::En);
    assert!(d.notes.iter().any(|n| n == "An input that matches no row: peak = 11"), "{:?}", d.notes);
    assert_eq!(behind(&d, EN), Some("a = 11, b = 0"));
    let (d, _) = the_error(&rule(128), "larger.rule", Lang::En);
    assert!(d.notes.iter().any(|n| n == "An input that matches no row: peak = 11"), "{:?}", d.notes);
    assert_eq!(behind(&d, EN), None, "{:?}", d.notes);
}

/// A gap no input reaches is no gap (§15.195). A `define` of a share asked here for a row at 310/3%,
/// past the 100% the share reaches, while E101 did not read what a `define` can come to (§15.189,
/// what was found); the walk tried all 31 totals, none made it, and nothing was named. E101 now
/// reads the share's reach, so the table that ends at 100% is complete, and no total makes a share
/// past it. Where the gap lies inside the reach, the input behind it is named as on any column.
#[test]
fn a_share_past_what_a_define_reaches_is_no_gap() {
    let src = "rule share v1\n\ninputs\n  total : number  range >=0 <=30\n\noutputs\n  full : bool\n\ndefine ratio : rate = total / 30\n\ntable t\npolicy unique\n| ratio | -> full : bool |\n| <100% | false          |\n| 100%  | true           |\n";
    assert!(errors(src, "share.rule", Lang::En).is_empty(), "{:?}", errors(src, "share.rule", Lang::En));
    let past = (0..=30).any(|t| {
        let (_, b) = evaluate(src, "share.rule", &[("total", t)]);
        matches!(b.get("ratio"), Some(Val::Num(v)) if v.cmp_to(Rat::int(1)) == std::cmp::Ordering::Greater)
    });
    assert!(!past, "a total makes a share past 100%");
    // A gap inside the reach: the share of a half, which a total of 15 makes.
    let gap = src.replace("| <100% | false          |", "| <50%  | false          |");
    let (d, _) = the_error(&gap, "share.rule", Lang::En);
    assert!(d.notes.iter().any(|n| n == "An input that matches no row: ratio = 50%"), "{:?}", d.notes);
    assert_eq!(behind(&d, EN), Some("total = 15"));
}
