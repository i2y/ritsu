//! The `rulec` CLI. M0 is `check`.

use rulec::diag::{Severity, render, render_json};
use rulec::tr;
use std::process::ExitCode;

fn usage() -> ExitCode {
    eprintln!(
        "{}",
        tr!(
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
             どのコマンドにも --lang ja|en を付けられます（既定は en、環境変数 RULEC_LANG でも指定できます）\n\n\
             exit code: 0 注記のみ / 1 エラーあり / 2 内部異常",
            "rulec {}\n\n\
             Usage:\n  \
             rulec check <file.rule>...  [--format json] [--show-shadow] [--diff-base <rev>] [--budget N]\n  \
             rulec fmt   <file.rule>...  [--check]\n  \
             rulec gen   <file.rule>...  [--out DIR] [--check]\n  \
             rulec vectors <file.rule>... [--out DIR]\n  \
             rulec coverage <file.rule>...\n  \
             rulec doc   <file.rule>...  [--out DIR]\n  \
             rulec test  <output directory>\n  \
\
             rulec fixtures lint <file.jsonl> <file.rule> [--manifest m.json] [--fill field=value]\n  \
\
             rulec replay <file.rule> --fixtures <f.jsonl> [--manifest m.json] [--fill field=value] [--format markdown]\n  \
\
             rulec diff <old> <new> --fixtures <f.jsonl> [same options]   # old/new: file.rule or 送料@v3\n  \
             rulec schema  <file.rule>\n  \
             rulec adapter <file.rule> --template python|go\n  \
             rulec verify  <file.rule> --adapter <cmd> [args...]\n\n\
             Every command accepts --lang ja|en (default en; the RULEC_LANG environment variable works too)\n\n\
             exit code: 0 notes only / 1 errors found / 2 internal failure",
            env!("CARGO_PKG_VERSION")
        )
    );
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    // `--lang ja|en` (or `--lang=en`) decides the output language before anything
    // is printed; `RULEC_LANG` is the fallback, English the default (i18n.rs).
    if let Some(i) = args.iter().position(|a| a == "--lang") {
        let Some(v) = args.get(i + 1).and_then(|v| rulec::i18n::Lang::parse(v)) else {
            eprintln!("error: --lang ja|en");
            return ExitCode::from(2);
        };
        rulec::i18n::set(v);
        args.drain(i..i + 2);
    } else if let Some(i) = args.iter().position(|a| a.starts_with("--lang=")) {
        let Some(v) = rulec::i18n::Lang::parse(&args[i]["--lang=".len()..]) else {
            eprintln!("error: --lang ja|en");
            return ExitCode::from(2);
        };
        rulec::i18n::set(v);
        args.remove(i);
    }
    if args.is_empty() {
        return usage();
    }
    let json = args.iter().any(|a| a == "--format=json")
        || args.windows(2).any(|w| w[0] == "--format" && w[1] == "json");
    // §12: the form that gets pasted into a PR. The tool owns the formatting; posting it is
    // one line of CI.
    let markdown = args.iter().any(|a| a == "--format=markdown")
        || args.windows(2).any(|w| w[0] == "--format" && w[1] == "markdown");
    // §4: partial shadowing is how a top-down (`first`) table normally looks, so by default
    // only the count is printed.
    let show_shadow = args.iter().any(|a| a == "--show-shadow");
    // §4: shadow pairs that need review and W114 surface only what is new since the base
    // revision.
    let budget: i64 = args
        .windows(2)
        .find(|w| w[0] == "--budget")
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(rulec::region::DEFAULT_BUDGET);
    let diff_base = args
        .windows(2)
        .find(|w| w[0] == "--diff-base")
        .map(|w| w[1].clone());
    // Pick up the file names, skipping the V of `--x V` and values that would otherwise look
    // like arguments.
    let mut skip: Vec<String> = vec!["json".into(), "markdown".into(), budget.to_string()];
    // --fill can be repeated, so skip every one of its values.
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
    // The CI of §12 writes `rulec check rules/`. A directory expands to the `.rule` files
    // inside it, in a deterministic order (by path) so that the order of the report does not
    // change with the machine. Only `test` takes the output directory itself as its
    // argument, so it is not expanded.
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
                eprintln!("{}", tr!("error: --adapter <cmd> が要ります", "error: --adapter <cmd> is required"));
                return ExitCode::from(2);
            };
            let cmd: Vec<String> = args[i + 1..].to_vec();
            // Everything after --adapter is the command, so exclude it from the file-name
            // candidates.
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
                eprintln!("{}", tr!("error: いまあるのは `rulec fixtures lint` だけです", "error: only `rulec fixtures lint` exists for now"));
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

