//! The commands of `ritsu` and their flags, in one table (DESIGN 8.1; ritsu-base's `cli`, as every
//! language draws its `--help` and reads its command line). The seven languages are in the table
//! for `ritsu --help` only: what follows `ritsu <language>` is the language's own command line.

use ritsu_base::cli::{Cmd, Flag, Reading, Table, flag, help_flag, lang_flag};
use ritsu_base::text::Text;
use ritsu_base::tr;

pub fn global_flags() -> Vec<Flag> {
    vec![lang_flag("RITSU_LANG"), help_flag()]
}

/// The languages, in the order `ritsu --help` lists them, each with what `ritsu <language>` reads
/// of the others.
pub const LANGUAGES: [&str; 7] = ["rulec", "dandori", "koyomi", "chobo", "geas", "yuen", "sakai"];

fn language(name: &'static str) -> Cmd {
    let purpose = match name {
        "rulec" => tr!("rulec のコマンド（業務の規則）", "rulec's commands (business rules)"),
        "dandori" => tr!(
            "dandori のコマンド（ワークフロー）。フローが使う規則、日付のファイル、帳簿を、同じプロセスの中で読む",
            "dandori's commands (workflows), reading the rules, dates files and books a flow uses in the same process"
        ),
        "koyomi" => tr!("koyomi のコマンド（締め日、支払日、営業日）", "koyomi's commands (closing days, payment days, business days)"),
        "chobo" => tr!("chobo のコマンド（在庫、お金、ポイント、予約の枠の帳簿）", "chobo's commands (books of stock, money, points and booking slots)"),
        "geas" => tr!("geas のコマンド（人が読んだ主張に、コードを従わせる）", "geas's commands (claims a person has read, held over the code)"),
        "yuen" => tr!(
            "yuen のコマンド（要件の来歴）。要件が名指すほかの言語のもの、借りた出典、主張の記録を、同じプロセスの中で読む",
            "yuen's commands (where requirements come from), reading in the same process what requirements name of the other languages, the sources they borrow and the records of claims"
        ),
        _ => tr!(
            "sakai のコマンド（境界づけられたコンテキスト）。地図が持つ規則、カレンダー、ワークフローの参照と、規則の列挙を、同じプロセスの中で読む",
            "sakai's commands (bounded contexts), reading in the same process what the rules, calendars and workflows of a map refer to, and the enums of its rules"
        ),
    };
    Cmd {
        usage: None,
        name,
        args: "<command> ...",
        purpose,
        params: vec![],
        flags: vec![],
        exits: vec![(0, tr!("その言語のコマンドのもの", "the language's command's"))],
        examples: vec![],
        codes: vec![],
    }
}

fn root_flag() -> Flag {
    flag(
        "--root",
        Some("<dir>"),
        tr!(
            "パスを数えるルート。無ければ、最初に渡したパスの上で .git を持つ一番近いディレクトリ（それも無ければ渡したパス）",
            "the root paths count from; else the nearest directory above the first path given that holds .git (else the path given)"
        ),
    )
}

