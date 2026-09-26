use dandori::check;
use dandori::diag::{self, Lang};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "dandori — a small typed language for workflows that call business rules

Usage:
  dandori check <file.flow>...                    check: types, every arm, every state a case can be left in, retries
  dandori build <file.flow> --target asl|temporal|temporal-python|durable|argo|pydantic-graph [--out <dir>]
                                                  compile to AWS Step Functions (ASL, JSONata), to Temporal (TypeScript or Python),
                                                  to AWS Lambda durable functions (TypeScript), to Argo Workflows (YAML),
                                                  or to pydantic-graph (Python); a build also refuses what the platform
                                                  cannot do, and a run that can outgrow its history
  dandori run <file.flow> --scenario <file.json> [--target reference|asl|temporal|temporal-python|durable|argo|pydantic-graph]
                                                  run the workflow in the reference interpreter against scripted answers,
                                                  and print the trace as the target would show it
  dandori scenarios <file.flow> [--out <dir>]     write scenarios that take every arm and every way a case can move

Flags:
  --format json   machine-facing JSON (check)
  --lang ja|en    language of the messages; else DANDORI_LANG, else en

The rules are read with rulec: DANDORI_RULEC names the binary, else `rulec` on the PATH.
Exit codes: 0 notes only / 1 errors found / 2 bad arguments or an unreadable file";

struct Args {
    cmd: String,
    files: Vec<PathBuf>,
    format_json: bool,
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
    let mut a = Args { cmd, files: vec![], format_json: false, lang: Lang::En, target: None, out: None, scenario: None };
    let mut lang_flag: Option<String> = None;
    while let Some(x) = it.next() {
        match x.as_str() {
            "--format" => match it.next().as_deref() {
                Some("json") => a.format_json = true,
                Some("text") => a.format_json = false,
                _ => return Err("--format takes json or text".into()),
            },
            "--lang" => lang_flag = Some(it.next().ok_or("--lang takes ja or en")?),
            "--target" => a.target = Some(it.next().ok_or("--target takes asl, temporal, temporal-python, durable, argo, pydantic-graph or reference")?),
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
        for d in &checked.diags {
            eprint!("{}", d.render(&file, &src, a.lang));
        }
        if print_ok && checked.model.is_some() {
            let warnings = checked.diags.len();
            if a.lang == Lang::Ja {
                eprintln!("{file}: 検査を通りました{}", if warnings > 0 { format!("（警告 {warnings} 件）") } else { String::new() });
            } else {
                eprintln!("{file}: ok{}", if warnings > 0 { format!(" ({warnings} warning(s))") } else { String::new() });
            }
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
            eprintln!("dandori build <file.flow> --target asl|temporal|temporal-python|durable|argo|pydantic-graph [--out <dir>]");
            return 2;
        }
    };
    let model = match load(&file, a, false) {
        Ok(Some(m)) => m,
        Ok(None) => return 1,
        Err(c) => return c,
    };
    let out = a.out.clone().unwrap_or_else(|| PathBuf::from("out"));
    let files = match a.target.as_deref() {
        Some("asl") => dandori::asl::build(&model),
        Some("temporal") => dandori::temporal::build(&model),
        Some("temporal-python") => dandori::temporal_py::build(&model),
        Some("durable") => dandori::temporal::build_flavor(&model, dandori::temporal::Flavor::Durable),
        Some("argo") => dandori::argo::build(&model),
        Some("pydantic-graph") => dandori::pydantic_graph::build(&model),
        _ => {
            eprintln!("--target takes asl, temporal, temporal-python, durable, argo or pydantic-graph");
            return 2;
        }
    };
    let files = match files {
        Ok(f) => f,
        Err(diags) => {
            let src = std::fs::read_to_string(&file).unwrap_or_default();
            for d in &diags {
                eprint!("{}", d.render(&file.display().to_string(), &src, a.lang));
            }
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
            eprintln!("dandori run <file.flow> --scenario <file.json> [--target reference|asl|temporal|temporal-python|durable|argo|pydantic-graph]");
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
        // both SDKs put the same names on the wire
        Some("temporal") | Some("temporal-python") => dandori::render::View::Temporal,
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