/// §1.5: the one and only formatter. `--check` is for CI: it lists the files that need
/// fixing and exits with 1.
fn fmt(files: &[&String], check_only: bool) -> ExitCode {
    if files.is_empty() {
        return usage();
    }
    let mut dirty = 0u8;
    for path in files {
        let Ok(src) = std::fs::read_to_string(path) else {
            eprintln!("{}", tr!("error: `{path}` を読めません", "error: cannot read `{path}`"));
            return ExitCode::from(2);
        };
        let out = rulec::fmt::format(&src);
        if out == src {
            continue;
        }
        if check_only {
            println!("{}", tr!("整形されていません: {path}", "not formatted: {path}"));
            dirty = 1;
        } else if std::fs::write(path, &out).is_err() {
            eprintln!("{}", tr!("error: `{path}` に書けません", "error: cannot write `{path}`"));
            return ExitCode::from(2);
        } else {
            println!("{}", tr!("整形しました: {path}", "formatted: {path}"));
        }
    }
    ExitCode::from(dirty)
}

/// Fetch the same file at the base revision. None when it is absent (the file is new).
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
            eprintln!("{}", tr!("error: `{path}` を読めません", "error: cannot read `{path}`"));
            return ExitCode::from(2);
        };
        let lines: Vec<String> = src.lines().map(|s| s.to_string()).collect();
        let r = rulec::report_with(&src, path, budget);
        let mut diags = r.diags;

        // Pairs are matched by the normal form of their cells. With line numbers, inserting a
        // single row would make everything new.
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
            println!(
                "{}",
                tr!(
                    "note {path}: 基準リビジョンに既にあった発見 {suppressed} 件は伏せました（--diff-base）",
                    "note {path}: suppressed {suppressed} findings already present at the base revision (--diff-base)"
                )
            );
        }
        // §4: only the pairs that need review are listed; the rest is a single count line.
        let s = r.shadow;
        if s.total() > 0 {
            println!(
                "{}",
                tr!(
                    "note {path}: 遮蔽 {} 対（構造的 {}、同値 {}、要確認 {}）",
                    "note {path}: {} shadow pairs ({} structural, {} equivalent, {} needs review)",
                    s.total(),
                    s.structural,
                    s.equivalent,
                    s.confirm
                )
            );
        }
        if !rulec::has_error(&diags) {
            println!("ok {path}");
        }
    }
    ExitCode::from(worst)
}

