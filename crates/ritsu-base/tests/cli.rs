//! The command-line table (DESIGN 4.4): `--help` drawn as koyomi and yuen drew theirs, letter
//! for letter (the commands here are copied from their tables, and what they print is held to
//! what the two printed, in `tests/golden/compat/`), and a command line read against it.

use ritsu_base::cli::{Args, Cmd, Flag, Misuse, Reading, Table, flag, help_flag, lang_flag};
use ritsu_base::text::{Lang, Text};
use ritsu_base::tr;
use std::path::Path;

const PATHS: (&str, &str) = (
    ".req のファイル。ディレクトリなら下の .req を全部（パスの順）。渡したものが一つのプロジェクト",
    ".req files; a directory stands for every .req under it, in path order. What is given is one project",
);

fn yuen_globals() -> Vec<Flag> {
    vec![
        flag("--lang", Some("ja|en"), tr!("文面の言語。無ければ環境変数 YUEN_LANG、それも無ければ en", "the language of the text; else the YUEN_LANG environment variable, else en"))
            .choices(&["ja", "en"])
            .default("en"),
        flag(
            "--root",
            Some("<dir>"),
            tr!(
                "パスを読むルート。無ければ、最初に渡したパスの上で .git を持つ一番近いディレクトリ（無ければ渡したディレクトリ）",
                "the root paths are read from; without it, the nearest directory above the first path given that has a .git (else the directory given)"
            ),
        ),
        flag("--help", None, tr!("この画面を出す", "print this page")),
    ]
}

fn yuen() -> Table {
    Table {
        tool: "yuen",
        version: "0.1.0",
        summary: tr!("要件の出どころを書く。満たすものにつなぐ。どこかが変われば止める。", "Write where each requirement comes from. Link what meets it. Stop when anything moves."),
        globals: yuen_globals(),
        commands: vec![Cmd {
            name: "review",
            usage: None,
            args: "<path>...",
            purpose: tr!(
                "人が確かめたことを書く。選んだリンクと見送りのうち印の付いたものに、確かめた記録（日付、役割、両端のハッシュ）を書き、確かめたときの中身を reviewed/ に置く",
                "record what a person has looked at: under each chosen link and waiver that is marked, write who looked, when, and the hashes of its ends, and keep what was looked at in reviewed/"
            ),
            params: vec![("<path>...", tr!("{}", "{}", PATHS.0; PATHS.1))],
            flags: vec![
                flag("--at", Some("<file.req>:<line>"), tr!("その行のリンクか見送り。記録の行の番号でもよく、check の診断の位置をそのまま渡せる", "the link or waiver on that line, or on the line of its record: the place a diagnostic of check gives")).repeats(),
                flag("--requirement", Some("'<name>[ v<n>]'"), tr!("その要件の、印の付いたリンクと見送りの全部（名前か別名）", "every marked link and waiver of that requirement (its name or alias)")).repeats(),
                flag("--all", None, tr!("プロジェクトの、印の付いたリンクと見送りの全部", "every marked link and waiver of the project")),
                flag("--by", Some("<role>"), tr!("確かめた人の役割（宣言した役割）。要る", "the role of whoever looked (a declared role); required")),
                flag("--date", Some("<YYYY-MM-DD>"), tr!("確かめた日。無ければその日（その機械のタイムゾーン）", "the day it was looked at; without it, today in the machine's time zone")),
            ],
            exits: vec![
                (0, tr!("書いた（印の付いたものが無く、何も書かなかったときも）", "written (also when nothing chosen was marked, and nothing was written)")),
                (1, tr!("書けないものがある（両端を読めないリンク、構文と名前のエラー）", "something could not be written (a link whose ends cannot be made, errors in the words or names)")),
                (2, tr!("引数の誤り、読めないファイル、書けないファイル", "bad arguments, or a file that cannot be read or written")),
            ],
            examples: vec![
                "yuen review tests/fixtures/period --all --by 法務 --date 2026-10-04",
                "yuen review tests/fixtures/period --at tests/fixtures/period/民法の期間.req:21 --by 開発",
            ],
            codes: vec![],
        }],
        footer: vec![],
        reading: Reading::default(),
    }
}

fn cmd(name: &'static str, args: &'static str, purpose: Text) -> Cmd {
    Cmd { name, usage: None, args, purpose, params: vec![], flags: vec![], exits: vec![], examples: vec![], codes: vec![] }
}

