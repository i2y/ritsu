//! The commands and their flags, in one table (DESIGN 7, rulec's §12.1). `--help` is drawn
//! from it and the command line is read against it, so a flag cannot be documented and
//! ignored, or taken and left undocumented. An unknown flag, a value outside a closed set, a
//! flag without its value and a flag given twice (unless the table says it repeats) all stop
//! the run with exit 2: an agent that is told nothing believes the flag worked.
//! The table is yuen's; how it is drawn and read is ritsu-base's ([`ritsu_base::cli`]).

use ritsu_base::cli::{Cmd, Flag, Reading, Table, flag, help_flag, lang_flag};

pub use ritsu_base::cli::Args;

pub fn global_flags() -> Vec<Flag> {
    vec![
        lang_flag("YUEN_LANG"),
        flag(
            "--root",
            Some("<dir>"),
            tr!(
                "パスを読むルート。無ければ、最初に渡したパスの上で .git を持つ一番近いディレクトリ（無ければ渡したディレクトリ）",
                "the root paths are read from; without it, the nearest directory above the first path given that has a .git (else the directory given)"
            ),
        ),
        help_flag(),
    ]
}

fn every_code() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = crate::codes::ledger().entries.iter().map(|e| e.code).collect();
    v.sort_by_key(|c| (c.starts_with('W'), *c));
    v
}

const PATHS: (&str, &str) = (
    ".req のファイル。ディレクトリなら下の .req を全部（パスの順）。渡したものが一つのプロジェクト",
    ".req files; a directory stands for every .req under it, in path order. What is given is one project",
);

