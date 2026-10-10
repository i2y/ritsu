//! `geas scenarios` (DESIGN §17): the scenarios of OpenSpec specs, each held to the claims of the
//! same name in the claims files given, and the scenarios no claim answers. With `--draft`, a
//! claim to fill in for each of those. Nothing is run. The specs are read as OpenSpec reads them
//! (ritsu's base layer, `ritsu_base::openspec`).

use crate::diag::{self, Diag, Show};
use crate::json::{self, J};
use crate::parse;
use ritsu_base::openspec::{self, Op};
use ritsu_base::text::{Lang, Text};
use std::path::{Path, PathBuf};

pub struct Opts<'a> {
    pub openspec: &'a [String],
    pub draft: bool,
    pub json: bool,
    pub lang: Lang,
}

/// One claim of a claims file given.
struct Claim {
    spec: String,
    number: usize,
    name: String,
    line: usize,
}

/// A requirement of a spec, with the scenarios it asks for.
struct Req {
    name: String,
    op: Option<Op>,
    line: usize,
    scenarios: Vec<openspec::Scenario>,
}

/// One file of OpenSpec read: a spec, or a change's delta spec.
struct File {
    path: String,
    delta: bool,
    reqs: Vec<Req>,
}

fn e081(path: &str, why: Text) -> (String, Diag) {
    (path.to_string(), diag::error("E081", 0, 0, tr!("このファイルを読めません: {}", "cannot read this file: {}", why.ja; why.en)))
}

fn e090(path: &str, why: Text) -> (String, Diag) {
    (
        path.to_string(),
        diag::error("E090", 0, 0, tr!("OpenSpec の仕様として読めません: {}", "does not read as an OpenSpec spec: {}", why.ja; why.en)).note(tr!(
            "渡すのは、仕様（`openspec/specs/<capability>/spec.md`）、変更の提案の差分（`openspec/changes/<id>/specs/<capability>/spec.md`）、そのどちらかを持つディレクトリです。形の誤りは `openspec validate` が言います",
            "give a spec (`openspec/specs/<capability>/spec.md`), a change's delta spec (`openspec/changes/<id>/specs/<capability>/spec.md`), or a directory that holds them; `openspec validate` says what is wrong with a form",
        )),
    )
}

/// The `spec.md` files under a directory, in path order, leaving out `archive/` and what a walk
/// passes over (names starting with `.`, `target`, `node_modules`).
fn spec_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = ritsu_base::fs::read_dir(dir) else { return };
    let mut names: Vec<String> = rd.filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().to_string()).collect();
    names.sort();
    for n in names {
        if n == "archive" || ritsu_base::paths::skipped_name(&n) {
            continue;
        }
        let p = dir.join(&n);
        if p.is_dir() {
            spec_files(&p, out);
        } else if n == "spec.md" {
            out.push(p);
        }
    }
}

fn shown(p: &Path) -> String {
    p.display().to_string()
}

fn read_openspec(given: &[String]) -> Result<Vec<File>, Vec<(String, Diag)>> {
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut errs = Vec::new();
    for g in given {
        let p = PathBuf::from(g);
        if p.is_dir() {
            let before = paths.len();
            spec_files(&p, &mut paths);
            if paths.len() == before {
                errs.push(e090(g, tr!("この下に spec.md がありません", "there is no spec.md under it")));
            }
        } else if p.is_file() {
            paths.push(p);
        } else {
            errs.push(e081(g, tr!("ありません", "it is not there")));
        }
    }
    let mut files = Vec::new();
    for p in paths {
        let path = shown(&p);
        let bytes = match ritsu_base::fs::read(&p) {
            Ok(b) => b,
            Err(e) => {
                errs.push(e081(&path, Text::same(e.to_string())));
                continue;
            }
        };
        let text = String::from_utf8_lossy(&bytes);
        let why = |e: openspec::SpecError| match e {
            openspec::SpecError::NotUtf8 => tr!("UTF-8 ではありません", "it is not UTF-8"),
            openspec::SpecError::NoRequirements { .. } => tr!(
                "`## Requirements` の節も、変更の提案の差分の節（`## ADDED Requirements` など）もありません",
                "it has neither a `## Requirements` section nor a section of a change's delta spec (`## ADDED Requirements` and the like)"
            ),
            openspec::SpecError::Twice { name, first, line } => tr!("要件「{name}」が {first} 行目と {line} 行目の二か所にあります", "the requirement {name} is written twice, on lines {first} and {line}"),
        };
        if openspec::is_delta(&text) {
            match openspec::read_delta(&bytes) {
                Ok(d) => {
                    let reqs = d
                        .changes
                        .into_iter()
                        .filter(|c| matches!(c.op, Op::Added | Op::Modified))
                        .filter_map(|c| c.requirement.map(|r| Req { name: r.name, op: Some(c.op), line: r.line, scenarios: r.scenarios }))
                        .collect();
                    files.push(File { path, delta: true, reqs });
                }
                Err(e) => errs.push(e090(&path, why(e))),
            }
        } else {
            match openspec::read_spec(&bytes) {
                Ok(s) => files.push(File { path, delta: false, reqs: s.requirements.into_iter().map(|r| Req { name: r.name, op: None, line: r.line, scenarios: r.scenarios }).collect() }),
                Err(e) => errs.push(e090(&path, why(e))),
            }
        }
    }
    if errs.is_empty() { Ok(files) } else { Err(errs) }
}

