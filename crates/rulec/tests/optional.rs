//! Optional values: inputs that are amounts, numbers, dates and truth values (§15.201), and
//! outputs of every type (§15.249).
//!
//! An optional input takes one value more than the type it wraps: none at all, which the cell
//! `none` tests. An enum's axis always had a coordinate for it. A number's, a date's and a truth
//! value's did not, and their cells were read in `T?` itself, where no number is a value: a
//! comparison took every coordinate and `none` took none. So `due : date?` with the one row
//! `<2026-07-01` passed `check`, and the generated code fell into `unreachable` on 2026-08-01;
//! and `none`, `<5`, `>=5` was called an overlap (E105) with a row no input reaches (E102). The
//! range of an optional input was not read either, so no generated code guarded it.
//!
//! An optional output answers with `none` where a row writes it, and with a value of the type it
//! wraps elsewhere. Only the type check had been left behind: `none` in an output cell was E012
//! whatever the type, a literal and a rounding grid of an optional number were read in `T?` and
//! were E103, and an optional number with no `round` passed. A table below that read an optional
//! column was asked no row for the absent value a table above passes on. Six of the generators
//! did not compile, or answered the word `none`, for an optional input handed on to an output.
//!
//! The materials are in `tests/optional`, English first, with a Japanese version beside each:
//! `parcel_cover.rule`, which writes a row for the absent value of four inputs and covers the
//! declared range beside it; `parcel_answers.rule`, which answers `none` from optional outputs
//! of the six types beside their values; `carried_answers.rule`, which hands optional inputs on
//! to optional outputs and reads one in a table below; and mutants that each say one finding
//! about the absent value. What `rulec check` prints for a mutant, in English and in Japanese, is
//! its golden file in `tests/golden/optional` (`RULEC_BLESS=1 cargo test --test optional` writes
//! them again).

use ritsu_testkit::tmp::tmpdir_in;
use ritsu_testkit::{Need, TempDir, need, ready, skip};
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
    // The optional outputs (§15.249): `none` under an output, an expected value and a table's
    // own column that are not optional; an input that may be absent passed on to an output that is
    // not; an optional amount without its rounding; a table below with no row for the absent value
    // a table above passes on; and an example that expects no value where the rule answers one.
    ("E103_none_under_an_output_that_is_not_optional", "E103_optional_でない出力に書いた_none", "E103", "`none` is written here", "`none` is written here"),
    ("E103_none_expected_of_an_output_that_is_not_optional", "E103_optional_でない出力の例に書いた_none", "E103", "`none` is written here", "`none` is written here"),
    ("E103_none_under_a_column_that_is_not_optional", "E103_optional_でない表の列に書いた_none", "E103", "`none` is written here", "`none` is written here"),
    ("E103_an_input_that_may_be_absent_under_an_output_that_is_not", "E103_無いことのある入力を_optional_でない出力に渡す", "E103", "`quoted` is money[USDc]?", "`見積もり` is money[USDc]?"),
    ("E104_an_optional_amount_with_no_rounding", "E104_丸めの無い_optional_の金額", "E104", "money[USDc]?", "money[USDc]?"),
    ("E101_no_row_for_the_absent_value_from_above", "E101_上の表から来る値の無いことに行が無い", "E101", "service = none", "種類 = none"),
    ("E107_an_example_that_expects_no_value", "E107_値の無いことを期待する例", "E107", "price should be none", "料金 should be none"),
];

/// The rule of optional inputs that passes, and its Japanese version.
const PASSING: [&str; 2] = ["parcel_cover", "小包の補償"];

