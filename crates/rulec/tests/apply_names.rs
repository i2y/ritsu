//! What an `apply` brings in is written under an identifier no other name of the rule has, in every
//! language rulec writes (§15.260).
//!
//! An apply brings the callee's names in as `<apply>:<name>`, and the generated code wrote each one
//! as the callee's alias followed by the name's (§15.69). Neither `check` nor `gen` said anything
//! when that identifier was one the rule already had. `apply read` brought the callee's `credit` in
//! as `read_credit`, which is also the name `credit -> read_credit` gives this rule's output. And
//! the callee's alias is the same for every apply of it, so a rule that applied
//! `段階の値引き(tier_credit)` three times brought `tier_credit_値引き` in three times. TypeScript,
//! JavaScript, Go, Swift and Java then declared one name twice and did not compile, and the SQL
//! query read the wrong column. `tests/apply_enum/order_credits.rule` and its Japanese version have
//! both shapes. The tests missed them because they ran the generated code of a few rules picked by
//! hand, each applying a callee once under names that did not meet, and `tests/apply_enum.rs` ran
//! `check` alone; `tests/apply_absent.rs` left the Japanese version out once it was found.
//!
//! What an apply brings in is now written as the apply's own identifier, its alias or its name,
//! followed by the identifier the callee's own code gives the name: `段階の値引き(tier_credit)`'s
//! `値引き(credit)` is `credit` there, and `campaign_credit` under `apply キャンペーン(campaign)`.
//! Every identifier rulec makes — that one, what a contribution brings in, and the `_` after a
//! keyword (§15.210) — is held apart from every other identifier of the rule in each way a language
//! spells it, and takes a number after it where it would meet one. The names a rule declares keep
//! their identifiers.
//!
//! The materials are in `tests/apply_names`, English first, with a Japanese version beside each:
//! three callees, and one rule for each way two identifiers meet — an output named as what an apply
//! brings in, one callee applied three times, two applies whose names run into each other, names
//! that Swift and Java spell alike or SQLite reads alike, an enum brought in twice beside one of
//! this rule's, names PostgreSQL cuts to one, and a keyword's `_` that is another name already.
//! What `rulec check` prints for the keyword's rule is its golden file in `tests/golden/apply_names`
//! (`RULEC_BLESS=1 cargo test --test apply_names` writes them again). Every rule of the tree that
//! applies another is generated and run in every language here too: the rules of this directory
//! once, with the rest dealt into a few directories that run side by side.

use ritsu_testkit::tmp::tmpdir_in;
use ritsu_testkit::{Need, TempDir, ready, side_by_side, skip};
use rulec::ast::{Item, Name, RuleFile};
use rulec::i18n::{self, Lang};
use rulec::json::Json;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The callees, English first with the Japanese version beside each.
const CALLEES: [(&str, &str); 3] = [("tier_credit", "段階の値引き"), ("member_credit", "会員の値引き"), ("lift_credit", "引き上げの値引き")];

