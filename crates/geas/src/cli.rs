//! The `geas` command (`geas::cli::run`): the binary of this crate runs it, and so does `ritsu
//! geas` (ritsu's DESIGN 8.2); and `geas check` of the specs of a project, as `ritsu check`
//! prints it ([`checked`]).

use crate::{affected, codes, diag, drift, map, model, parse, proc, report, run, sched, skill};

use diag::{Diag, Show};
use ritsu_base::text::{Lang, Text};
use run::ClaimStatus;
use std::io::Write as _;
use std::path::{Path, PathBuf};

const HELP_EN: &str = "\
geas: hold agent-written code to claims a person has read
Coding agents: `geas skill` prints the guide; `geas skill --install <dir>` installs it.

usage:
  geas check <spec.geas>...           run every claim; exit 0 when all hold
  geas snap <spec.geas>...            run them and keep every observation as the baseline
  geas drift <spec.geas>...           run them again and report what changed since the baseline
  geas map <spec.geas>...             run them with coverage on and record the lines each claim runs
  geas affected <spec.geas> <diff|->  the claims a diff touches, and the changed code no claim runs
  geas explain <code>... | --all      what a code means and how to fix it
  geas skill [--install <dir>]        the guide for coding agents, or the guide written as a skill folder

options:
  --json            the answer as JSON
  --lang ja         messages in Japanese (also GEAS_LANG=ja)
  --jobs N, -j N    run up to N claims at once (also GEAS_JOBS); default 1
  --root <dir>      map, affected: the directory the record's paths are relative to
  --out <file>      map: where to write the record
  --map <file>      affected: a record to read; give two for both sides of the diff
  --install <dir>   skill: write the skill's files to <dir>/geas
  --force           skill: write over a <dir>/geas that is already there
  --help, -h        this text
  --version         the version

exit: 0 all held, or nothing to report · 1 something failed or changed · 2 the spec, a file, or the arguments are wrong
";

const HELP_JA: &str = "\
geas: エージェントが書いたコードに、人が読んで確かめた主張を守らせる
コーディングエージェント向け: `geas skill` が手引きを出します。`geas skill --install <dir>` でスキルとしてインストールできます。

使い方:
  geas check <spec.geas>...           主張をすべて実行する。すべて成り立てば終了コード 0
  geas snap <spec.geas>...            主張を実行し、結果をすべてベースラインとして残す
  geas drift <spec.geas>...           もう一度実行し、ベースラインから変わったところを示す
  geas map <spec.geas>...             カバレッジを取りながら実行し、主張ごとに通った行を記録する
  geas affected <spec.geas> <diff|->  差分が関わる主張と、変わったコードのうちどの主張も通らないところを示す
  geas explain <code>... | --all      コードの意味と直し方
  geas skill [--install <dir>]        コーディングエージェント向けの手引きを出すか、スキルのフォルダーとして書く

オプション:
  --json            JSON で出力する
  --lang ja         メッセージを日本語で出す（GEAS_LANG=ja でも同じ）
  --jobs N, -j N    主張を一度に N 個まで並列に走らせる（GEAS_JOBS でも同じ）。既定は 1
  --root <dir>      map、affected: 記録のパスの基準にするディレクトリ
  --out <file>      map: 記録を書く先
  --map <file>      affected: 読む記録。変更前と変更後のコードの記録を二つ渡せる
  --install <dir>   skill: スキルのファイルを <dir>/geas に書く
  --force           skill: すでにある <dir>/geas に上書きする
  --help, -h        この説明を出す
  --version         バージョンを出す

終了コード: 0 すべて成り立った、または報告することがない · 1 成り立たない主張や変化、どの主張も通らないコードの変更がある · 2 主張のファイルや引数に誤りがあるか、ファイルを読み書きできない
";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Cmd {
    Check,
    Snap,
    Drift,
    Map,
    Affected,
    Explain,
    Skill,
}

/// The options that take a value, and how many times each may be given.
const VALUE_FLAGS: &[(&str, usize)] = &[("--lang", 1), ("--jobs", 1), ("--root", 1), ("--out", 1), ("--map", 2), ("--install", 1)];