/// §8.4: the generated files are committed to git, and `--check` in CI verifies that they
/// match a fresh generation.
fn generate(files: &[&String], out_dir: &str, check_only: bool) -> ExitCode {
    if files.is_empty() {
        return usage();
    }
    let mut dirty = 0u8;
    for path in files {
        let Ok(src) = std::fs::read_to_string(path) else {
            eprintln!("{}", tr!("error: `{path}` を読めません", "error: cannot read `{path}`"));
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
                eprintln!("{}", tr!("error: `{path}` は検査を通っていないので生成しません", "error: `{path}` does not pass check, so nothing is generated"));
                return ExitCode::from(1);
            }
        };
        let g = rulec::codegen::Gen::new(&f, &c, &src);
        let alias = f.name.ascii.clone().unwrap_or_else(|| f.name.text.clone());
        let pkg = alias.replace('_', "").to_lowercase();
        // Also emit the vectors and the expected values. Of the three uses in §9.3, the
        // cross-language agreement test and the golden files run on these.
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
            // Unit vectors for the rounding helpers (§8.5). They catch errors that table
            // agreement alone would hide.
            (format!("{out_dir}/python/_round_test.py"), rulec::codegen::round_tests_python()),
            (format!("{out_dir}/go/{pkg}/round_test.go"), rulec::codegen::round_tests_go(&pkg)),
        ];
        for (p, body) in targets {
            let existing = std::fs::read_to_string(&p).ok();
            if existing.as_deref() == Some(body.as_str()) {
                continue;
            }
            if check_only {
                println!("{}", tr!("生成物が古いか手で編集されています: {p}", "generated file is stale or hand-edited: {p}"));
                dirty = 1;
                continue;
            }
            if let Some(dir) = std::path::Path::new(&p).parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if std::fs::write(&p, &body).is_err() {
                eprintln!("{}", tr!("error: `{p}` に書けません", "error: cannot write `{p}`"));
                return ExitCode::from(2);
            }
            println!("{}", tr!("生成しました: {p}", "generated: {p}"));
        }
    }
    ExitCode::from(dirty)
}

/// If the argument is a directory, collect the `.rule` files under it; a file is returned as
/// is. A directory that does not exist does not become an empty list: its name is returned
/// unchanged so that "cannot read" stops the run (never a silent success with 0 files).
fn expand(arg: &str) -> Vec<String> {
    let p = std::path::Path::new(arg);
    if !p.is_dir() {
        return vec![arg.to_string()];
    }
    let mut out = Vec::new();
    collect_rules(p, &mut out);
    out.sort();
    if out.is_empty() {
        eprintln!("{}", tr!("注意: `{arg}` の下に .rule がありません", "warning: no .rule files under `{arg}`"));
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

/// §1.6: render a rule that passed check as markdown. Read-only; there is no reverse
/// direction. **Not treated as a generated file** — it is never committed; CI renders it and
/// pastes it into the PR. The biggest danger is a stale rendering that lingers looking
/// authoritative, so no long-lived artifact is produced.
fn doc(files: &[&String], out_dir: Option<&str>) -> ExitCode {
    if files.is_empty() {
        return usage();
    }
    for path in files {
        let Ok(src) = std::fs::read_to_string(path) else {
            eprintln!("{}", tr!("error: `{path}` を読めません", "error: cannot read `{path}`"));
            return ExitCode::from(2);
        };
        // A pretty rendering of a broken rule is a lie. Nothing is rendered unless check
        // passes (§1.6).
        let rep = rulec::report(&src, path);
        if rulec::has_error(&rep.diags) {
            let lines: Vec<String> = src.lines().map(|s| s.to_string()).collect();
            for d in rep.diags.iter().filter(|d| d.severity == rulec::diag::Severity::Error) {
                print!("{}", render(d, &lines));
                println!();
            }
            eprintln!("{}", tr!("error: `{path}` は検査を通っていないので描画しません（§1.6）", "error: `{path}` does not pass check, so it is not rendered (§1.6)"));
            return ExitCode::from(1);
        }
        let Ok((f, c)) = rulec::prepare(&src, path) else {
            eprintln!("{}", tr!("error: `{path}` は検査を通っていません", "error: `{path}` does not pass check"));
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
                    eprintln!("{}", tr!("error: `{p}` に書けません", "error: cannot write `{p}`"));
                    return ExitCode::from(2);
                }
                println!("{}", tr!("描画しました: {p}", "rendered: {p}"));
            }
            None => print!("{body}"),
        }
    }
    ExitCode::from(0)
}

// ── M3 replay ────────────────────────────────────────────────────────────

