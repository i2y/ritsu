//! The values a value worked out on a step takes (§15.323). `gifts × 2` is even and `gifts × 2 + 1`
//! odd: such a value never stands on half the points one step apart. The coverage audit asked for
//! the pair of `<4` over `gifts × 2` at 3 and 4, which no input gives, and said it missing for good,
//! while generated code with the boundary moved down to 2 passed `rulec test`. A `define` whose
//! range holds none of the values it takes (`gifts × 2 + 1  range >=2 <=2`) passed the check, and
//! the generated code refused every call. The audit now asks for the values the column takes
//! nearest the boundary on each side, the vectors stand on them, and such a range is E150.
//!
//! The materials: `tests/stepped/` — sheets of wrapping paper, two to a gift, over a number of the
//! call (`wrapping`) and over the lines of an order with one more for the card (`wrapped_lines`), a
//! boundary written between two values the sheets take (`between_steps`), a row for a number of
//! sheets they never are (`odd_cell`, which the check shows no input reaches, §15.325), a row for a
//! number of sheets on the step that no input gives and the check does not decide
//! (`square_rooms`), and a range of two sheets for an odd number of them (`never_even`) — in
//! English, with each one's Japanese version beside it. A value rounded to a grid takes the points
//! of the grid: for that, the fee in steps of 10 yen of `tests/computed/`.

use ritsu_testkit::run::finish;
use ritsu_testkit::{Need, TempDir, ready};
use std::path::{Path, PathBuf};
use std::process::Command;

/// How long a run of `rulec test` may take: it waits for the generated code without a limit of
/// its own.
const RULEC_LIMIT: std::time::Duration = std::time::Duration::from_secs(10 * 60);

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/stepped")
}

