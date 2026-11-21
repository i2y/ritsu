//! The `dandori` command: [`run`] takes the words of a command line, the port the rules are read
//! through, and where to print, and answers the exit code (ritsu's DESIGN 3.3). The dandori binary
//! of this crate runs it with a port that reads no rule; `ritsu dandori` runs it with rulec's.
//!
//! The commands and their flags are in one table (ritsu's DESIGN 4.4): `--help` is drawn from it,
//! each command's page too (`dandori <cmd> --help`), and the command line is read against it.
//! An unknown flag, a value outside a closed set, a flag without its value and a flag given
//! twice stop the run with exit 2, so a flag cannot be taken and quietly do nothing. The table
//! is dandori's; how it is drawn and read is ritsu-base's ([`ritsu_base::cli`]).

use crate::check;
use crate::commands::{self, RENAMED, TARGETS};
use crate::diag::{self, Lang, Text};
use ritsu_base::cli::{flag, help_flag, lang_flag, made_lang_flag, Cmd, Flag, Reading, Table};
use ritsu_ports::Rules;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;

pub use ritsu_base::cli::Args;

/// What `run --target` takes: the reference interpreter's own view, and the platforms'.
pub const RUN_TARGETS: &[&str] = &["reference", "asl", "temporal-typescript", "temporal-python", "temporal-go", "durable", "argo", "pydantic-graph"];

pub fn global_flags() -> Vec<Flag> {
    vec![lang_flag("DANDORI_LANG"), help_flag()]
}

/// `--format json|text`: how the diagnostics of the check every command runs first are printed.
fn diagnostics_format() -> Flag {
    flag("--format", Some("json|text"), tr!("診断の出し方。json はツール向けの JSON", "how the diagnostics are printed; json is the machine-facing JSON"))
        .choices(&["json", "text"])
        .default("text")
}

fn exits(done: ritsu_base::text::Text) -> Vec<(u8, ritsu_base::text::Text)> {
    exits_with(done, tr!("エラーがある", "errors found"))
}

/// The same, with what 1 says.
fn exits_with(done: ritsu_base::text::Text, one: ritsu_base::text::Text) -> Vec<(u8, ritsu_base::text::Text)> {
    vec![
        (0, done),
        (1, one),
        (2, tr!("引数が正しくないか、ファイルが読めない。規則、日付のファイル、帳簿を使うフローを、それらを読めないこの dandori で走らせた（E018）", "bad arguments, or a file that cannot be read; a flow that uses rules, dates files or books, run with this dandori, which reads none of them (E018)")),
    ]
}

