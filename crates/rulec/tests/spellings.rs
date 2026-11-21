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
//! language cannot take is said with that language, and the warning's targets are what `rulec gen`
//! then refuses to write (E153). The identifiers stay as the author wrote them. A name Ruby read as
//! a constant is the generator's to keep apart now: Ruby's function holds the value under another
//! name, and nothing is said (§15.336). An enum read with `import enum` is held to the constants of
//! Go's package as the rule's own are, since the package hands it on under the file's names.
//! Where every language that writes a kind of name spells two as one — the types of two enums, two
//! values of one enum, the functions of two prerequisites blocks, the element types of two
//! sequences, a shape and an input of one identifier — the check stops at them with E034, and their
//! materials are with the other names written twice, in `tests/names`.
//!
//! The materials are in `tests/spellings`, English first, with a Japanese version beside each: one
//! rule for each way two names meet, for each name a language cannot take, and for each name the
//! code keeps apart. What `rulec check` prints for each is its golden file in `tests/golden/spellings`
//! (`RULEC_BLESS=1 cargo test --test spellings` writes them again). Generated without the targets the
//! warning names, their code answers as the reference evaluator does in every language; and given an
//! alias where the warning says, it does in every language, generated for all of them, as do the
//! rules of `tests/names` that E034 stops at.

use ritsu_testkit::tmp::tmpdir_in;
use ritsu_testkit::{Need, TempDir, ready, side_by_side, skip};
use rulec::i18n::{self, Lang};
use rulec::json::Json;
use std::path::{Path, PathBuf};
use std::process::Command;
use ritsu_testkit::run::finish;

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
    ("wire_enum_types_in_upper_snake", "protoで頭が重なる二つの列挙", "PHP, Java and Connect"),
    ("enum_values_in_upper_case", "大文字にすると重なる列挙の値", "Python, TypeScript, JavaScript, Ruby, PHP, Java and Connect"),
    ("enum_values_in_swift", "大文字の並びだけ違う列挙の値", "Python, TypeScript, JavaScript, Ruby, PHP, Swift, Java and Connect"),
    ("go_members_across_enums", "Goで重なる二つの列挙の値", "Go"),
    ("groups_spelled_alike", "綴りが重なるグループ", "Go and Swift"),
    ("blocks_in_camel_case", "キャメルケースで重なる前提のつながり", "TypeScript, JavaScript, Go, Swift and Java"),
    // a name a language cannot take
    ("python_underscore_output", "下線で始まる出力", "Python and Connect"),
    ("python_underscore_field", "下線で始まるフィールド", "Python and Connect"),
    ("postgresql_long_output", "63バイトより長い出力", "SQL (PostgreSQL)"),
    ("postgresql_long_table", "63バイトより長い表", "SQL (PostgreSQL)"),
];

/// The forms whose warning names the `.proto` beside the service: protoc refuses the `.proto`, which
/// `--target proto` writes (two fields, or two values of an enum, of one name), and the warning's
/// targets are `proto` and `connect`.
const WIRE: &[&str] = &[
    "inputs_spelled_alike",
    "outputs_spelled_alike",
    "fields_spelled_alike",
    "input_and_sequence_in_go",
    "outputs_with_a_trailing_underscore",
    "wire_enum_types_in_upper_snake",
    "enum_values_in_upper_case",
    "enum_values_in_swift",
];

/// Names Ruby read as constants, which its function now keeps apart (§15.336): nothing is said of
/// them, and every language answers as the reference evaluator does.
const APART: &[(&str, &str)] = &[("ruby_constant", "大文字で始まる定義"), ("ruby_constant_shape", "大文字で始まるshape")];

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
    let mut listed: Vec<String> = FORMS
        .iter()
        .map(|(en, ja, _)| (*en, *ja))
        .chain(APART.iter().copied())
        .flat_map(|(en, ja)| [format!("{en}.rule"), format!("{ja}.rule")])
        .collect();
    listed.sort();
    assert_eq!(names, listed, "every rule of tests/spellings is listed in FORMS or APART, with its Japanese version");
}

