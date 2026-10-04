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
use crate::commands::{self, TARGETS};
use crate::diag::{self, Lang, Text};
use ritsu_base::cli::{flag, help_flag, lang_flag, Cmd, Flag, Reading, Table};
use ritsu_ports::Rules;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;

pub use ritsu_base::cli::Args;

/// What `run --target` takes: the reference interpreter's own view, and the platforms'.
pub const RUN_TARGETS: &[&str] = &["reference", "asl", "temporal", "temporal-python", "temporal-go", "durable", "argo", "pydantic-graph"];

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
    vec![
        (0, done),
        (1, tr!("エラーがある", "errors found")),
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
            usage: Some("dandori build <file.flow> --target asl|temporal|temporal-python|temporal-go|durable|argo|pydantic-graph [--out <dir>] [--format json|text]"),
            name: "build",
            args: "<file.flow> --target <target>",
            purpose: tr!(
                "AWS Step Functions（ASL、JSONata）、Temporal（TypeScript・Python・Go）、AWS Lambda durable functions（TypeScript）、Argo Workflows（YAML）、pydantic-graph（Python）向けのコードを書き出す。プラットフォームにできないことと、一回の実行が履歴の上限を超えうるワークフローはエラーにする",
                "compile to AWS Step Functions (ASL, JSONata), to Temporal (TypeScript, Python or Go), to AWS Lambda durable functions (TypeScript), to Argo Workflows (YAML), or to pydantic-graph (Python); a build also refuses what the platform cannot do, and a run that can outgrow its history"
            ),
            params: vec![("<file.flow>", tr!("ビルドする .flow のファイル", "the .flow file to build"))],
            flags: vec![
                flag("--target", Some("<target>"), tr!("ビルドするプラットフォーム（必ず書く）", "the platform to build for (required)")).choices(&TARGETS),
                flag("--out", Some("<dir>"), tr!("書き出す先", "where the files are written")).default("out"),
                diagnostics_format(),
            ],
            exits: exits(tr!("書き出した", "written")),
            examples: vec!["dandori build examples/hotel/temporal/hotel.flow --target temporal --out out"],
            codes: vec![],
        },
        Cmd {
            usage: Some("dandori run <file.flow> --scenario <file.json> [--target reference|asl|temporal|temporal-python|temporal-go|durable|argo|pydantic-graph] [--format json|text]"),
            name: "run",
            args: "<file.flow> --scenario <file.json>",
            purpose: tr!(
                "シナリオに書いた結果で、参照インタプリタがワークフローを動かし、呼び出しをターゲットが出す形で出す",
                "run the workflow in the reference interpreter against scripted answers, and print the trace as the target would show it"
            ),
            params: vec![("<file.flow>", tr!("動かす .flow のファイル", "the .flow file to run"))],
            flags: vec![
                flag("--scenario", Some("<file.json>"), tr!("動かすシナリオ（必ず書く）", "the scenario to play (required)")),
                flag("--target", Some("<target>"), tr!("呼び出しをどのターゲットの形で出すか", "the target whose calls the trace shows")).choices(RUN_TARGETS).default("reference"),
                diagnostics_format(),
            ],
            exits: exits(tr!("最後まで動いた", "the run went to its end")),
            examples: vec!["dandori run examples/hotel/temporal/hotel.flow --scenario scenarios/001.json --target temporal"],
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
            ],
            exits: exits(tr!("ページを出した", "the page is out")),
            examples: vec!["dandori doc examples/hotel/temporal/hotel.flow --format html --out site"],
            codes: vec![],
        },
        Cmd {
            usage: Some("dandori explain <CODE> | --all [--format markdown|json]"),
            name: "explain",
            args: "<CODE>",
            purpose: tr!("診断のコードを引く。いつ出るか、どう直すか、最小の再現", "look a diagnostic code up: when it comes, how to fix it, the smallest example"),
            params: vec![("<CODE>", tr!("`E014` のような診断のコード。`--all` なら要らない", "a diagnostic code such as `E014`; not needed with `--all`"))],
            flags: vec![
                flag("--all", None, tr!("全部のコードを出す", "print every code")),
                flag("--format", Some("markdown|json"), tr!("Markdown か、ツール向けの JSON で出す", "print Markdown, or the machine-facing JSON")).choices(&["markdown", "json"]),
            ],
            exits: vec![(0, tr!("引けた", "found")), (2, tr!("そのコードが無いか、引数の誤り", "no such code, or bad arguments"))],
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
                "どのコマンドにも --lang ja|en を付けられます（既定は en。環境変数 DANDORI_LANG か RITSU_LANG でも指定できます）。",
                "Every command takes --lang ja|en (default en; the DANDORI_LANG or RITSU_LANG environment variable works too)."
            ),
            tr!(
                "規則、日付のファイル、帳簿（`use rule`、`use dates`、`use book`）を使うワークフローは `ritsu dandori` で走らせます。それらを同じプロセスの中で読みます。",
                "A workflow that uses rules, dates files or books (`use rule`, `use dates`, `use book`) runs as `ritsu dandori`, which reads them in the same process."
            ),
            tr!(
                "exit code: 0 エラーなし / 1 エラーあり / 2 引数の誤りか、読めないファイル、規則、日付のファイル、帳簿を読めないこの dandori で、それらを使うフロー（E018）",
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
    target: Option<String>,
    out: Option<PathBuf>,
    scenario: Option<PathBuf>,
}