/// The rules of optional outputs that pass (§15.249), each with its Japanese version: the six
/// types answering `none` beside their values, and optional inputs handed on to optional outputs
/// with a table below that reads one.
const ANSWERING: [(&str, &str); 2] = [("parcel_answers", "小包の答え"), ("carried_answers", "運ばれる答え")];

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
    let mut listed: Vec<String> = MUTANTS
        .iter()
        .flat_map(|(en, ja, ..)| [format!("{en}.rule"), format!("{ja}.rule")])
        .chain(PASSING.iter().map(|s| format!("{s}.rule")))
        .chain(ANSWERING.iter().flat_map(|(en, ja)| [format!("{en}.rule"), format!("{ja}.rule")]))
        .collect();
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
    for (en, ja) in ANSWERING {
        let (re, se) = source(en);
        let (rj, sj) = source(ja);
        assert_eq!(found(&se, &re), found(&sj, &rj), "{en} and {ja}");
    }
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
/// absent, refuses it with the sentence the other languages say; handed every input absent, it
/// answers.
#[test]
fn every_language_says_the_same_sentence_for_a_value_outside_the_range() {
    // Every language there is a toolchain for, the tools level's (ritsu's DESIGN 10.2).
    if !need(Need::Python) {
        return;
    }
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
            // A refusal is a line of its own, in the place of the record (§15.204).
            let (ok, text) = run(&wrong);
            assert!(ok && text.starts_with("{\"refused\":\"input\""), "{stem}: {} did not refuse an amount outside its range:\n{text}", b.name);
            assert!(text.contains(said), "{stem}: {} does not say `{said}`:\n{text}", b.name);
            let (ok, text) = run(&absent);
            assert!(ok && text.starts_with("{\"in\""), "{stem}: {} did not answer every input absent:\n{text}", b.name);
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

// ---------------------------------------------------------------------------
// Optional outputs (§15.249)
// ---------------------------------------------------------------------------

/// The names of a rule's optional outputs, and its alias.
fn optional_outputs(stem: &str) -> (String, Vec<String>) {
    let (rel, src) = source(stem);
    let (f, c) = rulec::prepare(&src, &rel).unwrap_or_else(|_| panic!("{rel} does not read"));
    let names = f.outputs.iter().filter(|o| matches!(c.ty_of(&o.name.text), Some(rulec::types::Ty::Opt(_)))).map(|o| o.name.text.clone()).collect();
    (rulec::codegen::pub_name_of(&f.name), names)
}

/// The rules that answer `none` pass with nothing said: `none` under a column of `T?` of each of
/// the six types, beside a literal and a rounding grid of an optional number read in the type it
/// wraps; optional inputs handed on to optional outputs; and a table below with a row for the
/// absent value a table above hands on. Before, the `none` cells were E012, `439USDc` and
/// `round down(1USDc)` were E103, and the row for no service below was E102.
#[test]
fn optional_outputs_answer_none_beside_their_values() {
    for (en, ja) in ANSWERING {
        for stem in [en, ja] {
            let (rel, src) = source(stem);
            let ds = i18n::with(Lang::En, || rulec::check_source(&src, &rel));
            assert!(ds.is_empty(), "{rel}: {:?}", ds.iter().map(|d| (d.code, d.title.clone())).collect::<Vec<_>>());
        }
        let (_, names) = optional_outputs(en);
        assert!(!names.is_empty(), "{en} has no optional output");
    }
}

/// The vectors of an optional output meet the seven criteria of the coverage audit. An optional
/// amount handed on from an optional input owed a rounding tie it could not reach: the value an
/// optional input comes in with was not read as a number at all, even where it is one.
#[test]
fn the_suite_of_an_optional_output_meets_every_criterion() {
    for (en, ja) in ANSWERING {
        for stem in [en, ja] {
            let (code, said, e) = rulec("en", None, &["coverage", &format!("tests/optional/{stem}.rule")]);
            assert_eq!(code, 0, "{stem}: {said}{e}");
        }
    }
}

/// The reference evaluator answers the absent value where a row writes `none` or hands on an input
/// that has none, and a value of the type the output wraps elsewhere.
#[test]
fn the_reference_evaluator_answers_none() {
    let run = |stem: &str, ins: &[(&str, Val)]| -> Vec<(String, Option<Val>)> {
        let (rel, src) = source(stem);
        let (f, c) = rulec::prepare(&src, &rel).unwrap_or_else(|_| panic!("{rel} does not read"));
        let ins = ins.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
        rulec::eval::run_all(&f, &c, ins).0
    };
    let none = || Val::Enum("none".into());
    let outs = run("parcel_answers", &[("weight", Val::Num(Rat::int(1))), ("media_only", Val::Bool(false)), ("zone", Val::Enum("local".into()))]);
    assert!(outs.len() == 6 && outs.iter().all(|(_, v)| *v == Some(none())), "{outs:?}");
    let outs = run("parcel_answers", &[("weight", Val::Num(Rat::int(17))), ("media_only", Val::Bool(true)), ("zone", Val::Enum("national".into()))]);
    let want = vec![
        ("price".to_string(), Some(Val::Num(Rat::int(513)))),
        ("zone_number".to_string(), Some(Val::Num(Rat::int(8)))),
        ("kind".to_string(), Some(Val::Enum("library".into()))),
        ("signed".to_string(), Some(Val::Bool(true))),
        ("ships".to_string(), Some(Val::Date(2026, 11, 9))),
        ("note".to_string(), Some(Val::Str("MEDIA MAIL".into()))),
    ];
    assert_eq!(outs, want);
    let outs = run("carried_answers", &[("requested", none()), ("declared", none()), ("media_only", Val::Bool(false))]);
    assert_eq!(outs, vec![("service".to_string(), Some(none())), ("insured".to_string(), Some(none())), ("rush".to_string(), Some(Val::Bool(false)))]);
    let outs = run("carried_answers", &[("requested", Val::Enum("express".into())), ("declared", Val::Num(Rat::int(2500))), ("media_only", Val::Bool(false))]);
    assert_eq!(
        outs,
        vec![("service".to_string(), Some(Val::Enum("express".into()))), ("insured".to_string(), Some(Val::Num(Rat::int(2500)))), ("rush".to_string(), Some(Val::Bool(true)))]
    );
}

/// Every language answers the vectors as the reference evaluator does, `none` where it answers
/// none, the vectors asking for the absent value of every optional output: as a runner, through
/// MCP and Connect, as a WASI module and a Wasm module, as a query and a function on PostgreSQL,
/// and as the columns of NumPy. Before, Rust, Go, Swift and Wasm did not compile an optional input
/// handed on to an output, NumPy answered the word `none` for it, and NumPy wrote a date output as
/// its day number.
#[test]
fn every_language_answers_none_as_the_reference_evaluator_does() {
    if !ready(Need::Python, || have("python3", &["--version"]), "python3 is not here") {
        return;
    }
    for (en, ja) in ANSWERING {
        for (stem, lang) in [(en, "en"), (ja, "ja")] {
            let t = TempDir::new("optional");
            let out = t.path().join("out");
            let (code, said, e) = rulec(lang, None, &["gen", &format!("tests/optional/{stem}.rule"), "--out", &out.to_string_lossy()]);
            assert_eq!(code, 0, "{stem}: {said}{e}");
            let (alias, names) = optional_outputs(stem);
            let vectors = std::fs::read_to_string(out.join("vectors").join(format!("{alias}.jsonl"))).unwrap();
            for n in &names {
                assert!(vectors.contains(&format!("\"{n}\":null")), "{stem}: no vector asks for {n} = none");
            }
            let j = rulec_test(&out);
            let mut ran = Vec::new();
            for r in j.get("results").and_then(|v| v.as_arr()).unwrap_or_default() {
                if r.get("rule").and_then(|v| v.as_str()) != Some(alias.as_str()) || r.get("ran").and_then(|v| v.as_bool()) != Some(true) {
                    continue;
                }
                assert_eq!(r.get("ok").and_then(|v| v.as_bool()), Some(true), "{stem}: {r:?}");
                ran.push(format!("{}/{}", r.get("lang").and_then(|v| v.as_str()).unwrap_or_default(), r.get("via").and_then(|v| v.as_str()).unwrap_or_default()));
            }
            assert!(ran.iter().any(|x| x.starts_with("numpy/")) || !have("python3", &["-c", "import numpy"]), "{stem}: NumPy did not run: {ran:?}");
            assert!(ran.len() >= 2, "{stem}: {ran:?}");
            println!("optional: {stem} answered none by {}", ran.join(", "));
        }
    }
}

/// `mypy`, installed or through `uvx`.
fn mypy() -> Option<Vec<String>> {
    if have("mypy", &["--version"]) {
        return Some(vec!["mypy".into()]);
    }
    have("uvx", &["mypy", "--version"]).then(|| vec!["uvx".into(), "mypy".into()])
}

/// The types the generated code gives an optional output hold under the strict checkers: mypy for
/// Python, tsc for TypeScript, steep against the module's own signature for Ruby. Node and Python
/// run their code with the types unread, so every language agreed while TypeScript declared a
/// `bigint` local that an optional enum was handed (§15.249).
#[test]
fn the_generated_types_of_an_optional_output_check() {
    if !need(Need::Python) {
        return;
    }
    let t = TempDir::new("optional-types");
    let out = t.path().join("out");
    let rules: Vec<String> = ANSWERING.iter().map(|(en, _)| format!("tests/optional/{en}.rule")).collect();
    let mut args: Vec<&str> = vec!["gen"];
    args.extend(rules.iter().map(String::as_str));
    let dir = out.to_string_lossy().into_owned();
    args.extend(["--out", dir.as_str()]);
    let (code, said, e) = rulec("en", None, &args);
    assert_eq!(code, 0, "{said}{e}");
    let aliases: Vec<String> = ANSWERING.iter().map(|(en, _)| optional_outputs(en).0).collect();
    match mypy() {
        None => skip("mypy is not here, nor uvx; the Python is not held to mypy --strict"),
        Some(my) => {
            let py = out.join("python");
            let mut files: Vec<PathBuf> = std::fs::read_dir(&py)
                .unwrap()
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "py"))
                .filter(|p| {
                    let n = p.file_name().unwrap().to_string_lossy().into_owned();
                    !n.ends_with("_service.py") && !n.ends_with("_connect_runner.py")
                })
                .collect();
            files.sort();
            let o = Command::new(&my[0]).args(&my[1..]).args(["--strict", "--no-color-output"]).args(&files).current_dir(&py).output().expect("cannot start mypy");
            assert!(o.status.success(), "the Python does not pass mypy --strict:\n{}", String::from_utf8_lossy(&o.stdout));
        }
    }
    let tsc = ritsu_testkit::tools::find("TSC", Some(Path::new("tools/node_modules/.bin/tsc")), "tsc", &["--version"]);
    match tsc {
        None => skip("tsc is not here (npm ci --prefix tools puts it there); the TypeScript is not held to tsc --strict"),
        Some(tsc) if need(Need::Node) => {
            let ts = out.join("typescript");
            std::fs::write(ts.join("package.json"), "{\"type\": \"module\"}\n").unwrap();
            let files: Vec<PathBuf> = aliases.iter().flat_map(|a| [format!("{a}.ts"), format!("{a}_runner.ts"), format!("{a}_mcp.ts")]).map(|f| ts.join(f)).collect();
            let mut c = Command::new(&tsc);
            c.args(["--strict", "--noEmit", "--allowImportingTsExtensions", "--erasableSyntaxOnly", "--module", "nodenext", "--target", "es2022", "--types", "node"]);
            if let Ok(types) = std::fs::canonicalize(root().join("tools/node_modules/@types")) {
                c.arg("--typeRoots").arg(types);
            }
            let o = c.args(&files).current_dir(&ts).output().expect("cannot start tsc");
            assert!(o.status.success(), "the TypeScript does not pass tsc --strict:\n{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
        }
        Some(_) => {}
    }
    if ready(Need::Ruby, || have("steep", &["--version"]) && have("rbs", &["--version"]), "steep or rbs is not here; the Ruby is not held to its own signature") {
        let rb = out.join("ruby");
        let modules: Vec<String> = aliases.iter().map(|a| format!("\"{a}.rb\"")).collect();
        std::fs::write(rb.join("Steepfile"), format!("target :app do\n  signature \"sig\"\n  check {}\nend\n", modules.join(", "))).unwrap();
        let o = Command::new("steep").arg("check").current_dir(&rb).output().expect("cannot start steep");
        let said = String::from_utf8_lossy(&o.stdout).into_owned();
        assert!(said.contains("No type error detected"), "the Ruby does not pass steep against its own signature:\n{said}");
    }
}