/// Load a rule and run it through check. `送料@v3` is sugar for the git tag `rules/送料/v3`
/// (§1.4).
fn load_rule(spec: &str) -> Result<(String, rulec::ast::RuleFile, rulec::types::Checked), String> {
    let src = match spec.split_once('@') {
        Some((name, ver)) if !std::path::Path::new(spec).exists() => {
            let tag = format!("rules/{name}/{ver}");
            // Fetch it with `git show <tag>:<path>`; which path it lives at is looked up inside
            // the tag. `-z` makes the listing NUL-separated: by default git quotes non-ASCII
            // paths in octal, so a file with a Japanese name would not be found by plain
            // string comparison.
            let ls = std::process::Command::new("git")
                .args(["ls-tree", "-r", "-z", "--name-only", &tag])
                .output()
                .map_err(|e| tr!("git を起動できません: {e}", "cannot run git: {e}"))?;
            if !ls.status.success() {
                return Err(tr!("git タグ `{tag}` が引けません", "cannot resolve git tag `{tag}`"));
            }
            let listing = String::from_utf8_lossy(&ls.stdout).into_owned();
            let path = listing
                .split('\0')
                .find(|p| p.ends_with(&format!("{name}.rule")))
                .ok_or_else(|| tr!("`{tag}` の中に {name}.rule がありません", "no {name}.rule in `{tag}`"))?;
            let o = std::process::Command::new("git")
                .arg("show")
                .arg(format!("{tag}:{path}"))
                .output()
                .map_err(|e| tr!("git を起動できません: {e}", "cannot run git: {e}"))?;
            if !o.status.success() {
                return Err(tr!("`{tag}:{path}` が読めません", "cannot read `{tag}:{path}`"));
            }
            String::from_utf8_lossy(&o.stdout).into_owned()
        }
        _ => std::fs::read_to_string(spec).map_err(|_| tr!("`{spec}` を読めません", "cannot read `{spec}`"))?,
    };
    let (f, c) = rulec::prepare(&src, spec).map_err(|_| tr!("`{spec}` は検査を通っていません", "`{spec}` does not pass check"))?;
    Ok((src, f, c))
}

/// Assemble the default values used for filling from `--manifest` and `--fill` (§10.3).
/// `--fill` is a temporary override for sensitivity analysis, so it applies after the
/// manifest.
fn build_manifest(
    args: &[String],
    f: &rulec::ast::RuleFile,
    c: &rulec::types::Checked,
) -> Result<rulec::fixtures::Manifest, String> {
    let mut m = match args.windows(2).find(|w| w[0] == "--manifest") {
        Some(w) => {
            let src = std::fs::read_to_string(&w[1]).map_err(|_| tr!("`{}` を読めません", "cannot read `{}`", w[1]))?;
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
        .ok_or_else(|| tr!("--fixtures <file.jsonl> が要ります", "--fixtures <file.jsonl> is required"))?;
    let src = std::fs::read_to_string(&w[1]).map_err(|_| tr!("`{}` を読めません", "cannot read `{}`", w[1]))?;
    Ok((w[1].clone(), src))
}

/// §10.2: only the validation of types and ranges is done here. ETL is the user's job.
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
        eprintln!("{}", tr!("error: `{jsonl}` を読めません", "error: cannot read `{jsonl}`"));
        return ExitCode::from(2);
    };
    let l = rulec::fixtures::load(&src, &f, &c, &m);
    print!("{}", rulec::fixtures::render_lint(&l, jsonl));
    ExitCode::from(u8::from(!l.problems.is_empty()))
}

/// §10.3: apply the rule to past records and compare against the values produced at the time.
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
        print!("{}", rulec::report::markdown(&rep, &f, &c, &tr!("過去再生", "Replay")));
    } else {
        print!("{}", rulec::report::render(&rep, &f, &c));
    }
    ExitCode::from(u8::from(!rep.mismatches.is_empty()))
}

