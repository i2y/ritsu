mod drift;
mod json;
mod lex;
mod model;
mod parse;
mod run;

use run::{ClaimResult, ClaimStatus};
use std::path::Path;
use std::process::exit;

fn print_claim_failures(results: &[ClaimResult], file: &str) {
    for (i, r) in results.iter().enumerate() {
        match &r.status {
            ClaimStatus::Ok => {}
            ClaimStatus::Fail => {
                println!("not ok {} - {}", i + 1, r.name);
                for c in r.checks.iter().filter(|c| !c.ok) {
                    println!(
                        "    {}:{}: {} {} — got {}",
                        file, c.line, c.label, c.expected, c.actual
                    );
                }
            }
            ClaimStatus::Error { message, line } => {
                println!("not ok {} - {} (error)", i + 1, r.name);
                println!("    {}:{}: {}", file, line, message);
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut file: Option<String> = None;
    let mut as_json = false;
    let mut cmd = String::from("check");
    for a in &args {
        match a.as_str() {
            "--json" => as_json = true,
            "check" | "snap" | "drift" => cmd = a.clone(),
            _ => file = Some(a.clone()),
        }
    }
    let Some(file) = file else {
        eprintln!("usage: geas <check|snap|drift> <spec.geas> [--json]");
        exit(2);
    };
    let src = match std::fs::read_to_string(&file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}: {}", file, e);
            exit(2);
        }
    };
    let spec = match parse::parse(&src) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}:{}", file, e);
            exit(2);
        }
    };
    let dir0 = Path::new(&file).parent().unwrap_or(Path::new("."));
    let dir = if dir0.as_os_str().is_empty() { Path::new(".") } else { dir0 };
    let geas_dir = dir.join(".geas");
    let baseline_path = geas_dir.join("baseline.jsonl");
    let journal_path = geas_dir.join("journal.jsonl");

    // For drift, fail fast if there is no baseline before running anything.
    let pre_baseline = if cmd == "drift" {
        match drift::read_baseline(&baseline_path) {
            Ok(b) => Some(b),
            Err(e) => {
                eprintln!("{}", e);
                exit(2);
            }
        }
    } else {
        None
    };

    let mut journal: Vec<String> = Vec::new();
    let results = run::run_spec(&spec, dir, &mut journal);
    let _ = std::fs::create_dir_all(&geas_dir);
    let _ = std::fs::write(&journal_path, journal.join("\n") + "\n");

    let failed = results
        .iter()
        .filter(|r| !matches!(r.status, ClaimStatus::Ok))
        .count();

    match cmd.as_str() {
        "drift" => {
            let code = drift::drift(&spec, &results, pre_baseline.expect("read above"), &file);
            exit(code);
        }
        "snap" => {
            print_claim_failures(&results, &file);
            match drift::write_baseline(&baseline_path, &results) {
                Ok(n) => println!(
                    "{} claims · {} ok · {} failed · baseline: {} interactions → {}",
                    results.len(),
                    results.len() - failed,
                    failed,
                    n,
                    baseline_path.display()
                ),
                Err(e) => {
                    eprintln!("cannot write baseline: {}", e);
                    exit(2);
                }
            }
            if failed > 0 {
                println!("warning: baseline recorded from a run with failing claims");
            }
            exit(if failed == 0 { 0 } else { 1 });
        }
        _ => {}
    }

    if as_json {
        let claims: Vec<String> = results
            .iter()
            .map(|r| {
                let status = match &r.status {
                    ClaimStatus::Ok => "ok",
                    ClaimStatus::Fail => "fail",
                    ClaimStatus::Error { .. } => "error",
                };
                let error = match &r.status {
                    ClaimStatus::Error { message, line } => format!(
                        "{{\"line\":{},\"message\":\"{}\"}}",
                        line,
                        json::esc(message)
                    ),
                    _ => "null".into(),
                };
                let checks: Vec<String> = r
                    .checks
                    .iter()
                    .map(|c| {
                        format!(
                            "{{\"line\":{},\"check\":\"{}\",\"expected\":\"{}\",\"actual\":\"{}\",\"ok\":{}}}",
                            c.line,
                            json::esc(&c.label),
                            json::esc(&c.expected),
                            json::esc(&c.actual),
                            c.ok
                        )
                    })
                    .collect();
                format!(
                    "{{\"name\":\"{}\",\"line\":{},\"status\":\"{}\",\"error\":{},\"checks\":[{}]}}",
                    json::esc(&r.name),
                    r.line,
                    status,
                    error,
                    checks.join(",")
                )
            })
            .collect();
        println!(
            "{{\"ok\":{},\"file\":\"{}\",\"claims\":[{}]}}",
            failed == 0,
            json::esc(&file),
            claims.join(",")
        );
    } else {
        for (i, r) in results.iter().enumerate() {
            match &r.status {
                ClaimStatus::Ok => println!("ok {} - {}", i + 1, r.name),
                ClaimStatus::Fail => {
                    println!("not ok {} - {}", i + 1, r.name);
                    for c in r.checks.iter().filter(|c| !c.ok) {
                        println!(
                            "    {}:{}: {} {} — got {}",
                            file, c.line, c.label, c.expected, c.actual
                        );
                    }
                }
                ClaimStatus::Error { message, line } => {
                    println!("not ok {} - {} (error)", i + 1, r.name);
                    println!("    {}:{}: {}", file, line, message);
                }
            }
        }
        println!(
            "{} claims · {} ok · {} failed · journal: {}",
            results.len(),
            results.len() - failed,
            failed,
            journal_path.display()
        );
    }
    exit(if failed == 0 { 0 } else { 1 });
}
