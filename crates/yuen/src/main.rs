//! The `yuen` command. What exists and what it takes is `cli.rs`'s table.

use std::process::ExitCode;
use yuen::cli::{self, Args};
use yuen::i18n::{Lang, Text};
use yuen::tr;

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
    // A panic is a bug in yuen: exit 2, as DESIGN 6.1 says, rather than Rust's 101.
    std::panic::set_hook(Box::new(|info| {
        eprintln!("yuen: a bug in yuen, please report it: {info}");
    }));
    match std::panic::catch_unwind(run) {
        Ok(code) => code,
        Err(_) => ExitCode::from(2),
    }
}

fn run() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let lang = Lang::pick(lang_flag(&args).as_deref());
    let Some(first) = args.first() else {
        eprint!("{}", cli::help_all(lang));
        return ExitCode::from(2);
    };
    let cmds = cli::commands();
    match first.as_str() {
        "--help" | "-h" => {
            print!("{}", cli::help_all(lang));
            return ExitCode::SUCCESS;
        }
        "--version" | "-V" => {
            println!("yuen {}", env!("CARGO_PKG_VERSION"));
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
                    print!("{}", cli::help_all(lang));
                    ExitCode::SUCCESS
                }
                Some(n) => match cmds.iter().find(|c| c.name == n.as_str()) {
                    Some(c) => {
                        print!("{}", cli::help_cmd(c, lang));
                        ExitCode::SUCCESS
                    }
                    None => refuse(tr!("`{n}` というコマンドはありません。`yuen --help` を読んでください", "there is no command `{n}`; run `yuen --help`"), lang),
                },
            };
        }
        _ => {}
    }
    let Some(cmd) = cmds.iter().find(|c| c.name == first.as_str()) else {
        return refuse(tr!("`{first}` というコマンドはありません。`yuen --help` を読んでください", "there is no command `{first}`; run `yuen --help`"), lang);
    };
    let a = match cli::parse(cmd, &args[1..]) {
        Ok(a) => a,
        Err(e) => return refuse(e, lang),
    };
    if a.has("--help") {
        print!("{}", cli::help_cmd(cmd, lang));
        return ExitCode::SUCCESS;
    }
    match cmd.name {
        "check" => check_cmd(&a, lang),
        "review" => review_cmd(&a, lang),
        "trace" => trace_cmd(&a, lang),
        "api" => api_cmd(&a, lang),
        "export" => export_cmd(&a, lang),
        "source" => source_cmd(&a, lang),
        "explain" => explain_cmd(&a, lang),
        _ => unreachable!("every command in the table is dispatched"),
    }
}

fn check_cmd(a: &Args, lang: Lang) -> ExitCode {
    if a.pos.is_empty() {
        return refuse(tr!("`yuen check` には .req のファイルかディレクトリが要ります", "`yuen check` needs .req files or directories"), lang);
    }
    let c = match yuen::check::check(&a.pos, a.get("--root")) {
        Ok(c) => c,
        Err(r) => return refuse(r.0, lang),
    };
    let label = a.pos.join(", ");
    if a.get("--format") == Some("json") {
        println!("{}", yuen::check::to_json(&c, &label, lang));
    } else {
        print!("{}", yuen::check::render(&c, &label, lang));
    }
    ExitCode::from(if c.has_errors() { 1 } else { 0 })
}

fn review_cmd(a: &Args, lang: Lang) -> ExitCode {
    if a.pos.is_empty() {
        return refuse(tr!("`yuen review` には .req のファイルかディレクトリが要ります", "`yuen review` needs .req files or directories"), lang);
    }
    let Some(by) = a.get("--by") else {
        return refuse(tr!("`yuen review` には、確かめた人の役割 `--by <役割>` が要ります", "`yuen review` needs the role of whoever looked: `--by <role>`"), lang);
    };
    let choice = yuen::review::Choice { at: a.all("--at"), requirements: a.all("--requirement"), all: a.has("--all") };
    match yuen::review::review(&a.pos, a.get("--root"), &choice, by, a.get("--date")) {
        Ok(o) => {
            print!("{}", yuen::review::render(&o, lang));
            ExitCode::from(o.exit)
        }
        Err(r) => refuse(r.0, lang),
    }
}

/// Check a project for a command built on the check: None, with the diagnostics on standard
/// error and exit 1, when its words or names have errors.
fn checked(a: &Args, lang: Lang, cmd: &str) -> Result<yuen::check::Checked, ExitCode> {
    checked_paths(&a.pos, a.get("--root"), lang, cmd)
}

