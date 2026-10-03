//! The commands and their flags, in one table (ritsu's DESIGN 4.4): `--help` is drawn from it,
//! each command's page too (`dandori <cmd> --help`), and the command line is read against it.
//! An unknown flag, a value outside a closed set, a flag without its value and a flag given
//! twice stop the run with exit 2, so a flag cannot be taken and quietly do nothing. The table
//! is dandori's; how it is drawn and read is ritsu-base's ([`ritsu_base::cli`]).

use crate::commands::TARGETS;
use ritsu_base::cli::{flag, help_flag, lang_flag, Cmd, Flag, Reading, Table};

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
        (2, tr!("引数が正しくないか、ファイルが読めない", "bad arguments, or a file that cannot be read")),
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
                "規則は rulec で読みます。DANDORI_RULEC が rulec の実行ファイルを指し、無ければ PATH の rulec を使います。",
                "The rules are read with rulec: DANDORI_RULEC names the binary, else `rulec` on the PATH."
            ),
            tr!("exit code: 0 エラーなし / 1 エラーあり / 2 引数の誤りか、読めないファイル", "Exit codes: 0 notes only / 1 errors found / 2 bad arguments or an unreadable file"),
        ],
        reading: Reading::default(),
    }
}