/// W111 says of the values of an enum that no row names. A value a row answers with through a
/// name that carries it — an input handed on — is in that row where no table tests the enum in a
/// column: there is no table that could give it a row of its own, and `default` would say it falls
/// to one. A value nothing writes is said as before, and so is a value a table tests and leaves to
/// a `-`.
#[test]
fn w111_counts_a_value_a_row_hands_on() {
    let unnamed = |src: &str| -> Vec<String> {
        i18n::with(Lang::En, || rulec::check_source(src, "w111.rule"))
            .iter()
            .filter(|d| d.code == "W111")
            .flat_map(|d| d.notes.iter().filter_map(|n| n.strip_prefix("Values that never appear: ").map(str::to_string)))
            .collect()
    };
    let head = |opt: &str| {
        format!(
            "rule hand_on v1\ndescription \"The service a parcel goes by\"\n\nenum service = media | library | express\n\ninputs\n  requested  : service{opt}\n  media_only : bool\n\noutputs\n  service : service{opt}\n\n"
        )
    };
    for opt in ["?", ""] {
        let handed = format!("{}table service_of\npolicy unique\n| media_only | -> service : service{opt} |\n| false | requested |\n| true | media |\n", head(opt));
        assert_eq!(unnamed(&handed), Vec::<String>::new(), "handed on ({opt})");
        let tested = format!("{handed}\ntable rush_of\npolicy unique\n| requested | -> rush : bool |\n| express | true |\n| - | false |\n").replace("  service : service", "  rush : bool\n  service : service");
        assert_eq!(unnamed(&tested), vec!["library".to_string()], "tested ({opt})");
    }
    let written = format!("{}table service_of\npolicy unique\n| media_only | -> service : service? |\n| false | none |\n| true | media |\n", head("?"));
    assert_eq!(unnamed(&written), vec!["library / express".to_string()]);
}

