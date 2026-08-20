//! `rulec` の CLI。M0 は `check`。

use rulec::diag::{Severity, render, render_json};
use std::process::ExitCode;

fn usage() -> ExitCode {
    eprintln!(
        "rulec {}\n\n\
         使い方:\n  \
         rulec check <file.rule>...  [--format json] [--show-shadow] [--diff-base <rev>] [--budget N]\n  \
         rulec fmt   <file.rule>...  [--check]\n  \
         rulec gen   <file.rule>...  [--out DIR] [--check]\n  \
         rulec vectors <file.rule>... [--out DIR]\n  \
         rulec schema  <file.rule>\n  \
         rulec adapter <file.rule> --template python|go\n  \
         rulec verify  <file.rule> --adapter <cmd> [args...]\n\n\
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
    // `--x V` の V と、値そのものが引数に見えるものを除いてファイル名を拾う。
    let mut skip: Vec<String> = vec!["json".into(), budget.to_string()];
    for k in ["--out", "--diff-base", "--budget", "--format", "--template"] {
        if let Some(w) = args.windows(2).find(|w| w[0] == k) {
            skip.push(w[1].clone());
        }
    }
    let files: Vec<&String> = args
        .iter()
        .filter(|a| !a.starts_with("--") && !skip.contains(a))
        .skip(1)
        .collect();

    match args[0].as_str() {
        "check" => check(&files, json, show_shadow, diff_base.as_deref(), budget),
        "fmt" => fmt(&files, args.iter().any(|a| a == "--check")),
        "schema" => one(&files, |f, c, _| Some(rulec::verify::schema(f, c))),
        "adapter" => {
            let lang = args
                .windows(2)
                .find(|w| w[0] == "--template")
                .map(|w| w[1].clone())
                .unwrap_or_else(|| "python".into());
            one(&files, move |f, _, _| Some(rulec::verify::template(&lang, f)))
        }
        "verify" => {
            let i = args.iter().position(|a| a == "--adapter");
            let Some(i) = i else {
                eprintln!("error: --adapter <cmd> が要ります");
                return ExitCode::from(2);
            };
            let cmd: Vec<String> = args[i + 1..].to_vec();
            // --adapter の後ろはコマンドなので、ファイル名の候補から外す。
            let before: Vec<String> = args[..i].to_vec();
            let vfiles: Vec<&String> = before
                .iter()
                .filter(|a| !a.starts_with("--") && !skip.contains(a))
                .skip(1)
                .collect();
            verify(&vfiles, &cmd)
        }
        "vectors" => {
            let out = args
                .windows(2)
                .find(|w| w[0] == "--out")
                .map(|w| w[1].clone());
            vectors(&files, out.as_deref())
        }
        "gen" => {
            let out = args
                .windows(2)
                .find(|w| w[0] == "--out")
                .map(|w| w[1].clone())
                .unwrap_or_else(|| "generated".into());
            generate(&files, &out, args.iter().any(|a| a == "--check"))
        }
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

/// §8.4: 生成物は git にコミットし、CI の `--check` が再生成との一致を見る。
fn generate(files: &[&String], out_dir: &str, check_only: bool) -> ExitCode {
    if files.is_empty() {
        return usage();
    }
    let mut dirty = 0u8;
    for path in files {
        let Ok(src) = std::fs::read_to_string(path) else {
            eprintln!("error: `{path}` を読めません");
            return ExitCode::from(2);
        };
        let (f, c) = match rulec::prepare(&src, path) {
            Ok(v) => v,
            Err(ds) => {
                let lines: Vec<String> = src.lines().map(|s| s.to_string()).collect();
                for d in &ds {
                    print!("{}", render(d, &lines));
                    println!();
                }
                eprintln!("error: `{path}` は検査を通っていないので生成しません");
                return ExitCode::from(1);
            }
        };
        let g = rulec::codegen::Gen::new(&f, &c, &src);
        let alias = f.name.ascii.clone().unwrap_or_else(|| f.name.text.clone());
        let pkg = alias.replace('_', "").to_lowercase();
        // ベクタと期待値も出す。§9.3 の三つの使い道のうち、言語間一致テストと
        // golden がこれで回る。
        let vs = rulec::vectors::generate(&f, &c);
        let vec_body: String =
            vs.iter().map(|v| rulec::vectors::to_json(&f, v)).collect::<Vec<_>>().join("\n") + "\n";
        let exp_body: String =
            vs.iter().map(|v| rulec::vectors::expected_json(&f, v)).collect::<Vec<_>>().join("\n") + "\n";
        let targets = [
            (format!("{out_dir}/python/{alias}.py"), g.python()),
            (format!("{out_dir}/python/{alias}_runner.py"), g.python_runner()),
            (format!("{out_dir}/go/{pkg}/{alias}.go"), g.go()),
            (format!("{out_dir}/go/{pkg}/go.mod"), format!("module {pkg}\n\ngo 1.25\n")),
            (format!("{out_dir}/go/{pkg}runner/main.go"), g.go_runner()),
            (
                format!("{out_dir}/go/{pkg}runner/go.mod"),
                format!("module {pkg}runner\n\ngo 1.25\n\nrequire {pkg} v0.0.0\n\nreplace {pkg} => ../{pkg}\n"),
            ),
            (format!("{out_dir}/vectors/{alias}.jsonl"), vec_body),
            (format!("{out_dir}/vectors/{alias}.expected.jsonl"), exp_body),
            // 丸めヘルパの単体ベクタ（§8.5）。表の一致だけでは隠れる誤りを踏む。
            (format!("{out_dir}/python/_round_test.py"), rulec::codegen::round_tests_python()),
            (format!("{out_dir}/go/{pkg}/round_test.go"), rulec::codegen::round_tests_go(&pkg)),
        ];
        for (p, body) in targets {
            let existing = std::fs::read_to_string(&p).ok();
            if existing.as_deref() == Some(body.as_str()) {
                continue;
            }
            if check_only {
                println!("生成物が古いか手で編集されています: {p}");
                dirty = 1;
                continue;
            }
            if let Some(dir) = std::path::Path::new(&p).parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if std::fs::write(&p, &body).is_err() {
                eprintln!("error: `{p}` に書けません");
                return ExitCode::from(2);
            }
            println!("生成しました: {p}");
        }
    }
    ExitCode::from(dirty)
}

/// §9: 境界からベクタを作る。期待値と発火行は参照評価器が付ける。
fn vectors(files: &[&String], out_dir: Option<&str>) -> ExitCode {
    if files.is_empty() {
        return usage();
    }
    for path in files {
        let Ok(src) = std::fs::read_to_string(path) else {
            eprintln!("error: `{path}` を読めません");
            return ExitCode::from(2);
        };
        let Ok((f, c)) = rulec::prepare(&src, path) else {
            eprintln!("error: `{path}` は検査を通っていないのでベクタを作りません");
            return ExitCode::from(1);
        };
        let vs = rulec::vectors::generate(&f, &c);
        let body: String = vs
            .iter()
            .map(|v| rulec::vectors::to_json(&f, v))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        match out_dir {
            Some(d) => {
                let alias = f.name.ascii.clone().unwrap_or_else(|| f.name.text.clone());
                let p = format!("{d}/{alias}.jsonl");
                let _ = std::fs::create_dir_all(d);
                if std::fs::write(&p, &body).is_err() {
                    eprintln!("error: `{p}` に書けません");
                    return ExitCode::from(2);
                }
                println!("ベクタ {} 件: {p}", vs.len());
            }
            None => print!("{body}"),
        }
    }
    ExitCode::from(0)
}

/// 一本の規則に対して文字列を吐くだけの下位コマンド。
fn one(
    files: &[&String],
    f: impl Fn(&rulec::ast::RuleFile, &rulec::types::Checked, &str) -> Option<String>,
) -> ExitCode {
    if files.is_empty() {
        return usage();
    }
    for path in files {
        let Ok(src) = std::fs::read_to_string(path) else {
            eprintln!("error: `{path}` を読めません");
            return ExitCode::from(2);
        };
        let Ok((rf, c)) = rulec::prepare(&src, path) else {
            eprintln!("error: `{path}` は検査を通っていません");
            return ExitCode::from(1);
        };
        if let Some(s) = f(&rf, &c, path) {
            print!("{s}");
        }
    }
    ExitCode::from(0)
}

/// §10: 旧実装のアダプタにベクタを流し、突き合わせる。
fn verify(files: &[&String], adapter: &[String]) -> ExitCode {
    if files.is_empty() || adapter.is_empty() {
        return usage();
    }
    let mut worst = 0u8;
    for path in files {
        let Ok(src) = std::fs::read_to_string(path) else {
            eprintln!("error: `{path}` を読めません");
            return ExitCode::from(2);
        };
        let Ok((f, c)) = rulec::prepare(&src, path) else {
            eprintln!("error: `{path}` は検査を通っていません");
            return ExitCode::from(1);
        };
        let vs = rulec::vectors::generate(&f, &c);
        match rulec::verify::run(&f, &c, adapter, &vs) {
            Ok(rep) => {
                print!("{}", rulec::verify::render(&rep, &f));
                if !rep.mismatches.is_empty() {
                    worst = 1;
                }
            }
            Err(e) => {
                eprintln!("error: {e}");
                return ExitCode::from(2);
            }
        }
    }
    ExitCode::from(worst)
}
