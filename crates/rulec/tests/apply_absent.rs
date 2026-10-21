//! E043 for a value that may be absent, passed by an `apply` to an input of the callee that is not
//! optional, and `with none -> <value>`, which reads the absent value as one of the callee's
//! (§15.255).
//!
//! The callee's completeness was proved where its input has a value, and it has no answer for the
//! absent one. An `apply` that passed `tier?` to `tier`, or `number?` to `number`, used to stop at
//! E101 in a table of the callee's, with a row to add there — a row this rule cannot add, in a file
//! approved on its own. E043 now says where the absent value comes from — an input or a field of
//! an element declared `T?`, a row of a table of this rule that answers `none`, a definition that
//! names one — and what this rule can write: read it as a value of the callee's with `with`,
//! declare the name as one that always has a value, or decide one in a table of this rule that
//! answers `none` too. A table whose output may be absent by its type and never is passes as it
//! did.
//!
//! The materials are in `tests/apply_absent`, English first, with a Japanese version beside each:
//! four callees (a tier and a quantity that are always there, a tier that may be absent, the lines
//! of an order walked by their tier), mutants that each pass the absent value one way, two that
//! write `with none` where it reads nothing, and a rule that passes the absent value in each way the
//! callees take it, with examples. What `rulec check` prints for a mutant, in English and in
//! Japanese, is its golden file in `tests/golden/apply_absent` (`RULEC_BLESS=1 cargo test --test
//! apply_absent` writes them again).

use ritsu_testkit::{Need, TempDir, ready, skip};
use rulec::i18n::{self, Lang};
use rulec::json::Json;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The mutants: the English rule, its Japanese version, the one code it stops at, and what the
/// English report says. Each of the E043 ones stopped at E101 before; each of the E042 ones at E012
/// in the expanded table, with E042 beside it.
const MUTANTS: &[(&str, &str, &str, &str)] = &[
    // An input that may be absent
    ("E043_a_tier_that_may_be_absent", "E043_値の無いことがある段階", "E043", "`tier = member with none -> <value>`"),
    // A number: no `with` reads it
    ("E043_a_quantity_that_may_be_absent", "E043_値の無いことがある数量", "E043", "If it always has a value, declare `pieces` as number. If it can be absent, the callee's `quantity` has to take the absent value"),
    // A row of this rule's table that answers `none`
    ("E043_a_table_that_answers_none", "E043_none_を書く表", "E043", "row 1 of table lift_by_member answers none"),
    // A definition that names an input that may be absent
    ("E043_a_definition_of_a_tier_that_may_be_absent", "E043_値の無いことがある段階の定義", "E043", "`repeat_tier` names `member`, which is tier?"),
    // A field of the elements a callee walks
    ("E043_a_field_that_may_be_absent", "E043_値の無いことがあるフィールド", "E043", "The field `level` may be absent, and the callee's field `level` takes no absent value"),
    // `with none` for a value that is always there
    ("E042_with_none_of_a_tier_always_there", "E042_いつも値のある段階の_with_none", "E042", "`member` is tier, which is never absent"),
    // `with none` where the callee takes the absent value itself
    ("E042_with_none_where_the_callee_takes_it", "E042_元の規則が受け付ける_with_none", "E042", "leave `none` out of `with`"),
];

/// The rules that pass: the callees, and the rule that passes the absent value each way they take it.
const PASSING: [(&str, &str); 5] = [
    ("tier_credit", "段階の値引き"),
    ("quantity_fee", "数量の手数料"),
    ("optional_tier_credit", "値の無いこともある段階の値引き"),
    ("line_points", "明細のポイント"),
    ("absent_tiers_read", "値の無い段階を読む規則"),
];

