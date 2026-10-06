//! The step a computed column's values sit on (DESIGN §15.190).
//!
//! A derive that takes a fraction of an amount — `amount * 10%` — comes in tenths of a pound, and
//! the axis of its column has to be cut in tenths too: cut in whole pounds, a value falls between
//! two coordinates and no row is asked for it. `tests/mutants/m_e101frac.rule` is such a rule,
//! written in whole-pound bands, and `m_e101fracja.rule` its Japanese version. `check` says E101,
//! with the value in between and the input behind it, and the code generated without the check
//! stops on that very input. The scale the generated code holds a computed value at is the other
//! half: a `min` of two sides and a value over an input with no range were read back at a scale
//! of one, and the code answered otherwise than the reference evaluator.

use ritsu_testkit::{Need, TempDir, ready};
use rulec::eval::Val;
use rulec::i18n::{self, Lang};
use rulec::num::Rat;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The English rule, its Japanese version, and the names each gives the amount and the commission.
const PAIR: [(&str, &str, &str); 2] = [
    ("tests/mutants/m_e101frac.rule", "amount", "commission"),
    ("tests/mutants/m_e101fracja.rule", "金額", "手数料"),
];

fn source(rel: &str) -> String {
    std::fs::read_to_string(root().join(rel)).unwrap_or_else(|_| panic!("cannot read {rel}"))
}

fn have_python() -> bool {
    Command::new("python3").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// The one error a rule's text gets, in `lang`, as JSON.
fn the_error(src: &str, rel: &str, lang: Lang) -> (rulec::diag::Diag, rulec::json::Json) {
    i18n::with(lang, || {
        let ds = rulec::check_source(src, rel);
        let errors: Vec<_> = ds.into_iter().filter(|d| d.severity == rulec::diag::Severity::Error).collect();
        assert_eq!(errors.iter().map(|d| d.code).collect::<Vec<_>>(), ["E101"], "{rel}");
        let json = rulec::json::parse(&rulec::diag::render_json(&errors[0], rel)).unwrap();
        (errors.into_iter().next().unwrap(), json)
    })
}

/// The input a note hands over, `name = value, …`, as numbers.
fn input_of(note: &str) -> Vec<(String, i128)> {
    note.split(", ")
        .map(|p| {
            let (n, v) = p.split_once(" = ").unwrap_or_else(|| panic!("not `name = value`: {p}"));
            (n.to_string(), v.parse().unwrap_or_else(|_| panic!("not a whole number: {v}")))
        })
        .collect()
}

/// The gap between `<=1GBP` and `>=2GBP` is E101: the value in it as the number it is, the input
/// behind it, and the row to add written in cells the column takes. The Japanese version says the
/// same on the same line.
#[test]
fn a_gap_between_whole_units_is_e101_with_the_input_behind_it() {
    let mut lines = Vec::new();
    for (rel, amount, commission) in PAIR {
        let src = source(rel);
        let unit = if amount == "amount" { "GBP" } else { "円" };
        for (lang, said) in [(Lang::En, "An input producing this example: "), (Lang::Ja, "この例を作る入力: ")] {
            let (d, json) = the_error(&src, rel, lang);
            lines.push(d.where_.clone());
            let witness = json.get("witness").and_then(|w| w.get("inputs")).and_then(|w| w.get(commission)).and_then(|v| v.as_str());
            assert_eq!(witness, Some("1.1"), "{rel}: {}", json.compact());
            let input = d.notes.iter().find_map(|n| n.strip_prefix(said)).unwrap_or_else(|| panic!("{rel}: no input behind the example: {:?}", d.notes));
            assert_eq!(input_of(input), [(amount.to_string(), 11)], "{rel}");
            let fix = json.get("fix").and_then(|f| f.get("text")).and_then(|t| t.as_str());
            assert_eq!(fix, Some(format!("| >1{unit} <2{unit} | false |").as_str()), "{rel}");
        }
    }
    // on the table's heading, line 12 of both
    assert!(lines.iter().all(|l| l.contains(".rule:12 ")), "{lines:?}");
}

/// The input `check` hands over is one the generated code stops on: the code made from the rule
/// without the check — what `rulec gen` wrote before §15.190 — raises its `unreachable` assertion
/// there, and the reference evaluator finds no row either. The row `check` suggests closes the
/// gap, and then the code answers.
#[test]
fn the_generated_code_stops_on_the_input_check_hands_over() {
    if !ready(Need::Python, have_python, "python3 is not here") {
        return;
    }
    let tmp = TempDir::new("steps-unreachable");
    for (k, (rel, amount, _)) in PAIR.into_iter().enumerate() {
        let src = source(rel);
        let (d, json) = the_error(&src, rel, Lang::En);
        let input = d.notes.iter().find_map(|n| n.strip_prefix("An input producing this example: ")).unwrap();
        let [(name, at)]: [(String, i128); 1] = input_of(input).try_into().unwrap_or_else(|_| panic!("{input}"));
        assert_eq!(name, amount);

        // The reference evaluator: no row of the table takes it, so no answer.
        let (f, c) = rulec::prepare(&src, rel).expect("the rule reads");
        let env: HashMap<String, Val> = [(name.clone(), Val::Num(Rat::int(at)))].into_iter().collect();
        let (outs, fired, _) = rulec::eval::run_all(&f, &c, env);
        assert!(fired.is_empty(), "{rel}: a row took it: {fired:?}");
        assert!(outs.iter().all(|(_, v)| v.is_none()), "{rel}: an answer came: {outs:?}");

        // The generated code: it stops there, and answers on either side.
        let ask = |src: &str, tag: &str, amounts: &[i128]| -> String {
            let (f, c) = rulec::prepare(src, rel).expect("the rule reads");
            let dir = tmp.path().join(format!("{k}-{tag}"));
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("commission_band.py"), rulec::codegen::Gen::new(&f, &c, src, rel).python()).unwrap();
            let calls: Vec<String> = amounts.iter().map(|a| format!("ask({a})")).collect();
            let script = format!(
                "import sys\nsys.path.insert(0, {dir:?})\nimport commission_band as m\ndef ask(a):\n    try:\n        print(a, m.commission_band(a))\n    except AssertionError as e:\n        print(a, 'stopped:', e)\n{}\n",
                calls.join("\n"),
                dir = dir.to_string_lossy()
            );
            let o = Command::new("python3").arg("-c").arg(script).output().expect("python3 runs");
            assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
            String::from_utf8_lossy(&o.stdout).into_owned()
        };
        let said = ask(&src, "as-written", &[10, at, 20]);
        assert_eq!(said, format!("10 False\n{at} stopped: unreachable: completeness was statically checked by rulec\n20 True\n"), "{rel}");

        // The row `check` suggests, added under the header, makes the rule pass and the code answer.
        let row = json.get("fix").and_then(|f| f.get("text")).and_then(|t| t.as_str()).unwrap().to_string();
        let mut lines: Vec<&str> = src.lines().collect();
        let head = lines.iter().position(|l| l.starts_with("| ") && l.contains("->")).unwrap();
        lines.insert(head + 1, &row);
        let fixed = format!("{}\n", lines.join("\n"));
        let ds = rulec::check_source(&fixed, rel);
        assert!(!rulec::has_error(&ds), "{rel} with {row}: {:?}", ds.iter().map(|d| d.code).collect::<Vec<_>>());
        assert_eq!(ask(&fixed, "fixed", &[at]), format!("{at} False\n"), "{rel}");
    }
}

