//! The `koyomi` command (`crate::run::run`): the binary of this crate runs it, and so does
//! `ritsu koyomi` (ritsu's DESIGN 8.2). What exists and what it takes is `cli.rs`'s table.

use crate::calendar::Loader;
use crate::check::{self, Checked, Options};
use crate::cli::{self, Args};
use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;
use std::process::ExitCode;

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

/// The `koyomi` command, on `args` (the words after the program's name), printing to standard
/// output and standard error: the exit code.
pub fn run(args: Vec<String>) -> ExitCode {
    let lang = Lang::pick(lang_flag(&args).as_deref(), "KOYOMI_LANG");
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
            println!("koyomi {}", env!("CARGO_PKG_VERSION"));
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
                    None => refuse(tr!("`{n}` というコマンドはありません。`koyomi --help` を読んでください", "there is no command `{n}`; run `koyomi --help`"), lang),
                },
            };
        }
        _ => {}
    }
    let Some(cmd) = table.command(first) else {
        return refuse(tr!("`{first}` というコマンドはありません。`koyomi --help` を読んでください", "there is no command `{first}`; run `koyomi --help`"), lang);
    };
    let a = match table.parse(cmd, &args[1..]) {
        Ok(a) => a,
        Err(e) => return refuse(e, lang),
    };
    if a.has("--help") {
        print!("{}", table.help_cmd(cmd, lang));
        return ExitCode::SUCCESS;
    }
    // a path that starts with `@`, in a module, is read as a path written in a file is (ritsu's DESIGN
    // 17.6); `diff` reads its two sides by their own spelling (17.9.1)
    if cmd.name != "diff"
        && let Err(t) = ritsu_base::modules::given_all(&a.pos)
    {
        return refuse(t, lang);
    }
    match cmd.name {
        "check" => check_cmd(&a, lang),
        "eval" => eval_cmd(&a, lang),
        "diff" => diff_cmd(&a, lang),
        "explain" => explain_cmd(&a, lang),
        "api" => api_cmd(&a, lang),
        "gen" => gen_cmd(&a, &args, lang),
        "vectors" => vectors_cmd(&a, lang),
        "doc" => doc_cmd(&a, lang),
        "source" => source_cmd(&a, lang),
        _ => unreachable!("every command in the table is dispatched"),
    }
}

/// The `.cal` files a path stands for: itself, or every `.cal` under a directory, in path order.
/// The walk does not enter another module, nor the `.ritsu` of a module; when it did not start in
/// a module, it says on stderr which modules it passed over, and the command that reads each
/// (ritsu's DESIGN 17.6).
fn expand(arg: &str, command: &str, lang: Lang) -> Vec<String> {
    let p = std::path::Path::new(arg);
    if !ritsu_base::fs::is_dir(p) {
        return vec![arg.to_string()];
    }
    let mut out = Vec::new();
    let mut modules = Vec::new();
    fn walk(d: &std::path::Path, out: &mut Vec<String>, modules: &mut Vec<String>) {
        let Ok(rd) = ritsu_base::fs::read_dir(d) else { return };
        let mut es: Vec<_> = rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
        es.sort();
        for e in es {
            if ritsu_base::fs::is_dir(&e) {
                match ritsu_base::paths::not_entered(&e) {
                    Some(ritsu_base::paths::NotEntered::Module) => modules.push(e.to_string_lossy().to_string()),
                    Some(ritsu_base::paths::NotEntered::State) => {}
                    None => walk(&e, out, modules),
                }
            } else if e.extension().is_some_and(|x| x == "cal") {
                out.push(e.to_string_lossy().to_string());
            }
        }
    }
    walk(p, &mut out, &mut modules);
    if ritsu_base::modules::refer::module_of(p).is_none() {
        for m in modules {
            eprintln!("{}", ritsu_base::paths::other_module_note(&m, &format!("koyomi {command}")).get(lang));
        }
    }
    out
}

