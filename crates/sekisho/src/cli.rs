//! The commands and their flags, in one table (ritsu's DESIGN 4.4). `--help` is drawn from it and
//! the command line is read against it, so a flag cannot be documented and ignored, or taken and
//! left undocumented; an unknown flag, a value outside a closed set, a flag without its value and a
//! flag given twice stop the run with exit 2. The table is sekisho's; how it is drawn and read is
//! ritsu-base's ([`ritsu_base::cli`]).
//!
//! `check` and `explain` are the commands of stage A; `gen`, `doc`, `vectors` and `api` join the
//! table as they are written (DESIGN 11).

use crate::check::{self, Options};
use crate::suite::Suite;
use ritsu_base::cli::{Args, Cmd, Reading, Table, flag, help_flag, lang_flag};
use ritsu_base::text::{Lang, Text};
use std::io::Write;

fn every_code() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = crate::codes::ledger().entries.iter().map(|e| e.code).collect();
    v.sort_by_key(|c| (c.starts_with('W'), *c));
    v
}

pub fn table() -> Table {
    Table {
        tool: "sekisho",
        version: env!("CARGO_PKG_VERSION"),
        summary: tr!(
            "認可を書く小さな言語。だれが何に何をしてよいかを、役割・属性・関係と、rulec の規則と koyomi の日付を条件にして書き、宣言した範囲のすべての組み合わせで確かめて、Cedar を生成する",
            "A small language for who may do what: roles, attributes and relations, with rulec's rules and koyomi's dates as conditions, checked on every combination it declares and compiled to Cedar"
        ),
        globals: vec![lang_flag("SEKISHO_LANG"), help_flag()],
        commands: vec![
            Cmd {
                usage: None,
                name: "check",
                args: "<file.gate>...",
                purpose: tr!("検査する。構文、名前と型、宣言した範囲のすべての組み合わせ", "check the words, the names and the types, and every combination the file declares"),
                params: vec![("<file.gate>...", tr!("検査する .gate のファイル", "the .gate files to check"))],
                flags: vec![
                    flag("--format", Some("json"), tr!("ファイルごとに一つの JSON", "one JSON object a file")).choices(&["json"]),
                    flag("--budget", Some("<n>"), tr!("一つの action で数える組み合わせの上限", "the most combinations the check walks for one action")).default("100000000"),
                ],
                exits: vec![
                    (0, tr!("エラーなし（警告はありうる）", "no errors (there may be warnings)")),
                    (1, tr!("エラーが一つ以上", "at least one error")),
                    (2, tr!("引数の誤り、読めないファイル、規則・日付・カレンダー・帳簿・フローを読むファイルを、それらを読めないこの sekisho で走らせた（E209。ritsu sekisho で走らせる）", "bad arguments, a file that cannot be read, or a file that reads a rule, a dates file, a calendar, a book or a flow, run with this sekisho, which reads none of them (E209: run it as ritsu sekisho)")),
                ],
                examples: vec!["sekisho check shop.gate", "ritsu sekisho check examples/refunds/refunds.gate --format json --lang ja"],
                codes: every_code(),
            },
            Cmd {
                usage: Some("sekisho explain <code> | --all [--format markdown|json] [--lang ja|en]"),
                name: "explain",
                args: "<code>",
                purpose: tr!("診断のコードを説明する。いつ出るか、直し方、それを出す一番小さな例", "explain a diagnostic code: when it is printed, how to fix it, and the smallest file that prints it"),
                params: vec![("<code>", tr!("`E101` のようなコード", "a code such as `E101`"))],
                flags: vec![
                    flag("--all", None, tr!("全部のコード", "every code")),
                    flag("--format", Some("markdown|json"), tr!("Markdown（docs/codes.md の形）か JSON", "Markdown (as docs/codes.md is) or JSON")).choices(&["markdown", "json"]),
                ],
                exits: vec![(0, tr!("説明した", "explained")), (2, tr!("知らないコード、引数の誤り", "an unknown code, or bad arguments"))],
                examples: vec!["sekisho explain E101", "sekisho explain --all --format markdown --lang ja"],
                codes: vec![],
            },
        ],
        footer: vec![
            tr!("どのコマンドも `--lang ja|en` と `--help` を取ります。", "Every command takes `--lang ja|en` and `--help`."),
            tr!(
                "exit code: 0 エラーなし / 1 エラーあり / 2 引数の誤りか読めないファイル、またはほかの言語を読むファイルを、それを読めないこの sekisho で走らせた（E209）",
                "Exit codes: 0 no errors / 1 errors found / 2 bad arguments or an unreadable file, or a file that reads another language run with this sekisho, which reads none (E209)"
            ),
        ],
        reading: Reading::default(),
    }
}

fn refuse(w: &mut dyn Write, msg: Text, lang: Lang) -> u8 {
    let head = if lang == Lang::Ja { "エラー" } else { "error" };
    let _ = writeln!(w, "{head}: {}", msg.get(lang));
    2
}