pub fn commands() -> Vec<Cmd> {
    vec![
        Cmd {
            usage: None,
            name: "check",
            args: "<file.flow>...",
            purpose: tr!(
                "型、すべての分岐、案件が残りうる状態、リトライを検査する",
                "check: types, every arm, every state a case can be left in, retries"
            ),
            params: vec![("<file.flow>...", tr!("検査する .flow のファイル", "the .flow files to check"))],
            flags: vec![diagnostics_format()],
            exits: exits(tr!("エラーなし（警告だけか、何も無い）", "no errors (warnings only, or nothing)")),
            examples: vec!["dandori check examples/hotel/temporal/hotel.flow", "dandori check examples/hotel/temporal/hotel.ja.flow --format json --lang ja"],
            codes: vec![],
        },
        Cmd {
            usage: Some("dandori gen <file.flow> --target asl|temporal-typescript|temporal-python|temporal-go|durable|argo|pydantic-graph... [--out <dir>] [--check] [--format json|text]"),
            name: "gen",
            args: "<file.flow> --target <target>...",
            purpose: tr!(
                "AWS Step Functions（ASL、JSONata）、Temporal（TypeScript・Python・Go）、AWS Lambda durable functions（TypeScript）、Argo Workflows（YAML）、pydantic-graph（Python）向けのコードを、指定したプラットフォームごとに書き出す。プラットフォームにできないことと、一回の実行が履歴の上限を超えうるワークフローはエラーにする",
                "compile to AWS Step Functions (ASL, JSONata), to Temporal (TypeScript, Python or Go), to AWS Lambda durable functions (TypeScript), to Argo Workflows (YAML), or to pydantic-graph (Python), for each platform given; it also refuses what a platform cannot do, and a run that can outgrow its history"
            ),
            params: vec![("<file.flow>", tr!("書き出す .flow のファイル", "the .flow file to compile"))],
            flags: vec![
                flag("--target", Some("<target>"), tr!("書き出すプラットフォーム。`<out>/<target>/` に書く", "a platform to write for, into <out>/<target>/")).choices(&TARGETS).once_each().renamed(&RENAMED),
                flag("--out", Some("<dir>"), tr!("書き出す先", "where the files are written")).default("generated"),
                flag("--check", None, tr!("何も書かない。同じ --target で書くはずのファイルが無いか、ディスクのものと違えば 1 で終わる（CI 用）", "write nothing; exit 1 if a file the same --target write is missing or differs from the one on disk (for CI)")),
                diagnostics_format(),
            ],
            exits: vec![
                (0, tr!("書き出した。--check では、全部が同じ", "written, or under --check all the same")),
                (1, tr!("エラーがある。指定したプラットフォームのどれかがフローを受け取れない（何も書かない）。--check で、無いか違うファイルがある", "errors found, a platform given cannot take the flow (nothing is written), or under --check a file is missing or differs")),
                (2, tr!("引数が正しくない（`--target` が無い、0.40.0 の名前 `temporal` など）か、ファイルが読めない。規則、日付のファイル、帳簿を使うフローを、それらを読めないこの dandori で走らせた（E018）", "bad arguments (no `--target`, or the name `temporal` of 0.40.0, among them), or a file that cannot be read; a flow that uses rules, dates files or books, run with this dandori, which reads none of them (E018)")),
            ],
            examples: vec!["dandori gen examples/hotel/temporal/hotel.flow --target temporal-typescript", "dandori gen examples/review/aws/review.flow --target asl --target durable --check"],
            codes: vec![],
        },
        Cmd {
            usage: Some("dandori run <file.flow> --scenario <file.json> [--target reference|asl|temporal-typescript|temporal-python|temporal-go|durable|argo|pydantic-graph] [--format json|text]"),
            name: "run",
            args: "<file.flow> --scenario <file.json>",
            purpose: tr!(
                "シナリオに書いた結果を使って参照インタプリタでワークフローを動かし、トレースをターゲットでの形で出す",
                "run the workflow in the reference interpreter against scripted answers, and print the trace as the target would show it"
            ),
            params: vec![("<file.flow>", tr!("動かす .flow のファイル", "the .flow file to run"))],
            flags: vec![
                flag("--scenario", Some("<file.json>"), tr!("動かすシナリオ（必ず書く）", "the scenario to play (required)")),
                flag("--target", Some("<target>"), tr!("呼び出しをどのターゲットの形で出すか", "the target whose calls the trace shows")).choices(RUN_TARGETS).default("reference").renamed(&RENAMED),
                diagnostics_format(),
            ],
            exits: exits_with(
                tr!("最後まで動き、シナリオの結果を全部使った", "the run went to its end and took every answer of the scenario"),
                tr!(
                    "エラーがある。流れが途中で止まった（シナリオに結果が足りないなど）か、シナリオの結果を使い残した（流れは最後まで出す）",
                    "errors found; or the run could not go on (the scenario ran out of answers, say), or did not take every answer of the scenario (the run is printed to its end)"
                ),
            ),
            examples: vec!["dandori run examples/hotel/temporal/hotel.flow --scenario scenarios/001.json --target temporal-typescript"],
            codes: vec![],
        },
        Cmd {
            usage: None,
            name: "scenarios",
            args: "<file.flow>",
            purpose: tr!(
                "すべての分岐と、案件の状態のすべての変わり方を通るシナリオを書き出す",
                "write scenarios that take every arm and every way a case can move"
            ),
            params: vec![("<file.flow>", tr!("シナリオを作る .flow のファイル", "the .flow file to write scenarios for"))],
            flags: vec![
                flag("--out", Some("<dir>"), tr!("シナリオを一つずつそこに書く。無ければ全部を JSON で標準出力に出す", "write one file per scenario there; without it, all of them as JSON on standard output")),
                diagnostics_format(),
            ],
            exits: exits(tr!("書き出した", "written")),
            examples: vec!["dandori scenarios examples/hotel/temporal/hotel.flow --out scenarios"],
            codes: vec![],
        },
        Cmd {
            usage: None,
            name: "doc",
            args: "<file.flow>",
            purpose: tr!(
                "レビューする人のためにワークフローを図にする。流れ、各呼び出しのすることとエラーの行き先、終わり方のすべて。Mermaid の図を入れた Markdown か、シナリオごとに通るところが光る HTML のページ一枚",
                "draw the workflow for the person who reviews it: the flow, what each call does and where its errors go, every way it can end; Markdown with Mermaid, or one HTML page where each scenario lights up the way it goes"
            ),
            params: vec![("<file.flow>", tr!("図にする .flow のファイル", "the .flow file to draw"))],
            flags: vec![
                flag("--format", Some("html|md"), tr!("html は HTML のページ一枚。md は Markdown", "html is one HTML page; md is Markdown")).choices(&["html", "md"]).default("md"),
                flag("--out", Some("<dir>"), tr!("そこに `<名前>.md` か `<名前>.html` を書く。無ければ標準出力に出す", "write `<name>.md` or `<name>.html` there; without it, on standard output")),
                made_lang_flag("DANDORI_LANG"),
            ],
            exits: exits(tr!("ページを出した", "the page is out")),
            examples: vec!["dandori doc examples/hotel/temporal/hotel.flow --format html --out site"],
            codes: vec![],
        },
        Cmd {
            usage: None,
            name: "api",
            args: "<file.flow>",
            purpose: tr!(
                "ワークフローが宣言していることを、ソースを読まずに知る。入力と出力（型と範囲）、読むファイル、タスクの呼び先・期限・再試行・エラー、案件の追う状態機械、終わり方（`fail` の名前と `succeed` の値）、実装するサービスのメソッド、子のフローを JSON で出す",
                "know what the workflow declares without reading the source: its inputs and outputs with their types and ranges, the files it reads, each task's callee, deadline, retries and errors, the machines its cases follow, every way it ends (the names of its `fail`s and the values of its `succeed`s), the methods of the service it implements, and its child flows, as JSON"
            ),
            params: vec![("<file.flow>", tr!("検査を通る .flow のファイル", "a .flow file that passes check"))],
            flags: vec![diagnostics_format()],
            exits: exits(tr!("JSON を出した", "the JSON is out")),
            examples: vec!["dandori api examples/hotel/temporal/hotel.flow", "dandori api examples/review/aws/review.flow | jq '.tasks[].name'"],
            codes: vec![],
        },
        Cmd {
            usage: Some("dandori explain <CODE>... | --all [--format markdown|json]"),
            name: "explain",
            args: "<CODE>... | --all",
            purpose: tr!("診断のコードを引く。いつ出るか、どう直すか、最小の再現", "look diagnostic codes up: when each comes, how to fix it, the smallest example"),
            params: vec![("<CODE>...", tr!("`E014` のような診断のコード。いくつでも渡せて、渡した順に出す。`--all` とは一緒に渡さない", "diagnostic codes such as `E014`, any number, printed in the order given; not given with `--all`"))],
            flags: vec![
                flag("--all", None, tr!("全部のコードを出す", "print every code")),
                flag("--format", Some("markdown|json"), tr!("Markdown か、ツール向けの JSON で出す", "print Markdown, or the machine-facing JSON")).choices(&["markdown", "json"]),
            ],
            exits: vec![(0, tr!("引けた", "found")), (2, tr!("無いコードがあるか、引数の誤り", "a code it does not have, or bad arguments"))],
            examples: vec!["dandori explain E014", "dandori explain --all --format markdown --lang ja"],
            codes: vec![],
        },
    ]
}

