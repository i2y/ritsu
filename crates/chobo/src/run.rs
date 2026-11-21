//! The `chobo` command (`crate::run::run`): the binary of this crate runs it, and so does
//! `ritsu chobo` (ritsu's DESIGN 8.2).
//!
//! One table (`commands()`) knows every command and flag. `--help` renders from it and the
//! parser checks against it, so a flag cannot be documented and ignored, or accepted and
//! left undocumented. A flag chobo does not know stops the run with exit 2: dropped quietly,
//! it would let an agent believe its request went through.

use crate::check;
use crate::codes;
use crate::diag::{self, Show};
use ritsu_base::cli::{Cmd, Flag, Reading, Table, flag, help_flag, lang_flag, made_lang_flag};
use ritsu_base::text::{Lang, Text};
use crate::render;
use crate::scenario;
use crate::target::{BOOKS, Backend, LANGS, Lang as Language, RENAMED, Target};
use ritsu_base::tr;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn global_flags() -> Vec<Flag> {
    vec![lang_flag("CHOBO_LANG"), help_flag()]
}

fn every_code() -> Vec<&'static str> {
    codes::ledger().iter().map(|e| e.code).collect()
}

fn commands() -> Vec<Cmd> {
    let book = |many: bool| {
        (
            if many { "<file.book>..." } else { "<file.book>" },
            if many { tr!("検査する帳簿", "the books to check") } else { tr!("帳簿", "the book") },
        )
    };
    let usage_exit = tr!("使い方の誤り、読めないファイル", "a mistake in how the command is called, or a file that cannot be read");
    vec![
        Cmd {
            name: "check",
            usage: Some("chobo check <file.book>... [--format json] [--diff-base <rev>]"),
            args: "<file.book>...",
            purpose: tr!(
                "帳簿を検査する。書き方の誤り、呼ぶと分かること（そうなる例つき）、使われない宣言と効かない境界。そのあと、振替の種類と操作ごとに拒否されうる理由を並べる",
                "check a book: how it is written, what only shows when it is called (with the operations that get there), what is never used and what never matters; then list what each operation of each transfer can be refused with"
            ),
            params: vec![book(true)],
            flags: vec![
                flag("--format", Some("json"), tr!("診断と報告を JSON で出す（例の操作まで入る）", "print the diagnostics and the report as JSON, with the example operations")).choices(&["json"]),
                flag("--diff-base", Some("<rev>"), tr!("git のそのリビジョンの帳簿と比べ、データベースに残るものが変わらないかを確かめる（E050、E051、W107）", "compare with the book at that git revision, for changes the data in a database cannot follow (E050, E051, W107)")),
            ],
            exits: vec![
                (0, tr!("エラーが無い（警告はあってもよい）", "no errors (there may be warnings)")),
                (1, tr!("エラーがある", "errors")),
                (2, usage_exit.clone()),
            ],
            examples: vec!["chobo check inventory.book", "chobo check inventory.book --diff-base HEAD --format json"],
            codes: every_code(),
        },
        Cmd {
            name: "run",
            usage: Some("chobo run <file.book> --scenario <file.json> [--show postgres|tigerbeetle|mysql] [--format json]"),
            args: "<file.book>",
            purpose: tr!(
                "シナリオを参照インタプリタで流し、操作ごとの結果と、終わりの残高と仮押さえを出す。together があれば、とりうる結果を全部出す",
                "run a scenario in the reference interpreter, and print what each operation answered and the balances and holds at the end; with together, every way it can come out"
            ),
            params: vec![book(false)],
            flags: vec![
                flag("--scenario", Some("<file.json>"), tr!("流すシナリオ（一つ、または `chobo scenarios` が出す配列）", "the scenario to run (one, or the list `chobo scenarios` prints)")),
                flag(
                    "--show",
                    Some("postgres|tigerbeetle|mysql"),
                    tr!(
                        "結果の代わりに、クライアントがその出力先に送るものを操作ごとに出す（PostgreSQL では関数と引数、TigerBeetle では作る勘定とチェーン、MySQL の系統では文）",
                        "instead of the results, print what a client sends that target for each operation (the function and its arguments for PostgreSQL; the accounts it makes and the chain for TigerBeetle; the statements for the MySQL family)"
                    ),
                )
                .choices(&["postgres", "tigerbeetle", "mysql"]),
                flag("--format", Some("json"), tr!("結果を JSON で出す", "print the result as JSON")).choices(&["json"]),
            ],
            exits: vec![
                (0, tr!("流せた", "it ran")),
                (1, tr!("帳簿にエラーがある", "the book has errors")),
                (2, tr!("使い方の誤り、読めないファイル、シナリオの誤り", "a mistake in how the command is called, a file that cannot be read, or a mistake in the scenario")),
            ],
            examples: vec!["chobo run inventory.book --scenario 001.json", "chobo run inventory.book --scenario 001.json --show tigerbeetle --format json"],
            codes: vec![],
        },
        Cmd {
            name: "scenarios",
            usage: Some("chobo scenarios <file.book> [--out <dir>]"),
            args: "<file.book>",
            purpose: tr!(
                "帳簿からシナリオを作る。境界の手前・ちょうど・超える、同じキーの二度目、仮押さえの終わり方、移動の途中での拒否、二つの呼び出し元の取り合い",
                "write scenarios for a book: each bound just before, at and past it, each key used twice, every way a hold ends, a refusal partway through the moves, and two callers after the last of something"
            ),
            params: vec![book(false)],
            flags: vec![flag("--out", Some("<dir>"), tr!("001.json から一つずつ書く先。無ければ JSON の配列を標準出力に出す", "the directory to write them into, from 001.json; without it, a JSON list on standard output"))],
            exits: vec![(0, tr!("作れた", "written")), (1, tr!("帳簿にエラーがある", "the book has errors")), (2, usage_exit.clone())],
            examples: vec!["chobo scenarios inventory.book", "chobo scenarios inventory.book --out scenarios/"],
            codes: vec![],
        },
        Cmd {
            name: "gen",
            usage: Some("chobo gen <file.book> --target sql|typescript|python|go|rust... [--books postgres|tigerbeetle|mysql] [--out <dir>] [--check]"),
            args: "<file.book>",
            purpose: tr!(
                "帳簿を、指定した言語のコードにする。SQL は PostgreSQL のスキーマと関数か MySQL の系統のテーブル、TypeScript・Python・Go・Rust は `--books` のデータベース（PostgreSQL、TigerBeetle、MySQL の系統）を呼ぶクライアント",
                "build the book into the code of each language given: SQL is PostgreSQL's schema and functions or the MySQL family's tables, and TypeScript, Python, Go and Rust are clients that call the database of `--books` (PostgreSQL, TigerBeetle or the MySQL family)"
            ),
            params: vec![book(false)],
            flags: vec![
                flag("--target", Some("sql|typescript|python|go|rust"), tr!("書く言語。<out>/<言語>/ に書く", "a language to write, into <out>/<language>/"))
                    .choices(&LANGS)
                    .once_each()
                    .renamed(&RENAMED),
                flag(
                    "--books",
                    Some("postgres|tigerbeetle|mysql"),
                    tr!("帳簿を持つデータベース。SQL はその方言で書き、クライアントはそれを呼ぶ（mysql は MySQL・TiDB・Vitess）", "the database that keeps the books: the SQL is in its dialect, and the clients call it (mysql is MySQL, TiDB or Vitess)"),
                )
                .choices(&BOOKS)
                .default("postgres"),
                flag("--out", Some("<dir>"), tr!("書く先のディレクトリ", "the directory to write into")).default("generated"),
                flag(
                    "--check",
                    None,
                    tr!(
                        "書かずに、同じ --target と --books で書くはずのファイルを、ディスクのものと比べる。無いか違えば 1（CI 用）",
                        "write nothing; exit 1 if a file the same --target and --books write is missing or differs from the one on disk (for CI)"
                    ),
                ),
            ],
            exits: vec![
                (0, tr!("書いた、または --check で全部が同じ", "written, or under --check all the same")),
                (
                    1,
                    tr!(
                        "帳簿にエラーがある、指定した出力先のどれかが帳簿を受け取れない（E060、E061、E062、E063、E064。何も書かない）、または --check で無いか違うファイルがある",
                        "the book has errors, a target given cannot take it (E060, E061, E062, E063, E064; nothing is written), or under --check a file is missing or differs"
                    ),
                ),
                (
                    2,
                    tr!(
                        "使い方の誤り（`--target` が無い、`--target sql` と `--books tigerbeetle`、0.40.0 までの組の名前、も含む）、読めないファイル",
                        "a mistake in how the command is called (no `--target`, `--target sql` with `--books tigerbeetle`, or a pair as 0.40.0 named it, among them), or a file that cannot be read"
                    ),
                ),
            ],
            examples: vec![
                "chobo gen inventory.book --target sql",
                "chobo gen inventory.book --target go --books tigerbeetle --out internal/",
                "chobo gen inventory.book --target sql --target typescript --check",
            ],
            codes: vec!["E060", "E061", "E062", "E063", "E064"],
        },
        Cmd {
            name: "doc",
            usage: Some("chobo doc <file.book> [--format html] [--out <dir>]"),
            args: "<file.book>",
            purpose: tr!(
                "帳簿を、経理や運用の人が読むページにする。勘定と境界、勘定のあいだの流れの図、振替ごとの移動とキーと拒否されうる理由（そうなる例つき）、仮押さえのライフサイクルの図、シナリオとステップごとの残高",
                "write the book as a page for the people who keep the accounts and run the operations: the accounts and their bounds, a chart of how things move between them, each transfer's moves, key and what it can be refused with (with the operations that get there), the life of a hold as a chart, and the scenarios with the balances after each step"
            ),
            params: vec![book(false)],
            flags: vec![
                flag("--format", Some("html"), tr!("Mermaid の図を入れた Markdown の代わりに、一つの HTML を書く（図は chobo が描き、シナリオを一つずつ進めて見られる）", "write one HTML page instead of Markdown with Mermaid charts (chobo draws the charts, and a scenario can be stepped through)")).choices(&["html"]),
                flag("--out", Some("<dir>"), tr!("<ファイル名>.md か <ファイル名>.html を書く先のディレクトリ。無ければ標準出力に出す", "the directory to write <file>.md or <file>.html into; without it, standard output")),
                made_lang_flag("CHOBO_LANG"),
            ],
            exits: vec![(0, tr!("書いた", "written")), (1, tr!("帳簿にエラーがある", "the book has errors")), (2, usage_exit.clone())],
            examples: vec!["chobo doc inventory.book > inventory.md", "chobo doc inventory.book --format html --out site/ --lang ja"],
            codes: vec![],
        },
        Cmd {
            name: "api",
            usage: Some("chobo api <file.book>"),
            args: "<file.book>",
            purpose: tr!(
                "呼び方、操作ごとに拒否されうる理由、仮押さえのステートマシン、ID の決め方を JSON で出す（dandori のようなツールが読む）",
                "print how to call the book, what each operation can be refused with, the life of a hold as a state machine, and how IDs are made, as JSON (for tools such as dandori)"
            ),
            params: vec![book(false)],
            flags: vec![],
            exits: vec![(0, tr!("出せた", "printed")), (1, tr!("帳簿にエラーがある", "the book has errors")), (2, usage_exit.clone())],
            examples: vec!["chobo api inventory.book", "chobo api inventory.book > inventory.api.json"],
            codes: vec![],
        },
        Cmd {
            name: "explain",
            usage: Some("chobo explain <code>... | --all [--format markdown|json]"),
            args: "<code>... | --all",
            purpose: tr!(
                "診断のコードを説明する。いつ出るか、どう直すか、最小の再現",
                "explain diagnostic codes: when each appears, how to fix it, and the smallest book that shows it"
            ),
            params: vec![("<code>...", tr!("E020 のようなコード。いくつでも渡せて、渡した順に説明する。`--all` とは一緒に渡さない", "codes such as E020, any number, explained in the order given; not given with `--all`"))],
            flags: vec![
                flag("--all", None, tr!("全部のコードを出す", "every code")),
                flag("--format", Some("markdown|json"), tr!("Markdown か JSON で出す", "print Markdown or JSON")).choices(&["markdown", "json"]),
            ],
            exits: vec![(0, tr!("出せた", "printed")), (2, tr!("知らないコード、使い方の誤り", "a code chobo does not have, or a mistake in how the command is called"))],
            examples: vec!["chobo explain E020", "chobo explain --all --format markdown --lang ja"],
            codes: vec![],
        },
    ]
}

