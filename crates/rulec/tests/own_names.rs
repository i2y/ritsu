//! The names the generated code of each language gives itself, beside the rule's own (§15.265,
//! §15.336).
//!
//! An output called `trace` passed `rulec check`, and the `.proto` rulec wrote for it declared
//! `trace` twice in `DecideResponse`, whose own field `trace` carries the rows that matched: buf
//! refused it, and the Connect service was not built. So it went with a value of an enum called
//! `unspecified` (the `.proto`'s `<TYPE>_UNSPECIFIED`), with an enum called `output` in a rule of two
//! outputs (the type `Output` of ten languages), with a definition named like the helper Rust rounds
//! an output with, with a rule called `main` (Go's package, Swift's `main.swift`), and with a name
//! of `_` alone. The check held the names to one another and to the words of each language, and not
//! to the names the generators write for themselves.
//!
//! The check holds the rule's names to those, target by target (`codegen::own` lists them). A name
//! the generated code keeps inside its functions, a helper, a local or a builtin it reads, is the
//! generator's to keep apart: the code of a language where the two meet holds the rule's value
//! under another name, and nothing is said (§15.336). A name the code shows its callers, a type, a
//! field, the rule's function, module, package or file, a message of the `.proto`, stays as the
//! author wrote it, and W121 names it, the languages whose code cannot take it, and what it meets
//! there; the warning's targets are what `rulec gen` then refuses to write (E153). A sequence and a
//! field of one element whose names are not in ASCII and have no alias are public names as much as
//! an input's, and E011 stops at them.
//!
//! The materials are in `tests/own_names`, English first, with a Japanese version beside each: one
//! rule for each kind of name a language's generated code shows, for each name a language cannot
//! take, for each name the code keeps apart, and for each public name with no ASCII alias. What
//! `rulec check` prints for each is its golden file in `tests/golden/own_names` (`RULEC_BLESS=1 cargo
//! test --test own_names` writes them again). Generated without the targets the warning names, their
//! code answers as the reference evaluator does in every language; and given an alias where the
//! finding says, it does in every language, generated for all of them.

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

/// One rule for each kind of name a language's generated code shows its callers, and for each name
/// a language cannot take, the English first and the Japanese version beside it, with the languages
/// the warning names, as its English title lists them.
const FORMS: &[(&str, &str, &str)] = &[
    // the names of the `.proto` and its service
    ("output_named_trace", "traceという出力", "Connect"),
    ("enum_value_unspecified", "unspecifiedという列挙の値", "Connect"),
    ("enum_named_like_a_message", "メッセージと同じ名前の列挙", "Connect"),
    ("enum_named_like_a_python_error", "Pythonの例外と同じ名前の列挙", "Connect"),
    ("enum_not_in_ascii_on_the_wire", "ASCIIでない名前の列挙", "Connect"),
    // the types of each language, and a keyword a type is spelled as (`Self`)
    ("enum_named_output", "outputという列挙", "Python, TypeScript, Rust, Ruby, PHP, Go, Swift, Java, Wasm and Connect"),
    ("enum_named_self", "selfという列挙", "Rust, Swift and Wasm"),
    // the rule's function, class, module, package and file, and a keyword the package is spelled as
    ("rule_named_main", "mainという規則", "Go and Swift"),
    ("rule_named_code", "codeという規則", "Python and Connect"),
    ("rule_named_like_a_type", "型と同じ名前の規則", "Go and Java"),
    ("rule_named_with_a_leading_underscore", "下線で始まる規則", "Go"),
    ("rule_named_like_a_go_keyword", "Goの予約語になる規則", "Go"),
    // a name a language cannot take, and a keyword a value of an enum is spelled as (`BEGIN`)
    ("field_named_underscore", "下線だけのフィールド", "Python, Rust, Go, Swift, Java, Wasm and Connect"),
    ("output_named_underscore", "下線だけの出力", "Python, Rust, Go, Swift, Java, Wasm and Connect"),
    ("input_named_underscore", "下線だけの入力", "Go and Swift"),
    ("enum_value_begin", "beginという列挙の値", "Ruby"),
];

/// The forms whose warning names the `.proto` beside the service: buf refuses the `.proto`, which
/// `--target proto` writes, and the warning's targets are `proto` and `connect`.
const WIRE: &[&str] = &["output_named_trace", "enum_value_unspecified", "enum_named_like_a_message", "enum_not_in_ascii_on_the_wire"];

/// The forms whose warning names a target whose code meets the name only where `rulec gen` writes
/// the rule's module beside the language's own (`Diag::beside`), which `ritsu gen` writes in a package
/// of rules: the Python module `code`, which `import code` finds in place of the library's, in the
/// code beside it and in the service of Connect that imports it (the service's class `Code` is kept
/// apart from the import of Connect, §15.336), and Swift's `main.swift` (Go can import no package
/// `main`). Every other form names none.
const BESIDE: &[(&str, &[&str])] = &[("rule_named_code", &["python", "connect"]), ("rule_named_main", &["swift"])];