/// The whole table: the commands, the flags every command takes, and the lines at the end of
/// `dandori --help`.
pub fn table() -> Table {
    Table {
        tool: "dandori",
        version: env!("CARGO_PKG_VERSION"),
        summary: tr!("規則を呼ぶワークフローの、小さな型付き言語。", "A small typed language for workflows that call business rules."),
        globals: global_flags(),
        commands: commands(),
        footer: vec![
            tr!(
                "どのコマンドにも --lang ja|en を付けられます（既定は en。環境変数 DANDORI_LANG か RITSU_LANG は出す文の言語を決め、doc が書くページは --lang だけで決まります）。",
                "Every command takes --lang ja|en (default en; the DANDORI_LANG or RITSU_LANG environment variable sets the language of the text printed, and the page doc writes follows --lang alone)."
            ),
            tr!(
                "規則、日付のファイル、帳簿（`use rule`、`use dates`、`use book`）を使うワークフローは、`ritsu dandori` で走らせてください。`ritsu dandori` は、それらを同じプロセスの中で読みます。",
                "A workflow that uses rules, dates files or books (`use rule`, `use dates`, `use book`) runs as `ritsu dandori`, which reads them in the same process."
            ),
            tr!(
                "exit code: 0 エラーなし / 1 エラーあり / 2 引数の誤りか読めないファイル、または規則・日付のファイル・帳簿を使うフローを、それらを読めないこの dandori で走らせた（E018）",
                "Exit codes: 0 notes only / 1 errors found / 2 bad arguments or an unreadable file, or a flow that uses rules, dates files or books run with this dandori, which reads none of them (E018)"
            ),
        ],
        reading: Reading::default(),
    }
}