impl Asked {
    fn from(a: &Args, lang: Lang) -> Asked {
        Asked {
            files: a.pos.iter().map(PathBuf::from).collect(),
            format_json: a.get("--format") == Some("json"),
            format_html: a.get("--format") == Some("html"),
            lang,
            target: a.get("--target").map(str::to_string),
            out: a.get("--out").map(PathBuf::from),
            scenario: a.get("--scenario").map(PathBuf::from),
        }
    }
}

fn refuse(msg: Text, lang: Lang, err: &mut dyn Write) -> u8 {
    let head = if lang == Lang::Ja { "エラー" } else { "error" };
    let _ = writeln!(err, "{head}: {}", msg.get(lang));
    2
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
/// `build`, `run`, `scenarios` and `doc` put into the flow as checks the code makes when the
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

fn run_here(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> u8 {
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
                    None => refuse(tr!("`{n}` というコマンドはありません。`dandori --help` を読んでください", "there is no command `{n}`; run `dandori --help`"), lang, err),
                },
            };
        }
        _ => {}
    }
    let Some(cmd) = table.command(first) else {
        return refuse(tr!("`{first}` というコマンドはありません。`dandori --help` を読んでください", "there is no command `{first}`; run `dandori --help`"), lang, err);
    };
    let parsed = match table.parse(cmd, &args[1..]) {
        Ok(a) => a,
        Err(e) => return refuse(e, lang, err),
    };
    if parsed.has("--help") {
        let _ = write!(out, "{}", table.help_cmd(cmd, lang));
        return 0;
    }
    let a = Asked::from(&parsed, lang);
    match cmd.name {
        "check" => cmd_check(&a, out, err),
        "build" => cmd_build(&a, out, err),
        "run" => cmd_run(&a, out, err),
        "scenarios" => cmd_scenarios(&a, out, err),
        "doc" => cmd_doc(&a, out, err),
        "explain" => cmd_explain(&parsed, lang, out, err),
        _ => unreachable!("every command in the table is dispatched"),
    }
}