/// One rule for each way two identifiers meet, with the Japanese version beside it, and the
/// identifiers the generated code writes for the names it brings in, in each version: (the name in
/// English, its identifier, the name in Japanese, its identifier). Where the two versions bring a
/// name in under one identifier, the two meet the same names, since the identifiers are the aliases.
#[allow(clippy::type_complexity)]
const SHAPES: &[(&str, &str, &[(&str, &str, &str, &str)])] = &[
    // An output this rule names as what the apply brings in: `credit -> read_credit` under `apply read`
    ("renamed_output", "付け替えた出力", &[("read:credit", "read_credit_2", "読む:値引き", "read_credit_2")]),
    // One callee applied three times: in the Japanese version, the callee's alias was the same each time
    (
        "one_callee_three_times",
        "一つの規則を三度",
        &[
            ("campaign:credit", "campaign_credit", "キャンペーン:値引き", "campaign_credit"),
            ("own:credit", "own_credit", "本人:値引き", "own_credit"),
            ("repeat:credit", "repeat_credit", "再訪:値引き", "repeat_credit"),
        ],
    ),
    // `member` brings in `tier_credit`, `member_tier` brings in `credit`: both member_tier_credit
    (
        "two_applies_meet",
        "二つの準用が重なる",
        &[("member:tier_credit", "member_tier_credit", "会員:段階の値引き", "member_tier_credit"), ("member_tier:credit", "member_tier_credit_2", "会員の段階:値引き", "member_tier_credit_2")],
    ),
    // read_credit, which Swift and Java spell as readCredit, and SQLite reads as read_CREDIT
    ("spelled_alike", "綴りが重なる", &[("read:credit", "read_credit_2", "読む:値引き", "read_credit_2")]),
    // What a callee that writes its own enum brings in, twice
    (
        "enum_brought_twice",
        "列挙を二度持ち込む",
        &[("member:lift", "member_lift", "会員:引き上げ先", "member_lift"), ("campaign:lift", "campaign_lift", "キャンペーン:引き上げ先", "campaign_lift")],
    ),
    // Two names that agree for the 63 bytes PostgreSQL keeps of a name
    (
        "long_names",
        "長い名前",
        &[(
            "lift_every_member_one_tier_up_for_the_spring_campaign_2026:credit",
            "lift_every_member_one_tier_up_for_the_spring_campaign_2026_cr_2",
            "春のキャンペーンで会員を一つ上の段階に引き上げる準用:値引き",
            "春のキャンペーンで会員を一つ上の段階に引_2",
        )],
    ),
    // `class`, written `class_` (§15.210), which is the name of a definition of this rule
    ("keyword_meets_name", "予約語と名前", &[("class", "class_2", "class", "class_2")]),
];

/// The enums an apply brings in, and the identifiers their types are named after.
const ENUMS: &[(&str, &str, &str, &str)] = &[("member:tier", "member_tier_2", "会員:段階", "member_tier_2"), ("campaign:tier", "campaign_tier", "キャンペーン:段階", "campaign_tier")];

fn source(stem: &str) -> (String, String) {
    let rel = format!("tests/apply_names/{stem}.rule");
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

fn prepared(rel: &str, src: &str) -> RuleFile {
    match i18n::with(Lang::En, || rulec::prepare(src, rel)) {
        Ok((f, _)) => f,
        Err(ds) => panic!("{rel}: {:?}", ds.iter().map(|d| (d.code, d.title.clone())).collect::<Vec<_>>()),
    }
}

fn ident(n: &Name) -> String {
    n.ascii.clone().unwrap_or_else(|| n.text.clone())
}

/// The names a rule's generated code writes as values, each once, with its identifier: the inputs,
/// the outputs, the sequences and the fields of their elements, the groups, the shapes, and what
/// the items name.
fn values(f: &RuleFile) -> Vec<(String, String)> {
    let mut names: Vec<&Name> = Vec::new();
    names.extend(f.inputs.iter().map(|i| &i.name));
    names.extend(f.outputs.iter().map(|o| &o.name));
    for el in &f.elements {
        names.push(&el.name);
        names.extend(el.fields.iter().map(|fd| &fd.name));
    }
    names.extend(f.groups.iter().map(|g| &g.name));
    names.extend(f.shapes.iter().map(|s| &s.name));
    for it in &f.items {
        match it {
            Item::Derived(d) => names.push(&d.name),
            Item::Define(d) => names.push(&d.name),
            Item::Agg(d) => names.push(&d.name),
            Item::Fold(d) => names.extend(d.answer.iter().map(|a| &a.name)),
            Item::Table(t) => names.extend(t.outputs.iter().map(|o| &o.name)),
        }
    }
    let mut out: Vec<(String, String)> = Vec::new();
    for n in names {
        match out.iter_mut().find(|(t, _)| *t == n.text) {
            Some(x) if n.ascii.is_some() => x.1 = ident(n),
            Some(_) => {}
            None => out.push((n.text.clone(), ident(n))),
        }
    }
    out
}

/// The head of an identifier PostgreSQL keeps: 63 bytes, at the end of a whole character.
fn pg_head(id: &str) -> &str {
    let mut end = 0;
    for (i, c) in id.char_indices() {
        if i + c.len_utf8() > 63 {
            break;
        }
        end = i + c.len_utf8();
    }
    &id[..end]
}

/// Whether two columns of the SQL query are one to SQLite, which sets case aside, or to PostgreSQL,
/// which cuts a name at 63 bytes.
fn sql_meet(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b) || pg_head(a) == pg_head(b)
}