/// The doors that read past answers take `none` as the answer of no value: `fixtures lint` reads
/// `null` for an optional output, `replay` holds it to what the rule answers, `verify` to what a
/// legacy implementation answers, and `diff` says where an answer moves between no value and one.
#[test]
fn the_doors_of_past_answers_read_none() {
    let t = TempDir::new("optional-doors");
    let rule = "tests/optional/parcel_answers.rule";
    let records = t.path().join("records.jsonl");
    std::fs::write(
        &records,
        "{\"in\":{\"weight\":1,\"media_only\":false,\"zone\":\"local\"},\"observed\":{\"price\":null,\"zone_number\":null,\"kind\":null,\"signed\":null,\"ships\":null,\"note\":null}}\n\
         {\"in\":{\"weight\":1,\"media_only\":true,\"zone\":\"local\"},\"observed\":{\"price\":439,\"zone_number\":1,\"kind\":\"media\",\"signed\":false,\"ships\":\"2026-11-02\",\"note\":\"MEDIA MAIL\"}}\n\
         {\"in\":{\"weight\":20,\"media_only\":true,\"zone\":\"national\"},\"observed\":{\"price\":null,\"zone_number\":8,\"kind\":\"library\",\"signed\":true,\"ships\":\"2026-11-09\",\"note\":\"MEDIA MAIL\"}}\n",
    )
    .unwrap();
    let (code, said, e) = rulec("en", None, &["fixtures", "lint", &records.to_string_lossy(), rule]);
    assert_eq!(code, 0, "{said}{e}");
    let (code, said, e) = rulec("en", None, &["replay", rule, "--fixtures", &records.to_string_lossy(), "--format", "json"]);
    assert_eq!(code, 1, "{said}{e}");
    let j = rulec::json::parse(&said).unwrap_or_else(|x| panic!("{x}: {said}"));
    assert_eq!((j.get("compared").and_then(|v| v.as_int()), j.get("matched").and_then(|v| v.as_int())), (Some(3), Some(2)), "{said}");
    assert!(said.contains("\"theirs\":{\"price\":\"none\"}") || said.contains("\"price\":\"none\""), "{said}");
    // A version that charges where there was no price: the inputs that move, and how.
    let v2 = t.path().join("parcel_answers_v2.rule");
    std::fs::write(&v2, std::fs::read_to_string(root().join(rule)).unwrap().replace("rule parcel_answers v1", "rule parcel_answers v2").replace("| false      | -          | none                    |", "| false      | -          | 100USDc                 |")).unwrap();
    let (code, said, e) = rulec("en", None, &["diff", rule, &v2.to_string_lossy()]);
    assert_eq!(code, 1, "{said}{e}");
    assert!(said.contains("price: none → 100"), "{said}");
    if ready(Need::Python, || have("python3", &["--version"]), "python3 is not here; verify is not run") {
        let adapter = t.path().join("adapter.py");
        std::fs::write(
            &adapter,
            "import json, sys\nsys.stdin.readline()\nprint(json.dumps({\"ok\": True, \"impl\": \"legacy@test\"}), flush=True)\n\
             for line in sys.stdin:\n    line = line.strip()\n    if not line:\n        continue\n    req = json.loads(line)\n    d = req[\"in\"]\n\
             \x20   media = d[\"media_only\"]\n\
             \x20   out = {\"price\": (439 if d[\"weight\"] <= 16 else 513) if media else None, \"zone_number\": (1 if d[\"zone\"] == \"local\" else 8) if media else None,\n\
             \x20          \"kind\": (\"media\" if d[\"weight\"] <= 16 else \"library\") if media else None, \"signed\": (d[\"zone\"] == \"national\") if media else None,\n\
             \x20          \"ships\": (\"2026-11-02\" if d[\"zone\"] == \"local\" else \"2026-11-09\") if media else None, \"note\": \"MEDIA MAIL\" if media else None}\n\
             \x20   print(json.dumps({\"id\": req[\"id\"], \"out\": out}), flush=True)\n",
        )
        .unwrap();
        let (code, said, e) = rulec("en", None, &["verify", rule, "--adapter", "python3", &adapter.to_string_lossy()]);
        assert_eq!(code, 0, "{said}{e}");
        assert!(said.contains("No mismatches"), "{said}");
    }
}

/// What ritsu's port of rules says of an optional output is what it said of one before: its type
/// is `Opt` of the type it wraps, the evaluator answers no value as `Value::None`, and
/// `outputs_over` counts `Value::None` among the values, with an input that comes to it.
/// `output_values` is undecided for an optional number, as it was, and now says why: its answer is
/// a range of numbers, with no room for no value.
#[test]
fn the_port_says_no_value_as_it_did() {
    use ritsu_ports::{ColumnType, Found, Rules, Value};
    let e = rulec::ports::Engine::new();
    let rule = Path::new("tests/optional/carried_answers.rule");
    let facts = e.facts(rule).unwrap_or_else(|x| panic!("{x:?}"));
    let ty = |n: &str| facts.outputs.iter().find(|c| c.name == n).map(|c| c.ty.clone()).unwrap();
    assert_eq!(ty("service"), ColumnType::Opt(Box::new(ColumnType::Enum("service".into()))));
    match ty("insured") {
        ColumnType::Opt(inner) => assert!(matches!(*inner, ColumnType::Num { min: Some(0), max: Some(10_000), .. }), "{inner:?}"),
        other => panic!("{other:?}"),
    }
    match e.output_values(rule, "insured").unwrap() {
        Found::Undecided(t) => assert!(t.en.contains("may have no value") && !t.ja.is_empty(), "{t:?}"),
        Found::Value(v) => panic!("decided {v:?}"),
    }
    match e.outputs_over(rule, "service", &[]).unwrap() {
        Found::Value(vs) => {
            let values: Vec<&Value> = vs.iter().map(|(v, _)| v).collect();
            assert!(values.contains(&&Value::None) && values.contains(&&Value::Enum("express".into())), "{vs:?}");
            for (v, ins) in &vs {
                let got = e.eval(rule, ins).unwrap_or_else(|x| panic!("{x:?}"));
                assert_eq!(got.iter().find(|(n, _)| n == "service").map(|(_, x)| x), Some(v), "{ins:?}");
            }
        }
        Found::Undecided(t) => panic!("{t:?}"),
    }
    let got = e.eval(rule, &vec![("requested".into(), Value::None), ("declared".into(), Value::None), ("media_only".into(), Value::Bool(false))]).unwrap();
    assert_eq!(got, vec![("service".to_string(), Value::None), ("insured".to_string(), Value::None), ("rush".to_string(), Value::Bool(false))]);
}

/// The pages for people say so too: the customer's page reads an optional output as what it is
/// when it has a value, "or none", and the page that tries a case shows `none` for an output with
/// no value and the unit of the type it wraps beside one with a value.
#[test]
fn the_pages_say_an_output_may_have_no_value() {
    let rule = "tests/optional/parcel_answers.rule";
    let (code, said, e) = rulec("en", None, &["doc", rule, "--audience", "customer"]);
    assert_eq!(code, 0, "{said}{e}");
    assert!(said.contains("**price** is an amount in USDc, or none, rounded down to 1USDc."), "{said}");
    let (code, said, e) = rulec("ja", None, &["doc", "tests/optional/小包の答え.rule", "--audience", "customer"]);
    assert_eq!(code, 0, "{said}{e}");
    assert!(said.contains("**料金**は金額（USDc）、または無しで"), "{said}");
    let (code, said, e) = rulec("en", None, &["doc", rule, "--format", "html"]);
    assert_eq!(code, 0, "{said}{e}");
    assert!(said.contains("if (v === null) return \"none\";"), "the page shows no value as none");
    assert!(
        said.contains("\"name\":\"price\",\"alias\":\"price\",\"kind\":\"number\",\"unit\":\"USDc\",\"scale\":1,\"range\":{\"min\":439,\"max\":513},\"optional\":true"),
        "the page reads an optional amount in its unit"
    );
}

