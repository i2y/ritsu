//! One name, one declaration (§15.262).
//!
//! A rule that declared one input twice passed `rulec check`: `weight : number range >=0 <=9`
//! and then `weight : number range >=0 <=99` was `ok`, the check read the second line, the
//! `<=9` the author wrote was gone, and `rulec schema` wrote `weight` twice, `"required":
//! ["weight","weight"]`. Declared once as a number and once as a truth value, the second won and a
//! cell was told it was in a column of truth values. `enum e(e) = v1 | v1` passed too. The same
//! held for most lists of names a rule declares — the outputs, the definitions, the enums, the
//! groups, the shapes — and for two names given one alias, which the generated code then declared
//! twice. Nothing said anything; whatever read the name read whichever declaration it met.
//!
//! A file's own lines are now held to one declaration per name before anything reads one
//! (`names::check`): the second of two is E034, with the first marked, and the check stops there.
//! So are the identifiers the generated code writes, the columns of a heading, a group named like a
//! value of an enum, and an input a contribution adds; a fold's answer and the name an `apply`
//! gives a callee output are held under their own codes (E021, E041).
//!
//! The materials are in `tests/names`, English first, with a Japanese version beside each: one
//! rule for each way a name is written twice, the files two of them need (a callee, a contract),
//! and three rules whose names meet only where nothing reads them two ways, which pass. What
//! `rulec check` prints for each is its golden file in `tests/golden/names` (`RULEC_BLESS=1 cargo
//! test --test names` writes them again).

use ritsu_testkit::tmp::tmpdir_in;
use ritsu_testkit::{Need, TempDir, golden, ready, skip};
use rulec::i18n::{self, Lang};
use rulec::json::Json;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// One rule for each way a name is written twice, the English first and the Japanese version beside
/// it, with the one code each is said with.
const TWICE: &[(&str, &str, &str)] = &[
    // the values
    ("input_declared_twice", "入力を二度宣言", "E034"),
    ("input_of_two_types", "型の違う二つの入力", "E034"),
    ("definition_named_like_an_input", "入力と同じ名前の定義", "E034"),
    ("table_decides_a_definition", "定義と同じ名前を決める表", "E034"),
    ("output_named_like_an_input", "入力と同じ名前の出力", "E034"),
    ("two_outputs_of_one_name", "同じ名前の二つの出力", "E034"),
    ("two_summaries_of_one_name", "同じ名前の二つの集計", "E034"),
    ("open_named_like_a_definition", "定義と同じ名前の開いた値", "E034"),
    // the identifiers of the values
    ("two_inputs_one_alias", "同じ別名の二つの入力", "E034"),
    ("alias_is_another_name", "ほかの名前と同じ別名", "E034"),
    ("one_name_two_aliases", "二つの別名を書いた名前", "E034"),
    // the other lists
    ("enum_value_twice", "列挙の値を二度", "E034"),
    ("two_enums_of_one_name", "同じ名前の二つの列挙", "E034"),
    ("two_groups_of_one_name", "同じ名前の二つのグループ", "E034"),
    ("group_lists_a_value_twice", "値を二度書いたグループ", "E034"),
    ("group_named_like_a_value", "値と同じ名前のグループ", "E034"),
    ("two_shapes_of_one_name", "同じ名前の二つのshape", "E034"),
    ("two_applies_of_one_name", "同じ名前の二つの準用", "E034"),
    ("two_tables_of_one_alias", "同じ別名の二つの表", "E034"),
    // the `enums` files
    ("enum_name_is_an_alias", "ほかの列挙の別名と同じ名前", "E034"),
    ("two_blocks_of_one_alias", "同じ別名の二つの前提", "E034"),
    // the headings
    ("column_twice_in_a_heading", "見出しに二度書いた列", "E034"),
    ("output_column_twice", "見出しに二度書いた出力", "E034"),
    ("example_column_twice", "見出しに二度書いた例", "E034"),
    ("scenario_column_twice", "見出しに二度書いた手順の例", "E055"),
    ("sequence_column_twice", "見出しに二度書いた並び", "E026"),
    // under their own codes
    ("fold_answer_named_like_a_sequence", "並びと同じ名前の答え", "E021"),
    ("applied_output_named_like_a_field", "フィールドと同じ名前の準用の出力", "E041"),
    ("applied_output_renamed_twice", "二度名前を付けた準用の出力", "E041"),
];

/// Rules whose names meet only where nothing reads them two ways, which pass: an alias that is the
/// name of another input that has an alias of its own, and two tables that decide one output, one
/// writing the output's alias. And the callee the applying rules read.
const PASS: &[(&str, &str)] = &[
    ("alias_meets_another_name", "ほかの名前と同じ別名でも別名があれば通る"),
    ("one_definition_of_two_tables", "二つの表で決める一つの出力"),
    ("weight_is_big", "重さが大きい"),
];