fn check_cmd(a: &Args, lang: Lang) -> ExitCode {
    if a.pos.is_empty() {
        return refuse(tr!("`koyomi check` には .cal のファイルが要ります", "`koyomi check` needs .cal files"), lang);
    }
    let budget = match a.get("--budget") {
        None => check::DEFAULT_BUDGET,
        Some(b) => match b.replace([',', '_'], "").parse::<u64>() {
            Ok(n) if n > 0 => n,
            _ => return refuse(tr!("`--budget {b}` は正の整数ではありません", "`--budget {b}` is not a positive integer"), lang),
        },
    };
    let opts = Options { budget };
    let json = a.get("--format") == Some("json");
    let mut loader = Loader::default();
    let mut unreadable = false;
    let mut errors = false;
    // The files given before each, for what they make one in a target (W001, DESIGN 17).
    let mut earlier = Vec::new();
    for arg in &a.pos {
        for f in expand(arg, "check", lang) {
            match check::check_file(&f, &opts, &mut loader) {
                Ok(mut o) => {
                    check::compare_across(&mut o, &mut earlier);
                    errors |= o.has_errors();
                    if json {
                        println!("{}", check::to_json(&o, lang));
                    } else {
                        print!("{}", check::render(&o, lang));
                    }
                }
                Err(e) => {
                    unreadable = true;
                    let msg = tr!("`{f}` を読めません: {e}", "cannot read `{f}`: {e}");
                    eprintln!("{}: {}", if lang == Lang::Ja { "エラー" } else { "error" }, msg.get(lang));
                }
            }
        }
    }
    ExitCode::from(if unreadable {
        2
    } else if errors {
        1
    } else {
        0
    })
}

/// `koyomi diff <old> <new>` (ritsu's DESIGN 17.9.3).
fn diff_cmd(a: &Args, lang: Lang) -> ExitCode {
    let [old, new] = a.pos.as_slice() else {
        return refuse(tr!("`koyomi diff` には古い版と新しい版の二つを渡してください", "`koyomi diff` takes two sides, the old and the new"), lang);
    };
    let budget = match a.get("--budget") {
        None => check::DEFAULT_BUDGET,
        Some(b) => match b.replace([',', '_'], "").parse::<u64>() {
            Ok(n) if n > 0 => n,
            _ => return refuse(tr!("`--budget {b}` は正の整数ではありません", "`--budget {b}` is not a positive integer"), lang),
        },
    };
    let sides = crate::diff::spelled(old, budget).and_then(|o| crate::diff::spelled(new, budget).map(|n| (o, n)));
    let d = match sides.and_then(|(o, n)| crate::diff::diff(&o, &n, budget)) {
        Ok(d) => d,
        Err(t) => return refuse(t, lang),
    };
    match a.get("--format") {
        Some("json") => println!("{}", serde_json::to_string_pretty(&crate::diff::to_json(&d, lang)).unwrap()),
        Some("markdown") => print!("{}", crate::diff::markdown(&d, lang)),
        _ => print!("{}", crate::diff::text(&d, lang)),
    }
    ExitCode::from(u8::from(d.differs()))
}

fn print_diags(ds: &[crate::diag::Diag], lang: Lang) {
    for d in ds {
        print!("{}", d.render(lang));
    }
}

fn eval_cmd(a: &Args, lang: Lang) -> ExitCode {
    let Some(path) = a.pos.first() else {
        return refuse(tr!("`koyomi eval` には .cal のファイルが要ります", "`koyomi eval` needs a .cal file"), lang);
    };
    let json = a.get("--format") == Some("json");
    let Ok(text) = ritsu_base::fs::read_to_string(path) else {
        return refuse(tr!("`{path}` を読めません", "cannot read `{path}`"), lang);
    };
    let is_calendar = text.lines().map(|l| l.trim()).find(|l| !l.is_empty() && !l.starts_with('#')).is_some_and(|l| l.starts_with("calendar "));
    let mut loader = Loader::default();
    if is_calendar {
        let [day] = &a.pos[1..] else {
            return refuse(tr!("カレンダーには日付を一つ渡してください（`koyomi eval {path} 2026-05-04`）", "give a calendar one date (`koyomi eval {path} 2026-05-04`)"), lang);
        };
        let Some(d) = crate::date::parse(day) else {
            return refuse(tr!("`{day}` は日付（`2026-05-04` の形）ではありません", "`{day}` is not a date (`2026-05-04`)"), lang);
        };
        let o = match check::check_file(path, &Options::default(), &mut loader) {
            Ok(o) => o,
            Err(e) => return refuse(tr!("`{path}` を読めません: {e}", "cannot read `{path}`: {e}"), lang),
        };
        if o.has_errors() {
            print_diags(&o.diags, lang);
            return ExitCode::from(1);
        }
        let Some(Checked::Calendar(cal)) = &o.checked else { return ExitCode::from(1) };
        let (out, ok) = crate::eval::eval_calendar(cal, d, lang, json);
        print!("{out}");
        return ExitCode::from(if ok { 0 } else { 1 });
    }
    let (m, warnings) = match check::prepare(path, &mut loader) {
        Ok(x) => x,
        Err(Ok(ds)) => {
            print_diags(&ds, lang);
            return ExitCode::from(1);
        }
        Err(Err(e)) => return refuse(tr!("`{path}` を読めません: {e}", "cannot read `{path}`: {e}"), lang),
    };
    for w in &warnings {
        eprint!("{}", w.render(lang));
    }
    let vals = match crate::eval::parse_inputs(&m, &a.pos[1..]) {
        Ok(v) => v,
        Err(e) => return refuse(e, lang),
    };
    let (out, ok) = crate::eval::eval_dates(&m, &vals, lang, json);
    print!("{out}");
    ExitCode::from(if ok { 0 } else { 1 })
}

