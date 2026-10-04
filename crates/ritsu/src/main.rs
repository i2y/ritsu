//! `ritsu`: the one command of the toolchain (DESIGN 8.1). `ritsu check` checks a project's files,
//! each with its language's own check, the languages joined once by ritsu-project (DESIGN 6), and
//! then across the languages (ritsu-cross, whose codes `ritsu explain` looks up);
//! `ritsu <language> …` is the language's own command, with the languages it reads joined — what
//! the binary of a receiving language's crate cannot do, since that crate holds no other language
//! (DESIGN 2.3). Called by a language's name (a link named `rulec`), ritsu is that command
//! (DESIGN 2.3, 8.2). `ritsu run` runs a workflow with its rules, dates and books computed by
//! their languages (DESIGN 7.9); `ritsu gen` comes later in stage E, `ritsu lsp` in F.

use ritsu::{check, cli, explain, run};
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
        "dandori" => {
            // a flow reads its rules, dates files and books in the same process (DESIGN 7.8)
            let j = Joined::new();
            dandori::cli::run_with_ports(args, j.rules(), j.koyomi.clone(), j.chobo.clone(), &mut out, &mut err)
        }
        "yuen" => yuen::run::run(args, Joined::new().yuen(), &mut out, &mut err),
        "sakai" => sakai::run::run(args, Joined::new().sakai(), &mut out, &mut err),
        _ => unreachable!("only the seven languages come here"),
    };
    let _ = out.flush();
    ExitCode::from(code)
}

fn main() -> ExitCode {
    restore_sigpipe();
    // A panic is a bug in ritsu: exit 2, as DESIGN 8.4 says, rather than Rust's 101.
    std::panic::set_hook(Box::new(|info| {
        eprintln!("ritsu: a bug in ritsu, please report it: {info}");
    }));
    match std::panic::catch_unwind(run) {
        Ok(code) => code,
        Err(_) => ExitCode::from(2),
    }
}

fn run() -> ExitCode {
    let mut argv = std::env::args();
    let called = argv.next().unwrap_or_default();
    let args: Vec<String> = argv.collect();
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
            Some(l) if cli::LANGUAGES.contains(&l.as_str()) => language(l, &["--help".to_string()]),
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
        "check" | "explain" => {
            let mut rest: Vec<String> = args[..at].to_vec();
            rest.extend(args[at + 1..].iter().cloned());
            ExitCode::from(if first == "check" { check::command(&rest, lang) } else { explain::command(&rest, lang) })
        }
        "run" => {
            let mut rest: Vec<String> = args[..at].to_vec();
            rest.extend(args[at + 1..].iter().cloned());
            ExitCode::from(run::command(&rest, lang))
        }
        l if cli::LANGUAGES.contains(&l) => language(l, &args[at + 1..]),
        other => refuse(tr!("`{other}` というコマンドはありません。`ritsu --help` を読んでください", "there is no command `{other}`; run `ritsu --help`"), lang),
    }
}