impl Cmd {
    fn parse(s: &str) -> Option<Cmd> {
        match s {
            "check" => Some(Cmd::Check),
            "snap" => Some(Cmd::Snap),
            "drift" => Some(Cmd::Drift),
            "map" => Some(Cmd::Map),
            "affected" => Some(Cmd::Affected),
            "explain" => Some(Cmd::Explain),
            "skill" => Some(Cmd::Skill),
            _ => None,
        }
    }

    fn word(self) -> &'static str {
        match self {
            Cmd::Check => "check",
            Cmd::Snap => "snap",
            Cmd::Drift => "drift",
            Cmd::Map => "map",
            Cmd::Affected => "affected",
            Cmd::Explain => "explain",
            Cmd::Skill => "skill",
        }
    }

    /// The options the command takes, besides `--help` and `--version`.
    fn options(self) -> &'static [&'static str] {
        match self {
            Cmd::Explain => &["--all", "--json", "--lang"],
            Cmd::Skill => &["--install", "--force", "--lang"],
            Cmd::Map => &["--json", "--lang", "--jobs", "--root", "--out"],
            Cmd::Affected => &["--json", "--lang", "--root", "--map"],
            _ => &["--json", "--lang", "--jobs"],
        }
    }
}

struct Args {
    cmd: Cmd,
    files: Vec<String>,
    json: bool,
    all: bool,
    lang: Lang,
    root: Option<String>,
    out: Option<String>,
    maps: Vec<String>,
    /// Claims at once: `--jobs`, else `GEAS_JOBS`, else 1.
    jobs: usize,
    /// `skill`: the directory to write the skill folder into, and whether it may be
    /// written over.
    install: Option<String>,
    force: bool,
}

enum Parsed {
    Help(Lang),
    Version,
    Run(Args),
}

fn e080(en: impl Into<String>, ja: impl Into<String>) -> Diag {
    diag::error("E080", 0, 0, Text::new(ja, en))
}

fn commands_note() -> Text {
    tr!(
        "コマンドは check、snap、drift、map、affected、explain、skill です。詳しくは `geas --help` を見てください",
        "the commands: check, snap, drift, map, affected, explain, skill; `geas --help` says more",
    )
}