/// The table every command line is read against and `--help` is drawn from. Only what starts
/// with `--` is a flag, `--format=json` is one word, and a flag's value does not start with `--`.
fn table() -> Table {
    Table {
        tool: "chobo",
        version: env!("CARGO_PKG_VERSION"),
        summary: tr!(
            "在庫、お金、ポイント、予約の枠のように、数で持っていて勘定から勘定へ動かすものの小さな言語",
            "a small language for the things you count and move between accounts: stock, money, points, seats"
        ),
        globals: global_flags(),
        commands: commands(),
        footer: vec![],
        reading: Reading { no_dashes_in_values: true, no_inline_values: true, single_dash_args: true, ..Reading::default() },
    }
}

fn top_help(lang: Lang) -> String {
    let mut o = String::new();
    let t = table();
    o.push_str(&format!("chobo — {}\n\n", t.summary.get(lang)));
    o.push_str(tr!("使い方:\n", "Usage:\n").get(lang));
    let cmds = &t.commands;
    // the commands named the way the person ran this one: `ritsu chobo check …` through `ritsu`
    for c in cmds {
        o.push_str(&format!("  {}\n", ritsu_base::cli::as_run(&t.usage_line(c))));
    }
    o.push_str(tr!("\nコマンド:\n", "\nCommands:\n").get(lang));
    let w = cmds.iter().map(|c| c.name.len()).max().unwrap_or(0);
    for c in cmds {
        o.push_str(&format!("  {:w$}  {}\n", c.name, c.purpose.get(lang)));
    }
    o.push_str(tr!("\nどのコマンドにも付けられるフラグ:\n", "\nFlags for every command:\n").get(lang));
    for f in &t.globals {
        o.push_str(&format!("  {:14} {}\n", f.spelled(), f.help.get(lang)));
    }
    o.push_str(&format!("  {:14} {}\n", "--version", tr!("バージョンを出す", "print the version").get(lang)));
    let run = ritsu_base::cli::command_name("chobo");
    o.push_str(tr!(
        "\nコマンドごとの説明は `{run} <コマンド> --help`。\n終了コード: 0 問題なし（警告だけ）/ 1 エラー / 2 使い方の誤り、読めないファイル\n",
        "\nFor one command: `{run} <command> --help`.\nExit codes: 0 no errors (warnings only) / 1 errors / 2 a mistake in how chobo is called, or a file it cannot read\n"
    )
    .get(lang));
    o
}

