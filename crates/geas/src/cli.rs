//! The `geas` command (`geas::cli::run`): the binary of this crate runs it, and so does `ritsu
//! geas` (ritsu's DESIGN 8.2); and `geas check` of the specs of a project, as `ritsu check`
//! prints it ([`checked`]).

use crate::{affected, codes, contract, diag, drift, map, model, parse, proc, report, run, scenarios, sched, skill};

use diag::{Diag, Show};
use ritsu_base::cli::Misuse;
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
  geas scenarios <spec.geas>... --openspec <path>...
                                      the scenarios of OpenSpec specs, each with the claims of its name
  geas explain <code>... | --all      what a code means and how to fix it
  geas skill [--install <dir>]        the guide for coding agents, or the guide written as a skill folder

options:
  --json            the answer as JSON
  --lang ja         messages in Japanese (also GEAS_LANG=ja)
  --jobs N, -j N    run up to N claims at once (also GEAS_JOBS); default 1
  --root <dir>      map, affected: the directory the record's paths are relative to
  --out <file>      map: where to write the record
  --map <file>      affected: a record to read; give two for both sides of the diff
  --openspec <path> scenarios: a spec, a change's delta spec, or a directory of them
  --draft           scenarios: a claim to fill in for each scenario no claim answers
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
  geas scenarios <spec.geas>... --openspec <path>...
                                      OpenSpec の仕様のシナリオごとに、同じ名前の主張を示す
  geas explain <code>... | --all      コードの意味と直し方
  geas skill [--install <dir>]        コーディングエージェント向けの手引きを出すか、スキルのフォルダーとして書く

オプション:
  --json            JSON で出力する
  --lang ja         メッセージを日本語で出す（GEAS_LANG=ja でも同じ）
  --jobs N, -j N    主張を一度に N 個まで並列に走らせる（GEAS_JOBS でも同じ）。既定は 1
  --root <dir>      map、affected: 記録のパスの基準にするディレクトリ
  --out <file>      map: 記録を書く先
  --map <file>      affected: 読む記録。変更前と変更後のコードの記録を二つ渡せる
  --openspec <path> scenarios: 仕様、変更の提案の差分、それらを持つディレクトリ
  --draft           scenarios: 主張の無いシナリオごとに、書き足す主張の下書きを出す
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
    Scenarios,
}

/// The options that take a value, and how many times each may be given.
const VALUE_FLAGS: &[(&str, usize)] = &[("--lang", 1), ("--jobs", 1), ("--root", 1), ("--out", 1), ("--map", 2), ("--install", 1), ("--openspec", usize::MAX)];

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
            "scenarios" => Some(Cmd::Scenarios),
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
            Cmd::Scenarios => "scenarios",
        }
    }

    /// The options the command takes, besides `--help` and `--version`.
    fn options(self) -> &'static [&'static str] {
        match self {
            Cmd::Explain => &["--all", "--json", "--lang"],
            Cmd::Skill => &["--install", "--force", "--lang"],
            Cmd::Map => &["--json", "--lang", "--jobs", "--root", "--out"],
            Cmd::Affected => &["--json", "--lang", "--root", "--map"],
            Cmd::Scenarios => &["--json", "--lang", "--openspec", "--draft"],
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
    /// `scenarios`: the OpenSpec specs, delta specs and directories, and whether to draft claims.
    openspec: Vec<String>,
    draft: bool,
}

enum Parsed {
    Help(Lang),
    Version,
    Run(Args),
}

/// `geas --help`, the commands named the way the person ran this one: `ritsu geas check …` through
/// `ritsu` (ritsu's DESIGN 6.2, item 10).
fn help(lang: Lang) -> String {
    let page = Text::new(HELP_JA, HELP_EN).get(lang).to_string();
    let run = ritsu_base::cli::command_name("geas");
    if run == "geas" {
        return page;
    }
    // the line that goes on under a command's own keeps its column
    let more = " ".repeat(run.chars().count() - "geas".len());
    let column = format!("\n{}", " ".repeat(38));
    page.replace(&column, &format!("{column}{more}")).replace("\n  geas ", &format!("\n  {run} ")).replace("`geas ", &format!("`{run} "))
}

fn e080(en: impl Into<String>, ja: impl Into<String>) -> Diag {
    diag::error("E080", 0, 0, Text::new(ja, en))
}

/// E080 in the words every command of ritsu stops a command line with (ritsu's DESIGN 6.2, item
/// 10): the sentence, and the usage line it puts on its next line as a note.
fn e080_said(t: Text) -> Diag {
    let split = |s: &str| match s.split_once('\n') {
        Some((head, rest)) => (head.to_string(), Some(rest.trim().to_string())),
        None => (s.to_string(), None),
    };
    let ((ja, ja_note), (en, en_note)) = (split(&t.ja), split(&t.en));
    let d = diag::error("E080", 0, 0, Text::new(ja, en));
    match (ja_note, en_note) {
        (Some(j), Some(e)) => d.note(Text::new(j, e)),
        _ => d,
    }
}

/// `geas <cmd>` as the person runs it: `ritsu geas <cmd>` through `ritsu`.
fn geas_cmd(c: &str) -> String {
    format!("{} {c}", ritsu_base::cli::command_name("geas"))
}

/// The placeholder of each flag that takes a value, as `geas --help` writes it.
fn placeholder(flag: &str) -> &'static str {
    match flag {
        "--lang" => "en|ja",
        "--jobs" | "-j" => "<n>",
        "--root" | "--install" => "<dir>",
        "--out" | "--map" => "<file>",
        _ => "<path>",
    }
}

