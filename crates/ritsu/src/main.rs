//! `ritsu`: the one command of the toolchain (DESIGN 8.1). In stage D it has its least form
//! (DESIGN 8.6): the languages that read others run with the ports joined — `ritsu dandori …`
//! with rulec's answer to the port of rules, so that a workflow reads its rules in the same
//! process, `ritsu yuen …` with every language yuen reads, and `ritsu sakai …` with every
//! language sakai reads. What the binaries of their own crates cannot do, since those crates hold
//! no other language (DESIGN 2.3). The rest of DESIGN
//! 8.1 (`ritsu check`, `ritsu run`, `ritsu gen`, `ritsu explain`, `ritsu <language>` for every
//! language) comes in stage E.

use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;
use std::io::Write;
use std::process::ExitCode;
use std::rc::Rc;

/// The languages whose commands `ritsu <language>` will run (DESIGN 8.2), but for the ones that
/// read others (dandori, yuen, sakai), run as their own commands until stage E: they read no other
/// language, so their own binaries do all they do.
const ON_THEIR_OWN: [&str; 4] = ["rulec", "koyomi", "chobo", "geas"];

/// Every language yuen reads, joined through the ports (yuen's DESIGN 3.1): what a file holds
/// (`Items`), the sources a rule or a calendar pins (`Sources`), the aliases of a rule's and a dates
/// file's names (`Rules`, `Dates`), and the claims of a spec with their records (`Claims`).
fn yuen_suite() -> yuen::suite::Suite {
    let rules = Rc::new(rulec::ports::Engine::new());
    let dates = Rc::new(koyomi::ports::Engine);
    let claims = Rc::new(geas::ports::Engine);
    let mut s = yuen::suite::Suite::default();
    s.items.insert("rulec".into(), rules.clone());
    s.items.insert("koyomi".into(), dates.clone());
    s.items.insert("chobo".into(), Rc::new(chobo::ports::Engine));
    s.items.insert("geas".into(), claims.clone());
    s.items.insert("dandori".into(), Rc::new(dandori::ports::Engine));
    s.items.insert("sakai".into(), Rc::new(sakai::ports::Engine));
    s.sources.insert("rulec".into(), rules.clone());
    s.sources.insert("koyomi".into(), dates.clone());
    s.rules = Some(rules);
    s.dates = Some(dates);
    s.claims = Some(claims);
    s
}

/// Every language sakai reads, joined through the ports (sakai's DESIGN 4.1): a rule's enums, its
/// Connect service and its names (`Rules`), what a rule, a calendar and a workflow name outside
/// themselves (`References`), and a book's accounts and transfers (`Books`).
fn sakai_suite() -> sakai::suite::Suite {
    let rules = Rc::new(rulec::ports::Engine::new());
    let mut s = sakai::suite::Suite::default();
    s.references.insert("rulec".into(), rules.clone());
    s.references.insert("koyomi".into(), Rc::new(koyomi::ports::Engine));
    s.references.insert("dandori".into(), Rc::new(dandori::ports::Engine));
    s.rules = Some(rules);
    s.books = Some(Rc::new(chobo::ports::Engine));
    s
}

fn help(lang: Lang) -> String {
    let v = env!("CARGO_PKG_VERSION");
    let t = tr!(
        "ritsu {v} — 七つの小さな言語（rulec、dandori、koyomi、chobo、geas、yuen、sakai）を一つにまとめる処理系\n\n\
         使い方:\n  ritsu dandori <コマンド> ...   dandori のコマンド。フローが使う規則を同じプロセスの中で読む\n  ritsu yuen <コマンド> ...      yuen のコマンド。要件が名指すほかの言語のもの、借りた出典、主張の記録を同じプロセスの中で読む\n  ritsu sakai <コマンド> ...     sakai のコマンド。地図が持つ規則、カレンダー、ワークフローの参照と、規則の列挙を同じプロセスの中で読む\n  ritsu --help | --version\n\n\
         ほかの言語は、いまはそれぞれのコマンドで走らせます（rulec、koyomi、chobo、geas）。\n\
         --lang ja|en でこの画面の言語を選びます（無ければ環境変数 RITSU_LANG、それも無ければ en）。`ritsu dandori`、`ritsu yuen`、`ritsu sakai` の言語は、その言語が選びます。\n\
         exit code: 言語のコマンドのもの / 2 ritsu に無いコマンド\n",
        "ritsu {v} — one toolchain for seven small languages (rulec, dandori, koyomi, chobo, geas, yuen, sakai)\n\n\
         Usage:\n  ritsu dandori <command> ...   dandori's commands, reading the rules a flow uses in the same process\n  ritsu yuen <command> ...      yuen's commands, reading in the same process what requirements name of the other languages, the sources they borrow and the records of claims\n  ritsu sakai <command> ...     sakai's commands, reading in the same process what the rules, calendars and workflows of a map refer to, and the enums of its rules\n  ritsu --help | --version\n\n\
         The other languages run as their own commands for now (rulec, koyomi, chobo, geas).\n\
         --lang ja|en chooses the language of this page (else the RITSU_LANG environment variable, else en); `ritsu dandori`, `ritsu yuen` and `ritsu sakai` let the language choose its own.\n\
         Exit codes: the language's command's / 2 a command ritsu does not have\n"
    );
    t.get(lang).to_string()
}

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

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let lang = Lang::pick(lang_asked(&args).as_deref(), "RITSU_LANG");
    // the command is the first word that is not ritsu's own `--lang`
    let mut at = 0;
    while at < args.len() && (args[at] == "--lang" || args[at].starts_with("--lang=")) {
        at += if args[at] == "--lang" { 2 } else { 1 };
    }
    let Some(first) = args.get(at) else {
        eprint!("{}", help(lang));
        return ExitCode::from(2);
    };
    match first.as_str() {
        "--help" | "-h" | "help" => {
            print!("{}", help(lang));
            ExitCode::SUCCESS
        }
        "--version" | "-V" => {
            println!("ritsu {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        "dandori" => {
            let rules = Rc::new(rulec::ports::Engine::new());
            let (mut out, mut err) = (std::io::stdout(), std::io::stderr());
            let code = dandori::cli::run(&args[at + 1..], rules, &mut out, &mut err);
            let _ = out.flush();
            ExitCode::from(code)
        }
        "yuen" => {
            let (mut out, mut err) = (std::io::stdout(), std::io::stderr());
            let code = yuen::run::run(&args[at + 1..], yuen_suite(), &mut out, &mut err);
            let _ = out.flush();
            ExitCode::from(code)
        }
        "sakai" => {
            let (mut out, mut err) = (std::io::stdout(), std::io::stderr());
            let code = sakai::run::run(&args[at + 1..], sakai_suite(), &mut out, &mut err);
            let _ = out.flush();
            ExitCode::from(code)
        }
        l if ON_THEIR_OWN.contains(&l) => refuse(tr!("`ritsu {l}` はまだありません。`{l}` のコマンドで走らせてください", "there is no `ritsu {l}` yet; run the `{l}` command itself"), lang),
        other => refuse(tr!("`{other}` というコマンドはありません。`ritsu --help` を読んでください", "there is no command `{other}`; run `ritsu --help`"), lang),
    }
}
