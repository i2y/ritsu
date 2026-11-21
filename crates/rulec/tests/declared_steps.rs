//! A step written on a rate the rule works with and does not answer (§15.323). The heading of a
//! table (`-> markup : rate[step 1%]`) and the type of a `derive` or a `define` wrote a step that
//! held nothing: a cell of `0.5%` under a heading of whole percents passed the check, and so did a
//! cell naming a rate that comes in half percents and a definition of half a markup. The rule
//! worked on with the value between two steps: `rulec run` priced an item of 100 USD with a markup
//! of 0.5% at 101 USD. The cells, and the values a name in one of them or the expression of such a
//! definition takes, are now held to the step, as the cells of an input's column are (E114).
//!
//! The materials: `tests/declared_steps/` — a cell of 0.5% (`markup`), a cell naming a partner's
//! rate in half percents (`partner_markup`) and half of a markup by kind (`half_markup`), in
//! English, with each one's Japanese version beside it.

use std::path::{Path, PathBuf};
use std::process::Command;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/declared_steps")
}

fn run_in(dir: &Path, lang: &str, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", lang).current_dir(dir).args(args).output().expect("cannot start rulec");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// Each material, what `check` says of it, and the text that puts it on the step.
#[rustfmt::skip]
const MATERIALS: &[(&str, &str, &[&str], (&str, &str))] = &[
    ("markup.rule", "en", &[
        "error[E114]: `0.5%` does not sit on this column's step",
        "^^^^ the step is 1%",
        "Write a value on the step, such as `1%`, or declare a finer step on the heading.",
    ], ("| books     | 0.5%                      |", "| books     | 1%                        |")),
    ("上乗せ率.rule", "ja", &[
        "エラー[E114]: `0.5%` はこの列の刻みに載っていません",
        "^^^^ 刻みは 1% です",
        "`1%` のように刻みに載る値に直すか、見出しの刻みを細かくしてください。",
    ], ("| 本       | 0.5%                                |", "| 本       | 1%                                  |")),
    ("partner_markup.rule", "en", &[
        "error[E114]: `partner_rate` does not always sit on this column's step",
        "^^^^^^^^^^^^ the step is 1%, and partner_rate can be 0.5%",
        "Hint: write a value on the step, or declare on the heading the step partner_rate takes (`rate[step 0.5%]`).",
    ], ("rate[step 0.5%]", "rate[step 1%]")),
    ("取引先の上乗せ率.rule", "ja", &[
        "エラー[E114]: `取引先の率` は、この列の刻みに載らないことがあります",
        "^^^^^^^^^^ 刻みは 1% ですが、取引先の率 は 0.5% にもなります",
        "ヒント: 刻みに載る値を書くか、見出しの刻みを 取引先の率 の取る刻み（`rate[step 0.5%]`）にしてください。",
    ], ("rate[step 0.5%]", "rate[step 1%]")),
    ("half_markup.rule", "en", &[
        "error[E114]: The value of `sale_markup` does not always sit on its step",
        "^^^^^^^^^^^ the step is 1%, and sale_markup can be 2.5%",
        "Hint: round the expression to the step (`half_up(…, 1%)`, in the direction you choose), or declare the step sale_markup takes (`rate[step 2.5%]`).",
    ], ("= markup / 2", "= half_up(markup / 2, 1%)")),
    ("半分の上乗せ率.rule", "ja", &[
        "エラー[E114]: `セールの上乗せ率` の値は、宣言した刻みに載らないことがあります",
        "^^^^^^^^^^^^^^^^ 刻みは 1% ですが、セールの上乗せ率 は 2.5% にもなります",
        "ヒント: 式を刻みに丸めるか（`half_up(…, 1%)`。向きは選んでください）、セールの上乗せ率 の取る刻み（`rate[step 2.5%]`）を宣言してください。",
    ], ("= 上乗せ率 / 2", "= half_up(上乗せ率 / 2, 1%)")),
];

/// A value off the step a heading or a definition writes is E114, in English and in Japanese,
/// with where it is and what puts it on the step; in JSON too. Before, each of these passed.
#[test]
fn a_value_off_a_step_written_on_a_rate_the_rule_does_not_answer_is_e114() {
    for (file, lang, said, _) in MATERIALS {
        let (c, out, err) = run_in(&dir(), lang, &["check", file]);
        assert_eq!(c, 1, "{file}:\n{out}{err}");
        for s in *said {
            assert!(out.contains(s), "{file}: no `{s}`:\n{out}");
        }
        let (c, out, _) = run_in(&dir(), "en", &["check", file, "--format", "json"]);
        assert_eq!(c, 1);
        assert!(out.lines().any(|l| l.contains("\"code\":\"E114\"") && l.contains("\"severity\":\"error\"")), "{file}:\n{out}");
    }
    // Off the step by an offset: the hint names the step that holds every value, the one the
    // offset and the step share, and not the step alone, which holds none of them.
    let t = ritsu_testkit::TempDir::new("declared-steps-offset");
    let src = std::fs::read_to_string(dir().join("half_markup.rule")).unwrap();
    std::fs::write(t.path().join("offset.rule"), src.replace("= markup / 2", "= markup + 0.5%")).unwrap();
    let (c, out, err) = run_in(t.path(), "en", &["check", "offset.rule"]);
    assert_eq!(c, 1, "{out}{err}");
    for s in ["Every value its expression takes is 0.5% plus a whole multiple of", "(`rate[step 0.5%]`)"] {
        assert!(out.contains(s), "no `{s}`:\n{out}");
    }
}

/// The same rules with the value on the step pass: the cell written on it, the partner's rate in
/// whole percents, and half the markup rounded to the step. So does a definition whose expression
/// stays on the step: twice the markup, and the markup and one percent.
#[test]
fn the_same_rules_on_the_step_pass() {
    let t = ritsu_testkit::TempDir::new("declared-steps");
    for (file, lang, _, (from, to)) in MATERIALS {
        let src = std::fs::read_to_string(dir().join(file)).unwrap();
        assert_eq!(src.matches(from).count(), 1, "{file}: `{from}`");
        std::fs::write(t.path().join(file), src.replace(from, to)).unwrap();
        let (c, out, err) = run_in(t.path(), lang, &["check", file]);
        assert_eq!(c, 0, "{file} with `{to}`:\n{out}{err}");
    }
    let src = std::fs::read_to_string(dir().join("half_markup.rule")).unwrap();
    for (name, to) in [("twice.rule", "= markup * 2"), ("whole.rule", "= markup + 1%")] {
        std::fs::write(t.path().join(name), src.replace("= markup / 2", to)).unwrap();
        let (c, out, err) = run_in(t.path(), "en", &["check", name]);
        assert_eq!(c, 0, "{name}:\n{out}{err}");
    }
}