fn cmd_help(c: &Cmd, lang: Lang) -> String {
    let t = table();
    let mut o = format!("{}\n\n{}\n", ritsu_base::cli::as_run(&t.usage_line(c)), c.purpose.get(lang));
    o.push_str(tr!("\n引数:\n", "\nArguments:\n").get(lang));
    for (p, h) in &c.params {
        o.push_str(&format!("  {:16} {}\n", p, h.get(lang)));
    }
    o.push_str(tr!("\nフラグ:\n", "\nFlags:\n").get(lang));
    // a flag every command takes, which the command takes in a form of its own (the `--lang` of
    // `doc`, which writes what others keep), is said once, as its own (as ritsu-base's `help_cmd`)
    for f in c.flags.iter().chain(t.globals.iter().filter(|g| !c.flags.iter().any(|f| f.name == g.name))) {
        let mut h = f.help.get(lang).to_string();
        if let Some(d) = &f.default {
            h.push_str(&tr!("（既定: {d}）", " (default: {d})").get(lang).to_string());
        }
        if f.repeats {
            h.push_str(tr!("（何度でも書ける）", " (may be given more than once)").get(lang));
        }
        o.push_str(&format!("  {:24} {}\n", f.spelled(), h));
    }
    o.push_str(tr!("\n終了コード:\n", "\nExit codes:\n").get(lang));
    for (n, h) in &c.exits {
        o.push_str(&format!("  {n}  {}\n", h.get(lang)));
    }
    o.push_str(tr!("\n例:\n", "\nExamples:\n").get(lang));
    for e in &c.examples {
        o.push_str(&format!("  {}\n", ritsu_base::cli::as_run_of("chobo", e)));
    }
    if !c.codes.is_empty() {
        o.push_str(&format!("{}{}\n", tr!("\n出しうる診断: ", "\nDiagnostics it can print: ").get(lang), c.codes.join(" ")));
    }
    o
}