fn explain_cmd(a: &Args, lang: Lang) -> ExitCode {
    let md = a.get("--format") == Some("markdown");
    let ledger = crate::codes::ledger();
    if a.has("--all") {
        if !a.pos.is_empty() {
            return refuse(tr!("`--all` とコードは一緒に書けません", "`--all` takes no code"), lang);
        }
        if md {
            print!("{}", ledger.render_markdown(lang));
        } else {
            for (i, e) in ledger.entries.iter().enumerate() {
                if i > 0 {
                    println!();
                }
                print!("{}", ledger.render_text(e, lang));
            }
        }
        return ExitCode::SUCCESS;
    }
    let [code] = a.pos.as_slice() else {
        return refuse(tr!("`koyomi explain <CODE>` か `koyomi explain --all` です", "it is `koyomi explain <CODE>` or `koyomi explain --all`"), lang);
    };
    match ledger.find(code) {
        Some(e) => {
            if md {
                print!("{}", ledger.render_markdown_one(e, lang));
            } else {
                print!("{}", ledger.render_text(e, lang));
            }
            ExitCode::SUCCESS
        }
        None => refuse(tr!("診断のコード `{code}` はありません。`koyomi explain --all` で一覧が出ます", "there is no diagnostic code `{code}`; `koyomi explain --all` lists them"), lang),
    }
}

fn api_cmd(a: &Args, lang: Lang) -> ExitCode {
    let [path] = a.pos.as_slice() else {
        return refuse(tr!("`koyomi api` には .cal のファイルを一つ渡してください", "`koyomi api` takes one .cal file"), lang);
    };
    let mut loader = Loader::default();
    let o = match check::check_file(path, &Options::default(), &mut loader) {
        Ok(o) => o,
        Err(e) => return refuse(tr!("`{path}` を読めません: {e}", "cannot read `{path}`: {e}"), lang),
    };
    if o.has_errors() {
        print_diags(&o.diags, lang);
        return ExitCode::from(1);
    }
    let v = match &o.checked {
        Some(Checked::Calendar(c)) => crate::api::calendar_file_json(c),
        Some(Checked::Dates(m, _)) => crate::api::dates_json(m),
        None => return ExitCode::from(1),
    };
    println!("{}", serde_json::to_string_pretty(&v).unwrap());
    ExitCode::SUCCESS
}

/// Say that a file is refused: its diagnostics on standard output, as `check` prints them, and
/// one line on standard error.
fn refused(o: &check::Outcome, why: Text, lang: Lang) {
    print!("{}", check::render(o, lang));
    eprintln!("{}: {}", if lang == Lang::Ja { "エラー" } else { "error" }, why.get(lang));
}