/// The command line. An error comes back in the language `--lang` asked for when
/// that much could be read.
fn parse_args(raw: &[String]) -> Result<Parsed, (Diag, Lang)> {
    let mut lang_flag = None;
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == "--lang" {
            let lang = diag::pick(None);
            match raw.get(i + 1) {
                Some(v) => match diag::parse_lang(v) {
                    Some(l) => lang_flag = Some(l),
                    None => {
                        return Err((
                            e080(
                                format!("`--lang` takes en or ja, not `{v}`"),
                                format!("`--lang` に渡せるのは en か ja で、`{v}` は使えません"),
                            ),
                            lang,
                        ));
                    }
                },
                None => {
                    return Err((e080("`--lang` needs a value: en or ja", "`--lang` には値（en か ja）が要ります"), lang));
                }
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    let lang = diag::pick(lang_flag);
    let fail = |d: Diag| Err((d, lang));

    let mut word: Option<&str> = None;
    let mut files: Vec<String> = Vec::new();
    let mut flags: Vec<&str> = Vec::new();
    let mut values: Vec<(&str, String)> = Vec::new();
    let mut it = raw.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--help" | "-h" => return Ok(Parsed::Help(lang)),
            "--version" => return Ok(Parsed::Version),
            // `-j N` and `-jN`, as make and cargo take them
            "-j" => {
                let Some(v) = it.next() else {
                    return fail(e080("`-j` needs a value", "`-j` には値が要ります"));
                };
                flags.push("--jobs");
                values.push(("--jobs", v.clone()));
            }
            j if j.starts_with("-j") && j.len() > 2 => {
                flags.push("--jobs");
                values.push(("--jobs", j[2..].to_string()));
            }
            f if VALUE_FLAGS.iter().any(|(v, _)| *v == f) => {
                let Some(v) = it.next() else {
                    return fail(e080(format!("`{f}` needs a value"), format!("`{f}` には値が要ります")));
                };
                flags.push(f);
                values.push((f, v.clone()));
            }
            // `-` alone is stdin, an argument like any other
            s if s.starts_with('-') && s.len() > 1 => flags.push(s),
            s if word.is_none() => word = Some(s),
            s => files.push(s.to_string()),
        }
    }
    let Some(word) = word else {
        return fail(
            e080(
                "no command given; geas runs as `geas <command> …`",
                "コマンドがありません。geas は `geas <コマンド> …` の形で走らせます",
            )
            .note(commands_note()),
        );
    };
    let Some(cmd) = Cmd::parse(word) else {
        return fail(
            e080(format!("`{word}` is not a command"), format!("`{word}` というコマンドはありません")).note(commands_note()),
        );
    };
    let c = cmd.word();
    for f in &flags {
        if !cmd.options().contains(f) {
            return fail(
                e080(
                    format!("`{f}` is not an option of `geas {c}`"),
                    format!("`{f}` は `geas {c}` のオプションではありません"),
                )
                .note(tr!(
                    "使えるオプション: {}",
                    "it takes: {}",
                    cmd.options().join("、");
                    cmd.options().join(", "),
                )),
            );
        }
    }
    for (flag, most) in VALUE_FLAGS {
        let n = values.iter().filter(|(f, _)| f == flag).count();
        if n > *most && *flag != "--lang" {
            return fail(if *most == 1 {
                e080(format!("`{flag}` is given {n} times; give it once"), format!("`{flag}` が {n} 回あります。一回だけ渡してください"))
            } else {
                e080(
                    format!("`{flag}` is given {n} times; it takes at most {most}: a record of the code before the change and one of the code after it"),
                    format!("`{flag}` が {n} 回あります。渡せるのは {most} 回までで、変更前のコードの記録と変更後のコードの記録です"),
                )
            });
        }
    }
    let value = |flag: &str| values.iter().find(|(f, _)| *f == flag).map(|(_, v)| v.clone());
    let all = flags.contains(&"--all");
    let jobs = match value("--jobs") {
        None => None,
        Some(v) => match v.parse::<usize>() {
            Ok(n) if n >= 1 => Some(n),
            _ => {
                return fail(e080(
                    format!("`--jobs` takes a whole number from 1, not `{v}`"),
                    format!("`--jobs` に渡せるのは 1 以上の整数で、`{v}` は使えません"),
                ));
            }
        },
    };
    match cmd {
        Cmd::Explain if all && !files.is_empty() => {
            return fail(e080(
                "give `geas explain` codes or `--all`, not both",
                "`geas explain` に渡すのは、コードか `--all` のどちらかです",
            ));
        }
        Cmd::Explain if !all && files.is_empty() => {
            return fail(e080(
                "`geas explain` needs a code, or `--all`",
                "`geas explain` にはコードか `--all` が要ります",
            ));
        }
        Cmd::Explain => {}
        Cmd::Skill if !files.is_empty() => {
            return fail(
                e080(
                    format!("`geas skill` takes no file, and was given {} argument(s)", files.len()),
                    format!("`geas skill` はファイルを受け取りませんが、引数が {} 個渡されています", files.len()),
                )
                .note(tr!(
                    "使い方: geas skill [--install <dir> [--force]]",
                    "usage: geas skill [--install <dir> [--force]]",
                )),
            );
        }
        Cmd::Skill if flags.contains(&"--force") && value("--install").is_none() => {
            return fail(e080(
                "`--force` goes with `--install <dir>`: it lets geas write over the skill folder there",
                "`--force` は `--install <dir>` と一緒に使います。そこにあるスキルのフォルダーへの上書きを許すオプションです",
            ));
        }
        Cmd::Skill => {}
        Cmd::Affected if files.len() != 2 => {
            return fail(
                e080(
                    format!("`geas affected` takes a spec and a diff, and was given {} argument(s)", files.len()),
                    format!("`geas affected` に渡すのは主張のファイルと差分の二つで、渡された引数は {} 個です", files.len()),
                )
                .note(tr!(
                    "使い方: geas affected <spec.geas> <diff|->。`-` なら差分を標準入力から読みます",
                    "usage: geas affected <spec.geas> <diff|->; `-` reads the diff from stdin",
                )),
            );
        }
        _ if files.is_empty() => {
            return fail(
                e080(
                    format!("`geas {c}` needs at least one spec"),
                    format!("`geas {c}` には主張のファイルが一つ以上要ります"),
                )
                .note(tr!("使い方: geas {c} <spec.geas>...", "usage: geas {c} <spec.geas>...")),
            );
        }
        Cmd::Map if files.len() > 1 && value("--out").is_some() => {
            return fail(e080(
                format!("`--out` names one record, and `geas map` was given {} specs", files.len()),
                format!("`--out` で書ける記録は一つですが、`geas map` に主張のファイルが {} 個渡されています", files.len()),
            ));
        }
        _ => {}
    }
    Ok(Parsed::Run(Args {
        cmd,
        files,
        json: flags.contains(&"--json"),
        all,
        lang,
        root: value("--root"),
        out: value("--out"),
        maps: values.iter().filter(|(f, _)| *f == "--map").map(|(_, v)| v.clone()).collect(),
        jobs: sched::jobs(jobs),
        install: value("--install"),
        force: flags.contains(&"--force"),
    }))
}

