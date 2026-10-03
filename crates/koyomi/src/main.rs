//! The `koyomi` command. What exists and what it takes is `cli.rs`'s table.

use koyomi::calendar::Loader;
use koyomi::check::{self, Checked, Options};
use koyomi::cli::{self, Args};
use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;
use std::process::ExitCode;

/// Die quietly when the reader of a pipe goes away, as `cat` does.
#[cfg(unix)]
fn restore_sigpipe() {
    unsafe extern "C" {
        fn signal(signum: i32, handler: usize) -> usize;
    }
    // SIGPIPE is 13 on Linux and macOS; SIG_DFL is 0.
    unsafe { signal(13, 0) };
}

#[cfg(not(unix))]
fn restore_sigpipe() {}

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
    restore_sigpipe();
    // A panic is a bug in koyomi: exit 2, as DESIGN 4.1 says, rather than Rust's 101.
    std::panic::set_hook(Box::new(|info| {
        eprintln!("koyomi: a bug in koyomi, please report it: {info}");
    }));
    match std::panic::catch_unwind(run) {
        Ok(code) => code,
        Err(_) => ExitCode::from(2),
    }
}

fn run() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
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
    match cmd.name {
        "check" => check_cmd(&a, lang),
        "eval" => eval_cmd(&a, lang),
        "explain" => explain_cmd(&a, lang),
        "api" => api_cmd(&a, lang),
        "gen" => gen_cmd(&a, lang),
        "vectors" => vectors_cmd(&a, lang),
        "doc" => doc_cmd(&a, lang),
        "source" => source_cmd(&a, lang),
        _ => unreachable!("every command in the table is dispatched"),
    }
}

