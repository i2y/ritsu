//! `rulec` の CLI。M0 は `check`。

use rulec::diag::{Severity, render, render_json};
use std::process::ExitCode;

fn usage() -> ExitCode {
    eprintln!(
        "rulec {}\n\n\
         使い方:\n  \
         rulec check <file.rule>...  [--format json] [--show-shadow] [--diff-base <rev>] [--budget N]\n  \
         rulec fmt   <file.rule>...  [--check]\n\n\
         exit code: 0 注記のみ / 1 エラーあり / 2 内部異常",
        env!("CARGO_PKG_VERSION")
    );
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        return usage();
    }
    let json = args.iter().any(|a| a == "--format=json")
        || args.windows(2).any(|w| w[0] == "--format" && w[1] == "json");
    // §4: 上から の部分的な遮蔽は方式の通常の姿なので、既定では数だけ出す。
    let show_shadow = args.iter().any(|a| a == "--show-shadow");
    // §4: 遮蔽の要確認と W114 は、基準リビジョンから新たに生じた分だけ浮かせる。
    let budget: i64 = args
        .windows(2)
        .find(|w| w[0] == "--budget")
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(rulec::region::DEFAULT_BUDGET);
    let diff_base = args
        .windows(2)
        .find(|w| w[0] == "--diff-base")
        .map(|w| w[1].clone());
    let skip: Vec<String> = diff_base
        .iter()
        .cloned()
        .chain(["json".to_string(), budget.to_string()])
        .collect();
    let files: Vec<&String> = args
        .iter()
        .filter(|a| !a.starts_with("--") && !skip.contains(a))
        .skip(1)
        .collect();

    match args[0].as_str() {
        "check" => check(&files, json, show_shadow, diff_base.as_deref(), budget),
        "fmt" => fmt(&files, args.iter().any(|a| a == "--check")),
        _ => usage(),
    }
}

/// §1.5: 唯一の整形器。`--check` は CI 用で、直すべきファイルを並べて 1 で終わる。
fn fmt(files: &[&String], check_only: bool) -> ExitCode {
    if files.is_empty() {
        return usage();
    }
    let mut dirty = 0u8;
    for path in files {
        let Ok(src) = std::fs::read_to_string(path) else {
            eprintln!("error: `{path}` を読めません");
            return ExitCode::from(2);
        };
        let out = rulec::fmt::format(&src);
        if out == src {
            continue;
        }
        if check_only {
            println!("整形されていません: {path}");
            dirty = 1;
        } else if std::fs::write(path, &out).is_err() {
            eprintln!("error: `{path}` に書けません");
            return ExitCode::from(2);
        } else {
            println!("整形しました: {path}");
        }
    }
    ExitCode::from(dirty)
}

/// 基準リビジョンでの同じファイルを取り出す。無ければ None（そのファイルは新規）。
fn base_source(rev: &str, path: &str) -> Option<String> {
    let out = std::process::Command::new("git")
        .args(["show", &format!("{rev}:{path}")])
        .output()
        .ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn check(files: &[&String], json: bool, show_shadow: bool, diff_base: Option<&str>, budget: i64) -> ExitCode {
    if files.is_empty() {
        return usage();
    }
    let mut worst = 0u8;
    for path in files {
        let Ok(src) = std::fs::read_to_string(path) else {
            eprintln!("error: `{path}` を読めません");
            return ExitCode::from(2);
        };
        let lines: Vec<String> = src.lines().map(|s| s.to_string()).collect();
        let r = rulec::report_with(&src, path, budget);
        let mut diags = r.diags;

        // 対の同一性はセルの正規形で照合する。行番号だと、行を一つ挿入した
        // だけで全部が新規になる。
        let mut suppressed = 0usize;
        if let Some(rev) = diff_base {
            let known: std::collections::HashSet<String> = match base_source(rev, path) {
                Some(b) => rulec::report(&b, path).diags.iter().filter_map(|d| d.key.clone()).collect(),
                None => Default::default(),
            };
            let before = diags.len();
            diags.retain(|d| match &d.key {
                Some(k) => !known.contains(k),
                None => true,
            });
            suppressed = before - diags.len();
        }
        if show_shadow {
            diags.extend(r.quiet);
        }
        for d in &diags {
            if d.severity == Severity::Error {
                worst = worst.max(1);
            }
            if json {
                println!("{}", render_json(d, path));
            } else {
                print!("{}", render(d, &lines));
                println!();
            }
        }
        if json {
            continue;
        }
        if suppressed > 0 && !json {
            println!("note {path}: 基準リビジョンに既にあった発見 {suppressed} 件は伏せました（--diff-base）");
        }
        // §4: 要確認だけが一覧に出て、残りは件数一行。
        let s = r.shadow;
        if s.total() > 0 {
            println!(
                "note {path}: 遮蔽 {} 対（構造的 {}、同値 {}、要確認 {}）",
                s.total(),
                s.structural,
                s.equivalent,
                s.confirm
            );
        }
        if !rulec::has_error(&diags) {
            println!("ok {path}");
        }
    }
    ExitCode::from(worst)
}