/// What `gen` says when it is given no `--target` (DESIGN 16), from the words of its command line
/// after `koyomi`: the names it takes, and the command that writes what 0.40.0 wrote with none — the
/// same words, its `--lang` too (the code speaks it), with [`cli::UP_TO_0_40`] after them. An
/// `--extra` the line gives is in those already, and is left out.
fn no_target(typed: &[String]) -> Text {
    let mut words: Vec<String> = Vec::new();
    let mut i = 0;
    while i < typed.len() {
        if typed[i] == "--extra" {
            i += 2;
            continue;
        }
        if !typed[i].starts_with("--extra=") {
            words.push(ritsu_base::cli::shell_word(&typed[i]));
        }
        i += 1;
    }
    words.extend(cli::UP_TO_0_40.iter().map(|w| w.to_string()));
    let cmd = format!("koyomi {}", words.join(" "));
    let (t, e) = (cli::TARGET_NAMES.join("|"), cli::EXTRAS.join("|"));
    tr!(
        "`koyomi gen` が書くのは、指定した出力先だけです。`--target {t}` を一つ以上付けてください。ランナーは `--extra {e}` で指定します\n注記: 0.40.0 までの `koyomi gen` は、`--target` が無ければ、六つの出力先をランナーつきで書きました。同じファイルを書くのは次のコマンドです\n  {cmd}",
        "`koyomi gen` writes only the targets it is given: give one or more `--target {t}`, and `--extra {e}` for the runners\nnote: up to 0.40.0, `koyomi gen` with no `--target` wrote every target with its runner; this writes the same files:\n  {cmd}"
    )
}

/// Whether the file at `p` is one koyomi wrote: its first line is the head of every file koyomi
/// generates (`// Code generated by koyomi 0.42.0. DO NOT EDIT.`, in the comment of the file's
/// language), of whichever version.
fn generated_by_koyomi(p: &std::path::Path) -> bool {
    let Ok(text) = std::fs::read_to_string(p) else { return false };
    let first = text.lines().next().unwrap_or("");
    ["// ", "# ", "-- "].iter().filter_map(|c| first.strip_prefix(c)).any(|l| l.starts_with("Code generated by koyomi ") && l.ends_with("DO NOT EDIT."))
}