/// `dandori explain`: one code of the ledger (crate::codes), or every one, as text, Markdown or JSON.
fn cmd_explain(a: &Args, lang: Lang, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let ledger = crate::codes::ledger();
    let format = a.get("--format");
    if a.has("--all") {
        if !a.pos.is_empty() {
            return refuse(tr!("`--all` とコードは一緒に書けません", "`--all` takes no code"), lang, err);
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
    let [code] = a.pos.as_slice() else {
        return refuse(tr!("`dandori explain <CODE>` か `dandori explain --all` です", "it is `dandori explain <CODE>` or `dandori explain --all`"), lang, err);
    };
    match ledger.find(code) {
        Some(e) => {
            match format {
                Some("markdown") => {
                    let _ = write!(out, "{}", ledger.render_markdown_one(e, lang));
                }
                Some(_) => {
                    let _ = writeln!(out, "{}", ledger.to_json(e, lang).pretty());
                }
                None => {
                    let _ = write!(out, "{}", ledger.render_text(e, lang));
                }
            }
            0
        }
        None => refuse(tr!("診断のコード `{code}` はありません。`dandori explain --all` で一覧が出ます", "there is no diagnostic code `{code}`; `dandori explain --all` lists them"), lang, err),
    }
}

/// The line `dandori <cmd> --help` begins its usage with, for a command given the wrong number
/// of files.
fn usage(name: &str) -> String {
    let table = table();
    table.command(name).map(|c| table.usage_line(c)).unwrap_or_default()
}

/// Check one file and print its diagnostics; the model when it passes. `print_ok` is `check`'s: it
/// says a file passes, and leaves the preconditions ritsu could not decide out of the model, which
/// the other commands put in as checks (DESIGN 1.17).
fn load(path: &Path, a: &Asked, print_ok: bool, out: &mut dyn Write, err: &mut dyn Write) -> Result<Option<crate::model::Model>, u8> {
    let read = if print_ok { check::check_file(path) } else { check::built(path) };
    let (src, checked) = match read {
        Ok(x) => x,
        Err(msg) => {
            let _ = writeln!(err, "{msg}");
            return Err(2);
        }
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
        let _ = writeln!(err, "{}", usage("check"));
        return 2;
    }
    let mut worst = 0;
    for f in &a.files {
        if let Err(c) = load(f, a, true, out, err) {
            worst = worst.max(c);
        }
    }
    worst
}

fn cmd_build(a: &Asked, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let file = match a.files.as_slice() {
        [f] => f.clone(),
        _ => {
            let _ = writeln!(err, "{}", usage("build"));
            return 2;
        }
    };
    let model = match load(&file, a, false, out, err) {
        Ok(Some(m)) => m,
        Ok(None) => return 1,
        Err(c) => return c,
    };
    let dir = a.out.clone().unwrap_or_else(|| PathBuf::from("out"));
    let Some(files) = a.target.as_deref().and_then(|t| commands::build(&model, t)) else {
        let _ = writeln!(err, "--target takes asl, temporal, temporal-python, temporal-go, durable, argo or pydantic-graph");
        return 2;
    };
    let files = match files {
        Ok(f) => f,
        Err(diags) => {
            let src = crate::sources::read(&file).unwrap_or_default();
            let _ = write!(err, "{}", commands::render(&diags, &file.display().to_string(), &src, a.lang));
            return 1;
        }
    };
    for (name, text) in files {
        let p = dir.join(&name);
        if let Some(parent) = p.parent() {
            if let Err(e) = ritsu_base::fs::create_dir_all(parent) {
                let _ = writeln!(err, "cannot create {}: {e}", parent.display());
                return 2;
            }
        }
        if let Err(e) = ritsu_base::fs::write(&p, text) {
            let _ = writeln!(err, "cannot write {}: {e}", p.display());
            return 2;
        }
        let _ = writeln!(out, "{}", p.display());
    }
    0
}

fn cmd_run(a: &Asked, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let file = match a.files.as_slice() {
        [f] => f.clone(),
        _ => {
            let _ = writeln!(err, "{}", usage("run"));
            return 2;
        }
    };
    let model = match load(&file, a, false, out, err) {
        Ok(Some(m)) => m,
        Ok(None) => return 1,
        Err(c) => return c,
    };
    let sc = match &a.scenario {
        Some(p) => match std::fs::read_to_string(p).map_err(|e| e.to_string()).and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string())) {
            Ok(v) => v,
            Err(e) => {
                let _ = writeln!(err, "cannot read the scenario: {e}");
                return 2;
            }
        },
        None => {
            let _ = writeln!(err, "--scenario <file.json> is required");
            return 2;
        }
    };
    let view = match a.target.as_deref() {
        None | Some("reference") | Some("asl") => crate::render::View::Asl,
        // the three SDKs put the same names on the wire
        Some("temporal") | Some("temporal-python") | Some("temporal-go") => crate::render::View::Temporal,
        Some("durable") => crate::render::View::Durable,
        Some("argo") => crate::render::View::Argo,
        Some("pydantic-graph") => crate::render::View::Graph,
        Some(o) => {
            let _ = writeln!(err, "unknown target {o}");
            return 2;
        }
    };
    match crate::interp::run(&model, &sc, view) {
        Ok(trace) => {
            let _ = writeln!(out, "{}", serde_json::to_string_pretty(&trace).unwrap());
            0
        }
        Err(e) => {
            let _ = writeln!(err, "{e}");
            1
        }
    }
}