/// Names the generated code of some language used itself inside its module, which the code of that
/// language now keeps the rule's name apart from (§15.336): nothing is said of them, and every
/// language answers as the reference evaluator does. A group named like a helper of TypeScript and
/// JavaScript is one: the group's constant is written `_roundDown_` there, beside the helper
/// `_roundDown`. `define True` keeps the W121 that says every language writes it `True_`
/// (§15.210), which keeps no target from being written.
const APART: &[(&str, &str)] = &[
    ("group_named_like_a_helper", "補助関数と同じ名前のグループ"),
    ("definition_named_like_a_rounding_helper", "丸めの補助関数と同じ名前の定義"),
    ("summary_flag_in_camel_case", "最大値のフラグと同じ名前の定義"),
    ("input_named_out", "outという入力"),
    ("input_named_like_an_alias_of_a_builtin", "組み込み関数の別名と同じ名前の入力"),
    ("outputs_named_like_a_type", "型と同じ名前の出力"),
    ("definition_named_underscore", "下線だけの定義"),
    ("definition_named_true", "Trueという定義"),
];

/// The names kept apart whose meeting in Python only a type checker read: `mypy --strict` takes the
/// generated module of each as it is.
const READ_BY_MYPY: &[(&str, &str)] = &[("input_named_out", "outという入力"), ("outputs_named_like_a_type", "型と同じ名前の出力")];

/// A sequence and a field of one element whose names are not in ASCII and have no alias, which
/// E011 stops at, with what its note says is missing, in English.
const ALIASLESS: &[(&str, &str, &str, &str)] = &[
    ("sequence_not_in_ascii", "別名の無い並び", "Missing: sequence pièces", "Missing: sequence 明細"),
    ("field_not_in_ascii", "別名の無いフィールド", "Missing: element field café", "Missing: element field 金額"),
];

/// Names protobuf's Python plugin writes otherwise in the stubs (`escape_message_attr`,
/// `escape_proto_module_component`), which the service and its runner now read as it writes them:
/// nothing is said of them, and every language agrees.
const PLUGIN: &[(&str, &str)] = &[("names_the_plugin_writes_otherwise", "プラグインが別に書く名前")];

