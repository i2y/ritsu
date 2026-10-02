//! The `chobo` command.
//!
//! One table (`commands()`) knows every command and flag. `--help` renders from it and the
//! parser checks against it, so a flag cannot be documented and ignored, or accepted and
//! left undocumented. A flag chobo does not know stops the run with exit 2: dropped quietly,
//! it would let an agent believe its request went through.

use chobo::check;
use chobo::codes;
use chobo::diag::{self, Lang, Text};
use chobo::render;
use chobo::scenario;
use chobo::target::Target;
use chobo::tr;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

struct Flag {
    name: &'static str,
    /// the placeholder for its value; None for a flag without one
    value: Option<&'static str>,
    /// the only values it takes, when they are few
    choices: &'static [&'static str],
    default: Option<&'static str>,
    help: Text,
}

fn flag(name: &'static str, value: Option<&'static str>, help: Text) -> Flag {
    Flag { name, value, choices: &[], default: None, help }
}

impl Flag {
    fn choices(mut self, c: &'static [&'static str]) -> Flag {
        self.choices = c;
        self
    }
    fn default(mut self, d: &'static str) -> Flag {
        self.default = Some(d);
        self
    }
    fn spelled(&self) -> String {
        match self.value {
            Some(v) => format!("{} {v}", self.name),
            None => self.name.to_string(),
        }
    }
}

