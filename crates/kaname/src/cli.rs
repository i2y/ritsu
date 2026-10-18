//! The commands and their flags, in one table (ritsu's DESIGN 4.4). `--help` is drawn from it and
//! the command line is read against it, so a flag cannot be documented and ignored, or taken and
//! left undocumented; an unknown flag, a value outside a closed set, a flag without its value and a
//! flag given twice stop the run with exit 2. The table is kaname's; how it is drawn and read is
//! ritsu-base's ([`ritsu_base::cli`]).
//!
//! The commands are `check`, `run`, `vectors`, `doc`, `api` and `explain` (DESIGN 11.1).

use crate::check::{self, Run};
use crate::suite::Suite;
use ritsu_base::cli::{Args, Cmd, Flag, Reading, Table, flag, help_flag, lang_flag};
use ritsu_base::text::{Lang, Text};
use std::io::Write;

fn every_code() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = crate::codes::ledger().entries.iter().filter(|e| !matches!(e.repro, ritsu_base::ledger::Repro::Later)).map(|e| e.code).collect();
    v.sort_by_key(|c| (c.starts_with('W'), *c));
    v
}

/// `--root`, as the other languages take it (ritsu's DESIGN 6.2): where the paths in the JSON are
/// written from.
fn root_flag() -> Flag {
    flag(
        "--root",
        Some("<dir>"),
        tr!(
            "JSON に書くパスを数えるルート。無ければ、最初に渡したパスの上で .git を持つ一番近いディレクトリ（それも無ければ、そのファイルのあるディレクトリ）",
            "the root the paths in the JSON count from; else the nearest directory above the first path given that holds .git (else the directory of that file)"
        ),
    )
}

/// `--root` of `doc`: where the paths on the page are written from.
fn page_root_flag() -> Flag {
    flag(
        "--root",
        Some("<dir>"),
        tr!(
            "ページに書くパスを数えるルート。無ければ、渡したパスの上で .git を持つ一番近いディレクトリ（それも無ければ、そのファイルのあるディレクトリ）",
            "the root the paths on the page count from; else the nearest directory above the path given that holds .git (else the directory of that file)"
        ),
    )
}

