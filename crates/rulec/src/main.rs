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
         rulec coverage <file.rule>...\n  \
         rulec doc   <file.rule>...  [--out DIR]\n  \
         rulec test  <生成先ディレクトリ>\n  \
\
         rulec fixtures lint <file.jsonl> <file.rule> [--manifest m.json] [--fill 欄=値]\n  \
\
         rulec replay <file.rule> --fixtures <f.jsonl> [--manifest m.json] [--fill 欄=値] [--format markdown]\n  \
\
         rulec diff <旧> <新> --fixtures <f.jsonl> [同上]   # 旧/新 は file.rule か 送料@v3\n  \
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
    // §12: PR に貼る形。整形まで道具が持ち、投稿は CI の一行に任せる。
    let markdown = args.iter().any(|a| a == "--format=markdown")
        || args.windows(2).any(|w| w[0] == "--format" && w[1] == "markdown");
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
    let mut skip: Vec<String> = vec!["json".into(), "markdown".into(), budget.to_string()];
    // --fill は繰り返せるので、値を全部除く。
    for w in args.windows(2).filter(|w| w[0] == "--fill") {
        skip.push(w[1].clone());
    }
    for k in ["--out", "--diff-base", "--budget", "--format", "--template", "--fixtures", "--manifest"] {
        if let Some(w) = args.windows(2).find(|w| w[0] == k) {
            skip.push(w[1].clone());
        }
    }
    let raw: Vec<&String> = args
        .iter()
        .filter(|a| !a.starts_with("--") && !skip.contains(a))
        .skip(1)
        .collect();
    // §12 の CI は `rulec check rules/` と書く。ディレクトリは中の `.rule` に
    // 展開する。並び順は決定的（パス順）にして、報告の順が環境で変わらないようにする。
    // `test` だけは引数が生成先ディレクトリそのものなので展開しない。
    let expanded: Vec<String> = if args[0] == "test" {
        raw.iter().map(|a| (*a).clone()).collect()
    } else {
        raw.iter().flat_map(|a| expand(a)).collect()
    };
    let files: Vec<&String> = expanded.iter().collect();

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
        "coverage" => coverage(&files),
        "doc" => {
            let out = args.windows(2).find(|w| w[0] == "--out").map(|w| w[1].clone());
            doc(&files, out.as_deref())
        }
        "fixtures" => {
            // `rulec fixtures lint <jsonl> <rule>`
            if files.first().map(|s| s.as_str()) != Some("lint") {
                eprintln!("error: いまあるのは `rulec fixtures lint` だけです");
                return ExitCode::from(2);
            }
            fixtures_lint(&files[1..], &args)
        }
        "replay" => replay_cmd(&files, &args, markdown),
        "diff" => diff_cmd(&files, &args, markdown),
        "test" => {
            let Some(dir) = files.first() else { return usage() };
            match rulec::runtest::run(std::path::Path::new(dir.as_str())) {
                Ok(r) => {
                    print!("{}", rulec::runtest::render(&r));
                    ExitCode::from(u8::from(!r.ok()))
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::from(2)
                }
            }
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

/// 引数がディレクトリなら、その下の `.rule` を集める。ファイルならそのまま。
/// 見つからないディレクトリは空を返さず、そのままの名前を返して
/// 「読めません」で止める（黙って 0 件成功にしない）。
fn expand(arg: &str) -> Vec<String> {
    let p = std::path::Path::new(arg);
    if !p.is_dir() {
        return vec![arg.to_string()];
    }
    let mut out = Vec::new();
    collect_rules(p, &mut out);
    out.sort();
    if out.is_empty() {
        eprintln!("注意: `{arg}` の下に .rule がありません");
    }
    out
}

fn collect_rules(dir: &std::path::Path, out: &mut Vec<String>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_rules(&p, out);
        } else if p.extension().is_some_and(|x| x == "rule") {
            out.push(p.to_string_lossy().into_owned());
        }
    }
}