fn source(stem: &str) -> (String, String) {
    let rel = format!("tests/names/{stem}.rule");
    let src = std::fs::read_to_string(root().join(&rel)).unwrap_or_else(|_| panic!("cannot read {rel}"));
    (rel, src)
}

/// What `rulec check <rel>` prints, in `lang`, and the codes of what it found.
fn printed(rel: &str, src: &str, lang: Lang) -> (String, Vec<&'static str>) {
    let lines: Vec<String> = src.lines().map(String::from).collect();
    i18n::with(lang, || {
        let r = rulec::report(src, rel);
        let text = format!("{}{}", rulec::findings_text(&r.diags, &lines), rulec::check_tail(&r.shadow, &r.diags, rel, 0));
        (text, r.diags.iter().map(|d| d.code).collect())
    })
}

/// Each way of writing a name twice is said once, at the second, with the code decided for it, and
/// nothing else is said; the English and the Japanese wording are their golden files.
#[test]
fn each_name_written_twice_is_said_once_at_the_second() {
    for (en, ja, code) in TWICE {
        for (stem, lang, tag) in [(en, Lang::En, "en"), (ja, Lang::Ja, "ja")] {
            let (rel, src) = source(stem);
            let (text, codes) = printed(&rel, &src, lang);
            assert_eq!(codes, vec![*code], "{rel}:\n{text}");
            golden(root().join(format!("tests/golden/names/{stem}.{tag}.txt")), &text);
        }
    }
}

/// The finding is about the second declaration: it is said at its line, and its last mark is there.
#[test]
fn the_second_declaration_is_where_it_is_said() {
    for (en, ja, _) in TWICE {
        for stem in [en, ja] {
            let (rel, src) = source(stem);
            let ds = i18n::with(Lang::En, || rulec::report(&src, &rel).diags);
            let d = &ds[0];
            let line: usize = d.where_.split(':').nth(1).and_then(|t| t.split(' ').next()).and_then(|n| n.parse().ok()).unwrap_or(0);
            let last = d.marks.last().map(|m| m.span.line).unwrap_or(0);
            assert_eq!(line, last, "{rel}: said at line {line}, the second marked at line {last}");
            assert!(d.marks.iter().all(|m| m.span.line <= line), "{rel}: a mark below the second: {:?}", d.marks.iter().map(|m| m.span.line).collect::<Vec<_>>());
        }
    }
}