pub fn table() -> Table {
    Table {
        tool: "kaname",
        version: env!("CARGO_PKG_VERSION"),
        summary: tr!(
            "規則・日付・帳簿をつなぐコードを書く小さな言語。rulec の規則、koyomi の日付、chobo の帳簿の振替を、ritsu の型（単位、通貨と税区分、率の刻み、範囲、列挙、日付）のまま呼び、そのあいだの計算を同じ型と丸めの決まりで確かめる",
            "A small language for the code that joins rules, dates and books: it calls rulec's rules, koyomi's dates and chobo's transfers in ritsu's types (units, currencies and tax, the steps of rates, ranges, enums, dates), and checks the computation between them by the same rules of types and rounding"
        ),
        globals: vec![lang_flag("KANAME_LANG"), help_flag()],
        commands: vec![
            Cmd {
                usage: None,
                name: "check",
                args: "<file.kn>...",
                purpose: tr!(
                    "検査する。構文、名前、読むファイル（規則、日付のファイル、帳簿、ほかのモジュール）、型と単位と丸め、渡す値の範囲と int64 と 0 での割り算、関数のエフェクト、コードに書いた判断、`examples` の答え",
                    "check the words and the names, the files a module reads (rules, dates files, books, other modules), the types, units and rounding, the ranges of the values given, int64 and division by 0, the effects of the functions, the decisions written in code, and the answers of the `examples`"
                ),
                params: vec![("<file.kn>...", tr!("検査する .kn のファイル", "the .kn files to check"))],
                flags: vec![flag("--format", Some("json"), tr!("ファイルごとに一つの JSON", "one JSON object a file")).choices(&["json"]), root_flag()],
                exits: vec![
                    (0, tr!("エラーなし（警告はありうる）", "no errors (there may be warnings)")),
                    (1, tr!("エラーが一つ以上", "at least one error")),
                    (
                        2,
                        tr!(
                            "引数の誤り、読めないファイル、規則・日付のファイル・帳簿を読むモジュールを、それらを読めないこの kaname で走らせた（E202。ritsu kaname で走らせる）",
                            "bad arguments, a file that cannot be read, or a module that reads a rule, a dates file or a book, run with this kaname, which reads none of them (E202: run it as ritsu kaname)"
                        ),
                    ),
                ],
                examples: vec!["ritsu kaname check payout.kn", "ritsu kaname check late_payment.ja.kn --format json --lang ja"],
                codes: every_code(),
            },
            Cmd {
                usage: Some("kaname run <file.kn> <function> --input <file.json|-> [--trace] [--format json] [--root <dir>] [--lang ja|en]"),
                name: "run",
                args: "<file.kn> <function>",
                purpose: tr!(
                    "関数を一つ、JSON で渡した入力で参照インタプリタに流す。規則と日付の呼び出しとその答え、作った振替、関数の答えか失敗を出す",
                    "run one function in the reference interpreter on an input given as JSON: the calls of rules and dates with their answers, the transfers built, and the function's answer or its failure"
                ),
                params: vec![
                    ("<file.kn>", tr!("検査を通る .kn のファイル", "a .kn file that passes check")),
                    ("<function>", tr!("流す関数の名前か別名", "the function to run, by its name or its alias")),
                ],
                flags: vec![
                    flag("--input", Some("<file.json|->"), tr!("引数を、引数の名前をキーにした JSON のオブジェクトで渡す（必ず書く。- は標準入力）", "the arguments, a JSON object with a key for each parameter (required; - reads standard input)")),
                    flag("--trace", None, tr!("`let` ごとの値も出す", "print the value of every `let` too")),
                    flag("--format", Some("json"), tr!("一つの JSON", "one JSON object")).choices(&["json"]),
                    root_flag(),
                ],
                exits: vec![
                    (0, tr!("最後まで評価した（関数の答えか、関数の失敗）", "evaluated to the end: the function's answer, or its failure")),
                    (1, tr!("ファイルが検査を通らない、または評価が止まらなかった", "the file does not pass check, or the evaluation did not stop")),
                    (
                        2,
                        tr!(
                            "引数の誤り、読めないファイルや入力、無い関数、規則・日付のファイル・帳簿を読むモジュールを、それらを読めないこの kaname で走らせた（E202）",
                            "bad arguments, a file or an input that cannot be read, no such function, or a module that reads a rule, a dates file or a book, run with this kaname, which reads none of them (E202)"
                        ),
                    ),
                ],
                examples: vec!["ritsu kaname run late_payment.kn claim --input claim.json", "ritsu kaname run late_payment.ja.kn 遅れの請求 --input claim.ja.json --trace --lang ja"],
                codes: vec![],
            },
            Cmd {
                usage: None,
                name: "vectors",
                args: "<file.kn>",
                purpose: tr!(
                    "関数ごとに入力を選び、参照インタプリタの答えを添えて、一件一行の JSON で出す（範囲の両端、比べるリテラル、規則と日付の入力の範囲の両端などから選ぶ）",
                    "choose inputs for each function and print each with the reference interpreter's answer, one JSON object a line (from the ends of the ranges, the literals compared, the ends of the ranges of the rules' and dates' inputs)"
                ),
                params: vec![("<file.kn>", tr!("検査を通る .kn のファイル", "a .kn file that passes check"))],
                flags: vec![
                    flag("--function", Some("<name>"), tr!("この関数のベクタだけ（名前か別名）", "the vectors of this function only (its name or its alias)")),
                    flag("--trace", None, tr!("規則と日付の呼び出しとその答え、作った振替も書く（calls）", "write the calls of rules and dates with their answers, and the transfers built, too (calls)")),
                ],
                exits: vec![
                    (0, tr!("出した", "printed")),
                    (1, tr!("ファイルが検査を通らない", "the file does not pass check")),
                    (
                        2,
                        tr!(
                            "引数の誤り、読めないファイル、無い関数、規則・日付のファイル・帳簿を読むモジュールを、それらを読めないこの kaname で走らせた（E202）",
                            "bad arguments, a file that cannot be read, no such function, or a module that reads a rule, a dates file or a book, run with this kaname, which reads none of them (E202)"
                        ),
                    ),
                ],
                examples: vec!["ritsu kaname vectors late_payment.kn", "ritsu kaname vectors media_mail.kn --function quote --trace"],
                codes: vec![],
            },
            Cmd {
                usage: Some("kaname doc <file.kn> [--format markdown|html] [--root <dir>] [--lang ja|en]"),
                name: "doc",
                args: "<file.kn>",
                purpose: tr!(
                    "人が読むページを書く。表にある判断（呼ぶ規則と日付のファイルのページ、組み立てる振替と拒否されうる理由）、関数（total、実行時に確かめる場所とその理由、止まらないかもしれないもの）、コードにある判断、例の答え",
                    "write the page for people: the decisions in tables (the pages of the rules and the dates files it calls, the transfers it builds with the reasons they can be refused with), the functions (total, the places checked when they run and why, those that may not stop), the decisions in code, and what the examples answer"
                ),
                params: vec![("<file.kn>", tr!("名前と型が検査を通る .kn のファイル", "a .kn file whose names and types pass check"))],
                flags: vec![
                    flag("--format", Some("markdown|html"), tr!("Markdown（既定）か、外から何も読まない一つの HTML のファイル", "Markdown (the default), or one HTML file that reads nothing from outside")).choices(&["markdown", "html"]),
                    page_root_flag(),
                ],
                exits: vec![
                    (0, tr!("書いた", "written")),
                    (1, tr!("ファイルの名前か型が検査を通らない", "the file's names or types do not pass check")),
                    (
                        2,
                        tr!(
                            "引数の誤り、読めないファイル、規則・日付のファイル・帳簿を読むモジュールを、それらを読めないこの kaname で走らせた（E202）",
                            "bad arguments, a file that cannot be read, or a module that reads a rule, a dates file or a book, run with this kaname, which reads none of them (E202)"
                        ),
                    ),
                ],
                examples: vec!["ritsu kaname doc late_payment.kn > late_payment.md", "ritsu kaname doc payout.ja.kn --format html --lang ja > payout.html"],
                codes: vec![],
            },
            Cmd {
                usage: None,
                name: "api",
                args: "<file.kn>",
                purpose: tr!(
                    "外のツールのための JSON を出す。モジュール、読むファイル、列挙とレコード、関数の引数と結果（型、範囲、要素の数）、エフェクト、ほかの言語の呼び出しと言語をまたぐ確かめの答え、振替の操作ごとの拒否されうる理由、生成するコードの名前",
                    "print the JSON for tools outside: the module, the files it reads, its enums and records, its functions' parameters and results (types, ranges, counts), their effects, the calls into the other languages with what the checks across the borders came to, the reasons each operation of a transfer can be refused with, and the names of the generated code"
                ),
                params: vec![("<file.kn>", tr!("名前と型が検査を通る .kn のファイル", "a .kn file whose names and types pass check"))],
                flags: vec![root_flag()],
                exits: vec![
                    (0, tr!("出した", "printed")),
                    (1, tr!("ファイルの名前か型が検査を通らない", "the file's names or types do not pass check")),
                    (
                        2,
                        tr!(
                            "引数の誤り、読めないファイル、規則・日付のファイル・帳簿を読むモジュールを、それらを読めないこの kaname で走らせた（E202）",
                            "bad arguments, a file that cannot be read, or a module that reads a rule, a dates file or a book, run with this kaname, which reads none of them (E202)"
                        ),
                    ),
                ],
                examples: vec!["ritsu kaname api payout.kn", "ritsu kaname api late_payment.kn --root ."],
                codes: vec![],
            },
            Cmd {
                usage: Some("kaname explain <code> | --all [--format markdown|json] [--lang ja|en]"),
                name: "explain",
                args: "<code>",
                purpose: tr!(
                    "診断のコードを説明する。いつ出るか、直し方、それを出す一番小さな例",
                    "explain a diagnostic code: when it is printed, how to fix it, and the smallest module that prints it"
                ),
                params: vec![("<code>", tr!("`E102` のようなコード", "a code such as `E102`"))],
                flags: vec![
                    flag("--all", None, tr!("全部のコード", "every code")),
                    flag("--format", Some("markdown|json"), tr!("Markdown（docs/codes.md の形）か JSON", "Markdown (as docs/codes.md is) or JSON")).choices(&["markdown", "json"]),
                ],
                exits: vec![(0, tr!("説明した", "explained")), (2, tr!("知らないコード、引数の誤り", "an unknown code, or bad arguments"))],
                examples: vec!["kaname explain E102", "kaname explain --all --format markdown --lang ja"],
                codes: vec![],
            },
        ],
        footer: vec![
            tr!("どのコマンドも `--lang ja|en` と `--help` を取ります。", "Every command takes `--lang ja|en` and `--help`."),
            tr!(
                "exit code: 0 エラーなし / 1 エラーあり / 2 引数の誤りか読めないファイル、またはほかの言語を読むモジュールを、それを読めないこの kaname で走らせた（E202）",
                "Exit codes: 0 no errors / 1 errors found / 2 bad arguments or an unreadable file, or a module that reads another language run with this kaname, which reads none (E202)"
            ),
        ],
        reading: Reading::default(),
    }
}