fn koyomi() -> Table {
    Table {
        tool: "koyomi",
        version: "0.1.0",
        summary: tr!(
            "締めと支払、営業日、月の足し算を書く小さな言語。書いた条件を範囲のすべての日で確かめる。",
            "A small language for closing days, payment days, business days and month arithmetic, checked on every day of its range."
        ),
        globals: vec![lang_flag("KOYOMI_LANG"), help_flag()],
        commands: vec![
        cmd("check", "<file.cal>...", tr!(
                "検査する。範囲のすべての入力で計算し、条件と例を確かめる",
                "compute on every input of the range, and hold the claims and the examples to it"
            )),
        cmd("eval", "<file.cal> <name>=<value>...", tr!(
                "一つの入力ですべての日付を計算し、計算の段を見せる。カレンダーなら、その日が営業日か休みか",
                "compute every date on one input and show each step; for a calendar, whether a day is open or closed"
            )),
        cmd("gen", "<file.cal>...", tr!(
                "TypeScript・Python・Go・Rust・SQL のコードとランナーを生成する。検査を通らないファイルからは生成しない",
                "generate TypeScript, Python, Go, Rust and SQL, each with a runner; nothing is generated from a file that does not pass check"
            )),
        cmd("vectors", "<file.cal>", tr!(
                "範囲のすべての入力について、参照インタプリタの結果を JSON Lines で出す（範囲の両端の外の行も）",
                "print the reference interpreter's result for every input of the range as JSON Lines, with the inputs just outside it"
            )),
        cmd("doc", "<file.cal>", tr!(
                "承認する人（経理、法務、会社のカレンダーを決める人）が読むページを出す。Markdown か、一枚の HTML",
                "print the page for whoever approves the file (accounting, legal, whoever keeps the calendar): Markdown, or one HTML file"
            )),
        cmd("api", "<file.cal>", tr!(
                "関数・引数・カレンダー・データの範囲・出典のハッシュを JSON で出す（ほかのツールが読む形）",
                "print the functions, their inputs, the calendar, the data range and the sources' digests as JSON, for other tools"
            )),
        cmd("source", "fetch|pin|outdated <file.cal>", tr!(
                "出典の写しを扱う。fetch は写しを取ってきて .cal の隣に置き、pin は写しのハッシュを .cal に書き、outdated は元が変わったかを問う",
                "handle the copies of the sources: fetch brings them beside the .cal, pin writes their digests into it, outdated asks whether the originals moved on"
            )),
        cmd("explain", "<CODE>", tr!("診断のコードを引く。いつ出るか、どう直すか、最小の再現", "look a diagnostic code up: when it comes, how to fix it, the smallest reproduction")),
        ],
        footer: vec![
            tr!(
                "どのコマンドにも --lang ja|en を付けられます（既定は en。環境変数 KOYOMI_LANG でも指定できます）。",
                "Every command takes --lang ja|en (default en; the KOYOMI_LANG environment variable works too)."
            ),
            tr!("exit code: 0 エラーなし / 1 エラーあり / 2 引数の誤りか、読めないファイル", "Exit codes: 0 no errors / 1 errors / 2 bad arguments or a file that cannot be read"),
        ],
        reading: Reading { negative_numbers: true, ..Reading::default() },
    }
}

