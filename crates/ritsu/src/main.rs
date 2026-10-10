//! `ritsu`: the one command of the toolchain (DESIGN 8.1). `ritsu check` checks a project's files,
//! each with its language's own check, the languages joined once by ritsu-project (DESIGN 6), and
//! then across the languages (ritsu-cross, whose codes `ritsu explain` looks up);
//! `ritsu <language> …` is the language's own command, with the languages it reads joined — what
//! the binary of a receiving language's crate cannot do, since that crate holds no other language
//! (DESIGN 2.3). Called by a language's name (a link named `rulec`), ritsu is that command
//! (DESIGN 2.3, 8.2). `ritsu run` runs a workflow with its rules, dates and books computed by
//! their languages (DESIGN 7.9); `ritsu gen` writes a project as one package for each of
//! TypeScript, Python and Go (DESIGN 9.3); `ritsu skills` writes the nine Agent Skills the binary
//! carries where an agent reads them (PLAN F.3).

mod package;

use ritsu::{check, cli, explain, run, skills};
use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;
use ritsu_project::Joined;
use std::io::Write;
use std::process::ExitCode;

/// Die quietly when the reader of a pipe goes away, as `cat` does (`ritsu rulec vectors | head`).
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

/// `--lang ja`, `--lang=ja`, before the command.
fn lang_asked(args: &[String]) -> Option<String> {
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--lang" {
            return args.get(i + 1).cloned();
        }
        if let Some(v) = args[i].strip_prefix("--lang=") {
            return Some(v.to_string());
        }
        if !args[i].starts_with('-') {
            break;
        }
        i += 1;
    }
    None
}

fn refuse(msg: Text, lang: Lang) -> ExitCode {
    let head = if lang == Lang::Ja { "エラー" } else { "error" };
    eprintln!("{head}: {}", msg.get(lang));
    ExitCode::from(2)
}

/// The language's own command, on the words after its name, with the languages it reads joined.
fn language(name: &str, args: &[String]) -> ExitCode {
    let (mut out, mut err) = (std::io::stdout(), std::io::stderr());
    let code = match name {
        // rulec reads the days of `range from koyomi` through koyomi's port (DESIGN 7.5 (b))
        "rulec" => return rulec::days::with(Some(Joined::dates()), || rulec::cli::run(args.to_vec())),
        "koyomi" => return koyomi::run::run(args.to_vec()),
        "chobo" => return chobo::run::run(args.to_vec()),
        "geas" => {
            let code = geas::cli::run(args);
            let _ = out.flush();
            std::process::exit(code);
        }
        // a flow reads its rules, dates files and books in the same process (DESIGN 7.8), as the
        // page in the browser runs it too
        "dandori" => ritsu::languages::dandori(args, &mut out, &mut err),
        "yuen" => yuen::run::run(args, Joined::new().yuen(), &mut out, &mut err),
        "sakai" => sakai::run::run(args, Joined::new().sakai(), &mut out, &mut err),
        // a gate reads its rules, dates files, calendars, books and flows in the same process
        "sekisho" => sekisho::run::run(args, Joined::new().sekisho().into(), &mut out, &mut err),
        _ => unreachable!("only the languages come here"),
    };
    let _ = out.flush();
    ExitCode::from(code)
}

fn main() -> ExitCode {
    restore_sigpipe();
    // A panic is a bug in ritsu: exit 2, as DESIGN 8.4 says, rather than Rust's 101.
    std::panic::set_hook(Box::new(|info| {
        // Built for WASI there is no SIGPIPE: a reader that went away (`| head`) is an error of the
        // write, which std's printing panics on. The native binary dies of the signal quietly, as
        // `cat` does; this one goes quietly too, with the code a shell gives that death (DESIGN 8.8).
        #[cfg(target_os = "wasi")]
        if info.payload_as_str().is_some_and(|s| s.starts_with("failed printing to std") && s.contains("Broken pipe")) {
            std::process::exit(128 + 13);
        }
        eprintln!("ritsu: a bug in ritsu, please report it: {info}");
    }));
    match std::panic::catch_unwind(run) {
        Ok(code) => code,
        Err(_) => ExitCode::from(2),
    }
}