/// Say what is wrong with the command line, or with a file it cannot read: one line on `w`, exit 2.
pub(crate) fn refuse(w: &mut dyn Write, msg: Text, lang: Lang) -> u8 {
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
    let lang = Lang::pick(lang_word(args).as_deref(), "KANAME_LANG");
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
            let _ = writeln!(out, "kaname {}", env!("CARGO_PKG_VERSION"));
            return 0;
        }
        "help" => {
            let word = lang_word(args);
            let rest: Vec<&String> = args[1..].iter().filter(|a| !a.starts_with("--lang") && word.as_deref() != Some(a.as_str())).collect();
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
                    None => refuse(err, tr!("`{n}` というコマンドはありません。`kaname --help` を読んでください", "there is no command `{n}`; run `kaname --help`"), lang),
                },
            };
        }
        _ => {}
    }
    let Some(cmd) = table.command(first) else {
        return refuse(err, tr!("`{first}` というコマンドはありません。`kaname --help` を読んでください", "there is no command `{first}`; run `kaname --help`"), lang);
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
        "run" => run_cmd(&a, lang, suite, out, err),
        "vectors" => vectors_cmd(&a, lang, suite, out, err),
        "doc" => doc_cmd(&a, lang, suite, out, err),
        "api" => api_cmd(&a, lang, suite, out, err),
        "explain" => explain_cmd(&a, lang, out, err),
        _ => unreachable!("every command in the table is dispatched"),
    }
}