struct Args {
    cmd: String,
    positional: Vec<String>,
    flags: BTreeMap<String, Option<String>>,
    /// Every value of each flag, in the order given: `--target` may be given more than once.
    all: Vec<(String, String)>,
    lang: Lang,
}

impl Args {
    /// The language of what the command writes for others to keep (a page): `--lang`, else English,
    /// whatever the environment says (ritsu's DESIGN 4.1); `lang` is what the run prints.
    fn made(&self) -> Lang {
        Lang::made(self.flags.get("--lang").and_then(|v| v.as_deref()))
    }

    fn all(&self, flag: &str) -> Vec<&str> {
        self.all.iter().filter(|(k, _)| k == flag).map(|(_, v)| v.as_str()).collect()
    }
}

/// What a parse ends in: the arguments, or something to print and the exit code.
enum Parsed {
    Run(Args),
    Print(String, u8, bool),
    /// What stops the line, said in the words of every command of ritsu ([`refuse`]).
    Stop(Text, Lang),
}

/// What stops the command, as every command of ritsu says it (ritsu's DESIGN 6.2, item 10), and 2.
fn refuse(msg: Text, lang: Lang) -> u8 {
    ritsu_base::cli::refuse(&mut std::io::stderr(), &msg, lang)
}

/// What stops a command given other books than it takes, or none: what it takes, and its usage line.
fn usage(a: &Args) -> u8 {
    refuse(table().takes_of(&a.cmd), a.lang)
}

fn parse(argv: Vec<String>) -> Parsed {
    // the language first, so that every message, the help among them, is in it
    let lang_flag = argv.windows(2).find(|w| w[0] == "--lang").map(|w| w[1].clone());
    let lang = Lang::pick(lang_flag.as_deref(), "CHOBO_LANG");
    let Some(cmd) = argv.first().cloned() else {
        return Parsed::Print(top_help(lang), 2, true);
    };
    match cmd.as_str() {
        "--help" | "-h" => return Parsed::Print(top_help(lang), 0, false),
        "help" => return help_of(&argv[1..], lang),
        "--version" | "-V" => return Parsed::Print(format!("chobo {}\n", env!("CARGO_PKG_VERSION")), 0, false),
        _ => {}
    }
    let t = table();
    let Some(c) = t.command(&cmd) else {
        return no_command(&cmd, lang);
    };
    // `--help` ends the line: what comes before it is read, and what comes after it is not
    let rest = &argv[1..];
    let until = rest.iter().position(|x| x == "--help" || x == "-h").unwrap_or(rest.len());
    let got = match t.read(c, &rest[..until]) {
        Ok(g) => g,
        Err(m) => {
            let names: Vec<&str> = c.flags.iter().chain(t.globals.iter()).map(|f| f.name).collect();
            return Parsed::Stop(m.text_among(&ritsu_base::cli::command_name("chobo"), &cmd, &names), lang);
        }
    };
    if until < rest.len() {
        return Parsed::Print(cmd_help(c, lang), 0, false);
    }
    let all = got.got.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
    let mut a = Args { cmd: cmd.clone(), positional: got.pos.clone(), flags: BTreeMap::new(), all, lang };
    for (k, v) in &got.got {
        let takes = c.flags.iter().chain(t.globals.iter()).find(|f| f.name == *k).is_some_and(|f| f.value.is_some());
        a.flags.insert(k.to_string(), takes.then(|| v.clone()));
    }
    Parsed::Run(a)
}