fn rulec(cwd: &Path, tmp: &Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").env("TMPDIR", tmp).current_dir(cwd).args(args).output().expect("cannot start rulec");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// Nothing is written from a rule that writes a name twice: `schema` wrote `weight` twice, and `gen`
/// wrote functions that took one parameter twice.
#[test]
fn nothing_is_written_from_a_rule_that_writes_a_name_twice() {
    let t = TempDir::new("names-written");
    let tmp = tmpdir_in(t.path());
    let dir = root().join("tests/names");
    for (en, ja, _) in TWICE {
        for stem in [en, ja] {
            let file = format!("{stem}.rule");
            let enums = std::fs::read_to_string(dir.join(&file)).unwrap().starts_with("enums ");
            let commands: &[&str] = if enums { &["api"] } else { &["schema", "api"] };
            for cmd in commands {
                let (code, out, _) = rulec(&dir, &tmp, &[cmd, &file]);
                assert_eq!(code, 1, "rulec {cmd} {file} answered:\n{out}");
            }
            let out = t.path().join(format!("gen-{stem}"));
            let (code, said, _) = rulec(&dir, &tmp, &["gen", &file, "--out", &out.to_string_lossy()]);
            assert_eq!(code, 1, "rulec gen {file}:\n{said}");
            assert!(!out.exists() || std::fs::read_dir(&out).unwrap().next().is_none(), "rulec gen {file} wrote something");
        }
    }
}

fn have_python() -> bool {
    Command::new("python3").arg("--version").output().is_ok_and(|o| o.status.success())
}

/// The rules whose names meet only where nothing reads them two ways pass, and their generated code
/// answers as the reference evaluator does in every language that is here: an alias that is the
/// name of another input is not the other's identifier when that one has an alias of its own.
#[test]
fn names_that_meet_where_nothing_reads_them_two_ways_pass_and_run_alike() {
    for (en, ja) in PASS {
        for (stem, lang) in [(en, Lang::En), (ja, Lang::Ja)] {
            let (rel, src) = source(stem);
            let (text, codes) = printed(&rel, &src, lang);
            assert!(codes.is_empty(), "{rel}:\n{text}");
        }
    }
    if !ready(Need::Python, have_python, "python3 is not here") {
        return;
    }
    let dir = root().join("tests/names");
    for ja in [false, true] {
        let t = TempDir::new("names-run");
        let out = t.path().join("out");
        let tmp = tmpdir_in(t.path());
        let mut args: Vec<String> = vec!["gen".into()];
        args.extend(PASS.iter().map(|(en, j)| format!("{}.rule", if ja { j } else { en })));
        args.extend(["--out".into(), out.to_string_lossy().into_owned()]);
        let (code, said, e) = rulec(&dir, &tmp, &args.iter().map(String::as_str).collect::<Vec<_>>());
        assert_eq!(code, 0, "{said}{e}");
        let (_, said, e) = rulec(&dir, &tmp, &["test", &out.to_string_lossy(), "--format", "json"]);
        let j = rulec::json::parse(said.lines().next().unwrap_or_else(|| panic!("no result\n{e}"))).unwrap_or_else(|x| panic!("{x}: {said}"));
        for why in j.get("skipped").and_then(Json::as_arr).unwrap_or_default() {
            skip(&format!("rulec test: {}", why.as_str().unwrap_or_default()));
        }
        let results = j.get("results").and_then(Json::as_arr).unwrap_or_default();
        let field = |r: &Json, k: &str| r.get(k).and_then(Json::as_str).unwrap_or_default().to_string();
        let broken: Vec<String> = results
            .iter()
            .filter(|r| r.get("ok").and_then(|v| v.as_bool()) != Some(true))
            .map(|r| format!("{} {}/{}: {}", field(r, "rule"), field(r, "lang"), field(r, "via"), field(r, "error")))
            .collect();
        assert!(broken.is_empty(), "these did not run or did not agree:\n{}", broken.join("\n"));
        let mut ran: Vec<String> = results.iter().filter(|r| r.get("ran").and_then(|v| v.as_bool()) == Some(true)).map(|r| field(r, "lang")).collect();
        ran.sort();
        ran.dedup();
        assert!(ran.iter().any(|l| l == "python"), "Python did not run: {ran:?}");
        println!("names: the {} rules that pass ran alike in {}", if ja { "Japanese" } else { "English" }, ran.join(", "));
    }
}

/// An input a contribution adds is the rule's input of its name; under a name the rule declares as
/// another value — a table's output here — it would be one name for two values of the merged rule.
/// The English rule and its Japanese version, each with its contribution, pinned as `rulec source
/// pin` writes the pin.
#[test]
fn an_input_a_contribution_adds_is_held_to_the_names_of_the_rule() {
    let cases = [
        (
            "rule site_extra v1\n\ninputs\n  shelving : bool\n\noutputs\n  extra : number  round down(1)\n\ntable from_shelving\npolicy unique\n| shelving | -> shelving_extra : number |\n| true     | 10                         |\n| false    | 0                          |\n\nopen extra : number  combine sum  range >=0 <=100\n  shelving_extra\n",
            "contribution mezzanine v1\nto \"../site.rule\" signature:0000000000000000\n\ninputs\n  shelving_extra : bool\n\ntable from_mezzanine\npolicy unique\n| shelving_extra | -> mezzanine_extra : number |\n| true           | 15                          |\n| false          | 0                           |\n\ninto extra\n  mezzanine_extra\n",
            Lang::En,
            "The contribution's input `shelving_extra` has the name of a table's output of the rule it adds to",
        ),
        (
            "rule 置き場の追加(site_extra) v1\n\ninputs\n  棚(shelving) : bool\n\noutputs\n  追加(extra) : number  round down(1)\n\ntable 棚の分(from_shelving)\npolicy unique\n| 棚    | -> 棚の追加 : number |\n| true  | 10                   |\n| false | 0                    |\n\nopen 追加 : number  combine sum  range >=0 <=100\n  棚の追加\n",
            "contribution 中二階(mezzanine) v1\nto \"../site.rule\" signature:0000000000000000\n\ninputs\n  棚の追加(shelving_extra) : bool\n\ntable 中二階の分(from_mezzanine)\npolicy unique\n| 棚の追加 | -> 中二階の追加 : number |\n| true     | 15                       |\n| false    | 0                        |\n\ninto 追加\n  中二階の追加\n",
            Lang::Ja,
            "寄与の入力 `棚の追加` は、足す先の規則の表の出力と同じ名前です",
        ),
    ];
    for (rule, contribution, lang, title) in cases {
        let t = TempDir::new("names-contribution");
        let tmp = tmpdir_in(t.path());
        std::fs::create_dir_all(t.path().join("contrib")).unwrap();
        std::fs::write(t.path().join("site.rule"), rule).unwrap();
        std::fs::write(t.path().join("contrib/mezzanine.rule"), contribution).unwrap();
        let (code, said, e) = rulec(t.path(), &tmp, &["source", "pin", "contrib/mezzanine.rule"]);
        assert_eq!(code, 0, "{said}{e}");
        let (code, alone, _) = rulec(t.path(), &tmp, &["check", "site.rule"]);
        assert_eq!(code, 0, "the rule alone passes:\n{alone}");
        let tag = if lang == Lang::Ja { "ja" } else { "en" };
        for files in [vec!["site.rule", "contrib/"], vec!["contrib/mezzanine.rule"]] {
            let mut args = vec!["check", "--format", "json", "--lang", tag];
            args.extend(files.iter().copied());
            let (code, said, e) = rulec(t.path(), &tmp, &args);
            assert_eq!(code, 1, "{said}{e}");
            let found: Vec<(String, String)> = said
                .lines()
                .filter_map(|l| rulec::json::parse(l).ok())
                .map(|j| (j.get("code").and_then(Json::as_str).unwrap_or_default().to_string(), j.get("title").and_then(Json::as_str).unwrap_or_default().to_string()))
                .collect();
            assert_eq!(found, vec![("E034".to_string(), title.to_string())], "{files:?}");
        }
    }
}

/// `import enum` finds an enum by its name or by its alias, so an `enums` file whose enum is named
/// like another's alias does not pass, and a rule that reads one of its enums is told so (E066).
#[test]
fn an_enums_file_with_a_name_that_is_an_alias_is_read_by_no_rule() {
    let t = TempDir::new("names-enums");
    let tmp = tmpdir_in(t.path());
    std::fs::copy(root().join("tests/names/enum_name_is_an_alias.rule"), t.path().join("zones.rule")).unwrap();
    std::fs::write(
        t.path().join("fee.rule"),
        "rule fee v1\n\nimport enum band from \"zones.rule\"\n\ninputs\n  dest : band\n\noutputs\n  far : bool\n\ntable decide\npolicy unique\n| dest  | -> far |\n| inner | false  |\n| outer | true   |\n",
    )
    .unwrap();
    let (code, said, _) = rulec(t.path(), &tmp, &["check", "fee.rule"]);
    assert_eq!(code, 1, "{said}");
    assert!(said.contains("error[E066]") && said.contains("(E034)"), "{said}");
}

/// Every `.rule` under the workspace's crates and site, by its path from the workspace's root.
fn tree_rules() -> Vec<String> {
    fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            let name = e.file_name().to_string_lossy().into_owned();
            if p.is_dir() {
                if !matches!(name.as_str(), "target" | "node_modules" | ".venv" | ".lake" | ".git") {
                    walk(&p, base, out);
                }
            } else if name.ends_with(".rule") {
                out.push(p.strip_prefix(base).unwrap().to_string_lossy().into_owned());
            }
        }
    }
    let base = root().join("../..").canonicalize().unwrap();
    let mut out = Vec::new();
    for d in ["crates", "website"] {
        walk(&base.join(d), &base, &mut out);
    }
    out.sort();
    out
}

