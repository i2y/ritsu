//! The commands and their flags, in one table (DESIGN 6). `--help` is drawn from it and the
//! command line is read against it, so a flag cannot be documented and ignored, or taken and left
//! undocumented. An unknown flag, a value outside a closed set, a flag without its value and a
//! flag given twice all stop the run with exit 2: an agent that is told nothing believes the flag
//! worked (koyomi's and chobo's `cli.rs` work the same way).
//! The table is sakai's; how it is drawn and read is ritsu-base's ([`ritsu_base::cli`]).

use ritsu_base::cli::{Cmd, Flag, Reading, Table, flag, help_flag, lang_flag};

pub use ritsu_base::cli::Args;

pub fn global_flags() -> Vec<Flag> {
    vec![lang_flag("SAKAI_LANG"), help_flag()]
}

fn root_flag() -> Flag {
    flag(
        "--root",
        Some("<dir>"),
        tr!(
            "パスを数えるルート。無ければ、最初に渡したパスの上で .git を持つ一番近いディレクトリ（それも無ければ渡したディレクトリ）",
            "the root paths count from; else the nearest directory above the first path given that holds .git (else the directory given)"
        ),
    )
}

fn check_codes() -> Vec<&'static str> {
    crate::codes::ledger().entries.iter().filter(|e| crate::codes::implemented(e) && !e.code.starts_with("E5")).map(|e| e.code).collect()
}