fn golden(name: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/compat").join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

#[test]
fn help_is_drawn_as_koyomi_and_yuen_drew_it() {
    let mut failures = Vec::new();
    for lang in [Lang::En, Lang::Ja] {
        let y = yuen();
        let got = y.help_cmd(y.command("review").unwrap(), lang);
        let name = format!("yuen-review-help.{}.txt", lang.code());
        if got != golden(&name) {
            failures.push(format!("{name}:\n{}", ritsu_testkit::golden::line_diff(&golden(&name), &got)));
        }
        let got = koyomi().help_all(lang);
        let name = format!("koyomi-help.{}.txt", lang.code());
        if got != golden(&name) {
            failures.push(format!("{name}:\n{}", ritsu_testkit::golden::line_diff(&golden(&name), &got)));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn args(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn read(t: &Table, v: &[&str]) -> Result<Args, Text> {
    t.parse(t.command("review").or_else(|| t.command("check")).unwrap(), &args(v))
}

#[test]
fn what_stops_a_command_line() {
    let y = yuen();
    let e = read(&y, &["p", "--fromat", "json"]).unwrap_err();
    assert_eq!(e.en, "unknown flag `--fromat`; run `yuen review --help`");
    assert_eq!(e.ja, "`--fromat` というフラグはありません。`yuen review --help` を読んでください");
    assert_eq!(read(&y, &["p", "--lang", "fr"]).unwrap_err().en, "`--lang fr` is not a value this flag takes; it takes only ja | en");
    assert_eq!(read(&y, &["p", "--by"]).unwrap_err().en, "`--by <role>` is missing its value");
    assert_eq!(read(&y, &["p", "--by", "a", "--by", "b"]).unwrap_err().en, "`--by` is given twice");
    assert_eq!(read(&y, &["p", "--all=yes"]).unwrap_err().en, "`--all` takes no value (it was given `=yes`)");
}

#[test]
fn what_a_command_line_says() {
    let y = yuen();
    let a = read(&y, &["p", "q", "--at", "x:1", "--at=x:2", "--by", "法務", "-h", "-"]).unwrap();
    assert_eq!(a.pos, ["p", "q", "-"]);
    assert_eq!(a.all("--at"), ["x:1", "x:2"], "a flag the table says repeats");
    assert_eq!(a.get("--by"), Some("法務"));
    assert!(a.has("--help"), "-h is --help");
    assert!(!a.has("--all"));
    // yuen reads `-5` as a flag, koyomi as an argument; sakai takes no value that starts with `--`.
    assert!(read(&y, &["-5"]).is_err());
    let k = koyomi();
    let mut k2 = k;
    k2.commands = vec![Cmd { flags: vec![flag("--budget", Some("<n>"), Text::same("n"))], ..cmd("check", "<file.cal>...", Text::same("check")) }];
    assert_eq!(read(&k2, &["-5"]).unwrap().pos, ["-5"]);
    assert_eq!(read(&k2, &["--budget", "--lang"]).unwrap().get("--budget"), Some("--lang"));
    k2.reading.no_dashes_in_values = true;
    assert_eq!(read(&k2, &["--budget", "--lang"]).unwrap_err().en, "`--budget <n>` is missing its value");
    // chobo takes no `--flag=value`, and reads anything that does not start with `--` as an argument.
    let mut c = koyomi();
    c.commands = vec![Cmd { flags: vec![flag("--format", Some("json"), Text::same("f")).choices(&["json"])], ..cmd("check", "<file.book>...", Text::same("check")) }];
    c.reading = Reading { no_dashes_in_values: true, no_inline_values: true, single_dash_args: true, ..Reading::default() };
    let check = c.command("check").unwrap();
    assert_eq!(c.read(check, &args(&["--format=json"])), Err(Misuse::UnknownFlag("--format=json".into())));
    assert_eq!(c.read(check, &args(&["-x", "-5", "-"])).unwrap().pos, ["-x", "-5", "-"]);
    assert_eq!(c.read(check, &args(&["--format", "yaml"])), Err(Misuse::NotAChoice { flag: "--format", value: "yaml".into(), choices: &["json"] }));
    assert_eq!(c.read(check, &args(&["--format"])), Err(Misuse::MissingValue { flag: "--format", placeholder: "json" }));
    assert_eq!(c.read(check, &args(&["--format", "json", "--format", "json"])), Err(Misuse::Twice("--format")));
    assert_eq!(Misuse::Twice("--format").text("chobo", "check").ja, "`--format` が二度書かれています");
}

#[test]
fn the_usage_line_and_a_flag_spelled() {
    let y = yuen();
    assert_eq!(
        y.usage_line(y.command("review").unwrap()),
        "yuen review <path>... [--at <file.req>:<line>...] [--requirement '<name>[ v<n>]'...] [--all] [--by <role>] [--date <YYYY-MM-DD>]"
    );
    let mut explain = cmd("explain", "<code>", Text::same("explain"));
    explain.usage = Some("chobo explain <code> | --all [--format markdown]");
    assert_eq!(y.usage_line(&explain), "chobo explain <code> | --all [--format markdown]", "a line written by hand is the line");
    assert_eq!(flag("--map", Some("<spec>"), Text::default()).repeats().spelled(), "--map <spec>...");
    assert_eq!(help_flag().spelled(), "--help");
}