/// No rule of the tree writes a name twice, but the materials here and the mutants seeded with one:
/// what the check now stops was never written on purpose.
#[test]
fn no_rule_of_the_tree_writes_a_name_twice() {
    let base = root().join("../..").canonicalize().unwrap();
    let mut seen = 0usize;
    let mut found: Vec<String> = Vec::new();
    for rel in tree_rules() {
        if rel.starts_with("crates/rulec/tests/names/") || rel.starts_with("crates/rulec/tests/mutants/m_e034") {
            continue;
        }
        let path = base.join(&rel);
        let Ok(src) = std::fs::read_to_string(&path) else { continue };
        let shown = path.to_string_lossy().into_owned();
        let parsed = i18n::with(Lang::En, || rulec::parse::parse(&src, &shown));
        found.extend(parsed.diags.iter().filter(|d| d.title.starts_with("The heading has the column")).map(|d| format!("{rel}: {}", d.title)));
        let Some(f) = parsed.file else { continue };
        seen += 1;
        found.extend(i18n::with(Lang::En, || rulec::names::check(&f, &shown)).iter().map(|d| format!("{rel}: {}", d.title)));
    }
    assert!(found.is_empty(), "{}", found.join("\n"));
    assert!(seen >= 600, "fewer rules of the tree were read than there are: {seen}");
}