/// What one command line asked for, as the commands below read it.
struct Asked {
    files: Vec<PathBuf>,
    format_json: bool,
    format_html: bool,
    lang: Lang,
    /// The language of what the command writes for others to keep (a page): `--lang`, else English,
    /// whatever the environment says (ritsu's DESIGN 4.1).
    made: Lang,
    target: Option<String>,
    /// every `--target` of `gen`, as given
    targets: Vec<String>,
    check: bool,
    out: Option<PathBuf>,
    scenario: Option<PathBuf>,
}

impl Asked {
    /// The command line read once the run's way of writing paths is set (ritsu's DESIGN 4.15): the
    /// flows given, and the paths `--out` and `--scenario` take, as the run writes them, so that
    /// `./order.flow` and `x/../order.flow` are `order.flow` (6.2 item 8), and `--out ./generated`
    /// is `generated`.
    fn from(a: &Args, lang: Lang) -> Asked {
        let held = |p: &str| PathBuf::from(ritsu_base::paths::fold_given(p));
        Asked {
            files: a.pos.iter().map(|p| PathBuf::from(ritsu_base::paths::show_given(p))).collect(),
            format_json: a.get("--format") == Some("json"),
            format_html: a.get("--format") == Some("html"),
            lang,
            made: Lang::made(a.get("--lang")),
            target: a.get("--target").map(str::to_string),
            targets: a.all("--target").into_iter().map(str::to_string).collect(),
            check: a.has("--check"),
            out: a.get("--out").map(held),
            scenario: a.get("--scenario").map(held),
        }
    }
}

/// What stops the command, as every command of ritsu says it (ritsu's DESIGN 6.2, item 10), and 2.
fn refuse(msg: Text, lang: Lang, err: &mut dyn Write) -> u8 {
    ritsu_base::cli::refuse(err, &msg, lang)
}