fn read_claims(specs: &[String]) -> Result<Vec<Claim>, Vec<(String, Diag, String)>> {
    let mut out = Vec::new();
    let mut errs = Vec::new();
    for f in specs {
        let src = match ritsu_base::fs::read_to_string(f) {
            Ok(s) => s,
            Err(e) => {
                let (p, d) = e081(f, Text::same(e.to_string()));
                errs.push((p, d, String::new()));
                continue;
            }
        };
        match parse::parse(&src) {
            Ok(spec) => out.extend(spec.claims.iter().enumerate().map(|(i, c)| Claim { spec: f.clone(), number: i + 1, name: c.name.clone(), line: c.pos.line })),
            Err(ds) => errs.extend(ds.into_iter().map(|d| (f.clone(), d, src.clone()))),
        }
    }
    if errs.is_empty() { Ok(out) } else { Err(errs) }
}

/// The fold OpenSpec uses for its near names: case and the runs of white space.
fn fold(s: &str) -> String {
    openspec::fold(s)
}

pub fn command(specs: &[String], o: &Opts) -> i32 {
    let lang = o.lang;
    let claims = match read_claims(specs) {
        Ok(c) => c,
        Err(errs) => {
            for (f, d, src) in errs {
                eprint!("{}", d.shown(&f, &src, lang));
            }
            return 2;
        }
    };
    let files = match read_openspec(o.openspec) {
        Ok(f) => f,
        Err(errs) => {
            for (f, d) in errs {
                eprint!("{}", d.shown(&f, "", lang));
            }
            return 2;
        }
    };
    let answering = |name: &str| -> Vec<&Claim> { claims.iter().filter(|c| c.name == name).collect() };
    let near = |name: &str| -> Vec<&Claim> { claims.iter().filter(|c| c.name != name && fold(&c.name) == fold(name)).collect() };
    let mut total = 0;
    let mut answered = 0;
    let mut named: Vec<&str> = Vec::new();
    for f in &files {
        for r in &f.reqs {
            for s in &r.scenarios {
                total += 1;
                named.push(&s.name);
                if !answering(&s.name).is_empty() {
                    answered += 1;
                }
            }
        }
    }
    let unnamed: Vec<&Claim> = claims.iter().filter(|c| !named.contains(&c.name.as_str())).collect();
    let exit = if answered < total { 1 } else { 0 };

    if o.draft {
        let mut out = String::new();
        for f in &files {
            for r in &f.reqs {
                for s in r.scenarios.iter().filter(|s| answering(&s.name).is_empty()) {
                    out.push_str(&format!("# {}\n# Requirement: {}\n#   Scenario: {}\n", f.path, r.name, s.name));
                    for l in &s.body {
                        out.push_str(&format!("#   {}\n", l.trim_end()).replace("#   \n", "#\n"));
                    }
                    let (when, then) = match lang {
                        Lang::Ja => ("# when <ターゲット>.<呼び出し>(…)", "# then <チェックするもの> <比べ方>"),
                        _ => ("# when <target>.<call>(…)", "# then <subject> <matcher>"),
                    };
                    out.push_str(&format!("claim {} {{\n  {when}\n  {then}\n}}\n\n", json::quote(&s.name)));
                }
            }
        }
        if out.is_empty() {
            eprintln!("{}", tr!("どのシナリオにも、同じ名前の主張があります", "every scenario has a claim of its name").get(lang));
        } else {
            print!("{}", out.trim_end_matches('\n').to_string() + "\n");
        }
        return 0;
    }

    if o.json {
        let claim_j = |c: &Claim| J::Obj(vec![("spec".into(), J::Str(c.spec.clone())), ("number".into(), J::Num(c.number as f64)), ("name".into(), J::Str(c.name.clone())), ("line".into(), J::Num(c.line as f64))]);
        let specs_j: Vec<J> = files
            .iter()
            .map(|f| {
                let reqs: Vec<J> = f
                    .reqs
                    .iter()
                    .map(|r| {
                        let scen: Vec<J> = r
                            .scenarios
                            .iter()
                            .map(|s| {
                                J::Obj(vec![
                                    ("name".into(), J::Str(s.name.clone())),
                                    ("line".into(), J::Num(s.line as f64)),
                                    ("claims".into(), J::Arr(answering(&s.name).into_iter().map(claim_j).collect())),
                                    ("near".into(), J::Arr(near(&s.name).into_iter().map(claim_j).collect())),
                                ])
                            })
                            .collect();
                        J::Obj(vec![
                            ("name".into(), J::Str(r.name.clone())),
                            ("op".into(), r.op.map(|o| J::Str(o.word().into())).unwrap_or(J::Null)),
                            ("line".into(), J::Num(r.line as f64)),
                            ("scenarios".into(), J::Arr(scen)),
                        ])
                    })
                    .collect();
                J::Obj(vec![("path".into(), J::Str(f.path.clone())), ("delta".into(), J::Bool(f.delta)), ("requirements".into(), J::Arr(reqs))])
            })
            .collect();
        let j = J::Obj(vec![
            ("geas".into(), J::Num(1.0)),
            ("specs".into(), J::Arr(specs_j)),
            ("claims_no_scenario_names".into(), J::Arr(unnamed.iter().map(|c| claim_j(c)).collect())),
            ("scenarios".into(), J::Num(total as f64)),
            ("answered".into(), J::Num(answered as f64)),
            ("unanswered".into(), J::Num((total - answered) as f64)),
            ("exit".into(), J::Num(exit as f64)),
        ]);
        println!("{}", json::render(&j));
        return exit;
    }

    let mut out: Vec<Text> = Vec::new();
    let where_ = |c: &Claim| -> Text { tr!("{} の主張 {}（{} 行目）", "claim {} of {} (line {})", c.spec, c.number, c.line; c.number, c.spec, c.line) };
    for f in &files {
        out.push(if f.delta { tr!("{}（変更の提案の差分）", "{} (a change's delta spec)", f.path; f.path) } else { Text::same(f.path.clone()) });
        if f.reqs.is_empty() {
            out.push(tr!("  シナリオを持つ要件はありません", "  no requirement with scenarios"));
        }
        for r in &f.reqs {
            out.push(match r.op {
                Some(op) => Text::same(format!("  {} {}", op.word(), r.name)),
                None => Text::same(format!("  {}", r.name)),
            });
            if r.scenarios.is_empty() {
                out.push(tr!("    シナリオがありません", "    no scenario"));
            }
            for s in &r.scenarios {
                let cs = answering(&s.name);
                if cs.is_empty() {
                    let mut t = tr!("    {}: 主張がありません", "    {}: no claim", s.name; s.name);
                    if let Some(n) = near(&s.name).first() {
                        let w = where_(n);
                        t = t.then(&tr!("。近い名前の主張: 「{}」、{}", "; a claim of a near name: {}, {}", n.name, w.ja; json::quote(&n.name), w.en));
                    }
                    out.push(t);
                } else {
                    let ja: Vec<String> = cs.iter().map(|c| where_(c).ja).collect();
                    let en: Vec<String> = cs.iter().map(|c| where_(c).en).collect();
                    out.push(tr!("    {}: {}", "    {}: {}", s.name, ja.join("、"); s.name, en.join(", ")));
                }
            }
        }
    }
    if unnamed.is_empty() {
        out.push(tr!("どのシナリオとも名前が合わない主張: なし", "claims no scenario names: none"));
    } else {
        out.push(tr!("どのシナリオとも名前が合わない主張:", "claims no scenario names:"));
        for c in &unnamed {
            let w = where_(c);
            out.push(tr!("  {}: {}", "  {}: {}", c.name, w.ja; json::quote(&c.name), w.en));
        }
    }
    let none = total - answered;
    out.push(tr!(
        "シナリオ {total} 個 · 主張のあるもの {answered} 個 · 主張の無いもの {none} 個",
        "{total} scenarios · {answered} with a claim · {none} with none"
    ));
    for t in out {
        println!("{}", t.get(lang));
    }
    exit
}