/// A DOM small enough to run the panel the page carries, as in `tests/fold.rs`: elements, the five
/// ids the script looks up, and the listeners it attaches. `seen.address` keeps the address the
/// script writes after a run, and the page is opened on `PAGE_SEARCH`.
const DOM: &str = r##"
class El {
  constructor(tag) {
    this.tagName = tag.toUpperCase();
    this.children = [];
    this.listeners = {};
    this.classList = { add() {}, remove() {}, toggle() {}, contains: () => false };
    this.style = {};
    this.value = "";
    this.checked = false;
    this._text = "";
    this.name = "";
  }
  get textContent() { return this._text; }
  set textContent(v) { this._text = String(v); }
  appendChild(c) { this.children.push(c); c.parentNode = this; return c; }
  remove() { const p = this.parentNode; if (p) p.children = p.children.filter((x) => x !== this); }
  addEventListener(k, f) { (this.listeners[k] ||= []).push(f); }
  click() { for (const f of this.listeners.click || []) f(); }
  querySelectorAll() { return []; }
  get elements() {
    const o = {};
    const walk = (n) => { if (n.name) o[n.name] = n; n.children.forEach(walk); };
    this.children.forEach(walk);
    return o;
  }
}
const ids = {};
for (const id of ["try-form", "try-rows", "try-buttons", "try-result", "try-record"]) {
  ids["#" + id] = new El(id === "try-form" ? "form" : "div");
}
const seen = { address: "" };
globalThis.document = { createElement: (t) => new El(t), querySelector: (s) => ids[s] ?? null, querySelectorAll: () => [] };
globalThis.CSS = { escape: (s) => s };
globalThis.history = { replaceState(_s, _t, u) { seen.address = u; } };
globalThis.location = { hash: "", search: process.env.PAGE_SEARCH ?? "" };
globalThis.window = { addEventListener() {} };
globalThis.window.parent = globalThis.window;
export { ids, seen };
"##;

/// Driving that panel: what the page opened on, then each example button with the answer and the
/// address it leaves, then every optional input emptied and the case run.
const DRIVE: &str = r##"
const said = { opened: ids["#try-result"].textContent, examples: [], addresses: [] };
const btns = ids["#try-buttons"].children;
for (let i = 1; i < btns.length; i++) {
  btns[i].click();
  said.examples.push(ids["#try-result"].textContent);
  said.addresses.push(seen.address);
}
for (const inp of RULE.inputs) if (inp.optional) ids["#try-form"].elements[inp.alias].value = "";
btns[0].click();
said.empty = ids["#try-result"].textContent;
console.log(JSON.stringify(said));
"##;

/// The page's panel, run in node on that DOM and opened on `search`: what it says, as JSON.
fn page_said(stem: &str, search: &str) -> Json {
    let (code, html, e) = rulec("en", None, &["doc", &format!("tests/optional/{stem}.rule"), "--format", "html"]);
    assert_eq!(code, 0, "{stem}: {e}");
    let head = html.find("<script type=\"module\">").expect("the page has no script") + "<script type=\"module\">".len();
    let body = &html[head..head + html[head..].find("</script>").expect("the script is not closed")];
    let t = TempDir::new("optional-page");
    std::fs::write(t.path().join("dom.mjs"), DOM).unwrap();
    let p = t.path().join("page.mjs");
    std::fs::write(&p, format!("import {{ ids, seen }} from \"./dom.mjs\";\n{body}\n{DRIVE}")).unwrap();
    let o = Command::new("node").arg(&p).env("PAGE_SEARCH", search).output().expect("cannot start node");
    let out = String::from_utf8_lossy(&o.stdout).into_owned();
    assert!(o.status.success(), "{stem}: {out}{}", String::from_utf8_lossy(&o.stderr));
    rulec::json::parse(out.trim()).unwrap_or_else(|_| panic!("{stem}: {out}"))
}

/// The answers on a result line, without the names: `fee = 0JPY   bulky = false` is `0JPY`,
/// `false`.
fn values(line: &str) -> Vec<String> {
    line.split("   ").map(|p| p.split_once(" = ").map(|(_, v)| v.to_string()).unwrap_or_else(|| p.to_string())).collect()
}

fn strs(j: &Json, key: &str) -> Vec<String> {
    j.get(key).and_then(|v| v.as_arr()).unwrap_or_default().iter().map(|v| v.as_str().unwrap_or_default().to_string()).collect()
}