/// `--lang ja`, `--lang=ja`, anywhere on the line: decided before anything is printed.
fn lang_asked(args: &[String]) -> Option<String> {
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

/// The `dandori` command, run on `args` (the words after the program's name), reading the rules
/// through `rules`, printing to `out` and `err`; the exit code. The dandori binary of this crate
/// hands it a port that reads no rule (crate::sources::NoRules); `ritsu dandori` and the tests hand
/// it rulec's own answer (ritsu's DESIGN 3.3).
pub fn run(args: &[String], rules: Rc<dyn Rules>, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    COMMAND.with(|c| *c.borrow_mut() = Some(args.to_vec()));
    let code = crate::sources::with_rules(rules, || run_here(args, out, err));
    COMMAND.with(|c| *c.borrow_mut() = None);
    code
}

thread_local! {
    /// The command line `run` was given, the words after the program's name, to say again with
    /// `ritsu dandori` in front when rulec is not joined (E018).
    static COMMAND: std::cell::RefCell<Option<Vec<String>>> = const { std::cell::RefCell::new(None) };
}

/// A word as a shell reads it back: as it is, or in single quotes.
fn shell_word(w: &str) -> String {
    if !w.is_empty() && w.chars().all(|c| !c.is_whitespace() && !"'\"\\$`;&|<>()*?[]#~".contains(c)) {
        w.to_string()
    } else {
        format!("'{}'", w.replace('\'', "'\\''"))
    }
}

/// The command to run instead, where rulec is not joined: the one given, with `ritsu dandori` in
/// front (`ritsu dandori …` when no command line is known, as for the library).
pub fn with_ritsu() -> String {
    match COMMAND.with(|c| c.borrow().clone()) {
        Some(words) => {
            let shown: Vec<String> = words.iter().map(|w| shell_word(w)).collect();
            format!("ritsu dandori {}", shown.join(" "))
        }
        None => "ritsu dandori …".to_string(),
    }
}

/// The `dandori` command as `run_with_ports`, with what the checks across the borders could not
/// decide of a flow's calls of rules (`undecided`, ritsu-cross's answer to ritsu's X2), which
/// `gen`, `run`, `scenarios` and `doc` put into the flow as checks the code makes when the
/// workflow runs (DESIGN 1.17): what `ritsu dandori` hands over.
pub fn run_with_undecided(args: &[String], rules: Rc<dyn Rules>, dates: Rc<dyn ritsu_ports::Dates>, books: Rc<dyn ritsu_ports::Books>, undecided: Rc<dyn ritsu_ports::Undecided>, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    COMMAND.with(|c| *c.borrow_mut() = Some(args.to_vec()));
    let code = crate::sources::with_undecided(rules, dates, books, undecided, || run_here(args, out, err));
    COMMAND.with(|c| *c.borrow_mut() = None);
    code
}

/// The `dandori` command as `run`, reading the dates files through `dates` (koyomi's answer) and
/// the books through `books` (chobo's) besides the rules: what a program that joins all three
/// hands over.
pub fn run_with_ports(args: &[String], rules: Rc<dyn Rules>, dates: Rc<dyn ritsu_ports::Dates>, books: Rc<dyn ritsu_ports::Books>, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    COMMAND.with(|c| *c.borrow_mut() = Some(args.to_vec()));
    let code = crate::sources::with_ports(rules, dates, books, || run_here(args, out, err));
    COMMAND.with(|c| *c.borrow_mut() = None);
    code
}

/// A line that asks for JSON is answered in JSON when the command stops before its work too: the
/// last value on standard output is `{"error": …}` (ritsu's DESIGN 6.2, item 10).
fn run_here(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    ritsu_base::cli::answering(args, out, |out| command(args, out, err))
}

fn command(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let lang = Lang::pick(lang_asked(args).as_deref(), "DANDORI_LANG");
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
            let _ = writeln!(out, "dandori {}", env!("CARGO_PKG_VERSION"));
            return 0;
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
                    let _ = write!(out, "{}", table.help_all(lang));
                    0
                }
                Some(n) => match table.command(n) {
                    Some(c) => {
                        let _ = write!(out, "{}", table.help_cmd(c, lang));
                        0
                    }
                    None if n == "build" => refuse(ritsu_base::cli::renamed("dandori build", "dandori gen", "0.41.0"), lang, err),
                    None => refuse(ritsu_base::cli::no_command("dandori", n), lang, err),
                },
            };
        }
        _ => {}
    }
    let Some(cmd) = table.command(first) else {
        // the verb up to 0.40.0 is not read as `gen` (DESIGN 21)
        if first == "build" {
            return refuse(ritsu_base::cli::renamed("dandori build", "dandori gen", "0.41.0"), lang, err);
        }
        return refuse(ritsu_base::cli::no_command("dandori", first), lang, err);
    };
    let parsed = match table.parse(cmd, &args[1..]) {
        Ok(a) => a,
        Err(e) => return refuse(e, lang, err),
    };
    if parsed.has("--help") {
        let _ = write!(out, "{}", table.help_cmd(cmd, lang));
        return 0;
    }
    // a path that starts with `@`, in a module, is read as a path written in a file is (ritsu's DESIGN 17.6)
    if let Err(t) = ritsu_base::modules::given_all(&parsed.pos) {
        return refuse(t, lang, err);
    }
    // the paths the run holds are written the way the first path given was (ritsu's DESIGN 4.15)
    let _way = ritsu_base::paths::run_from(parsed.pos.first().map_or(".", String::as_str));
    let a = Asked::from(&parsed, lang);
    match cmd.name {
        "check" => cmd_check(&a, out, err),
        "gen" => cmd_gen(&a, out, err),
        "run" => cmd_run(&a, out, err),
        "scenarios" => cmd_scenarios(&a, out, err),
        "doc" => cmd_doc(&a, out, err),
        "api" => cmd_api(&a, out, err),
        "explain" => cmd_explain(&parsed, lang, out, err),
        _ => unreachable!("every command in the table is dispatched"),
    }
}