/// `ritsu run` (DESIGN 7.9): a workflow run with its rules, dates and books computed.
fn run() -> Cmd {
    Cmd {
        usage: Some("ritsu run <file.flow> --scenario <file.json> [--target reference|asl|temporal|temporal-python|temporal-go|durable|argo|pydantic-graph] [--format json]"),
        name: "run",
        args: "<file.flow> --scenario <file.json>",
        purpose: tr!(
            "ワークフローを dandori の参照インタプリタで流す。規則は rulec が、期日は koyomi が、帳簿の振替は chobo が計算し（帳簿は動きを覚え、仮押さえは時間がたてば期限が切れる）、ほかのタスクの結果はシナリオから取る",
            "run a workflow in dandori's reference interpreter, its rules computed by rulec, its dates by koyomi and the operations of its books by chobo (a book keeps what was done, and a hold expires as the run's time goes by); every other task is answered by the scenario"
        ),
        params: vec![("<file.flow>", tr!("流す .flow のファイル", "the .flow file to run"))],
        flags: vec![
            flag(
                "--scenario",
                Some("<file.json>"),
                tr!(
                    "流すシナリオ（必ず書く）。`dandori run` のシナリオ（入力、`now`、ほかのタスクの結果）に、走らせる前に帳簿にした操作（`books`）を足せる",
                    "the scenario to run (required): `dandori run`'s (the input, `now`, the other tasks' answers), with what was done to the books before the run (`books`)"
                ),
            ),
            flag("--target", Some("<target>"), tr!("呼び出しを、どのプラットフォームの形で出すか", "the platform whose calls the trace shows"))
                .choices(dandori::cli::RUN_TARGETS)
                .default("reference"),
            flag("--format", Some("json"), tr!("一つの JSON で出す", "print one JSON object")).choices(&["json"]),
        ],
        exits: vec![
            (0, tr!("最後まで流れた（成功、失敗、キャンセルのどれで終わっても）", "the run came to its end (it succeeded, failed or was cancelled)")),
            (1, tr!("フローにエラーがある、または流れが途中で止まった（シナリオに結果が足りないなど）", "the flow has errors, or the run could not go on (the scenario ran out of answers, say)")),
            (2, tr!("引数の誤り、読めないファイル、流せないシナリオ", "bad arguments, a file that cannot be read, or a scenario that cannot be run")),
        ],
        examples: vec!["ritsu run invoice.flow --scenario scenarios/paid.json", "ritsu run invoice.flow --scenario scenarios/paid.json --target temporal --format json"],
        codes: vec![],
    }
}

pub fn commands() -> Vec<Cmd> {
    let mut cmds = vec![Cmd {
        usage: None,
        name: "check",
        args: "[<path>...]",
        purpose: tr!(
            "プロジェクトのファイルを、それぞれの言語の check で確かめる（言語をまたぐ検査も）",
            "check the files of a project, each with its language's check (and across the languages)"
        ),
        params: vec![(
            "<path>...",
            tr!(
                "ファイルかディレクトリ。無ければ今いるディレクトリ。ディレクトリは、下の .rule、.flow、.cal、.book、.geas、.req、.ctx、.proto を全部",
                "files or directories; else the directory you are in. A directory stands for every .rule, .flow, .cal, .book, .geas, .req, .ctx and .proto under it"
            ),
        )],
        flags: vec![flag("--format", Some("json"), tr!("一つの JSON で出す", "print one JSON object")).choices(&["json"]), root_flag()],
        exits: vec![
            (0, tr!("エラーなし（警告はありうる）", "no errors (there may be warnings)")),
            (1, tr!("どれかの言語か、言語をまたぐ検査にエラーがある", "an error in a language, or across them")),
            (2, tr!("引数の誤り、読めないファイル、言語が確かめられなかったファイル", "bad arguments, a file that cannot be read, or one a language could not check")),
        ],
        examples: vec!["ritsu check", "ritsu check rules/ flows/order.flow --format json", "ritsu check . --lang ja"],
        codes: ritsu_cross::codes::codes(),
    }];
    cmds.push(run());
    cmds.push(Cmd {
        usage: None,
        name: "gen",
        args: "[<path>...]",
        purpose: tr!(
            "プロジェクトの規則、期日、帳簿のクライアント、ワークフローを、TypeScript、Python、Go のそれぞれ一つのパッケージにする。ワークフローは同じパッケージの規則と期日と帳簿を読む",
            "write the rules, the dates, the clients of the books and the workflows of a project as one package for each of TypeScript, Python and Go, whose workflows read the package's own rules, dates and books"
        ),
        params: vec![(
            "<path>...",
            tr!(
                "ファイルかディレクトリ。無ければ今いるディレクトリ。下の .rule、.cal、.book、.flow がパッケージに入る",
                "files or directories; else the directory you are in. The .rule, .cal, .book and .flow files under them go into the package"
            ),
        )],
        flags: vec![
            flag("--target", Some("<language>"), tr!("書くパッケージの言語。無ければ三つとも", "the language of the package to write; else all three")).choices(&["typescript", "python", "go"]),
            flag("--out", Some("<dir>"), tr!("パッケージを書く先。言語ごとに <dir>/<language> に書く", "where the packages go: each in <dir>/<language>")).default("generated"),
            flag("--check", None, tr!("書かずに、パッケージが古ければ 1 で落ちる（CI 用）", "write nothing, and exit 1 if a package is stale (for CI)")),
            flag("--books", Some("<database>"), tr!("帳簿のクライアントがつなぐ先", "what the clients of the books call")).choices(&["postgres", "tigerbeetle"]).default("postgres"),
            flag("--name", Some("<name>"), tr!("パッケージの名前（npm のパッケージ、Python のパッケージのディレクトリ、Go の import のパスの既定）", "the package's name: the npm package's, the Python package's directory, and the Go import path unless --module says one")).default("generated"),
            flag("--module", Some("<path>"), tr!("Go のパッケージのディレクトリの import のパス", "the Go import path of the package's directory")),
            root_flag(),
        ],
        exits: vec![
            (0, tr!("書いた、または --check で古いものが無い", "written, or --check found nothing stale")),
            (1, tr!("検査を通らないファイルがある、または --check で古いものがある", "a file does not pass its check, or --check found something stale")),
            (2, tr!(
                "引数の誤り、読めないファイル、書けないファイル、パッケージの同じファイルを書く二つのファイル、生成先の言語が予約している語のモジュールの名前",
                "bad arguments, a file that cannot be read or written, two files that write one file of a package, or a module named by a word the language keeps"
            )),
        ],
        examples: vec!["ritsu gen", "ritsu gen --target typescript --out generated --books tigerbeetle", "ritsu gen --check"],
        codes: vec![],
    });
    cmds.push(Cmd {
        usage: Some("ritsu explain <CODE> | --all [--format markdown|json]"),
        name: "explain",
        args: "<CODE>",
        purpose: tr!(
            "ritsu の診断のコードを引く。いつ出るか、どう直すか、最小の再現（言語のコードは `ritsu <言語> explain`）",
            "look one of ritsu's diagnostic codes up: when it comes, how to fix it, the smallest reproduction (a language's code: `ritsu <language> explain`)"
        ),
        params: vec![("<CODE>", tr!("`E101` のような ritsu のコード。`--all` なら要らない", "one of ritsu's codes, such as `E101`; not needed with `--all`"))],
        flags: vec![
            flag("--all", None, tr!("全部のコードを出す", "print every code")),
            flag("--format", Some("markdown|json"), tr!("Markdown（docs/codes.md の元）か JSON で出す", "print Markdown (what docs/codes.md is made from) or JSON")).choices(&["markdown", "json"]),
        ],
        exits: vec![(0, tr!("引けた", "found")), (2, tr!("ritsu にそのコードが無い、または引数の誤り", "ritsu has no such code, or bad arguments"))],
        examples: vec!["ritsu explain E101", "ritsu explain --all --format markdown --lang ja"],
        codes: vec![],
    });
    cmds.extend(LANGUAGES.into_iter().map(language));
    cmds
}