/// Where a spec's files go: beside it, under `.geas/`, named after it.
struct Paths {
    /// The directory the targets' commands run in.
    cwd: PathBuf,
    geas: PathBuf,
    journal: PathBuf,
    baseline: PathBuf,
    record: PathBuf,
    /// The name the spike gave every spec's baseline.
    old_baseline: PathBuf,
    /// The spec's file name without `.geas`.
    stem: String,
}

fn paths(file: &str) -> Paths {
    let p = Path::new(file);
    let parent = p.parent().filter(|d| !d.as_os_str().is_empty());
    let cwd = parent.map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
    let geas = match parent {
        Some(d) => d.join(".geas"),
        None => PathBuf::from(".geas"),
    };
    let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "spec".into());
    Paths {
        cwd,
        journal: geas.join(format!("{stem}.journal.jsonl")),
        baseline: geas.join(format!("{stem}.baseline.jsonl")),
        record: geas.join(format!("{stem}.map.jsonl")),
        old_baseline: geas.join("baseline.jsonl"),
        geas,
        stem,
    }
}

fn shown(p: &Path) -> String {
    p.display().to_string()
}

/// A spec geas could not run: its diagnostics on stderr, or as JSON on stdout.
fn fail(a: &Args, file: &str, diags: &[(String, Diag)], src: &str) {
    if a.json {
        println!("{}", report::failure_json(file, diags, a.lang));
    } else {
        for (f, d) in diags {
            eprint!("{}", d.shown(f, src, a.lang));
        }
    }
}

/// Why `drift` cannot compare: the file the diagnostic points at, the diagnostic,
/// and that file's text.
fn baseline_problem(e: drift::BaselineError, file: &str, p: &Paths) -> (String, Diag, String) {
    let base = shown(&p.baseline);
    let again = tr!(
        "ベースラインは `geas snap` が書きます。`geas snap {file}` を走らせると書き直します",
        "`geas snap` writes the baseline; run `geas snap {file}` to write it again",
    );
    match e {
        drift::BaselineError::Missing => {
            let mut d = diag::error(
                "E050",
                0,
                0,
                tr!("{base} にベースラインがありません", "there is no baseline at {base}"),
            )
            .note(tr!(
                "先に `geas snap {file}` を走らせてください。ドリフトは今回の実行を、snap が残したものと比べます",
                "run `geas snap {file}` first; drift compares a run with what snap kept",
            ));
            if p.old_baseline.is_file() {
                let old = shown(&p.old_baseline);
                let new = p.baseline.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                d = d.note(tr!(
                    "{old} は前の名前のベースラインです。{new} に名前を変えるか、snap をやり直してください",
                    "{old} is a baseline under its older name; rename it to {new}, or snap again",
                ));
            }
            (file.to_string(), d, String::new())
        }
        drift::BaselineError::Unreadable(err) => (
            base,
            diag::error(
                "E081",
                0,
                0,
                tr!("ベースラインを読めません: {err}", "cannot read the baseline: {err}"),
            ),
            String::new(),
        ),
        drift::BaselineError::Bad { line, why } => {
            let text = std::fs::read_to_string(&p.baseline).unwrap_or_default();
            (base, diag::error("E051", line, 0, why).note(again), text)
        }
    }
}

fn write_journal(p: &Paths, journal: &[String]) -> Result<(), (String, Diag)> {
    if let Err(e) = std::fs::create_dir_all(&p.geas) {
        return Err((
            shown(&p.geas),
            diag::error(
                "E081",
                0,
                0,
                tr!("このディレクトリを作れません: {e}", "cannot make this directory: {e}"),
            ),
        ));
    }
    std::fs::write(&p.journal, journal.join("\n") + "\n").map_err(|e| {
        (
            shown(&p.journal),
            diag::error("E081", 0, 0, tr!("ジャーナルを書けません: {e}", "cannot write the journal: {e}")),
        )
    })
}