fn checked_paths(paths: &[String], root: Option<&str>, lang: Lang, cmd: &str) -> Result<yuen::check::Checked, ExitCode> {
    if paths.is_empty() {
        return Err(refuse(tr!("`yuen {cmd}` には .req のファイルかディレクトリが要ります", "`yuen {cmd}` needs .req files or directories"), lang));
    }
    let c = match yuen::check::check(paths, root) {
        Ok(c) => c,
        Err(r) => return Err(refuse(r.0, lang)),
    };
    if !c.named() {
        let label = paths.join(", ");
        eprint!("{}", yuen::check::render(&c, &label, lang));
        return Err(ExitCode::from(1));
    }
    Ok(c)
}

/// `yuen export reqif|prov <path>...` (DESIGN 12, 13).
fn export_cmd(a: &Args, lang: Lang) -> ExitCode {
    let what = a.pos.first().map(|s| s.as_str()).unwrap_or("");
    if !matches!(what, "reqif" | "prov") {
        return refuse(tr!("`yuen export reqif <path>...` か `yuen export prov <path>...` です", "It is `yuen export reqif <path>...` or `yuen export prov <path>...`"), lang);
    }
    if what == "reqif" && a.has("--format") {
        return refuse(tr!("`--format` は prov のときだけ書けます（ReqIF の形は一つです）", "`--format` is for prov only (ReqIF has one form)"), lang);
    }
    if what == "prov" && a.has("--time") {
        return refuse(tr!("`--time` は reqif のときだけ書けます（PROV の時刻は、記録の日付だけです）", "`--time` is for reqif only (the times of PROV are the days the records give)"), lang);
    }
    let time = match a.get("--time") {
        None => None,
        Some(t) => match yuen::export::reqif::read_time(t) {
            Some(x) => Some(x),
            None => return refuse(tr!("`--time {t}` は RFC 3339 の日時（`2026-10-03T09:00:00+09:00` のように、時差まで）ではありません", "`--time {t}` is not an RFC 3339 date and time (`2026-10-03T09:00:00+09:00`, with its offset)"), lang),
        },
    };
    let paths: Vec<String> = a.pos[1..].to_vec();
    let c = match checked_paths(&paths, a.get("--root"), lang, "export") {
        Ok(c) => c,
        Err(code) => return code,
    };
    if yuen::export::blocked(&c) {
        let label = paths.join(", ");
        eprint!("{}", yuen::check::render(&c, &label, lang));
        return ExitCode::from(1);
    }
    let (p, m) = (c.project.as_ref().unwrap(), c.model.as_ref().unwrap());
    let g = yuen::export::graph(p, m);
    let text = match what {
        "reqif" => match yuen::export::reqif::write(&g, time.as_deref(), yuen::api::VERSION) {
            Ok(t) => t,
            Err(r) => return refuse(r.0, lang),
        },
        _ if a.get("--format") == Some("json") => yuen::export::prov::provjson(&g),
        _ => yuen::export::prov::provn(&g),
    };
    match a.get("--out") {
        Some(out) => {
            if let Err(e) = std::fs::write(out, &text) {
                return refuse(tr!("`{out}` に書けません: {e}", "Cannot write `{out}`: {e}"), lang);
            }
            println!("{}", tr!("書き出しました: {out}", "wrote: {out}").get(lang));
        }
        None => print!("{text}"),
    }
    ExitCode::SUCCESS
}

