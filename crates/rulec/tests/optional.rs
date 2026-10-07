//! Optional inputs that are amounts, numbers, dates and truth values (§15.201).
//!
//! An optional input takes one value more than the type it wraps: none at all, which the cell
//! `none` tests. An enum's axis always had a coordinate for it. A number's, a date's and a truth
//! value's did not, and their cells were read in `T?` itself, where no number is a value: a
//! comparison took every coordinate and `none` took none. So `due : date?` with the one row
//! `<2026-07-01` passed `check`, and the generated code fell into `unreachable` on 2026-08-01;
//! and `none`, `<5`, `>=5` was called an overlap (E105) with a row no input reaches (E102). The
//! range of an optional input was not read either, so no generated code guarded it.
//!
//! The materials are in `tests/optional`: `parcel_cover.rule`, which writes a row for the absent
//! value in each of four tables and covers the declared range beside it, and eight mutants that
//! each say one finding about the absent value — English first, with a Japanese version beside
//! each. What `rulec check` prints for a mutant, in English and in Japanese, is its golden file in
//! `tests/golden/optional` (`RULEC_BLESS=1 cargo test --test optional` writes them again).

use ritsu_testkit::tmp::tmpdir_in;
use ritsu_testkit::{Need, TempDir, ready, skip};
use rulec::eval::Val;
use rulec::i18n::{self, Lang};
use rulec::json::Json;
use rulec::num::Rat;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The mutants: the English rule, its Japanese version, the code each says once, and what the
/// English report names — the input of E101's gap and of E105's overlap, the row of E102.
const MUTANTS: &[(&str, &str, &str, &str, &str)] = &[
    ("E101_a_date_with_one_row", "E101_一行だけの日付", "E101", "due = none", "期日 = none"),
    ("E101_a_number_with_one_row", "E101_一行だけの数", "E101", "bonus = none", "加点 = none"),
    ("E101_a_date_short_of_rows_beside_none", "E101_値の無い行のほかが足りない日付", "E101", "due = 2026-07-01", "期日 = 2026-07-01"),
    ("E101_a_number_short_of_rows_beside_none", "E101_値の無い行のほかが足りない数", "E101", "bonus = 5", "加点 = 5"),
    ("E105_none_and_a_row_for_every_date", "E105_値の無い行と全部を取る行が重なる日付", "E105", "due = none", "期日 = none"),
    ("E105_none_and_not_five", "E105_値の無い行と5でない行が重なる数", "E105", "bonus = none", "加点 = none"),
    ("E102_none_after_a_row_for_every_number", "E102_全部を取る行のあとの値の無い行", "E102", "row 2 never matches", "row 2 never matches"),
    ("E102_none_after_not_july_first", "E102_7月1日でない行のあとの値の無い行", "E102", "row 2 never matches", "row 2 never matches"),
];

/// The rule that passes, and its Japanese version.
const PASSING: [&str; 2] = ["parcel_cover", "小包の補償"];