pub fn commands() -> Vec<Cmd> {
    vec![
        Cmd {
            usage: None,
            name: "check",
            args: "<map.ctx|dir>...",
            purpose: tr!(
                "検査する。成果物の属し方、パターンの整合、境界を越える参照、対応の網羅、同じ語",
                "check a map: who owns each artifact, the patterns, the references that cross a boundary, the mappings and the words"
            ),
            params: vec![(
                "<map.ctx|dir>...",
                tr!(
                    "map のファイル。ディレクトリなら、下の map のファイルを全部（パスの順）。どの地図にも読まれない context のファイルがあれば W103",
                    "map files; a directory stands for every map file under it, in path order, and a context file no map reads there gets W103"
                ),
            )],
            flags: vec![flag("--format", Some("json"), tr!("地図ごとに一行の JSON", "one JSON object per map")).choices(&["json"]), root_flag()],
            exits: vec![
                (0, tr!("エラーなし（警告はありうる）", "no errors (there may be warnings)")),
                (1, tr!("エラーが一つ以上", "at least one error")),
                (2, tr!("引数の誤り、読めないファイル、つながっていない言語の成果物（E104）", "bad arguments, a file that cannot be read, or an artifact of a language not joined (E104)")),
            ],
            examples: vec!["sakai check tests/maps/基本/基本.ctx", "sakai check tests/maps --format json --lang ja"],
            codes: check_codes(),
        },
        Cmd {
            usage: None,
            name: "build",
            args: "<map.ctx>",
            purpose: tr!(
                "コードの import の検査の設定を、地図から書く（import-linter、dependency-cruiser、ArchUnit、go-arch-lint）",
                "write the settings of an import linter from the map (import-linter, dependency-cruiser, ArchUnit, go-arch-lint)"
            ),
            params: vec![("<map.ctx>", tr!("検査を通る map のファイル", "a map file that passes check"))],
            flags: vec![
                flag("--target", Some("<tool>"), tr!("書く設定のツール（必ず書く）", "the tool whose settings to write (required)")).choices(&["import-linter", "dependency-cruiser", "archunit", "go-arch-lint"]),
                flag(
                    "--out",
                    Some("<dir>"),
                    tr!(
                        "書くディレクトリ。無ければ、その言語の `code` の置き場所（archunit は `test` の置き場所）",
                        "the directory to write to; else where the map's `code` line puts the language (for archunit, its `test`)"
                    ),
                ),
                flag("--check", None, tr!("書く代わりに、いまの設定が地図から書くものと同じかを確かめる（違えば E502）", "rather than write, check that the settings there are what the map writes (else E502)")),
                root_flag(),
            ],
            exits: vec![
                (0, tr!("書いた（--check なら、同じだった）", "written (with --check: up to date)")),
                (1, tr!("地図にエラーがある、書けない（E501）、違う（E502）", "the map has errors, the settings cannot be written (E501), or differ (E502)")),
                (2, tr!("引数の誤り、読めないファイル", "bad arguments, or a file that cannot be read")),
            ],
            examples: vec!["sakai build examples/shop/shop.ctx --target import-linter", "sakai build examples/shop/shop.ctx --target go-arch-lint --check --lang ja"],
            codes: vec!["E501", "E502"],
        },
        Cmd {
            usage: None,
            name: "export",
            args: "cml <map.ctx>",
            purpose: tr!("地図を Context Mapper の CML に書き出す", "write the map as Context Mapper's CML"),
            params: vec![
                ("cml", tr!("書き出す形（いまは cml だけ）", "the form to write (cml, for now)")),
                ("<map.ctx>", tr!("検査を通る map のファイル", "a map file that passes check")),
            ],
            flags: vec![flag("--out", Some("<file>"), tr!("書くファイル。無ければ標準出力", "the file to write; else standard output")), root_flag()],
            exits: vec![
                (0, tr!("書いた", "written")),
                (1, tr!("地図にエラーがある（診断は標準エラーに出す）", "the map has errors (the diagnostics go to standard error)")),
                (2, tr!("引数の誤り、読めないファイル", "bad arguments, or a file that cannot be read")),
            ],
            examples: vec!["sakai export cml examples/shop/shop.ctx --out shop.cml"],
            codes: vec![],
        },
        Cmd {
            usage: None,
            name: "doc",
            args: "<map.ctx>",
            purpose: tr!(
                "地図のページを書く。コンテキストマップの図、コンテキストごとの成果物と用語集、関係と越える参照、対応の表",
                "write the map's page: the context map, each context's artifacts and glossary, the relationships and what crosses them, the mappings"
            ),
            params: vec![("<map.ctx>", tr!("検査を通る map のファイル", "a map file that passes check"))],
            flags: vec![
                flag("--format", Some("markdown|html"), tr!("ページの形（既定は markdown）", "the form of the page (markdown unless given)")).choices(&["markdown", "html"]),
                flag(
                    "--out",
                    Some("<dir>"),
                    tr!(
                        "書くディレクトリ。地図のファイルの名前から <名前>.md か <名前>.html を書く。無ければ標準出力",
                        "the directory to write to, as <name>.md or <name>.html after the map file's name; else standard output"
                    ),
                ),
                root_flag(),
            ],
            exits: vec![
                (0, tr!("書いた", "written")),
                (1, tr!("地図にエラーがある（診断は標準エラーに出す）", "the map has errors (the diagnostics go to standard error)")),
                (2, tr!("引数の誤り、読めないファイル、つながっていない言語の成果物（E104）", "bad arguments, a file that cannot be read, or an artifact of a language not joined (E104)")),
            ],
            examples: vec!["sakai doc examples/shop/shop.ctx", "sakai doc examples/shop.ja/通販.ctx --format html --out site --lang ja"],
            codes: vec![],
        },
        Cmd {
            usage: None,
            name: "api",
            args: "<map.ctx>",
            purpose: tr!(
                "地図、属し方、境界を越える参照を JSON で出す（yuen などほかのツールが読む形）",
                "print the map, who owns each artifact, and the references that cross a boundary, as JSON for other tools (yuen, for one)"
            ),
            params: vec![("<map.ctx>", tr!("検査を通る map のファイル", "a map file that passes check"))],
            flags: vec![root_flag()],
            exits: vec![
                (0, tr!("出した", "printed")),
                (1, tr!("地図にエラーがある（診断は標準エラーに出す）", "the map has errors (the diagnostics go to standard error)")),
                (2, tr!("引数の誤り、読めないファイル", "bad arguments, or a file that cannot be read")),
            ],
            examples: vec!["sakai api tests/maps/基本/基本.ctx"],
            codes: vec![],
        },
        Cmd {
            usage: None,
            name: "explain",
            args: "<CODE>",
            purpose: tr!("診断のコードを引く。いつ出るか、どう直すか、最小の再現", "look a diagnostic code up: when it comes, how to fix it, the smallest reproduction"),
            params: vec![("<CODE>", tr!("`E201` のような診断のコード。`--all` なら要らない", "a diagnostic code such as `E201`; not needed with `--all`"))],
            flags: vec![
                flag("--all", None, tr!("全部のコードを出す", "print every code")),
                flag("--format", Some("markdown"), tr!("Markdown で出す（docs/codes.md の元）", "print Markdown (what docs/codes.md is made from)")).choices(&["markdown"]),
            ],
            exits: vec![(0, tr!("引けた", "found")), (2, tr!("そのコードが無い、または引数の誤り", "no such code, or bad arguments"))],
            examples: vec!["sakai explain E401", "sakai explain --all --format markdown --lang ja"],
            codes: vec![],
        },
    ]
}

/// The whole table: the commands, the flags every command takes, and the lines at the end of
/// `sakai --help`. A flag's value never starts with `--`: `--root --format` misses its value.
pub fn table() -> Table {
    Table {
        tool: "sakai",
        version: env!("CARGO_PKG_VERSION"),
        summary: tr!(
            "境界づけられたコンテキストと、そのあいだの関係を書く小さな言語。コンテキストマップのうち、成果物と突き合わせられる部分だけを書き、確かめる。",
            "A small language for bounded contexts and the relationships between them: the part of a context map that can be checked against the artifacts, and checked."
        ),
        globals: global_flags(),
        commands: commands(),
        footer: vec![
            tr!(
                "どのコマンドにも --lang ja|en を付けられます（既定は en。環境変数 SAKAI_LANG か RITSU_LANG でも指定できます）。",
                "Every command takes --lang ja|en (default en; the SAKAI_LANG or RITSU_LANG environment variable works too)."
            ),
            tr!(
                "exit code: 0 エラーなし / 1 エラーあり / 2 引数の誤りか、読めないファイル、つながっていない言語の成果物（E104）",
                "Exit codes: 0 no errors / 1 errors / 2 bad arguments, a file that cannot be read, or an artifact of a language not joined (E104)"
            ),
        ],
        reading: Reading { no_dashes_in_values: true, ..Reading::default() },
    }
}