/// The whole table.
pub fn table() -> Table {
    Table {
        tool: "ritsu",
        version: env!("CARGO_PKG_VERSION"),
        summary: tr!(
            "七つの小さな言語を、一つの処理系で。ある言語が確かめたことを、隣の言語が前提にできる。",
            "Seven small languages, one toolchain. What one checks, the next can build on."
        ),
        globals: global_flags(),
        commands: commands(),
        footer: footer(),
        reading: Reading { no_dashes_in_values: true, ..Reading::default() },
    }
}

fn footer() -> Vec<Text> {
    vec![
        tr!(
            "`ritsu <言語> …` は、その言語のコマンドを、それが読むほかの言語をつないで走らせます。`rulec` などの名前で呼んでも同じです。",
            "`ritsu <language> …` runs the language's command with the languages it reads joined; called by a language's name (`rulec`), ritsu is that command."
        ),
        tr!(
            "--lang ja|en で文面の言語を選びます（無ければ環境変数 RITSU_LANG、それも無ければ en）。`ritsu <言語>` では、その言語が選びます。",
            "--lang ja|en chooses the language of the text (else the RITSU_LANG environment variable, else en); after `ritsu <language>`, the language chooses its own."
        ),
        tr!(
            "exit code: 0 エラーなし / 1 エラーあり / 2 引数の誤りか、読めないファイル。`ritsu <言語>` では、その言語のコマンドのもの",
            "Exit codes: 0 no errors / 1 errors / 2 bad arguments or a file that cannot be read; after `ritsu <language>`, the language's command's"
        ),
    ]
}