/// §1.6: check を通った規則を markdown に描画する。読み取り専用で、逆方向は無い。
/// **生成物としては扱わない** — コミットさせず、CI が生成して PR に貼る。
/// 古い描画が正の顔をして残るのが最大の危険なので、長生きする成果物を作らない。
fn doc(files: &[&String], out_dir: Option<&str>) -> ExitCode {
    if files.is_empty() {
        return usage();
    }
    for path in files {
        let Ok(src) = std::fs::read_to_string(path) else {
            eprintln!("error: `{path}` を読めません");
            return ExitCode::from(2);
        };
        // 壊れた規則の綺麗な描画は嘘になる。検査を通っていなければ描かない（§1.6）。
        let rep = rulec::report(&src, path);
        if rulec::has_error(&rep.diags) {
            let lines: Vec<String> = src.lines().map(|s| s.to_string()).collect();
            for d in rep.diags.iter().filter(|d| d.severity == rulec::diag::Severity::Error) {
                print!("{}", render(d, &lines));
                println!();
            }
            eprintln!("error: `{path}` は検査を通っていないので描画しません（§1.6）");
            return ExitCode::from(1);
        }
        let Ok((f, c)) = rulec::prepare(&src, path) else {
            eprintln!("error: `{path}` は検査を通っていません");
            return ExitCode::from(1);
        };
        let body = rulec::doc::render(&f, &c, &src, path);
        match out_dir {
            Some(d) => {
                let alias = f.name.ascii.clone().unwrap_or_else(|| f.name.text.clone());
                let p = format!("{d}/{alias}.md");
                if let Some(dir) = std::path::Path::new(&p).parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                if std::fs::write(&p, &body).is_err() {
                    eprintln!("error: `{p}` に書けません");
                    return ExitCode::from(2);
                }
                println!("描画しました: {p}");
            }
            None => print!("{body}"),
        }
    }
    ExitCode::from(0)
}

// ── M3 過去再生 ────────────────────────────────────────────────────────────

/// 規則を読んで検査を通す。`送料@v3` は git タグ `rules/送料/v3` の糖衣（§1.4）。
fn load_rule(spec: &str) -> Result<(String, rulec::ast::RuleFile, rulec::types::Checked), String> {
    let src = match spec.split_once('@') {
        Some((name, ver)) if !std::path::Path::new(spec).exists() => {
            let tag = format!("rules/{name}/{ver}");
            // `git show <tag>:<path>` で拾う。どのパスに在るかは tag の中を引く。
            // `-z` で NUL 区切りにする。既定の git は非 ASCII のパスを八進数で
            // クォートするので、和名のファイルが素の文字列一致では見つからない。
            let ls = std::process::Command::new("git")
                .args(["ls-tree", "-r", "-z", "--name-only", &tag])
                .output()
                .map_err(|e| format!("git を起動できません: {e}"))?;
            if !ls.status.success() {
                return Err(format!("git タグ `{tag}` が引けません"));
            }
            let listing = String::from_utf8_lossy(&ls.stdout).into_owned();
            let path = listing
                .split('\0')
                .find(|p| p.ends_with(&format!("{name}.rule")))
                .ok_or_else(|| format!("`{tag}` の中に {name}.rule がありません"))?;
            let o = std::process::Command::new("git")
                .arg("show")
                .arg(format!("{tag}:{path}"))
                .output()
                .map_err(|e| format!("git を起動できません: {e}"))?;
            if !o.status.success() {
                return Err(format!("`{tag}:{path}` が読めません"));
            }
            String::from_utf8_lossy(&o.stdout).into_owned()
        }
        _ => std::fs::read_to_string(spec).map_err(|_| format!("`{spec}` を読めません"))?,
    };
    let (f, c) = rulec::prepare(&src, spec).map_err(|_| format!("`{spec}` は検査を通っていません"))?;
    Ok((src, f, c))
}

/// `--manifest` と `--fill` から補完の既定値を組み立てる（§10.3）。
/// `--fill` は感度分析の一時上書きなので、マニフェストより後に効く。
fn build_manifest(
    args: &[String],
    f: &rulec::ast::RuleFile,
    c: &rulec::types::Checked,
) -> Result<rulec::fixtures::Manifest, String> {
    let mut m = match args.windows(2).find(|w| w[0] == "--manifest") {
        Some(w) => {
            let src = std::fs::read_to_string(&w[1]).map_err(|_| format!("`{}` を読めません", w[1]))?;
            rulec::fixtures::Manifest::load(&src, f, c)?
        }
        None => rulec::fixtures::Manifest::default(),
    };
    for w in args.windows(2).filter(|w| w[0] == "--fill") {
        m.add(&w[1], f, c)?;
    }
    Ok(m)
}