fn commands_note() -> Text {
    let run = ritsu_base::cli::command_name("geas");
    tr!(
        "コマンドは check、snap、drift、map、affected、scenarios、explain、skill です。詳しくは `{run} --help` を見てください",
        "the commands: check, snap, drift, map, affected, scenarios, explain, skill; `{run} --help` says more",
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
                        let m = Misuse::NotAChoice { flag: "--lang", value: v.clone(), choices: &["en", "ja"] };
                        return Err((e080_said(m.text("geas", "")), lang));
                    }
                },
                None => {
                    let m = Misuse::MissingValue { flag: "--lang", placeholder: placeholder("--lang") };
                    return Err((e080_said(m.text("geas", "")), lang));
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
                    return fail(e080_said(Misuse::MissingValue { flag: "-j", placeholder: placeholder("-j") }.text("geas", "")));
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
                    let flag = VALUE_FLAGS.iter().find(|(v, _)| *v == f).map_or("--lang", |(v, _)| *v);
                    return fail(e080_said(Misuse::MissingValue { flag, placeholder: placeholder(f) }.text("geas", "")));
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
        let run = ritsu_base::cli::command_name("geas");
        return fail(
            e080(format!("no command given; geas runs as `{run} <command> …`"), format!("コマンドがありません。geas は、`{run} <コマンド> …` の形で走らせてください"))
                .note(commands_note()),
        );
    };
    let Some(cmd) = Cmd::parse(word) else {
        return fail(e080_said(ritsu_base::cli::no_command("geas", word)));
    };
    let c = cmd.word();
    for f in &flags {
        if !cmd.options().contains(f) {
            let m = Misuse::UnknownFlag(f.to_string());
            return fail(e080_said(m.text_among(&ritsu_base::cli::command_name("geas"), c, cmd.options())));
        }
    }
    // a flag given a second time stops the line there, in the words every command of ritsu says it
    // in, however many times it was given; `--map` takes two (the records before and after the change)
    for (i, f) in flags.iter().enumerate() {
        let n = flags[..=i].iter().filter(|g| *g == f).count();
        let most = if *f == "--map" { 2 } else { 1 };
        if n > most && *f != "--lang" {
            if most == 1 {
                let name = cmd.options().iter().chain(["--jobs"].iter()).find(|o| **o == *f).copied().unwrap_or("--jobs");
                return fail(e080_said(Misuse::Twice(name).text("geas", "")));
            }
            return fail(e080(
                format!("`{f}` is given {n} times; it takes at most {most}: a record of the code before the change and one of the code after it"),
                format!("`{f}` が {n} 回あります。渡せるのは {most} 回までで、変更前のコードの記録と変更後のコードの記録です"),
            ));
        }
    }
    let value = |flag: &str| values.iter().find(|(f, _)| *f == flag).map(|(_, v)| v.clone());
    let all = flags.contains(&"--all");
    let jobs = match value("--jobs") {
        None => None,
        Some(v) => match v.parse::<usize>() {
            Ok(n) if n >= 1 => Some(n),
            _ => return fail(e080_said(ritsu_base::cli::not_positive("--jobs", &v))),
        },
    };
    match cmd {
        // codes and `--all` are the two ways to call it, not one: what it takes, as without either
        Cmd::Explain if all == !files.is_empty() => {
            return fail(e080_said(ritsu_base::cli::takes(&geas_cmd("explain"), "<code>... | --all", "geas explain <code>... | --all [--json]")));
        }
        Cmd::Explain => {}
        Cmd::Skill if !files.is_empty() => {
            return fail(e080_said(ritsu_base::cli::takes(&geas_cmd("skill"), "", "geas skill [--install <dir> [--force]]")));
        }
        Cmd::Skill if flags.contains(&"--force") && value("--install").is_none() => {
            return fail(e080(
                "`--force` goes with `--install <dir>`: it lets geas write over the skill folder there",
                "`--force` は、`--install <dir>` と一緒に使ってください。そこにあるスキルのフォルダーへの上書きを許すオプションです",
            ));
        }
        Cmd::Skill => {}
        Cmd::Affected if files.len() != 2 => {
            return fail(
                e080_said(ritsu_base::cli::takes(&geas_cmd("affected"), "<spec.geas> <diff|->", "geas affected <spec.geas> <diff|-> [--root <dir>] [--map <file>]... [--json]"))
                    .note(tr!("`-` なら差分を標準入力から読みます", "`-` reads the diff from stdin")),
            );
        }
        _ if files.is_empty() => {
            let usage = match cmd {
                Cmd::Scenarios => "geas scenarios <spec.geas>... --openspec <path>... [--draft] [--json]".to_string(),
                Cmd::Map => "geas map <spec.geas>... [--root <dir>] [--out <file>] [--jobs <n>] [--json]".to_string(),
                _ => format!("geas {c} <spec.geas>... [--jobs <n>] [--json]"),
            };
            return fail(e080_said(ritsu_base::cli::takes(&geas_cmd(c), "<spec.geas>...", &usage)));
        }
        Cmd::Scenarios if value("--openspec").is_none() => {
            return fail(e080_said(ritsu_base::cli::needs(&geas_cmd("scenarios"), "--openspec <path>", "geas scenarios <spec.geas>... --openspec <path>... [--draft] [--json]")));
        }
        Cmd::Scenarios if flags.contains(&"--draft") && flags.contains(&"--json") => {
            return fail(e080("`--draft` writes claims, and `--json` a report: give one of them", "`--draft` は主張を書き、`--json` は報告を出します。どちらか一つを渡してください"));
        }
        Cmd::Map if files.len() > 1 && value("--out").is_some() => {
            let c = geas_cmd("map");
            return fail(e080(
                format!("`--out` names one record, and `{c}` was given {} specs", files.len()),
                format!("`--out` で書ける記録は一つですが、`{c}` に主張のファイルが {} 個渡されています", files.len()),
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
        openspec: values.iter().filter(|(f, _)| *f == "--openspec").map(|(_, v)| v.clone()).collect(),
        draft: flags.contains(&"--draft"),
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

/// A spec as a run reads it: parsed, its contracts read and its calls held to them (DESIGN §20),
/// with the warnings that leaves; or what stops it.
fn with_contracts(src: &str, p: &Paths) -> Result<(model::Spec, Vec<Diag>), Vec<Diag>> {
    let mut spec = parse::parse(src)?;
    let mut said = contract::read(&mut spec, &p.cwd);
    said.append(&mut spec.warnings);
    diag::sort(&mut said);
    if said.iter().any(Diag::is_error) { Err(said) } else { Ok((spec, said)) }
}

/// A spec geas could not run: its diagnostics on stderr, or as JSON on stdout. What stops the
/// run before its work (E080 for the line, E081 for a file) is said on stderr with the JSON too,
/// as every command of ritsu says it (ritsu's DESIGN 6.2, item 10).
fn fail(a: &Args, file: &str, diags: &[(String, Diag)], src: &str) {
    if a.json {
        println!("{}", report::failure_json(file, diags, a.lang));
    }
    for (f, d) in diags.iter().filter(|(_, d)| !a.json || matches!(d.code, "E080" | "E081")) {
        eprint!("{}", d.shown(f, src, a.lang));
    }
}

/// Why `drift` cannot compare: the file the diagnostic points at, the diagnostic,
/// and that file's text.
fn baseline_problem(e: drift::BaselineError, file: &str, p: &Paths) -> (String, Diag, String) {
    let base = shown(&p.baseline);
    // the commands named the way the person ran this one (ritsu's DESIGN 6.2, item 10)
    let run = ritsu_base::cli::command_name("geas");
    let again = tr!(
        "ベースラインは `{run} snap` が書きます。`{run} snap {file}` を走らせると、ベースラインが書き直されます",
        "`{run} snap` writes the baseline; run `{run} snap {file}` to write it again",
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
                "先に `{run} snap {file}` を走らせてください。`{run} drift` は、今回の実行を snap が残したものと比べます",
                "run `{run} snap {file}` first; drift compares a run with what snap kept",
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
        drift::BaselineError::Unreadable(err) => {
            let d = diag::named("E081", ritsu_base::cli::cannot_read(&base, &err));
            (base, d, String::new())
        }
        drift::BaselineError::Bad { line, why } => {
            let text = std::fs::read_to_string(&p.baseline).unwrap_or_default();
            (base, diag::error("E051", line, 0, why).note(again), text)
        }
    }
}

fn write_journal(p: &Paths, journal: &[String]) -> Result<(), (String, Diag)> {
    if let Err(e) = ritsu_base::fs::create_dir_all(&p.geas) {
        let dir = shown(&p.geas);
        let d = diag::named("E081", ritsu_base::cli::cannot_create(&dir, &e));
        return Err((dir, d));
    }
    ritsu_base::fs::write(&p.journal, journal.join("\n") + "\n").map_err(|e| {
        let journal = shown(&p.journal);
        let d = diag::named("E081", ritsu_base::cli::cannot_write(&journal, &e));
        (journal, d)
    })
}

/// check, snap or drift on one spec; its exit status.
fn run_file(file: &str, a: &Args) -> i32 {
    let lang = a.lang;
    let src = match ritsu_base::fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            let d = diag::named("E081", ritsu_base::cli::cannot_read(file, &e));
            fail(a, file, &[(file.to_string(), d)], "");
            return 2;
        }
    };
    // `check` says the keys written in the spec (W901), whatever else it holds
    let mut keys = if a.cmd == Cmd::Check { diag::keys(&src) } else { Vec::new() };
    let p = paths(file);
    let (spec, warnings) = match with_contracts(&src, &p) {
        Ok(s) => s,
        Err(diags) => {
            let ds: Vec<(String, Diag)> = keys.into_iter().chain(diags).map(|d| (file.to_string(), d)).collect();
            fail(a, file, &ds, &src);
            return 2;
        }
    };
    if a.cmd == Cmd::Check {
        keys.extend(warnings);
    }
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
    let geas_dir = ritsu_base::fs::canonicalize(&p.cwd).map(|d| d.join(".geas")).unwrap_or_else(|_| p.geas.clone());
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
                // the keys of the spec (W901), `[]` when it has none: the shape a reader reads does not
                // change with the spec, as `map`'s diagnostics do not
                let more = format!(",\"diagnostics\":[{}]", keys.iter().map(|d| d.json_in(file, lang)).collect::<Vec<_>>().join(","));
                println!("{}", report::claims_json_with(file, &results, lang, report::failed(&results) == 0, &more));
            } else {
                for d in &keys {
                    print!("{}", d.shown(file, &src, lang));
                }
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
                Err(e) => {
                    let base = shown(&p.baseline);
                    let d = diag::named("E081", ritsu_base::cli::cannot_write(&base, &e));
                    problems.push((base, d));
                }
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
        Cmd::Map | Cmd::Affected | Cmd::Explain | Cmd::Skill | Cmd::Scenarios => unreachable!("handled apart"),
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
    let cwd = match ritsu_base::fs::canonicalize(&p.cwd) {
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
        match ritsu_base::fs::write(&record_path, r.render()) {
            Ok(()) => written = Some(r.clone()),
            Err(e) => problems.push((record_path.clone(), diag::named("E081", ritsu_base::cli::cannot_write(&record_path, &e)))),
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
        // every code it does not have, named in one sentence before anything is printed; a line
        // that asks for JSON gets it as the E080 of a line it cannot run (ritsu's DESIGN 6.2, item 10)
        let unknown: Vec<String> = a.files.iter().filter(|c| codes::find(c).is_none()).cloned().collect();
        if !unknown.is_empty() {
            let d = e080_said(ritsu_base::cli::no_codes("geas", &unknown));
            eprint!("{}", d.shown("", "", a.lang));
            if a.json {
                println!("{}", report::failure_json("", &[(String::new(), d)], a.lang));
            }
            return 2;
        }
        a.files.iter().filter_map(|c| codes::find(c)).collect()
    };
    if a.json {
        // one code is one indented document; `--all` and two or more codes, one indented array,
        // as every tool's `explain` writes them (ritsu's DESIGN 6.2, item 10)
        let each: Vec<ritsu_base::json::Json> = entries.iter().map(|e| ritsu_base::json::parse(&codes::render_json(e, a.lang)).expect("geas reads the JSON it writes")).collect();
        if each.len() == 1 && !a.all {
            println!("{}", each[0].pretty());
        } else {
            println!("{}", ritsu_base::json::Json::Arr(each).pretty());
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
            print!("{}", help(lang));
            0
        }
        Ok(Parsed::Version) => {
            println!("geas {}", env!("CARGO_PKG_VERSION"));
            0
        }
        // a path that starts with `@`, in a module, is read as a path written in a file is (ritsu's
        // DESIGN 17.6): what is wrong with it, as this file cannot be read (E081)
        Ok(Parsed::Run(a)) if a.files.iter().any(|f| ritsu_base::modules::given(f).is_err()) => {
            for f in &a.files {
                if let Err(t) = ritsu_base::modules::given(f) {
                    fail(&a, f, &[(f.clone(), diag::named("E081", t))], "");
                    break;
                }
            }
            2
        }
        Ok(Parsed::Run(mut a)) => {
            // the paths the run holds are written the way the first path given was (ritsu's
            // DESIGN 4.15), and so are the files given: `./orders.geas` and `x/../orders.geas`
            // are `orders.geas` (6.2 item 8), and the paths `--out`, `--map`, `--openspec` and
            // `--install` take; the codes `explain` takes are no paths
            let _way = ritsu_base::paths::run_from(a.files.first().filter(|_| !matches!(a.cmd, Cmd::Explain)).map_or(".", String::as_str));
            if !matches!(a.cmd, Cmd::Explain) {
                for f in &mut a.files {
                    *f = ritsu_base::paths::show_given(f);
                }
            }
            for p in a.out.iter_mut().chain(a.install.iter_mut()).chain(a.maps.iter_mut()).chain(a.openspec.iter_mut()) {
                *p = ritsu_base::paths::fold_given(p);
            }
            run_parsed(&a)
        }
        Err((d, lang)) => {
            eprint!("{}", d.shown("", "", lang));
            // a line that asks for JSON anywhere gets the E080 in the JSON a spec that cannot be
            // read gets its E081, whatever the command: a program reading it has one JSON
            // whatever stopped the run (ritsu's DESIGN 6.2, item 10)
            if raw.iter().any(|w| w == "--json") {
                println!("{}", report::failure_json("", &[(String::new(), d)], lang));
            }
            2
        }
    };
    let code = proc::signalled().map_or(code, |sig| 128 + sig);
    let _ = std::io::stdout().flush();
    code
}

/// A command line read, run: the exit code.
fn run_parsed(a: &Args) -> i32 {
    match a.cmd {
        Cmd::Explain => explain(a),
        Cmd::Skill => skill_command(a),
        Cmd::Scenarios => scenarios::command(&a.files, &scenarios::Opts { openspec: &a.openspec, draft: a.draft, json: a.json, lang: a.lang }),
        Cmd::Affected => affected::command(
            &a.files[0],
            &a.files[1],
            &affected::Opts { root: a.root.as_deref(), maps: &a.maps, json: a.json, lang: a.lang },
        ),
        _ => {
            let mut worst = 0;
            for f in &a.files {
                worst = worst.max(run_file(f, a));
                if proc::signalled().is_some() {
                    break;
                }
            }
            worst
        }
    }
}

/// `geas check` of each spec, as `ritsu check` prints it (ritsu's DESIGN 8.3): the claims run
/// as `geas check` runs them, the journal written beside the spec, and what the command prints —
/// a line for each claim, the diagnostic of a claim that could not run (a finding, with its JSON
/// as `--json` writes a diagnostic), the summary line. A spec that does not parse is its
/// diagnostics; one that cannot be read, or whose journal cannot be written, is a spec geas could
/// not check (it exits 2 for both). `files` are as the person gave them, from where the program
/// runs; `root` is the project's.
pub fn checked(root: &Path, files: &[String], lang: Lang) -> Vec<ritsu_ports::Checked> {
    specs_checked(root, files, true, lang)
}

/// [`checked`] with no claim run and no journal written: a file that cannot be read, what does not
/// parse, what a contract says of the calls, and the keys written in a spec. For a project of someone
/// else's, whose claims would run that person's programs here, and for a revision not on the disk.
pub fn read(root: &Path, files: &[String], lang: Lang) -> Vec<ritsu_ports::Checked> {
    specs_checked(root, files, false, lang)
}

fn specs_checked(root: &Path, files: &[String], run: bool, lang: Lang) -> Vec<ritsu_ports::Checked> {
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
        let src = match ritsu_base::fs::read_to_string(file) {
            Ok(s) => s,
            Err(e) => {
                let d = diag::named("E081", ritsu_base::cli::cannot_read(file, &e));
                let json = ritsu_base::json::parse(&report::failure_json(file, &[(file.clone(), d.clone())], lang)).unwrap_or(ritsu_base::json::Json::Null);
                out.push(Checked { label: file.clone(), parts: vec![Part::Finding(finding(file, &d, d.shown(file, "", lang)))], verdict: Verdict::Unchecked, json });
                continue;
            }
        };
        ritsu_base::tally::count("geas", Path::new(file));
        // the keys written in the spec (W901), whatever else it holds
        let mut key_diags = diag::keys(&src);
        let p = paths(file);
        let spec = match with_contracts(&src, &p) {
            Ok((s, warnings)) => {
                key_diags.extend(warnings);
                s
            }
            Err(diags) => {
                let parts = key_diags.iter().chain(&diags).map(|d| Part::Finding(finding(file, d, d.shown(file, &src, lang)))).collect();
                // the object `geas check --json` prints of a spec it could not run
                let ds: Vec<(String, Diag)> = key_diags.iter().chain(&diags).map(|d| (file.clone(), d.clone())).collect();
                let json = ritsu_base::json::parse(&report::failure_json(file, &ds, lang)).unwrap_or(ritsu_base::json::Json::Null);
                out.push(Checked { label: file.clone(), parts, verdict: Verdict::Fails, json });
                continue;
            }
        };
        let keys: Vec<Part> = key_diags.iter().map(|d| Part::Finding(finding(file, d, d.shown(file, &src, lang)))).collect();
        if !run {
            let n = spec.claims.len();
            let mut parts = keys;
            parts.push(Part::Text(report::read_summary(n, lang)));
            let more = format!(",\"claims_not_run\":{n},\"diagnostics\":[{}]", key_diags.iter().map(|d| d.json_in(file, lang)).collect::<Vec<_>>().join(","));
            let json = ritsu_base::json::parse(&report::claims_json_with(file, &[], lang, true, &more)).unwrap_or(ritsu_base::json::Json::Null);
            out.push(Checked { label: file.clone(), parts, verdict: Verdict::Passes, json });
            continue;
        }
        let geas_dir = ritsu_base::fs::canonicalize(&p.cwd).map(|d| d.join(".geas")).unwrap_or_else(|_| p.geas.clone());
        let (results, journal) = run::run_spec(&spec, &p.cwd, &geas_dir, &p.stem, sched::jobs(None));
        let mut parts: Vec<Part> = keys;
        parts.extend(results.iter().enumerate().map(|(i, r)| {
            let text = report::claim(file, &src, i + 1, r, lang, false);
            match &r.status {
                ClaimStatus::Error(d) => Part::Finding(finding(file, d, text)),
                _ => Part::Text(text),
            }
        }));
        parts.push(Part::Text(report::check_summary(&results, &shown(&p.journal), lang)));
        let mut verdict = if report::failed(&results) == 0 { Verdict::Passes } else { Verdict::Fails };
        if let Err((f, d)) = write_journal(&p, &journal) {
            parts.push(Part::Finding(finding(&f, &d, d.shown(&f, "", lang))));
            verdict = Verdict::Unchecked;
        }
        // the object `geas check --json` prints of the run, with the keys of the spec as its
        // diagnostics
        let more = format!(",\"diagnostics\":[{}]", key_diags.iter().map(|d| d.json_in(file, lang)).collect::<Vec<_>>().join(","));
        let json = ritsu_base::json::parse(&report::claims_json_with(file, &results, lang, report::failed(&results) == 0, &more)).unwrap_or(ritsu_base::json::Json::Null);
        out.push(Checked { label: file.clone(), parts, verdict, json });
        if proc::signalled().is_some() {
            break;
        }
    }
    out
}

