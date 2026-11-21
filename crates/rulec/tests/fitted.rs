//! The vectors of a walk whose `count` or `sum` has a bottom (§15.323). The vectors aim a count at
//! the boundaries of the tables that read it with copies of one element, a total with as few
//! elements as carry it, a `max` or a `min` with one element, and they move the values of one call
//! with each sequence at rest, one element long. Where a summary over the sequence has a bottom — a
//! total held to 200 yen, a count held to two lines — such a sequence comes in under it, and the
//! walk refuses it: a boundary was left with no case, and a rule with inputs beside its lines had no
//! answered case at all. `rulec coverage` named them missing, and `rulec test` passed generated code
//! with a boundary moved one step. The cases are now brought inside the summary's range, with what
//! they were aimed at kept.
//!
//! The materials: `tests/fitted/` — a count read beside a total's bottom (`gift_box`), a total
//! beside another total's bottom (`weighed_order`), a `max` beside a count's bottom
//! (`dearest_of_two`), a `max` and a `min` beside a total's bottom (`dearest_line`,
//! `cheapest_line`: the lines put beside them stay under the largest value and over the smallest),
//! a total beside a count's bottom (`three_or_more`), a table over the values of the call beside
//! a count's bottom (`coupon_order`), a count of the lines of one kind, which no table reads,
//! among three tables beside a total's bottom (`toy_box`: no line was ever a toy), and six tables
//! over five counts and a total of units beside a total's bottom (`packing`: the cases that stand
//! on a boundary came after more cases than are brought inside, and each held every count and the
//! units where they were, which left no room for the line the total needs) — in English, with each
//! one's Japanese version beside it.

use ritsu_testkit::run::finish;
use ritsu_testkit::{Need, TempDir, ready};
use std::path::{Path, PathBuf};
use std::process::Command;

/// How long a run of `rulec test` may take: it waits for the generated code without a limit of
/// its own.
const RULEC_LIMIT: std::time::Duration = std::time::Duration::from_secs(10 * 60);

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fitted")
}

