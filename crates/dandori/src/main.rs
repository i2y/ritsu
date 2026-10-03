//! The `dandori` command. What exists and what it takes is `cli.rs`'s table (ritsu's DESIGN 4.4).

use dandori::check;
use dandori::cli;
use dandori::commands;
use dandori::diag::{self, Lang, Text};
use ritsu_base::tr;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// What one command line asked for, as the commands below read it.
struct Args {
    files: Vec<PathBuf>,
    format_json: bool,
    format_html: bool,
    lang: Lang,
    target: Option<String>,
    out: Option<PathBuf>,
    scenario: Option<PathBuf>,
}

impl Args {
    fn from(a: &cli::Args, lang: Lang) -> Args {
        Args {
            files: a.pos.iter().map(PathBuf::from).collect(),
            format_json: a.get("--format") == Some("json"),
            format_html: a.get("--format") == Some("html"),
            lang,
            target: a.get("--target").map(str::to_string),
            out: a.get("--out").map(PathBuf::from),
            scenario: a.get("--scenario").map(PathBuf::from),
        }
    }
}

fn refuse(msg: Text, lang: Lang) -> ExitCode {
    let head = if lang == Lang::Ja { "エラー" } else { "error" };
    eprintln!("{head}: {}", msg.get(lang));
    ExitCode::from(2)
}

/// `--lang ja`, `--lang=ja`, anywhere on the line: decided before anything is printed.
fn lang_flag(args: &[String]) -> Option<String> {
    for (i, a) in args.iter().enumerate() {
        if a == "--lang" {
            return args.get(i + 1).cloned();
        }
        if let Some(v) = a.strip_prefix("--lang=") {
            return Some(v.to_string());
        }
    }
    None
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let lang = Lang::pick(lang_flag(&args).as_deref(), "DANDORI_LANG");
    let table = cli::table();
    let Some(first) = args.first() else {
        eprint!("{}", table.help_all(lang));
        return ExitCode::from(2);
    };
    match first.as_str() {
        "--help" | "-h" => {
            print!("{}", table.help_all(lang));
            return ExitCode::SUCCESS;
        }
        "--version" | "-V" => {
            println!("dandori {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        "help" => {
            let mut rest = Vec::new();
            let mut i = 1;
            while i < args.len() {
                if args[i] == "--lang" {
                    i += 2;
                    continue;
                }
                if !args[i].starts_with("--lang=") {
                    rest.push(args[i].clone());
                }
                i += 1;
            }
            return match rest.first() {
                None => {
                    print!("{}", table.help_all(lang));
                    ExitCode::SUCCESS
                }
                Some(n) => match table.command(n) {
                    Some(c) => {
                        print!("{}", table.help_cmd(c, lang));
                        ExitCode::SUCCESS
                    }
                    None => refuse(tr!("`{n}` というコマンドはありません。`dandori --help` を読んでください", "there is no command `{n}`; run `dandori --help`"), lang),
                },
            };
        }
        _ => {}
    }
    let Some(cmd) = table.command(first) else {
        return refuse(tr!("`{first}` というコマンドはありません。`dandori --help` を読んでください", "there is no command `{first}`; run `dandori --help`"), lang);
    };
    let parsed = match table.parse(cmd, &args[1..]) {
        Ok(a) => a,
        Err(e) => return refuse(e, lang),
    };
    if parsed.has("--help") {
        print!("{}", table.help_cmd(cmd, lang));
        return ExitCode::SUCCESS;
    }
    let a = Args::from(&parsed, lang);
    let code = match cmd.name {
        "check" => cmd_check(&a),
        "build" => cmd_build(&a),
        "run" => cmd_run(&a),
        "scenarios" => cmd_scenarios(&a),
        "doc" => cmd_doc(&a),
        _ => unreachable!("every command in the table is dispatched"),
    };
    ExitCode::from(code)
}

/// The line `dandori <cmd> --help` begins its usage with, for a command given the wrong number
/// of files.
fn usage(name: &str) -> String {
    let table = cli::table();
    table.command(name).map(|c| table.usage_line(c)).unwrap_or_default()
}

/// Check one file and print its diagnostics; the model when it passes.
fn load(path: &Path, a: &Args, print_ok: bool) -> Result<Option<dandori::model::Model>, u8> {
    let (src, checked) = match check::check_file(path) {
        Ok(x) => x,
        Err(msg) => {
            eprintln!("{msg}");
            return Err(2);
        }
    };
    let file = path.display().to_string();
    if a.format_json {
        let v: Vec<_> = checked.diags.iter().map(|d| d.to_json(a.lang)).collect();
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({ "file": file, "diagnostics": v })).unwrap());
    } else {
        eprint!("{}", commands::render(&checked.diags, &file, &src, a.lang));
        if print_ok && checked.model.is_some() {
            eprint!("{}", commands::passed(&file, checked.diags.len(), a.lang));
        }
    }
    if diag::has_errors(&checked.diags) {
        return Err(1);
    }
    Ok(checked.model)
}

fn cmd_check(a: &Args) -> u8 {
    if a.files.is_empty() {
        eprintln!("{}", usage("check"));
        return 2;
    }
    let mut worst = 0;
    for f in &a.files {
        if let Err(c) = load(f, a, true) {
            worst = worst.max(c);
        }
    }
    worst
}