/// The `.cal` files a path stands for: itself, or every `.cal` under a directory, in path order.
fn expand(arg: &str) -> Vec<String> {
    let p = std::path::Path::new(arg);
    if !p.is_dir() {
        return vec![arg.to_string()];
    }
    let mut out = Vec::new();
    fn walk(d: &std::path::Path, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        let mut es: Vec<_> = rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
        es.sort();
        for e in es {
            if e.is_dir() {
                walk(&e, out);
            } else if e.extension().is_some_and(|x| x == "cal") {
                out.push(e.to_string_lossy().to_string());
            }
        }
    }
    walk(p, &mut out);
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
    for arg in &a.pos {
        for f in expand(arg) {
            match check::check_file(&f, &opts, &mut loader) {
                Ok(o) => {
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

fn print_diags(ds: &[koyomi::diag::Diag], lang: Lang) {
    for d in ds {
        print!("{}", d.render(lang));
    }
}

fn eval_cmd(a: &Args, lang: Lang) -> ExitCode {
    let Some(path) = a.pos.first() else {
        return refuse(tr!("`koyomi eval` には .cal のファイルが要ります", "`koyomi eval` needs a .cal file"), lang);
    };
    let json = a.get("--format") == Some("json");
    let Ok(text) = std::fs::read_to_string(path) else {
        return refuse(tr!("`{path}` を読めません", "cannot read `{path}`"), lang);
    };
    let is_calendar = text.lines().map(|l| l.trim()).find(|l| !l.is_empty() && !l.starts_with('#')).is_some_and(|l| l.starts_with("calendar "));
    let mut loader = Loader::default();
    if is_calendar {
        let [day] = &a.pos[1..] else {
            return refuse(tr!("カレンダーには日付を一つ渡します（`koyomi eval {path} 2026-05-04`）", "give a calendar one date (`koyomi eval {path} 2026-05-04`)"), lang);
        };
        let Some(d) = koyomi::date::parse(day) else {
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
        let (out, ok) = koyomi::eval::eval_calendar(cal, d, lang, json);
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
    let vals = match koyomi::eval::parse_inputs(&m, &a.pos[1..]) {
        Ok(v) => v,
        Err(e) => return refuse(e, lang),
    };
    let (out, ok) = koyomi::eval::eval_dates(&m, &vals, lang, json);
    print!("{out}");
    ExitCode::from(if ok { 0 } else { 1 })
}

fn explain_cmd(a: &Args, lang: Lang) -> ExitCode {
    let md = a.get("--format") == Some("markdown");
    let ledger = koyomi::codes::ledger();
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
        return refuse(tr!("`koyomi api` には .cal のファイルを一つ渡します", "`koyomi api` takes one .cal file"), lang);
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
        Some(Checked::Calendar(c)) => koyomi::api::calendar_file_json(c),
        Some(Checked::Dates(m, _)) => koyomi::api::dates_json(m),
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

fn gen_cmd(a: &Args, lang: Lang) -> ExitCode {
    use koyomi::naming::{TARGETS, Target};
    if a.pos.is_empty() {
        return refuse(tr!("`koyomi gen` には .cal のファイルが要ります", "`koyomi gen` needs .cal files"), lang);
    }
    let targets: Vec<Target> = match a.get("--target") {
        Some(t) => TARGETS.iter().copied().filter(|x| x.key() == t).collect(),
        None => TARGETS.to_vec(),
    };
    let out_dir = std::path::PathBuf::from(a.get("--out").unwrap_or("generated"));
    let check_only = a.has("--check");
    let mut loader = Loader::default();
    let mut failed = false;
    // Every file the run writes, and the .cal it comes from: two files with one alias would
    // write over each other.
    let mut planned: Vec<(String, String, String)> = Vec::new();
    for arg in &a.pos {
        for f in expand(arg) {
            let o = match check::check_file(&f, &Options::default(), &mut loader) {
                Ok(o) => o,
                Err(e) => return refuse(tr!("`{f}` を読めません: {e}", "cannot read `{f}`: {e}"), lang),
            };
            let Some(checked) = o.checked.as_ref().filter(|_| !o.has_errors()) else {
                refused(&o, tr!("`{f}` は検査を通らないので、生成しません", "`{f}` does not pass check, so nothing is generated from it"), lang);
                failed = true;
                continue;
            };
            let u = koyomi::codegen::unit_of(checked, lang);
            for t in &targets {
                for (rel, body) in koyomi::codegen::files(&u, *t) {
                    if let Some((_, _, from)) = planned.iter().find(|(p, _, _)| *p == rel) {
                        return refuse(
                            tr!(
                                "`{f}` と `{from}` が同じ {rel} を書きます。どちらかの別名を変えます",
                                "`{f}` and `{from}` both write {rel}; give one of them another alias"
                            ),
                            lang,
                        );
                    }
                    planned.push((rel, body, f.clone()));
                }
            }
        }
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
    ExitCode::from(if failed || dirty { 1 } else { 0 })
}

fn vectors_cmd(a: &Args, lang: Lang) -> ExitCode {
    use std::io::Write;
    let [path] = a.pos.as_slice() else {
        return refuse(tr!("`koyomi vectors` には .cal のファイルを一つ渡します", "`koyomi vectors` takes one .cal file"), lang);
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
            let lines = koyomi::vectors::DatesLines::new(m);
            for r in koyomi::vectors::DatesRows::new(m) {
                let _ = writeln!(w, "{}", lines.line(&r));
            }
        }
        Checked::Calendar(c) => {
            for r in koyomi::vectors::calendar_rows(c) {
                let _ = writeln!(w, "{}", koyomi::vectors::calendar_line(&r));
            }
        }
    }
    let _ = w.flush();
    ExitCode::SUCCESS
}

fn doc_cmd(a: &Args, lang: Lang) -> ExitCode {
    let [path] = a.pos.as_slice() else {
        return refuse(tr!("`koyomi doc` には .cal のファイルを一つ渡します", "`koyomi doc` takes one .cal file"), lang);
    };
    let months = match a.get("--months") {
        None => None,
        Some(m) => match koyomi::doc::months::parse_span(m) {
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
    let format = if a.get("--format") == Some("html") { koyomi::doc::Format::Html } else { koyomi::doc::Format::Markdown };
    let mut loader = Loader::default();
    let o = match check::check_file(path, &Options::default(), &mut loader) {
        Ok(o) => o,
        Err(e) => return refuse(tr!("`{path}` を読めません: {e}", "cannot read `{path}`: {e}"), lang),
    };
    match koyomi::doc::page(&o, lang, &koyomi::doc::Options { months }) {
        Ok(p) => {
            print!("{}", koyomi::doc::render(&p, format));
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
    let Ok(src) = std::fs::read_to_string(path) else {
        return refuse(tr!("`{path}` を読めません", "cannot read `{path}`"), lang);
    };
    let parsed = koyomi::parse::parse(path, &src);
    let Some(f) = parsed.file.filter(|_| !parsed.diags.iter().any(|d| d.is_error())) else {
        print_diags(&parsed.diags, lang);
        return ExitCode::from(1);
    };
    let dir = koyomi::fetch::dir_of(path);
    let result = match verb.as_str() {
        "fetch" => koyomi::fetch::fetch(&f, &dir),
        "outdated" => koyomi::fetch::outdated(&f, &dir),
        _ => koyomi::fetch::pin(&f, &dir, &src).and_then(|(text, o)| {
            if text != src {
                std::fs::write(path, &text).map_err(|e| tr!("`{path}` に書けません: {e}", "cannot write `{path}`: {e}"))?;
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