/// `koyomi gen`, on what the command line said and its words after `koyomi` (`typed`). It writes the
/// targets given and, with `--extra runner`, their runners (DESIGN 16).
fn gen_cmd(a: &Args, typed: &[String], lang: Lang) -> ExitCode {
    use crate::naming::{TARGETS, Target};
    if a.pos.is_empty() {
        return refuse(tr!("`koyomi gen` には .cal のファイルが要ります", "`koyomi gen` needs .cal files"), lang);
    }
    // It writes only the targets it is given: with none, nothing, and the command line that writes
    // what 0.40.0 wrote (DESIGN 16).
    let given = a.all("--target");
    if given.is_empty() {
        return refuse(no_target(typed), lang);
    }
    // in the order of TARGETS, not of the flags
    let targets: Vec<Target> = TARGETS.iter().copied().filter(|t| given.contains(&t.key())).collect();
    let with_runner = a.all("--extra").contains(&"runner");
    let out_dir = std::path::PathBuf::from(a.get("--out").unwrap_or("generated"));
    let check_only = a.has("--check");
    let mut loader = Loader::default();
    let mut failed = false;
    // Every file the run writes, and the .cal it comes from: two files with one alias would
    // write over each other.
    let mut planned: Vec<(String, String, String)> = Vec::new();
    // The runners of the targets written, when the run does not ask for them: one koyomi wrote there
    // is left from an earlier run (or from 0.40.0, which wrote every runner), and is said.
    let mut unasked: Vec<String> = Vec::new();
    // Whether a file's names break a target the run writes (E016, DESIGN 15): then it writes nothing.
    let mut stopped = false;
    // The files given before each, for what they make one in a target (W001, DESIGN 17).
    let mut earlier = Vec::new();
    for arg in &a.pos {
        for f in expand(arg, "gen", lang) {
            let mut o = match check::check_file(&f, &Options::default(), &mut loader) {
                Ok(o) => o,
                Err(e) => return refuse(tr!("`{f}` を読めません: {e}", "cannot read `{f}`: {e}"), lang),
            };
            check::compare_across(&mut o, &mut earlier);
            let Some(checked) = o.checked.as_ref().filter(|_| !o.has_errors()) else {
                refused(&o, tr!("`{f}` は検査を通らないので、生成しません", "`{f}` does not pass check, so nothing is generated from it"), lang);
                failed = true;
                continue;
            };
            let u = crate::codegen::unit_of(checked, lang);
            // what stops the targets, each finding once (one names Rust and Wasm alike)
            let mut stops: Vec<crate::diag::Diag> = Vec::new();
            let mut broken: Vec<&str> = Vec::new();
            for t in &targets {
                let files = match crate::codegen::build(&o, &u, *t, with_runner) {
                    Ok(files) => files,
                    Err(ds) => {
                        broken.push(t.key());
                        for d in ds {
                            if !stops.iter().any(|s| s.place() == d.place() && s.message.en == d.message.en) {
                                stops.push(d);
                            }
                        }
                        continue;
                    }
                };
                if !with_runner {
                    unasked.push(crate::codegen::runner_file(*t, &u.alias));
                }
                for (rel, body) in files {
                    if let Some((_, _, from)) = planned.iter().find(|(p, _, _)| *p == rel) {
                        return refuse(
                            tr!(
                                "`{f}` と `{from}` が同じ {rel} を書き出します。どちらかの別名を変えてください",
                                "`{f}` and `{from}` both write {rel}; give one of them another alias"
                            ),
                            lang,
                        );
                    }
                    planned.push((rel, body, f.clone()));
                }
            }
            if !stops.is_empty() {
                print!("{}", stops.iter().map(|d| d.render(lang)).collect::<String>());
                let named: Vec<String> = broken.iter().map(|k| format!("`{k}`")).collect();
                let (list_ja, list_en) = (named.join("、"), named.join(", "));
                let why = tr!("`{f}` からは {list_ja} のコードを生成できないので、何も生成しません", "`{f}` cannot be made into the code of {list_en}, so nothing is generated");
                eprintln!("{}: {}", if lang == Lang::Ja { "エラー" } else { "error" }, why.get(lang));
                stopped = true;
            }
        }
    }
    if stopped {
        return ExitCode::from(1);
    }
    let mut dirty = false;
    for (rel, body, _) in &planned {
        let p = out_dir.join(rel);
        let shown = p.to_string_lossy().to_string();
        let existing = std::fs::read(&p).ok();
        if existing.as_deref() == Some(body.as_bytes()) {
            continue;
        }
        if check_only {
            dirty = true;
            let msg = match existing {
                None => tr!("ありません: {shown}", "missing: {shown}"),
                Some(_) => tr!("生成し直すと変わります: {shown}", "differs from what gen writes: {shown}"),
            };
            println!("{}", msg.get(lang));
            continue;
        }
        if let Some(dir) = p.parent()
            && std::fs::create_dir_all(dir).is_err()
        {
            return refuse(tr!("`{}` を作れません", "cannot create `{}`", dir.display()), lang);
        }
        if std::fs::write(&p, body).is_err() {
            return refuse(tr!("`{shown}` に書けません", "cannot write `{shown}`"), lang);
        }
        println!("{}", tr!("生成しました: {shown}", "generated: {shown}").get(lang));
    }
    // The runners not asked for that koyomi wrote, still where the run would have written them: said,
    // with or without `--check`, and kept; the exit code stays what it is (DESIGN 16). A Go runner is
    // a test file of the module's package, which `go test` builds as it stands.
    let mut left = false;
    for rel in &unasked {
        let p = out_dir.join(rel);
        if planned.iter().any(|(q, _, _)| q == rel) || !generated_by_koyomi(&p) {
            continue;
        }
        let shown = p.to_string_lossy().to_string();
        println!("{}", tr!("もう生成しないファイルが残っています: {shown}", "generated no more, and still there: {shown}").get(lang));
        left = true;
    }
    if left {
        let note = tr!(
            "注記: これらは `--extra runner` が書くファイルで、この実行は頼んでいません。gen は出力先のファイルを消さないので、要らなければ消してかまいません",
            "note: `--extra runner` writes these files, and this run does not ask for it; gen deletes nothing in the output directory, so they can be deleted if they are not wanted"
        );
        println!("{}", note.get(lang));
    }
    ExitCode::from(if failed || dirty { 1 } else { 0 })
}

fn vectors_cmd(a: &Args, lang: Lang) -> ExitCode {
    use std::io::Write;
    let [path] = a.pos.as_slice() else {
        return refuse(tr!("`koyomi vectors` には .cal のファイルを一つ渡してください", "`koyomi vectors` takes one .cal file"), lang);
    };
    let mut loader = Loader::default();
    let o = match check::check_file(path, &Options::default(), &mut loader) {
        Ok(o) => o,
        Err(e) => return refuse(tr!("`{path}` を読めません: {e}", "cannot read `{path}`: {e}"), lang),
    };
    let Some(checked) = o.checked.as_ref().filter(|_| !o.has_errors()) else {
        refused(&o, tr!("`{path}` は検査を通らないので、ベクタを出しません", "`{path}` does not pass check, so it has no vectors"), lang);
        return ExitCode::from(1);
    };
    let stdout = std::io::stdout();
    let mut w = std::io::BufWriter::new(stdout.lock());
    match checked {
        Checked::Dates(m, _) => {
            let lines = crate::vectors::DatesLines::new(m);
            for r in crate::vectors::DatesRows::new(m) {
                let _ = writeln!(w, "{}", lines.line(&r));
            }
        }
        Checked::Calendar(c) => {
            for r in crate::vectors::calendar_rows(c) {
                let _ = writeln!(w, "{}", crate::vectors::calendar_line(&r));
            }
        }
    }
    let _ = w.flush();
    ExitCode::SUCCESS
}

