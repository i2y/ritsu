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
use ritsu_base::cli::{Cmd, Flag, Misuse, Reading, Table, flag, help_flag, lang_flag};
use ritsu_base::text::Lang;
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
            args: "",
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
            args: "",
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
            args: "",
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
            args: "",
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
            args: "",
            purpose: tr!(
                "帳簿を、経理や運用の人が読むページにする。勘定と境界、勘定のあいだの流れの図、振替ごとの移動とキーと拒否されうる理由（そうなる例つき）、仮押さえのライフサイクルの図、シナリオとステップごとの残高",
                "write the book as a page for the people who keep the accounts and run the operations: the accounts and their bounds, a chart of how things move between them, each transfer's moves, key and what it can be refused with (with the operations that get there), the life of a hold as a chart, and the scenarios with the balances after each step"
            ),
            params: vec![book(false)],
            flags: vec![
                flag("--format", Some("html"), tr!("Mermaid の図を入れた Markdown の代わりに、一つの HTML を書く（図は chobo が描き、シナリオを一つずつ進めて見られる）", "write one HTML page instead of Markdown with Mermaid charts (chobo draws the charts, and a scenario can be stepped through)")).choices(&["html"]),
                flag("--out", Some("<dir>"), tr!("<ファイル名>.md か <ファイル名>.html を書く先のディレクトリ。無ければ標準出力に出す", "the directory to write <file>.md or <file>.html into; without it, standard output")),
            ],
            exits: vec![(0, tr!("書いた", "written")), (1, tr!("帳簿にエラーがある", "the book has errors")), (2, usage_exit.clone())],
            examples: vec!["chobo doc inventory.book > inventory.md", "chobo doc inventory.book --format html --out site/ --lang ja"],
            codes: vec![],
        },
        Cmd {
            name: "api",
            usage: Some("chobo api <file.book>"),
            args: "",
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
            usage: Some("chobo explain <code> | --all [--format markdown]"),
            args: "",
            purpose: tr!(
                "診断のコードを説明する。いつ出るか、どう直すか、最小の再現",
                "explain a diagnostic code: when it appears, how to fix it, and the smallest book that shows it"
            ),
            params: vec![("<code>", tr!("E020 のようなコード", "a code such as E020"))],
            flags: vec![
                flag("--all", None, tr!("全部のコードを出す", "every code")),
                flag("--format", Some("markdown"), tr!("Markdown で出す", "print Markdown")).choices(&["markdown"]),
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
    for c in cmds {
        o.push_str(&format!("  {}\n", t.usage_line(c)));
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
    o.push_str(tr!(
        "\nコマンドごとの説明は `chobo <コマンド> --help`。\n終了コード: 0 問題なし（警告だけ）/ 1 エラー / 2 使い方の誤り、読めないファイル\n",
        "\nFor one command: `chobo <command> --help`.\nExit codes: 0 no errors (warnings only) / 1 errors / 2 a mistake in how chobo is called, or a file it cannot read\n"
    )
    .get(lang));
    o
}

fn cmd_help(c: &Cmd, lang: Lang) -> String {
    let t = table();
    let mut o = format!("{}\n\n{}\n", t.usage_line(c), c.purpose.get(lang));
    o.push_str(tr!("\n引数:\n", "\nArguments:\n").get(lang));
    for (p, h) in &c.params {
        o.push_str(&format!("  {:16} {}\n", p, h.get(lang)));
    }
    o.push_str(tr!("\nフラグ:\n", "\nFlags:\n").get(lang));
    for f in c.flags.iter().chain(t.globals.iter()) {
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
        o.push_str(&format!("  {e}\n"));
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
    fn all(&self, flag: &str) -> Vec<&str> {
        self.all.iter().filter(|(k, _)| k == flag).map(|(_, v)| v.as_str()).collect()
    }
}

/// What a parse ends in: the arguments, or something to print and the exit code.
enum Parsed {
    Run(Args),
    Print(String, u8, bool),
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
        Err(m) => return Parsed::Print(format!("{}: {}", stop_head(lang), misuse(&m, &cmd, c, lang)), 2, true),
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

/// The head of a line that stops a command, as every language's command writes it (DESIGN 19).
fn stop_head(lang: Lang) -> &'static str {
    if lang == Lang::Ja { "エラー" } else { "error" }
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
    let head = stop_head(lang);
    if cmd == "build" {
        return Parsed::Print(format!("{head}: {}\n", ritsu_base::cli::renamed("chobo build", "chobo gen", "0.41.0").get(lang)), 2, true);
    }
    let msg = tr!("知らないコマンド `{cmd}` です\n\n", "unknown command `{cmd}`\n\n");
    Parsed::Print(format!("{head}: {}{}", msg.get(lang), top_help(lang)), 2, true)
}

/// What stopped a command line, in chobo's words; a flag it does not know is followed by the
/// command's help.
fn misuse(m: &Misuse, cmd: &str, c: &Cmd, lang: Lang) -> String {
    match m {
        Misuse::UnknownFlag(x) => tr!("`chobo {cmd}` に `{x}` というフラグはありません\n\n", "`chobo {cmd}` has no flag `{x}`\n\n").get(lang).to_string() + &cmd_help(c, lang),
        Misuse::TakesNoValue { flag, value } => {
            let x = format!("{flag}={value}");
            tr!("`chobo {cmd}` に `{x}` というフラグはありません\n\n", "`chobo {cmd}` has no flag `{x}`\n\n").get(lang).to_string() + &cmd_help(c, lang)
        }
        Misuse::Twice(x) => tr!("`{x}` が二度あります\n", "`{x}` is given twice\n").get(lang).to_string(),
        Misuse::SameTwice { flag: x, value: v } => tr!("`{x} {v}` が二度あります\n", "`{x} {v}` is given twice\n").get(lang).to_string(),
        Misuse::Renamed { .. } => format!("{}\n", m.text("chobo", cmd).get(lang)),
        Misuse::NotAChoice { flag: x, value: v, choices } => {
            let ch = choices.join(", ");
            tr!("`{x}` に渡せるのは {ch} です（`{v}` ではなく）\n", "`{x}` takes {ch}, not `{v}`\n").get(lang).to_string()
        }
        Misuse::MissingValue { flag: x, placeholder: ph } => tr!("`{x}` には値 {ph} が要ります\n", "`{x}` needs a value: {ph}\n").get(lang).to_string(),
    }
}

/// The `chobo` command, on `argv` (the words after the program's name), printing to standard
/// output and standard error: the exit code.
pub fn run(argv: Vec<String>) -> ExitCode {
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
    };
    // a path that starts with `@`, in a module, is read as a path written in a file is (ritsu's DESIGN 17.6)
    if let Err(t) = ritsu_base::modules::given_all(&a.positional) {
        let head = if a.lang == Lang::Ja { "エラー" } else { "error" };
        eprintln!("{head}: {}", t.get(a.lang));
        return ExitCode::from(2);
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

fn usage(a: &Args) -> u8 {
    let t = table();
    if let Some(c) = t.command(&a.cmd) {
        eprintln!("{}", t.usage_line(c));
    }
    2
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
        Err(e) => {
            eprintln!("{}: {e}", stop_head(lang));
            return Err(2);
        }
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
    for f in &a.positional {
        let path = Path::new(f);
        let (src, mut c) = match check::check_file(path) {
            Ok(x) => x,
            Err(e) => {
                eprintln!("{}: {e}", stop_head(a.lang));
                worst = 2;
                continue;
            }
        };
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
                Err(e) => {
                    eprintln!("{}: {}", stop_head(a.lang), e.get(a.lang));
                    return 2;
                }
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
        println!("{}", serde_json::to_string_pretty(&json!({"v": 2, "files": files})).unwrap());
    }
    worst
}

fn cmd_run(a: &Args) -> u8 {
    let Some(path) = one_book(a) else { return usage(a) };
    let Some(Some(sc)) = a.flags.get("--scenario") else {
        eprintln!("{}: {}", stop_head(a.lang), tr!("`--scenario <file.json>` が要ります", "`--scenario <file.json>` is required").get(a.lang));
        return 2;
    };
    let (_, book, _) = match load(&path, a.lang) {
        Ok(x) => x,
        Err(c) => return c,
    };
    let v: Value = match ritsu_base::fs::read_to_string(sc).map_err(|e| e.to_string()).and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string())) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{}: {sc}: {e}", stop_head(a.lang));
            return 2;
        }
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
        let s = match scenario::from_json(&book, x) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{}: {sc}: {}{e}", stop_head(a.lang), if list.len() > 1 { format!("scenario {}: ", i + 1) } else { String::new() });
                return 2;
            }
        };
        let runs = match scenario::run(&book, &s) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{}: {sc}: {e}", stop_head(a.lang));
                return 2;
            }
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
                Err(e) => {
                    eprintln!("{}: {sc}: {e}", stop_head(a.lang));
                    return 2;
                }
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
            if let Err(e) = std::fs::create_dir_all(&dir) {
                eprintln!("{}: {}: {e}", stop_head(a.lang), dir.display());
                return 2;
            }
            for (i, s) in list.iter().enumerate() {
                let p = dir.join(format!("{:03}.json", i + 1));
                if let Err(e) = std::fs::write(&p, serde_json::to_string_pretty(s).unwrap() + "\n") {
                    eprintln!("{}: {}: {e}", stop_head(a.lang), p.display());
                    return 2;
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
    let head = if a.lang == Lang::Ja { "エラー" } else { "error" };
    let given = a.all("--target");
    if given.is_empty() {
        let msg = tr!(
            "`chobo gen` には `--target sql|typescript|python|go|rust` が一つ以上要ります。どのデータベースのためのものかは `--books postgres|tigerbeetle|mysql` で指定します（無ければ postgres）",
            "`chobo gen` needs one or more `--target sql|typescript|python|go|rust`; `--books postgres|tigerbeetle|mysql` says which database they are for (postgres if it is not given)"
        );
        eprintln!("{head}: {}", msg.get(a.lang));
        return 2;
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
                eprintln!("{head}: {}", msg.get(a.lang));
                return 2;
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
        if let Err(e) = p.parent().map(std::fs::create_dir_all).unwrap_or(Ok(())).and_then(|_| std::fs::write(p, text)) {
            eprintln!("{}: {}: {e}", stop_head(a.lang), p.display());
            return 2;
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
    let input = crate::doc::Input { book: &book, file: &file, src: &src, diags: &c.diags, report: &rep, lang: a.lang };
    let html = a.flags.contains_key("--format");
    let page = if html { crate::doc::html(&input) } else { crate::doc::markdown(&input) };
    match a.flags.get("--out").cloned().flatten() {
        Some(dir) => {
            let stem = path.file_name().map(|f| f.to_string_lossy().trim_end_matches(".book").to_string()).unwrap_or_default();
            let p = PathBuf::from(dir).join(format!("{stem}.{}", if html { "html" } else { "md" }));
            if let Err(e) = p.parent().map(std::fs::create_dir_all).unwrap_or(Ok(())).and_then(|_| std::fs::write(&p, &page)) {
                eprintln!("{}: {}: {e}", stop_head(a.lang), p.display());
                return 2;
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
    let md = a.flags.contains_key("--format");
    if a.flags.contains_key("--all") {
        if !a.positional.is_empty() {
            return usage(a);
        }
        print!("{}", if md { codes::markdown_all(a.lang) } else { codes::text_all(a.lang) });
        return 0;
    }
    let [code] = a.positional.as_slice() else { return usage(a) };
    match codes::find(code) {
        Some(e) => {
            print!("{}", if md { codes::render_markdown(&e, a.lang) } else { codes::render_text(&e, a.lang) });
            0
        }
        None => {
            eprintln!("{}: {}", stop_head(a.lang), tr!("`{code}` というコードはありません。一覧は `chobo explain --all` で出ます", "there is no code `{code}`; `chobo explain --all` lists them").get(a.lang));
            2
        }
    }
}
