//! The `sakai` command. What exists and what it takes is `cli.rs`'s table.

use sakai::check;
use sakai::cli::{self, Args};
use sakai::i18n::{Lang, Text};
use sakai::paths;
use sakai::tr;
use std::path::{Path, PathBuf};
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
    // A panic is a bug in sakai: exit 2, as DESIGN 5.1 says, rather than Rust's 101.
    std::panic::set_hook(Box::new(|info| {
        eprintln!("sakai: a bug in sakai, please report it: {info}");
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
            println!("sakai {}", env!("CARGO_PKG_VERSION"));
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
                    None => refuse(tr!("`{n}` というコマンドはありません。`sakai --help` を読んでください", "there is no command `{n}`; run `sakai --help`"), lang),
                },
            };
        }
        _ => {}
    }
    let Some(cmd) = cmds.iter().find(|c| c.name == first.as_str()) else {
        return refuse(tr!("`{first}` というコマンドはありません。`sakai --help` を読んでください", "there is no command `{first}`; run `sakai --help`"), lang);
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
        "build" => build_cmd(&a, lang),
        "export" => export_cmd(&a, lang),
        "api" => api_cmd(&a, lang),
        "explain" => explain_cmd(&a, lang),
        _ => unreachable!("every command in the table is dispatched"),
    }
}

/// The root (DESIGN 2.4): `--root`, else found from the first path.
fn root_of(a: &Args, first: &str) -> Result<PathBuf, Text> {
    match a.get("--root") {
        Some(r) => {
            let p = paths::absolute(Path::new(r));
            if p.is_dir() {
                Ok(p)
            } else {
                Err(tr!("`--root {r}` はディレクトリではありません", "`--root {r}` is not a directory"))
            }
        }
        None => Ok(paths::find_root(Path::new(first))),
    }
}

/// The paths of the command line, as paths from the root.
fn from_root(root: &Path, args: &[String]) -> Result<Vec<String>, Text> {
    args.iter()
        .map(|f| {
            if !Path::new(f).exists() {
                return Err(tr!("`{f}` がありません", "`{f}` is not there"));
            }
            paths::from_root(root, Path::new(f)).ok_or_else(|| {
                let r = root.display().to_string();
                tr!("`{f}` はルート {r} の外にあります", "`{f}` is outside the root {r}")
            })
        })
        .collect()
}

fn check_cmd(a: &Args, lang: Lang) -> ExitCode {
    let Some(first) = a.pos.first() else {
        return refuse(tr!("`sakai check` には map のファイルかディレクトリが要ります", "`sakai check` needs map files or directories"), lang);
    };
    let root = match root_of(a, first) {
        Ok(r) => r,
        Err(e) => return refuse(e, lang),
    };
    paths::show_from(paths::Shown::new(&root, first));
    let rel = match from_root(&root, &a.pos) {
        Ok(r) => r,
        Err(e) => return refuse(e, lang),
    };
    let outcomes = match check::check_args(&root, &rel) {
        Ok(o) => o,
        Err(e) => return refuse(e, lang),
    };
    let json = a.get("--format") == Some("json");
    let mut errors = false;
    for o in &outcomes {
        errors |= o.has_errors();
        if json {
            println!("{}", check::to_json(o, lang));
        } else {
            print!("{}", check::render(o, lang));
        }
    }
    ExitCode::from(if errors { 1 } else { 0 })
}

/// The one map file a command takes: the root, the map from it, and the paths shown from here.
fn one_map(a: &Args, cmd: &str, lang: Lang) -> Result<(PathBuf, String), ExitCode> {
    let [map] = a.pos.as_slice() else {
        return Err(refuse(tr!("`sakai {cmd}` には map のファイルを一つ渡します", "`sakai {cmd}` takes one map file"), lang));
    };
    if Path::new(map).is_dir() {
        return Err(refuse(tr!("`sakai {cmd}` には map のファイルを渡します（ディレクトリではなく）", "`sakai {cmd}` takes a map file, not a directory"), lang));
    }
    let root = root_of(a, map).map_err(|e| refuse(e, lang))?;
    paths::show_from(paths::Shown::new(&root, map));
    let rel = from_root(&root, &a.pos).map_err(|e| refuse(e, lang))?;
    Ok((root, rel[0].clone()))
}