/// `chobo help [<command>]`: the page of the command named, or of them all, as dandori and sakai
/// read it; `--lang` is not the command named.
fn help_of(words: &[String], lang: Lang) -> Parsed {
    let mut rest = words.iter();
    let mut asked = None;
    while let Some(w) = rest.next() {
        if w == "--lang" {
            rest.next();
        } else if !w.starts_with("--lang=") {
            asked = Some(w.as_str());
            break;
        }
    }
    let Some(name) = asked else {
        return Parsed::Print(top_help(lang), 0, false);
    };
    let t = table();
    match t.command(name) {
        Some(c) => Parsed::Print(cmd_help(c, lang), 0, false),
        None => no_command(name, lang),
    }
}

/// A word that is no command of chobo's: the verb up to 0.40.0 is not read as the new one (ritsu's
/// DESIGN 9.6), and says it.
fn no_command(cmd: &str, lang: Lang) -> Parsed {
    if cmd == "build" {
        return Parsed::Stop(ritsu_base::cli::renamed("chobo build", "chobo gen", "0.41.0"), lang);
    }
    Parsed::Stop(ritsu_base::cli::no_command("chobo", cmd), lang)
}

/// The `chobo` command, on `argv` (the words after the program's name), printing to standard
/// output and standard error: the exit code. A line that asks for JSON is answered in JSON when the
/// command stops before its work too (ritsu's DESIGN 6.2, item 10): `check` puts `error` in its
/// document, and every other command writes `{"error": …}` as the last value on standard output.
pub fn run(argv: Vec<String>) -> ExitCode {
    let line = argv.clone();
    ritsu_base::cli::answering(&line, &mut std::io::stdout(), |_| run_line(argv))
}

fn run_line(argv: Vec<String>) -> ExitCode {
    let a = match parse(argv) {
        Parsed::Run(a) => a,
        Parsed::Print(text, code, to_err) => {
            if to_err {
                eprint!("{text}");
            } else {
                print!("{text}");
            }
            return ExitCode::from(code);
        }
        Parsed::Stop(t, lang) => return ExitCode::from(refuse(t, lang)),
    };
    // a path that starts with `@`, in a module, is read as a path written in a file is (ritsu's DESIGN 17.6)
    if let Err(t) = ritsu_base::modules::given_all(&a.positional) {
        return ExitCode::from(refuse(t, a.lang));
    }
    // the paths the run holds are written the way the first path given was (ritsu's DESIGN 4.15),
    // and so are the books given: `./stock.book` and `x/../stock.book` are `stock.book` (6.2 item
    // 8); the code `explain` takes is no path
    let _way = ritsu_base::paths::run_from(a.positional.first().filter(|_| a.cmd != "explain").map_or(".", String::as_str));
    let mut a = a;
    if a.cmd != "explain" {
        for p in &mut a.positional {
            *p = ritsu_base::paths::show_given(p);
        }
    }
    // and so are the paths the flags take: `--out ./generated` is `generated`
    for flag in ["--out", "--scenario"] {
        if let Some(Some(v)) = a.flags.get_mut(flag) {
            *v = ritsu_base::paths::fold_given(v);
        }
    }
    for (k, v) in &mut a.all {
        if k == "--out" || k == "--scenario" {
            *v = ritsu_base::paths::fold_given(v);
        }
    }
    let code = match a.cmd.as_str() {
        "check" => cmd_check(&a),
        "run" => cmd_run(&a),
        "scenarios" => cmd_scenarios(&a),
        "gen" => cmd_gen(&a),
        "doc" => cmd_doc(&a),
        "api" => cmd_api(&a),
        "explain" => cmd_explain(&a),
        _ => 2,
    };
    ExitCode::from(code)
}

fn one_book(a: &Args) -> Option<PathBuf> {
    match a.positional.as_slice() {
        [f] => Some(PathBuf::from(f)),
        _ => None,
    }
}

/// Check one book for a command that needs it whole: its diagnostics go to standard error,
/// and with any error there is nothing else to do.
fn load(path: &Path, lang: Lang) -> Result<(String, crate::model::Book, check::Checked), u8> {
    let (src, c) = match check::check_file(path) {
        Ok(x) => x,
        Err(e) => return Err(refuse(ritsu_base::cli::cannot_read(&path.display().to_string(), &e), lang)),
    };
    let file = path.display().to_string();
    for d in &c.diags {
        eprint!("{}", d.shown(&file, &src, lang));
    }
    if diag::has_errors(&c.diags) {
        eprintln!("{}", diag::summary(&file, &c.diags, lang));
        return Err(1);
    }
    let book = c.book.clone().unwrap();
    Ok((src, book, c))
}

