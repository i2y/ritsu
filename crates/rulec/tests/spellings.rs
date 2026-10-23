//! Two names the author wrote that some language rulec writes spells as one identifier, and a name
//! some language cannot take where its code puts it (§15.263).
//!
//! `define base_fee` and `define baseFee` passed `rulec check`, and the Swift and the Java rulec
//! wrote for them declared `baseFee` twice and did not compile. So it went with two names SQLite
//! reads as one column (`afee` and `aFee`: the query answered with the wrong one), with two names
//! PostgreSQL cuts to one at 63 bytes, with two inputs Go's `Input` struct and the `.proto` spell
//! alike, and with an output called `READ_CREDIT`, which Ruby reads as a constant. The check held
//! the names to one another as written, and every generator spells them its own way.
//!
//! The check now holds the author's identifiers to the spellings of the languages: W121 names the
//! two names, the languages that write them as one, the identifier each writes, and why; a name a
//! language cannot take is said with that language. The identifiers stay as the author wrote them.
//! Where every language that writes a kind of name spells two as one — the types of two enums, two
//! values of one enum, the functions of two prerequisites blocks, the element types of two
//! sequences, a shape and an input of one identifier — the check stops at them with E034, and their
//! materials are with the other names written twice, in `tests/names`.
//!
//! The materials are in `tests/spellings`, English first, with a Japanese version beside each: one
//! rule for each way two names meet, and for each name a language cannot take. What `rulec check`
//! prints for each is its golden file in `tests/golden/spellings` (`RULEC_BLESS=1 cargo test --test
//! spellings` writes them again). Their generated code answers as the reference evaluator does in
//! every language the warning does not name; and given an alias where the warning says, it does in
//! every language, as do the rules of `tests/names` that E034 stops at.

use ritsu_testkit::tmp::tmpdir_in;
use ritsu_testkit::{Need, TempDir, ready, side_by_side, skip};
use rulec::i18n::{self, Lang};
use rulec::json::Json;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// One rule for each way two names meet, and for each name a language cannot take, the English first
/// and the Japanese version beside it, with the languages the warning names, as its English title
/// lists them.
const FORMS: &[(&str, &str, &str)] = &[
    // the values of one function, and the fields of the structs, the records and the messages
    ("values_in_camel_case", "キャメルケースで重なる定義", "Swift and Java"),
    ("inputs_spelled_alike", "綴りが重なる入力", "Go, Swift, Java and Connect"),
    ("outputs_spelled_alike", "綴りが重なる出力", "Go, Swift, Java and Connect"),
    ("fields_spelled_alike", "綴りが重なるフィールド", "Go, Swift, Java and Connect"),
    ("input_and_sequence_in_go", "下線だけ違う入力と並び", "Go and Connect"),
    ("outputs_with_a_trailing_underscore", "下線だけ違う二つの出力", "Go and Connect"),
    ("java_keyword_escape", "Javaの予約語と下線", "Java"),
    ("php_variable_escape", "PHPの変数と下線", "PHP"),
    // the columns of the SQL query
    ("values_sqlite_reads_as_one", "大文字と小文字だけ違う定義", "SQL (SQLite)"),
    ("tables_sqlite_reads_as_one", "大文字と小文字だけ違う表", "SQL (SQLite)"),
    ("table_and_value_sqlite", "表の行の列と大文字と小文字だけ違う定義", "SQL (SQLite)"),
    ("values_postgresql_cuts_to_one", "先頭の63バイトが同じ二つの定義", "SQL (PostgreSQL)"),
    // the enums, the groups and the prerequisites blocks
    ("enum_types_differ_in_case", "大文字と小文字だけ違う列挙", "PHP and Java"),
    ("enum_values_in_upper_case", "大文字にすると重なる列挙の値", "Python, TypeScript, JavaScript, Ruby, PHP, Java and Connect"),
    ("enum_values_in_swift", "大文字の並びだけ違う列挙の値", "Python, TypeScript, JavaScript, Ruby, PHP, Swift, Java and Connect"),
    ("go_members_across_enums", "Goで重なる二つの列挙の値", "Go"),
    ("groups_spelled_alike", "綴りが重なるグループ", "Go and Swift"),
    ("blocks_in_camel_case", "キャメルケースで重なる前提のつながり", "TypeScript, JavaScript, Go, Swift and Java"),
    // a name a language cannot take
    ("ruby_constant", "大文字で始まる定義", "Ruby"),
    ("ruby_constant_shape", "大文字で始まるshape", "Ruby"),
    ("python_underscore_output", "下線で始まる出力", "Python"),
    ("python_underscore_field", "下線で始まるフィールド", "Python"),
    ("postgresql_long_output", "63バイトより長い出力", "SQL (PostgreSQL)"),
    ("postgresql_long_table", "63バイトより長い表", "SQL (PostgreSQL)"),
];

