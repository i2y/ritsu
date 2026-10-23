//! The names the generated code of each language gives itself, beside the rule's own (§15.265).
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
//! The check now holds the rule's names to those, target by target (`codegen::own` lists them):
//! W121 names the name, the languages whose generated code uses it already, and what it meets
//! there. The names stay as the author wrote them, and the generators write their own as they did
//! for every rule. A sequence and a field of one element whose names are not in ASCII and have no
//! alias are public names as much as an input's, and E011 stops at them.
//!
//! The materials are in `tests/own_names`, English first, with a Japanese version beside each: one
//! rule for each kind of name a language's generated code uses itself, for each name a language
//! cannot take, and for each public name with no ASCII alias. What `rulec check` prints for each is
//! its golden file in `tests/golden/own_names` (`RULEC_BLESS=1 cargo test --test own_names` writes
//! them again). Their generated code answers as the reference evaluator does in every language the
//! warning does not name; and given an alias where the finding says, it does in every language.

use ritsu_testkit::tmp::tmpdir_in;
use ritsu_testkit::{Need, TempDir, ready, side_by_side, skip};
use rulec::i18n::{self, Lang};
use rulec::json::Json;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// One rule for each kind of name a language's generated code uses itself, and for each name a
/// language cannot take, the English first and the Japanese version beside it, with the languages
/// the warning names, as its English title lists them.
const FORMS: &[(&str, &str, &str)] = &[
    // the names of the `.proto` and its service
    ("output_named_trace", "traceという出力", "Connect"),
    ("enum_value_unspecified", "unspecifiedという列挙の値", "Connect"),
    ("enum_named_like_a_message", "メッセージと同じ名前の列挙", "Connect"),
    ("enum_not_in_ascii_on_the_wire", "ASCIIでない名前の列挙", "Connect"),
    // the types, the helpers and the locals of each language
    ("enum_named_output", "outputという列挙", "Python, TypeScript, Rust, Ruby, PHP, Go, Swift, Java, Wasm and Connect"),
    ("definition_named_like_a_rounding_helper", "丸めの補助関数と同じ名前の定義", "Rust and Wasm"),
    ("summary_flag_in_camel_case", "最大値のフラグと同じ名前の定義", "Swift and Java"),
    ("group_named_like_a_helper", "補助関数と同じ名前のグループ", "TypeScript and JavaScript"),
    ("input_named_out", "outという入力", "Python"),
    ("outputs_named_like_a_type", "型と同じ名前の出力", "Python and Go"),
    // the rule's function, class, module and package
    ("rule_named_main", "mainという規則", "Go and Swift"),
    ("rule_named_code", "codeという規則", "Python and Connect"),
    ("rule_named_like_a_type", "型と同じ名前の規則", "Go and Java"),
    // a name a language cannot take
    ("definition_named_underscore", "下線だけの定義", "Rust, Go, Swift, Java and Wasm"),
    ("field_named_underscore", "下線だけのフィールド", "Python, Rust, Go, Swift, Java, Wasm and Connect"),
    ("definition_named_true", "Trueという定義", "Ruby"),
];

/// The forms whose warning names Python for what only a type checker reads: mypy refuses the
/// generated module, and runs alike.
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
        .chain(PLUGIN.iter().copied())
        .flat_map(|(en, ja)| [format!("{en}.rule"), format!("{ja}.rule")])
        .collect();
    listed.sort();
    assert_eq!(names, listed, "every rule of tests/own_names is listed, with its Japanese version");
}