/// check, snap or drift on one spec; its exit status.
fn run_file(file: &str, a: &Args) -> i32 {
    let lang = a.lang;
    let src = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            let d = diag::error("E081", 0, 0, tr!("このファイルを読めません: {e}", "cannot read this file: {e}"));
            fail(a, file, &[(file.to_string(), d)], "");
            return 2;
        }
    };
    let spec = match parse::parse(&src) {
        Ok(s) => s,
        Err(diags) => {
            let ds: Vec<(String, Diag)> = diags.into_iter().map(|d| (file.to_string(), d)).collect();
            fail(a, file, &ds, &src);
            return 2;
        }
    };
    let p = paths(file);
    if a.cmd == Cmd::Map {
        return map_file(file, a, &src, &spec, &p);
    }
    // drift stops before running anything when it has nothing to compare with
    let baseline = if a.cmd == Cmd::Drift {
        match drift::read_baseline(&p.baseline) {
            Ok(b) => Some(b),
            Err(e) => {
                let (f, d, text) = baseline_problem(e, file, &p);
                fail(a, file, &[(f, d)], &text);
                return 2;
            }
        }
    } else {
        None
    };

    // what GUI targets write goes under `.geas/`, named absolutely, since the
    // programs that write it run in the spec's directory
    let geas_dir = std::fs::canonicalize(&p.cwd).map(|d| d.join(".geas")).unwrap_or_else(|_| p.geas.clone());
    let (results, journal) = run::run_spec(&spec, &p.cwd, &geas_dir, &p.stem, a.jobs);
    // a signal killed what the run started: nothing it says now would be true
    if let Some(sig) = proc::signalled() {
        return 128 + sig;
    }
    let mut problems: Vec<(String, Diag)> = Vec::new();
    if let Err(problem) = write_journal(&p, &journal) {
        problems.push(problem);
    }
    let mut code = if report::failed(&results) == 0 { 0 } else { 1 };
    match a.cmd {
        Cmd::Check => {
            if a.json {
                println!("{}", report::claims_json(file, &results, lang));
            } else {
                print!("{}", report::claims(file, &src, &results, lang, false));
                print!("{}", report::check_summary(&results, &shown(&p.journal), lang));
            }
        }
        Cmd::Snap => {
            if !a.json {
                print!("{}", report::claims(file, &src, &results, lang, true));
            }
            let written = drift::write_baseline(&p.baseline, &spec, &results);
            if a.json {
                println!("{}", report::claims_json(file, &results, lang));
            }
            match written {
                Ok(n) => {
                    if !a.json {
                        print!("{}", report::snap_summary(&results, n, &shown(&p.baseline), lang));
                    }
                }
                Err(e) => problems.push((
                    shown(&p.baseline),
                    diag::error(
                        "E081",
                        0,
                        0,
                        tr!("ベースラインを書けません: {e}", "cannot write the baseline: {e}"),
                    ),
                )),
            }
        }
        Cmd::Drift => {
            let r = drift::drift(&spec, &results, baseline.expect("read before the run"));
            let errored = results.iter().any(|x| matches!(x.status, ClaimStatus::Error(_)));
            if a.json {
                println!("{}", r.json(file, &report::error_diagnostics(file, &results, lang), lang));
            } else {
                for x in &results {
                    if let ClaimStatus::Error(d) = &x.status {
                        print!("{}", d.shown(file, &src, lang));
                    }
                }
                print!("{}", r.text(lang));
            }
            code = if errored {
                2
            } else if r.unclaimed + r.claimed > 0 {
                1
            } else {
                0
            };
        }
        Cmd::Map | Cmd::Affected | Cmd::Explain | Cmd::Skill => unreachable!("handled apart"),
    }
    for (f, d) in &problems {
        eprint!("{}", d.shown(f, "", lang));
        code = 2;
    }
    code
}