/// The rules of `tests/names` that every language spells two names of as one, which E034 stops at
/// (`tests/names.rs` holds what is said for each).
const SPELLED: &[(&str, &str)] = &[
    ("enum_types_of_one_name", "型の名前が重なる二つの列挙"),
    ("enum_values_of_one_name", "名前が重なる列挙の値"),
    ("blocks_of_one_name", "関数の名前が重なる前提のつながり"),
    ("sequences_of_one_element_type", "要素の型が重なる二つの並び"),
    ("shape_named_like_an_input", "入力と同じ名前のshape"),
];

fn source(dir: &str, stem: &str) -> (String, String) {
    let rel = format!("tests/{dir}/{stem}.rule");
    let src = std::fs::read_to_string(root().join(&rel)).unwrap_or_else(|_| panic!("cannot read {rel}"));
    (rel, src)
}

/// What `rulec check <rel>` prints, in `lang`, and what it found.
fn printed(rel: &str, src: &str, lang: Lang) -> (String, Vec<rulec::diag::Diag>) {
    let lines: Vec<String> = src.lines().map(String::from).collect();
    i18n::with(lang, || {
        let r = rulec::report(src, rel);
        let text = format!("{}{}", rulec::findings_text(&r.diags, &lines), rulec::check_tail(&r.shadow, &r.diags, rel, 0));
        (text, r.diags)
    })
}

/// The languages an English W121 of these forms names: the list after `generated for`, or the
/// one language of a name it cannot take.
fn named(title: &str) -> Option<String> {
    if let Some((_, l)) = title.split_once("are one identifier in the code generated for ") {
        return Some(l.to_string());
    }
    if let Some((_, l)) = title.split_once("which the code generated for ") {
        return l.strip_suffix(" cannot take").map(String::from);
    }
    title.contains("PostgreSQL cuts").then(|| "SQL (PostgreSQL)".to_string())
}

/// Every rule here is listed above, with its Japanese version.
#[test]
fn every_rule_here_is_one_of_the_forms() {
    let mut names: Vec<String> = std::fs::read_dir(root().join("tests/spellings"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".rule"))
        .collect();
    names.sort();
    let mut listed: Vec<String> = FORMS.iter().flat_map(|(en, ja, _)| [format!("{en}.rule"), format!("{ja}.rule")]).collect();
    listed.sort();
    assert_eq!(names, listed, "every rule of tests/spellings is listed in FORMS, with its Japanese version");
}