fn run_in(dir: &Path, lang: &str, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", lang).current_dir(dir).args(args).args(["--lang", lang]).output().expect("cannot start rulec");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// The materials whose pairs stand on the values taken, English first, each with its Japanese
/// version, the comparison of the generated Python that writes the boundary, and the boundaries it
/// is moved to: one value taken down and one up, where the answer of some call changes.
const MATERIALS: &[(&str, &str, &str, &[i64])] = &[
    ("wrapping", "包装紙", "sheets < 4:", &[2, 6]),
    ("wrapped_lines", "包む明細", "sheets < 6:", &[4, 8]),
    ("between_steps", "刻みの間の境", "sheets < 5:", &[3, 7]),
];

/// Every pair of the audit is met, in English and in Japanese, on the values the sheets take. Before,
/// each of these left both pairs missing: one point of each was a value the sheets never are.
#[test]
fn the_pairs_stand_on_the_values_taken() {
    for (en, ja, _, _) in MATERIALS {
        let (c, out, err) = run_in(&dir(), "en", &["check", &format!("{en}.rule")]);
        assert_eq!(c, 0, "{en}:\n{out}{err}");
        let (c, out, err) = run_in(&dir(), "en", &["coverage", &format!("{en}.rule")]);
        assert_eq!(c, 0, "{en}:\n{out}{err}");
        assert!(out.contains("boundary-pair coverage") && !out.contains("missing"), "{en}:\n{out}");
        let (c, out, err) = run_in(&dir(), "ja", &["coverage", &format!("{ja}.rule")]);
        assert_eq!(c, 0, "{ja}:\n{out}{err}");
        assert!(out.contains("境界の両側カバー") && !out.contains("欠け"), "{ja}:\n{out}");
    }
    // The cases of each pair: two gifts and one for the boundary of four sheets (2 and 4 sheets), two
    // and three wrapped lines for six (5 and 7), and two and three gifts for five (4 and 6).
    #[rustfmt::skip]
    let cases: &[(&str, &str, &[&str])] = &[
        ("wrapping.rule", "en", &[
            r#"{"in":{"gifts":1},"out":{"large_roll":false},"trace":["table roll_of row 1"],"why":"boundary pair: table roll_of row 1 sheets 4 inside"}"#,
            r#"{"in":{"gifts":2},"out":{"large_roll":true},"trace":["table roll_of row 2"],"why":"boundary pair: table roll_of row 1 sheets 4 outside"}"#,
        ]),
        ("包装紙.rule", "ja", &[
            r#"{"in":{"贈り物の数":1},"out":{"大きなロール":false},"trace":["表 ロールの判定 行1"],"why":"境界両側: 表 ロールの判定 行1 枚数 4 の内側"}"#,
        ]),
        ("between_steps.rule", "en", &[
            r#"{"in":{"gifts":2},"out":{"large_roll":false},"trace":["table roll_of row 1"],"why":"boundary pair: table roll_of row 1 sheets 5 inside"}"#,
            r#"{"in":{"gifts":3},"out":{"large_roll":true},"trace":["table roll_of row 2"],"why":"boundary pair: table roll_of row 1 sheets 5 outside"}"#,
        ]),
    ];
    for (file, lang, lines) in cases {
        let (c, out, err) = run_in(&dir(), lang, &["vectors", file]);
        assert_eq!(c, 0, "{file}:\n{out}{err}");
        for line in *lines {
            assert!(out.lines().any(|l| l == *line), "{file}: no line `{line}`:\n{out}");
        }
    }
    // Over the lines of an order: two and three wrapped lines, five and seven sheets.
    let (c, out, err) = run_in(&dir(), "en", &["vectors", "wrapped_lines.rule"]);
    assert_eq!(c, 0, "{out}{err}");
    let gifts = |l: &str| l.split(r#""out""#).next().unwrap_or_default().matches(r#""gift":true"#).count();
    for n in [2, 3] {
        assert!(out.lines().any(|l| gifts(l) == n), "no case of {n} wrapped lines:\n{out}");
    }
}

/// A value rounded to a grid takes only the points of the grid: a fee of 10% of the amount rounded
/// up to 10 yen is 10, 20 or 30 yen, so the pairs of its rows are 10 and 20 yen, and 20 and 30 yen,
/// and the cases the rule has already meet them. Before, the audit asked for 11, 19, 21 and 29 yen,
/// which no amount comes to, and said four pairs missing. The material is the fee of the computed
/// columns (`tests/computed/shapes/`), in English and in Japanese.
#[test]
fn a_value_rounded_to_a_grid_is_paired_on_the_grid() {
    let shapes = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/computed/shapes");
    for (file, lang, said) in [
        ("fee_in_steps.rule", "en", &["boundary-pair", "coverage", "4", "/", "4", "satisfied"][..]),
        ("十円刻みの手数料だけを読む表.rule", "ja", &["境界の両側カバー", "4", "/", "4", "満たす"][..]),
    ] {
        let (c, out, err) = run_in(&shapes, lang, &["coverage", file]);
        assert_eq!(c, 0, "{file}:\n{out}{err}");
        assert!(out.lines().any(|l| l.split_whitespace().eq(said.iter().copied())), "{file}: no `{}`:\n{out}", said.join(" "));
    }
}

/// A cell that names a value off the step the column takes is a row no input reaches, and the
/// check shows it (§15.325): three sheets are never reached, since the sheets come in twos, so the
/// row is E102 with the values the sheets take. Before, the check left the row undecided (W128)
/// and passed the rule.
#[test]
fn a_cell_off_the_step_is_a_row_no_input_reaches() {
    for (file, lang, said) in [
        ("odd_cell.rule", "en", ["error[E102]: Unreachable row: row 1 never matches", "`sheets` only comes to multiples of 2, and this row asks for none of them."]),
        ("三枚の行.rule", "ja", ["エラー[E102]: 行1 はどの入力にも当てはまりません", "`枚数` は 2 の倍数しかとらず、この行が求める値になりません。"]),
    ] {
        let (c, out, err) = run_in(&dir(), lang, &["check", file]);
        assert_eq!(c, 1, "{file}:\n{out}{err}");
        for s in said {
            assert!(out.contains(s) || err.contains(s), "{file}: no `{s}`:\n{out}{err}");
        }
    }
}

/// A cell that names a value on the step the column never takes keeps the point it names: no two
/// whole squares add up to seven, so fourteen sheets are never reached, the check cannot show it
/// (W128) and passes the rule, and the row and its pairs stay missing, not met by twelve sheets
/// and sixteen.
#[test]
fn a_cell_on_a_value_never_taken_stays_missing() {
    for (file, lang, said) in [
        ("square_rooms.rule", "en", ["table note_of row 1 column sheets boundary 14 (pair at 14 / 12)", "table note_of row 1 column sheets boundary 14 (pair at 14 / 16)"]),
        ("正方形の部屋.rule", "ja", ["表 注記の判定 行1 列 枚数 の境界 14（14 と 12 の対）", "表 注記の判定 行1 列 枚数 の境界 14（14 と 16 の対）"]),
    ] {
        let (c, out, err) = run_in(&dir(), lang, &["check", file]);
        assert_eq!(c, 0, "{file}:\n{out}{err}");
        assert!(out.contains("W128") || err.contains("W128"), "{file}:\n{out}{err}");
        let (c, out, err) = run_in(&dir(), lang, &["coverage", file]);
        assert_eq!(c, 1, "{file}:\n{out}{err}");
        for s in said {
            assert!(out.contains(s), "{file}: no `{s}`:\n{out}");
        }
    }
}

/// A range that holds none of the values the definition takes is E150, in English and in Japanese,
/// with the values it takes and the two nearest the range; a range that holds one passes.
#[test]
fn a_range_that_holds_no_value_taken_is_e150() {
    for (file, lang, said) in [
        (
            "never_even.rule",
            "en",
            &[
                "error[E150]: The range of the definition does not meet the values it can reach",
                "^^^^^^^^^^^^^ sheets takes only 1 plus a multiple of 2",
                "Every value sheets takes is 1 plus a whole multiple of 2 (1, 3, 5, …), on any input, and none of them lies in this range (>=2 <=2).",
                "Hint: write ends that hold a value sheets takes: 1 or 3, say.",
            ][..],
        ),
        (
            "偶数にならない枚数.rule",
            "ja",
            &[
                "エラー[E150]: 定義の範囲が、その値の届く範囲と重なりません",
                "枚数 が取るのは、2 の倍数に 1 を足した値だけです",
                "枚数 の値は、どの入力でも 2 の倍数に 1 を足したもの（1、3、5、…）です。この範囲（>=2 <=2）には、その値が一つもありません。",
                "ヒント: 範囲の端を、枚数 の取る値（1 や 3）が入るように直してください。",
            ][..],
        ),
    ] {
        let (c, out, err) = run_in(&dir(), lang, &["check", file]);
        assert_eq!(c, 1, "{file}:\n{out}{err}");
        for s in said {
            assert!(out.contains(s), "{file}: no `{s}`:\n{out}");
        }
        let (c, out, _) = run_in(&dir(), "en", &["check", file, "--format", "json"]);
        assert_eq!(c, 1);
        assert!(out.lines().any(|l| l.contains("\"code\":\"E150\"") && l.contains("\"severity\":\"error\"")), "{file}:\n{out}");
    }
    let t = TempDir::new("stepped-range");
    let src = std::fs::read_to_string(dir().join("never_even.rule")).unwrap();
    assert_eq!(src.matches("range >=2 <=2").count(), 1);
    std::fs::write(t.path().join("odd_range.rule"), src.replace("range >=2 <=2", "range >=2 <=3")).unwrap();
    let (c, out, err) = run_in(t.path(), "en", &["check", "odd_range.rule"]);
    assert_eq!(c, 0, "{out}{err}");
}

/// `rulec test` passes the generated Python as it is written, and stops it with the boundary moved
/// to the next value the sheets take either way. Before, the move down passed: no case stood on the
/// value below the boundary.
#[test]
fn rulec_test_stops_a_boundary_moved_one_value_taken() {
    if !ready(Need::Python, || Command::new("python3").arg("--version").output().is_ok_and(|o| o.status.success()), "python3 is not here") {
        return;
    }
    let t = TempDir::new("stepped-moved");
    let tmp = TempDir::new("stepped-moved-tmp");
    let test = |out: &Path| -> (i32, String) {
        let run = finish(
            Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").env("TMPDIR", tmp.path()).arg("test").arg(out),
            RULEC_LIMIT,
            "rulec test running the generated code",
        );
        (run.code.unwrap_or(-1), run.stdout.clone() + &run.stderr)
    };
    for (en, ja, line, moves) in MATERIALS {
        for (name, module) in [(*en, en.to_string()), (*ja, format!("{en}_ja"))] {
            let out = t.path().join(&module);
            let o = out.to_string_lossy().into_owned();
            let (c, said, err) = run_in(&dir(), "en", &["gen", &format!("{name}.rule"), "--target", "python", "--extra", "runner", "--extra", "vectors", "--out", &o]);
            assert_eq!(c, 0, "{name}:\n{said}{err}");
            let (c, said) = test(&out);
            assert!(c == 0 && said.contains(&format!("ok    {module} (Python)")), "{name} as written: {said}");
            let py = out.join("python").join(format!("{module}.py"));
            let body = std::fs::read_to_string(&py).unwrap();
            assert_eq!(body.matches(&format!(" {line}  #")).count(), 1, "{name}: `{line}`:\n{body}");
            let head = line.rsplit_once(' ').unwrap().0;
            for moved in *moves {
                std::fs::write(&py, body.replace(&format!(" {line}  #"), &format!(" {head} {moved}:  #"))).unwrap();
                let (c, said) = test(&out);
                assert!(c != 0 && said.contains("FAIL"), "{name}: a boundary written `{head} {moved}:` passed: {said}");
            }
            std::fs::write(&py, &body).unwrap();
        }
    }
}