fn source(stem: &str) -> (String, String) {
    let rel = format!("tests/apply_absent/{stem}.rule");
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

/// Each mutant stops at its one finding, says what the case is, and prints what its golden file
/// holds in both languages.
#[test]
fn each_mutant_stops_at_its_one_finding() {
    let mut names: Vec<String> = std::fs::read_dir(root().join("tests/apply_absent")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    let mut listed: Vec<String> = MUTANTS
        .iter()
        .flat_map(|(en, ja, ..)| [format!("{en}.rule"), format!("{ja}.rule")])
        .chain(PASSING.iter().flat_map(|(en, ja)| [format!("{en}.rule"), format!("{ja}.rule")]))
        .collect();
    listed.sort();
    assert_eq!(names, listed, "every rule of tests/apply_absent is listed here, with its Japanese version");
    let mut failures = Vec::new();
    for (en, ja, code, said) in MUTANTS {
        for stem in [en, ja] {
            let (rel, src) = source(stem);
            let codes: Vec<String> = found(&src, &rel).into_iter().map(|(c, _)| c).collect();
            if codes != [*code] {
                failures.push(format!("{rel}: {codes:?}, not one {code}"));
            }
            for (lang, tag) in [(Lang::En, "en"), (Lang::Ja, "ja")] {
                let out = printed(&rel, &src, lang);
                if let Err(e) = ritsu_testkit::golden::check(&root().join(format!("tests/golden/apply_absent/{stem}.{tag}.txt")), &out) {
                    failures.push(e);
                }
            }
        }
        let (rel, src) = source(en);
        let out = printed(&rel, &src, Lang::En);
        if !out.contains(said) {
            failures.push(format!("{rel} does not say `{said}`:\n{out}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The Japanese version is the same rule with other names: the same findings on the same lines.
#[test]
fn the_japanese_version_says_the_same_things_in_the_same_places() {
    for (en, ja) in MUTANTS.iter().map(|(en, ja, ..)| (*en, *ja)).chain(PASSING) {
        let (re, se) = source(en);
        let (rj, sj) = source(ja);
        assert_eq!(found(&se, &re), found(&sj, &rj), "{en} and {ja}");
    }
}

/// The absent value passes where a rule takes it: read as basic through `with`, decided in a table
/// of this rule that answers `none` too, out of a table whose output may be absent by its type and
/// never is, handed on to a callee that takes it itself. The callees pass on their own, and the
/// examples hold — the absent value read as basic answers basic's credit.
#[test]
fn the_absent_value_passes_where_a_rule_takes_it() {
    for (en, ja) in PASSING {
        for stem in [en, ja] {
            let (rel, src) = source(stem);
            let ds = i18n::with(Lang::En, || rulec::check_source(&src, &rel));
            assert!(ds.is_empty(), "{rel}: {:?}", ds.iter().map(|d| (d.code, d.title.clone())).collect::<Vec<_>>());
        }
    }
}

/// Nothing is made from a mutant: the generated code and ritsu's port read the rule `prepare`
/// gives, and the finding leaves none.
#[test]
fn a_mutant_is_prepared_for_nothing() {
    for (en, ja, code, _) in MUTANTS {
        for stem in [en, ja] {
            let (rel, src) = source(stem);
            let codes: Vec<&str> = match i18n::with(Lang::En, || rulec::prepare(&src, &rel)) {
                Ok(_) => panic!("{rel} was prepared"),
                Err(ds) => ds.iter().map(|d| d.code).collect(),
            };
            assert!(codes.contains(code), "{rel}: {codes:?}");
        }
    }
}

fn rulec(tmp: &Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").env("TMPDIR", tmp).current_dir(root()).args(args).output().expect("cannot start rulec");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// The generated code of every language answers the absent value as `with` reads it, as the
/// reference evaluator does: the vectors ask for the member's tier absent, and every language
/// that runs gives the same answers. The English rule only: in the Japanese version the callee is
/// applied three times, and the names it brings in take the callee's ASCII alias rather than the
/// apply's, so they meet in the generated code (DESIGN §15.255, found and left).
#[test]
fn every_language_reads_the_absent_value_as_with_says() {
    if !ready(Need::Python, || Command::new("python3").arg("--version").output().is_ok_and(|o| o.status.success()), "python3 is not here") {
        return;
    }
    for (stem, alias, member) in [(PASSING[4].0, "absent_tiers_read", "member")] {
        let t = TempDir::new("apply-absent");
        let out = t.path().join("out");
        let tmp = t.path().join("tmp");
        std::fs::create_dir_all(&tmp).unwrap();
        let (code, said, e) = rulec(&tmp, &["gen", &format!("tests/apply_absent/{stem}.rule"), "--out", &out.to_string_lossy()]);
        assert_eq!(code, 0, "{stem}: {said}{e}");
        let vectors = std::fs::read_to_string(out.join("vectors").join(format!("{alias}.jsonl"))).unwrap();
        assert!(vectors.contains(&format!("\"{member}\":null")), "{stem}: no vector asks for the member's tier absent:\n{vectors}");
        let (_, said, e) = rulec(&tmp, &["test", &out.to_string_lossy(), "--format", "json"]);
        let j = rulec::json::parse(said.lines().next().unwrap_or_else(|| panic!("no result\n{e}"))).unwrap_or_else(|x| panic!("{x}: {said}"));
        for why in j.get("skipped").and_then(Json::as_arr).unwrap_or_default() {
            skip(&format!("rulec test: {}", why.as_str().unwrap_or_default()));
        }
        let mut ran = Vec::new();
        for r in j.get("results").and_then(Json::as_arr).unwrap_or_default() {
            if r.get("rule").and_then(Json::as_str) != Some(alias) || r.get("ran").and_then(|v| v.as_bool()) != Some(true) {
                continue;
            }
            assert_eq!(r.get("ok").and_then(|v| v.as_bool()), Some(true), "{stem}: {r:?}");
            ran.push(format!("{}/{}", r.get("lang").and_then(Json::as_str).unwrap_or_default(), r.get("via").and_then(Json::as_str).unwrap_or_default()));
        }
        assert!(ran.iter().any(|x| x.starts_with("python/")), "{stem}: Python did not run: {ran:?}");
        let broken: Vec<String> = j
            .get("results")
            .and_then(Json::as_arr)
            .unwrap_or_default()
            .iter()
            .filter(|r| r.get("rule").and_then(Json::as_str) == Some(alias) && r.get("ok").and_then(|v| v.as_bool()) == Some(false))
            .map(|r| format!("{}/{}", r.get("lang").and_then(Json::as_str).unwrap_or_default(), r.get("via").and_then(Json::as_str).unwrap_or_default()))
            .collect();
        assert!(broken.is_empty(), "{stem}: these did not run or did not agree: {broken:?}");
        println!("apply_absent: {stem} read the absent value as with says in {}", ran.join(", "));
    }
}