/// The other side of the same axis: a row written between two whole pounds is reached — by every
/// amount from 11GBP to 19GBP — and is no longer called dead (it was E102, earlier rows covering
/// it, while the axis had no coordinate between 1GBP and 2GBP).
#[test]
fn a_row_between_whole_units_is_reached() {
    let src = source(PAIR[0].0).replace("| <=1GBP     | false              |\n", "| <=1GBP     | false              |\n| >1GBP <2GBP | false             |\n");
    assert_ne!(src, source(PAIR[0].0));
    let ds = rulec::check_source(&src, "between.rule");
    assert!(!rulec::has_error(&ds), "{:?}", ds.iter().map(|d| (d.code, d.title.clone())).collect::<Vec<_>>());
    let (f, c) = rulec::prepare(&src, "between.rule").unwrap();
    let row = i18n::with(Lang::En, || rulec::eval::row_tag("review", 2));
    let fired: Vec<i128> = (1..=100)
        .filter(|a| {
            let env: HashMap<String, Val> = [("amount".to_string(), Val::Num(Rat::int(*a)))].into_iter().collect();
            let (_, fired, _) = i18n::with(Lang::En, || rulec::eval::run_all(&f, &c, env));
            fired.contains(&row)
        })
        .collect();
    assert_eq!(fired, (11..=19).collect::<Vec<i128>>());
}