fn cmd_scenarios(a: &Asked, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let file = match a.files.as_slice() {
        [f] => f.clone(),
        _ => {
            let _ = writeln!(err, "{}", usage("scenarios"));
            return 2;
        }
    };
    let model = match load(&file, a, false, out, err) {
        Ok(Some(m)) => m,
        Ok(None) => return 1,
        Err(c) => return c,
    };
    let list = crate::scenarios::generate(&model);
    match &a.out {
        Some(dir) => {
            if let Err(e) = std::fs::create_dir_all(dir) {
                let _ = writeln!(err, "cannot create {}: {e}", dir.display());
                return 2;
            }
            for (i, s) in list.iter().enumerate() {
                let p = dir.join(format!("{:03}.json", i + 1));
                if let Err(e) = std::fs::write(&p, serde_json::to_string_pretty(s).unwrap()) {
                    let _ = writeln!(err, "cannot write {}: {e}", p.display());
                    return 2;
                }
            }
            let _ = writeln!(err, "{} scenario(s) written to {}", list.len(), dir.display());
        }
        None => {
            let _ = writeln!(out, "{}", serde_json::to_string_pretty(&list).unwrap());
        }
    }
    0
}

fn cmd_doc(a: &Asked, out: &mut dyn Write, err: &mut dyn Write) -> u8 {
    let file = match a.files.as_slice() {
        [f] => f.clone(),
        _ => {
            let _ = writeln!(err, "{}", usage("doc"));
            return 2;
        }
    };
    let (src, drawn) = match check::drawable(&file) {
        Ok(x) => x,
        Err(msg) => {
            let _ = writeln!(err, "{msg}");
            return 2;
        }
    };
    let shown = file.display().to_string();
    let _ = write!(err, "{}", commands::render(&drawn.diags, &shown, &src, a.lang));
    // a workflow whose names or types do not resolve has nothing to draw; nor has one whose rules
    // this dandori cannot read (E018), which is where it runs, not what the flow says
    let Some(model) = &drawn.model else { return if drawn.diags.iter().any(|d| d.code == "E018") { 2 } else { 1 } };
    let input = crate::doc::Input { m: model, src: &src, file: &shown, facts: &drawn.facts, diags: &drawn.diags, lang: a.lang };
    let page = if a.format_html { crate::doc::html(&input) } else { crate::doc::markdown(&input) };
    match &a.out {
        Some(dir) => {
            if let Err(e) = ritsu_base::fs::create_dir_all(dir) {
                let _ = writeln!(err, "cannot create {}: {e}", dir.display());
                return 2;
            }
            let stem = file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| model.name.clone());
            let p = dir.join(format!("{stem}.{}", if a.format_html { "html" } else { "md" }));
            if let Err(e) = ritsu_base::fs::write(&p, page) {
                let _ = writeln!(err, "cannot write {}: {e}", p.display());
                return 2;
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