fn cmd_check(a: &Args) -> u8 {
    if a.positional.is_empty() {
        return usage(a);
    }
    let json_out = a.flags.contains_key("--format");
    let rev = a.flags.get("--diff-base").cloned().flatten();
    let mut worst = 0u8;
    let mut files = Vec::new();
    // The books given before each, for what makes them one in a target (W108, DESIGN 18).
    let mut earlier = Vec::new();
    for f in &a.positional {
        let path = Path::new(f);
        let (src, mut c) = match check::check_file(path) {
            Ok(x) => x,
            Err(e) => {
                worst = refuse(ritsu_base::cli::cannot_read(f, &e), a.lang);
                continue;
            }
        };
        check::compare_across(f, &mut c, &mut earlier);
        let mut note = None;
        if let (Some(rev), Some(book)) = (&rev, &c.book) {
            match crate::diffbase::read_at(path, rev) {
                Ok(before) => {
                    let (more, n) = crate::diffbase::against(book, before.as_deref(), rev);
                    c.diags.extend(more);
                    crate::model::sort(&mut c.diags);
                    if diag::has_errors(&c.diags) {
                        c.report = None;
                    }
                    note = n;
                }
                Err(e) => return refuse(e, a.lang),
            }
        }
        if diag::has_errors(&c.diags) {
            worst = worst.max(1);
        }
        if json_out {
            let mut v = check::to_json(f, &src, &c, a.lang);
            if let Some(n) = &note {
                v["note"] = json!(n.get(a.lang));
            }
            files.push(v);
        } else {
            if let Some(n) = &note {
                println!("{}: {}", f, n.get(a.lang));
            }
            print!("{}", check::render(f, &src, &c, a.lang));
        }
    }
    if json_out {
        // 2: the diagnostics have ritsu's keys (`col`, `message`, `fix`) since chobo moved onto
        // ritsu-base; 1 had `v`, `column` and `title` in each (DESIGN 8.1)
        let mut doc = json!({"v": 2, "files": files});
        // a book it could not read: the document says so as every command's JSON does (ritsu's
        // DESIGN 6.2, item 10), beside the books it read
        if let Some(said) = ritsu_base::cli::take_refusal() {
            doc["error"] = json!({"code": 2, "message": said});
        }
        println!("{}", serde_json::to_string_pretty(&doc).unwrap());
    }
    worst
}

fn cmd_run(a: &Args) -> u8 {
    let Some(path) = one_book(a) else { return usage(a) };
    let Some(Some(sc)) = a.flags.get("--scenario") else {
        return refuse(table().needs_of("run", "--scenario <file.json>"), a.lang);
    };
    let (_, book, _) = match load(&path, a.lang) {
        Ok(x) => x,
        Err(c) => return c,
    };
    let v: Value = match ritsu_base::fs::read_to_string(sc).map_err(|e| ritsu_base::cli::cannot_read(sc, &e)).and_then(|t| serde_json::from_str(&t).map_err(|e| ritsu_base::cli::not_json(sc, &e.to_string()))) {
        Ok(v) => v,
        Err(e) => return refuse(e, a.lang),
    };
    let list: Vec<Value> = match v {
        Value::Array(xs) => xs,
        other => vec![other],
    };
    let file = path.display().to_string();
    let show = a.flags.get("--show").cloned().flatten();
    let mut outs = Vec::new();
    let mut text = String::new();
    for (i, x) in list.iter().enumerate() {
        // what is wrong with a scenario, after the file and, of a list of them, which one
        let wrong = |e: Text| match list.len() > 1 {
            true => {
                let n = i + 1;
                tr!("`{sc}`: シナリオ {n}: {}", "`{sc}`: scenario {n}: {}", e.ja; e.en)
            }
            false => tr!("`{sc}`: {}", "`{sc}`: {}", e.ja; e.en),
        };
        let s = match scenario::from_json(&book, x) {
            Ok(s) => s,
            Err(e) => return refuse(wrong(e), a.lang),
        };
        let runs = match scenario::run(&book, &s) {
            Ok(r) => r,
            Err(e) => return refuse(wrong(e), a.lang),
        };
        if let Some(target) = &show {
            if a.flags.contains_key("--format") {
                let ops = match target.as_str() {
                    "postgres" => render::postgres_json(&book, "", &s),
                    "mysql" => render::mysql_json(&book, "", &s),
                    _ => render::tigerbeetle_json(&book, "", &s),
                };
                outs.push(json!({"scenario": s.name, "target": target, "tenant": "", "operations": ops}));
            } else if target == "postgres" {
                text.push_str(&render::postgres_text(&book, &file, "", &s, a.lang));
            } else if target == "mysql" {
                text.push_str(&render::mysql_text(&book, &file, "", &s, a.lang));
            } else {
                text.push_str(&render::tigerbeetle_text(&book, &file, "", &s, a.lang));
            }
            continue;
        }
        if a.flags.contains_key("--format") {
            match scenario::run_json(&book, &s) {
                Ok(r) => outs.push(r),
                Err(e) => return refuse(wrong(e), a.lang),
            }
        } else {
            text.push_str(&scenario::render(&book, &file, &s, &runs, a.lang));
        }
    }
    if a.flags.contains_key("--format") {
        let out = if list.len() == 1 && outs.len() == 1 { outs.pop().unwrap() } else { Value::Array(outs) };
        println!("{}", serde_json::to_string_pretty(&out).unwrap());
    } else {
        print!("{text}");
    }
    0
}