fn cmd_build(a: &Args) -> u8 {
    let file = match a.files.as_slice() {
        [f] => f.clone(),
        _ => {
            eprintln!("{}", usage("build"));
            return 2;
        }
    };
    let model = match load(&file, a, false) {
        Ok(Some(m)) => m,
        Ok(None) => return 1,
        Err(c) => return c,
    };
    let out = a.out.clone().unwrap_or_else(|| PathBuf::from("out"));
    let Some(files) = a.target.as_deref().and_then(|t| commands::build(&model, t)) else {
        eprintln!("--target takes asl, temporal, temporal-python, temporal-go, durable, argo or pydantic-graph");
        return 2;
    };
    let files = match files {
        Ok(f) => f,
        Err(diags) => {
            let src = std::fs::read_to_string(&file).unwrap_or_default();
            eprint!("{}", commands::render(&diags, &file.display().to_string(), &src, a.lang));
            return 1;
        }
    };
    for (name, text) in files {
        let p = out.join(&name);
        if let Some(parent) = p.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                eprintln!("cannot create {}: {e}", parent.display());
                return 2;
            }
        }
        if let Err(e) = std::fs::write(&p, text) {
            eprintln!("cannot write {}: {e}", p.display());
            return 2;
        }
        println!("{}", p.display());
    }
    0
}

fn cmd_run(a: &Args) -> u8 {
    let file = match a.files.as_slice() {
        [f] => f.clone(),
        _ => {
            eprintln!("{}", usage("run"));
            return 2;
        }
    };
    let model = match load(&file, a, false) {
        Ok(Some(m)) => m,
        Ok(None) => return 1,
        Err(c) => return c,
    };
    let sc = match &a.scenario {
        Some(p) => match std::fs::read_to_string(p).map_err(|e| e.to_string()).and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string())) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("cannot read the scenario: {e}");
                return 2;
            }
        },
        None => {
            eprintln!("--scenario <file.json> is required");
            return 2;
        }
    };
    let view = match a.target.as_deref() {
        None | Some("reference") | Some("asl") => dandori::render::View::Asl,
        // the three SDKs put the same names on the wire
        Some("temporal") | Some("temporal-python") | Some("temporal-go") => dandori::render::View::Temporal,
        Some("durable") => dandori::render::View::Durable,
        Some("argo") => dandori::render::View::Argo,
        Some("pydantic-graph") => dandori::render::View::Graph,
        Some(o) => {
            eprintln!("unknown target {o}");
            return 2;
        }
    };
    match dandori::interp::run(&model, &sc, view) {
        Ok(trace) => {
            println!("{}", serde_json::to_string_pretty(&trace).unwrap());
            0
        }
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}

fn cmd_scenarios(a: &Args) -> u8 {
    let file = match a.files.as_slice() {
        [f] => f.clone(),
        _ => {
            eprintln!("{}", usage("scenarios"));
            return 2;
        }
    };
    let model = match load(&file, a, false) {
        Ok(Some(m)) => m,
        Ok(None) => return 1,
        Err(c) => return c,
    };
    let list = dandori::scenarios::generate(&model);
    match &a.out {
        Some(dir) => {
            if let Err(e) = std::fs::create_dir_all(dir) {
                eprintln!("cannot create {}: {e}", dir.display());
                return 2;
            }
            for (i, s) in list.iter().enumerate() {
                let p = dir.join(format!("{:03}.json", i + 1));
                if let Err(e) = std::fs::write(&p, serde_json::to_string_pretty(s).unwrap()) {
                    eprintln!("cannot write {}: {e}", p.display());
                    return 2;
                }
            }
            eprintln!("{} scenario(s) written to {}", list.len(), dir.display());
        }
        None => println!("{}", serde_json::to_string_pretty(&list).unwrap()),
    }
    0
}

fn cmd_doc(a: &Args) -> u8 {
    let file = match a.files.as_slice() {
        [f] => f.clone(),
        _ => {
            eprintln!("{}", usage("doc"));
            return 2;
        }
    };
    let (src, drawn) = match check::drawable(&file) {
        Ok(x) => x,
        Err(msg) => {
            eprintln!("{msg}");
            return 2;
        }
    };
    let shown = file.display().to_string();
    eprint!("{}", commands::render(&drawn.diags, &shown, &src, a.lang));
    // a workflow whose names or types do not resolve has nothing to draw
    let Some(model) = &drawn.model else { return 1 };
    let input = dandori::doc::Input { m: model, src: &src, file: &shown, facts: &drawn.facts, diags: &drawn.diags, lang: a.lang };
    let page = if a.format_html { dandori::doc::html(&input) } else { dandori::doc::markdown(&input) };
    match &a.out {
        Some(dir) => {
            if let Err(e) = std::fs::create_dir_all(dir) {
                eprintln!("cannot create {}: {e}", dir.display());
                return 2;
            }
            let stem = file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| model.name.clone());
            let p = dir.join(format!("{stem}.{}", if a.format_html { "html" } else { "md" }));
            if let Err(e) = std::fs::write(&p, page) {
                eprintln!("cannot write {}: {e}", p.display());
                return 2;
            }
            println!("{}", p.display());
        }
        None => print!("{page}"),
    }
    // the page is written even so: the runs of the errors are on it
    if diag::has_errors(&drawn.diags) {
        1
    } else {
        0
    }
}