/// Each form is said once, by W121, with the languages it meets in, in both versions; the English
/// and the Japanese wording are their golden files. `define True` is said beside the W121 that says
/// rulec writes it as `True_`, which is the name Ruby cannot take.
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
            let others = ds.iter().filter(|d| named(&d.title).is_none()).count();
            if others != usize::from(*en == "definition_named_true") {
                failures.push(format!("{rel}: {others} other warnings\n{text}"));
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
/// take in the ways this section says, but the materials here and the mutants seeded with one: what
/// the check now says was never written on purpose. No sequence or field of the tree lacks an ASCII
/// alias but the materials and the mutant seeded with one.
#[test]
fn no_rule_of_the_tree_takes_a_name_its_code_uses_but_the_materials() {
    let base = root().join("../..").canonicalize().unwrap();
    let mut seen = 0usize;
    let mut found: Vec<String> = Vec::new();
    for rel in tree_rules() {
        if rel.starts_with("crates/rulec/tests/own_names/") || rel.starts_with("crates/rulec/tests/mutants/m_w121own") || rel.starts_with("crates/rulec/tests/mutants/m_e011seq") {
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
/// `code(renamed_1_6)` for `rule code`, the alias written over when there is one. The alias starts
/// with a small letter, has no `_` at its head, and is short, so no language refuses it.
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

fn rulec(cwd: &Path, tmp: &Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").env("TMPDIR", tmp).current_dir(cwd).args(args).output().expect("cannot start rulec");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

fn have(cmd: &str) -> bool {
    Command::new(cmd).arg("--version").output().is_ok_and(|o| o.status.success())
}

/// One rule to generate and run: its file name, its text, and the languages whose results do not
/// count (those the warning names), by how `rulec test` reports a result.
struct Run {
    file: String,
    src: String,
    excused: String,
}

/// Whether a result of `rulec test` is of a language the warning names: `Connect` is the Python
/// service's passes, and a language the rest of it, the rounding helpers too (Go's are in the
/// rule's package).
fn excused(langs: &str, lang: &str, via: &str) -> bool {
    langs.replace(" and ", ", ").split(", ").any(|l| match l {
        "Connect" => lang == "python" && via.starts_with("connect"),
        other => lang == other.to_ascii_lowercase(),
    })
}

/// The identifier of the rule a heading line declares: `rule 送料と記録(fee_and_trace) v1` is
/// `fee_and_trace`.
fn rule_alias(heading: &str) -> String {
    let name = heading.strip_prefix("rule ").unwrap_or(heading).split(' ').next().unwrap_or_default();
    match name.split_once('(') {
        Some((_, a)) => a.trim_end_matches(')').to_string(),
        None => name.to_string(),
    }
}

/// Generate the rules into one directory and run what was generated: every result that is of no
/// language a rule's warning names answers as the reference evaluator does, Python among them.
fn runs_alike(runs: &[Run], what: &str) {
    let t = TempDir::new("own-names");
    let tmp = tmpdir_in(t.path());
    let dir = t.path().join("rules");
    let out = t.path().join("out");
    std::fs::create_dir_all(&dir).unwrap();
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

/// The runs, dealt into as many directories as there are cores, at most four.
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
/// language its warning does not name, and the rules nothing is said of answer so in every
/// language: the warning names each language where the code it wrote does not build or answers
/// otherwise.
#[test]
fn the_generated_code_agrees_in_every_language_the_warning_does_not_name() {
    if !ready(Need::Python, || have("python3"), "python3 is not here") {
        return;
    }
    for ja in [false, true] {
        let pick = |en: &'static str, j: &'static str| if ja { j } else { en };
        let runs: Vec<Run> = FORMS
            .iter()
            .map(|(en, j, langs)| (pick(en, j), langs.to_string()))
            .chain(PLUGIN.iter().map(|(en, j)| (pick(en, j), String::new())))
            .map(|(stem, excused)| {
                let (_, src) = source(stem);
                Run { file: format!("{stem}.rule"), src, excused }
            })
            .collect();
        let dirs = deal(runs);
        side_by_side(&dirs, |runs| runs_alike(runs, if ja { "the Japanese versions" } else { "the English rules" }));
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
        for (en, j) in forms {
            let stem = if ja { j } else { en };
            let (rel, src) = source(stem);
            let mut fixed = src.clone();
            // As many aliases as there are findings: `define True` is said twice, of one name.
            for _ in 0..4 {
                let ds = i18n::with(Lang::En, || rulec::check_source(&fixed, &rel));
                let Some(d) = ds.first() else { break };
                fixed = if d.code == "E011" { with_the_missing_alias(&fixed, d) } else { with_an_alias(&fixed, d) };
            }
            let after: Vec<String> = i18n::with(Lang::En, || rulec::check_source(&fixed, &rel)).iter().map(|d| format!("{}: {}", d.code, d.title)).collect();
            if !after.is_empty() || fixed == src {
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

/// The two forms whose warning names Python for what only a type checker reads: `mypy --strict`
/// refuses the generated module, and takes it once the name has the alias the finding says. (Go's
/// code for the second does not build, which the runs above hold.)
#[test]
fn mypy_refuses_what_only_a_type_checker_reads_and_takes_it_with_an_alias() {
    if !ready(Need::Python, || have("mypy"), "mypy is not here") {
        return;
    }
    let mut failures = Vec::new();
    for (en, ja) in READ_BY_MYPY {
        for stem in [*en, *ja] {
            let (rel, src) = source(stem);
            let ds = i18n::with(Lang::En, || rulec::check_source(&src, &rel));
            let fixed = with_an_alias(&src, &ds[0]);
            for (text, refused) in [(&src, true), (&fixed, false)] {
                let t = TempDir::new("own-names-mypy");
                let tmp = tmpdir_in(t.path());
                std::fs::write(t.path().join(format!("{stem}.rule")), text).unwrap();
                let (code, said, e) = rulec(t.path(), &tmp, &["gen", &format!("{stem}.rule"), "--out", "out"]);
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
                if o.status.success() == refused {
                    failures.push(format!("{rel}{}: mypy {}\n{}", if refused { "" } else { " with an alias" }, if refused { "took it" } else { "refused it" }, String::from_utf8_lossy(&o.stdout)));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}
