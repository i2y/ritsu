//! A value worked out once a walk is over from two summaries that both have to move (§15.323).
//! `cover = highest * 2 + lowest` — twice the price of the dearest line, and the price of the
//! cheapest line wrapped as a gift — comes to 9,999 yen, and to 99, only with both moved: the
//! dearest line down, and a wrapped line put in. Each try at a sequence had to bring every summary
//! that moved where the point had it, and each moved one of them: the boundary of 10,000 yen had
//! no case on 9,999 yen, and the range of `cover` no case one yen under its bottom. `rulec coverage`
//! named the pair missing, and `rulec test` passed generated code whose door let 99 yen in, or
//! whose table moved the boundary to 9,999 yen. The summaries are now brought there one after
//! another, and a line the summary looks at is put in where the sequence has none.
//!
//! A try for one summary can take another along: a wrapped line put in for a wrapped price adds to
//! the total. `cover = total * 2 + dearest_gift` comes to its boundary only with the total, which a
//! wrapped line took along, brought back where the point has it. `cover = total * 3 +
//! cheapest_gift` comes to either side of 9,999 yen only with the cheapest wrapped price brought
//! there first and the total after it, the other way round from the order written. And `cover =
//! dearest_gift + total * 2 + dearest_plain * 2`, with every line at most 1,000 yen, comes to
//! either side of 10,000 yen only with a line of the sequence made a wrapped one, at the price the
//! dearest wrapped line has to have. Each boundary had no case on either side.
//!
//! The materials: `tests/two_summaries/` — `top_cover`, `gift_cover`, `parcel_cover` (the same
//! with the dearest price of all the lines, which the value does not read), `least_gift_cover` and
//! `both_dearest_cover`, in English, with each one's Japanese version beside it.

use ritsu_testkit::run::finish;
use ritsu_testkit::{Need, TempDir, ready};
use std::path::{Path, PathBuf};
use std::process::Command;

/// How long a run of `rulec test` may take: it waits for the generated code without a limit of
/// its own.
const RULEC_LIMIT: std::time::Duration = std::time::Duration::from_secs(10 * 60);

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/two_summaries")
}