/// Each form is said once, by W121, with the languages it meets in, in both versions; the English
/// and the Japanese wording are their golden files.
#[test]
fn each_form_is_said_once_with_the_languages_it_meets_in() {
    let mut failures = Vec::new();
    for (en, ja, langs) in FORMS {
        for (stem, lang, tag) in [(*en, Lang::En, "en"), (*ja, Lang::Ja, "ja")] {
            let (rel, src) = source("spellings", stem);
            let (text, ds) = printed(&rel, &src, lang);
            let codes: Vec<&str> = ds.iter().map(|d| d.code).collect();
            if codes != ["W121"] {
                failures.push(format!("{rel}: {codes:?}\n{text}"));
                continue;
            }
            if let Err(e) = ritsu_testkit::golden::check(&root().join(format!("tests/golden/spellings/{stem}.{tag}.txt")), &text) {
                failures.push(e);
            }
            // the languages, said in English whichever version is read
            let (_, ds) = printed(&rel, &src, Lang::En);
            match named(&ds[0].title) {
                Some(l) if l == *langs => {}
                other => failures.push(format!("{rel}: names {other:?}, not {langs}: {}", ds[0].title)),
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// No rule of the tree meets another name in a language's spelling, or is a name a language cannot
/// take, but the materials here and the mutants seeded with one: what the check now says was never
/// written on purpose. The rules of `tests/names` are held to E034 by `tests/names.rs`.
#[test]
fn no_rule_of_the_tree_is_spelled_so_but_the_materials() {
    let base = root().join("../..").canonicalize().unwrap();
    let mut seen = 0usize;
    let mut found: Vec<String> = Vec::new();
    for rel in tree_rules() {
        if rel.starts_with("crates/rulec/tests/spellings/") || rel.starts_with("crates/rulec/tests/names/") || rel.starts_with("crates/rulec/tests/mutants/m_w121") {
            continue;
        }
        let path = base.join(&rel);
        let Ok(src) = std::fs::read_to_string(&path) else { continue };
        let shown = path.to_string_lossy().into_owned();
        seen += 1;
        let ds = i18n::with(Lang::En, || rulec::check_source(&src, &shown));
        found.extend(ds.iter().filter(|d| d.code == "W121" && named(&d.title).is_some()).map(|d| format!("{rel}: {}", d.title)));
    }
    assert!(found.is_empty(), "{}", found.join("\n"));
    assert!(seen >= 700, "fewer rules of the tree were read than there are: {seen}");
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

/// The files of a rule and those beside it, written into a directory of their own.
fn written(files: &[(&str, &str)]) -> TempDir {
    let t = TempDir::new("spellings-files");
    for (name, text) in files {
        let p = t.path().join(name);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }
    t
}

/// What `rulec check` says of `args` in a directory, as (code, title) in `lang`.
fn found(dir: &Path, args: &[&str], lang: &str) -> Vec<(String, String)> {
    let tmp = tmpdir_in(dir);
    let mut all = vec!["check", "--format", "json", "--lang", lang];
    all.extend(args.iter().copied());
    let (_, said, _) = rulec(dir, &tmp, &all);
    said.lines()
        .filter_map(|l| rulec::json::parse(l).ok())
        .map(|j| (j.get("code").and_then(Json::as_str).unwrap_or_default().to_string(), j.get("title").and_then(Json::as_str).unwrap_or_default().to_string()))
        .collect()
}

/// An enum read with `import enum` is its file's type, and the rule's module names it so: an enum of
/// the rule whose type takes that name is E034, in English and in Japanese, and with an alias of its
/// own the rule passes.
#[test]
fn an_enum_read_with_import_enum_is_held_to_the_types_of_the_rule() {
    let tiers = "enums tiers v1\ndescription \"Tiers several rules share\"\n\nenum tierLevel = basic | gold\n";
    let rule = |own: &str| {
        format!(
            "rule imported_tier v1\ndescription \"An enum of its own beside one it reads\"\n\nimport enum tierLevel from \"tiers.rule\"\n\nenum {own} = low | high\n\ninputs\n  member : tierLevel\n  band   : tier_level\n\noutputs\n  fee : money[USD]  round down(1USD)\n\ntable fee_by_member\npolicy unique\n| member | band | -> fee |\n| basic  | -    | 1USD   |\n| gold   | low  | 2USD   |\n| gold   | high | 3USD   |\n"
        )
    };
    for (own, lang, want) in [
        ("tier_level", "en", Some("The types of enums `tierLevel` and `tier_level` take the one name `TierLevel` in the generated code")),
        ("tier_level", "ja", Some("列挙 `tierLevel` と `tier_level` の型が、生成するコードで同じ名前 `TierLevel` になります")),
        ("tier_level(band_level)", "en", None),
    ] {
        let t = written(&[("tiers.rule", tiers), ("fee.rule", &rule(own))]);
        let got = found(t.path(), &["fee.rule"], lang);
        match want {
            Some(title) => assert_eq!(got, vec![("E034".to_string(), title.to_string())], "{own} ({lang})"),
            None => assert!(got.is_empty(), "{own}: {got:?}"),
        }
    }
}

/// A value a contribution adds to an open enum is held to the members of the enum: `Web` beside
/// `web` is E034, said for the contribution, in English and in Japanese, and with an alias of its
/// own the rule and its contribution pass.
#[test]
fn a_value_a_contribution_adds_is_held_to_the_members_of_its_enum() {
    let rule = "rule channel_fee v1\ndescription \"A fee by channel, open to channels a contribution adds\"\n\nenum channel = web | store default  open\n\ninputs\n  via : channel\n\noutputs\n  fee : money[USD]  round down(1USD)\n\ntable fee_by_channel\npolicy first\n| via | -> fee |\n| web | 1USD   |\n| -   | 2USD   |\n";
    let contribution = |value: &str| format!("contribution app_channel v1\ndescription \"A channel for orders from the app\"\nto \"../channel_fee.rule\" signature:0000000000000000\n\nextend channel with {value} default\n");
    for (value, lang, want) in [
        ("Web", "en", Some("The value `Web` the contribution adds takes the name of the value `web` of the enum channel in the generated code")),
        ("Web", "ja", Some("寄与が足す値 `Web` は、列挙 channel の値 `web` と、生成するコードで同じ名前になります")),
        ("Web(app)", "en", None),
    ] {
        let t = written(&[("channel_fee.rule", rule), ("contrib/app.rule", &contribution(value))]);
        let tmp = tmpdir_in(t.path());
        let (code, said, e) = rulec(t.path(), &tmp, &["source", "pin", "contrib/app.rule"]);
        assert_eq!(code, 0, "{said}{e}");
        let got = found(t.path(), &["channel_fee.rule", "contrib/"], lang);
        match want {
            Some(title) => assert_eq!(got, vec![("E034".to_string(), title.to_string())], "{value} ({lang})"),
            None => assert!(got.is_empty(), "{value}: {got:?}"),
        }
    }
}

/// The rule with an alias given to the name the first finding marks last, as its last note says:
/// `baseFee(renamed_11_7)` for `define baseFee`, the alias written over when there is one. The alias
/// starts with a small letter, has no `_` at its head, and is short, so no language refuses it.
fn with_an_alias(src: &str, d: &rulec::diag::Diag) -> String {
    let span = &d.marks.last().expect("a finding with a mark").span;
    let mut lines: Vec<String> = src.lines().map(String::from).collect();
    let line = &mut lines[span.line - 1];
    let end = span.col + span.len;
    let alias = format!("(renamed_{}_{})", span.line, span.col);
    let rest = &line[end..];
    let after = match rest.strip_prefix('(').and_then(|r| r.find(')').map(|i| &r[i + 1..])) {
        Some(after) => after.to_string(),
        None => rest.to_string(),
    };
    *line = format!("{}{alias}{after}", &line[..end]);
    lines.join("\n") + "\n"
}

fn rulec(cwd: &Path, tmp: &Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").env("TMPDIR", tmp).current_dir(cwd).args(args).output().expect("cannot start rulec");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

fn have_python() -> bool {
    Command::new("python3").arg("--version").output().is_ok_and(|o| o.status.success())
}

/// One rule to generate and run: its file name, its text, and the languages whose results do not
/// count (those the warning names), by how `rulec test` reports a result.
struct Run {
    file: String,
    src: String,
    excused: String,
}

/// Whether a result of `rulec test` is of a language the warning names: `Connect` is the Python
/// service's passes, `SQL (SQLite)` the query's run and `SQL (PostgreSQL)` the function's, and a
/// language the rest of it, the rounding helpers too (Go's are in the rule's package).
fn excused(langs: &str, lang: &str, via: &str) -> bool {
    langs.replace(" and ", ", ").split(", ").any(|l| match l {
        "Connect" => lang == "python" && via.starts_with("connect"),
        "SQL (SQLite)" => lang == "sql" && via == "runner",
        "SQL (PostgreSQL)" => lang == "sql" && via == "function",
        other => lang == other.to_ascii_lowercase(),
    })
}

/// Generate the rules into one directory and run what was generated: every result that is of no
/// language a rule's warning names answers as the reference evaluator does, Python among them.
fn runs_alike(runs: &[Run], what: &str) {
    let t = TempDir::new("spellings");
    let tmp = tmpdir_in(t.path());
    let dir = t.path().join("rules");
    let out = t.path().join("out");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(root().join("tests/spellings/orders.schema.json"), dir.join("orders.schema.json")).unwrap();
    for r in runs {
        std::fs::write(dir.join(&r.file), &r.src).unwrap();
    }
    let mut args: Vec<&str> = vec!["gen"];
    args.extend(runs.iter().map(|r| r.file.as_str()));
    let o = out.to_string_lossy().into_owned();
    args.extend(["--out", o.as_str()]);
    let (code, said, e) = rulec(&dir, &tmp, &args);
    assert_eq!(code, 0, "{what}: {said}{e}");
    let (_, said, e) = rulec(&dir, &tmp, &["test", o.as_str(), "--format", "json"]);
    let j = rulec::json::parse(said.lines().next().unwrap_or_else(|| panic!("{what}: no result\n{e}"))).unwrap_or_else(|x| panic!("{what}: {x}: {said}"));
    for why in j.get("skipped").and_then(Json::as_arr).unwrap_or_default() {
        skip(&format!("rulec test: {}", why.as_str().unwrap_or_default()));
    }
    let results = j.get("results").and_then(Json::as_arr).unwrap_or_default();
    let field = |r: &Json, k: &str| r.get(k).and_then(Json::as_str).unwrap_or_default().to_string();
    // What one rule's warning names reaches the others of the directory in two ways: the `.proto`
    // files of every rule are one buf module, which buf builds whole for each rule's Connect pass,
    // and the rounding helpers run once for each language, Go's built in a rule's package.
    let any_excused = |lang: &str, via: &str| runs.iter().any(|r| excused(&r.excused, lang, via));
    let broken: Vec<String> = results
        .iter()
        .filter(|r| r.get("ok").and_then(|v| v.as_bool()) != Some(true))
        .filter(|r| {
            let (rule, lang, via) = (field(r, "rule"), field(r, "lang"), field(r, "via"));
            if via.starts_with("connect") && any_excused(&lang, &via) {
                return false;
            }
            match runs.iter().find(|x| x.src.lines().next().is_some_and(|l| rule_alias(l) == rule)) {
                Some(x) => !excused(&x.excused, &lang, &via),
                None => !(rule == "_round" && any_excused(&lang, &via)),
            }
        })
        .map(|r| format!("{} {}/{}: {}", field(r, "rule"), field(r, "lang"), field(r, "via"), field(r, "error")))
        .collect();
    assert!(broken.is_empty(), "{what}: these did not run or did not agree:\n{}", broken.join("\n"));
    let ran: Vec<String> = results.iter().filter(|r| r.get("ran").and_then(|v| v.as_bool()) == Some(true)).map(|r| field(r, "lang")).collect();
    assert!(ran.iter().any(|l| l == "python"), "{what}: Python did not run: {ran:?}");
}

/// The identifier of the rule a heading line declares: `rule 定額の送料(flat_fee) v1` is `flat_fee`.
fn rule_alias(heading: &str) -> String {
    let name = heading.strip_prefix("rule ").unwrap_or(heading).split(' ').next().unwrap_or_default();
    match name.split_once('(') {
        Some((_, a)) => a.trim_end_matches(')').to_string(),
        None => name.to_string(),
    }
}

/// The runs, dealt into as many directories as there are cores, the English rules in some and the
/// Japanese versions, which share their aliases, in others.
fn deal(runs: Vec<Run>) -> Vec<Vec<Run>> {
    let n = std::thread::available_parallelism().map_or(1, |n| n.get()).clamp(1, 4);
    let mut dirs: Vec<Vec<Run>> = (0..n).map(|_| Vec::new()).collect();
    for (i, r) in runs.into_iter().enumerate() {
        dirs[i % n].push(r);
    }
    dirs.retain(|d| !d.is_empty());
    dirs
}

/// The generated code of every rule here answers as the reference evaluator does in every
/// language its warning does not name: the warning names each language where the code it wrote
/// does not build or answers otherwise.
#[test]
fn the_generated_code_agrees_in_every_language_the_warning_does_not_name() {
    if !ready(Need::Python, have_python, "python3 is not here") {
        return;
    }
    for ja in [false, true] {
        let runs: Vec<Run> = FORMS
            .iter()
            .map(|(en, j, langs)| {
                let stem = if ja { j } else { en };
                let (_, src) = source("spellings", stem);
                Run { file: format!("{stem}.rule"), src, excused: langs.to_string() }
            })
            .collect();
        let dirs = deal(runs);
        side_by_side(&dirs, |runs| runs_alike(runs, if ja { "the Japanese versions" } else { "the English rules" }));
    }
}

/// With an alias given to the second of the two names, or to the name a language cannot take, as
/// the finding says, every rule here passes the check with nothing said, and its generated code
/// answers as the reference evaluator does in every language; so do the rules of `tests/names`
/// that every language spells two names of as one (E034).
#[test]
fn an_alias_where_the_finding_says_makes_every_language_agree() {
    if !ready(Need::Python, have_python, "python3 is not here") {
        return;
    }
    let mut failures = Vec::new();
    for ja in [false, true] {
        let mut runs: Vec<Run> = Vec::new();
        let forms = FORMS.iter().map(|(en, j, _)| ("spellings", *en, *j)).chain(SPELLED.iter().map(|(en, j)| ("names", *en, *j)));
        for (dir, en, j) in forms {
            let stem = if ja { j } else { en };
            let (rel, src) = source(dir, stem);
            let ds = i18n::with(Lang::En, || rulec::check_source(&src, &rel));
            let Some(d) = ds.first() else {
                failures.push(format!("{rel}: nothing is said"));
                continue;
            };
            let fixed = with_an_alias(&src, d);
            let after: Vec<String> = i18n::with(Lang::En, || rulec::check_source(&fixed, &rel)).iter().map(|d| format!("{}: {}", d.code, d.title)).collect();
            if !after.is_empty() {
                failures.push(format!("{rel} with an alias:\n{}\n{fixed}", after.join("\n")));
                continue;
            }
            runs.push(Run { file: format!("{stem}.rule"), src: fixed, excused: String::new() });
        }
        let dirs = deal(runs);
        side_by_side(&dirs, |runs| runs_alike(runs, if ja { "the Japanese versions with an alias" } else { "the English rules with an alias" }));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}