fn doc_cmd(a: &Args, lang: Lang) -> ExitCode {
    let [path] = a.pos.as_slice() else {
        return refuse(tr!("`koyomi doc` には .cal のファイルを一つ渡してください", "`koyomi doc` takes one .cal file"), lang);
    };
    let months = match a.get("--months") {
        None => None,
        Some(m) => match crate::doc::months::parse_span(m) {
            Some(x) => Some(x),
            None => {
                return refuse(
                    tr!(
                        "`--months {m}` は `2026-01..2027-12` の形ではありません（前の月から後の月へ）",
                        "`--months {m}` is not of the form `2026-01..2027-12` (an earlier month, then a later one)"
                    ),
                    lang,
                );
            }
        },
    };
    let format = if a.get("--format") == Some("html") { crate::doc::Format::Html } else { crate::doc::Format::Markdown };
    let mut loader = Loader::default();
    let o = match check::check_file(path, &Options::default(), &mut loader) {
        Ok(o) => o,
        Err(e) => return refuse(tr!("`{path}` を読めません: {e}", "cannot read `{path}`: {e}"), lang),
    };
    match crate::doc::page(&o, lang, &crate::doc::Options { months }) {
        Ok(p) => {
            print!("{}", crate::doc::render(&p, format));
            ExitCode::SUCCESS
        }
        Err(ds) => {
            for d in &ds {
                eprint!("{}", d.render(lang));
            }
            let msg = tr!(
                "`{path}` にはページを作れないエラーがあるので、ページを出しません",
                "`{path}` has errors that keep its page from being made"
            );
            eprintln!("{}: {}", if lang == Lang::Ja { "エラー" } else { "error" }, msg.get(lang));
            ExitCode::from(1)
        }
    }
}

fn source_cmd(a: &Args, lang: Lang) -> ExitCode {
    let [verb, path] = a.pos.as_slice() else {
        return refuse(tr!("`koyomi source fetch|pin|outdated <file.cal>` です", "it is `koyomi source fetch|pin|outdated <file.cal>`"), lang);
    };
    if !matches!(verb.as_str(), "fetch" | "pin" | "outdated") {
        return refuse(tr!("`{verb}` という source のコマンドはありません。fetch、pin、outdated のどれかです", "`{verb}` is not one of source's: fetch, pin, outdated"), lang);
    }
    let Ok(src) = ritsu_base::fs::read_to_string(path) else {
        return refuse(tr!("`{path}` を読めません", "cannot read `{path}`"), lang);
    };
    let parsed = crate::parse::parse(path, &src);
    let Some(f) = parsed.file.filter(|_| !parsed.diags.iter().any(|d| d.is_error())) else {
        print_diags(&parsed.diags, lang);
        return ExitCode::from(1);
    };
    let dir = crate::fetch::dir_of(path);
    let result = match verb.as_str() {
        "fetch" => crate::fetch::fetch(&f, &dir),
        "outdated" => crate::fetch::outdated(&f, &dir),
        _ => crate::fetch::pin(&f, &dir, &src).and_then(|(text, o)| {
            if text != src {
                ritsu_base::fs::write(path, &text).map_err(|e| tr!("`{path}` に書けません: {e}", "cannot write `{path}`: {e}"))?;
            }
            Ok(o)
        }),
    };
    match result {
        Ok(o) => {
            if o.lines.is_empty() {
                println!("{}", tr!("{path}: 出典の宣言がありません", "{path}: no source is declared").get(lang));
            }
            for l in &o.lines {
                println!("{}", l.get(lang));
            }
            ExitCode::from(if verb == "outdated" && o.changed { 1 } else { 0 })
        }
        Err(e) => refuse(e, lang),
    }
}