pub fn commands() -> Vec<Cmd> {
    vec![
        Cmd {
            usage: None,
            name: "check",
            args: "<path>...",
            purpose: tr!(
                "検査する。名前、出典のコピーと固定、確かめた記録と今のハッシュ、版の期間、カバレッジ、範囲",
                "check the names, the copies of the sources against their pins, every record against the hashes now, the periods, the coverage and the scope"
            ),
            params: vec![("<path>...", tr!("{}", "{}", PATHS.0; PATHS.1))],
            flags: vec![flag("--format", Some("json"), tr!("プロジェクトに一つの JSON（診断と差分も）", "one JSON object for the project, the diagnostics and their diffs in it")).choices(&["json"])],
            exits: vec![
                (0, tr!("エラーなし（警告はありうる）", "no errors (there may be warnings)")),
                (1, tr!("エラーが一つ以上（印の付いたリンクも）", "at least one error (a marked link is one)")),
                (2, tr!("引数の誤り、読めないファイル、つながっていない言語の成果物（yuen 単独のバイナリのとき。ritsu yuen で走らせる）", "bad arguments, a file that cannot be read, or an artifact of a language not joined (the binary of yuen's own crate: run it as ritsu yuen)")),
            ],
            examples: vec!["yuen check tests/fixtures/period", "yuen check tests/fixtures/period --format json --lang ja"],
            codes: every_code(),
        },
        Cmd {
            usage: None,
            name: "review",
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
                flag("--date", Some("<YYYY-MM-DD>"), tr!("確かめた日。無ければ今日（そのマシンのタイムゾーンで）", "the day it was looked at; without it, today in the machine's time zone")),
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
        },
        Cmd {
            usage: None,
            name: "trace",
            args: "<path>...",
            purpose: tr!(
                "一つの要件か成果物か出典の条から、なぜこうなっているかを、出典までたどって見せる",
                "show why something is the way it is, from one requirement, artifact or article back to the sources"
            ),
            params: vec![("<path>...", tr!("{}", "{}", PATHS.0; PATHS.1))],
            flags: vec![
                flag("--requirement", Some("'<name>[ v<n>]'"), tr!("この要件から（名前か別名）", "from this requirement (its name or alias)")),
                flag("--artifact", Some("'<naming>'"), tr!("この成果物から（`file \"src/app.py\"` のような名指し）", "from this artifact (a naming such as `file \"src/app.py\"`)")),
                flag("--source", Some("'@<source> <article>'"), tr!("この出典の条から", "from this article of a source")),
                flag("--format", Some("json"), tr!("同じ中身を、たどった順の木の JSON で", "the same, as a JSON tree in the order it was followed")).choices(&["json"]),
            ],
            exits: vec![
                (0, tr!("たどった（印が付いていても）", "followed (marks or not)")),
                (1, tr!("構文か名前にエラーがある", "the words or names have errors")),
                (2, tr!("引数の誤り（三つのどれか一つを渡す。無い要件や成果物）、読めないファイル", "bad arguments (give exactly one of the three; one that does not exist), or a file that cannot be read")),
            ],
            examples: vec!["yuen trace tests/fixtures/period --requirement 満了日_142条 --lang ja", "yuen trace tests/fixtures/period --source '@民法 第142条'"],
            codes: vec![],
        },
        Cmd {
            usage: None,
            name: "affected",
            args: "<path>...",
            purpose: tr!(
                "差分が触る要件と、その持ち主、出どころ、最後に決めたことを答える。コードは geas の記録から主張を通って要件までたどる",
                "answer which requirements a diff touches, with their owners, where they come from and the last decision on them; code is followed through geas's records to the claims, and on to the requirements"
            ),
            params: vec![("<path>...", tr!("{}", "{}", PATHS.0; PATHS.1))],
            flags: vec![
                flag("--diff", Some("<file|->"), tr!("統一形式の差分（git diff か diff -u）。- なら標準入力。要る", "a unified diff (git diff or diff -u); - reads standard input; required")),
                flag(
                    "--map",
                    Some("<spec.geas>=<record>"),
                    tr!(
                        "その spec の記録（geas map が書く .map.jsonl）。二度書けば、変更の前と後の記録。無ければ spec の隣の .geas/ の記録",
                        "a record of that spec (the .map.jsonl geas map writes); give it twice for the records before and after the change; without it, the record in .geas/ beside the spec"
                    ),
                )
                .repeats(),
                flag("--format", Some("json"), tr!("同じ答えを JSON で", "the same answer as JSON")).choices(&["json"]),
            ],
            exits: vec![
                (0, tr!("答えた。要件の届かない変更は無い", "answered; no change is out of every requirement's reach")),
                (
                    1,
                    tr!(
                        "要件の届かない変更がある（どの主張も走らせず、どのリンクも名指さないコード、範囲の中でどの要件にも辿れないファイル）。構文か名前にエラーがある",
                        "some change no requirement reaches (code no claim runs and no link names, a file of a scope no requirement leads to); or the words or names have errors"
                    ),
                ),
                (2, tr!("引数の誤り、読めない差分、geas が受け付けなかった記録、読めないファイル", "bad arguments, a diff that cannot be read, geas refusing a record, or a file that cannot be read")),
            ],
            examples: vec![
                "yuen affected tests/fixtures/geas --diff tests/fixtures/geas/changes/change.diff --map tests/fixtures/geas/greeter/greeter.geas=tests/fixtures/geas/greeter/.geas/greeter.map.jsonl --map tests/fixtures/geas/greeter/greeter.geas=tests/fixtures/geas/changes/after.map.jsonl",
            ],
            codes: vec![],
        },
        Cmd {
            usage: None,
            name: "doc",
            args: "<path>...",
            purpose: tr!(
                "要件と来歴のページを出す（Markdown か一枚の HTML）。コードが実現すべきものを理解し、確かめる人が読む",
                "print the page of the requirements and where they come from (Markdown, or one HTML file), for whoever has to understand and check what the code is meant to do"
            ),
            params: vec![("<path>...", tr!("{}", "{}", PATHS.0; PATHS.1))],
            flags: vec![
                flag("--format", Some("markdown|html"), tr!("ページの形。既定は Markdown", "the form of the page; Markdown unless html")).choices(&["markdown", "html"]),
                flag("--out", Some("<dir>"), tr!("ページを書くディレクトリ。最初の .req の名前から <名前>.md か <名前>.html を書く", "the directory to write the page to, as <name>.md or <name>.html after the first .req")),
            ],
            exits: vec![
                (0, tr!("出した（印や欠け、範囲の外れはページに載せる）", "printed (marks, gaps and what the scope misses are on the page)")),
                (1, tr!("構文、名前、出典、成果物にエラーがある（ページを作らず、診断を標準エラーに出す）", "the words, names, sources or artifacts have errors (no page; the diagnostics go to standard error)")),
                (2, tr!("引数の誤り、読めないファイル、つながっていない言語の成果物", "bad arguments, a file that cannot be read, or an artifact of a language not joined")),
            ],
            examples: vec!["yuen doc tests/fixtures/period", "yuen doc tests/fixtures/period --format html --out site --lang ja"],
            codes: vec![],
        },
        Cmd {
            usage: None,
            name: "api",
            args: "<path>...",
            purpose: tr!(
                "プロジェクトのグラフ全体を JSON で出す（ほかのツールが CLI の出力だけから読む形）",
                "print the whole graph of the project as JSON, for other tools to read from the command's output alone"
            ),
            params: vec![("<path>...", tr!("{}", "{}", PATHS.0; PATHS.1))],
            flags: vec![],
            exits: vec![
                (0, tr!("出した（印や欠けがあっても、状態として載せる）", "printed (marks and gaps are in it as states)")),
                (1, tr!("構文か名前にエラーがある", "the words or names have errors")),
                (2, tr!("引数の誤り、読めないファイル", "bad arguments, or a file that cannot be read")),
            ],
            examples: vec!["yuen api tests/fixtures/period"],
            codes: vec![],
        },
        Cmd {
            usage: None,
            name: "export",
            args: "reqif|prov <path>...",
            purpose: tr!(
                "ReqIF（要件管理ツールとのやりとり）か W3C PROV（来歴）に書き出す。印や欠けは状態として書く",
                "write the project out as ReqIF (for requirements tools) or W3C PROV (provenance); marks and gaps go in as states"
            ),
            params: vec![
                ("reqif|prov", tr!("書き出す形。reqif は ReqIF 1.2 の文書、prov は W3C PROV（既定は PROV-N）", "what to write: reqif is a ReqIF 1.2 document, prov is W3C PROV (PROV-N unless --format json)")),
                ("<path>...", tr!("{}", "{}", PATHS.0; PATHS.1)),
            ],
            flags: vec![
                flag("--format", Some("provn|json"), tr!("PROV のとき、PROV-N（既定）か PROV-JSON か", "for prov: PROV-N (the default) or PROV-JSON")).choices(&["provn", "json"]),
                flag(
                    "--time",
                    Some("<RFC 3339>"),
                    tr!(
                        "ReqIF のとき、日付を持たないものの時刻と、文書の作成時刻（CREATION-TIME）。無ければ、プロジェクトに書かれたいちばん新しい日（決めた日、確かめた日）",
                        "for reqif: the time of what has no day of its own, and the document's CREATION-TIME; without it, the latest day the project writes down (a decision, a look)"
                    ),
                ),
                flag("--out", Some("<file>"), tr!("標準出力ではなく、このファイルに書く", "write to this file instead of standard output")),
            ],
            exits: vec![
                (0, tr!("書き出した（印や欠けがあっても、状態として書く）", "written (marks and gaps go in as states)")),
                (1, tr!("構文、名前、出典、成果物にエラーがある（ハッシュを取れないものがあるので、何も書かない）", "the words, names, sources or artifacts have errors (some end cannot be made, so nothing is written)")),
                (
                    2,
                    tr!(
                        "引数の誤り、読めないファイル、書けないファイル、ReqIF の時刻が決まらない、文字列の型に入らない値",
                        "bad arguments, a file that cannot be read or written, no time for ReqIF, or a value the string type cannot hold"
                    ),
                ),
            ],
            examples: vec!["yuen export reqif tests/fixtures/period --out period.reqif", "yuen export prov tests/fixtures/period", "yuen export prov tests/fixtures/period --format json"],
            codes: vec![],
        },
        Cmd {
            usage: None,
            name: "source",
            args: "fetch|pin|outdated <path>...",
            purpose: tr!(
                "出典を取ってきて保存する（fetch）、コピーのハッシュを固定の行に書く（pin）、元が変わったかを問う（outdated）。通信するのは fetch と outdated だけ",
                "fetch the copies of the sources, pin their hashes, or ask whether the originals moved on (only fetch and outdated read the network)"
            ),
            params: vec![
                (
                    "fetch|pin|outdated",
                    tr!(
                        "fetch は e-Gov か eCFR か url から取ってきて保存する。pin はコピーの SHA-256 の先頭 16 桁を書く（ほかは一字も変えない）。outdated は asof より後の版と url を問う",
                        "fetch takes the copies from e-Gov, the eCFR or the url; pin writes the first 16 digits of each copy's SHA-256 (and changes nothing else); outdated asks about the revisions after asof, and about each url"
                    ),
                ),
                ("<path>...", tr!("{}", "{}", PATHS.0; PATHS.1)),
            ],
            flags: vec![],
            exits: vec![
                (0, tr!("済んだ（outdated なら、どの元も変わっていない）", "done (for outdated: no original moved on)")),
                (1, tr!("outdated で、元が変わっていた。構文か名前にエラーがある", "for outdated, an original moved on; or the words or names have errors")),
                (
                    2,
                    tr!(
                        "引数の誤り、読めないファイル、書けないファイル、curl の失敗、つながっていない言語から借りた出典（outdated。ritsu yuen で走らせる）",
                        "bad arguments, a file that cannot be read or written, curl failing, or a source borrowed from a language not joined (outdated: run it as ritsu yuen)"
                    ),
                ),
            ],
            examples: vec!["yuen source fetch tests/fixtures/period", "yuen source pin tests/fixtures/period", "yuen source outdated tests/fixtures/period --lang ja"],
            codes: vec![],
        },
        Cmd {
            usage: None,
            name: "explain",
            args: "<CODE>",
            purpose: tr!("診断のコードを引く。いつ出るか、どう直すか、最小の再現", "look a diagnostic code up: when it comes, how to fix it, the smallest reproduction"),
            params: vec![("<CODE>", tr!("`E302` のような診断のコード。`--all` なら要らない", "a diagnostic code such as `E302`; not needed with `--all`"))],
            flags: vec![
                flag("--all", None, tr!("全部のコードを出す", "print every code")),
                flag("--format", Some("markdown"), tr!("Markdown で出す（docs/codes.md の元）", "print Markdown (what docs/codes.md is made from)")).choices(&["markdown"]),
            ],
            exits: vec![(0, tr!("引けた", "found")), (2, tr!("そのコードが無い、または引数の誤り", "no such code, or bad arguments"))],
            examples: vec!["yuen explain E302", "yuen explain --all --format markdown --lang ja"],
            codes: vec![],
        },
    ]
}

/// The whole table: the commands, the flags every command takes, and the lines at the end of
/// `yuen --help`.
pub fn table() -> Table {
    Table {
        tool: "yuen",
        version: env!("CARGO_PKG_VERSION"),
        summary: tr!("要件の出どころを書く。満たすものにつなぐ。どこかが変われば止める。", "Write where each requirement comes from. Link what meets it. Stop when anything moves."),
        globals: global_flags(),
        commands: commands(),
        footer: vec![
            tr!(
                "どのコマンドにも --lang ja|en（既定は en。環境変数 YUEN_LANG か RITSU_LANG でも指定できる）と --root <dir> を付けられます。",
                "Every command takes --lang ja|en (default en; the YUEN_LANG or RITSU_LANG environment variable works too) and --root <dir>."
            ),
            tr!("exit code: 0 エラーなし / 1 エラーあり / 2 引数の誤りか、読めないファイル", "Exit codes: 0 no errors / 1 errors / 2 bad arguments or a file that cannot be read"),
        ],
        reading: Reading::default(),
    }
}