/// `--lang ja`, `--lang=ja`, anywhere on the line: decided before anything is printed.
fn lang_word(args: &[String]) -> Option<String> {
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

/// The command line, read against the table and run: the exit code.
pub fn run(args: &[String], suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let lang = Lang::pick(lang_word(args).as_deref(), "SEKISHO_LANG");
    let table = table();
    let Some(first) = args.first() else {
        let _ = write!(err, "{}", table.help_all(lang));
        return 2;
    };
    match first.as_str() {
        "--help" | "-h" => {
            let _ = write!(out, "{}", table.help_all(lang));
            return 0;
        }
        "--version" | "-V" => {
            let _ = writeln!(out, "sekisho {}", env!("CARGO_PKG_VERSION"));
            return 0;
        }
        "help" => {
            let rest: Vec<&String> = args[1..].iter().filter(|a| !a.starts_with("--lang")).collect();
            let rest: Vec<&String> = rest.into_iter().filter(|a| lang_word(args).as_deref() != Some(a.as_str())).collect();
            return match rest.first() {
                None => {
                    let _ = write!(out, "{}", table.help_all(lang));
                    0
                }
                Some(n) => match table.command(n) {
                    Some(c) => {
                        let _ = write!(out, "{}", table.help_cmd(c, lang));
                        0
                    }
                    None => refuse(err, tr!("`{n}` というコマンドはありません。`sekisho --help` を読んでください", "there is no command `{n}`; run `sekisho --help`"), lang),
                },
            };
        }
        _ => {}
    }
    let Some(cmd) = table.command(first) else {
        return refuse(err, tr!("`{first}` というコマンドはありません。`sekisho --help` を読んでください", "there is no command `{first}`; run `sekisho --help`"), lang);
    };
    let a = match table.parse(cmd, &args[1..]) {
        Ok(a) => a,
        Err(e) => return refuse(err, e, lang),
    };
    if a.has("--help") {
        let _ = write!(out, "{}", table.help_cmd(cmd, lang));
        return 0;
    }
    match cmd.name {
        "check" => check_cmd(&a, lang, suite, out, err),
        "explain" => explain_cmd(&a, lang, out, err),
        _ => unreachable!("every command in the table is dispatched"),
    }
}

fn check_cmd(a: &Args, lang: Lang, suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    if a.pos.is_empty() {
        return refuse(err, tr!("`sekisho check` には .gate のファイルが要ります", "`sekisho check` needs .gate files"), lang);
    }
    let budget = match a.get("--budget") {
        None => check::DEFAULT_BUDGET,
        Some(b) => match b.replace([',', '_'], "").parse::<u64>() {
            Ok(n) if n > 0 => n,
            _ => return refuse(err, tr!("`--budget {b}` は正の整数ではありません", "`--budget {b}` is not a positive integer"), lang),
        },
    };
    let opts = Options { budget };
    let json = a.get("--format") == Some("json");
    let mut worst = 0u8;
    for f in &a.pos {
        match check::check_file(f, suite, &opts) {
            Ok(o) => {
                worst = worst.max(o.exit());
                if json {
                    let _ = writeln!(out, "{}", check::to_json(&o, lang).pretty());
                } else {
                    let _ = write!(out, "{}", check::render(&o, lang));
                }
            }
            Err(e) => {
                worst = 2;
                refuse(err, tr!("`{f}` を読めません: {e}", "cannot read `{f}`: {e}"), lang);
            }
        }
    }
    worst
}

fn explain_cmd(a: &Args, lang: Lang, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let format = a.get("--format");
    let ledger = crate::codes::ledger();
    if a.has("--all") {
        match format {
            Some("markdown") => {
                let _ = write!(out, "{}", ledger.render_markdown(lang));
            }
            Some("json") => {
                let _ = writeln!(out, "{}", ritsu_base::json::Json::arr(ledger.entries.iter().map(|e| ledger.to_json(e, lang))).pretty());
            }
            _ => {
                for (i, e) in ledger.entries.iter().enumerate() {
                    if i > 0 {
                        let _ = writeln!(out);
                    }
                    let _ = write!(out, "{}", ledger.render_text(e, lang));
                }
            }
        }
        return 0;
    }
    let Some(code) = a.pos.first() else {
        return refuse(err, tr!("`sekisho explain` には `E101` のようなコードか `--all` が要ります", "`sekisho explain` needs a code such as `E101`, or `--all`"), lang);
    };
    match ledger.find(code) {
        Some(e) => {
            match format {
                Some("markdown") => {
                    let _ = write!(out, "{}", ledger.render_markdown_one(e, lang));
                }
                Some("json") => {
                    let _ = writeln!(out, "{}", ledger.to_json(e, lang).pretty());
                }
                _ => {
                    let _ = write!(out, "{}", ledger.render_text(e, lang));
                }
            }
            0
        }
        None => refuse(err, tr!("`{code}` という診断のコードはありません。`sekisho explain --all` で全部を見られます", "there is no diagnostic code `{code}`; `sekisho explain --all` lists them all"), lang),
    }
}