/// `geas map` on one spec: the claims run as `check` runs them, with coverage on,
/// and the record written unless something stopped it.
fn map_file(file: &str, a: &Args, src: &str, spec: &model::Spec, p: &Paths) -> i32 {
    let lang = a.lang;
    let early = |d: Diag| {
        fail(a, file, &[(file.to_string(), d)], "");
        2
    };
    let (root, spec_rel) = match map::root_and_spec(file, a.root.as_deref()) {
        Ok(x) => x,
        Err(d) => return early(d),
    };
    let cwd = match std::fs::canonicalize(&p.cwd) {
        Ok(c) => c,
        Err(e) => {
            return early(diag::error(
                "E081",
                0,
                0,
                tr!("主張のファイルのディレクトリを読めません: {e}", "cannot read the spec's directory: {e}"),
            ));
        }
    };
    let geas_dir = cwd.join(".geas");
    let place = map::Place { cwd: &p.cwd, geas_dir: &geas_dir, root: &root, spec_rel: &spec_rel };
    let mut journal = Vec::new();
    let m = match map::run(spec, &place, a.jobs, &mut journal) {
        Ok(m) => m,
        Err(d) => return early(d),
    };
    if let Some(sig) = proc::signalled() {
        return 128 + sig;
    }
    let mut problems: Vec<(String, Diag)> = Vec::new();
    if let Err(problem) = write_journal(p, &journal) {
        problems.push(problem);
    }
    let record_path = a.out.clone().unwrap_or_else(|| shown(&p.record));
    let mut written = None;
    if let Some(r) = &m.record {
        match std::fs::write(&record_path, r.render()) {
            Ok(()) => written = Some(r.clone()),
            Err(e) => problems.push((
                record_path.clone(),
                diag::error("E081", 0, 0, tr!("記録を書けません: {e}", "cannot write the record: {e}")),
            )),
        }
    }
    let mut code = if report::failed(&m.results) == 0 { 0 } else { 1 };
    if m.stopped() || !problems.is_empty() {
        code = 2;
    }
    if a.json {
        let diags: Vec<String> = m
            .diags
            .iter()
            .map(|d| d.json_in(file, lang))
            .chain(problems.iter().map(|(f, d)| d.json_in(f, lang)))
            .collect();
        let more = format!(",\"map\":{},\"diagnostics\":[{}]", map::json_part(&written, &record_path), diags.join(","));
        println!("{}", report::claims_json_with(file, &m.results, lang, code == 0, &more));
    } else {
        print!("{}", report::claims(file, src, &m.results, lang, false));
        print!("{}", report::check_summary(&m.results, &shown(&p.journal), lang));
        if let Some(r) = &written {
            print!("{}", map::summary(r, &record_path, lang));
        }
        for d in &m.diags {
            eprint!("{}", d.shown(file, src, lang));
        }
        for (f, d) in &problems {
            eprint!("{}", d.shown(f, "", lang));
        }
    }
    code
}

fn explain(a: &Args) -> i32 {
    let entries = if a.all {
        codes::table()
    } else {
        let mut v = Vec::new();
        for c in &a.files {
            match codes::find(c) {
                Some(e) => v.push(e),
                None => {
                    let d = e080(format!("`{c}` is not a code geas has"), format!("`{c}` というコードはありません")).note(tr!(
                        "`geas explain --all` で、すべてのコードが出ます",
                        "`geas explain --all` prints every code",
                    ));
                    eprint!("{}", d.shown("", "", a.lang));
                    return 2;
                }
            }
        }
        v
    };
    if a.json {
        for e in &entries {
            println!("{}", codes::render_json(e, a.lang));
        }
    } else {
        let texts: Vec<String> = entries.iter().map(|e| codes::render_text(e, a.lang)).collect();
        print!("{}", texts.join("\n"));
    }
    0
}

/// `geas skill`: the guide on stdout, or the skill folder written into a directory.
fn skill_command(a: &Args) -> i32 {
    let Some(dir) = &a.install else {
        print!("{}", skill::guide());
        return 0;
    };
    match skill::install(Path::new(dir), a.force) {
        Ok(folder) => {
            let n = skill::FILES.len();
            let shown = folder.display();
            println!(
                "{}",
                Text::new(&format!("geas のスキルを {shown} に書きました（ファイル {n} 個）"), &format!("wrote the geas skill to {shown} ({n} files)")).get(a.lang)
            );
            0
        }
        Err(d) => {
            eprint!("{}", d.shown("", "", a.lang));
            2
        }
    }
}