/// The names `--target` and `--extra` take of the languages an English title lists, as a W121's
/// targets name them (`Diag::targets`).
fn ids(langs: &str) -> Vec<&'static str> {
    langs
        .replace(" and ", ", ")
        .split(", ")
        .map(|l| match l {
            "Python" => "python",
            "TypeScript" => "typescript",
            "JavaScript" => "javascript",
            "Rust" => "rust",
            "Ruby" => "ruby",
            "PHP" => "php",
            "Go" => "go",
            "Swift" => "swift",
            "Java" => "java",
            "Wasm" => "wasm",
            "Connect" => "connect",
            "SQL (SQLite)" | "SQL (PostgreSQL)" => "sql",
            other => panic!("no target is named {other}"),
        })
        .collect()
}

/// Each form is said once, by W121, with the languages it meets in, in both versions, and the
/// warning's targets are those languages; the English and the Japanese wording are their golden
/// files.
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
            let mut want = ids(langs);
            if WIRE.contains(en) {
                want.push("proto");
            }
            want.sort();
            let mut got = ds[0].targets.clone();
            got.sort();
            if got != want {
                failures.push(format!("{rel}: the targets are {got:?}, not {want:?}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// Nothing is said of a name the code keeps apart, in either version; the English and the Japanese
/// wording are their golden files.
#[test]
fn nothing_is_said_of_a_name_kept_apart() {
    let mut failures = Vec::new();
    for (en, ja) in APART {
        for (stem, lang, tag) in [(*en, Lang::En, "en"), (*ja, Lang::Ja, "ja")] {
            let (rel, src) = source("spellings", stem);
            let (text, ds) = printed(&rel, &src, lang);
            if !ds.is_empty() {
                failures.push(format!("{rel}: {:?}\n{text}", ds.iter().map(|d| d.code).collect::<Vec<_>>()));
            }
            if let Err(e) = ritsu_testkit::golden::check(&root().join(format!("tests/golden/spellings/{stem}.{tag}.txt")), &text) {
                failures.push(e);
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

/// An enum read with `import enum` is handed on in Go's package of the rule under its file's names,
/// beside the constants of the rule's own enums: a value of the rule's own that comes to the type or
/// to a constant of the enum read is W121, naming Go, in English and in Japanese, the rule's own
/// name second, as the one to give an alias. Generated without Go, every language answers as the
/// reference evaluator does; with the aliases, nothing is said, and every language does.
#[test]
fn an_enum_read_with_import_enum_is_held_to_the_constants_of_go() {
    let sides = "enums sides v1\ndescription \"Side plans several rules share\"\n\nenum plan_gold = plus | base\n";
    let rule = |plus: &str, gold: &str| {
        format!(
            "rule plan_fee v1\ndescription \"A fee by the plan and the side plan, an enum read from a file beside one of its own\"\n\nimport enum plan_gold from \"sides.rule\"\n\nenum plan = {plus} | {gold}\n\ninputs\n  main : plan\n  side : plan_gold\n\noutputs\n  fee : number  round down(1)\n\ntable fee_by_plan\npolicy unique\n| main      | side | -> fee |\n| gold_plus | -    | 3      |\n| gold      | plus | 2      |\n| gold      | base | 1      |\n"
        )
    };
    let read = rule("gold_plus", "gold");
    for (lang, want) in [
        ("en", ["`plan_gold` and `gold` are one identifier in the code generated for Go", "`plus` and `gold_plus` are one identifier in the code generated for Go"]),
        ("ja", ["`plan_gold` と `gold` は、Go の生成コードで同じ識別子になります", "`plus` と `gold_plus` は、Go の生成コードで同じ識別子になります"]),
    ] {
        let t = written(&[("sides.rule", sides), ("plan_fee.rule", &read)]);
        let mut got = found(t.path(), &["plan_fee.rule"], lang);
        got.sort();
        let mut want: Vec<(String, String)> = want.iter().map(|w| ("W121".to_string(), w.to_string())).collect();
        want.sort();
        assert_eq!(got, want, "{lang}");
    }
    let t = written(&[("sides.rule", sides), ("plan_fee.rule", &read)]);
    let path = t.path().join("plan_fee.rule").to_string_lossy().into_owned();
    let ds = i18n::with(Lang::En, || rulec::check_source(&read, &path));
    assert_eq!(ds.len(), 2, "{:?}", ds.iter().map(|d| d.title.as_str()).collect::<Vec<_>>());
    for d in &ds {
        assert_eq!(d.targets, ["go"], "{}", d.title);
        assert!(d.notes.iter().any(|n| n.contains("of `sides.rule` (the rule's package hands it on under its name)")), "{:?}", d.notes);
        assert_eq!(d.marks.last().map(|m| m.span.line), Some(6), "{}: the rule's own name is marked last", d.title);
    }
    let aliased = rule("gold_plus(premium)", "gold(standard)");
    let t = written(&[("sides.rule", sides), ("plan_fee.rule", &aliased)]);
    assert!(found(t.path(), &["plan_fee.rule"], "en").is_empty());
    if !ready(Need::Python, have_python, "python3 is not here") {
        return;
    }
    for (src, without, what) in [(&read, vec!["go"], "the rule beside an enum read, without Go"), (&aliased, Vec::new(), "the rule with the aliases")] {
        let r = Run { file: "plan_fee.rule".to_string(), src: src.clone(), without, beside: vec![("sides.rule".to_string(), sides.to_string())] };
        runs_alike(&[r], what);
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

/// How long a run of rulec may take: `rulec test` talks to the servers of the generated code,
/// and waits for their answers without a limit of its own. Past it, rulec is stopped with every
/// process it started, and the test fails saying what it ran and what it printed until then.
const RULEC_LIMIT: std::time::Duration = std::time::Duration::from_secs(20 * 60);

fn rulec(cwd: &Path, tmp: &Path, args: &[&str]) -> (i32, String, String) {
    let o = finish(Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").env("TMPDIR", tmp).current_dir(cwd).args(args), RULEC_LIMIT, &format!("rulec {}", args.join(" ")));
    (o.code.unwrap_or(-1), o.stdout, o.stderr)
}

fn have_python() -> bool {
    Command::new("python3").arg("--version").output().is_ok_and(|o| o.status.success())
}

/// One rule to generate and run: its file name, its text, the targets its warnings name
/// (`Diag::targets`), which it is generated without, and the files it reads beside it.
struct Run {
    file: String,
    src: String,
    without: Vec<&'static str>,
    beside: Vec<(String, String)>,
}

impl Run {
    /// The rule of `file`, generated without the targets the check of `src` names, as read at `rel`.
    fn of(file: String, src: String, rel: &str) -> Run {
        let mut without: Vec<&'static str> = i18n::with(Lang::En, || rulec::check_source(&src, rel)).iter().filter(|d| d.code == "W121").flat_map(|d| d.targets.clone()).collect();
        without.sort();
        without.dedup();
        Run { file, src, without, beside: Vec::new() }
    }
}

/// The flags of every target and extra of 0.40.0 but those `without` names, and but the extras that
/// are then left with none of the targets they are written beside (`targets::Asked::new`).
fn flags_without(without: &[&str]) -> Vec<&'static str> {
    let mut targets: Vec<&'static str> = Vec::new();
    let mut extras: Vec<&'static str> = Vec::new();
    for pair in rulec::targets::UP_TO_0_40.chunks(2) {
        match pair {
            ["--target", t] if !without.contains(t) => targets.push(*t),
            ["--extra", e] if !without.contains(e) => extras.push(*e),
            _ => {}
        }
    }
    extras.retain(|e| match *e {
        "connect" => targets.contains(&"python") && targets.contains(&"proto"),
        "runner" | "round-test" | "vectors" => true,
        e => targets.iter().any(|t| rulec::targets::has_extra(t, e)),
    });
    targets.iter().flat_map(|t| ["--target", *t]).chain(extras.iter().flat_map(|e| ["--extra", *e])).collect()
}

/// Generate the rules into one directory, without the targets their warnings name, which are the
/// same for each of them, and run what was generated: every result answers as the reference
/// evaluator does, Python's among them where Python is generated.
fn runs_alike(runs: &[Run], what: &str) {
    let t = TempDir::new("spellings");
    let tmp = tmpdir_in(t.path());
    let dir = t.path().join("rules");
    let out = t.path().join("out");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(root().join("tests/spellings/orders.schema.json"), dir.join("orders.schema.json")).unwrap();
    for r in runs {
        std::fs::write(dir.join(&r.file), &r.src).unwrap();
        for (name, text) in &r.beside {
            std::fs::write(dir.join(name), text).unwrap();
        }
    }
    let without = &runs[0].without;
    let mut args: Vec<&str> = vec!["gen"];
    args.extend(runs.iter().map(|r| r.file.as_str()));
    let o = out.to_string_lossy().into_owned();
    args.extend(["--out", o.as_str()]);
    args.extend(flags_without(without));
    let (code, said, e) = rulec(&dir, &tmp, &args);
    assert_eq!(code, 0, "{what} (without {without:?}): {said}{e}");
    let (_, said, e) = rulec(&dir, &tmp, &["test", o.as_str(), "--format", "json"]);
    let j = rulec::json::parse(said.lines().next().unwrap_or_else(|| panic!("{what}: no result\n{e}"))).unwrap_or_else(|x| panic!("{what}: {x}: {said}"));
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
    assert!(broken.is_empty(), "{what} (without {without:?}): these did not run or did not agree:\n{}", broken.join("\n"));
    let ran: Vec<String> = results.iter().filter(|r| r.get("ran").and_then(|v| v.as_bool()) == Some(true)).map(|r| field(r, "lang")).collect();
    if !without.contains(&"python") {
        assert!(ran.iter().any(|l| l == "python"), "{what}: Python did not run: {ran:?}");
    } else {
        assert!(!ran.is_empty(), "{what}: nothing ran");
    }
}

/// The runs, one directory for each set of targets they are generated without, and those of one set
/// dealt into as many directories as there are cores, at most four; the English rules in some and
/// the Japanese versions, which share their aliases, in others.
fn deal(runs: Vec<Run>) -> Vec<Vec<Run>> {
    let n = std::thread::available_parallelism().map_or(1, |n| n.get()).clamp(1, 4);
    let mut sets: Vec<(Vec<&'static str>, Vec<Run>)> = Vec::new();
    for r in runs {
        match sets.iter_mut().find(|(w, _)| *w == r.without) {
            Some((_, rs)) => rs.push(r),
            None => sets.push((r.without.clone(), vec![r])),
        }
    }
    let mut dirs: Vec<Vec<Run>> = Vec::new();
    for (_, rs) in sets {
        let mut part: Vec<Vec<Run>> = (0..n.min(rs.len())).map(|_| Vec::new()).collect();
        let k = part.len();
        for (i, r) in rs.into_iter().enumerate() {
            part[i % k].push(r);
        }
        dirs.extend(part);
    }
    dirs
}

/// Each directory run as [`runs_alike`] runs it, four at a time at most.
fn run_dirs(dirs: &[Vec<Run>], what: &str) {
    for some in dirs.chunks(4) {
        side_by_side(some, |runs| runs_alike(runs, what));
    }
}

/// The generated code of every rule here answers as the reference evaluator does in every
/// language its warning does not name, generated without those, and the names kept apart answer so
/// in every language: the warning names each language where the code it wrote does not build or
/// answers otherwise.
#[test]
fn the_generated_code_agrees_in_every_language_the_warning_does_not_name() {
    if !ready(Need::Python, have_python, "python3 is not here") {
        return;
    }
    for ja in [false, true] {
        let runs: Vec<Run> = FORMS
            .iter()
            .map(|(en, j, _)| (*en, *j))
            .chain(APART.iter().copied())
            .map(|(en, j)| {
                let stem = if ja { j } else { en };
                let (rel, src) = source("spellings", stem);
                Run::of(format!("{stem}.rule"), src, &rel)
            })
            .collect();
        run_dirs(&deal(runs), if ja { "the Japanese versions" } else { "the English rules" });
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
            runs.push(Run::of(format!("{stem}.rule"), fixed, &rel));
        }
        run_dirs(&deal(runs), if ja { "the Japanese versions with an alias" } else { "the English rules with an alias" });
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}