fn source(stem: &str) -> (String, String) {
    let rel = format!("tests/optional/{stem}.rule");
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

fn rulec(lang: &str, tmp: Option<&Path>, args: &[&str]) -> (i32, String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_rulec"));
    c.env("RULEC_LANG", lang).current_dir(root()).args(args);
    if let Some(t) = tmp {
        c.env("TMPDIR", t);
    }
    let o = c.output().expect("cannot start rulec");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// Whether a command answers. swiftc leaves an empty directory in its TMPDIR almost every time it
/// answers `--version`, so the probe gets a TMPDIR of its own, which goes when this returns.
fn have(cmd: &str, args: &[&str]) -> bool {
    let tmp = TempDir::new("optional-probe");
    Command::new(cmd).args(args).env("TMPDIR", tmp.path()).output().map(|o| o.status.success()).unwrap_or(false)
}

/// Each mutant says its one finding, about the absent value or beside it, and prints what its
/// golden file holds in both languages. Before §15.201 the two E101 mutants with one row passed,
/// and `none`, `<5`, `>=5` was an E105 with an E102.
#[test]
fn each_mutant_says_its_finding_about_the_absent_value() {
    let mut names: Vec<String> = std::fs::read_dir(root().join("tests/optional")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    let mut listed: Vec<String> =
        MUTANTS.iter().flat_map(|(en, ja, ..)| [format!("{en}.rule"), format!("{ja}.rule")]).chain(PASSING.iter().map(|s| format!("{s}.rule"))).collect();
    listed.sort();
    assert_eq!(names, listed, "every rule of tests/optional is listed here, with its Japanese version");
    let mut failures = Vec::new();
    for (en, ja, code, said_en, said_ja) in MUTANTS {
        for (stem, said) in [(en, said_en), (ja, said_ja)] {
            let (rel, src) = source(stem);
            let ds = i18n::with(Lang::En, || rulec::check_source(&src, &rel));
            let errors: Vec<_> = ds.iter().filter(|d| d.severity == rulec::diag::Severity::Error).map(|d| d.code).collect();
            if errors != [*code] {
                failures.push(format!("{rel}: {errors:?}, not one {code}"));
                continue;
            }
            let out = printed(&rel, &src, Lang::En);
            if !out.contains(said) {
                failures.push(format!("{rel} does not say `{said}`:\n{out}"));
            }
            for (lang, tag) in [(Lang::En, "en"), (Lang::Ja, "ja")] {
                let out = printed(&rel, &src, lang);
                if let Err(e) = ritsu_testkit::golden::check(&root().join(format!("tests/golden/optional/{stem}.{tag}.txt")), &out) {
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
    for (en, ja, ..) in MUTANTS {
        let (re, se) = source(en);
        let (rj, sj) = source(ja);
        assert_eq!(found(&se, &re), found(&sj, &rj), "{en} and {ja}");
    }
    let (re, se) = source(PASSING[0]);
    let (rj, sj) = source(PASSING[1]);
    assert_eq!(found(&se, &re), found(&sj, &rj));
}

/// A table that writes a row for the absent value and covers the declared range beside it passes,
/// and so do the two shapes that were called an overlap and a row no input reaches.
#[test]
fn a_row_for_none_beside_the_whole_range_passes() {
    for stem in PASSING {
        let (rel, src) = source(stem);
        let ds = i18n::with(Lang::En, || rulec::check_source(&src, &rel));
        assert!(ds.is_empty(), "{rel}: {:?}", ds.iter().map(|d| (d.code, d.title.clone())).collect::<Vec<_>>());
    }
    for (ty, rows) in [
        ("number?", ["none", "<5", ">=5"]),
        ("date?  range >=2026-01-01 <=2026-12-31", ["none", "<2026-07-01", ">=2026-07-01"]),
        ("rate[step 1%]?  range >=0% <=100%", ["none", "<50%", ">=50%"]),
    ] {
        let src = format!(
            "rule shape v1\ndescription \"one optional column\"\n\ninputs\n  x : {ty}\n\noutputs\n  y : bool\n\ntable pick\npolicy unique\n| x | -> y : bool |\n{}",
            rows.iter().map(|r| format!("| {r} | false |\n")).collect::<String>()
        );
        let ds = i18n::with(Lang::En, || rulec::check_source(&src, "shape.rule"));
        assert!(ds.is_empty(), "{ty}: {:?}", ds.iter().map(|d| (d.code, d.title.clone())).collect::<Vec<_>>());
    }
}

/// The checker's word against the reference evaluator's: every value an input of the passing rule
/// takes — the absent value, every count and every day of the year, and every amount at and
/// around each boundary — is taken by exactly one row of each table.
#[test]
fn every_value_is_taken_by_exactly_one_row() {
    for stem in PASSING {
        let (rel, src) = source(stem);
        let (f, c) = rulec::prepare(&src, &rel).unwrap_or_else(|_| panic!("{rel} does not read"));
        for it in &f.items {
            let rulec::ast::Item::Table(t) = it else { continue };
            let (col, _) = &t.inputs[0];
            let ty = c.ty_of(col).unwrap();
            let mut values = vec![Val::Enum("none".into())];
            match ty.present() {
                rulec::types::Ty::Bool => values.extend([Val::Bool(true), Val::Bool(false)]),
                rulec::types::Ty::Date => {
                    let (lo, hi) = c.ranges[col];
                    for d in lo.unwrap().num..=hi.unwrap().num {
                        let (y, m, dd) = rulec::types::ord_to_date(Rat::int(d));
                        values.push(Val::Date(y, m, dd));
                    }
                }
                _ => {
                    let (lo, hi) = c.ranges[col];
                    let (lo, hi) = (lo.unwrap().num, hi.unwrap().num);
                    let mut xs: Vec<i128> = if hi - lo <= 1000 { (lo..=hi).collect() } else { vec![lo, lo + 1, 29_999, 30_000, 30_001, 99_999, 100_000, 100_001, hi - 1, hi] };
                    xs.dedup();
                    values.extend(xs.into_iter().map(|x| Val::Num(Rat::int(x))));
                }
            }
            for v in &values {
                let taken = t.rows.iter().filter(|r| r.cells.first().is_none_or(|cell| rulec::eval::cell_matches(&c, cell, v, &ty))).count();
                assert_eq!(taken, 1, "{rel}: {col} = {v:?} is taken by {taken} rows");
            }
        }
    }
}

/// `rulec test` over a generated directory, as JSON, with a SKIP line for whatever it did not run.
fn rulec_test(out: &Path) -> Json {
    let (_, said, e) = rulec("en", Some(&tmpdir_in(out)), &["test", &out.to_string_lossy(), "--format", "json"]);
    let j = rulec::json::parse(said.lines().next().unwrap_or_else(|| panic!("no result\n{e}"))).unwrap_or_else(|x| panic!("{x}: {said}"));
    for why in j.get("skipped").and_then(|v| v.as_arr()).unwrap_or_default() {
        skip(&format!("rulec test: {}", why.as_str().unwrap_or_default()));
    }
    j
}

/// The inputs of each passing rule that hold a value outside the declared range: an amount, a
/// count and a day past each end, with the other inputs absent or inside.
fn outside(stem: &str) -> Vec<String> {
    let names: [&str; 4] = if stem == PASSING[0] { ["declared", "pieces", "ship_on", "signature"] } else { ["申告額", "個数", "発送日", "署名"] };
    let one = |vals: [&str; 4]| format!("{{\"in\":{{{}}}}}", names.iter().zip(vals).map(|(n, v)| format!("\"{n}\":{v}")).collect::<Vec<_>>().join(","));
    vec![
        one(["300001", "null", "null", "null"]),
        one(["-1", "3", "\"2026-12-31\"", "true"]),
        one(["null", "21", "null", "null"]),
        one(["null", "0", "null", "false"]),
        one(["null", "null", "\"2025-12-31\"", "null"]),
        one(["0", "null", "\"2027-01-01\"", "true"]),
    ]
}

/// Every language answers the vectors as the reference evaluator does — the absent value in each
/// table among them — and refuses a value that is there but outside the declared range, as a
/// runner, as a WASI module, as a function on PostgreSQL, as an MCP server. The generated code
/// guarded no optional input before: it answered 2025-01-01 for a date declared over 2026.
#[test]
fn every_language_agrees_and_refuses_a_value_outside_the_range() {
    if !ready(Need::Python, || have("python3", &["--version"]), "python3 is not here") {
        return;
    }
    for (stem, lang, alias) in [(PASSING[0], "en", "parcel_cover"), (PASSING[1], "ja", "parcel_cover_ja")] {
        let t = TempDir::new("optional");
        let out = t.path().join("out");
        let (code, said, e) = rulec(lang, None, &["gen", &format!("tests/optional/{stem}.rule"), "--out", &out.to_string_lossy()]);
        assert_eq!(code, 0, "{stem}: {said}{e}");
        let wrong = outside(stem);
        std::fs::write(out.join("vectors").join(format!("{alias}.refused.jsonl")), wrong.join("\n") + "\n").unwrap();
        let j = rulec_test(&out);
        let mut ran = Vec::new();
        for r in j.get("results").and_then(|v| v.as_arr()).unwrap_or_default() {
            if r.get("rule").and_then(|v| v.as_str()) != Some(alias) || r.get("ran").and_then(|v| v.as_bool()) != Some(true) {
                continue;
            }
            assert_eq!(r.get("ok").and_then(|v| v.as_bool()), Some(true), "{stem}: {r:?}");
            assert_eq!(r.get("refused").and_then(|v| v.as_int()), Some(wrong.len() as i128), "{stem}: not every input was tried: {r:?}");
            ran.push(format!("{}/{}", r.get("lang").and_then(|v| v.as_str()).unwrap_or_default(), r.get("via").and_then(|v| v.as_str()).unwrap_or_default()));
        }
        assert!(ran.iter().any(|x| x.starts_with("numpy/")) || !have("python3", &["-c", "import numpy"]), "{stem}: NumPy did not run: {ran:?}");
        assert!(ran.len() >= 2, "{stem}: {ran:?}");
        println!("optional: {stem} refused by {}", ran.join(", "));
    }
    // A rule whose only weight and rate are optional: the brand of each is declared all the
    // same — the runners build them, and Python's said `module has no attribute 'Gram'` — and
    // the rate is guarded in its steps, a kilogram in grams.
    let t = TempDir::new("optional");
    let rule = t.path().join("light_parcel.rule");
    std::fs::write(
        &rule,
        "rule light_parcel v1\ndescription \"Whether a parcel is light, and whether a share is most of it, from a weight and a share that may be missing\"\n\n\
         inputs\n  weight : mass[g]?  range >=0g <=30kg\n  share  : rate[step 1%]?  range >=0% <=100%\n\n\
         outputs\n  light : bool\n  most  : bool\n\n\
         table by_weight\npolicy unique\n| weight | -> light : bool |\n| <500g  | true |\n| >=500g | false |\n| none   | false |\n\n\
         table by_share\npolicy unique\n| share | -> most : bool |\n| none  | false |\n| <=50% | false |\n| >50%  | true |\n",
    )
    .unwrap();
    let out = t.path().join("out");
    let (code, said, e) = rulec("en", None, &["gen", &rule.to_string_lossy(), "--out", &out.to_string_lossy()]);
    assert_eq!(code, 0, "{said}{e}");
    let wrong = ["{\"in\":{\"weight\":30001,\"share\":null}}", "{\"in\":{\"weight\":null,\"share\":101}}", "{\"in\":{\"weight\":-1,\"share\":50}}"];
    std::fs::write(out.join("vectors/light_parcel.refused.jsonl"), wrong.join("\n") + "\n").unwrap();
    let j = rulec_test(&out);
    let mut ran = 0;
    for r in j.get("results").and_then(|v| v.as_arr()).unwrap_or_default() {
        if r.get("rule").and_then(|v| v.as_str()) != Some("light_parcel") || r.get("ran").and_then(|v| v.as_bool()) != Some(true) {
            continue;
        }
        assert_eq!(r.get("ok").and_then(|v| v.as_bool()), Some(true), "light_parcel: {r:?}");
        assert_eq!(r.get("refused").and_then(|v| v.as_int()), Some(wrong.len() as i128), "light_parcel: {r:?}");
        ran += 1;
    }
    assert!(ran >= 2, "light_parcel: {j:?}");
    // An optional field of the elements a rule walks: read as absent where it is `null` by the MCP
    // servers, the Connect runner and the Swift runner, written back as `null` by Go's record
    // function (which panicked on it), and guarded on each element when it is there.
    let rule = t.path().join("match_count.rule");
    std::fs::write(
        &rule,
        "rule match_count v1\ndescription \"What to do with a list of candidates, from the score each may lack\"\n\n\
         enum match_result = same | diff\nenum action_kind = register | auto | review\n\n\
         inputs\n  auto_ok : bool\n\nelements candidates\n  score : number?  range >=0 <=100\n\n\
         outputs\n  action : action_kind\n\n\
         table row_of\npolicy unique\n| score | -> kind : match_result |\n| >=80 | same |\n| none | diff |\n| <80 | diff |\n\n\
         count hits over candidates where kind = same  range >=0 <=50\n\n\
         table action_of\npolicy unique\n| hits | auto_ok | -> action : action_kind |\n| 0 | - | register |\n| 1 | true | auto |\n| 1 | false | review |\n| >=2 | - | review |\n",
    )
    .unwrap();
    let out = t.path().join("walk");
    let (code, said, e) = rulec("en", None, &["gen", &rule.to_string_lossy(), "--out", &out.to_string_lossy()]);
    assert_eq!(code, 0, "{said}{e}");
    let wrong = ["{\"in\":{\"auto_ok\":true,\"candidates\":[{\"score\":101}]}}", "{\"in\":{\"auto_ok\":false,\"candidates\":[{\"score\":null},{\"score\":-1}]}}"];
    std::fs::write(out.join("vectors/match_count.refused.jsonl"), wrong.join("\n") + "\n").unwrap();
    let j = rulec_test(&out);
    let mut ran = 0;
    for r in j.get("results").and_then(|v| v.as_arr()).unwrap_or_default() {
        if r.get("rule").and_then(|v| v.as_str()) != Some("match_count") || r.get("ran").and_then(|v| v.as_bool()) != Some(true) {
            continue;
        }
        assert_eq!(r.get("ok").and_then(|v| v.as_bool()), Some(true), "match_count: {r:?}");
        assert_eq!(r.get("refused").and_then(|v| v.as_int()), Some(wrong.len() as i128), "match_count: {r:?}");
        ran += 1;
    }
    assert!(ran >= 2, "match_count: {j:?}");
}

/// Each language's runner, handed an amount past the end of its range with the other inputs
/// absent, stops and says the sentence the other languages say; handed every input absent, it
/// answers.
#[test]
fn every_language_says_the_same_sentence_for_a_value_outside_the_range() {
    let ready_b = |b: &rulec::backend::Backend| b.ready.map(|r| r()).unwrap_or(Ok(()));
    let present: Vec<&rulec::backend::Backend> =
        rulec::backend::ALL.iter().filter(|b| have(b.tool, &["--version"]) || have(b.tool, &["version"])).filter(|b| ready_b(b).is_ok()).collect();
    for b in rulec::backend::ALL {
        if !present.iter().any(|p| p.id == b.id) {
            skip(&format!("{} is not here; {} is not run", b.tool, b.name));
        }
    }
    println!("optional: {}", present.iter().map(|b| b.name).collect::<Vec<_>>().join(", "));
    for (stem, lang, alias, said) in [
        (PASSING[0], "en", "parcel_cover", "declared is out of range"),
        (PASSING[1], "ja", "parcel_cover_ja", "申告額 が範囲の外です"),
    ] {
        let t = TempDir::new("optional");
        let out = t.path().join("out");
        let (code, printed, e) = rulec(lang, None, &["gen", &format!("tests/optional/{stem}.rule"), "--out", &out.to_string_lossy()]);
        assert_eq!(code, 0, "{stem}: {printed}{e}");
        let wrong = out.join("vectors").join(".outside.jsonl");
        std::fs::write(&wrong, format!("{}\n", outside(stem)[0])).unwrap();
        let absent = out.join("vectors").join(".absent.jsonl");
        let names: [&str; 4] = if lang == "en" { ["declared", "pieces", "ship_on", "signature"] } else { ["申告額", "個数", "発送日", "署名"] };
        std::fs::write(&absent, format!("{{\"in\":{{{}}}}}\n", names.iter().map(|n| format!("\"{n}\":null")).collect::<Vec<_>>().join(","))).unwrap();
        let pkg = alias.replace('_', "");
        for b in &present {
            let plan = (b.run)(alias, &pkg);
            let cwd = out.join(&plan.cwd);
            if !cwd.exists() {
                continue;
            }
            if let Some((cmd, args)) = &plan.build {
                let built = Command::new(cmd).current_dir(&cwd).env("TMPDIR", tmpdir_in(&out)).args(args).output().unwrap_or_else(|e| panic!("{}: {cmd}: {e}", b.name));
                assert!(built.status.success(), "{stem}: {} does not build:\n{}", b.name, String::from_utf8_lossy(&built.stderr));
            }
            let run = |input: &Path| {
                let o = Command::new(&plan.cmd)
                    .current_dir(&cwd)
                    .env("TMPDIR", tmpdir_in(&out))
                    .args(&plan.args)
                    .stdin(std::fs::File::open(input).unwrap())
                    .output()
                    .unwrap_or_else(|e| panic!("{stem}: {}: {e}", b.name));
                (o.status.success(), format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr)))
            };
            let (ok, text) = run(&wrong);
            assert!(!ok, "{stem}: {} answered an amount outside its range:\n{text}", b.name);
            assert!(text.contains(said), "{stem}: {} does not say `{said}`:\n{text}", b.name);
            let (ok, text) = run(&absent);
            assert!(ok, "{stem}: {} did not answer every input absent:\n{text}", b.name);
        }
    }
}

/// What the coverage audit owes on an optional number or date is what it owes on the type it
/// wraps: the boundaries of each row. It skipped the columns, and owed none.
#[test]
fn the_boundaries_of_an_optional_column_are_owed() {
    let (code, said, e) = rulec("en", None, &["coverage", "tests/optional/parcel_cover.rule"]);
    assert_eq!(code, 0, "{said}{e}");
    let line = said.lines().find(|l| l.contains("boundary-pair coverage")).unwrap_or_else(|| panic!("{said}"));
    let owed: i64 = line.split('/').nth(1).and_then(|x| x.split_whitespace().next()).and_then(|x| x.parse().ok()).unwrap_or(0);
    assert!(owed >= 6, "{line}");
    assert!(line.contains("satisfied"), "{line}");
}

fn recheck_py(cert: &str, rule: &str) -> (i32, String) {
    use std::io::Write;
    use std::process::Stdio;
    let mut p = Command::new("python3")
        .current_dir(root())
        .args(["tools/recheck.py", "--rule", rule])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("cannot start python3");
    p.stdin.as_mut().unwrap().write_all(cert.as_bytes()).unwrap();
    let o = p.wait_with_output().unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr))
}

fn recheck_lean(bin: &Path, cert: &str, rule: &str) -> (i32, String) {
    use std::io::Write;
    use std::process::Stdio;
    let mut p = Command::new(bin)
        .current_dir(root())
        .args(["--rule", rule])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("cannot start the Lean re-checker");
    p.stdin.as_mut().unwrap().write_all(cert.as_bytes()).unwrap();
    let o = p.wait_with_output().unwrap();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned() + &String::from_utf8_lossy(&o.stderr))
}

/// A member of an object, to change in place.
fn member<'a>(j: &'a mut Json, key: &str) -> &'a mut Json {
    match j {
        Json::Obj(kv) => &mut kv.iter_mut().find(|(k, _)| k == key).unwrap_or_else(|| panic!("no `{key}`")).1,
        _ => panic!("not an object where `{key}` was looked for"),
    }
}