/// The `geas` command, on `raw` (the words after the program's name), printing to standard output
/// and standard error: the exit code. A signal stops the process groups of what it started first.
pub fn run(raw: &[String]) -> i32 {
    proc::stop_groups_on_signals();
    let code = match parse_args(raw) {
        Ok(Parsed::Help(lang)) => {
            print!("{}", Text::new(HELP_JA, HELP_EN).get(lang));
            0
        }
        Ok(Parsed::Version) => {
            println!("geas {}", env!("CARGO_PKG_VERSION"));
            0
        }
        Ok(Parsed::Run(a)) => match a.cmd {
            Cmd::Explain => explain(&a),
            Cmd::Skill => skill_command(&a),
            Cmd::Affected => affected::command(
                &a.files[0],
                &a.files[1],
                &affected::Opts { root: a.root.as_deref(), maps: &a.maps, json: a.json, lang: a.lang },
            ),
            _ => {
                let mut worst = 0;
                for f in &a.files {
                    worst = worst.max(run_file(f, &a));
                    if proc::signalled().is_some() {
                        break;
                    }
                }
                worst
            }
        },
        Err((d, lang)) => {
            eprint!("{}", d.shown("", "", lang));
            2
        }
    };
    let code = proc::signalled().map_or(code, |sig| 128 + sig);
    let _ = std::io::stdout().flush();
    code
}

/// `geas check` of each spec, as `ritsu check` prints it (ritsu's DESIGN 8.3): the claims run
/// as `geas check` runs them, the journal written beside the spec, and what the command prints —
/// a line for each claim, the diagnostic of a claim that could not run (a finding, with its JSON
/// as `--json` writes a diagnostic), the summary line. A spec that does not parse is its
/// diagnostics; one that cannot be read, or whose journal cannot be written, is a spec geas could
/// not check (it exits 2 for both). `files` are as the person gave them, from where the program
/// runs; `root` is the project's.
pub fn checked(root: &Path, files: &[String], lang: Lang) -> Vec<ritsu_ports::Checked> {
    use ritsu_ports::{Checked, Finding, Part, Verdict};
    let finding = |file: &str, d: &Diag, text: String| Finding {
        code: d.code.to_string(),
        severity: d.severity,
        file: ritsu_base::paths::from_root(root, Path::new(file)),
        line: d.line,
        text,
        json: ritsu_base::json::parse(&d.json_in(file, lang)).unwrap_or(ritsu_base::json::Json::Null),
    };
    let mut out = Vec::new();
    for file in files {
        let src = match std::fs::read_to_string(file) {
            Ok(s) => s,
            Err(e) => {
                let d = diag::error("E081", 0, 0, tr!("このファイルを読めません: {e}", "cannot read this file: {e}"));
                out.push(Checked { label: file.clone(), parts: vec![Part::Finding(finding(file, &d, d.shown(file, "", lang)))], verdict: Verdict::Unchecked });
                continue;
            }
        };
        let spec = match parse::parse(&src) {
            Ok(s) => s,
            Err(diags) => {
                let parts = diags.iter().map(|d| Part::Finding(finding(file, d, d.shown(file, &src, lang)))).collect();
                out.push(Checked { label: file.clone(), parts, verdict: Verdict::Fails });
                continue;
            }
        };
        let p = paths(file);
        let geas_dir = std::fs::canonicalize(&p.cwd).map(|d| d.join(".geas")).unwrap_or_else(|_| p.geas.clone());
        let (results, journal) = run::run_spec(&spec, &p.cwd, &geas_dir, &p.stem, sched::jobs(None));
        let mut parts: Vec<Part> = results
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let text = report::claim(file, &src, i + 1, r, lang, false);
                match &r.status {
                    ClaimStatus::Error(d) => Part::Finding(finding(file, d, text)),
                    _ => Part::Text(text),
                }
            })
            .collect();
        parts.push(Part::Text(report::check_summary(&results, &shown(&p.journal), lang)));
        let mut verdict = if report::failed(&results) == 0 { Verdict::Passes } else { Verdict::Fails };
        if let Err((f, d)) = write_journal(&p, &journal) {
            parts.push(Part::Finding(finding(&f, &d, d.shown(&f, "", lang))));
            verdict = Verdict::Unchecked;
        }
        out.push(Checked { label: file.clone(), parts, verdict });
        if proc::signalled().is_some() {
            break;
        }
    }
    out
}