/// The page tries a case with values that may be absent, and every example of such a rule. An
/// optional input gets the field of the type it wraps: a list that starts with `none`, or a field
/// left empty for the absent value. An example passes `null` for `none`, an absent answer shows as
/// `none`, and the address carries an absent value as an empty one. Before, an optional input was a
/// text field whose words no generated code took for an amount, a date or a truth value, and the
/// examples of a rule with an optional amount, number or date were left off the page.
#[test]
fn the_page_tries_a_case_with_values_that_may_be_absent() {
    if !ready(Need::Node, || have("node", &["--version"]), "node is not here") {
        return;
    }
    let cover = page_said("parcel_cover", "?declared=&pieces=7&ship_on=&signature=false");
    assert_eq!(cover.get("opened").and_then(|v| v.as_str()), Some("fee = 0JPY   bulky = true   rush = false   handled = false"), "{cover:?}");
    assert_eq!(
        strs(&cover, "examples"),
        [
            "fee = 0JPY   bulky = false   rush = false   handled = false",
            "fee = 300JPY   bulky = true   rush = true   handled = true",
            "fee = 800JPY   bulky = false   rush = false   handled = false",
        ]
    );
    assert_eq!(
        strs(&cover, "addresses"),
        [
            "?declared=&pieces=&ship_on=&signature=",
            "?declared=50000&pieces=5&ship_on=2026-12-24&signature=true",
            "?declared=300000&pieces=4&ship_on=2026-12-19&signature=false",
        ]
    );
    assert_eq!(cover.get("empty").and_then(|v| v.as_str()), Some("fee = 0JPY   bulky = false   rush = false   handled = false"));

    let answers = page_said("parcel_answers", "");
    assert_eq!(
        strs(&answers, "examples"),
        [
            "price = none   zone_number = none   kind = none   signed = none   ships = none   note = none",
            "price = 439USDc   zone_number = 1   kind = media   signed = false   ships = 2026-11-02   note = MEDIA MAIL",
            "price = 513USDc   zone_number = 8   kind = library   signed = true   ships = 2026-11-09   note = MEDIA MAIL",
        ]
    );
    let carried = page_said("carried_answers", "");
    assert_eq!(
        strs(&carried, "examples"),
        [
            "service = none   insured = none   rush = false",
            "service = express   insured = 2500USDc   rush = true",
            "service = media   insured = none   rush = false",
            "service = library   insured = none   rush = false",
        ]
    );
    assert_eq!(strs(&carried, "addresses")[0], "?requested=&declared=&media_only=false");
    assert_eq!(carried.get("empty").and_then(|v| v.as_str()), Some("service = none   insured = none   rush = false"));

    // The Japanese versions answer every example on the page too, `none` in the same places (their
    // values have names of their own: `メディア`, `円`).
    for (en, ja, j) in [("parcel_cover", "小包の補償", &cover), ("parcel_answers", "小包の答え", &answers), ("carried_answers", "運ばれる答え", &carried)] {
        let jj = page_said(ja, "");
        let none = |j: &Json| -> Vec<Vec<bool>> { strs(j, "examples").iter().map(|l| values(l).iter().map(|v| v == "none").collect()).collect() };
        assert_eq!(none(j), none(&jj), "{en} and {ja} answer none in different places: {jj:?}");
    }
}

/// What a table above writes is read off its rows in the certificate, `none` among it: a table
/// below that leaves out a value no row above writes rests on that fact, and both re-checkers
/// settle it. A row that wrote `none` was read as writing nothing, so the table below rested on no
/// fact and both refused the certificate. One that says the table above never writes the absent
/// value, or that reads another word where a row writes `none`, fails in both.
#[test]
fn both_rechecks_settle_what_a_table_above_writes_as_none() {
    if !ready(Need::Python, || have("python3", &["--version"]), "python3 is not here") {
        return;
    }
    let t = TempDir::new("optional-above");
    let rule = t.path().join("never_express.rule");
    std::fs::write(
        &rule,
        "rule never_express v1\ndescription \"Whether a parcel is approved, from the service a table above decides, which no row above makes express\"\n\n\
         enum service = media | library | express\n\n\
         inputs\n  media_only : bool\n  weight     : mass[oz]  range >=1oz <=32oz\n\n\
         outputs\n  approved : bool\n\n\
         table kind_of\npolicy unique\n| media_only | weight | -> kind : service? |\n| false | - | none |\n| true | <=1lb | media |\n| true | >1lb | library |\n\n\
         table approved_of\npolicy unique\n| kind | -> approved : bool |\n| none | false |\n| media | true |\n| library | false |\n",
    )
    .unwrap();
    let path = rule.to_string_lossy().into_owned();
    let (code, cert, e) = rulec("en", None, &["certificate", &path]);
    assert_eq!(code, 0, "{cert}{e}");
    let j = rulec::json::parse(&cert).unwrap();
    let tables = j.get("tables").and_then(|v| v.as_arr()).unwrap();
    let produced: Vec<String> = tables[0].get("rows").and_then(|v| v.as_arr()).unwrap().iter().map(|r| rulec::json::unparse(r.get("produces").unwrap())).collect();
    assert_eq!(produced, ["[\"none\"]", "[\"media\"]", "[\"library\"]"]);
    let lean = {
        let p = root().join("../../proofs/.lake/build/bin/rulec-recheck");
        ready(Need::Lean, || p.exists(), "ritsu's proofs/ is not built (lake build there makes it); the Lean re-check is not run").then_some(p)
    };
    let (code, said) = recheck_py(&cert, &path);
    assert_eq!(code, 0, "{said}");
    if let Some(bin) = &lean {
        let (code, said) = recheck_lean(bin, &cert, &path);
        assert_eq!(code, 0, "{said}");
    }
    let below = |f: &dyn Fn(&mut Json)| -> String {
        let mut j = rulec::json::parse(&cert).unwrap();
        let t = items(member(&mut j, "tables")).iter_mut().find(|t| t.get("table").and_then(|v| v.as_str()) == Some("approved_of")).unwrap();
        f(t);
        rulec::json::unparse(&j)
    };
    let above = |f: &dyn Fn(&mut Json)| -> String {
        let mut j = rulec::json::parse(&cert).unwrap();
        let t = items(member(&mut j, "tables")).iter_mut().find(|t| t.get("table").and_then(|v| v.as_str()) == Some("kind_of")).unwrap();
        f(t);
        rulec::json::unparse(&j)
    };
    let forgeries = [
        ("the fact that no row above writes the absent value", below(&|t| items(member(member(t, "above"), "never")).push(rulec::json::parse("{\"axis\":0,\"coord\":0}").unwrap()))),
        ("the fact the leaf rests on left out", below(&|t| items(member(member(t, "above"), "never")).clear())),
        ("another word read where a row writes none", above(&|t| *member(&mut items(member(t, "rows"))[0], "produces") = rulec::json::parse("[\"express\"]").unwrap())),
    ];
    for (what, bad) in forgeries {
        assert_ne!(bad, cert, "{what}: nothing was forged");
        let (code, said) = recheck_py(&bad, &path);
        assert!(code != 0 && said.contains("FAILED"), "{what}: recheck.py passed it\n{said}");
        if let Some(bin) = &lean {
            let (code, said) = recheck_lean(bin, &bad, &path);
            assert!(code != 0 && said.contains("FAILED"), "{what}: the Lean re-checker passed it\n{said}");
        }
    }
}