fn run_in(dir: &Path, lang: &str, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", lang).current_dir(dir).args(args).args(["--lang", lang]).output().expect("cannot start rulec");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// The materials, English first, each with its Japanese version and the comparisons of the
/// generated Python that write a boundary of a table.
const MATERIALS: &[(&str, &str, &[&str])] = &[
    ("gift_box", "贈り物の箱", &["total < 250:", "gifts < 2:"]),
    ("weighed_order", "重さのある注文", &["total < 150:", "load < 100:"]),
    ("dearest_of_two", "二件からの最高値", &["dearest < 50:"]),
    ("dearest_line", "いちばん高い品", &["dearest < 50:"]),
    ("cheapest_line", "いちばん安い品", &["cheapest <= 80:"]),
    ("three_or_more", "三件からの注文", &["total < 150:"]),
    ("coupon_order", "クーポンのある注文", &["dearest < 50:", "coupon < 300:", "coupon < 600:"]),
    ("toy_box", "おもちゃの箱", &["toys < 2:", "units < 10:", "gifts < 3:"]),
    ("packing", "荷造り", &["toys < 3:", "books < 2:", "units < 20:", "gifts < 4:", "rushes < 2:", "colds < 3:"]),
];

/// Every criterion of the audit is met, in English and in Japanese. Before, each of these left a
/// boundary pair missing, four of them a row with no case, and `coupon_order` had no vector.
#[test]
fn every_criterion_is_met_beside_a_bottom() {
    for (en, ja, _) in MATERIALS {
        let (c, out, err) = run_in(&dir(), "en", &["check", &format!("{en}.rule")]);
        assert_eq!(c, 0, "{en}:\n{out}{err}");
        let (c, out, err) = run_in(&dir(), "en", &["coverage", &format!("{en}.rule")]);
        assert_eq!(c, 0, "{en}:\n{out}{err}");
        assert!(out.contains("boundary-pair coverage") && !out.contains("missing"), "{en}:\n{out}");
        let (c, out, err) = run_in(&dir(), "ja", &["coverage", &format!("{ja}.rule")]);
        assert_eq!(c, 0, "{ja}:\n{out}{err}");
        assert!(out.contains("境界の両側カバー") && !out.contains("欠け"), "{ja}:\n{out}");
    }
}

/// The cases on each boundary, and why each is there: what the case stands on, kept, and the
/// summary brought inside its range. A `max` is aimed with a line beside it that does not take its
/// place, which also brings the count of lines to its bottom.
#[test]
fn the_cases_keep_what_they_were_aimed_at() {
    #[rustfmt::skip]
    let cases: &[(&str, &str, &[(&str, &str)])] = &[
        // The call at rest: the one gift of the pool's first line, with a line no count takes beside
        // it to bring the total to its bottom. It stands on the count's boundary.
        ("gift_box.rule", "en", &[
            (r#"{"in":{"lines":[{"price":100,"gift":true},{"price":100,"gift":false}]},"out":{"free":false,"card":false},"#, r#""why":"baseline"}"#),
        ]),
        ("weighed_order.rule", "en", &[
            (r#"{"in":{"lines":[{"price":100,"weight":19},{"price":50,"weight":1}]},"out":{"large":true,"heavy":false},"#, r#""why":"total = 150 kept, load brought inside its range"}"#),
            (r#"{"in":{"lines":[{"price":100,"weight":19},{"price":49,"weight":1}]},"out":{"large":false,"heavy":false},"#, r#""why":"total = 149 kept, load brought inside its range"}"#),
        ]),
        ("dearest_of_two.rule", "en", &[
            (r#"{"in":{"lines":[{"price":49,"shipped":true},{"price":1,"shipped":true}]},"out":{"pricey":false},"#, r#""why":"dearest at 49, beside an element that does not take its place"}"#),
            (r#"{"in":{"lines":[{"price":50,"shipped":true},{"price":1,"shipped":true}]},"out":{"pricey":true},"#, r#""why":"dearest at 50, beside an element that does not take its place"}"#),
        ]),
        // The lines put beside the largest value are not over it, and those beside the smallest
        // not under it: 3 yen more after three lines of 49 yen, 80 yen more after the 80 yen line
        // and a line of 100 yen, where 70 yen would have been the total's due.
        ("dearest_line.rule", "en", &[
            (r#"{"in":{"lines":[{"price":49},{"price":49},{"price":49},{"price":3}]},"out":{"pricey":false},"#, r#""why":"dearest = 49 kept, total brought inside its range"}"#),
            (r#"{"in":{"lines":[{"price":50},{"price":50},{"price":50}]},"out":{"pricey":true},"#, r#""why":"dearest = 50 kept, total brought inside its range"}"#),
        ]),
        ("cheapest_line.rule", "en", &[
            (r#"{"in":{"lines":[{"price":80},{"price":100},{"price":80}]},"out":{"cheap":true},"#, r#""why":"cheapest = 80 kept, total brought inside its range"}"#),
            (r#"{"in":{"lines":[{"price":81},{"price":100},{"price":81}]},"out":{"cheap":false},"#, r#""why":"cheapest = 81 kept, total brought inside its range"}"#),
        ]),
        ("three_or_more.rule", "en", &[
            (r#"{"in":{"lines":[{"price":99,"paid":true},{"price":50,"paid":true},{"price":1,"paid":true}]},"out":{"large":true},"#, r#""why":"total = 150 kept, paid_lines brought inside its range"}"#),
            (r#"{"in":{"lines":[{"price":99,"paid":true},{"price":49,"paid":true},{"price":1,"paid":true}]},"out":{"large":false},"#, r#""why":"total = 149 kept, paid_lines brought inside its range"}"#),
        ]),
        // The values of the call move with two shipped lines, which the walk answers: before, they
        // moved with one, and every case was refused. The rebate's tie is looked for with them too.
        ("coupon_order.rule", "en", &[
            (r#"{"in":{"lines":[{"price":1,"shipped":true},{"price":1,"shipped":true}],"member":false,"region":"north","coupon":299},"out":{"rebate":0,"pricey":false,"fee":500},"#, r#""why":"boundary pair: table fee_of row 2 coupon 300JPY inside"}"#),
            (r#"{"in":{"lines":[{"price":1,"shipped":true},{"price":1,"shipped":true}],"member":false,"region":"north","coupon":300},"out":{"rebate":0,"pricey":false,"fee":300},"#, r#""why":"boundary pair: table fee_of row 2 coupon 300JPY outside"}"#),
            (r#"{"in":{"lines":[{"price":1,"shipped":true},{"price":1,"shipped":true}],"member":true,"region":"north","coupon":100},"out":{"rebate":2,"pricey":false,"fee":0},"#, r#""why":"rounding tie: rebate"}"#),
        ]),
        ("クーポンのある注文.rule", "ja", &[
            (r#"{"in":{"明細":[{"価格":1,"出荷":true},{"価格":1,"出荷":true}],"会員":false,"地域区分":"北","クーポン":299},"out":{"還元額":0,"高い品":false,"手数料":500},"#, r#""why":"境界両側: 表 手数料の表 行2 クーポン 300円 の内側"}"#),
            (r#"{"in":{"明細":[{"価格":1,"出荷":true},{"価格":1,"出荷":true}],"会員":true,"地域区分":"北","クーポン":100},"out":{"還元額":2,"高い品":false,"手数料":0},"#, r#""why":"丸めの同着: 還元額"}"#),
        ]),
        // Two toys: the kind of a line is a candidate the count of toys asks for. What a case is
        // held to is what it stands on a boundary with: the units of the case of two toys move, and
        // those of the case of ten units do not.
        ("toy_box.rule", "en", &[
            (r#"{"in":{"lines":[{"price":100,"qty":1,"kind":"toy","gift":true},{"price":100,"qty":1,"kind":"toy","gift":true}]},"out":{"toy_box":true,"bulky":false,"wrapped":false},"#, r#""why":"toys = 2, gifts = 2 kept, total brought inside its range"}"#),
            (r#"{"in":{"lines":[{"price":100,"qty":9,"kind":"food","gift":true},{"price":100,"qty":1,"kind":"food","gift":true}]},"out":{"toy_box":false,"bulky":true,"wrapped":false},"#, r#""why":"gifts = 2, units = 10 kept, total brought inside its range"}"#),
        ]),
        ("おもちゃの箱.rule", "ja", &[
            (r#"{"in":{"明細":[{"価格":100,"数量":1,"品目":"おもちゃ","贈り物":true},{"価格":100,"数量":1,"品目":"おもちゃ","贈り物":true}]},"out":{"おもちゃ箱":true,"大きな箱":false,"包む箱":false},"#, r#""why":"おもちゃの数 = 2、贈り物の数 = 2 はそのままで、金額 を範囲の中に戻した"}"#),
        ]),
        // Three toys, and with them three gifts and three cold lines, each on a boundary.
        ("packing.rule", "en", &[
            (r#"{"in":{"lines":[{"price":100,"qty":1,"kind":"toy","gift":true,"rush":true,"cold":true},{"price":100,"qty":1,"kind":"toy","gift":true,"rush":true,"cold":true},{"price":100,"qty":1,"kind":"toy","gift":true,"rush":true,"cold":true}]},"out":{"toy_box":true,"shelf":false,"bulky":false,"wrapped":false,"fast":true,"chilled":true},"#, r#""why":"toys = 3, gifts = 3, colds = 3 kept, total brought inside its range"}"#),
        ]),
        ("贈り物の箱.rule", "ja", &[
            (r#"{"in":{"明細":[{"価格":100,"贈り物":true},{"価格":100,"贈り物":false}]},"out":{"送料無料":false,"カード":false},"#, r#""why":"基準"}"#),
        ]),
        ("二件からの最高値.rule", "ja", &[
            (r#"{"in":{"明細":[{"価格":49,"出荷":true},{"価格":1,"出荷":true}]},"out":{"高い品":false},"#, r#""why":"最高値 が 49 で、そこに届かない要素も並ぶ"}"#),
        ]),
    ];
    for (file, lang, lines) in cases {
        let (c, out, err) = run_in(&dir(), lang, &["vectors", file]);
        assert_eq!(c, 0, "{file}:\n{out}{err}");
        for (input, why) in *lines {
            assert!(out.lines().any(|l| l.starts_with(input) && l.ends_with(why)), "{file}: no line `{input}…{why}`:\n{out}");
        }
    }
}

/// `rulec test` passes the generated Python as it is written, and stops it with any boundary of a
/// table moved one step either way. Before, the moves toward the summary that has the bottom
/// passed: no case stood on that side.
#[test]
fn rulec_test_stops_a_boundary_moved_one_step() {
    if !ready(Need::Python, || Command::new("python3").arg("--version").output().is_ok_and(|o| o.status.success()), "python3 is not here") {
        return;
    }
    let t = TempDir::new("fitted-moved");
    let tmp = TempDir::new("fitted-moved-tmp");
    let test = |out: &Path| -> (i32, String) {
        let run = finish(
            Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").env("TMPDIR", tmp.path()).arg("test").arg(out),
            RULEC_LIMIT,
            "rulec test running the generated code",
        );
        (run.code.unwrap_or(-1), run.stdout.clone() + &run.stderr)
    };
    for (en, ja, lines) in MATERIALS {
        for (name, module) in [(*en, en.to_string()), (*ja, format!("{en}_ja"))] {
            let out = t.path().join(&module);
            let o = out.to_string_lossy().into_owned();
            let (c, said, err) = run_in(&dir(), "en", &["gen", &format!("{name}.rule"), "--target", "python", "--extra", "runner", "--extra", "vectors", "--out", &o]);
            assert_eq!(c, 0, "{name}:\n{said}{err}");
            let (c, said) = test(&out);
            assert!(c == 0 && said.contains(&format!("ok    {module} (Python)")), "{name} as written: {said}");
            let py = out.join("python").join(format!("{module}.py"));
            let body = std::fs::read_to_string(&py).unwrap();
            for line in *lines {
                let (head, n) = line.rsplit_once(' ').unwrap();
                let n: i64 = n.trim_end_matches(':').parse().unwrap();
                assert_eq!(body.matches(&format!(" {line}  #")).count(), 1, "{name}: `{line}`:\n{body}");
                for moved in [n - 1, n + 1] {
                    std::fs::write(&py, body.replace(&format!(" {line}  #"), &format!(" {head} {moved}:  #"))).unwrap();
                    let (c, said) = test(&out);
                    assert!(c != 0 && said.contains("FAIL"), "{name}: a boundary written `{head} {moved}:` passed: {said}");
                }
            }
            // The rebate rounded the other way at its tie: the tie is looked for with two lines,
            // which the walk answers.
            if *en == "coupon_order" {
                let (written, other) = ("(_round_half(raw, 200) // 200)", "(_round_half_down(raw, 200) // 200)");
                assert_eq!(body.matches(written).count(), 1, "{name}:\n{body}");
                std::fs::write(&py, body.replace(written, other)).unwrap();
                let (c, said) = test(&out);
                assert!(c != 0 && said.contains("FAIL"), "{name}: the rebate rounded half down passed: {said}");
            }
            std::fs::write(&py, &body).unwrap();
        }
    }
}