fn source(stem: &str) -> (String, String) {
    let rel = format!("tests/own_names/{stem}.rule");
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

/// The languages an English W121 of this section names: the list after `generated for`.
fn named(title: &str) -> Option<String> {
    if let Some((_, l)) = title.split_once("takes a name the code generated for ") {
        return l.strip_suffix(" uses itself").map(String::from);
    }
    if let Some((_, l)) = title.split_once("is a name the code generated for ") {
        return l.strip_suffix(" cannot take").map(String::from);
    }
    None
}

/// Every rule here is listed above, with its Japanese version.
#[test]
fn every_rule_here_is_one_of_the_forms() {
    let mut names: Vec<String> = std::fs::read_dir(root().join("tests/own_names"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".rule"))
        .collect();
    names.sort();
    let mut listed: Vec<String> = FORMS
        .iter()
        .map(|(en, ja, _)| (*en, *ja))
        .chain(ALIASLESS.iter().map(|(en, ja, _, _)| (*en, *ja)))
        .chain(APART.iter().copied())
        .chain(PLUGIN.iter().copied())
        .flat_map(|(en, ja)| [format!("{en}.rule"), format!("{ja}.rule")])
        .collect();
    listed.sort();
    assert_eq!(names, listed, "every rule of tests/own_names is listed, with its Japanese version");
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
            let (rel, src) = source(stem);
            let (text, ds) = printed(&rel, &src, lang);
            if ds.iter().any(|d| d.code != "W121") {
                failures.push(format!("{rel}: {:?}\n{text}", ds.iter().map(|d| d.code).collect::<Vec<_>>()));
                continue;
            }
            if let Err(e) = ritsu_testkit::golden::check(&root().join(format!("tests/golden/own_names/{stem}.{tag}.txt")), &text) {
                failures.push(e);
            }
            // the languages, said in English whichever version is read
            let (_, ds) = printed(&rel, &src, Lang::En);
            let said: Vec<String> = ds.iter().filter_map(|d| named(&d.title)).collect();
            if said != [langs.to_string()] {
                failures.push(format!("{rel}: names {said:?}, not {langs}"));
            }
            // A name some language writes with `_` after it is said beside, and keeps no target
            // from being written (§15.344): `self`, whose class PHP writes `Self_`.
            let others = ds.iter().filter(|d| named(&d.title).is_none() && !only_respelled(d)).count();
            if others != 0 {
                failures.push(format!("{rel}: {others} other warnings\n{text}"));
            }
            let mut want = ids(langs);
            if WIRE.contains(en) {
                want.push("proto");
            }
            want.sort();
            let beside: &[&str] = BESIDE.iter().find(|(b, _)| b == en).map_or(&[], |(_, ts)| *ts);
            for d in ds.iter().filter(|d| named(&d.title).is_some()) {
                let mut got = d.targets.clone();
                got.sort();
                if got != want {
                    failures.push(format!("{rel}: the targets are {got:?}, not {want:?}"));
                }
                if d.beside != beside {
                    failures.push(format!("{rel}: the targets beside the language's own are {:?}, not {beside:?}", d.beside));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// Each name the code now keeps apart is said of no target, in both versions: nothing is said, but
/// for `define True` the one W121 that says how every language writes it, which names no target; the
/// English and the Japanese wording are their golden files.
#[test]
fn each_name_kept_apart_is_said_of_no_target() {
    let mut failures = Vec::new();
    for (en, ja) in APART {
        for (stem, lang, tag) in [(*en, Lang::En, "en"), (*ja, Lang::Ja, "ja")] {
            let (rel, src) = source(stem);
            let (text, ds) = printed(&rel, &src, lang);
            if let Err(e) = ritsu_testkit::golden::check(&root().join(format!("tests/golden/own_names/{stem}.{tag}.txt")), &text) {
                failures.push(e);
            }
            let want = usize::from(*en == "definition_named_true");
            if ds.len() != want || ds.iter().any(|d| d.code != "W121" || !d.targets.is_empty() || named(&d.title).is_some()) {
                let said: Vec<String> = ds.iter().map(|d| format!("{} {:?}: {}", d.code, d.targets, d.title)).collect();
                failures.push(format!("{rel}: {said:?}\n{text}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// A sequence and a field of one element whose names are not in ASCII and have no alias stop the
/// check with E011, which names them, in both versions; the wording is their golden files.
#[test]
fn a_sequence_and_a_field_with_no_ascii_alias_stop_at_e011() {
    let mut failures = Vec::new();
    for (en, ja, missing_en, missing_ja) in ALIASLESS {
        for (stem, lang, tag, missing) in [(*en, Lang::En, "en", *missing_en), (*ja, Lang::Ja, "ja", *missing_ja)] {
            let (rel, src) = source(stem);
            let (text, _) = printed(&rel, &src, lang);
            if let Err(e) = ritsu_testkit::golden::check(&root().join(format!("tests/golden/own_names/{stem}.{tag}.txt")), &text) {
                failures.push(e);
            }
            let (_, ds) = printed(&rel, &src, Lang::En);
            let codes: Vec<&str> = ds.iter().map(|d| d.code).collect();
            if codes != ["E011"] || !ds[0].notes.iter().any(|n| n == missing) {
                failures.push(format!("{rel}: {codes:?}, not E011 with {missing:?}\n{text}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// No rule of the tree takes a name the generated code uses itself, or is a name a language cannot
/// take in the ways this section says, but the materials here, those of the names the code writes
/// apart (`tests/apart`, where a language that does not yet is named) and those of the builtins it
/// reaches another way (`tests/builtins`, whose rules take such names to be generated), and the
/// mutants seeded with one: what the check now says was never written on purpose. No sequence or field of
/// the tree lacks an ASCII alias but the materials and the mutant seeded with one.
#[test]
fn no_rule_of_the_tree_takes_a_name_its_code_uses_but_the_materials() {
    let base = root().join("../..").canonicalize().unwrap();
    let mut seen = 0usize;
    let mut found: Vec<String> = Vec::new();
    for rel in tree_rules() {
        if rel.starts_with("crates/rulec/tests/own_names/")
            || rel.starts_with("crates/rulec/tests/apart/")
            || rel.starts_with("crates/rulec/tests/builtins/")
            || rel.starts_with("crates/rulec/tests/mutants/m_w121own")
            || rel.starts_with("crates/rulec/tests/mutants/m_w121self")
            || rel.starts_with("crates/rulec/tests/mutants/m_w121blank")
            || rel.starts_with("crates/rulec/tests/mutants/m_w121gofile")
            || rel.starts_with("crates/rulec/tests/mutants/m_e011seq")
        {
            continue;
        }
        let path = base.join(&rel);
        let Ok(src) = std::fs::read_to_string(&path) else { continue };
        let shown = path.to_string_lossy().into_owned();
        seen += 1;
        let ds = i18n::with(Lang::En, || rulec::check_source(&src, &shown));
        found.extend(ds.iter().filter(|d| d.code == "W121" && named(&d.title).is_some()).map(|d| format!("{rel}: {}", d.title)));
        found.extend(
            ds.iter()
                .filter(|d| d.code == "E011" && d.notes.iter().any(|n| n.contains("sequence ") || n.contains("element field ")))
                .map(|d| format!("{rel}: {} ({})", d.title, d.notes.join(" "))),
        );
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

/// The rule with an alias given to the name the first finding marks last, as its last note says:
/// `code(renamed3_1_6)` for `rule code`, the third rule given aliases, the alias written over when
/// there is one. The alias starts with a small letter, has no `_` at its head, and is short, so no
/// language refuses it. The number of the rule keeps apart the aliases of the rules' names, which
/// all stand at the same place of the first line, and are generated side by side.
fn with_an_alias(src: &str, d: &rulec::diag::Diag, k: usize) -> String {
    let span = &d.marks.last().expect("a finding with a mark").span;
    let mut lines: Vec<String> = src.lines().map(String::from).collect();
    let line = &mut lines[span.line - 1];
    let end = span.col + span.len;
    let alias = format!("(renamed{k}_{}_{})", span.line, span.col);
    let rest = &line[end..];
    let after = match rest.strip_prefix('(').and_then(|r| r.find(')').map(|i| &r[i + 1..])) {
        Some(after) => after.to_string(),
        None => rest.to_string(),
    };
    *line = format!("{}{alias}{after}", &line[..end]);
    lines.join("\n") + "\n"
}

/// The rule with an alias given to the sequence or the field E011 names as missing, where it is
/// declared: `elements 明細` is `elements 明細(renamed)`.
fn with_the_missing_alias(src: &str, d: &rulec::diag::Diag) -> String {
    let note = d.notes.iter().find(|n| n.starts_with("Missing: ")).expect("a note of what is missing");
    let name = note.rsplit(' ').next().unwrap_or_default();
    let mut done = false;
    let lines: Vec<String> = src
        .lines()
        .map(|l| {
            let t = l.trim_start();
            let head = t.strip_prefix("elements ").unwrap_or(t);
            if !done && head.starts_with(name) && (head[name.len()..].is_empty() || head[name.len()..].starts_with(' ')) {
                done = true;
                let at = l.len() - head.len() + name.len();
                format!("{}(renamed){}", &l[..at], &l[at..])
            } else {
                l.to_string()
            }
        })
        .collect();
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

fn have(cmd: &str) -> bool {
    Command::new(cmd).arg("--version").output().is_ok_and(|o| o.status.success())
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
        let without = named_targets(&src, rel);
        Run { file, src, without, beside: Vec::new() }
    }
}

/// Every target the W121s of `src` name, as read at `rel`, once each.
fn named_targets(src: &str, rel: &str) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = i18n::with(Lang::En, || rulec::check_source(src, rel)).iter().filter(|d| d.code == "W121").flat_map(|d| d.targets.clone()).collect();
    out.sort();
    out.dedup();
    out
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
    let t = TempDir::new("own-names");
    let tmp = tmpdir_in(t.path());
    let dir = t.path().join("rules");
    let out = t.path().join("out");
    std::fs::create_dir_all(&dir).unwrap();
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
/// dealt into as many directories as there are cores, at most four.
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
/// language its warning does not name, generated without those; the rules nothing is said of, those
/// whose names the code keeps apart among them, answer so in every language: the warning names each
/// language where the code it wrote does not build or answers otherwise.
#[test]
fn the_generated_code_agrees_in_every_language_the_warning_does_not_name() {
    if !ready(Need::Python, || have("python3"), "python3 is not here") {
        return;
    }
    for ja in [false, true] {
        let pick = |en: &'static str, j: &'static str| if ja { j } else { en };
        let runs: Vec<Run> = FORMS
            .iter()
            .map(|(en, j, _)| pick(en, j))
            .chain(APART.iter().map(|(en, j)| pick(en, j)))
            .chain(PLUGIN.iter().map(|(en, j)| pick(en, j)))
            .map(|stem| {
                let (rel, src) = source(stem);
                Run::of(format!("{stem}.rule"), src, &rel)
            })
            .collect();
        run_dirs(&deal(runs), if ja { "the Japanese versions" } else { "the English rules" });
    }
}

/// With an alias given to the name the finding marks, as it says, every rule here passes the check
/// with nothing said, and its generated code answers as the reference evaluator does in every
/// language.
#[test]
fn an_alias_where_the_finding_says_makes_every_language_agree() {
    if !ready(Need::Python, || have("python3"), "python3 is not here") {
        return;
    }
    let mut failures = Vec::new();
    for ja in [false, true] {
        let mut runs: Vec<Run> = Vec::new();
        let forms = FORMS.iter().map(|(en, j, _)| (*en, *j)).chain(ALIASLESS.iter().map(|(en, j, _, _)| (*en, *j)));
        for (k, (en, j)) in forms.enumerate() {
            let stem = if ja { j } else { en };
            let (rel, src) = source(stem);
            let mut fixed = src.clone();
            // As many aliases as there are findings: `define True` is said twice, of one name.
            for _ in 0..4 {
                let ds = i18n::with(Lang::En, || rulec::check_source(&fixed, &rel));
                let Some(d) = ds.first() else { break };
                fixed = if d.code == "E011" { with_the_missing_alias(&fixed, d) } else { with_an_alias(&fixed, d, k) };
            }
            let after: Vec<String> = i18n::with(Lang::En, || rulec::check_source(&fixed, &rel)).iter().map(|d| format!("{}: {}", d.code, d.title)).collect();
            if !after.is_empty() || fixed == src {
                failures.push(format!("{rel} with an alias:\n{}\n{fixed}", after.join("\n")));
                continue;
            }
            runs.push(Run::of(format!("{stem}.rule"), fixed, &rel));
        }
        run_dirs(&deal(runs), if ja { "the Japanese versions with an alias" } else { "the English rules with an alias" });
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// The two names kept apart whose meeting in Python only a type checker read (a parameter `out`
/// beside the local of that name, an output named like the type of the one after it): `mypy
/// --strict` takes the generated module of each as it is, in both versions.
#[test]
fn mypy_takes_the_module_of_the_names_kept_apart() {
    if !ready(Need::Python, || have("mypy"), "mypy is not here") {
        return;
    }
    let mut failures = Vec::new();
    for (en, ja) in READ_BY_MYPY {
        for stem in [*en, *ja] {
            let (rel, src) = source(stem);
            let t = TempDir::new("own-names-mypy");
            let tmp = tmpdir_in(t.path());
            std::fs::write(t.path().join(format!("{stem}.rule")), &src).unwrap();
            let (code, said, e) = rulec(t.path(), &tmp, &rulec::targets::up_to_0_40(&["gen", &format!("{stem}.rule"), "--out", "out"]));
            assert_eq!(code, 0, "{rel}: {said}{e}");
            let py = t.path().join("out/python");
            let module: Vec<String> = std::fs::read_dir(&py)
                .unwrap()
                .flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.ends_with(".py") && !n.starts_with('_') && !n.contains("runner") && !n.ends_with("_mcp.py") && !n.ends_with("_service.py"))
                .collect();
            let o = Command::new("mypy")
                .current_dir(&py)
                .env("TMPDIR", &tmp)
                .args(["--strict", "--no-color-output", "--cache-dir", tmp.join("mypy").to_str().unwrap()])
                .args(&module)
                .output()
                .expect("cannot start mypy");
            if !o.status.success() {
                failures.push(format!("{rel}: mypy refused it\n{}", String::from_utf8_lossy(&o.stdout)));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// An enum read with `import enum` is declared, or imported, in the rule's module under its file's
/// type, which the module shows its callers. One whose type is a name the module declares itself
/// (`Output`, `RuleInputError`, `Said`) is W121, said with the file it is read from, naming the
/// languages whose module declares that name, in English and in Japanese, and the warning's targets
/// are those languages (§15.336). Generated without them, every language answers as the reference
/// evaluator does; with the enum given an alias in its file, as the warning says, nothing is said,
/// and every language does.
#[test]
fn an_enum_read_under_a_name_the_module_declares_is_said_with_its_file() {
    let kinds = |output: &str, refusal: &str, said: &str| {
        format!("enums kinds v1\ndescription \"Kinds several rules share\"\n\nenum {output} = small | large\nenum {refusal} = missing | wrong\nenum {said} = yes | no\n")
    };
    let rules: [(&str, String, &str, &str); 3] = [
        (
            "parcel_by_size.rule",
            "rule parcel_by_size v1\ndescription \"A parcel's fee and band by its size, an enum read from a file\"\n\nimport enum output from \"kinds.rule\"\n\ninputs\n  size : output\n\noutputs\n  fee  : money[USD]  round down(1USD)\n  band : number  round down(1)\n\ntable fee_by_size\npolicy unique\n| size  | -> fee | band |\n| small | 3USD   | 1    |\n| large | 9USD   | 2    |\n".to_string(),
            "`output` takes a name the code generated for Python, TypeScript, Rust, Ruby, PHP, Swift, Wasm and Connect uses itself; `Output` is declared twice in the code generated for Swift",
            "`output` は、Python・TypeScript・Rust・Ruby・PHP・Swift・Wasm・Connect の生成コードが自分で使う名前と同じになります; `Output` が、Swift の生成コードで二度宣言されます",
        ),
        (
            "refusal_fee.rule",
            "rule refusal_fee v1\ndescription \"A fee by the kind of a refusal, an enum read from a file\"\n\nimport enum rule_input_error from \"kinds.rule\"\n\ninputs\n  kind : rule_input_error\n\noutputs\n  fee : money[USD]  round down(1USD)\n\ntable fee_by_kind\npolicy unique\n| kind    | -> fee |\n| missing | 1USD   |\n| wrong   | 2USD   |\n".to_string(),
            "`rule_input_error` takes a name the code generated for Python, TypeScript, JavaScript, Ruby, PHP and Connect uses itself",
            "`rule_input_error` は、Python・TypeScript・JavaScript・Ruby・PHP・Connect の生成コードが自分で使う名前と同じになります",
        ),
        (
            "answer_fee.rule",
            "rule answer_fee v1\ndescription \"A fee by whether it was said, an enum read from a file\"\n\nimport enum said from \"kinds.rule\"\n\ninputs\n  answer : said\n\noutputs\n  fee : money[USD]  round down(1USD)\n\ntable fee_by_answer\npolicy unique\n| answer | -> fee |\n| yes    | 1USD   |\n| no     | 2USD   |\n".to_string(),
            "`said` takes a name the code generated for Rust and Wasm uses itself",
            "`said` は、Rust・Wasm の生成コードが自分で使う名前と同じになります",
        ),
    ];
    let t = TempDir::new("own-names-read-enum");
    let as_read = kinds("output", "rule_input_error", "said");
    std::fs::write(t.path().join("kinds.rule"), &as_read).unwrap();
    let mut runs: Vec<Run> = Vec::new();
    for (file, src, en, ja) in &rules {
        let path = t.path().join(file).to_string_lossy().into_owned();
        std::fs::write(&path, src).unwrap();
        for (lang, title) in [(Lang::En, *en), (Lang::Ja, *ja)] {
            let ds = i18n::with(lang, || rulec::check_source(src, &path));
            let titles: Vec<&str> = ds.iter().map(|d| d.title.as_str()).collect();
            let want: Vec<&str> = title.split("; ").collect();
            assert_eq!(titles, want, "{file} ({lang:?})");
            assert!(ds[0].notes.iter().any(|n| n.contains("`kinds.rule`")), "{file} ({lang:?}): {:?}", ds[0].notes);
        }
        let ds = i18n::with(Lang::En, || rulec::check_source(src, &path));
        let mut want = ids(&named(&ds[0].title).unwrap());
        want.sort();
        let mut got = ds[0].targets.clone();
        got.sort();
        assert_eq!(got, want, "{file}");
        let mut r = Run::of(file.to_string(), src.clone(), &path);
        r.beside.push(("kinds.rule".to_string(), as_read.clone()));
        runs.push(r);
    }
    // With an alias in the file, as the warning says.
    let aliased = kinds("output(size_kind)", "rule_input_error(refusal_kind)", "said(answer_kind)");
    std::fs::write(t.path().join("kinds.rule"), &aliased).unwrap();
    let mut fixed: Vec<Run> = Vec::new();
    for (file, src, _, _) in &rules {
        let path = t.path().join(file).to_string_lossy().into_owned();
        let ds = i18n::with(Lang::En, || rulec::check_source(src, &path));
        assert!(ds.is_empty(), "{file} with an alias in kinds.rule: {:?}", ds.iter().map(|d| &d.title).collect::<Vec<_>>());
        let mut r = Run::of(file.to_string(), src.clone(), &path);
        r.beside.push(("kinds.rule".to_string(), aliased.clone()));
        fixed.push(r);
    }
    if !ready(Need::Python, || have("python3"), "python3 is not here") {
        return;
    }
    run_dirs(&deal(runs), "the rules that read an enum named like a type of the module");
    run_dirs(&[fixed], "the rules that read an enum given an alias in its file");
}

/// The `enums` files of `tests/own_names/enums_files/`, each named like a word some languages have,
/// English first and the Japanese version beside it, which gives the word as the alias: the
/// languages the W121 names, and the targets of those it names only beside the language's own
/// (`Diag::beside`).
const ENUMS_FILES: &[(&str, &str, &str, &[&str])] = &[
    ("enums_file_named_string", "文字列という名前の列挙ファイル", "Python, Ruby, Go, Java and Connect", &["python", "connect"]),
    ("enums_file_named_like_a_keyword", "予約語と同じ名前の列挙ファイル", "Python, Rust, Go and Connect", &[]),
    ("enums_file_named_like_a_library_module", "標準のモジュールと同じ名前の列挙ファイル", "Python, Ruby, Go and Connect", &["python", "go", "connect"]),
    ("enums_file_named_main", "mainという列挙ファイル", "Go and Swift", &[]),
    ("enums_file_named_std", "stdという列挙ファイル", "Rust and Go", &["go"]),
    ("enums_file_named_like_a_java_lang_class", "Javaの標準のクラスと同じ名前の列挙ファイル", "Java", &[]),
];

fn enums_file(stem: &str) -> (String, String) {
    let rel = format!("tests/own_names/enums_files/{stem}.rule");
    let src = std::fs::read_to_string(root().join(&rel)).unwrap_or_else(|_| panic!("cannot read {rel}"));
    (rel, src)
}

/// The rules here (`tests/own_names/respelled`) whose name, an enum's or a value's a language
/// writes with `_` after it, a keyword of its own there, with their Japanese versions and the
/// languages each finding names, in their order (§15.344).
const RESPELLED: &[(&str, &str, &[&str])] = &[
    ("rule_and_enum_named_like_php_keywords", "PHPの予約語になる規則と列挙", &["PHP", "PHP", "PHP"]),
    ("rule_named_like_a_java_keyword", "Javaの予約語になる規則", &["Java"]),
];

/// The languages a finding of [`RESPELLED`] names, from its English title: a name some language
/// writes with `_` after it, which keeps no target from being written.
fn respelled_named(title: &str) -> Option<String> {
    let (_, l) = title.split_once(" is a keyword as written in ")?;
    l.strip_suffix(", so the generated code there writes it with `_` after it").map(String::from)
}

/// Whether `d` says only that some language writes a name with `_` after it: a W121 of
/// [`respelled_named`]'s, which names no target.
fn only_respelled(d: &rulec::diag::Diag) -> bool {
    d.code == "W121" && respelled_named(&d.title).is_some() && d.targets.is_empty() && d.beside.is_empty()
}

/// What a finding of [`RESPELLED`] says each language writes, from its English notes: the language,
/// and each name the note gives in backticks.
fn respelled_names(d: &rulec::diag::Diag) -> Vec<(String, Vec<String>)> {
    d.notes
        .iter()
        .filter_map(|n| {
            let (lang, rest) = n.split_once(": ")?;
            matches!(lang, "PHP" | "Java").then(|| (lang.to_string(), rest.split('`').skip(1).step_by(2).map(String::from).collect()))
        })
        .collect()
}

/// Every file under `dir`, read as text, one after another.
fn text_under(dir: &Path) -> String {
    let mut out = String::new();
    let Ok(rd) = std::fs::read_dir(dir) else { return out };
    let mut ps: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
    ps.sort();
    for p in ps {
        if p.is_dir() {
            out.push_str(&text_under(&p));
        } else if let Ok(s) = std::fs::read_to_string(&p) {
            out.push_str(&s);
        }
    }
    out
}

/// A rule's name, an enum's or a value's that a language writes with `_` after it, a keyword of its
/// own there, builds in that language: W121 says each such name once, with the languages it names
/// and the names their code takes, and keeps no target from being written, in both versions, whose
/// wording is their golden files. `rulec gen` writes PHP and Java, and each name a finding gives is
/// in the code of its language; the code of every language, PHP's and Java's among them, answers as
/// the reference evaluator does (§15.344).
#[test]
fn a_keyword_a_language_writes_with_an_underscore_is_said_and_keeps_no_target() {
    let mut here: Vec<String> = std::fs::read_dir(root().join("tests/own_names/respelled"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".rule"))
        .collect();
    here.sort();
    let mut listed: Vec<String> = RESPELLED.iter().flat_map(|(en, ja, _)| [format!("{en}.rule"), format!("{ja}.rule")]).collect();
    listed.sort();
    assert_eq!(here, listed, "every file of tests/own_names/respelled is listed, with its Japanese version");
    let mut failures = Vec::new();
    let mut runs: [Vec<Run>; 2] = [Vec::new(), Vec::new()];
    for (en, ja, langs) in RESPELLED {
        for (k, (stem, lang, tag)) in [(*en, Lang::En, "en"), (*ja, Lang::Ja, "ja")].into_iter().enumerate() {
            let rel = format!("tests/own_names/respelled/{stem}.rule");
            let src = std::fs::read_to_string(root().join(&rel)).unwrap_or_else(|_| panic!("cannot read {rel}"));
            let (text, _) = printed(&rel, &src, lang);
            if let Err(e) = ritsu_testkit::golden::check(&root().join(format!("tests/golden/own_names/respelled/{stem}.{tag}.txt")), &text) {
                failures.push(e);
            }
            let (_, ds) = printed(&rel, &src, Lang::En);
            let said: Vec<String> = ds.iter().filter_map(|d| respelled_named(&d.title)).collect();
            if said != *langs || ds.len() != langs.len() {
                failures.push(format!("{rel}: names {said:?} in {} findings, not {langs:?}\n{text}", ds.len()));
            }
            for d in &ds {
                if !d.targets.is_empty() || !d.beside.is_empty() {
                    failures.push(format!("{rel}: `{}` keeps {:?} from being written", d.title, d.targets));
                }
            }
            // What each finding says PHP and Java write is what their generators write.
            let t = TempDir::new("respelled");
            let tmp = tmpdir_in(t.path());
            let file = format!("{stem}.rule");
            std::fs::write(t.path().join(&file), &src).unwrap();
            let o = t.path().join("out").to_string_lossy().into_owned();
            let (code, said, e) = rulec(t.path(), &tmp, &["gen", file.as_str(), "--target", "php", "--target", "java", "--out", o.as_str()]);
            if code != 0 {
                failures.push(format!("{rel}: rulec gen --target php --target java gave {code}: {said}{e}"));
            }
            for d in &ds {
                for (lang, names) in respelled_names(d) {
                    let written = text_under(&t.path().join("out").join(lang.to_lowercase()));
                    for n in names.iter().filter(|n| !written.contains(n.as_str())) {
                        failures.push(format!("{rel}: `{}` says {lang} writes `{n}`, which its code does not have", d.title));
                    }
                }
            }
            runs[k].push(Run::of(format!("{stem}.rule"), src, &rel));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
    if !ready(Need::Python, || have("python3"), "python3 is not here") {
        return;
    }
    // The English rules and the Japanese versions take one identifier each: run apart.
    for (rs, what) in runs.into_iter().zip(["the English rules a language writes with `_` after a keyword", "their Japanese versions"]) {
        run_dirs(&deal(rs), what);
    }
}

/// The name of an `enums` file is held to each language's words as the module, the class or the
/// package the language writes the file as, which the rules that read from it import (§15.265):
/// said once, by W121, with the languages and the targets it names, in both versions, whose wording
/// is their golden files.
#[test]
fn an_enums_file_is_held_to_the_words_of_each_language() {
    let mut here: Vec<String> = std::fs::read_dir(root().join("tests/own_names/enums_files"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".rule"))
        .collect();
    here.sort();
    let mut listed: Vec<String> = ENUMS_FILES.iter().flat_map(|(en, ja, _, _)| [format!("{en}.rule"), format!("{ja}.rule")]).collect();
    listed.sort();
    assert_eq!(here, listed, "every file of tests/own_names/enums_files is listed, with its Japanese version");
    let mut failures = Vec::new();
    for (en, ja, langs, beside) in ENUMS_FILES {
        for (stem, lang, tag) in [(*en, Lang::En, "en"), (*ja, Lang::Ja, "ja")] {
            let (rel, src) = enums_file(stem);
            let (text, _) = printed(&rel, &src, lang);
            if let Err(e) = ritsu_testkit::golden::check(&root().join(format!("tests/golden/own_names/enums_files/{stem}.{tag}.txt")), &text) {
                failures.push(e);
            }
            // A file's name PHP writes with `_` after it (`For_`) is said beside, and keeps no
            // target from being written (§15.344).
            let (_, all) = printed(&rel, &src, Lang::En);
            let ds: Vec<&rulec::diag::Diag> = all.iter().filter(|d| !only_respelled(d)).collect();
            let said: Vec<String> = ds.iter().filter_map(|d| named(&d.title)).collect();
            if said != [langs.to_string()] || ds.len() != 1 {
                failures.push(format!("{rel}: names {said:?} in {} findings, not {langs} in one\n{text}", ds.len()));
                continue;
            }
            let mut want = ids(langs);
            want.sort();
            let mut got = ds[0].targets.clone();
            got.sort();
            if got != want {
                failures.push(format!("{rel}: the targets are {got:?}, not {want:?}"));
            }
            if ds[0].beside != *beside {
                failures.push(format!("{rel}: the targets beside the language's own are {:?}, not {beside:?}", ds[0].beside));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// With an alias in place of the word, as the finding says, each of those files passes the check
/// with nothing said, and a rule that reads its enum from it answers as the reference evaluator
/// does in every language.
#[test]
fn an_alias_of_an_enums_file_makes_every_language_agree() {
    if !ready(Need::Python, || have("python3"), "python3 is not here") {
        return;
    }
    let mut runs: Vec<Run> = Vec::new();
    for (k, (en, ja, _, _)) in ENUMS_FILES.iter().enumerate() {
        for (stem, tag, zone, values) in [(*en, "en", "zone", ["north", "south"]), (*ja, "ja", "地域", ["北", "南"])] {
            let (rel, src) = enums_file(stem);
            let first = src.lines().next().unwrap_or_default();
            let name = first.strip_prefix("enums ").and_then(|r| r.strip_suffix(" v1")).unwrap_or_else(|| panic!("{rel}: {first}"));
            let word = name.split('(').next().unwrap_or(name);
            let alias = format!("zones{k}{tag}");
            let fixed = src.replacen(first, &format!("enums {word}({alias}) v1"), 1);
            let ds = i18n::with(Lang::En, || rulec::check_source(&fixed, &rel));
            assert!(ds.is_empty(), "{rel} with the alias {alias}: {:?}", ds.iter().map(|d| d.title.clone()).collect::<Vec<_>>());
            let file = format!("e{k}_{tag}.rule");
            let rule = format!(
                "rule fee_by_zone_{k}_{tag} v1\ndescription \"A rule that reads its enum from an enums file\"\n\nimport enum {zone} from \"{file}\"\n\ninputs\n  z : {zone}\n\noutputs\n  fee : money[USD]  round down(1USD)\n\ntable fee_by_z\npolicy unique\n| z | -> fee |\n| {} | 3USD |\n| {} | 4USD |\n",
                values[0], values[1]
            );
            runs.push(Run { file: format!("fee_by_zone_{k}_{tag}.rule"), src: rule, without: Vec::new(), beside: vec![(file, fixed)] });
        }
    }
    run_dirs(&deal(runs), "the enums files with an alias");
}