struct Cmd {
    name: &'static str,
    usage: &'static str,
    purpose: Text,
    params: Vec<(&'static str, Text)>,
    flags: Vec<Flag>,
    exits: Vec<(u8, Text)>,
    examples: [&'static str; 2],
    codes: Vec<&'static str>,
}

fn global_flags() -> Vec<Flag> {
    vec![
        flag("--lang", Some("ja|en"), tr!("文面の言語。無ければ環境変数 CHOBO_LANG、それも無ければ en", "the language of the messages; else the CHOBO_LANG environment variable, else en"))
            .choices(&["ja", "en"])
            .default("en"),
        flag("--help", None, tr!("この画面を出す", "print this page")),
    ]
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
            usage: "chobo check <file.book>... [--format json] [--diff-base <rev>]",
            purpose: tr!(
                "帳簿を検査する。書き方の誤り、呼ぶと分かること（そうなる例つき）、使われない宣言と効かない境界。そのあと、振替の種類と操作ごとに断られうる理由を並べる",
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
            examples: ["chobo check inventory.book", "chobo check inventory.book --diff-base HEAD --format json"],
            codes: every_code(),
        },
        Cmd {
            name: "run",
            usage: "chobo run <file.book> --scenario <file.json> [--show postgres|tigerbeetle] [--format json]",
            purpose: tr!(
                "シナリオを参照インタプリタで流し、操作ごとの結果と、終わりの残高と仮押さえを出す。together があれば、とりうる結果を全部出す",
                "run a scenario in the reference interpreter, and print what each operation answered and the balances and holds at the end; with together, every way it can come out"
            ),
            params: vec![book(false)],
            flags: vec![
                flag("--scenario", Some("<file.json>"), tr!("流すシナリオ（一つ、または `chobo scenarios` が出す配列）", "the scenario to run (one, or the list `chobo scenarios` prints)")),
                flag(
                    "--show",
                    Some("postgres|tigerbeetle"),
                    tr!(
                        "結果の代わりに、クライアントがその出力先に送るものを操作ごとに出す（PostgreSQL では関数と引数、TigerBeetle では作る勘定とチェーン）",
                        "instead of the results, print what a client sends that target for each operation (the function and its arguments for PostgreSQL; the accounts it makes and the chain for TigerBeetle)"
                    ),
                )
                .choices(&["postgres", "tigerbeetle"]),
                flag("--format", Some("json"), tr!("結果を JSON で出す", "print the result as JSON")).choices(&["json"]),
            ],
            exits: vec![
                (0, tr!("流せた", "it ran")),
                (1, tr!("帳簿にエラーがある", "the book has errors")),
                (2, tr!("使い方の誤り、読めないファイル、シナリオの誤り", "a mistake in how the command is called, a file that cannot be read, or a mistake in the scenario")),
            ],
            examples: ["chobo run inventory.book --scenario 001.json", "chobo run inventory.book --scenario 001.json --show tigerbeetle --format json"],
            codes: vec![],
        },
        Cmd {
            name: "scenarios",
            usage: "chobo scenarios <file.book> [--out <dir>]",
            purpose: tr!(
                "帳簿からシナリオを作る。境界の手前・ちょうど・超える、同じキーの二度目、仮押さえの終わり方、移動の途中での断り、二つの呼び出し元の取り合い",
                "write scenarios for a book: each bound just before, at and past it, each key used twice, every way a hold ends, a refusal partway through the moves, and two callers after the last of something"
            ),
            params: vec![book(false)],
            flags: vec![flag("--out", Some("<dir>"), tr!("001.json から一つずつ書く先。無ければ JSON の配列を標準出力に出す", "the directory to write them into, from 001.json; without it, a JSON list on standard output"))],
            exits: vec![(0, tr!("作れた", "written")), (1, tr!("帳簿にエラーがある", "the book has errors")), (2, usage_exit.clone())],
            examples: ["chobo scenarios inventory.book", "chobo scenarios inventory.book --out scenarios/"],
            codes: vec![],
        },
        Cmd {
            name: "build",
            usage: "chobo build <file.book> --target <target> [--out <dir>]",
            purpose: tr!(
                "帳簿を、PostgreSQL のスキーマと関数の SQL か、TypeScript・Python・Go のクライアント（PostgreSQL を呼ぶもの、TigerBeetle を呼ぶもの）にする",
                "build the book into PostgreSQL's schema and functions, as SQL, or into a client in TypeScript, Python or Go that calls PostgreSQL or TigerBeetle"
            ),
            params: vec![book(false)],
            flags: vec![
                flag("--target", Some("<target>"), tr!("書くもの。postgres は SQL、ほかはクライアント", "what to write: postgres is the SQL, the others are clients")).choices(&TARGETS),
                flag("--out", Some("<dir>"), tr!("書く先のディレクトリ", "the directory to write into")).default("."),
            ],
            exits: vec![
                (0, tr!("書いた", "written")),
                (1, tr!("帳簿にエラーがある、またはその出力先が帳簿を受け取れない（E060、E061）", "the book has errors, or the target cannot take it (E060, E061)")),
                (2, usage_exit.clone()),
            ],
            examples: ["chobo build inventory.book --target postgres --out db/", "chobo build inventory.book --target tigerbeetle-go --out internal/"],
            codes: vec!["E060", "E061"],
        },
        Cmd {
            name: "doc",
            usage: "chobo doc <file.book> [--format html] [--out <dir>]",
            purpose: tr!(
                "帳簿を、経理や運用の人が読むページにする。勘定と境界、勘定のあいだの流れの図、振替ごとの移動とキーと断られうる理由（そうなる例つき）、仮押さえのライフサイクルの図、シナリオとステップごとの残高",
                "write the book as a page for the people who keep the accounts and run the operations: the accounts and their bounds, a chart of how things move between them, each transfer's moves, key and what it can be refused with (with the operations that get there), the life of a hold as a chart, and the scenarios with the balances after each step"
            ),
            params: vec![book(false)],
            flags: vec![
                flag("--format", Some("html"), tr!("Mermaid の図を入れた Markdown の代わりに、一つの HTML を書く（図は chobo が描き、シナリオを一つずつ進めて見られる）", "write one HTML page instead of Markdown with Mermaid charts (chobo draws the charts, and a scenario can be stepped through)")).choices(&["html"]),
                flag("--out", Some("<dir>"), tr!("<ファイル名>.md か <ファイル名>.html を書く先のディレクトリ。無ければ標準出力に出す", "the directory to write <file>.md or <file>.html into; without it, standard output")),
            ],
            exits: vec![(0, tr!("書いた", "written")), (1, tr!("帳簿にエラーがある", "the book has errors")), (2, usage_exit.clone())],
            examples: ["chobo doc inventory.book > inventory.md", "chobo doc inventory.book --format html --out site/ --lang ja"],
            codes: vec![],
        },
        Cmd {
            name: "api",
            usage: "chobo api <file.book>",
            purpose: tr!(
                "呼び方、操作ごとに断られうる理由、仮押さえのステートマシン、ID の決め方を JSON で出す（dandori のようなツールが読む）",
                "print how to call the book, what each operation can be refused with, the life of a hold as a state machine, and how IDs are made, as JSON (for tools such as dandori)"
            ),
            params: vec![book(false)],
            flags: vec![],
            exits: vec![(0, tr!("出せた", "printed")), (1, tr!("帳簿にエラーがある", "the book has errors")), (2, usage_exit.clone())],
            examples: ["chobo api inventory.book", "chobo api inventory.book > inventory.api.json"],
            codes: vec![],
        },
        Cmd {
            name: "explain",
            usage: "chobo explain <code> | --all [--format markdown]",
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
            examples: ["chobo explain E020", "chobo explain --all --format markdown --lang ja"],
            codes: vec![],
        },
    ]
}

/// The values of `--target`: chobo::target::Target's names, in its order.
const TARGETS: [&str; 7] = ["postgres", "postgres-typescript", "postgres-python", "postgres-go", "tigerbeetle-typescript", "tigerbeetle-python", "tigerbeetle-go"];

fn top_help(lang: Lang) -> String {
    let mut o = String::new();
    o.push_str(tr!(
        "chobo — 在庫、お金、ポイント、予約の枠のように、数で持っていて勘定から勘定へ動かすものの小さな言語\n\n",
        "chobo — a small language for the things you count and move between accounts: stock, money, points, seats\n\n"
    )
    .get(lang));
    o.push_str(tr!("使い方:\n", "Usage:\n").get(lang));
    let cmds = commands();
    for c in &cmds {
        o.push_str(&format!("  {}\n", c.usage));
    }
    o.push_str(tr!("\nコマンド:\n", "\nCommands:\n").get(lang));
    let w = cmds.iter().map(|c| c.name.len()).max().unwrap_or(0);
    for c in &cmds {
        o.push_str(&format!("  {:w$}  {}\n", c.name, c.purpose.get(lang)));
    }
    o.push_str(tr!("\nどのコマンドにも付けられるフラグ:\n", "\nFlags for every command:\n").get(lang));
    for f in global_flags() {
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
    let mut o = format!("{}\n\n{}\n", c.usage, c.purpose.get(lang));
    o.push_str(tr!("\n引数:\n", "\nArguments:\n").get(lang));
    for (p, h) in &c.params {
        o.push_str(&format!("  {:16} {}\n", p, h.get(lang)));
    }
    o.push_str(tr!("\nフラグ:\n", "\nFlags:\n").get(lang));
    let mut flags: Vec<Flag> = c.flags.iter().map(|f| Flag { name: f.name, value: f.value, choices: f.choices, default: f.default, help: f.help.clone() }).collect();
    flags.extend(global_flags());
    for f in &flags {
        let mut h = f.help.get(lang).to_string();
        if let Some(d) = f.default {
            h.push_str(&tr!("（既定: {d}）", " (default: {d})").get(lang).to_string());
        }
        o.push_str(&format!("  {:24} {}\n", f.spelled(), h));
    }
    o.push_str(tr!("\n終了コード:\n", "\nExit codes:\n").get(lang));
    for (n, h) in &c.exits {
        o.push_str(&format!("  {n}  {}\n", h.get(lang)));
    }
    o.push_str(tr!("\n例:\n", "\nExamples:\n").get(lang));
    for e in c.examples {
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
    lang: Lang,
}

/// What a parse ends in: the arguments, or something to print and the exit code.
enum Parsed {
    Run(Args),
    Print(String, u8, bool),
}

fn parse(argv: Vec<String>) -> Parsed {
    // the language first, so that every message, the help among them, is in it
    let lang_flag = argv.windows(2).find(|w| w[0] == "--lang").map(|w| w[1].clone());
    let lang = Lang::pick(lang_flag.as_deref());
    let Some(cmd) = argv.first().cloned() else {
        return Parsed::Print(top_help(lang), 2, true);
    };
    match cmd.as_str() {
        "--help" | "-h" | "help" => return Parsed::Print(top_help(lang), 0, false),
        "--version" | "-V" => return Parsed::Print(format!("chobo {}\n", env!("CARGO_PKG_VERSION")), 0, false),
        _ => {}
    }
    let cmds = commands();
    let Some(c) = cmds.iter().find(|c| c.name == cmd) else {
        let msg = tr!("知らないコマンド `{cmd}` です\n\n", "unknown command `{cmd}`\n\n").get(lang).to_string();
        return Parsed::Print(msg + &top_help(lang), 2, true);
    };
    let mut flags_known: Vec<Flag> = global_flags();
    flags_known.extend(c.flags.iter().map(|f| Flag { name: f.name, value: f.value, choices: f.choices, default: f.default, help: f.help.clone() }));
    let mut a = Args { cmd: cmd.clone(), positional: vec![], flags: BTreeMap::new(), lang };
    let mut it = argv.into_iter().skip(1);
    while let Some(x) = it.next() {
        if x == "--help" || x == "-h" {
            return Parsed::Print(cmd_help(c, lang), 0, false);
        }
        if !x.starts_with("--") {
            a.positional.push(x);
            continue;
        }
        let Some(f) = flags_known.iter().find(|f| f.name == x) else {
            let msg = tr!(
                "`chobo {cmd}` に `{x}` というフラグはありません\n\n",
                "`chobo {cmd}` has no flag `{x}`\n\n"
            );
            return Parsed::Print(msg.get(lang).to_string() + &cmd_help(c, lang), 2, true);
        };
        if a.flags.contains_key(f.name) {
            return Parsed::Print(tr!("`{x}` が二度あります\n", "`{x}` is given twice\n").get(lang).to_string(), 2, true);
        }
        let value = match f.value {
            None => None,
            Some(ph) => match it.next() {
                Some(v) if !v.starts_with("--") => {
                    if !f.choices.is_empty() && !f.choices.contains(&v.as_str()) {
                        let ch = f.choices.join(", ");
                        return Parsed::Print(tr!("`{x}` に渡せるのは {ch} です（`{v}` ではなく）\n", "`{x}` takes {ch}, not `{v}`\n").get(lang).to_string(), 2, true);
                    }
                    Some(v)
                }
                _ => return Parsed::Print(tr!("`{x}` には値 {ph} が要ります\n", "`{x}` needs a value: {ph}\n").get(lang).to_string(), 2, true),
            },
        };
        a.flags.insert(f.name.to_string(), value);
    }
    Parsed::Run(a)
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
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
    let code = match a.cmd.as_str() {
        "check" => cmd_check(&a),
        "run" => cmd_run(&a),
        "scenarios" => cmd_scenarios(&a),
        "build" => cmd_build(&a),
        "doc" => cmd_doc(&a),
        "api" => cmd_api(&a),
        "explain" => cmd_explain(&a),
        _ => 2,
    };
    ExitCode::from(code)
}

fn usage(a: &Args) -> u8 {
    if let Some(c) = commands().iter().find(|c| c.name == a.cmd) {
        eprintln!("{}", c.usage);
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
fn load(path: &Path, lang: Lang) -> Result<(String, chobo::model::Book, check::Checked), u8> {
    let (src, c) = match check::check_file(path) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("{e}");
            return Err(2);
        }
    };
    let file = path.display().to_string();
    for d in &c.diags {
        eprint!("{}", d.render(&file, &src, lang));
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
                eprintln!("{e}");
                worst = 2;
                continue;
            }
        };
        let mut note = None;
        if let (Some(rev), Some(book)) = (&rev, &c.book) {
            match chobo::diffbase::read_at(path, rev) {
                Ok(before) => {
                    let (more, n) = chobo::diffbase::against(book, before.as_deref(), rev);
                    c.diags.extend(more);
                    chobo::model::sort(&mut c.diags);
                    if diag::has_errors(&c.diags) {
                        c.report = None;
                    }
                    note = n;
                }
                Err(e) => {
                    eprintln!("{e}");
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
        println!("{}", serde_json::to_string_pretty(&json!({"v": 1, "files": files})).unwrap());
    }
    worst
}

fn cmd_run(a: &Args) -> u8 {
    let Some(path) = one_book(a) else { return usage(a) };
    let Some(Some(sc)) = a.flags.get("--scenario") else {
        eprintln!("{}", tr!("`--scenario <file.json>` が要ります", "`--scenario <file.json>` is required").get(a.lang));
        return 2;
    };
    let (_, book, _) = match load(&path, a.lang) {
        Ok(x) => x,
        Err(c) => return c,
    };
    let v: Value = match std::fs::read_to_string(sc).map_err(|e| e.to_string()).and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string())) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{sc}: {e}");
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
                eprintln!("{sc}: {}{e}", if list.len() > 1 { format!("scenario {}: ", i + 1) } else { String::new() });
                return 2;
            }
        };
        let runs = match scenario::run(&book, &s) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{sc}: {e}");
                return 2;
            }
        };
        if let Some(target) = &show {
            if a.flags.contains_key("--format") {
                let ops = if target == "postgres" { render::postgres_json(&book, "", &s) } else { render::tigerbeetle_json(&book, "", &s) };
                outs.push(json!({"scenario": s.name, "target": target, "tenant": "", "operations": ops}));
            } else if target == "postgres" {
                text.push_str(&render::postgres_text(&book, &file, "", &s, a.lang));
            } else {
                text.push_str(&render::tigerbeetle_text(&book, &file, "", &s, a.lang));
            }
            continue;
        }
        if a.flags.contains_key("--format") {
            match scenario::run_json(&book, &s) {
                Ok(r) => outs.push(r),
                Err(e) => {
                    eprintln!("{sc}: {e}");
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
    let list: Vec<Value> = chobo::scenarios::generate(&book).iter().map(|s| scenario::to_json(&book, s)).collect();
    match a.flags.get("--out").cloned().flatten() {
        Some(dir) => {
            let dir = PathBuf::from(dir);
            if let Err(e) = std::fs::create_dir_all(&dir) {
                eprintln!("{}: {e}", dir.display());
                return 2;
            }
            for (i, s) in list.iter().enumerate() {
                let p = dir.join(format!("{:03}.json", i + 1));
                if let Err(e) = std::fs::write(&p, serde_json::to_string_pretty(s).unwrap() + "\n") {
                    eprintln!("{}: {e}", p.display());
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

fn cmd_build(a: &Args) -> u8 {
    let Some(path) = one_book(a) else { return usage(a) };
    let Some(Some(t)) = a.flags.get("--target") else {
        eprintln!("{}", tr!("`--target <target>` が要ります", "`--target <target>` is required").get(a.lang));
        return 2;
    };
    let target = Target::parse(t).expect("the choices of --target are the targets");
    let (src, book, _) = match load(&path, a.lang) {
        Ok(x) => x,
        Err(c) => return c,
    };
    let stem = path.file_name().map(|f| f.to_string_lossy().trim_end_matches(".book").to_string()).unwrap_or_default();
    let file = path.display().to_string();
    let files = match chobo::target::build(&book, &stem, target) {
        Ok(f) => f,
        Err(diags) => {
            for d in &diags {
                eprint!("{}", d.render(&file, &src, a.lang));
            }
            eprintln!("{}", diag::summary(&file, &diags, a.lang));
            return 1;
        }
    };
    let out = PathBuf::from(a.flags.get("--out").cloned().flatten().unwrap_or_else(|| ".".into()));
    for (rel, text) in &files {
        let p = out.join(rel);
        if let Err(e) = p.parent().map(std::fs::create_dir_all).unwrap_or(Ok(())).and_then(|_| std::fs::write(&p, text)) {
            eprintln!("{}: {e}", p.display());
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
    let input = chobo::doc::Input { book: &book, file: &file, src: &src, diags: &c.diags, report: &rep, lang: a.lang };
    let html = a.flags.contains_key("--format");
    let page = if html { chobo::doc::html(&input) } else { chobo::doc::markdown(&input) };
    match a.flags.get("--out").cloned().flatten() {
        Some(dir) => {
            let stem = path.file_name().map(|f| f.to_string_lossy().trim_end_matches(".book").to_string()).unwrap_or_default();
            let p = PathBuf::from(dir).join(format!("{stem}.{}", if html { "html" } else { "md" }));
            if let Err(e) = p.parent().map(std::fs::create_dir_all).unwrap_or(Ok(())).and_then(|_| std::fs::write(&p, &page)) {
                eprintln!("{}: {e}", p.display());
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
    println!("{}", serde_json::to_string_pretty(&chobo::api::api(&book, &src, &rep, &stem)).unwrap());
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
            eprintln!("{}", tr!("`{code}` というコードはありません。一覧は `chobo explain --all`", "there is no code `{code}`; `chobo explain --all` lists them").get(a.lang));
            2
        }
    }
}