/// §10.4: apply two versions to the same records and report how many change and by how much.
fn diff_cmd(files: &[&String], args: &[String], md: bool) -> ExitCode {
    let (Some(a), Some(b)) = (files.first(), files.get(1)) else {
        eprintln!("{}", tr!("error: `rulec diff <旧> <新> --fixtures <f.jsonl>`", "error: `rulec diff <old> <new> --fixtures <f.jsonl>`"));
        return ExitCode::from(2);
    };
    let r = (|| -> Result<_, String> {
        let (_, of, oc) = load_rule(a)?;
        let (_, nf, nc) = load_rule(b)?;
        if of.name.text != nf.name.text {
            return Err(tr!(
                "別の規則を比べようとしています（`{}` と `{}`）",
                "comparing different rules (`{}` and `{}`)",
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
        print!("{}", rulec::report::markdown(&rep, &nf, &nc, &tr!("版の差分", "Version diff")));
    } else {
        print!("{}", rulec::report::render(&rep, &nf, &nc));
    }
    ExitCode::from(u8::from(!rep.mismatches.is_empty()))
}

/// §9.2: decide whether the generated vectors meet the three coverage criteria. The
/// obligations are counted independently of the generator; the missing ones are named and
/// the exit code is 1.
fn coverage(files: &[&String]) -> ExitCode {
    if files.is_empty() {
        return usage();
    }
    let mut worst = 0u8;
    for path in files {
        let Ok(src) = std::fs::read_to_string(path) else {
            eprintln!("{}", tr!("error: `{path}` を読めません", "error: cannot read `{path}`"));
            return ExitCode::from(2);
        };
        let Ok((f, c)) = rulec::prepare(&src, path) else {
            eprintln!("{}", tr!("error: `{path}` は検査を通っていません", "error: `{path}` does not pass check"));
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

/// §9: build the vectors from the boundaries. The reference evaluator attaches the expected
/// values and the fired rows.
fn vectors(files: &[&String], out_dir: Option<&str>) -> ExitCode {
    if files.is_empty() {
        return usage();
    }
    for path in files {
        let Ok(src) = std::fs::read_to_string(path) else {
            eprintln!("{}", tr!("error: `{path}` を読めません", "error: cannot read `{path}`"));
            return ExitCode::from(2);
        };
        let Ok((f, c)) = rulec::prepare(&src, path) else {
            eprintln!("{}", tr!("error: `{path}` は検査を通っていないのでベクタを作りません", "error: `{path}` does not pass check, so no vectors are generated"));
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
                    eprintln!("{}", tr!("error: `{p}` に書けません", "error: cannot write `{p}`"));
                    return ExitCode::from(2);
                }
                println!("{}", tr!("ベクタ {} 件: {p}", "{} vectors: {p}", vs.len()));
            }
            None => print!("{body}"),
        }
    }
    ExitCode::from(0)
}

/// A subcommand that only emits a string for a single rule.
fn one(
    files: &[&String],
    f: impl Fn(&rulec::ast::RuleFile, &rulec::types::Checked, &str) -> Option<String>,
) -> ExitCode {
    if files.is_empty() {
        return usage();
    }
    for path in files {
        let Ok(src) = std::fs::read_to_string(path) else {
            eprintln!("{}", tr!("error: `{path}` を読めません", "error: cannot read `{path}`"));
            return ExitCode::from(2);
        };
        let Ok((rf, c)) = rulec::prepare(&src, path) else {
            eprintln!("{}", tr!("error: `{path}` は検査を通っていません", "error: `{path}` does not pass check"));
            return ExitCode::from(1);
        };
        if let Some(s) = f(&rf, &c, path) {
            print!("{s}");
        }
    }
    ExitCode::from(0)
}

/// §10: feed the vectors through the adapter of the legacy implementation and compare.
fn verify(files: &[&String], adapter: &[String]) -> ExitCode {
    if files.is_empty() || adapter.is_empty() {
        return usage();
    }
    let mut worst = 0u8;
    for path in files {
        let Ok(src) = std::fs::read_to_string(path) else {
            eprintln!("{}", tr!("error: `{path}` を読めません", "error: cannot read `{path}`"));
            return ExitCode::from(2);
        };
        let Ok((f, c)) = rulec::prepare(&src, path) else {
            eprintln!("{}", tr!("error: `{path}` は検査を通っていません", "error: `{path}` does not pass check"));
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