fn cmd_scenarios(a: &Args) -> u8 {
    let Some(path) = one_book(a) else { return usage(a) };
    let (_, book, _) = match load(&path, a.lang) {
        Ok(x) => x,
        Err(c) => return c,
    };
    let list: Vec<Value> = crate::scenarios::generate(&book).iter().map(|s| scenario::to_json(&book, s)).collect();
    match a.flags.get("--out").cloned().flatten() {
        Some(dir) => {
            let dir = PathBuf::from(dir);
            if let Err(e) = ritsu_base::fs::create_dir_all(&dir) {
                return refuse(ritsu_base::cli::cannot_create(&dir.display().to_string(), &e), a.lang);
            }
            for (i, s) in list.iter().enumerate() {
                let p = dir.join(format!("{:03}.json", i + 1));
                if let Err(e) = ritsu_base::fs::write(&p, serde_json::to_string_pretty(s).unwrap() + "\n") {
                    return refuse(ritsu_base::cli::cannot_write(&p.display().to_string(), &e), a.lang);
                }
            }
            let (n, d) = (list.len(), dir.display());
            eprintln!("{}", tr!("{d} にシナリオを {n} 本書きました", "{n} scenario(s) written to {d}").get(a.lang));
        }
        None => println!("{}", serde_json::to_string_pretty(&Value::Array(list)).unwrap()),
    }
    0
}

/// `chobo gen` (DESIGN 17): the languages given, each for the database of `--books`, into
/// `<out>/<language>/`. Every target is built before anything is written: one that cannot take
/// the book stops the run, and nothing is written.
fn cmd_gen(a: &Args) -> u8 {
    let Some(path) = one_book(a) else { return usage(a) };
    let given = a.all("--target");
    if given.is_empty() {
        return refuse(table().needs_of("gen", "--target sql|typescript|python|go|rust"), a.lang);
    }
    let books = Backend::parse(a.flags.get("--books").cloned().flatten().as_deref().unwrap_or("postgres")).expect("the choices of --books are the databases");
    // in the order of the languages, not of the flags
    let langs: Vec<Language> = Language::ALL.into_iter().filter(|l| given.contains(&l.key())).collect();
    let mut targets = Vec::new();
    for l in langs {
        match Target::of(l, books) {
            Some(t) => targets.push(t),
            None => {
                let msg = tr!(
                    "`--books tigerbeetle` には `--target sql` がありません。TigerBeetle は SQL を受け取りません。クライアントは `--target typescript|python|go|rust` で書きます",
                    "there is no `--target sql` for `--books tigerbeetle`: TigerBeetle takes no SQL; its clients are `--target typescript|python|go|rust`"
                );
                return refuse(msg, a.lang);
            }
        }
    }
    let (src, book, c) = match load(&path, a.lang) {
        Ok(x) => x,
        Err(c) => return c,
    };
    let stem = path.file_name().map(|f| f.to_string_lossy().trim_end_matches(".book").to_string()).unwrap_or_default();
    let file = path.display().to_string();
    let origin = crate::target::Origin::named(&file, src.as_bytes());
    let out = PathBuf::from(a.flags.get("--out").cloned().flatten().unwrap_or_else(|| "generated".into()));
    let mut planned: Vec<(PathBuf, String)> = Vec::new();
    // what stops the targets, each once: E060, E061 and E062 are of the database, which every
    // language given is built for
    let mut stops: Vec<crate::diag::Diag> = Vec::new();
    for t in &targets {
        match crate::target::build(&book, &stem, *t, &origin) {
            Ok(files) => planned.extend(files.into_iter().map(|(rel, text)| (out.join(t.lang().key()).join(rel), text))),
            Err(diags) => {
                for d in diags {
                    let same = |s: &crate::diag::Diag| (s.code, s.line, s.col, &s.message.en, &s.notes.iter().map(|n| &n.en).collect::<Vec<_>>()) == (d.code, d.line, d.col, &d.message.en, &d.notes.iter().map(|n| &n.en).collect::<Vec<_>>());
                    if !stops.iter().any(same) {
                        stops.push(d);
                    }
                }
            }
        }
    }
    if !stops.is_empty() {
        for d in &stops {
            eprint!("{}", d.shown(&file, &src, a.lang));
        }
        // the summary counts the warnings `load` printed above as well
        let mut all = c.diags.clone();
        all.extend(stops);
        eprintln!("{}", diag::summary(&file, &all, a.lang));
        return 1;
    }
    if a.flags.contains_key("--check") {
        let mut differs = false;
        for (p, text) in &planned {
            let shown = p.display();
            match std::fs::read(p) {
                Ok(on_disk) if on_disk == text.as_bytes() => {}
                Ok(_) => {
                    eprintln!("{}", tr!("生成物が古いか手で編集されています: {shown}", "generated file is stale or hand-edited: {shown}").get(a.lang));
                    differs = true;
                }
                Err(_) => {
                    eprintln!("{}", tr!("ありません: {shown}", "missing: {shown}").get(a.lang));
                    differs = true;
                }
            }
        }
        return u8::from(differs);
    }
    for (p, text) in &planned {
        if let Some(dir) = p.parent()
            && let Err(e) = ritsu_base::fs::create_dir_all(dir)
        {
            return refuse(ritsu_base::cli::cannot_create(&dir.display().to_string(), &e), a.lang);
        }
        if let Err(e) = ritsu_base::fs::write(p, text) {
            return refuse(ritsu_base::cli::cannot_write(&p.display().to_string(), &e), a.lang);
        }
        let shown = p.display();
        eprintln!("{}", tr!("{shown} を書きました", "wrote {shown}").get(a.lang));
    }
    0
}