/// The scale the generated code holds a computed value at is the scale the rule reads it at
/// (§15.190). A `min` gives one of its two sides, so it sits at the scale both sides share, and a
/// value over an input with no range has a scale all the same. Both used to be read back as whole
/// pounds while the code held them in hundredths, and the code answered otherwise than the
/// reference evaluator for every amount up to 10GBP.
#[test]
fn the_generated_code_holds_a_computed_value_at_its_own_scale() {
    if !ready(Need::Python, have_python, "python3 is not here") {
        return;
    }
    let rules = [
        ("lowest", "rule lowest v1\n\ninputs\n  amount : money[GBP]  range >=1GBP <=100GBP\n\noutputs\n  small : bool\n\nderive low : money[GBP] = min(amount, amount * 10%)  range >=0GBP <=100GBP\n\ntable t\npolicy unique\n| low    | -> small : bool |\n| <=1GBP | true            |\n| >1GBP  | false           |\n"),
        ("tenths", "rule tenths v1\n\ninputs\n  amount : money[GBP]\n\noutputs\n  small : bool\n\nderive tenth : money[GBP] = amount * 10%\n\ntable t\npolicy unique\n| tenth  | -> small : bool |\n| <=1GBP | true            |\n| >1GBP  | false           |\n"),
    ];
    let tmp = TempDir::new("steps-scale");
    for (name, src) in rules {
        let rel = format!("{name}.rule");
        let ds = rulec::check_source(src, &rel);
        assert!(!rulec::has_error(&ds), "{name}: {:?}", ds.iter().map(|d| d.code).collect::<Vec<_>>());
        let (f, c) = rulec::prepare(src, &rel).unwrap();
        let derived = if name == "lowest" { "low" } else { "tenth" };
        assert_eq!(c.scales.get(derived), Some(&100), "{name}: the scale `check` keeps for {derived}");
        std::fs::write(tmp.path().join(format!("{name}.py")), rulec::codegen::Gen::new(&f, &c, src, &rel).python()).unwrap();
        let script = format!(
            "import sys\nsys.path.insert(0, {dir:?})\nimport {name} as m\nprint(' '.join(str(m.{name}(a)) for a in range(1, 101)))\n",
            dir = tmp.path().to_string_lossy()
        );
        let o = Command::new("python3").arg("-c").arg(script).output().expect("python3 runs");
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        let code: Vec<String> = String::from_utf8_lossy(&o.stdout).split_whitespace().map(String::from).collect();
        let reference: Vec<String> = (1..=100)
            .map(|a| {
                let env: HashMap<String, Val> = [("amount".to_string(), Val::Num(Rat::int(a)))].into_iter().collect();
                let (outs, _, _) = rulec::eval::run_all(&f, &c, env);
                match outs.iter().find(|(n, _)| n == "small").and_then(|(_, v)| v.clone()) {
                    Some(Val::Bool(true)) => "True".to_string(),
                    Some(Val::Bool(false)) => "False".to_string(),
                    other => panic!("{name}: {a}: {other:?}"),
                }
            })
            .collect();
        assert_eq!(code, reference, "{name}: the generated code and the reference evaluator");
        assert_eq!(reference.iter().filter(|v| *v == "True").count(), 10, "{name}: up to 10GBP the value is at most 1GBP");
    }
}

/// The certificate states the step a tenth of a pound sits on, and the scales it is worked out
/// from; `tools/recheck.py` is held to both in `tests/cert.rs`, and the Lean program in
/// `tests/lean.rs`.
#[test]
fn the_certificate_cuts_a_column_of_tenths_in_tenths() {
    let src = source(PAIR[0].0).replace("| >=2GBP     | true               |", "| >1GBP      | true               |");
    let tmp = TempDir::new("steps-cert");
    let p: &Path = &tmp.path().join("commission_band.rule");
    std::fs::write(p, &src).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).args(["certificate", p.to_str().unwrap()]).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stdout));
    let cert = rulec::json::parse(&String::from_utf8_lossy(&o.stdout)).unwrap();
    let axis = cert.get("tables").and_then(|t| t.as_arr()).and_then(|t| t[0].get("axes")).and_then(|a| a.as_arr()).map(|a| a[0].clone()).unwrap();
    assert_eq!(axis.get("step").and_then(|s| s.as_str()), Some("1/10"), "{}", axis.compact());
    let scales = cert.get("scales").unwrap();
    assert_eq!(scales.get("amount").and_then(|s| s.as_int()), Some(1));
    assert_eq!(scales.get("commission").and_then(|s| s.as_int()), Some(100));
}