/// `dandori explain`: the codes of the ledger (crate::codes) given, in the order given, or every one,
/// as text, Markdown or JSON.
fn cmd_explain(a: &Args, lang: Lang, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let ledger = crate::codes::ledger();
    let format = a.get("--format");
    if a.has("--all") {
        // codes and `--all` are the two ways to call it, not one: what it takes
        if !a.pos.is_empty() {
            return refuse(table().takes_of("explain"), lang, err);
        }
        match format {
            Some("markdown") => {
                let _ = write!(out, "{}", ledger.render_markdown(lang));
            }
            Some(_) => {
                let _ = writeln!(out, "{}", ritsu_base::json::Json::arr(ledger.entries.iter().map(|e| ledger.to_json(e, lang))).pretty());
            }
            None => {
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
    if a.pos.is_empty() {
        return refuse(table().takes_of("explain"), lang, err);
    }
    let entries = match ledger.look_up(&a.pos) {
        Ok(entries) => entries,
        Err(unknown) => return refuse(ritsu_base::cli::no_codes("dandori", &unknown), lang, err),
    };
    let shape = match format {
        Some("markdown") => ritsu_base::ledger::Shape::Markdown,
        Some(_) => ritsu_base::ledger::Shape::Json,
        None => ritsu_base::ledger::Shape::Text,
    };
    let _ = write!(out, "{}", ledger.explained(&entries, shape, lang));
    0
}

/// What stops a command given other files than it takes, or none: what it takes, and its usage line.
fn takes(name: &str, lang: Lang, err: &mut dyn Write) -> u8 {
    refuse(table().takes_of(name), lang, err)
}

/// What `gen`, `scenarios` and `doc` say of a directory of `--out` they cannot make.
fn cannot_create(dir: &Path, e: &std::io::Error) -> Text {
    ritsu_base::cli::cannot_create(&dir.display().to_string(), e)
}

/// What they say of a file they cannot write.
fn cannot_write(p: &Path, e: &std::io::Error) -> Text {
    ritsu_base::cli::cannot_write(&p.display().to_string(), e)
}

/// Check one file and print its diagnostics; the model when it passes. `print_ok` is `check`'s: it
/// says a file passes, and leaves the preconditions ritsu could not decide out of the model, which
/// the other commands put in as checks (DESIGN 1.17).
fn load(path: &Path, a: &Asked, print_ok: bool, out: &mut dyn Write, err: &mut dyn Write) -> Result<Option<crate::model::Model>, u8> {
    let read = if print_ok { check::check_kept(path) } else { check::built(path) };
    let (src, checked) = match read {
        Ok(x) => x,
        Err(msg) => return Err(refuse(msg, a.lang, err)),
    };
    let file = path.display().to_string();
    if a.format_json {
        let v: Vec<_> = checked.diags.iter().map(|d| d.to_json(a.lang)).collect();
        let _ = writeln!(out, "{}", serde_json::to_string_pretty(&serde_json::json!({ "file": file, "diagnostics": v })).unwrap());
    } else {
        let _ = write!(err, "{}", commands::render(&checked.diags, &file, &src, a.lang));
        if print_ok && checked.model.is_some() {
            let _ = write!(err, "{}", commands::passed(&file, checked.diags.len(), a.lang));
        }
    }
    // E018: rulec is not joined, which is where the command runs, not what the flow says
    if checked.diags.iter().any(|d| d.code == "E018") {
        return Err(2);
    }
    if diag::has_errors(&checked.diags) {
        return Err(1);
    }
    Ok(checked.model)
}

fn cmd_check(a: &Asked, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    if a.files.is_empty() {
        return takes("check", a.lang, err);
    }
    let mut worst = 0;
    for f in &a.files {
        if let Err(c) = load(f, a, true, out, err) {
            worst = worst.max(c);
        }
    }
    worst
}

/// `dandori gen`: every platform given built before anything is written, each into
/// `<out>/<target>/` in the order of [`TARGETS`]; nothing is written when one cannot take the flow
/// (DESIGN 21).
fn cmd_gen(a: &Asked, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let file = match a.files.as_slice() {
        [f] => f.clone(),
        _ => return takes("gen", a.lang, err),
    };
    if a.targets.is_empty() {
        let names = TARGETS.join("|");
        return refuse(table().needs_of("gen", &format!("--target {names}")), a.lang, err);
    }
    let model = match load(&file, a, false, out, err) {
        Ok(Some(m)) => m,
        Ok(None) => return 1,
        Err(c) => return c,
    };
    let dir = a.out.clone().unwrap_or_else(|| PathBuf::from("generated"));
    let mut planned: Vec<(PathBuf, String)> = Vec::new();
    let mut stops: Vec<crate::diag::Diag> = Vec::new();
    for t in TARGETS.iter().filter(|t| a.targets.iter().any(|g| g == *t)) {
        match commands::build(&model, t).expect("the choices of --target are the targets") {
            Ok(files) => planned.extend(files.into_iter().map(|(name, text)| (dir.join(t).join(name), text))),
            // what two platforms of one generator both stop at (temporal-typescript and durable) is said once
            Err(diags) => {
                for d in diags {
                    let same = |s: &crate::diag::Diag| (s.code, s.line, s.col, &s.en, s.notes.iter().map(|n| &n.en).collect::<Vec<_>>()) == (d.code, d.line, d.col, &d.en, d.notes.iter().map(|n| &n.en).collect::<Vec<_>>());
                    if !stops.iter().any(same) {
                        stops.push(d);
                    }
                }
            }
        }
    }
    if !stops.is_empty() {
        let src = crate::sources::read(&file).unwrap_or_default();
        let _ = write!(err, "{}", commands::render(&stops, &file.display().to_string(), &src, a.lang));
        return 1;
    }
    // what differs, on standard output, where `gen` says the paths it writes
    if a.check {
        let mut differs = false;
        for (p, text) in &planned {
            let shown = p.display();
            match ritsu_base::fs::read(p) {
                Ok(on_disk) if on_disk == text.as_bytes() => {}
                Ok(_) => {
                    let _ = writeln!(out, "{}", tr!("生成物が古いか手で編集されています: {shown}", "generated file is stale or hand-edited: {shown}").get(a.lang));
                    differs = true;
                }
                Err(_) => {
                    let _ = writeln!(out, "{}", tr!("ありません: {shown}", "missing: {shown}").get(a.lang));
                    differs = true;
                }
            }
        }
        return u8::from(differs);
    }
    for (p, text) in planned {
        if let Some(parent) = p.parent() {
            if let Err(e) = ritsu_base::fs::create_dir_all(parent) {
                return refuse(cannot_create(parent, &e), a.lang, err);
            }
        }
        if let Err(e) = ritsu_base::fs::write(&p, text) {
            return refuse(cannot_write(&p, &e), a.lang, err);
        }
        let _ = writeln!(out, "{}", p.display());
    }
    0
}

fn cmd_run(a: &Asked, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let file = match a.files.as_slice() {
        [f] => f.clone(),
        _ => return takes("run", a.lang, err),
    };
    let model = match load(&file, a, false, out, err) {
        Ok(Some(m)) => m,
        Ok(None) => return 1,
        Err(c) => return c,
    };
    let sc = match &a.scenario {
        Some(p) => match ritsu_base::fs::read_to_string(p).map_err(|e| ritsu_base::cli::cannot_read(&p.display().to_string(), &e)).and_then(|t| {
            serde_json::from_str(&t).map_err(|e| ritsu_base::cli::not_json(&p.display().to_string(), &e.to_string()))
        }) {
            Ok(v) => v,
            Err(e) => return refuse(e, a.lang, err),
        },
        None => return refuse(table().needs_of("run", "--scenario <file.json>"), a.lang, err),
    };
    let view = match a.target.as_deref() {
        None | Some("reference") | Some("asl") => crate::render::View::Asl,
        // the three SDKs put the same names on the wire
        Some("temporal-typescript") | Some("temporal-python") | Some("temporal-go") => crate::render::View::Temporal,
        Some("durable") => crate::render::View::Durable,
        Some("argo") => crate::render::View::Argo,
        Some("pydantic-graph") => crate::render::View::Graph,
        // the table takes nothing else (RUN_TARGETS), and refuses another value before the run
        Some(o) => unreachable!("`--target {o}` is not one of RUN_TARGETS, which the table refuses"),
    };
    // The run's clock moves on as time goes by in the run, and `now` reads it (DESIGN 5). A scenario
    // that says its calls of rules, of dates or of books are computed (`ritsu dandori scenarios`
    // writes `"rules": "rulec"`, `"dates": "koyomi"` and `books`) is run with them computed through
    // the ports, as `ritsu run` computes them; a program that joins rulec only computes the rules.
    let ran = match crate::computed::said_ports(&sc) {
        Some((rules, dates, books)) => match crate::computed::run_said(&model, &sc, view, rules, dates, books) {
            Ok(ran) => Ok((ran.trace.clone(), ran.unused.clone(), ran.computes())),
            // as `ritsu run` says it
            Err(crate::computed::Stopped::Scenario(t)) => {
                let sc = a.scenario.as_ref().map(|p| p.display().to_string()).unwrap_or_default();
                return refuse(tr!("シナリオ `{sc}` を流せません: {}", "the scenario `{sc}` cannot be run: {}", t.ja; t.en), a.lang, err);
            }
            Err(crate::computed::Stopped::Run(e)) => Err(e),
        },
        None => crate::interp::run_moving(&model, &sc, view).map(|(trace, unused)| (trace, unused, false)),
    };
    match ran {
        Ok((trace, unused, computes)) => {
            let _ = writeln!(out, "{}", serde_json::to_string_pretty(&trace).unwrap());
            // the run is shown to its end; answers it did not take are the scenario's error, and the
            // run ends as one that runs out of answers does (DESIGN 36)
            match unused {
                Some(left) => {
                    for (i, line) in left.said(computes).iter().enumerate() {
                        let _ = if i == 0 { writeln!(err, "{}", ritsu_base::cli::refusal(line.get(a.lang), a.lang)) } else { writeln!(err, "  {}", line.get(a.lang)) };
                    }
                    1
                }
                None => 0,
            }
        }
        Err(e) => {
            let _ = writeln!(err, "{}", e.get(a.lang));
            1
        }
    }
}

fn cmd_scenarios(a: &Asked, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let file = match a.files.as_slice() {
        [f] => f.clone(),
        _ => return takes("scenarios", a.lang, err),
    };
    let model = match load(&file, a, false, out, err) {
        Ok(Some(m)) => m,
        Ok(None) => return 1,
        Err(c) => return c,
    };
    // The calls of rules are rulec's, as `ritsu run` answers them (§15.209 of rulec's DESIGN). What
    // no scenario takes is said after the scenarios, and why where rulec says (DESIGN 36). Both ask
    // the ports many times, with the disk asked each question once (DESIGN 36.3).
    let (list, untaken) = ritsu_base::fs::steady(|| {
        let list = crate::scenarios::generate_computed(&model);
        let untaken = crate::scenarios::untaken(&model, &list);
        (list, untaken)
    });
    let note = ritsu_base::diag::Severity::Note.word(a.lang);
    let say_untaken = |err: &mut dyn Write| {
        for u in &untaken {
            let _ = writeln!(err, "{note}: {}", u.said().get(a.lang));
        }
    };
    match &a.out {
        Some(dir) => {
            if let Err(e) = ritsu_base::fs::create_dir_all(dir) {
                return refuse(cannot_create(dir, &e), a.lang, err);
            }
            for (i, s) in list.iter().enumerate() {
                let p = dir.join(format!("{:03}.json", i + 1));
                if let Err(e) = ritsu_base::fs::write(&p, serde_json::to_string_pretty(s).unwrap()) {
                    return refuse(cannot_write(&p, &e), a.lang, err);
                }
            }
            let (n, d) = (list.len(), dir.display());
            let _ = writeln!(err, "{}", ritsu_base::tr!("{d} にシナリオを {n} 本書きました", "{n} scenario(s) written to {d}").get(a.lang));
            say_untaken(err);
        }
        None => {
            let _ = writeln!(out, "{}", serde_json::to_string_pretty(&list).unwrap());
            say_untaken(err);
        }
    }
    0
}

/// `dandori api`: what a workflow that passes declares, as JSON on standard output. The diagnostics
/// go where `check` puts them; a workflow that does not pass has no `api`.
fn cmd_api(a: &Asked, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let file = match a.files.as_slice() {
        [f] => f.clone(),
        _ => return takes("api", a.lang, err),
    };
    let (src, checked) = match check::check_kept(&file) {
        Ok(x) => x,
        Err(msg) => return refuse(msg, a.lang, err),
    };
    let shown = file.display().to_string();
    if a.format_json && checked.model.is_none() {
        let v: Vec<_> = checked.diags.iter().map(|d| d.to_json(a.lang)).collect();
        let _ = writeln!(out, "{}", serde_json::to_string_pretty(&serde_json::json!({ "file": shown, "diagnostics": v })).unwrap());
    } else {
        let _ = write!(err, "{}", commands::render(&checked.diags, &shown, &src, a.lang));
    }
    if checked.diags.iter().any(|d| d.code == "E018") {
        return 2;
    }
    let Some(model) = checked.model else { return 1 };
    let _ = writeln!(out, "{}", crate::api::json(&model).pretty());
    0
}

fn cmd_doc(a: &Asked, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let file = match a.files.as_slice() {
        [f] => f.clone(),
        _ => return takes("doc", a.lang, err),
    };
    let (src, drawn) = match check::drawable(&file) {
        Ok(x) => x,
        Err(msg) => return refuse(msg, a.lang, err),
    };
    let shown = file.display().to_string();
    let _ = write!(err, "{}", commands::render(&drawn.diags, &shown, &src, a.lang));
    // a workflow whose names or types do not resolve has nothing to draw; nor has one whose rules
    // this dandori cannot read (E018), which is where it runs, not what the flow says
    let Some(model) = &drawn.model else { return if drawn.diags.iter().any(|d| d.code == "E018") { 2 } else { 1 } };
    let input = crate::doc::Input { m: model, src: &src, file: &shown, facts: &drawn.facts, diags: &drawn.diags, lang: a.made };
    let page = if a.format_html { crate::doc::html(&input) } else { crate::doc::markdown(&input) };
    match &a.out {
        Some(dir) => {
            if let Err(e) = ritsu_base::fs::create_dir_all(dir) {
                return refuse(cannot_create(dir, &e), a.lang, err);
            }
            let stem = file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| model.name.clone());
            let p = dir.join(format!("{stem}.{}", if a.format_html { "html" } else { "md" }));
            if let Err(e) = ritsu_base::fs::write(&p, page) {
                return refuse(cannot_write(&p, &e), a.lang, err);
            }
            let _ = writeln!(out, "{}", p.display());
        }
        None => {
            let _ = write!(out, "{page}");
        }
    }
    // the page is written even so: the runs of the errors are on it
    if diag::has_errors(&drawn.diags) {
        1
    } else {
        0
    }
}