fn check_cmd(a: &Args, lang: Lang, suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    if a.pos.is_empty() {
        return refuse(err, tr!("`kaname check` には .kn のファイルが要ります", "`kaname check` needs .kn files"), lang);
    }
    let root = match a.get("--root") {
        Some(r) if !ritsu_base::fs::is_dir(std::path::Path::new(r)) => return refuse(err, tr!("`--root {r}` はディレクトリではありません", "`--root {r}` is not a directory"), lang),
        given => check::root_of(given.map(std::path::Path::new), &a.pos[0]),
    };
    let json = a.get("--format") == Some("json");
    let run = Run::new(suite.clone());
    let mut worst = 0u8;
    for f in &a.pos {
        match run.file(f) {
            Ok(o) => {
                worst = worst.max(o.exit());
                if json {
                    let _ = writeln!(out, "{}", check::to_json(&o, &root, lang).pretty());
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
        return refuse(err, tr!("`kaname explain` には `E102` のようなコードか `--all` が要ります", "`kaname explain` needs a code such as `E102`, or `--all`"), lang);
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
        None => {
            refuse(err, tr!("`{code}` という診断のコードはありません。`kaname explain --all` で全部を見られます", "there is no diagnostic code `{code}`; `kaname explain --all` lists them all"), lang)
        }
    }
}

/// The root of a run, from `--root` or from the first file: exit 2 when `--root` is no directory.
fn root(a: &Args, first: &str, lang: Lang, err: &mut dyn Write) -> Result<std::path::PathBuf, u8> {
    match a.get("--root") {
        Some(r) if !ritsu_base::fs::is_dir(std::path::Path::new(r)) => Err(refuse(err, tr!("`--root {r}` はディレクトリではありません", "`--root {r}` is not a directory"), lang)),
        given => Ok(check::root_of(given.map(std::path::Path::new), first)),
    }
}

/// A file as the JSON and the text name it: from the root.
fn rooted(root: &std::path::Path, f: &str) -> String {
    ritsu_base::paths::from_root(root, std::path::Path::new(f)).unwrap_or_else(|| f.to_string())
}

/// The file checked, when it passes: else what check says of it, exit 1 (2 when the languages it
/// reads are not joined).
fn passing(f: &str, what: Text, suite: &Suite, lang: Lang, out: &mut dyn Write, err: &mut dyn Write) -> Result<std::rc::Rc<check::Checked>, u8> {
    let o = match Run::new(suite.clone()).file(f) {
        Ok(o) => o,
        Err(e) => return Err(refuse(err, tr!("`{f}` を読めません: {e}", "cannot read `{f}`: {e}"), lang)),
    };
    if o.unjoined() {
        let _ = write!(out, "{}", check::render(&o, lang));
        return Err(2);
    }
    match &o.checked {
        Some(c) if !o.has_errors() => Ok(c.clone()),
        _ => {
            let _ = write!(out, "{}", check::render(&o, lang));
            let head = if lang == Lang::Ja { "エラー" } else { "error" };
            let _ = writeln!(err, "{head}: {}", what.get(lang));
            Err(1)
        }
    }
}

/// The function a word names, by its name or its alias: exit 2 with the functions there are when
/// none.
fn function(c: &check::Checked, f: &str, word: &str, lang: Lang, err: &mut dyn Write) -> Result<usize, u8> {
    crate::interp::function(c, word).ok_or_else(|| {
        let all = c.decls.fns.iter().map(|x| format!("`{}`", x.name.text)).collect::<Vec<_>>().join(", ");
        refuse(err, tr!("`{f}` に関数 `{word}` はありません（関数は {all}）", "`{f}` has no function `{word}` (its functions are {all})"), lang)
    })
}

/// Where something happened, as `run` says it: the line, and the file when it is another module's.
fn place(root: &std::path::Path, here: &str, file: &str, line: usize, lang: Lang) -> String {
    let other = (file != here).then(|| rooted(root, file));
    match (lang, other) {
        (Lang::Ja, None) => format!("{line} 行目"),
        (Lang::Ja, Some(f)) => format!("{f} の {line} 行目"),
        (Lang::En, None) => format!("line {line}"),
        (Lang::En, Some(f)) => format!("{f}, line {line}"),
    }
}

fn run_cmd(a: &Args, lang: Lang, suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    use crate::interp::{Event, Interp, Stop};
    let [f, word] = a.pos.as_slice() else {
        return refuse(err, tr!("`kaname run` には .kn のファイルと関数の名前を渡してください", "`kaname run` takes a .kn file and the name of a function"), lang);
    };
    let Some(input_at) = a.get("--input") else {
        return refuse(err, tr!("`kaname run` には `--input <file.json|->` が要ります", "`kaname run` needs `--input <file.json|->`"), lang);
    };
    let root = match root(a, f, lang, err) {
        Ok(r) => r,
        Err(code) => return code,
    };
    let c = match passing(f, tr!("`{f}` は検査を通らないので、流しません", "`{f}` does not pass check, so it is not run"), suite, lang, out, err) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let fi = match function(&c, f, word, lang, err) {
        Ok(i) => i,
        Err(code) => return code,
    };
    let text = if input_at == "-" {
        let mut s = String::new();
        match std::io::Read::read_to_string(&mut std::io::stdin(), &mut s) {
            Ok(_) => s,
            Err(e) => return refuse(err, tr!("標準入力を読めません: {e}", "cannot read standard input: {e}"), lang),
        }
    } else {
        match ritsu_base::fs::read_to_string(input_at) {
            Ok(s) => s,
            Err(e) => return refuse(err, tr!("`{input_at}` を読めません: {e}", "cannot read `{input_at}`: {e}"), lang),
        }
    };
    let input = match ritsu_base::json::parse(&text) {
        Ok(j) => j,
        Err(e) => {
            let shown = if input_at == "-" { tr!("標準入力", "standard input") } else { Text::same(input_at) };
            return refuse(err, tr!("{} は JSON として読めません: {}", "{} is not JSON: {}", shown.ja, e.message.ja; shown.en, e.message.en), lang);
        }
    };
    let p = crate::value::Program::of(&c);
    let mut it = Interp::new(&p, suite);
    it.lets = a.has("--trace");
    let info = &c.decls.fns[fi];
    let entered = it.enter_json(&c, fi, &input);
    let outcome = match &entered {
        Ok(args) => it.call(&c, fi, args.clone(), 0),
        Err(fl) => Err(Stop::Failed(fl.clone())),
    };
    let diverged = matches!(outcome, Err(Stop::Diverged(_)));
    if a.get("--format") == Some("json") {
        let mut fields: Vec<(String, ritsu_base::json::Json)> = vec![
            ("kaname".into(), ritsu_base::json::Json::str(env!("CARGO_PKG_VERSION"))),
            ("file".into(), ritsu_base::json::Json::str(rooted(&root, f))),
            ("module".into(), ritsu_base::json::Json::str(c.module.name.text.clone())),
            ("function".into(), ritsu_base::json::Json::str(info.name.text.clone())),
            ("input".into(), input.clone()),
            ("calls".into(), ritsu_base::json::Json::arr(it.trace.calls().map(|x| x.json.clone()))),
        ];
        if it.lets {
            let lets = it.trace.events.iter().filter_map(|e| match e {
                Event::Let(b) => Some(ritsu_base::json::Json::obj([
                    ("file", ritsu_base::json::Json::str(rooted(&root, &b.file))),
                    ("module", ritsu_base::json::Json::str(b.module.clone())),
                    ("function", ritsu_base::json::Json::str(b.function.clone())),
                    ("line", ritsu_base::json::Json::int(b.line as i128)),
                    ("let", ritsu_base::json::Json::str(b.name.clone())),
                    ("value", ritsu_base::json::Json::str(b.text.clone())),
                ])),
                Event::Call(_) => None,
            });
            fields.push(("lets".into(), ritsu_base::json::Json::arr(lets)));
        }
        match &outcome {
            Ok(v) => fields.push(("result".into(), p.to_json(v, &info.ret.ty))),
            Err(Stop::Failed(fl)) => fields.push((
                "failure".into(),
                ritsu_base::json::Json::obj([
                    ("kind", ritsu_base::json::Json::str(fl.kind.word())),
                    ("file", ritsu_base::json::Json::str(rooted(&root, &fl.file))),
                    ("module", ritsu_base::json::Json::str(fl.module.clone())),
                    ("line", ritsu_base::json::Json::int(fl.line as i128)),
                    ("message", ritsu_base::json::Json::str(ritsu_base::text::spaced(&fl.message, lang))),
                ]),
            )),
            Err(Stop::Diverged(d)) => fields.push((
                "diverged".into(),
                ritsu_base::json::Json::obj([("function", ritsu_base::json::Json::str(d.function.clone())), ("module", ritsu_base::json::Json::str(d.module.clone())), ("line", ritsu_base::json::Json::int(d.line as i128)), ("depth", ritsu_base::json::Json::int(d.depth as i128))]),
            )),
        }
        let _ = writeln!(out, "{}", ritsu_base::json::Json::Obj(fields).pretty());
        return if diverged { 1 } else { 0 };
    }
    // the call as the module would write it, then what it went through, then how it ended
    if let Ok(args) = &entered {
        let shown: Vec<String> = info.params.iter().zip(args).map(|(prm, v)| format!("{}: {}", prm.name.text, p.show(&c, v, &prm.slot.ty))).collect();
        let _ = writeln!(out, "{}({})", info.name.text, shown.join(", "));
    }
    for e in &it.trace.events {
        let line = match e {
            Event::Call(x) => format!("{}: {}", place(&root, &c.path, &x.file, x.line, lang), x.text),
            Event::Let(b) => format!("{}: let {} = {}", place(&root, &c.path, &b.file, b.line, lang), b.name, b.text),
        };
        let _ = writeln!(out, "  {line}");
    }
    match &outcome {
        Ok(v) => {
            let _ = writeln!(out, "= {}", p.show(&c, v, &info.ret.ty));
        }
        Err(Stop::Failed(fl)) => {
            let (kind, at, msg) = (fl.kind.word(), place(&root, &c.path, &fl.file, fl.line, lang), ritsu_base::text::spaced(&fl.message, lang));
            let _ = match lang {
                Lang::Ja => writeln!(out, "失敗: {kind}（{at}）: {msg}"),
                Lang::En => writeln!(out, "fails: {kind} ({at}): {msg}"),
            };
        }
        Err(Stop::Diverged(d)) => {
            let (n, fun, at) = (d.depth, &d.function, d.line);
            let _ = match lang {
                Lang::Ja => writeln!(out, "止まりません: 関数の呼び出しの深さが {n} を超えました（`{fun}` を {at} 行目で呼んだところ）"),
                Lang::En => writeln!(out, "does not stop: the calls of functions go deeper than {n} (`{fun}`, called on line {at})"),
            };
        }
    }
    if diverged { 1 } else { 0 }
}

fn vectors_cmd(a: &Args, lang: Lang, suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let [f] = a.pos.as_slice() else {
        return refuse(err, tr!("`kaname vectors` には .kn のファイルを一つ渡してください", "`kaname vectors` takes one .kn file"), lang);
    };
    let c = match passing(f, tr!("`{f}` は検査を通らないので、ベクタを出しません", "`{f}` does not pass check, so it has no vectors"), suite, lang, out, err) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let only = match a.get("--function") {
        None => None,
        Some(w) => match function(&c, f, w, lang, err) {
            Ok(i) => Some(i),
            Err(code) => return code,
        },
    };
    let p = crate::value::Program::of(&c);
    for fi in 0..c.decls.fns.len() {
        if only.is_some_and(|o| o != fi) {
            continue;
        }
        let (vs, cut) = crate::vectors::choose(&p, &c, fi);
        for v in &vs {
            let _ = writeln!(out, "{}", crate::vectors::line(&p, suite, &c, fi, v, a.has("--trace"), lang).compact());
        }
        if cut {
            let (fun, most) = (&c.decls.fns[fi].name.text, crate::vectors::MOST);
            let head = if lang == Lang::Ja { "注意" } else { "note" };
            let msg = tr!("関数 `{fun}` の入力は {most} 件を超えるので、最初の {most} 件だけを書きました", "the inputs of `{fun}` are more than {most}; the first {most} are written");
            let _ = writeln!(err, "{head}: {}", msg.get(lang));
        }
    }
    0
}

/// `api`: the module as the tools outside read it (DESIGN 11.1), for a module whose names and types
/// pass, whatever else its check finds.
fn api_cmd(a: &Args, lang: Lang, suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let [f] = a.pos.as_slice() else {
        return refuse(err, tr!("`kaname api` には .kn のファイルを一つ渡してください", "`kaname api` takes one .kn file"), lang);
    };
    let root = match root(a, f, lang, err) {
        Ok(r) => r,
        Err(code) => return code,
    };
    let o = match Run::new(suite.clone()).file(f) {
        Ok(o) => o,
        Err(e) => return refuse(err, tr!("`{f}` を読めません: {e}", "cannot read `{f}`: {e}"), lang),
    };
    if o.unjoined() {
        let _ = write!(out, "{}", check::render(&o, lang));
        return 2;
    }
    match &o.analysed {
        Some(c) => {
            let _ = writeln!(out, "{}", crate::api::json(c, &root).pretty());
            0
        }
        None => {
            let _ = write!(out, "{}", check::render(&o, lang));
            let head = if lang == Lang::Ja { "エラー" } else { "error" };
            let msg = tr!("`{f}` の名前か型が検査を通らないので、api を出しません", "the names or the types of `{f}` do not pass check, so it has no api");
            let _ = writeln!(err, "{head}: {}", msg.get(lang));
            1
        }
    }
}

/// `doc`: the page for people (DESIGN 13), for a module whose names and types pass, whatever else
/// its check finds: the page shows where its functions can fail.
fn doc_cmd(a: &Args, lang: Lang, suite: &Suite, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let [f] = a.pos.as_slice() else {
        return refuse(err, tr!("`kaname doc` には .kn のファイルを一つ渡してください", "`kaname doc` takes one .kn file"), lang);
    };
    let root = match root(a, f, lang, err) {
        Ok(r) => r,
        Err(code) => return code,
    };
    let format = if a.get("--format") == Some("html") { crate::doc::Format::Html } else { crate::doc::Format::Markdown };
    let o = match Run::new(suite.clone()).file(f) {
        Ok(o) => o,
        Err(e) => return refuse(err, tr!("`{f}` を読めません: {e}", "cannot read `{f}`: {e}"), lang),
    };
    if o.unjoined() {
        let _ = write!(out, "{}", check::render(&o, lang));
        return 2;
    }
    match crate::doc::page(&o, suite, &root, lang, format) {
        Some(p) => {
            let _ = write!(out, "{}", crate::doc::write(&p, format));
            0
        }
        None => {
            let _ = write!(out, "{}", check::render(&o, lang));
            let head = if lang == Lang::Ja { "エラー" } else { "error" };
            let msg = tr!("`{f}` の名前か型が検査を通らないので、ページを書きません", "the names or the types of `{f}` do not pass check, so it has no page");
            let _ = writeln!(err, "{head}: {}", msg.get(lang));
            1
        }
    }
}