/// Where an identifier rulec makes meets another of the rule's: two values some language spells as
/// one, two enums whose types take one name when case is set aside, a table's SQL row column and a
/// value's column, or two tables' row columns. A name rulec makes the identifier of is one an
/// `apply` or a contribution brings in (`:` in it), or one written with `_` after it for a keyword.
fn meetings(f: &RuleFile) -> Vec<String> {
    let made = |text: &str, id: &str| text.contains(':') || (id != text && id.starts_with(text) && id[text.len()..].starts_with('_'));
    let mut out = Vec::new();
    let vs = values(f);
    for (i, (a, ia)) in vs.iter().enumerate() {
        for (b, ib) in &vs[i + 1..] {
            if !made(a, ia) && !made(b, ib) {
                continue;
            }
            let ways = rulec::codegen::values_meet(ia, ib);
            if !ways.is_empty() {
                out.push(format!("{a} ({ia}) and {b} ({ib}) meet: {ways:?}"));
            }
        }
    }
    let types: Vec<(String, String)> = f.enums.iter().map(|e| (e.name.text.clone(), rulec::codegen::type_name_of(&e.name))).collect();
    for (i, (a, ta)) in types.iter().enumerate() {
        for (b, tb) in &types[i + 1..] {
            if (a.contains(':') || b.contains(':')) && ta.eq_ignore_ascii_case(tb) {
                out.push(format!("the enums {a} and {b} both name the type {ta}"));
            }
        }
    }
    let rows: Vec<(String, String)> = f.items.iter().filter_map(|it| if let Item::Table(t) = it { t.name.as_ref().map(|n| (n.text.clone(), format!("{}_row", ident(n)))) } else { None }).collect();
    for (i, (a, ra)) in rows.iter().enumerate() {
        for (b, rb) in &rows[i + 1..] {
            if (a.contains(':') || b.contains(':')) && sql_meet(ra, rb) {
                out.push(format!("the SQL row columns of the tables {a} ({ra}) and {b} ({rb}) are one"));
            }
        }
        for (b, ib) in &vs {
            if (a.contains(':') || made(b, ib)) && sql_meet(ra, ib) {
                out.push(format!("the SQL row column of the table {a} ({ra}) and the column of {b} ({ib}) are one"));
            }
        }
    }
    out
}