/// The range the certificate states for an optional number holds the values its rows write where
/// they write one, and both re-checkers hold it there: a row whose answer cell is `none` writes no
/// number. recheck.py refused every such certificate, reading a `none` row as a write it could not
/// read, and the Lean re-checker did not look at the column at all. A range narrower than what the
/// rows write fails in both, and so does a row that writes an amount passed off as `none`.
#[test]
fn both_rechecks_hold_the_range_of_an_optional_number() {
    if !ready(Need::Python, || have("python3", &["--version"]), "python3 is not here") {
        return;
    }
    let lean = {
        let p = root().join("../../proofs/.lake/build/bin/rulec-recheck");
        ready(Need::Lean, || p.exists(), "ritsu's proofs/ is not built (lake build there makes it); the Lean re-check is not run").then_some(p)
    };
    for (en, ja) in ANSWERING {
        for stem in [en, ja] {
            let rule = format!("tests/optional/{stem}.rule");
            let (code, cert, e) = rulec("en", None, &["certificate", &rule]);
            assert_eq!(code, 0, "{cert}{e}");
            let (code, said) = recheck_py(&cert, &rule);
            assert_eq!(code, 0, "{stem}: {said}");
            assert!(said.contains("columns a table writes numbers into hold every value their rows write"), "{stem}: no range was held\n{said}");
            if let Some(bin) = &lean {
                let (code, said) = recheck_lean(bin, &cert, &rule);
                assert_eq!(code, 0, "{stem}: {said}");
                assert!(said.contains("columns a table writes numbers into hold every value their rows write"), "{stem}: no range was held\n{said}");
            }
        }
    }
    let rule = "tests/optional/parcel_answers.rule";
    let (_, cert, _) = rulec("en", None, &["certificate", rule]);
    let with = |f: &dyn Fn(&mut Json)| -> String {
        let mut j = rulec::json::parse(&cert).unwrap();
        f(&mut j);
        rulec::json::unparse(&j)
    };
    let forgeries = [
        ("a range narrower than the amounts written", with(&|j| *member(member(j, "ranges"), "price") = rulec::json::parse("[\"439\",\"500\"]").unwrap())),
        (
            "an amount passed off as none",
            with(&|j| {
                {
                    let t = items(member(j, "tables")).iter_mut().find(|t| t.get("table").and_then(|v| v.as_str()) == Some("price_of")).unwrap();
                    let r = &mut items(member(t, "rows"))[2];
                    *member(r, "writes") = Json::Arr(vec![Json::Null]);
                    *member(r, "produces") = rulec::json::parse("[\"none\"]").unwrap();
                }
                *member(member(j, "ranges"), "price") = rulec::json::parse("[\"439\",\"439\"]").unwrap();
            }),
        ),
    ];
    for (what, bad) in forgeries {
        assert_ne!(bad, cert, "{what}: nothing was forged");
        let (code, said) = recheck_py(&bad, rule);
        assert!(code != 0 && said.contains("FAILED"), "{what}: recheck.py passed it\n{said}");
        if let Some(bin) = &lean {
            let (code, said) = recheck_lean(bin, &bad, rule);
            assert!(code != 0 && said.contains("FAILED"), "{what}: the Lean re-checker passed it\n{said}");
        }
    }
}

/// A `once` line over an optional output is laid on the rows: its words take `none` where its
/// cell does, and a row that writes `none` reads it. It was not laid, since no value of an
/// optional output was read off the rows. A row that hands a name on reads no word, and counts
/// wherever it may: it was read as the name's own word, which no cell takes, so the row counted
/// nowhere. Both re-checkers hold each word to the output's values, and refuse a certificate that
/// reads a name as one, or a word the output does not have.
#[test]
fn both_rechecks_hold_a_once_line_to_the_values_of_its_output() {
    if !ready(Need::Python, || have("python3", &["--version"]), "python3 is not here") {
        return;
    }
    let t = TempDir::new("optional-once");
    // The corpus's one-time perk, with a trial granted that a guest asking for one has no answer to.
    let perk = std::fs::read_to_string(root().join("tests/corpus/one_time_perk.rule"))
        .unwrap()
        .replace("rule one_time_perk v1", "rule optional_perk v1")
        .replace("  trial_granted    : bool\n", "  trial_granted    : bool?\n")
        .replace(
            "| guest     | ask_trial | -           | status         | trial_taken      | false         |",
            "| guest     | ask_trial | -           | status         | trial_taken      | none          |",
        );
    let handed = "rule handed_flag v1\ndescription \"A flag a guest hands on once, on joining\"\n\n\
                  enum membership = guest | member\n\n\
                  inputs\n  status : membership\n  flag   : bool\n\n\
                  outputs\n  next_status : membership\n  echo        : bool\n\n\
                  table step\npolicy unique\n| status | flag | -> next_status | echo |\n| guest | - | member | flag |\n| member | true | status | false |\n| member | false | status | false |\n\n\
                  machine membership over step\n  carry   status -> next_status\n  initial guest\n  once    echo true\n";
    let lean = {
        let p = root().join("../../proofs/.lake/build/bin/rulec-recheck");
        ready(Need::Lean, || p.exists(), "ritsu's proofs/ is not built (lake build there makes it); the Lean re-check is not run").then_some(p)
    };
    for (name, src, row, honest, forged) in [("optional_perk", perk.as_str(), 6, "\"none\"", "\"nothing\""), ("handed_flag", handed, 1, "null", "\"flag\"")] {
        let rule = t.path().join(format!("{name}.rule"));
        std::fs::write(&rule, src).unwrap();
        let path = rule.to_string_lossy().into_owned();
        let (code, cert, e) = rulec("en", None, &["certificate", &path]);
        assert_eq!(code, 0, "{name}: {cert}{e}");
        let j = rulec::json::parse(&cert).unwrap();
        let once = &j.get("machine").and_then(|m| m.get("once")).and_then(|v| v.as_arr()).unwrap()[0];
        let at = once.get("rows").and_then(|v| v.as_arr()).unwrap().iter().find(|r| r.get("row").and_then(|v| v.as_int()) == Some(row)).unwrap();
        assert_eq!(rulec::json::unparse(at.get("value").unwrap()), honest, "{name}: row {row}");
        let (code, said) = recheck_py(&cert, &path);
        assert!(code == 0 && !said.contains("is not laid on the rows"), "{name}: {said}");
        if let Some(bin) = &lean {
            let (code, said) = recheck_lean(bin, &cert, &path);
            assert!(code == 0 && !said.contains("is not laid on the rows"), "{name}: {said}");
        }
        let mut bad = rulec::json::parse(&cert).unwrap();
        let once = &mut items(member(member(&mut bad, "machine"), "once"))[0];
        let r = items(member(once, "rows")).iter_mut().find(|r| r.get("row").and_then(|v| v.as_int()) == Some(row)).unwrap();
        *member(r, "value") = rulec::json::parse(forged).unwrap();
        let bad = rulec::json::unparse(&bad);
        let (code, said) = recheck_py(&bad, &path);
        assert!(code != 0 && said.contains("is not a value of"), "{name}: recheck.py passed {forged}\n{said}");
        if let Some(bin) = &lean {
            let (code, said) = recheck_lean(bin, &bad, &path);
            assert!(code != 0 && said.contains("is not a value of"), "{name}: the Lean re-checker passed {forged}\n{said}");
        }
    }
}