fn fixtures_arg(args: &[String]) -> Result<(String, String), String> {
    let w = args
        .windows(2)
        .find(|w| w[0] == "--fixtures")
        .ok_or_else(|| "--fixtures <file.jsonl> が要ります".to_string())?;
    let src = std::fs::read_to_string(&w[1]).map_err(|_| format!("`{}` を読めません", w[1]))?;
    Ok((w[1].clone(), src))
}

/// §10.2: 型と範囲の検証だけを引き受ける。ETL は利用者の仕事。
fn fixtures_lint(files: &[&String], args: &[String]) -> ExitCode {
    let (Some(jsonl), Some(rule)) = (files.first(), files.get(1)) else {
        eprintln!("error: `rulec fixtures lint <file.jsonl> <file.rule>`");
        return ExitCode::from(2);
    };
    let (_, f, c) = match load_rule(rule) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
    let m = match build_manifest(args, &f, &c) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
    let Ok(src) = std::fs::read_to_string(jsonl.as_str()) else {
        eprintln!("error: `{jsonl}` を読めません");
        return ExitCode::from(2);
    };
    let l = rulec::fixtures::load(&src, &f, &c, &m);
    print!("{}", rulec::fixtures::render_lint(&l, jsonl));
    ExitCode::from(u8::from(!l.problems.is_empty()))
}

/// §10.3: 規則を過去の記録に当て、そのとき出た値と突き合わせる。
fn replay_cmd(files: &[&String], args: &[String], md: bool) -> ExitCode {
    let Some(rule) = files.first() else { return usage() };
    let r = (|| -> Result<(String, rulec::ast::RuleFile, rulec::types::Checked, rulec::fixtures::Manifest, String, String), String> {
        let (_, f, c) = load_rule(rule)?;
        let m = build_manifest(args, &f, &c)?;
        let (path, src) = fixtures_arg(args)?;
        Ok((String::new(), f, c, m, path, src))
    })();
    let (_, f, c, m, path, src) = match r {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
    let l = rulec::fixtures::load(&src, &f, &c, &m);
    let rep = rulec::replay::replay(&f, &c, &l, &m, &path);
    if md {
        print!("{}", rulec::report::markdown(&rep, &f, &c, "過去再生"));
    } else {
        print!("{}", rulec::report::render(&rep, &f, &c));
    }
    ExitCode::from(u8::from(!rep.mismatches.is_empty()))
}

/// §10.4: 二つの版を同じ記録に当てて、何件・いくら動くかを出す。
fn diff_cmd(files: &[&String], args: &[String], md: bool) -> ExitCode {
    let (Some(a), Some(b)) = (files.first(), files.get(1)) else {
        eprintln!("error: `rulec diff <旧> <新> --fixtures <f.jsonl>`");
        return ExitCode::from(2);
    };
    let r = (|| -> Result<_, String> {
        let (_, of, oc) = load_rule(a)?;
        let (_, nf, nc) = load_rule(b)?;
        if of.name.text != nf.name.text {
            return Err(format!(
                "別の規則を比べようとしています（`{}` と `{}`）",
                of.name.text, nf.name.text
            ));
        }
        let m = build_manifest(args, &nf, &nc)?;
        let (path, src) = fixtures_arg(args)?;
        Ok((of, oc, nf, nc, m, path, src))
    })();
    let (of, oc, nf, nc, m, path, src) = match r {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
    let l = rulec::fixtures::load(&src, &nf, &nc, &m);
    let rep = rulec::replay::diff((&of, &oc), (&nf, &nc), &l, &m, (a, b));
    let _ = path;
    if md {
        print!("{}", rulec::report::markdown(&rep, &nf, &nc, "版の差分"));
    } else {
        print!("{}", rulec::report::render(&rep, &nf, &nc));
    }
    ExitCode::from(u8::from(!rep.mismatches.is_empty()))
}

/// §9.2: 作ったベクタが三つの被覆基準を満たしているかを判定する。
/// 生成器と独立に義務を数え、欠けたものを名指しして 1 で終わる。
fn coverage(files: &[&String]) -> ExitCode {
    if files.is_empty() {
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
        let (a, vs) = rulec::coverage::audit_file(&f, &c, path);
        println!("{path}");
        print!("{}", rulec::coverage::render(&a, &vs));
        if !a.ok() {
            worst = 1;
        }
    }
    ExitCode::from(worst)
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
                print!("{}", rulec::report::render(&rep, &f, &c));
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