/// Every rule here passes `check`. The rules that apply others say nothing; the keyword's rule
/// says, in W121, the identifier its input takes, and prints what its golden file holds.
#[test]
fn every_rule_here_passes_check() {
    let mut names: Vec<String> = std::fs::read_dir(root().join("tests/apply_names")).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    let mut listed: Vec<String> = CALLEES.iter().copied().chain(SHAPES.iter().map(|(en, ja, _)| (*en, *ja))).flat_map(|(en, ja)| [format!("{en}.rule"), format!("{ja}.rule")]).collect();
    listed.sort();
    assert_eq!(names, listed, "every rule of tests/apply_names is listed here, with its Japanese version");
    let mut failures = Vec::new();
    for (en, ja) in CALLEES.iter().copied().chain(SHAPES.iter().map(|(en, ja, _)| (*en, *ja))) {
        for stem in [en, ja] {
            let (rel, src) = source(stem);
            let codes: Vec<&str> = i18n::with(Lang::En, || rulec::check_source(&src, &rel)).iter().map(|d| d.code).collect();
            let want: &[&str] = if en == "keyword_meets_name" { &["W121"] } else { &[] };
            if codes != want {
                failures.push(format!("{rel}: {codes:?}"));
            }
        }
    }
    for stem in ["keyword_meets_name", "予約語と名前"] {
        let (rel, src) = source(stem);
        for (lang, tag) in [(Lang::En, "en"), (Lang::Ja, "ja")] {
            let out = printed(&rel, &src, lang);
            if let Err(e) = ritsu_testkit::golden::check(&root().join(format!("tests/golden/apply_names/{stem}.{tag}.txt")), &out) {
                failures.push(e);
            }
        }
    }
    let (rel, src) = source("keyword_meets_name");
    let out = printed(&rel, &src, Lang::En);
    if !out.contains("so the generated code writes it as `class_2`") {
        failures.push(format!("{rel} does not say the input takes class_2:\n{out}"));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// What each apply brings in takes the identifier the table above says, in both versions, and no
/// identifier rulec makes meets another of the rule's in any language's spelling.
#[test]
fn what_comes_in_is_written_apart_from_every_other_identifier() {
    let mut failures = Vec::new();
    for (en, ja, want) in SHAPES {
        for (stem, ja_side) in [(*en, false), (*ja, true)] {
            let (rel, src) = source(stem);
            let f = prepared(&rel, &src);
            let vs = values(&f);
            for (en_name, en_id, ja_name, ja_id) in *want {
                let (name, id) = if ja_side { (ja_name, ja_id) } else { (en_name, en_id) };
                match vs.iter().find(|(t, _)| t == name) {
                    Some((_, got)) if got == id => {}
                    Some((_, got)) => failures.push(format!("{rel}: {name} is written {got}, not {id}")),
                    None => failures.push(format!("{rel}: no value {name}: {vs:?}")),
                }
            }
            if *en == "enum_brought_twice" {
                for (en_name, en_id, ja_name, ja_id) in ENUMS {
                    let (name, id) = if ja_side { (ja_name, ja_id) } else { (en_name, en_id) };
                    match f.enums.iter().find(|e| e.name.text == *name) {
                        Some(e) if ident(&e.name) == *id => {}
                        Some(e) => failures.push(format!("{rel}: the enum {name} is written {}, not {id}", ident(&e.name))),
                        None => failures.push(format!("{rel}: no enum {name}")),
                    }
                }
            }
            failures.extend(meetings(&f).into_iter().map(|m| format!("{rel}: {m}")));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
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

/// No identifier rulec makes meets another in any rule of the tree that passes: what an apply or a
/// contribution brings in, and the `_` after a keyword.
#[test]
fn no_identifier_rulec_makes_meets_another_in_the_tree() {
    let base = root().join("../..").canonicalize().unwrap();
    let (mut seen, mut failures) = (0usize, Vec::new());
    for rel in tree_rules() {
        let path = base.join(&rel);
        let Ok(src) = std::fs::read_to_string(&path) else { continue };
        let Ok((f, _)) = i18n::with(Lang::En, || rulec::prepare(&src, &path.to_string_lossy())) else { continue };
        if f.items.iter().any(|it| match it {
            Item::Table(t) => t.outputs.iter().any(|o| o.name.text.contains(':')),
            Item::Define(d) => d.name.text.contains(':'),
            _ => false,
        }) {
            seen += 1;
        }
        failures.extend(meetings(&f).into_iter().map(|m| format!("{rel}: {m}")));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
    assert!(seen >= 30, "fewer rules of the tree bring names in than there were: {seen}");
}

fn rulec(cwd: &Path, tmp: &Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec")).env("RULEC_LANG", "en").env("TMPDIR", tmp).current_dir(cwd).args(args).output().expect("cannot start rulec");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// Generate the rules into one directory and run what was generated: every language that runs gives
/// the answers of the reference evaluator, Python among them. Returns the languages that ran.
fn runs_alike(cwd: &Path, rels: &[String], what: &str) -> Vec<String> {
    let t = TempDir::new("apply-names");
    let out = t.path().join("out");
    std::fs::create_dir_all(&out).unwrap();
    let tmp = tmpdir_in(&out);
    let mut args: Vec<&str> = vec!["gen"];
    args.extend(rels.iter().map(String::as_str));
    args.extend(["--out", out.to_str().unwrap()]);
    let (code, said, e) = rulec(cwd, &tmp, &args);
    assert_eq!(code, 0, "{what}: {said}{e}");
    let (_, said, e) = rulec(cwd, &tmp, &["test", out.to_str().unwrap(), "--format", "json"]);
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
    assert!(broken.is_empty(), "{what}: these did not run or did not agree:\n{}", broken.join("\n"));
    let mut ran: Vec<String> = results.iter().filter(|r| r.get("ran").and_then(|v| v.as_bool()) == Some(true)).map(|r| field(r, "lang")).collect();
    ran.sort();
    ran.dedup();
    assert!(ran.iter().any(|l| l == "python"), "{what}: Python did not run: {ran:?}");
    ran
}

fn have_python() -> bool {
    Command::new("python3").arg("--version").output().is_ok_and(|o| o.status.success())
}

/// The rules `every_language_answers_alike` runs, by their path from the workspace's root: every
/// rule of `SHAPES`, in both versions.
fn shape_rules() -> Vec<String> {
    SHAPES.iter().flat_map(|(en, ja, _)| [*en, *ja]).map(|stem| format!("crates/rulec/tests/apply_names/{stem}.rule")).collect()
}

/// The generated code of every rule here compiles and answers as the reference evaluator does, in
/// every language that is here: the English rules in one directory, the Japanese versions, which
/// share their aliases, in another. The test of the tree's rules below leaves these to this one.
#[test]
fn every_language_answers_alike() {
    if !ready(Need::Python, have_python, "python3 is not here") {
        return;
    }
    let callers: Vec<(&str, &str)> = SHAPES.iter().map(|(en, ja, _)| (*en, *ja)).collect();
    for ja in [false, true] {
        let rels: Vec<String> = callers.iter().map(|(en, j)| format!("tests/apply_names/{}.rule", if ja { j } else { en })).collect();
        let ran = runs_alike(&root(), &rels, if ja { "the Japanese versions" } else { "the English rules" });
        println!("apply_names: the {} rules ran alike in {}", if ja { "Japanese" } else { "English" }, ran.join(", "));
    }
}

/// How many directories the rules of the tree that apply another are dealt into: the cores of the
/// runners CI has. The rule that takes longest is one directory's work on its own, so more would
/// not make the test shorter, and each directory runs the rounding helpers and asks for the
/// toolchains once more.
const DIRECTORIES: usize = 4;

/// The rules, each as its path, its alias and the bytes of its file, dealt into the directories:
/// the largest first, each to the directory with the fewest bytes so far among those that have no
/// rule of its alias, so that no two rules of a directory write the same files. A rule whose alias
/// every directory has opens one more.
fn deal(mut rules: Vec<(String, String, usize)>) -> Vec<Vec<String>> {
    rules.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
    let mut dirs: Vec<(Vec<String>, Vec<String>, usize)> = vec![(Vec::new(), Vec::new(), 0); DIRECTORIES];
    for (rel, alias, bytes) in rules {
        match dirs.iter_mut().filter(|(_, aliases, _)| !aliases.contains(&alias)).min_by_key(|(_, _, size)| *size) {
            Some((rels, aliases, size)) => {
                rels.push(rel);
                aliases.push(alias);
                *size += bytes;
            }
            None => dirs.push((vec![rel], vec![alias], bytes)),
        }
    }
    dirs.into_iter().map(|(rels, _, _)| rels).filter(|rels| !rels.is_empty()).collect()
}

/// Every rule of the tree that applies another and passes `check` — the English ones and the
/// Japanese versions — runs in every language that is here, each answering as the reference
/// evaluator does. The rules of `every_language_answers_alike` are run there, in the same way;
/// the others are generated into a few directories run side by side, as many at once as the
/// machine has cores, and rules whose aliases would write the same files go to different ones.
#[test]
fn every_rule_of_the_tree_that_applies_another_runs_alike() {
    if !ready(Need::Python, have_python, "python3 is not here") {
        return;
    }
    let base = root().join("../..").canonicalize().unwrap();
    let there = shape_rules();
    let (mut here, mut elsewhere) = (Vec::new(), 0usize);
    for rel in tree_rules() {
        let path = base.join(&rel);
        let Ok(src) = std::fs::read_to_string(&path) else { continue };
        if !src.lines().any(|l| l.starts_with("apply ")) {
            continue;
        }
        let Ok((f, _)) = i18n::with(Lang::En, || rulec::prepare(&src, &path.to_string_lossy())) else { continue };
        if there.contains(&rel) {
            elsewhere += 1;
        } else {
            here.push((rel, ident(&f.name), src.len()));
        }
    }
    let rules = here.len() + elsewhere;
    assert!(rules >= 24, "fewer rules of the tree apply another than there were: {rules}");
    let dirs = deal(here);
    side_by_side(&dirs, |rels| {
        runs_alike(&base, rels, &rels.join(", "));
    });
    println!(
        "apply_names: {rules} rules of the tree that apply another ran alike: {} here, in {} directories side by side, and {elsewhere} in every_language_answers_alike",
        rules - elsewhere,
        dirs.len()
    );
}