/// A value that may be absent is tested in a table's cells and handed on to an optional column, and
/// works in no expression (§2.1): `define d : money[USDc]? = quoted * 2` was typed `money[USDc]?`
/// and passed, and the generated code multiplied the absent value. It is E103, said of the name. A
/// definition that only names one hands the absent value on, as a cell does.
#[test]
fn a_value_that_may_be_absent_works_in_no_expression() {
    let rule = |define: &str| {
        format!(
            "rule doubled v1\ndescription \"A quote, doubled\"\n\ninputs\n  quoted     : money[USDc]?  range >=0USDc <=10_000USDc\n  media_only : bool\n\n\
             outputs\n  price : money[USDc]?  round down(1USDc)\n\n{define}\n\n\
             table price_of\npolicy unique\n| media_only | -> price : money[USDc]? |\n| false | doubled |\n| true | none |\n"
        )
    };
    for define in ["define doubled : money[USDc]? = quoted * 2", "define doubled : money[USDc]? = round_down(quoted, 1USDc)"] {
        let ds = i18n::with(Lang::En, || rulec::check_source(&rule(define), "doubled.rule"));
        let found: Vec<(&str, String)> = ds.iter().map(|d| (d.code, d.title.clone())).collect();
        assert_eq!(found, vec![("E103", "`quoted` may have no value, and an expression cannot work with one".to_string())], "{define}");
    }
    let ds = i18n::with(Lang::En, || rulec::check_source(&rule("define doubled : money[USDc]? = quoted"), "doubled.rule"));
    assert!(ds.is_empty(), "{:?}", ds.iter().map(|d| (d.code, d.title.clone())).collect::<Vec<_>>());
}

/// An applied rule's outputs become definitions of this rule (DESIGN §15.69). An optional one with
/// no rounding — an enum, a truth value, a date, a string — names the callee's column and hands the
/// absent value on, through `with` as well; an optional number, rounded as every number is, would be
/// a definition that rounds a value that may be absent, and is E044 at the `apply`. Before, both
/// were stopped by the callee's own E012 and E103.
#[test]
fn an_applied_optional_output_hands_none_on_or_says_why_it_cannot() {
    let t = TempDir::new("optional-apply");
    std::fs::write(
        t.path().join("kind_callee.rule"),
        "rule kind_callee v1\ndescription \"A service, which a parcel that is not media has none of\"\n\nenum service = media | library\n\n\
         inputs\n  weight     : mass[oz]  range >=1oz <=32oz\n  media_only : bool\n\n\
         outputs\n  kind : service?\n\n\
         table kind_of\npolicy unique\n| media_only | weight | -> kind : service? |\n| false | - | none |\n| true | <=1lb | media |\n| true | >1lb | library |\n",
    )
    .unwrap();
    std::fs::write(
        t.path().join("price_callee.rule"),
        "rule price_callee v1\ndescription \"A price, which a parcel that is not media has none of\"\n\n\
         inputs\n  weight     : mass[oz]  range >=1oz <=32oz\n  media_only : bool\n\n\
         outputs\n  price : money[USDc]?  round down(1USDc)\n\n\
         table price_of\npolicy unique\n| media_only | weight | -> price : money[USDc]? |\n| false | - | none |\n| true | <=1lb | 439USDc |\n| true | >1lb | 513USDc |\n",
    )
    .unwrap();
    let pin = |callee: &str| rulec::link::pin::meaning(&std::fs::read_to_string(t.path().join(callee)).unwrap());
    // The caller: its outputs, then the apply with what it reads the callee's outputs as.
    let caller = |callee: &str, outputs: &str, reads: &str| -> (String, String, Vec<rulec::diag::Diag>) {
        let src = format!(
            "rule caller v1\ndescription \"What a callee answers, applied\"\n\nenum service = media | library\nenum lane = fast_lane | slow_lane\n\n\
             inputs\n  weight     : mass[oz]  range >=1oz <=32oz\n  media_only : bool\n\n{outputs}\n\
             apply k = \"{callee}\" meaning:{}\n  weight = weight\n  media_only = media_only\n{reads}",
            pin(callee)
        );
        let path = t.path().join("caller.rule").to_string_lossy().into_owned();
        std::fs::write(&path, &src).unwrap();
        let ds = i18n::with(Lang::En, || rulec::check_source(&src, &path));
        (path, src, ds)
    };
    let (_, _, ds) = caller("price_callee.rule", "outputs\n  price : money[USDc]?  round down(1USDc)\n", "");
    let found: Vec<(&str, String)> = ds.iter().map(|d| (d.code, d.title.clone())).collect();
    assert_eq!(found, vec![("E044", "`k` cannot be applied: the callee's output `price` may have no value and declares a rounding".to_string())]);
    for (outputs, reads, out, present) in [
        ("outputs\n  kind : service?\n", "", "kind", "media"),
        ("outputs\n  lane : lane?\n", "  kind -> lane with media -> fast_lane, library -> slow_lane\n", "lane", "fast_lane"),
    ] {
        let (path, src, ds) = caller("kind_callee.rule", outputs, reads);
        assert!(ds.iter().all(|d| d.severity != rulec::diag::Severity::Error), "{out}: {:?}", ds.iter().map(|d| (d.code, d.title.clone())).collect::<Vec<_>>());
        let (f, c) = rulec::prepare(&src, &path).unwrap_or_else(|_| panic!("{out}: does not read"));
        let at = |weight: i128, media: bool| {
            let ins = [("weight".to_string(), Val::Num(Rat::int(weight))), ("media_only".to_string(), Val::Bool(media))].into_iter().collect();
            rulec::eval::run_all(&f, &c, ins).0
        };
        assert_eq!(at(1, false), vec![(out.to_string(), Some(Val::Enum("none".into())))], "{out}");
        assert_eq!(at(1, true), vec![(out.to_string(), Some(Val::Enum(present.into())))], "{out}");
    }
}