fn build_cmd(a: &Args, lang: Lang) -> ExitCode {
    let Some(target) = a.get("--target").and_then(sakai::build::Target::from_word) else {
        return refuse(
            tr!("`sakai build` には --target が要ります（import-linter | dependency-cruiser | archunit | go-arch-lint）", "`sakai build` needs --target (import-linter | dependency-cruiser | archunit | go-arch-lint)"),
            lang,
        );
    };
    let (root, map) = match one_map(a, "build", lang) {
        Ok(x) => x,
        Err(code) => return code,
    };
    let built = match sakai::build::run(&root, &map, target, a.get("--out").map(Path::new), a.has("--check"), lang) {
        Ok(b) => b,
        Err(e) => return refuse(e, lang),
    };
    let o = &built.outcome;
    for d in &o.diags {
        print!("{}", d.render(lang));
    }
    if let Some(done) = &built.done {
        print!("{}", sakai::build::render_done(&root, done, lang));
    }
    ExitCode::from(if o.has_errors() { 1 } else { 0 })
}

fn export_cmd(a: &Args, lang: Lang) -> ExitCode {
    if a.pos.first().map(String::as_str) != Some("cml") {
        return refuse(tr!("`sakai export` のあとには、書き出す形 `cml` を書きます", "`sakai export` is followed by the form to write, `cml`"), lang);
    }
    let rest = Args { got: a.got.clone(), pos: a.pos[1..].to_vec() };
    let (root, map) = match one_map(&rest, "export cml", lang) {
        Ok(x) => x,
        Err(code) => return code,
    };
    let o = match check::check_map(&root, &map) {
        Ok(o) => o,
        Err(e) => return refuse(e, lang),
    };
    if o.has_errors() {
        eprint!("{}", check::render(&o, lang));
        return ExitCode::from(1);
    }
    let c = o.checked.as_ref().expect("a map with no errors is checked through");
    let name = Path::new(&map).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let text = sakai::cml::render(c, &name, lang);
    match a.get("--out") {
        Some(f) => {
            if let Err(e) = std::fs::write(f, &text) {
                let e = e.to_string();
                return refuse(tr!("{f} に書けません: {e}", "cannot write {f}: {e}"), lang);
            }
        }
        None => print!("{text}"),
    }
    ExitCode::SUCCESS
}

fn api_cmd(a: &Args, lang: Lang) -> ExitCode {
    if a.pos.len() != 1 {
        return refuse(tr!("`sakai api` には map のファイルを一つ渡します", "`sakai api` takes one map file"), lang);
    }
    if Path::new(&a.pos[0]).is_dir() {
        return refuse(tr!("`sakai api` には map のファイルを渡します（ディレクトリではなく）", "`sakai api` takes a map file, not a directory"), lang);
    }
    let root = match root_of(a, &a.pos[0]) {
        Ok(r) => r,
        Err(e) => return refuse(e, lang),
    };
    paths::show_from(paths::Shown::new(&root, &a.pos[0]));
    let rel = match from_root(&root, &a.pos) {
        Ok(r) => r,
        Err(e) => return refuse(e, lang),
    };
    let o = match check::check_map(&root, &rel[0]) {
        Ok(o) => o,
        Err(e) => return refuse(e, lang),
    };
    if o.has_errors() {
        eprint!("{}", check::render(&o, lang));
        return ExitCode::from(1);
    }
    let v = sakai::api::api(o.checked.as_ref().expect("a map with no errors is checked through"));
    println!("{}", serde_json::to_string_pretty(&v).unwrap());
    ExitCode::SUCCESS
}

fn explain_cmd(a: &Args, lang: Lang) -> ExitCode {
    let md = a.get("--format") == Some("markdown");
    if a.has("--all") {
        if !a.pos.is_empty() {
            return refuse(tr!("`--all` とコードは一緒に書けません", "`--all` takes no code"), lang);
        }
        if md {
            print!("{}", sakai::codes::render_markdown(lang));
        } else {
            for (i, e) in sakai::codes::ledger().iter().enumerate() {
                if i > 0 {
                    println!();
                }
                print!("{}", sakai::codes::render_text(e, lang));
            }
        }
        return ExitCode::SUCCESS;
    }
    let [code] = a.pos.as_slice() else {
        return refuse(tr!("`sakai explain` には `E201` のようなコードを一つ渡します", "`sakai explain` takes one code, like `E201`"), lang);
    };
    match sakai::codes::find(code) {
        Some(e) => {
            if md {
                print!("{}", sakai::codes::render_markdown_one(&e, lang));
            } else {
                print!("{}", sakai::codes::render_text(&e, lang));
            }
            ExitCode::SUCCESS
        }
        None => refuse(tr!("`{code}` という診断のコードはありません", "there is no diagnostic code `{code}`"), lang),
    }
}