/// Built for WASI (the npm package, DESIGN 8.8), ritsu starts in the directory the host gives
/// it, which is `/` under Node; the loader names the one it was run in (`RITSU_WASI_CWD`), and
/// ritsu works there, as the native binary works in the one its shell gives it. A directory it
/// cannot enter stops it, rather than leaving it to read from the root of the disk. Without the
/// variable (wasmtime and other hosts, which give the directory with their own flags) ritsu
/// stays where the host put it.
#[cfg(target_os = "wasi")]
fn enter_the_directory(args: &[String]) -> Result<(), ExitCode> {
    let Some(dir) = std::env::var_os("RITSU_WASI_CWD") else { return Ok(()) };
    match std::env::set_current_dir(&dir) {
        Ok(()) => Ok(()),
        Err(e) => {
            let lang = Lang::pick(lang_asked(args).as_deref(), "RITSU_LANG");
            let (d, e) = (std::path::Path::new(&dir).display().to_string(), ritsu_base::fs::as_native(e));
            Err(refuse(tr!("RITSU_WASI_CWD のディレクトリ `{d}` で作業できません: {e}", "cannot work in `{d}`, the directory RITSU_WASI_CWD names: {e}"), lang))
        }
    }
}

fn run() -> ExitCode {
    let mut argv = std::env::args();
    let called = argv.next().unwrap_or_default();
    let args: Vec<String> = argv.collect();
    #[cfg(target_os = "wasi")]
    if let Err(code) = enter_the_directory(&args) {
        return code;
    }
    // called by a language's name, ritsu is that command
    let name = std::path::Path::new(&called).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    if cli::LANGUAGES.contains(&name.as_str()) {
        return language(&name, &args);
    }
    let lang = Lang::pick(lang_asked(&args).as_deref(), "RITSU_LANG");
    let table = cli::table();
    // the command is the first word that is not ritsu's own `--lang`
    let mut at = 0;
    while at < args.len() && (args[at] == "--lang" || args[at].starts_with("--lang=")) {
        at += if args[at] == "--lang" { 2 } else { 1 };
    }
    let Some(first) = args.get(at) else {
        eprint!("{}", table.help_all(lang));
        return ExitCode::from(2);
    };
    match first.as_str() {
        "--help" | "-h" => {
            print!("{}", table.help_all(lang));
            ExitCode::SUCCESS
        }
        "help" => match args.get(at + 1) {
            None => {
                print!("{}", table.help_all(lang));
                ExitCode::SUCCESS
            }
            Some(l) if cli::is_language(l) => language(l, &["--help".to_string()]),
            Some(c) => match table.command(c) {
                Some(cmd) => {
                    print!("{}", table.help_cmd(cmd, lang));
                    ExitCode::SUCCESS
                }
                None => refuse(tr!("`{c}` というコマンドはありません。`ritsu --help` を読んでください", "there is no command `{c}`; run `ritsu --help`"), lang),
            },
        },
        "--version" | "-V" => {
            println!("ritsu {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        "check" | "explain" | "gen" | "skills" => {
            let mut rest: Vec<String> = args[..at].to_vec();
            rest.extend(args[at + 1..].iter().cloned());
            ExitCode::from(match first.as_str() {
                "check" => check::command(&rest, lang),
                "gen" => package::command(&rest, lang),
                "skills" => skills::command(&rest, lang),
                _ => explain::command(&rest, lang),
            })
        }
        "run" => {
            let mut rest: Vec<String> = args[..at].to_vec();
            rest.extend(args[at + 1..].iter().cloned());
            ExitCode::from(run::command(&rest, lang))
        }
        l if cli::is_language(l) => language(l, &args[at + 1..]),
        other => refuse(tr!("`{other}` というコマンドはありません。`ritsu --help` を読んでください", "there is no command `{other}`; run `ritsu --help`"), lang),
    }
}