fn items(j: &mut Json) -> &mut Vec<Json> {
    match j {
        Json::Arr(xs) => xs,
        _ => panic!("not an array"),
    }
}

/// The certificate of the passing rule, with the table that decides `bulky` changed by `f`.
fn forged(cert: &str, f: impl Fn(&mut Json)) -> String {
    let mut j = rulec::json::parse(cert).unwrap();
    let t = items(member(&mut j, "tables")).iter_mut().find(|t| t.get("table").and_then(|v| v.as_str()) == Some("by_pieces")).expect("no by_pieces");
    f(t);
    rulec::json::unparse(&j)
}

/// The certificate states the absent value as the first coordinate of each optional axis, and both
/// re-checkers hold it: a certificate that leaves the absent value out, or the top of the range,
/// or that lets a comparison's box take the absent value, fails in both.
#[test]
fn both_rechecks_hold_the_absent_value() {
    if !ready(Need::Python, || have("python3", &["--version"]), "python3 is not here") {
        return;
    }
    let rule = "tests/optional/parcel_cover.rule";
    let (code, cert, e) = rulec("en", None, &["certificate", rule]);
    assert_eq!(code, 0, "{cert}{e}");
    let j = rulec::json::parse(&cert).unwrap();
    for t in j.get("tables").and_then(|v| v.as_arr()).unwrap() {
        let a = &t.get("axes").and_then(|v| v.as_arr()).unwrap()[0];
        let first = a.get("coords").and_then(|v| v.as_arr()).unwrap()[0].as_str();
        assert_eq!(first, Some("none"), "{t:?}");
    }
    let lean = {
        let p = root().join("../../proofs/.lake/build/bin/rulec-recheck");
        ready(Need::Lean, || p.exists(), "ritsu's proofs/ is not built (lake build there makes it); the Lean re-check is not run").then_some(p)
    };
    let (code, said) = recheck_py(&cert, rule);
    assert_eq!(code, 0, "{said}");
    if let Some(bin) = &lean {
        let (code, said) = recheck_lean(bin, &cert, rule);
        assert_eq!(code, 0, "{said}");
    }
    let forgeries = [
        (
            "the absent value left out",
            forged(&cert, |t| {
                let a = &mut items(member(t, "axes"))[0];
                items(member(a, "coords")).remove(0);
                items(member(a, "bounds")).remove(0);
                for r in items(member(t, "rows")) {
                    let acc = &mut items(member(r, "accepts"))[0];
                    let cs: Vec<Json> = items(acc).iter().filter_map(|x| x.as_int()).filter(|x| *x > 0).map(|x| Json::Int(x - 1)).collect();
                    *acc = Json::Arr(cs);
                }
                items(member(member(t, "cover"), "split")).remove(0);
            }),
            "absent value",
        ),
        (
            "the top of the range left out",
            forged(&cert, |t| {
                let a = &mut items(member(t, "axes"))[0];
                items(member(a, "coords")).pop();
                items(member(a, "bounds")).pop();
                let n = items(member(a, "coords")).len() as i128;
                for r in items(member(t, "rows")) {
                    let acc = &mut items(member(r, "accepts"))[0];
                    let cs: Vec<Json> = items(acc).iter().filter_map(|x| x.as_int()).filter(|x| *x < n).map(Json::Int).collect();
                    *acc = Json::Arr(cs);
                }
                items(member(member(t, "cover"), "split")).pop();
            }),
            "",
        ),
        (
            "a comparison's box takes the absent value",
            forged(&cert, |t| {
                let r = items(member(t, "rows")).iter_mut().find(|r| r.get("row").and_then(|v| v.as_int()) == Some(2)).unwrap();
                items(&mut items(member(r, "accepts"))[0]).insert(0, Json::Int(0));
            }),
            "",
        ),
    ];
    for (what, bad, says) in forgeries {
        assert_ne!(bad, cert, "{what}: nothing was forged");
        let (code, said) = recheck_py(&bad, rule);
        assert_ne!(code, 0, "{what}: recheck.py passed it\n{said}");
        assert!(said.contains("FAILED") && said.contains(says), "{what}: recheck.py does not say why\n{said}");
        if let Some(bin) = &lean {
            let (code, said) = recheck_lean(bin, &bad, rule);
            assert_ne!(code, 0, "{what}: the Lean re-checker passed it\n{said}");
            assert!(said.contains("FAILED") && said.contains(says), "{what}: the Lean re-checker does not say why\n{said}");
        }
    }
}