/// `yuen source fetch|pin|outdated <path>...` (DESIGN 14): the project read as far as its
/// names, and checked no further.
fn source_cmd(a: &Args, lang: Lang) -> ExitCode {
    let verb = a.pos.first().map(|s| s.as_str()).unwrap_or("");
    if !matches!(verb, "fetch" | "pin" | "outdated") {
        return refuse(tr!("`yuen source fetch|pin|outdated <path>...` です", "It is `yuen source fetch|pin|outdated <path>...`"), lang);
    }
    let paths: Vec<String> = a.pos[1..].to_vec();
    if paths.is_empty() {
        return refuse(tr!("`yuen source {verb}` には .req のファイルかディレクトリが要ります", "`yuen source {verb}` needs .req files or directories"), lang);
    }
    let label = paths.join(", ");
    let (project, mut diags) = match yuen::project::load(&paths, a.get("--root")) {
        Ok(x) => x,
        Err(r) => return refuse(r.0, lang),
    };
    let named = project.map(|mut p| {
        diags.extend(yuen::project::check_names(&mut p));
        p
    });
    let p = match named {
        Some(p) if !yuen::diag::has_errors(&diags) => p,
        _ => {
            for d in &diags {
                eprint!("{}", d.render(lang));
            }
            let (e, _) = yuen::diag::count(&diags);
            eprintln!("{}", tr!("{label}: エラー {e} 件", "{label}: {}", ; yuen::i18n::plural(e, "error", "errors")).get(lang));
            return ExitCode::from(1);
        }
    };
    let result = match verb {
        "fetch" => yuen::fetch::fetch(&p),
        "pin" => {
            let (texts, o) = yuen::fetch::pin(&p);
            for (fi, text) in texts {
                let f = &p.files[fi];
                if let Err(e) = std::fs::write(&f.abs, text) {
                    let d = &f.display;
                    return refuse(tr!("`{d}` に書けません: {e}", "Cannot write `{d}`: {e}"), lang);
                }
            }
            Ok(o)
        }
        _ => {
            if let Some((fi, line, what)) = yuen::fetch::borrowed(&p) {
                let at = format!("{}:{line}", p.files[fi].display);
                return refuse(tr!(
                    "yuen はまだ借りた出典を読めません: {what}（{at}）。借りた出典の改正は、それを固定しているツールの source outdated で問います",
                    "yuen cannot read borrowed sources yet: {what} ({at}); ask the tool that pins it, with its own source outdated"
                ), lang);
            }
            yuen::fetch::outdated(&p)
        }
    };
    match result {
        Ok(o) => {
            if o.lines.is_empty() {
                println!("{}", tr!("{label}: 出典の宣言がありません", "{label}: no source is declared").get(lang));
            }
            for l in &o.lines {
                println!("{}", l.get(lang));
            }
            ExitCode::from(if verb == "outdated" && o.changed { 1 } else { 0 })
        }
        Err(e) => refuse(e, lang),
    }
}

fn trace_cmd(a: &Args, lang: Lang) -> ExitCode {
    let picks: Vec<yuen::trace::Start> = [
        a.get("--requirement").map(|s| yuen::trace::Start::Requirement(s.to_string())),
        a.get("--artifact").map(|s| yuen::trace::Start::Artifact(s.to_string())),
        a.get("--source").map(|s| yuen::trace::Start::Source(s.to_string())),
    ]
    .into_iter()
    .flatten()
    .collect();
    if picks.len() != 1 {
        return refuse(tr!("`--requirement`、`--artifact`、`--source` のどれか一つを渡します", "Give exactly one of `--requirement`, `--artifact` and `--source`"), lang);
    }
    let c = match checked(a, lang, "trace") {
        Ok(c) => c,
        Err(code) => return code,
    };
    match yuen::trace::trace(&c, &picks[0]) {
        Ok(t) => {
            if a.get("--format") == Some("json") {
                println!("{}", serde_json::to_string_pretty(&t.json).unwrap());
            } else {
                print!("{}", t.render(lang));
            }
            ExitCode::SUCCESS
        }
        Err(r) => refuse(r.0, lang),
    }
}

fn api_cmd(a: &Args, lang: Lang) -> ExitCode {
    let c = match checked(a, lang, "api") {
        Ok(c) => c,
        Err(code) => return code,
    };
    let label = a.pos.join(", ");
    match yuen::api::api(&c, &label, lang) {
        Some(v) => {
            println!("{}", serde_json::to_string_pretty(&v).unwrap());
            ExitCode::SUCCESS
        }
        None => ExitCode::from(1),
    }
}

fn explain_cmd(a: &Args, lang: Lang) -> ExitCode {
    let markdown = a.get("--format") == Some("markdown");
    if a.has("--all") {
        if markdown {
            print!("{}", yuen::codes::render_markdown(lang));
        } else {
            for (i, e) in yuen::codes::ledger().iter().enumerate() {
                if i > 0 {
                    println!();
                }
                print!("{}", yuen::codes::render_text(e, lang));
            }
        }
        return ExitCode::SUCCESS;
    }
    let Some(code) = a.pos.first() else {
        return refuse(tr!("`yuen explain` には `E302` のようなコードか `--all` が要ります", "`yuen explain` needs a code such as `E302`, or `--all`"), lang);
    };
    match yuen::codes::find(code) {
        Some(e) => {
            if markdown {
                print!("{}", yuen::codes::render_markdown_one(&e, lang));
            } else {
                print!("{}", yuen::codes::render_text(&e, lang));
            }
            ExitCode::SUCCESS
        }
        None => refuse(tr!("`{code}` という診断のコードはありません。`yuen explain --all` で全部を見られます", "there is no diagnostic code `{code}`; `yuen explain --all` lists them all"), lang),
    }
}
