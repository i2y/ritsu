use dandori::check;
use dandori::commands;
use dandori::diag::{self, Lang};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "dandori — a small typed language for workflows that call business rules

Usage:
  dandori check <file.flow>...                    check: types, every arm, every state a case can be left in, retries
  dandori build <file.flow> --target asl|temporal|temporal-python|temporal-go|durable|argo|pydantic-graph [--out <dir>]
                                                  compile to AWS Step Functions (ASL, JSONata), to Temporal (TypeScript, Python or Go),
                                                  to AWS Lambda durable functions (TypeScript), to Argo Workflows (YAML),
                                                  or to pydantic-graph (Python); a build also refuses what the platform
                                                  cannot do, and a run that can outgrow its history
  dandori run <file.flow> --scenario <file.json> [--target reference|asl|temporal|temporal-python|temporal-go|durable|argo|pydantic-graph]
                                                  run the workflow in the reference interpreter against scripted answers,
                                                  and print the trace as the target would show it
  dandori scenarios <file.flow> [--out <dir>]     write scenarios that take every arm and every way a case can move
  dandori doc <file.flow> [--format html] [--out <dir>]
                                                  draw the workflow for the person who reviews it: the flow, what each
                                                  call does and where its errors go, every way it can end; Markdown with
                                                  Mermaid, or one HTML page where each scenario lights up the way it goes

Flags:
  --format json   machine-facing JSON (check)
  --format html   one HTML page (doc); Markdown by default
  --lang ja|en    language of the messages; else DANDORI_LANG, else en

The rules are read with rulec: DANDORI_RULEC names the binary, else `rulec` on the PATH.
Exit codes: 0 notes only / 1 errors found / 2 bad arguments or an unreadable file";

struct Args {
    cmd: String,
    files: Vec<PathBuf>,
    format_json: bool,
    format_html: bool,
    lang: Lang,
    target: Option<String>,
    out: Option<PathBuf>,
    scenario: Option<PathBuf>,
}

fn parse_args() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let cmd = it.next().ok_or_else(|| USAGE.to_string())?;
    if cmd == "--help" || cmd == "-h" || cmd == "help" {
        return Err(USAGE.to_string());
    }
    let mut a = Args { cmd, files: vec![], format_json: false, format_html: false, lang: Lang::En, target: None, out: None, scenario: None };
    let mut lang_flag: Option<String> = None;
    while let Some(x) = it.next() {
        match x.as_str() {
            "--format" => match it.next().as_deref() {
                Some("json") => a.format_json = true,
                Some("text") => a.format_json = false,
                Some("html") => a.format_html = true,
                Some("md") => a.format_html = false,
                _ => return Err("--format takes json or text (check), html or md (doc)".into()),
            },
            "--lang" => lang_flag = Some(it.next().ok_or("--lang takes ja or en")?),
            "--target" => a.target = Some(it.next().ok_or("--target takes asl, temporal, temporal-python, temporal-go, durable, argo, pydantic-graph or reference")?),
            "--out" => a.out = Some(PathBuf::from(it.next().ok_or("--out takes a directory")?)),
            "--scenario" => a.scenario = Some(PathBuf::from(it.next().ok_or("--scenario takes a file")?)),
            "--help" | "-h" => return Err(USAGE.to_string()),
            f if f.starts_with("--") => return Err(format!("unknown flag {f}\n\n{USAGE}")),
            f => a.files.push(PathBuf::from(f)),
        }
    }
    a.lang = Lang::pick(lang_flag.as_deref());
    Ok(a)
}

fn main() -> ExitCode {
    let a = match parse_args() {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("{msg}");
            return ExitCode::from(2);
        }
    };
    let code = match a.cmd.as_str() {
        "check" => cmd_check(&a),
        "build" => cmd_build(&a),
        "run" => cmd_run(&a),
        "scenarios" => cmd_scenarios(&a),
        "doc" => cmd_doc(&a),
        other => {
            eprintln!("unknown command {other}\n\n{USAGE}");
            2
        }
    };
    ExitCode::from(code)
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
        eprintln!("dandori check <file.flow>...");
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
            eprintln!("dandori build <file.flow> --target asl|temporal|temporal-python|temporal-go|durable|argo|pydantic-graph [--out <dir>]");
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
            eprintln!("dandori run <file.flow> --scenario <file.json> [--target reference|asl|temporal|temporal-python|temporal-go|durable|argo|pydantic-graph]");
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
            eprintln!("dandori scenarios <file.flow> [--out <dir>]");
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
            eprintln!("dandori doc <file.flow> [--format html] [--out <dir>]");
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