fn run_in(dir: &Path, lang: &str, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", lang).current_dir(dir).args(args).args(["--lang", lang]).output().expect("cannot start rulec");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// The two versions: the file, the language, the module the generated code is, the names the
/// lines travel under (the sequence, the price, the gift) and the reasons of the cases on and
/// past the ends of the range of `cover`.
const VERSIONS: &[(&str, &str, &str, (&str, &str, &str), [&str; 4])] = &[
    (
        "top_cover.rule",
        "en",
        "top_cover",
        ("lines", "price", "gift"),
        [
            "the definition cover at the bottom of the range it accepts",
            "the definition cover at the top of the range it accepts",
            "the definition cover past the bottom of the range it accepts",
            "the definition cover past the top of the range it accepts",
        ],
    ),
    (
        "補償額.rule",
        "ja",
        "top_cover_ja",
        ("明細", "価格", "贈り物"),
        ["定義 補償額 を、受け付ける範囲の下端に", "定義 補償額 を、受け付ける範囲の上端に", "定義 補償額 を、受け付ける範囲の下端の先に", "定義 補償額 を、受け付ける範囲の上端の先に"],
    ),
];

/// What a case is insured for, from its lines: twice the dearest price, and the cheapest price of
/// a line wrapped as a gift (0 yen where none is).
fn cover(j: &rulec::json::Json, (seq, price, gift): (&str, &str, &str)) -> i128 {
    let lines = j.get("in").and_then(|i| i.get(seq)).and_then(|l| l.as_arr()).unwrap_or_default();
    let p = |l: &rulec::json::Json| l.get(price).and_then(|p| p.as_int()).unwrap_or(0);
    let highest = lines.iter().map(p).max().unwrap_or(0);
    let lowest = lines.iter().filter(|l| l.get(gift).and_then(|g| g.as_bool()) == Some(true)).map(p).min().unwrap_or(0);
    highest * 2 + lowest
}

/// Both rows have a case and the boundary between them its pair, on 9,999 yen and 10,000; the
/// range of `cover` has a case on each end and a refused one a yen past each. Before, the pair
/// had no case on 9,999 yen and the bottom none on 99.
#[test]
fn each_end_and_boundary_has_its_cases() {
    let t = TempDir::new("two-summaries");
    for (file, lang, module, keys, said) in VERSIONS {
        let (c, out, err) = run_in(&dir(), lang, &["check", file]);
        assert_eq!(c, 0, "{file}:\n{out}{err}");
        let (c, out, err) = run_in(&dir(), lang, &["coverage", file]);
        assert_eq!(c, 0, "{file}:\n{out}{err}");
        let lines: Vec<Vec<&str>> = out.lines().map(|l| l.split_whitespace().collect()).collect();
        let (rows, pairs): (&[&str], &[&str]) = if *lang == "en" {
            (&["row", "coverage", "2", "/", "2", "satisfied"], &["boundary-pair", "coverage", "2", "/", "2", "satisfied"])
        } else {
            (&["行カバー", "2", "/", "2", "満たす"], &["境界の両側カバー", "2", "/", "2", "満たす"])
        };
        assert!(lines.iter().any(|l| l == rows) && lines.iter().any(|l| l == pairs), "{file}:\n{out}");
        let o = t.path().join(module);
        let (c, out, err) = run_in(&dir(), lang, &["gen", file, "--target", "python", "--extra", "vectors", "--out", &o.to_string_lossy()]);
        assert_eq!(c, 0, "{file}:\n{out}{err}");
        let read = |name: &str| -> Vec<rulec::json::Json> {
            std::fs::read_to_string(o.join("vectors").join(name)).unwrap().lines().map(|l| rulec::json::parse(l).unwrap_or_else(|x| panic!("{x}: {l}"))).collect()
        };
        let why = |j: &rulec::json::Json| j.get("why").and_then(|w| w.as_str()).unwrap_or_default().to_string();
        let answered = read(&format!("{module}.jsonl"));
        let refused = read(&format!("{module}.refused.jsonl"));
        for at in [100, 9999, 10000, 60000] {
            assert!(answered.iter().any(|j| cover(j, *keys) == at), "{file}: no case insured for {at}: {answered:?}");
        }
        for (reason, at) in [(said[0], 100), (said[1], 60000)] {
            let on: Vec<i128> = answered.iter().filter(|j| why(j) == reason).map(|j| cover(j, *keys)).collect();
            assert_eq!(on, [at], "{file}: {reason}: {answered:?}");
        }
        let error = if *lang == "en" { "cover is out of range" } else { "補償額 が範囲の外です" };
        let under: Vec<(i128, String)> = refused.iter().filter(|j| why(j) == said[2]).map(|j| (cover(j, *keys), j.get("error").and_then(|x| x.as_str()).unwrap_or_default().to_string())).collect();
        assert_eq!(under, [(99, error.to_string())], "{file}: {refused:?}");
        let over: Vec<i128> = refused.iter().filter(|j| why(j) == said[3]).map(|j| cover(j, *keys)).collect();
        assert!(over.len() == 1 && over[0] > 60000, "{file}: {refused:?}");
    }
}

/// `rulec test` passes the generated Python as it is written, and stops it with the bottom of the
/// door a yen off either way, or the boundary of the table moved a yen either way. Before, a door
/// that let 99 yen in passed, and so did a boundary moved down to 9,999 yen.
#[test]
fn rulec_test_stops_a_door_or_a_boundary_a_yen_off() {
    if !ready(Need::Python, || Command::new("python3").arg("--version").output().is_ok_and(|o| o.status.success()), "python3 is not here") {
        return;
    }
    let t = TempDir::new("two-summaries-moved");
    let tmp = TempDir::new("two-summaries-moved-tmp");
    let test = |out: &Path| -> (i32, String) {
        let run = finish(
            Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").env("TMPDIR", tmp.path()).arg("test").arg(out),
            RULEC_LIMIT,
            "rulec test running the generated code",
        );
        (run.code.unwrap_or(-1), run.stdout.clone() + &run.stderr)
    };
    for (file, _, module, _, _) in VERSIONS {
        let out = t.path().join(module);
        let (c, said, err) = run_in(&dir(), "en", &["gen", file, "--target", "python", "--extra", "runner", "--extra", "vectors", "--out", &out.to_string_lossy()]);
        assert_eq!(c, 0, "{file}:\n{said}{err}");
        let (c, said) = test(&out);
        assert!(c == 0 && said.contains(&format!("ok    {module} (Python)")), "{file} as written: {said}");
        let py = out.join("python").join(format!("{module}.py"));
        let body = std::fs::read_to_string(&py).unwrap();
        for (written, moved) in [
            ("    if not 100 <= cover <= 60000:\n", ["    if not 99 <= cover <= 60000:\n", "    if not 101 <= cover <= 60000:\n"]),
            ("    if cover < 10000:", ["    if cover < 9999:", "    if cover < 10001:"]),
        ] {
            assert_eq!(body.matches(written).count(), 1, "{file}: `{}`:\n{body}", written.trim());
            for m in moved {
                std::fs::write(&py, body.replace(written, m)).unwrap();
                let (c, said) = test(&out);
                assert!(c != 0 && said.contains("FAIL"), "{file}: `{}` passed: {said}", m.trim());
            }
        }
        std::fs::write(&py, &body).unwrap();
    }
}

/// A summary a try took along is brought back: `gift_cover` has the pair on 9,999 yen and 10,000
/// yen, in English and in Japanese, and so does `parcel_cover`, where the dearest price of all the
/// lines, which `cover` does not read, is left where the tries take it. `rulec test` stops the generated Python with the boundary moved a yen
/// either way.
#[test]
fn a_summary_a_try_took_along_is_brought_back() {
    // What a case is insured for: twice the total of its lines, and the dearest price of a line
    // wrapped as a gift (0 yen where none is).
    let cover = |j: &rulec::json::Json, (seq, price, gift): (&str, &str, &str)| -> i128 {
        let lines = j.get("in").and_then(|i| i.get(seq)).and_then(|l| l.as_arr()).unwrap_or_default();
        let p = |l: &rulec::json::Json| l.get(price).and_then(|p| p.as_int()).unwrap_or(0);
        let total: i128 = lines.iter().map(p).sum();
        let dearest = lines.iter().filter(|l| l.get(gift).and_then(|g| g.as_bool()) == Some(true)).map(p).max().unwrap_or(0);
        total * 2 + dearest
    };
    let (en, ja) = (("lines", "price", "gift"), ("明細", "価格", "贈り物"));
    for (file, lang, keys, n) in [
        ("gift_cover.rule", "en", en, "2"),
        ("贈り物の補償額.rule", "ja", ja, "2"),
        ("parcel_cover.rule", "en", en, "4"),
        ("小包の補償額.rule", "ja", ja, "4"),
    ] {
        let (c, out, err) = run_in(&dir(), lang, &["check", file]);
        assert_eq!(c, 0, "{file}:\n{out}{err}");
        let (c, out, err) = run_in(&dir(), lang, &["coverage", file]);
        assert_eq!(c, 0, "{file}:\n{out}{err}");
        let pairs: Vec<&str> = if lang == "en" { vec!["boundary-pair", "coverage", n, "/", n, "satisfied"] } else { vec!["境界の両側カバー", n, "/", n, "満たす"] };
        assert!(out.lines().any(|l| l.split_whitespace().collect::<Vec<_>>() == pairs), "{file}:\n{out}");
        let (c, out, err) = run_in(&dir(), lang, &["vectors", file]);
        assert_eq!(c, 0, "{file}:\n{out}{err}");
        let cases: Vec<rulec::json::Json> = out.lines().map(|l| rulec::json::parse(l).unwrap_or_else(|x| panic!("{x}: {l}"))).collect();
        for at in [9999, 10000] {
            assert!(cases.iter().any(|j| cover(j, keys) == at), "{file}: no case insured for {at}:\n{out}");
        }
    }
    if !ready(Need::Python, || Command::new("python3").arg("--version").output().is_ok_and(|o| o.status.success()), "python3 is not here") {
        return;
    }
    let t = TempDir::new("two-summaries-order");
    let tmp = TempDir::new("two-summaries-order-tmp");
    let test = |out: &Path| -> (i32, String) {
        let run = finish(
            Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").env("TMPDIR", tmp.path()).arg("test").arg(out),
            RULEC_LIMIT,
            "rulec test running the generated code",
        );
        (run.code.unwrap_or(-1), run.stdout.clone() + &run.stderr)
    };
    for (file, module) in [("gift_cover.rule", "gift_cover"), ("贈り物の補償額.rule", "gift_cover_ja"), ("parcel_cover.rule", "parcel_cover"), ("小包の補償額.rule", "parcel_cover_ja")] {
        let out = t.path().join(module);
        let (c, said, err) = run_in(&dir(), "en", &["gen", file, "--target", "python", "--extra", "runner", "--extra", "vectors", "--out", &out.to_string_lossy()]);
        assert_eq!(c, 0, "{file}:\n{said}{err}");
        let (c, said) = test(&out);
        assert!(c == 0 && said.contains(&format!("ok    {module} (Python)")), "{file} as written: {said}");
        let py = out.join("python").join(format!("{module}.py"));
        let body = std::fs::read_to_string(&py).unwrap();
        let written = "    if cover < 10000:";
        assert_eq!(body.matches(written).count(), 1, "{file}: `{written}`:\n{body}");
        for moved in ["    if cover < 9999:", "    if cover < 10001:"] {
            std::fs::write(&py, body.replace(written, moved)).unwrap();
            let (c, said) = test(&out);
            assert!(c != 0 && said.contains("FAIL"), "{file}: `{}` passed: {said}", moved.trim());
        }
        std::fs::write(&py, &body).unwrap();
    }
}

/// Three times the total of the lines, and the price of the cheapest line wrapped as a gift (0 yen
/// where none is).
fn least_cover(j: &rulec::json::Json, (seq, price, gift): (&str, &str, &str)) -> i128 {
    let lines = j.get("in").and_then(|i| i.get(seq)).and_then(|l| l.as_arr()).unwrap_or_default();
    let p = |l: &rulec::json::Json| l.get(price).and_then(|p| p.as_int()).unwrap_or(0);
    let total: i128 = lines.iter().map(p).sum();
    let cheapest = lines.iter().filter(|l| l.get(gift).and_then(|g| g.as_bool()) == Some(true)).map(p).min().unwrap_or(0);
    total * 3 + cheapest
}

/// The price of the dearest line wrapped as a gift, twice the total of the lines, and twice the
/// price of the dearest line not wrapped (0 yen where there is none).
fn both_cover(j: &rulec::json::Json, (seq, price, gift): (&str, &str, &str)) -> i128 {
    let lines = j.get("in").and_then(|i| i.get(seq)).and_then(|l| l.as_arr()).unwrap_or_default();
    let p = |l: &rulec::json::Json| l.get(price).and_then(|p| p.as_int()).unwrap_or(0);
    let total: i128 = lines.iter().map(p).sum();
    let dearest = |wrapped: bool| lines.iter().filter(|l| l.get(gift).and_then(|g| g.as_bool()) == Some(wrapped)).map(p).max().unwrap_or(0);
    dearest(true) + total * 2 + dearest(false) * 2
}

/// Two points only one way reaches: `least_gift_cover` has the pair on 9,998 yen and 9,999 only
/// with the cheapest wrapped price brought there before the total, the other way round from the
/// order written, and `both_dearest_cover` the pair on 9,999 yen and 10,000 only with a line of the
/// sequence made a wrapped one at the price the dearest wrapped line has to have; in English and in
/// Japanese. `rulec test` stops the generated Python with the boundary moved a yen either way.
/// Before, neither boundary had a case on either side.
#[test]
fn points_reached_only_the_other_way_round_or_by_wrapping_a_line() {
    type Cover = fn(&rulec::json::Json, (&str, &str, &str)) -> i128;
    let (en, ja) = (("lines", "price", "gift"), ("明細", "価格", "贈り物"));
    let versions: [(&str, &str, &str, (&str, &str, &str), Cover, i128); 4] = [
        ("least_gift_cover.rule", "en", "least_gift_cover", en, least_cover, 9999),
        ("包む品の最安値の補償額.rule", "ja", "least_gift_cover_ja", ja, least_cover, 9999),
        ("both_dearest_cover.rule", "en", "both_dearest_cover", en, both_cover, 10000),
        ("二つの最高値の補償額.rule", "ja", "both_dearest_cover_ja", ja, both_cover, 10000),
    ];
    for (file, lang, _, keys, cover, at) in versions {
        let (c, out, err) = run_in(&dir(), lang, &["check", file]);
        assert_eq!(c, 0, "{file}:\n{out}{err}");
        let (c, out, err) = run_in(&dir(), lang, &["coverage", file]);
        assert_eq!(c, 0, "{file}:\n{out}{err}");
        let pairs: Vec<&str> = if lang == "en" { vec!["boundary-pair", "coverage", "2", "/", "2", "satisfied"] } else { vec!["境界の両側カバー", "2", "/", "2", "満たす"] };
        assert!(out.lines().any(|l| l.split_whitespace().collect::<Vec<_>>() == pairs), "{file}:\n{out}");
        let (c, out, err) = run_in(&dir(), lang, &["vectors", file]);
        assert_eq!(c, 0, "{file}:\n{out}{err}");
        let cases: Vec<rulec::json::Json> = out.lines().map(|l| rulec::json::parse(l).unwrap_or_else(|x| panic!("{x}: {l}"))).collect();
        for side in [at - 1, at] {
            assert!(cases.iter().any(|j| cover(j, keys) == side), "{file}: no case insured for {side}:\n{out}");
        }
    }
    if !ready(Need::Python, || Command::new("python3").arg("--version").output().is_ok_and(|o| o.status.success()), "python3 is not here") {
        return;
    }
    let t = TempDir::new("two-summaries-one-way");
    let tmp = TempDir::new("two-summaries-one-way-tmp");
    let test = |out: &Path| -> (i32, String) {
        let run = finish(
            Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").env("TMPDIR", tmp.path()).arg("test").arg(out),
            RULEC_LIMIT,
            "rulec test running the generated code",
        );
        (run.code.unwrap_or(-1), run.stdout.clone() + &run.stderr)
    };
    for (file, _, module, _, _, at) in versions {
        let out = t.path().join(module);
        let (c, said, err) = run_in(&dir(), "en", &["gen", file, "--target", "python", "--extra", "runner", "--extra", "vectors", "--out", &out.to_string_lossy()]);
        assert_eq!(c, 0, "{file}:\n{said}{err}");
        let (c, said) = test(&out);
        assert!(c == 0 && said.contains(&format!("ok    {module} (Python)")), "{file} as written: {said}");
        let py = out.join("python").join(format!("{module}.py"));
        let body = std::fs::read_to_string(&py).unwrap();
        let written = format!("    if cover < {at}:");
        assert_eq!(body.matches(&written).count(), 1, "{file}: `{}`:\n{body}", written.trim());
        for moved in [at - 1, at + 1] {
            let moved = format!("    if cover < {moved}:");
            std::fs::write(&py, body.replace(&written, &moved)).unwrap();
            let (c, said) = test(&out);
            assert!(c != 0 && said.contains("FAIL"), "{file}: `{}` passed: {said}", moved.trim());
        }
        std::fs::write(&py, &body).unwrap();
    }
}