fn cmd_doc(a: &Args) -> u8 {
    let Some(path) = one_book(a) else { return usage(a) };
    let (src, book, c) = match load(&path, a.lang) {
        Ok(x) => x,
        Err(c) => return c,
    };
    let rep = c.report.unwrap_or_else(|| check::report(&book));
    let file = path.display().to_string();
    let input = crate::doc::Input { book: &book, file: &file, src: &src, diags: &c.diags, report: &rep, lang: a.made() };
    let html = a.flags.contains_key("--format");
    let page = if html { crate::doc::html(&input) } else { crate::doc::markdown(&input) };
    match a.flags.get("--out").cloned().flatten() {
        Some(dir) => {
            let stem = path.file_name().map(|f| f.to_string_lossy().trim_end_matches(".book").to_string()).unwrap_or_default();
            let p = PathBuf::from(dir).join(format!("{stem}.{}", if html { "html" } else { "md" }));
            if let Some(dir) = p.parent()
                && let Err(e) = ritsu_base::fs::create_dir_all(dir)
            {
                return refuse(ritsu_base::cli::cannot_create(&dir.display().to_string(), &e), a.lang);
            }
            if let Err(e) = ritsu_base::fs::write(&p, &page) {
                return refuse(ritsu_base::cli::cannot_write(&p.display().to_string(), &e), a.lang);
            }
            let shown = p.display();
            eprintln!("{}", tr!("{shown} を書きました", "wrote {shown}").get(a.lang));
        }
        None => print!("{page}"),
    }
    0
}

fn cmd_api(a: &Args) -> u8 {
    let Some(path) = one_book(a) else { return usage(a) };
    let (src, book, c) = match load(&path, a.lang) {
        Ok(x) => x,
        Err(c) => return c,
    };
    let rep = c.report.unwrap_or_else(|| check::report(&book));
    let stem = path.file_name().map(|f| f.to_string_lossy().trim_end_matches(".book").to_string()).unwrap_or_default();
    println!("{}", serde_json::to_string_pretty(&crate::api::api(&book, &src, &rep, &stem)).unwrap());
    0
}

fn cmd_explain(a: &Args) -> u8 {
    let format = a.flags.get("--format").cloned().flatten();
    let (md, json) = (format.as_deref() == Some("markdown"), format.as_deref() == Some("json"));
    if a.flags.contains_key("--all") {
        if !a.positional.is_empty() {
            return usage(a);
        }
        print!("{}", if json { codes::json_of(&codes::ledger(), true, a.lang) } else if md { codes::markdown_all(a.lang) } else { codes::text_all(a.lang) });
        return 0;
    }
    if a.positional.is_empty() {
        return usage(a);
    }
    // every code it does not have, named in one sentence before anything is printed
    let unknown: Vec<String> = a.positional.iter().filter(|c| codes::find(c).is_none()).cloned().collect();
    if !unknown.is_empty() {
        return refuse(ritsu_base::cli::no_codes("chobo", &unknown), a.lang);
    }
    // one code as it is written alone; several in the order given, as `--all` writes every code
    // (in Markdown, each code's section without the page's head)
    let entries: Vec<codes::Entry> = a.positional.iter().filter_map(|c| codes::find(c)).collect();
    if json {
        print!("{}", codes::json_of(&entries, false, a.lang));
        return 0;
    }
    let shown: Vec<String> = entries.iter().map(|e| if md { codes::render_markdown(e, a.lang) } else { codes::render_text(e, a.lang) }).collect();
    print!("{}", shown.join("\n"));
    0
}
