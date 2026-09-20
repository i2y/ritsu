//! The `rulec` CLI.
//!
//! One table (`commands()`) is the only place that knows which flags exist. `--help`
//! renders from it and `parse` validates against it, so a flag can never be documented
//! and then ignored, nor accepted and left undocumented. The first reader of this
//! surface is an agent that has nobody to ask, so a misspelled flag stops the run with
//! exit 2 instead of being silently dropped.

use rulec::diag::{Severity, render, render_json};
use rulec::tr;
use std::process::ExitCode;

mod mcp;

// ── The one table ────────────────────────────────────────────────────────

/// A `--flag`, and what may follow it.
struct Flag {
    name: &'static str,
    /// Placeholder for the value; `None` for a boolean flag.
    value: Option<&'static str>,
    /// The only values accepted, when the set is closed. Empty means "anything".
    choices: &'static [&'static str],
    /// What is used when the flag is absent.
    default: Option<String>,
    help: String,
    /// May be given more than once (`--fill`).
    repeat: bool,
    /// Everything after this flag belongs to it verbatim (`--adapter`).
    rest: bool,
}

fn flag(name: &'static str, value: Option<&'static str>, help: String) -> Flag {
    Flag { name, value, choices: &[], default: None, help, repeat: false, rest: false }
}

impl Flag {
    fn choices(mut self, cs: &'static [&'static str]) -> Flag {
        self.choices = cs;
        self
    }
    fn default(mut self, d: impl Into<String>) -> Flag {
        self.default = Some(d.into());
        self
    }
    fn repeat(mut self) -> Flag {
        self.repeat = true;
        self
    }
    fn rest(mut self) -> Flag {
        self.rest = true;
        self
    }
    /// `--out <dir>` — the flag as it appears in a usage line.
    fn spelled(&self) -> String {
        match self.value {
            Some(v) => format!("{} {v}", self.name),
            None => self.name.to_string(),
        }
    }
}

/// One subcommand: what it is for, what it takes, and what its exit codes mean.
struct Cmd {
    name: &'static str,
    /// The positional part of the usage line.
    args: &'static str,
    purpose: String,
    /// One line per positional argument.
    params: Vec<(&'static str, String)>,
    flags: Vec<Flag>,
    /// What each exit code means here. §11 principle 6 fixed 0/1/2 for the tool; this
    /// says which condition produces which for this command.
    exits: Vec<(u8, String)>,
    /// Two runnable examples. Fewer than two teaches nothing about combining flags.
    examples: Vec<String>,
    /// The diagnostic codes this command can print.
    codes: &'static [&'static str],
}

/// `--lang` and `--help` work everywhere, so they are appended to every command rather
/// than repeated in the table.
fn global_flags() -> Vec<Flag> {
    vec![
        flag(
            "--lang",
            Some("ja|en"),
            tr!(
                "文面の言語。無ければ環境変数 RULEC_LANG、それも無ければ en",
                "language of the prose; else the RULEC_LANG environment variable, else en"
            ),
        )
        .choices(&["ja", "en"])
        .default("en"),
        flag("--help", None, tr!("この画面を出す", "print this page")),
    ]
}

fn rule_files() -> (&'static str, String) {
    (
        "<file.rule>...",
        tr!(
            "規則ファイル。ディレクトリを渡すと下の .rule を全部（パス順）",
            "rule files; a directory expands to every .rule under it, in path order"
        ),
    )
}

fn commands() -> Vec<Cmd> {
    let out_flag = |what: &str| {
        flag(
            "--out",
            Some("<dir>"),
            tr!("{what} の書き出し先", "directory to write the {what} into"),
        )
    };
    vec![
        Cmd {
            name: "check",
            args: "<file.rule>...",
            purpose: tr!(
                "規則を検査する。完全性・重なり・当てはまらない行・単位・丸め・溢れ・例",
                "check a rule: completeness, overlap, redundancy, units, rounding, overflow, examples"
            ),
            params: vec![rule_files()],
            flags: vec![
                flag("--format", Some("json"), tr!("GitHub annotations に流せる一行一件の JSON", "one JSON object per line, ready for GitHub annotations")).choices(&["json"]),
                flag("--show-shadow", None, tr!("件数に畳んである隠れ対も全部並べる", "list every shadow pair, including the ones folded into the count")),
                flag("--terse", None, tr!("一件を三行に絞る（見出し・位置・その入力）。詳しくは rulec explain", "cut each finding to three lines: heading, position, witness; `rulec explain` has the rest")),
                flag("--diff-base", Some("<rev>"), tr!("その git リビジョンに既にあった指摘を伏せる", "hide findings that were already present at that git revision")),
                flag("--budget", Some("<n>"), tr!("検査が訪れるノード数の上限。超えたら E109", "cap on the nodes the check visits; over it, E109")).default(rulec::region::DEFAULT_BUDGET.to_string()),
            ],
            exits: vec![
                (0, tr!("エラーなし（警告と注記はありうる）", "no errors (warnings and notes are possible)")),
                (1, tr!("エラーが一つ以上", "at least one error")),
                (2, tr!("引数の誤り、読めないファイル", "bad arguments, or a file that cannot be read")),
            ],
            examples: vec![
                "rulec check rules/".into(),
                "rulec check rules/送料.rule --format json --diff-base origin/main".into(),
            ],
            codes: &[
                "E001", "E002", "E003", "E004", "E005", "E006", "E007", "E008", "E009", "E010",
                "E011", "E012", "E013", "E014", "E015", "E016", "E017", "E018", "E019", "E020",
                "E021", "E022", "E023", "E024", "E025", "E026", "E027", "E028", "E029", "E030",
                "E031", "E032", "E033", "E034", "E035", "E036", "E037", "E038", "E039", "E040", "E041", "E042", "E043", "E044", "E045", "E046", "E101", "E102", "E103",
                "E104", "E105", "E106", "E107", "E108", "E109", "E110", "E111", "E112", "E113",
                "E114", "E115", "W105", "W110", "W111", "W114", "W115", "W116", "W117", "W118", "W119",
            ],
        },
        Cmd {
            name: "explain",
            args: "<CODE>",
            purpose: tr!(
                "診断コードを一つ引く。いつ出るか、どう直すか、最小の再現",
                "look one diagnostic code up: when it appears, how to fix it, the smallest reproduction"
            ),
            params: vec![(
                "<CODE>",
                tr!("`E101` のような診断コード。`--all` なら要らない", "a diagnostic code such as `E101`; not needed with `--all`"),
            )],
            flags: vec![
                flag("--all", None, tr!("全部のコードを出す", "print every code")),
                flag("--format", Some("markdown|json"), tr!("出し方。既定は端末向けの text", "how to render it; the default is text for a terminal"))
                    .choices(&["markdown", "json"]),
            ],
            exits: vec![
                (0, tr!("引けた", "found")),
                (2, tr!("そのコードが無い、または引数の誤り", "no such code, or bad arguments")),
            ],
            examples: vec![
                "rulec explain E101".into(),
                "rulec explain --all --format json".into(),
            ],
            codes: &[],
        },
        Cmd {
            name: "fmt",
            args: "<file.rule>...",
            purpose: tr!(
                "決まった形に整える。列を揃え、記号を ASCII に直す",
                "format to the canonical shape: align the columns, write the symbols in ASCII"
            ),
            params: vec![rule_files()],
            flags: vec![
                flag("--check", None, tr!("書き換えず、整形されていないファイルを名指しする（CI 用）", "name the unformatted files instead of rewriting them (for CI)")),
                flag("--format", Some("json"), tr!("機械向けの JSON（docs/formats.md）", "machine-facing JSON (docs/formats.md)")).choices(&["json"]),
            ],
            exits: vec![
                (0, tr!("整形済み（--check）、または書き換えた", "already formatted (--check), or rewritten")),
                (1, tr!("--check で整形されていないファイルがある", "--check found a file that is not formatted")),
                (2, tr!("引数の誤り、読めないファイル", "bad arguments, or a file that cannot be read")),
            ],
            examples: vec!["rulec fmt rules/送料.rule".into(), "rulec fmt --check rules/".into()],
            codes: &[],
        },
        Cmd {
            name: "gen",
            args: "<file.rule>...",
            purpose: tr!(
                "{} と単体ベクタを生成する。検査を通らない規則からは生成しない",
                "generate {}, and the unit vectors; nothing is generated from a rule that does not pass check",
                rulec::backend::names()
            ),
            params: vec![rule_files()],
            flags: vec![
                out_flag(&tr!("生成物", "generated files")).default("generated"),
                flag("--check", None, tr!("書き換えず、生成物が古ければ 1 で落ちる（CI 用）", "write nothing and exit 1 if a generated file is stale (for CI)")),
                flag("--format", Some("json"), tr!("機械向けの JSON（docs/formats.md）", "machine-facing JSON (docs/formats.md)")).choices(&["json"]),
            ],
            exits: vec![
                (0, tr!("生成した、または --check が一致を確かめた", "generated, or --check found everything up to date")),
                (1, tr!("--check でずれがある、または規則が検査を通らない", "--check found a difference, or the rule does not pass check")),
                (2, tr!("引数の誤り、書けないファイル", "bad arguments, or a file that cannot be written")),
            ],
            examples: vec![
                "rulec gen rules/ --out generated/".into(),
                "rulec gen rules/ --out generated/ --check".into(),
            ],
            codes: &["E001", "E003", "E009", "E011", "E012", "E013", "E103", "E104", "E106", "E108", "E112", "E113"],
        },
        Cmd {
            name: "test",
            args: "<dir>",
            purpose: tr!(
                "生成物を実際に走らせ、ベクタの期待値と突き合わせる",
                "run the generated code and compare it against the expected values of the vectors"
            ),
            params: vec![(
                "<dir>",
                tr!("gen --out で書いた生成先ディレクトリ", "the output directory that `gen --out` wrote"),
            )],
            flags: vec![
                flag("--format", Some("json"), tr!("機械向けの JSON（docs/formats.md）", "machine-facing JSON (docs/formats.md)")).choices(&["json"]),
                flag(
                    "--require-all",
                    None,
                    tr!(
                        "toolchain が無くて飛ばした言語があれば落とす。一致の主張を全言語ぶん要求する",
                        "fail when any language was skipped for a missing toolchain, demanding the agreement across all of them"
                    ),
                ),
                flag(
                    "--proofs",
                    None,
                    tr!(
                        "生成した Rust の証明ハーネスも走らせる（kani が要る。ベクタではなく宣言した範囲の全体）",
                        "also run the proof harnesses beside the generated Rust (needs kani; the whole declared domain, not the vectors)"
                    ),
                ),
            ],
            exits: vec![
                (0, tr!("全部一致した、または toolchain が無くて飛ばした（--require-all を付けると飛ばした時点で 1）", "everything matched, or the toolchain is absent and it was skipped (with --require-all, skipping is 1)")),
                (1, tr!("食い違いがある", "something disagreed")),
                (2, tr!("引数の誤り、読めないディレクトリ", "bad arguments, or a directory that cannot be read")),
            ],
            examples: vec![
                "rulec test generated/".into(),
                "rulec test generated/ --require-all".into(),
                "rulec test generated/ --proofs".into(),
            ],
            codes: &[],
        },
        Cmd {
            name: "coverage",
            args: "<file.rule>...",
            purpose: tr!(
                "作ったテストケースの側を検査する。行・境界の両側・隠れ対・丸めの同着・畳み込みの遷移の五つ",
                "check the vector suite itself against five criteria: rows, both sides of a boundary, shadow pairs, rounding ties, fold transitions"
            ),
            params: vec![rule_files()],
            flags: vec![
                flag("--format", Some("json"), tr!("機械向けの JSON（docs/formats.md）", "machine-facing JSON (docs/formats.md)")).choices(&["json"]),
            ],
            exits: vec![
                (0, tr!("五基準すべてを満たす", "all five criteria are met")),
                (1, tr!("欠けている義務がある（名指しされる）", "an obligation is missing (it is named)")),
                (2, tr!("引数の誤り、読めないファイル", "bad arguments, or a file that cannot be read")),
            ],
            examples: vec!["rulec coverage rules/送料.rule".into(), "rulec coverage rules/".into()],
            codes: &[],
        },
        Cmd {
            name: "vectors",
            args: "<file.rule>...",
            purpose: tr!(
                "境界から作ったテストケースを JSON Lines で出す",
                "emit the test cases built from the boundaries, as JSON Lines"
            ),
            params: vec![rule_files()],
            flags: vec![out_flag(&tr!("ベクタ", "vectors"))],
            exits: vec![
                (0, tr!("出した", "emitted")),
                (1, tr!("規則が検査を通らない", "the rule does not pass check")),
                (2, tr!("引数の誤り、書けないファイル", "bad arguments, or a file that cannot be written")),
            ],
            examples: vec![
                "rulec vectors rules/送料.rule".into(),
                "rulec vectors rules/ --out vectors/".into(),
            ],
            codes: &[],
        },
        Cmd {
            name: "source",
            args: "fetch|pin|outdated <file.rule>",
            purpose: tr!(
                "出典の写しを扱う。fetch は写しを取ってきて規則の隣に置き（法令の引用箇所は e-Gov から、ファイルの出典はその `url` から、引いている表は文書そのものから取り出して）、pin は写しのハッシュを規則に書き込み、outdated は元の文書が変わったかを問い合わせる（法令なら後の改正、ファイルなら `url` の先。コミットで固定した GitHub の URL なら、何がいつ変えたかまで言う）",
                "handle the copies of a rule's sources: fetch brings them to the rule's side — a law's cited fragments from e-Gov, a file source from its url, a cited table out of the document itself — pin writes the copies' digests into the rule, and outdated asks whether the original moved on (a later amendment for a law; what is at the url for a file, and for a GitHub URL pinned to a commit, what changed that path since and when)"
            ),
            params: vec![
                ("fetch|pin|outdated", tr!("fetch と outdated は通信する（curl を呼ぶ）。引いている表を文書から取り出すのも fetch で、check は文書を読み解かない。pin は規則ファイルのハッシュの行だけを書き換える", "fetch and outdated read the network (through curl); fetch is also where a cited table is taken out of a document, since check never reads one; pin rewrites only the pin lines of the rule file")),
                ("<file.rule>", tr!("`source` の宣言と `@` の引用を持つ規則", "a rule with `source` lines and `@` citations")),
            ],
            flags: vec![
                flag(
                    "--via",
                    Some("<cmd>"),
                    tr!(
                        "自分で読めない形式（PDF、スキャン）の表を取り出す抽出器。これ以降は全部そのコマンドの引数で、文書のパスが最後に足される。やりとりは extract/1（docs/formats.md）。テンプレートは `rulec adapter --template docling`",
                        "the extractor that takes the tables out of a format this program cannot read itself (a PDF, a scan); everything after it is that command's own arguments, and the document's path is appended. The protocol is extract/1 (docs/formats.md), and `rulec adapter --template docling` prints a template"
                    ),
                )
                .rest(),
            ],
            exits: vec![
                (0, tr!("済んだ。outdated では、引いている文書が変わっていない", "done; for outdated, nothing the rule cites has moved on")),
                (1, tr!("outdated: 引いている文書が変わっている（後の改正、または `url` の先）。規則が読めないときも", "outdated: something moved on (a later amendment, or what is at the url); also when the rule cannot be parsed")),
                (2, tr!("引数の誤り、読めないファイル、curl の失敗", "bad arguments, a file that cannot be read, or curl failing")),
            ],
            examples: vec![
                "rulec source fetch rules/印紙税.rule".into(),
                "rulec source pin rules/印紙税.rule".into(),
                "rulec source outdated rules/印紙税.rule".into(),
                "rulec source fetch rules/運賃.rule --via ./extract.py".into(),
            ],
            codes: &[],
        },
        Cmd {
            name: "doc",
            args: "<file.rule>...",
            purpose: tr!(
                "承認する人に見せる資料。検査器が知っていて表には出てこない事実を添える",
                "the rendering for the person who approves: the facts the checker knows that the text does not show"
            ),
            params: vec![rule_files()],
            flags: vec![
                out_flag(&tr!("資料", "rendering")),
                flag("--format", Some("html"), tr!("承認する人が自分の件を試せる一枚の HTML。生成した JavaScript がその場で動く", "one HTML page the approver can try a case on; the generated JavaScript runs in it")).choices(&["html"]),
                flag("--audience", Some("approver|customer"), tr!("誰に見せるか。customer はヘルプセンター向けの案内で、別名・範囲・診断コードを省き、境目の例を添える。既定は approver", "who reads it. customer renders the article a help centre publishes (no aliases, ranges or diagnostic codes, and the cases on either side of every threshold); the default is approver")).choices(&["approver", "customer"]),
            ],
            exits: vec![
                (0, tr!("資料を書き出した", "rendered")),
                (1, tr!("規則が検査を通らない（壊れた規則からは書き出さない）", "the rule does not pass check (a broken rule is not rendered)")),
                (2, tr!("引数の誤り、書けないファイル", "bad arguments, or a file that cannot be written")),
            ],
            examples: vec![
                "rulec doc rules/送料.rule --lang ja > doc.md".into(),
                "rulec doc rules/送料.rule --lang ja --format html > doc.html".into(),
                "rulec doc rules/送料.rule --lang ja --audience customer > help.md".into(),
                "rulec doc rules/ --out docs/".into(),
            ],
            codes: &[],
        },
        Cmd {
            name: "api",
            args: "<file.rule>",
            purpose: tr!(
                "生成物の呼び方を、コードを読まずに得る",
                "how to call the generated code, without reading it"
            ),
            params: vec![("<file.rule>", tr!("規則ファイル", "the rule file"))],
            flags: vec![
                flag("--format", Some("json"), tr!("機械向けの JSON（docs/formats.md）。既定も json", "machine-facing JSON (docs/formats.md); also the default")).choices(&["json"]),
            ],
            exits: vec![
                (0, tr!("出した", "emitted")),
                (1, tr!("規則が検査を通らない", "the rule does not pass check")),
                (2, tr!("引数の誤り、読めないファイル", "bad arguments, or a file that cannot be read")),
            ],
            examples: vec![
                "rulec api rules/送料.rule".into(),
                "rulec api rules/送料.rule | jq -r .python.signature".into(),
            ],
            codes: &[],
        },
        Cmd {
            name: "schema",
            args: "<file.rule>",
            purpose: tr!(
                "アダプタとやりとりする JSON Schema を出す",
                "emit the JSON Schema of the wire an adapter speaks"
            ),
            params: vec![("<file.rule>", tr!("規則ファイル", "the rule file"))],
            flags: vec![
                flag("--keys", Some("name|alias"), tr!("プロパティの名前を、規則の名前（届け先）にするか、ASCII の別名（dest）にするか。別名のときは元の名前が title に入る", "name each property by the rule's own name (届け先) or by its ASCII alias (dest); with alias, the rule's name goes in title")).choices(&["name", "alias"]).default("name"),
            ],
            exits: vec![
                (0, tr!("出した", "emitted")),
                (1, tr!("規則が検査を通らない", "the rule does not pass check")),
                (2, tr!("引数の誤り、読めないファイル", "bad arguments, or a file that cannot be read")),
            ],
            examples: vec![
                "rulec schema rules/送料.rule".into(),
                "rulec schema rules/送料.rule --keys alias > request.schema.json".into(),
            ],
            codes: &[],
        },
        Cmd {
            name: "adapter",
            args: "<file.rule>",
            purpose: tr!(
                "いま動いている実装を包む 20〜30 行のテンプレートを出す",
                "emit the 20-to-30-line template that wraps a legacy implementation"
            ),
            params: vec![("<file.rule>", tr!("規則ファイル", "the rule file"))],
            flags: vec![
                flag(
                    "--template",
                    Some("python|go|docling"),
                    tr!(
                        "テンプレートの言語。`docling` は実装ではなく抽出器のテンプレートで、文書を受け取って表を返す（extract/1、`rulec source fetch --via` の相手）",
                        "the language of the template; `docling` is not a legacy implementation but an **extractor** — it takes a document and returns its tables (extract/1, the counterpart of `rulec source fetch --via`)"
                    ),
                )
                .choices(&["python", "go", "docling"])
                .default("python"),
            ],
            exits: vec![
                (0, tr!("出した", "emitted")),
                (1, tr!("規則が検査を通らない", "the rule does not pass check")),
                (2, tr!("引数の誤り、読めないファイル", "bad arguments, or a file that cannot be read")),
            ],
            examples: vec![
                "rulec adapter rules/送料.rule --template python > adapter.py".into(),
                "rulec adapter rules/送料.rule --template go > adapter.go".into(),
                "rulec adapter rules/運賃.rule --template docling > extract.py".into(),
            ],
            codes: &[],
        },
        Cmd {
            name: "verify",
            args: "<file.rule>",
            purpose: tr!(
                "いま動いている実装をプロセスとして立て、同じ入力で同じ答えを出すか確かめる",
                "stand the legacy implementation up as a process and check it answers the same on the same inputs"
            ),
            params: vec![("<file.rule>", tr!("規則ファイル", "the rule file"))],
            flags: vec![
                flag("--format", Some("json"), tr!("機械向けの JSON（docs/formats.md）", "machine-facing JSON (docs/formats.md)")).choices(&["json"]),
                flag(
                "--adapter",
                Some("<cmd> [args...]"),
                tr!(
                    "いま動いている実装を立てるコマンド。rulec を知らないコードでも、rulec が前に生成したものでも構いません。これ以降は全部そのコマンドの引数",
                    "the command that starts the legacy implementation; everything after it is that command's own arguments"
                ),
            )
            .rest(),
            ],
            exits: vec![
                (0, tr!("全件一致した", "every record agreed")),
                (1, tr!("不一致がある（件数・差・入力例つきで出る）", "there are mismatches (reported with counts, differences and witnesses)")),
                (2, tr!("引数の誤り、アダプタを起動できない", "bad arguments, or the adapter could not be started")),
            ],
            examples: vec![
                "rulec verify rules/送料.rule --adapter python3 adapter.py".into(),
                "rulec verify rules/送料.rule --adapter ./legacy --port 0".into(),
            ],
            codes: &[],
        },
        Cmd {
            name: "fixtures",
            args: "lint <file.jsonl> <file.rule>",
            purpose: tr!(
                "過去の記録が宣言どおりの形かを検査する。ETL はしない",
                "check that past records have the declared shape; no ETL is done here"
            ),
            params: vec![
                ("lint", tr!("いまあるのはこの一つだけ", "the only subcommand there is for now")),
                ("<file.jsonl>", tr!("1 件 1 行の記録", "the records, one per line")),
                ("<file.rule>", tr!("突き合わせる規則", "the rule to check them against")),
            ],
            flags: vec![
                flag("--manifest", Some("<m.json>"), tr!("欄が欠けた記録を補完する既定値の宣言", "the declaration of the default values that fill a missing field")),
                flag("--fill", Some("<欄=値>"), tr!("既定値をその場で上書きする（何度でも書ける）", "override one default value in place (may be repeated)")).repeat(),
                flag("--format", Some("json"), tr!("機械向けの JSON（docs/formats.md）", "machine-facing JSON (docs/formats.md)")).choices(&["json"]),
            ],
            exits: vec![
                (0, tr!("形式の問題なし", "no format problems")),
                (1, tr!("問題がある（種類ごとに数えて出る）", "there are problems (counted by kind)")),
                (2, tr!("引数の誤り、読めないファイル", "bad arguments, or a file that cannot be read")),
            ],
            examples: vec![
                "rulec fixtures lint replay/2025-08.jsonl rules/送料.rule".into(),
                "rulec fixtures lint replay/2025-08.jsonl rules/送料.rule --fill 重量=1000".into(),
            ],
            codes: &[],
        },
        Cmd {
            name: "replay",
            args: "<file.rule>",
            purpose: tr!(
                "過去の記録に規則を当て、そのとき出た値と突き合わせる",
                "apply the rule to past records and compare with the values that came out at the time"
            ),
            params: vec![("<file.rule>", tr!("規則ファイル、または 送料@v3 や rules/送料.rule@origin/main（git の中の版）", "the rule file, or 送料@v3 or rules/送料.rule@origin/main (a version inside git)"))],
            flags: vec![
                flag("--fixtures", Some("<f.jsonl>"), tr!("過去の記録（必須）", "the past records (required)")),
                flag("--manifest", Some("<m.json>"), tr!("補完の既定値の宣言", "the declaration of the default values used for filling")),
                flag("--fill", Some("<欄=値>"), tr!("既定値をその場で上書きする（何度でも書ける）", "override one default value in place (may be repeated)")).repeat(),
                flag("--format", Some("markdown|json"), tr!("PR に貼れる markdown、または機械向けの JSON（docs/formats.md）", "markdown to paste into a PR, or machine-facing JSON (docs/formats.md)")).choices(&["markdown", "json"]),
                flag("--terse", None, tr!("入力例を出さない。件数と金額だけにして、本番の記録の値を PR に貼らない", "leave the witnesses out: counts and amounts only, so that no value from a production record is pasted into a pull request")),
            ],
            exits: vec![
                (0, tr!("全件一致した", "every record agreed")),
                (1, tr!("不一致がある", "there are mismatches")),
                (2, tr!("引数の誤り、読めないファイル", "bad arguments, or a file that cannot be read")),
            ],
            examples: vec![
                "rulec replay rules/送料.rule --fixtures replay/2025-08.jsonl".into(),
                "rulec replay 送料@v3 --fixtures replay/2025-08.jsonl --format markdown".into(),
            ],
            codes: &[],
        },
        Cmd {
            name: "import",
            args: "csv|xlsx <file>",
            purpose: tr!(
                "表計算のシートから .rule の下書きを起こす。推定した箇所には全部 # 推定 が付く",
                "make a first draft of a .rule from a spreadsheet; every guess is marked"
            ),
            params: vec![
                ("csv|xlsx", tr!("CSV か、Excel の .xlsx。xlsx は書き出さずにそのまま読む", "a CSV, or an Excel .xlsx — the workbook is read as it is, with no export step")),
                ("<file>", tr!("見出し行つきの表。最後の列が出力（--outputs で本数を変えられる）", "a table with a header row; the last column is the output (--outputs changes how many)")),
            ],
            flags: vec![
                flag("--name", Some("<名前>"), tr!("規則の名前。既定はファイル名", "the rule's name; the default is the file's stem")),
                flag("--outputs", Some("<n>"), tr!("末尾の何列が出力か", "how many of the trailing columns are outputs")).default("1"),
                flag("--sheet", Some("<名前>"), tr!("xlsx のどのシートを読むか。既定は最初のシート", "which sheet of an xlsx to read; the default is the first")),
            ],
            exits: vec![
                (0, tr!("下書きを書き出した。check はまだ通していない", "the draft was written; it has not been through check")),
                (2, tr!("読めないファイル、または表の形をしていないシート", "a file that cannot be read, or a sheet that is not a table")),
            ],
            examples: vec![
                "rulec import csv tariff.csv > rules/tariff.rule".into(),
                "rulec import xlsx 運賃表.xlsx --sheet 本則 > rules/運賃.rule".into(),
                "rulec import csv rates.csv --name 料率 --outputs 2".into(),
            ],
            codes: &[],
        },
        Cmd {
            name: "mcp",
            args: "",
            purpose: tr!(
                "MCP サーバとして stdio で待ち受ける。上のコマンド一つ一つがツール、文書がリソース",
                "serve over stdio as an MCP server: every command above as a tool, the documents as resources"
            ),
            params: vec![],
            flags: vec![],
            exits: vec![(0, tr!("stdin が閉じた", "stdin was closed"))],
            examples: vec![
                "rulec mcp".into(),
                "rulec mcp --lang ja".into(),
            ],
            codes: &[],
        },
        Cmd {
            name: "diff",
            args: "<old> <new>",
            purpose: tr!(
                "二つの版を同じ記録に当て、何件がいくら動くかを出す",
                "apply two versions to the same records and report how many change and by how much"
            ),
            params: vec![
                ("<old>", tr!("旧の規則。file.rule、送料@v3（git タグ rules/送料/v3）、または rules/送料.rule@origin/main（そのリビジョンのそのパス）", "the old rule: file.rule, 送料@v3 (the git tag rules/送料/v3), or rules/送料.rule@origin/main (that path at that revision)")),
                ("<new>", tr!("新の規則。同じ書き方", "the new rule, written the same way")),
            ],
            flags: vec![
                flag("--fixtures", Some("<f.jsonl>"), tr!("過去の記録（必須）", "the past records (required)")),
                flag("--manifest", Some("<m.json>"), tr!("補完の既定値の宣言", "the declaration of the default values used for filling")),
                flag("--fill", Some("<欄=値>"), tr!("既定値をその場で上書きする（何度でも書ける）", "override one default value in place (may be repeated)")).repeat(),
                flag("--format", Some("markdown|json"), tr!("PR に貼れる markdown、または機械向けの JSON（docs/formats.md）", "markdown to paste into a PR, or machine-facing JSON (docs/formats.md)")).choices(&["markdown", "json"]),
                flag("--terse", None, tr!("入力例を出さない。件数と金額だけにして、本番の記録の値を PR に貼らない", "leave the witnesses out: counts and amounts only, so that no value from a production record is pasted into a pull request")),
            ],
            exits: vec![
                (0, tr!("全件同じ答え（影響なし）", "both versions answered the same everywhere (no impact)")),
                (1, tr!("影響がある", "there is an impact")),
                (2, tr!("引数の誤り、読めないファイル", "bad arguments, or a file that cannot be read")),
            ],
            examples: vec![
                "rulec diff 送料@v3 送料@v4 --fixtures replay/2025-08.jsonl".into(),
                "rulec diff rules/送料.rule@origin/main rules/送料.rule --fixtures \"$FIXTURES\" --format markdown --terse".into(),
            ],
            codes: &[],
        },
    ]
}

// ── Rendering the table ──────────────────────────────────────────────────

fn usage_line(c: &Cmd) -> String {
    let mut o = format!("rulec {} {}", c.name, c.args).trim_end().to_string();
    for f in &c.flags {
        o.push_str(&format!(" [{}]", f.spelled()));
    }
    o
}

/// `rulec <cmd> --help` and `rulec help <cmd>`.
fn help_cmd(c: &Cmd) -> String {
    let mut o = format!("rulec {} — {}\n\n", c.name, c.purpose);
    o.push_str(&tr!("使い方:\n", "Usage:\n"));
    o.push_str(&format!("  {}\n", usage_line(c)));

    if !c.params.is_empty() {
        o.push_str(&tr!("\n引数:\n", "\nArguments:\n"));
        let w = c.params.iter().map(|(n, _)| n.len()).max().unwrap_or(0);
        for (n, h) in &c.params {
            o.push_str(&format!("  {n:w$}  {h}\n", w = w));
        }
    }

    o.push_str(&tr!("\nフラグ:\n", "\nFlags:\n"));
    let flags: Vec<&Flag> = c.flags.iter().chain(GLOBALS.get_or_init(global_flags)).collect();
    let w = flags.iter().map(|f| rulec::diag::width(&f.spelled())).max().unwrap_or(0);
    for f in &flags {
        let s = f.spelled();
        let pad = " ".repeat(w - rulec::diag::width(&s));
        let d = match &f.default {
            Some(d) => tr!(" (既定 {d})", " (default {d})"),
            None => String::new(),
        };
        o.push_str(&format!("  {s}{pad}  {}{d}\n", f.help));
    }

    o.push_str(&tr!("\nexit code:\n", "\nExit codes:\n"));
    for (n, h) in &c.exits {
        o.push_str(&format!("  {n}  {h}\n"));
    }

    o.push_str(&tr!("\n例:\n", "\nExamples:\n"));
    for e in &c.examples {
        o.push_str(&format!("  $ {e}\n"));
    }

    if !c.codes.is_empty() {
        o.push_str(&tr!(
            "\n出しうる診断（`rulec explain <CODE>` が引きます）:\n  ",
            "\nDiagnostics it can print (`rulec explain <CODE>` looks one up):\n  "
        ));
        o.push_str(&c.codes.join(" "));
        o.push('\n');
    }
    o
}

/// `rulec --help`: the list of commands, one line each.
fn help_all() -> String {
    let cs = commands();
    let mut o = format!("rulec {}\n\n", env!("CARGO_PKG_VERSION"));
    // The set of targets is src/backend.rs's to state, here as everywhere (§15.20).
    o.push_str(&tr!(
        "業務ルールを一枚の表として書き、検査し、{} に生成する。\n\n",
        "Write business rules as one table, check them, and generate {}.\n\n",
        rulec::backend::names()
    ));
    o.push_str(&tr!("使い方:\n", "Usage:\n"));
    let w = cs.iter().map(|c| c.name.len() + c.args.len()).max().unwrap_or(0);
    for c in &cs {
        let head = format!("{} {}", c.name, c.args);
        o.push_str(&format!("  rulec {head:w$}  {}\n", c.purpose, w = w));
    }
    o.push_str(&tr!(
        "\n一つのコマンドの詳しい説明は `rulec <cmd> --help`（`rulec help <cmd>` も同じ）。\n",
        "\nFor one command in detail: `rulec <cmd> --help` (`rulec help <cmd>` is the same page).\n"
    ));
    o.push_str(&tr!(
        "どのコマンドにも --lang ja|en を付けられます（既定は en、環境変数 RULEC_LANG でも指定できます）。\n",
        "Every command accepts --lang ja|en (default en; the RULEC_LANG environment variable works too).\n"
    ));
    o.push_str(&tr!(
        "exit code: 0 注記のみ / 1 エラーあり / 2 引数の誤りか、読めないファイル\n",
        "Exit codes: 0 notes only / 1 errors found / 2 bad arguments or an unreadable file\n"
    ));
    o
}

static GLOBALS: std::sync::OnceLock<Vec<Flag>> = std::sync::OnceLock::new();

// ── Parsing against the table ────────────────────────────────────────────

/// What one command line said.
struct Args {
    /// Flag name → value. A boolean flag stores the empty string. A repeatable flag
    /// appears once per occurrence.
    got: Vec<(&'static str, String)>,
    /// Everything after a `rest` flag (`--adapter`), verbatim.
    rest: Vec<String>,
    pos: Vec<String>,
}

impl Args {
    fn has(&self, n: &str) -> bool {
        self.got.iter().any(|(k, _)| *k == n)
    }
    fn get(&self, n: &str) -> Option<&str> {
        self.got.iter().find(|(k, _)| *k == n).map(|(_, v)| v.as_str())
    }
    fn all(&self, n: &str) -> Vec<&str> {
        self.got.iter().filter(|(k, _)| *k == n).map(|(_, v)| v.as_str()).collect()
    }
}

/// Walk the command line against the command's flags. An unknown flag, a missing value
/// and a value outside a closed set all stop the run; none of them is dropped quietly.
fn parse(c: &Cmd, argv: &[String]) -> Result<Args, String> {
    let globals = GLOBALS.get_or_init(global_flags);
    let find = |name: &str| c.flags.iter().chain(globals.iter()).find(|f| f.name == name);
    let mut out = Args { got: Vec::new(), rest: Vec::new(), pos: Vec::new() };
    let mut i = 0;
    while i < argv.len() {
        let a = &argv[i];
        if !a.starts_with("--") {
            out.pos.push(a.clone());
            i += 1;
            continue;
        }
        let (name, inline) = match a.split_once('=') {
            Some((k, v)) => (k.to_string(), Some(v.to_string())),
            None => (a.clone(), None),
        };
        let Some(f) = find(&name) else {
            return Err(tr!(
                "知らないフラグ `{name}` です。`rulec {} --help` を読んでください",
                "unknown flag `{name}`; run `rulec {} --help`",
                c.name
            ));
        };
        if f.rest {
            out.got.push((f.name, String::new()));
            out.rest = argv[i + 1..].to_vec();
            if out.rest.is_empty() {
                return Err(tr!(
                    "`{}` にはコマンドが要ります",
                    "`{}` needs a command",
                    f.spelled()
                ));
            }
            return Ok(out);
        }
        let v = match (f.value, inline) {
            (None, Some(v)) => {
                return Err(tr!(
                    "`{}` は値を取りません（`={v}` が付いています）",
                    "`{}` takes no value (it was given `={v}`)",
                    f.name
                ));
            }
            (None, None) => String::new(),
            (Some(_), Some(v)) => v,
            (Some(_), None) => {
                i += 1;
                match argv.get(i) {
                    Some(v) => v.clone(),
                    None => {
                        return Err(tr!(
                            "`{}` に値がありません",
                            "`{}` is missing its value",
                            f.spelled()
                        ));
                    }
                }
            }
        };
        if !f.choices.is_empty() && !f.choices.contains(&v.as_str()) {
            return Err(tr!(
                "`{} {v}` は知らない値です。書けるのは {} だけです",
                "`{} {v}` is not a value this flag takes; it takes only {}",
                f.name,
                f.choices.join(" | ")
            ));
        }
        if !f.repeat && out.has(f.name) {
            return Err(tr!(
                "`{}` が二度書かれています",
                "`{}` is given twice",
                f.name
            ));
        }
        out.got.push((f.name, v));
        i += 1;
    }
    Ok(out)
}

/// Stop before doing anything, and say which command's help explains it.
fn refuse(msg: String) -> ExitCode {
    eprintln!("error: {msg}");
    ExitCode::from(2)
}

/// `rulec source fetch|pin|outdated <file.rule>` (§15.68). `fetch` and `outdated` are the
/// only commands that read the network; `check` never does. `pin` rewrites the pin lines of
/// the file and nothing else.
fn source_cmd(files: &[&String], via: &[String]) -> ExitCode {
    let verb = files.first().map(|s| s.as_str()).unwrap_or("");
    if !matches!(verb, "fetch" | "pin" | "outdated") || files.len() != 2 {
        return refuse(tr!("`rulec source fetch|pin|outdated <file.rule>` です", "it is `rulec source fetch|pin|outdated <file.rule>`"));
    }
    let path = files[1];
    let Ok(src) = std::fs::read_to_string(path) else {
        return refuse(tr!("`{path}` を読めません", "cannot read `{path}`"));
    };
    let parsed = rulec::parse::parse(&src, path);
    let Some(f) = parsed.file.filter(|_| parsed.diags.is_empty()) else {
        let lines: Vec<String> = src.lines().map(|s| s.to_string()).collect();
        print!("{}", rulec::findings_text(&parsed.diags, &lines));
        return ExitCode::from(1);
    };
    let result = match verb {
        "fetch" => rulec::sources::fetch(&f, path, via),
        "outdated" => rulec::sources::outdated(&f, path),
        _ => rulec::sources::pin(&f, path, &src).and_then(|(text, o)| {
            if text != src {
                std::fs::write(path, &text).map_err(|e| tr!("`{path}` に書けません: {e}", "cannot write `{path}`: {e}"))?;
            }
            Ok(o)
        }),
    };
    match result {
        Ok(o) => {
            if o.lines.is_empty() {
                println!("{}", tr!("{path}: 出典の宣言がありません", "{path}: no source is declared"));
            }
            for l in &o.lines {
                println!("{l}");
            }
            ExitCode::from(if verb == "outdated" && o.changed { 1 } else { 0 })
        }
        Err(e) => refuse(e),
    }
}

/// `--terse` shortens what a person reads; the JSON is already the machine-facing shape.
fn terse_with_json() -> ExitCode {
    refuse(tr!(
        "`--format json` と `--terse` は一緒に書けません。JSON は既に機械向けの形です",
        "`--format json` and `--terse` cannot be combined; the JSON is already the machine-facing shape"
    ))
}

/// A command that needs files but was given none.
fn need_args(c: &Cmd) -> ExitCode {
    refuse(tr!(
        "`rulec {}` には引数が要ります: {}。`rulec {} --help` を読んでください",
        "`rulec {}` needs arguments: {}; run `rulec {} --help`",
        c.name,
        c.args,
        c.name
    ))
}

/// Die quietly when the reader of a pipe goes away, the way `cat` does. Rust ignores SIGPIPE
/// at startup, so `rulec vectors | head` would have `println!` see EPIPE and panic instead.
#[cfg(unix)]
fn restore_sigpipe() {
    unsafe extern "C" {
        fn signal(signum: i32, handler: usize) -> usize;
    }
    // SIGPIPE is 13 on Linux and macOS; SIG_DFL is 0.
    unsafe { signal(13, 0) };
}

#[cfg(not(unix))]
fn restore_sigpipe() {}

fn main() -> ExitCode {
    restore_sigpipe();
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    // `--lang ja|en` (or `--lang=en`) decides the output language before anything
    // is printed; `RULEC_LANG` is the fallback, English the default (i18n.rs).
    if let Some(i) = args.iter().position(|a| a == "--lang") {
        let Some(v) = args.get(i + 1).and_then(|v| rulec::i18n::Lang::parse(v)) else {
            eprintln!("error: --lang ja|en");
            return ExitCode::from(2);
        };
        rulec::i18n::set(v);
        args.drain(i..i + 2);
    } else if let Some(i) = args.iter().position(|a| a.starts_with("--lang=")) {
        let Some(v) = rulec::i18n::Lang::parse(&args[i]["--lang=".len()..]) else {
            eprintln!("error: --lang ja|en");
            return ExitCode::from(2);
        };
        rulec::i18n::set(v);
        args.remove(i);
    }

    if args.is_empty() {
        eprint!("{}", help_all());
        return ExitCode::from(2);
    }
    if args[0] == "--version" || args[0] == "-V" {
        println!("rulec {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::from(0);
    }
    if args[0] == "--help" || args[0] == "-h" {
        print!("{}", help_all());
        return ExitCode::from(0);
    }
    let cmds = commands();
    if args[0] == "help" {
        return match args.get(1) {
            None => {
                print!("{}", help_all());
                ExitCode::from(0)
            }
            Some(n) => match cmds.iter().find(|c| c.name == n.as_str()) {
                Some(c) => {
                    print!("{}", help_cmd(c));
                    ExitCode::from(0)
                }
                None => refuse(tr!(
                    "知らないコマンド `{n}` です。`rulec --help` に一覧があります",
                    "unknown command `{n}`; `rulec --help` lists them"
                )),
            },
        };
    }
    let Some(cmd) = cmds.iter().find(|c| c.name == args[0].as_str()) else {
        let n = &args[0];
        return refuse(tr!(
            "知らないコマンド `{n}` です。`rulec --help` に一覧があります",
            "unknown command `{n}`; `rulec --help` lists them"
        ));
    };
    let a = match parse(cmd, &args[1..]) {
        Ok(a) => a,
        Err(e) => return refuse(e),
    };
    if a.has("--help") {
        print!("{}", help_cmd(cmd));
        return ExitCode::from(0);
    }

    // The CI of §12 writes `rulec check rules/`. A directory expands to the `.rule` files
    // inside it, in a deterministic order (by path) so that the order of the report does not
    // change with the machine. Only `test` takes the output directory itself as its
    // argument, so it is not expanded.
    let expanded: Vec<String> = if cmd.name == "test" {
        a.pos.clone()
    } else {
        a.pos.iter().flat_map(|x| expand(x)).collect()
    };
    let files: Vec<&String> = expanded.iter().collect();
    if files.is_empty() && !cmd.args.is_empty() && !a.has("--all") {
        return need_args(cmd);
    }
    let json = a.get("--format") == Some("json");
    let markdown = a.get("--format") == Some("markdown");

    match cmd.name {
        "check" => {
            let budget = match a.get("--budget") {
                Some(v) => match v.parse::<i64>() {
                    Ok(n) if n > 0 => n,
                    _ => return refuse(tr!("`--budget {v}` は正の整数ではありません", "`--budget {v}` is not a positive integer")),
                },
                None => rulec::region::DEFAULT_BUDGET,
            };
            if json && a.has("--terse") {
                return terse_with_json();
            }
            check(&files, json, a.has("--terse"), a.has("--show-shadow"), a.get("--diff-base"), budget)
        }
        "explain" => explain(&files, &a),
        "fmt" => fmt(&files, a.has("--check"), json),
        "schema" => {
            let alias = a.get("--keys") == Some("alias");
            one(&files, move |f, c, _| Some(rulec::verify::schema(f, c, alias)))
        }
        // `api` needs the source text (the generator stamps its hash), so it does not go
        // through `one`.
        "api" => api(&files),
        "adapter" => {
            let lang = a.get("--template").unwrap_or("python").to_string();
            // The extraction adapter is about a document, not about this rule, so it needs
            // nothing from the file — but it is asked for the same way, beside its twin.
            one(&files, move |f, _, path| {
                Some(match lang.as_str() {
                    // The extraction adapter is about the rule's documents rather than about
                    // its inputs, but it is asked for the same way, beside its twin.
                    "docling" => rulec::extract::template(f, path),
                    _ => rulec::verify::template(&lang, f),
                })
            })
        }
        "verify" => {
            if a.rest.is_empty() {
                return refuse(tr!(
                    "`--adapter <cmd>` が要ります。`rulec verify --help` を読んでください",
                    "`--adapter <cmd>` is required; run `rulec verify --help`"
                ));
            }
            verify(&files, &a.rest, json)
        }
        "coverage" => coverage(&files, json),
        "doc" => doc(&files, a.get("--out"), a.get("--format") == Some("html"), a.get("--audience") == Some("customer")),
        "source" => source_cmd(&files, &a.rest),
        "fixtures" => {
            // `rulec fixtures lint <jsonl> <rule>`
            if files.first().map(|s| s.as_str()) != Some("lint") {
                return refuse(tr!(
                    "いまあるのは `rulec fixtures lint` だけです",
                    "only `rulec fixtures lint` exists for now"
                ));
            }
            fixtures_lint(&files[1..], &a, json)
        }
        "replay" => replay_cmd(&files, &a, markdown, json),
        "diff" => diff_cmd(&files, &a, markdown, json),
        "test" => {
            let Some(dir) = files.first() else { return need_args(cmd) };
            match rulec::runtest::run_with(std::path::Path::new(dir.as_str()), a.has("--proofs")) {
                Ok(r) => {
                    if json {
                        println!("{}", rulec::runtest::render_json(&r));
                    } else {
                        print!("{}", rulec::runtest::render(&r));
                    }
                    // A language skipped for a missing toolchain narrows what the run proved.
                    // The default stays lenient, because not every machine carries six
                    // toolchains; `--require-all` is what CI uses to demand the whole claim.
                    let short = a.has("--require-all") && !r.skipped.is_empty();
                    ExitCode::from(u8::from(!r.ok() || short))
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::from(2)
                }
            }
        }
        "vectors" => vectors(&files, a.get("--out")),
        "mcp" => mcp::serve(),
        "import" => {
            let kind = files.first().map(|s| s.as_str()).unwrap_or("");
            if !matches!(kind, "csv" | "xlsx") || files.len() != 2 {
                return refuse(tr!(
                    "`rulec import csv <file.csv>` か `rulec import xlsx <file.xlsx>` です",
                    "it is `rulec import csv <file.csv>` or `rulec import xlsx <file.xlsx>`"
                ));
            }
            let path = files[1];
            let stem = std::path::Path::new(path).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let name = a.get("--name").map(|s| s.to_string()).unwrap_or(stem);
            let n = match a.get("--outputs").unwrap_or("1").parse::<usize>() {
                Ok(n) => n,
                Err(_) => return refuse(tr!("`--outputs` は正の整数です", "`--outputs` is a positive integer")),
            };
            // The draft names its source by the file's name, not by wherever it was read from.
            let base = std::path::Path::new(path).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| path.to_string());
            let drafted = if kind == "csv" {
                let Ok(src) = std::fs::read_to_string(path) else {
                    return refuse(tr!("`{path}` を読めません", "cannot read `{path}`"));
                };
                rulec::import::draft(&src, &name, &base, n)
            } else {
                let Ok(bytes) = std::fs::read(path) else {
                    return refuse(tr!("`{path}` を読めません", "cannot read `{path}`"));
                };
                // A workbook has several sheets; the draft says which one it came from,
                // because "the tariff.xlsx" is not an answer to "which table is this".
                match rulec::xlsx::grid(&bytes, a.get("--sheet")) {
                    Ok((sheet, rows)) => {
                        let from = tr!("{base}（シート {sheet}）", "{base} (sheet {sheet})");
                        rulec::import::draft_rows(rows, &name, &from, n)
                    }
                    Err(e) => return refuse(e),
                }
            };
            match drafted {
                Ok(d) => {
                    print!("{d}");
                    ExitCode::from(0)
                }
                Err(e) => refuse(e),
            }
        }
        "gen" => generate(&files, a.get("--out").unwrap_or("generated"), a.has("--check"), json),
        _ => unreachable!("the table and the dispatch are the same list"),
    }
}

/// §11: the ledger of codes, rendered. `docs/codes.md` and `docs/codes.ja.md` are
/// `--all --format markdown` in the two languages, checked in and held to this output by a
/// test, so there is no second place where a code's meaning is written down.
fn explain(files: &[&String], a: &Args) -> ExitCode {
    let fmt = a.get("--format");
    if a.has("--all") {
        print!(
            "{}",
            match fmt {
                Some("markdown") => rulec::codes::markdown_all(),
                Some("json") => rulec::codes::json_all(),
                _ => rulec::codes::text_all(),
            }
        );
        return ExitCode::from(0);
    }
    let Some(code) = files.first() else {
        return refuse(tr!(
            "`rulec explain <CODE>` か `rulec explain --all` です",
            "it is `rulec explain <CODE>` or `rulec explain --all`"
        ));
    };
    let Some(e) = rulec::codes::find(code) else {
        return refuse(tr!(
            "`{code}` というコードはありません。`rulec explain --all` に全部あります",
            "There is no code `{code}`; `rulec explain --all` lists every one"
        ));
    };
    print!(
        "{}",
        match fmt {
            Some("markdown") => rulec::codes::render_markdown(&e),
            Some("json") => format!("{}\n", rulec::codes::render_json(&e)),
            _ => rulec::codes::render_text(&e),
        }
    );
    ExitCode::from(0)
}

/// §1.5: the one and only formatter. `--check` is for CI: it lists the files that need
/// fixing and exits with 1.
fn fmt(files: &[&String], check_only: bool, json: bool) -> ExitCode {
    let mut dirty = 0u8;
    let (mut unformatted, mut formatted): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    for path in files {
        let Ok(src) = std::fs::read_to_string(path) else {
            eprintln!("{}", tr!("error: `{path}` を読めません", "error: cannot read `{path}`"));
            return ExitCode::from(2);
        };
        let out = rulec::fmt::format(&src);
        if out == src {
            continue;
        }
        if check_only {
            unformatted.push((*path).clone());
            if !json {
                println!("{}", tr!("整形されていません: {path}", "not formatted: {path}"));
            }
            dirty = 1;
        } else if std::fs::write(path, &out).is_err() {
            eprintln!("{}", tr!("error: `{path}` に書けません", "error: cannot write `{path}`"));
            return ExitCode::from(2);
        } else {
            formatted.push((*path).clone());
            if !json {
                println!("{}", tr!("整形しました: {path}", "formatted: {path}"));
            }
        }
    }
    if json {
        println!(
            "{}",
            rulec::json::Obj::new()
                .raw("unformatted", rulec::json::strs(&unformatted))
                .raw("formatted", rulec::json::strs(&formatted))
                .finish()
        );
    }
    ExitCode::from(dirty)
}

/// Fetch the same file at the base revision. None when it is absent (the file is new).
fn base_source(rev: &str, path: &str) -> Option<String> {
    let out = std::process::Command::new("git")
        .args(["show", &format!("{rev}:{path}")])
        .output()
        .ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

fn check(
    files: &[&String],
    json: bool,
    terse: bool,
    show_shadow: bool,
    diff_base: Option<&str>,
    budget: i64,
) -> ExitCode {
    let mut printed = 0usize;
    let mut worst = 0u8;
    for path in files {
        let Ok(src) = std::fs::read_to_string(path) else {
            eprintln!("{}", tr!("error: `{path}` を読めません", "error: cannot read `{path}`"));
            return ExitCode::from(2);
        };
        let lines: Vec<String> = src.lines().map(|s| s.to_string()).collect();
        let r = rulec::report_with(&src, path, budget);
        let mut diags = r.diags;

        // Pairs are matched by the normal form of their cells. With line numbers, inserting a
        // single row would make everything new.
        let mut suppressed = 0usize;
        if let Some(rev) = diff_base {
            let known: std::collections::HashSet<String> = match base_source(rev, path) {
                Some(b) => rulec::vfs::with_git_rev(rev, || rulec::report(&b, path)).diags.iter().filter_map(|d| d.key.clone()).collect(),
                None => Default::default(),
            };
            let before = diags.len();
            diags.retain(|d| match &d.key {
                Some(k) => !known.contains(k),
                None => true,
            });
            suppressed = before - diags.len();
        }
        if show_shadow {
            diags.extend(r.quiet);
        }
        for d in &diags {
            if d.severity == Severity::Error {
                worst = worst.max(1);
            }
            printed += 1;
            if json {
                println!("{}", render_json(d, path));
            } else if terse {
                print!("{}", rulec::diag::render_terse(d));
            }
        }
        if json {
            continue;
        }
        // The default rendering and everything after the findings are shared with the
        // playground (§15.48), so the page and the terminal cannot answer differently.
        if !terse {
            print!("{}", rulec::findings_text(&diags, &lines));
        }
        print!("{}", rulec::check_tail(&r.shadow, &diags, path, suppressed));
    }
    // §11 principle 3 still has to reach the reader; in terse mode it reaches them once,
    // as a pointer, instead of once per finding.
    if terse && printed > 0 {
        print!("{}", rulec::diag::terse_footer());
    }
    ExitCode::from(worst)
}

/// §8.4: the generated files are committed to git, and `--check` in CI verifies that they
/// match a fresh generation.
fn generate(files: &[&String], out_dir: &str, check_only: bool, json: bool) -> ExitCode {
    let mut dirty = 0u8;
    let (mut written, mut stale, mut missing): (Vec<String>, Vec<String>, Vec<String>) =
        (Vec::new(), Vec::new(), Vec::new());
    for path in files {
        let Ok(src) = std::fs::read_to_string(path) else {
            eprintln!("{}", tr!("error: `{path}` を読めません", "error: cannot read `{path}`"));
            return ExitCode::from(2);
        };
        // Nothing is generated from a rule that does not pass (README, AGENTS.md, the
        // generate page). `prepare` only gets as far as the types, so the checks the tables
        // themselves carry — completeness, overlap, dead rows, overflow and the examples —
        // are run here, the way `doc` runs them (§1.6).
        let rep = rulec::report(&src, path);
        if rulec::has_error(&rep.diags) {
            let lines: Vec<String> = src.lines().map(|s| s.to_string()).collect();
            print!("{}", rulec::findings_text(&rep.diags, &lines));
            eprintln!("{}", tr!("error: `{path}` は検査を通っていないので生成しません", "error: `{path}` does not pass check, so nothing is generated"));
            return ExitCode::from(1);
        }
        let (f, c) = match rulec::prepare(&src, path) {
            Ok(v) => v,
            Err(ds) => {
                let lines: Vec<String> = src.lines().map(|s| s.to_string()).collect();
                print!("{}", rulec::findings_text(&ds, &lines));
                eprintln!("{}", tr!("error: `{path}` は検査を通っていないので生成しません", "error: `{path}` does not pass check, so nothing is generated"));
                return ExitCode::from(1);
            }
        };
        let g = rulec::codegen::Gen::new(&f, &c, &src).at(path);
        let alias = f.name.ascii.clone().unwrap_or_else(|| f.name.text.clone());
        let pkg = alias.replace('_', "").to_lowercase();
        // Also emit the vectors and the expected values. Of the three uses in §9.3, the
        // cross-language agreement test and the golden files run on these.
        let suite = rulec::vectors::suite(&f, &c);
        let vs = &suite.vectors;
        let vec_body: String =
            vs.iter().map(|v| rulec::vectors::to_json(&f, &c, v)).collect::<Vec<_>>().join("\n") + "\n";
        let exp_body: String =
            vs.iter().map(|v| rulec::vectors::expected_json(&f, &c, v)).collect::<Vec<_>>().join("\n") + "\n";
        // Every backend says which files it writes (src/backend.rs). The list used to be
        // here, spelled out per language, which is one of the seven places a new target had
        // to be added by hand.
        let mut targets: Vec<(String, String)> = Vec::new();
        for b in rulec::backend::ALL {
            // A walk is written only by the backends that can write one (§15.56). A file that
            // cannot run is worse than a missing one, so the rest are named and skipped.
            if f.elements.is_some() && !b.folds {
                if !json {
                    println!(
                        "{}",
                        tr!(
                            "{}: この言語には並びをたどる規則を生成しません（DESIGN §15.56）",
                            "{}: a rule that walks a sequence is not generated for this target (DESIGN §15.56)",
                            b.name
                        )
                    );
                }
                continue;
            }
            for (rel, body) in (b.files)(&g, &alias, &pkg) {
                targets.push((format!("{out_dir}/{rel}"), body));
            }
        }
        targets.push((format!("{out_dir}/vectors/{alias}.jsonl"), vec_body));
        targets.push((format!("{out_dir}/vectors/{alias}.expected.jsonl"), exp_body));
        // The cases the reference evaluator refuses, where there are any. They have no
        // expected record — refusing is the expectation — so they get a file of their own
        // and `rulec test` holds every generated language to raising on them (§15.56).
        if !suite.refused.is_empty() {
            let body: String = suite
                .refused
                .iter()
                .map(|v| rulec::vectors::refused_json(&f, &c, v))
                .collect::<Vec<_>>()
                .join("\n")
                + "\n";
            targets.push((format!("{out_dir}/vectors/{alias}.refused.jsonl"), body));
        }
        for (p, body) in targets {
            let existing = std::fs::read_to_string(&p).ok();
            if existing.as_deref() == Some(body.as_str()) {
                continue;
            }
            if check_only {
                if existing.is_none() { missing.push(p.clone()) } else { stale.push(p.clone()) }
                if !json {
                    println!("{}", tr!("生成物が古いか手で編集されています: {p}", "generated file is stale or hand-edited: {p}"));
                }
                dirty = 1;
                continue;
            }
            if let Some(dir) = std::path::Path::new(&p).parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            if std::fs::write(&p, &body).is_err() {
                eprintln!("{}", tr!("error: `{p}` に書けません", "error: cannot write `{p}`"));
                return ExitCode::from(2);
            }
            written.push(p.clone());
            if !json {
                println!("{}", tr!("生成しました: {p}", "generated: {p}"));
            }
        }
    }
    if json {
        println!(
            "{}",
            rulec::json::Obj::new()
                .raw("written", rulec::json::strs(&written))
                .raw("stale", rulec::json::strs(&stale))
                .raw("missing", rulec::json::strs(&missing))
                .finish()
        );
    }
    ExitCode::from(dirty)
}

/// If the argument is a directory, collect the `.rule` files under it; a file is returned as
/// is. A directory that does not exist does not become an empty list: its name is returned
/// unchanged so that "cannot read" stops the run (never a silent success with 0 files).
fn expand(arg: &str) -> Vec<String> {
    let p = std::path::Path::new(arg);
    if !p.is_dir() {
        return vec![arg.to_string()];
    }
    let mut out = Vec::new();
    collect_rules(p, &mut out);
    out.sort();
    if out.is_empty() {
        eprintln!("{}", tr!("注意: `{arg}` の下に .rule がありません", "warning: no .rule files under `{arg}`"));
    }
    out
}

fn collect_rules(dir: &std::path::Path, out: &mut Vec<String>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_rules(&p, out);
        } else if p.extension().is_some_and(|x| x == "rule") {
            out.push(p.to_string_lossy().into_owned());
        }
    }
}

/// §1.6: render a rule that passed check as markdown. Read-only; there is no reverse
/// direction. **Not treated as a generated file** — it is never committed; CI renders it and
/// pastes it into the PR. The biggest danger is a stale rendering that lingers looking
/// authoritative, so no long-lived artifact is produced.
fn doc(files: &[&String], out_dir: Option<&str>, html: bool, customer: bool) -> ExitCode {
    if html && customer {
        eprintln!("{}", tr!("error: 顧客向けの案内は markdown だけです。--format html と --audience customer は同時に指定できません", "error: the customer rendering is markdown only; --format html and --audience customer cannot be combined"));
        return ExitCode::from(2);
    }
    for path in files {
        let Ok(src) = std::fs::read_to_string(path) else {
            eprintln!("{}", tr!("error: `{path}` を読めません", "error: cannot read `{path}`"));
            return ExitCode::from(2);
        };
        // A pretty rendering of a broken rule is a lie. Nothing is rendered unless check
        // passes (§1.6).
        let rep = rulec::report(&src, path);
        if rulec::has_error(&rep.diags) {
            let lines: Vec<String> = src.lines().map(|s| s.to_string()).collect();
            for d in rep.diags.iter().filter(|d| d.severity == rulec::diag::Severity::Error) {
                print!("{}", render(d, &lines));
                println!();
            }
            eprintln!("{}", tr!("error: `{path}` は検査を通っていないので資料を書き出しません（§1.6）", "error: `{path}` does not pass check, so it is not rendered (§1.6)"));
            return ExitCode::from(1);
        }
        let Ok((f, c)) = rulec::prepare(&src, path) else {
            eprintln!("{}", tr!("error: `{path}` は検査を通っていません", "error: `{path}` does not pass check"));
            return ExitCode::from(1);
        };
        let body = if html {
            // The page runs the generated JavaScript — the same code `rulec gen` writes.
            let js = rulec::codegen::Gen::new(&f, &c, &src).javascript();
            rulec::doc::render_html(&f, &c, &src, path, &js)
        } else if customer {
            rulec::doc::render_customer(&f, &c, &src, path)
        } else {
            rulec::doc::render(&f, &c, &src, path)
        };
        match out_dir {
            Some(d) => {
                let alias = f.name.ascii.clone().unwrap_or_else(|| f.name.text.clone());
                let p = format!("{d}/{alias}.{}", if html { "html" } else if customer { "customer.md" } else { "md" });
                if let Some(dir) = std::path::Path::new(&p).parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                if std::fs::write(&p, &body).is_err() {
                    eprintln!("{}", tr!("error: `{p}` に書けません", "error: cannot write `{p}`"));
                    return ExitCode::from(2);
                }
                println!("{}", tr!("書き出しました: {p}", "rendered: {p}"));
            }
            None => print!("{body}"),
        }
    }
    ExitCode::from(0)
}

// ── M3 replay ────────────────────────────────────────────────────────────

/// Load a rule and run it through check. A spelling with `@` reads the rule out of git rather
/// than the working tree (§1.4, §15.42): `送料@v3` is the tag `rules/送料/v3`, or failing that
/// `v3` as any revision with `送料.rule` looked up in its tree; `rules/送料.rule@origin/main` is
/// that path at that revision, which is what a pull request compares against.
fn load_rule(spec: &str) -> Result<(String, rulec::ast::RuleFile, rulec::types::Checked), String> {
    load_rule_with(spec, false)
}

/// `lenient` lets a rule whose applied callee is unpinned or changed (E040) through: `diff`
/// exists to show what that change does. A rule read at a revision (`path@rev`) reads the
/// rules it applies from the same revision.
fn load_rule_with(spec: &str, lenient: bool) -> Result<(String, rulec::ast::RuleFile, rulec::types::Checked), String> {
    let prep = |src: &str, path: &str| if lenient { rulec::prepare_lenient(src, path) } else { rulec::prepare(src, path) };
    let not_checked = || tr!("`{spec}` は検査を通っていません", "`{spec}` does not pass check");
    match spec.split_once('@') {
        Some((left, rev)) if !std::path::Path::new(spec).exists() => {
            let src = git_source(left, rev)?;
            let path = if left.ends_with(".rule") { left.to_string() } else { spec.to_string() };
            let (f, c) = rulec::vfs::with_git_rev(rev, || prep(&src, &path)).map_err(|_| not_checked())?;
            Ok((src, f, c))
        }
        _ => {
            let src = std::fs::read_to_string(spec).map_err(|_| tr!("`{spec}` を読めません", "cannot read `{spec}`"))?;
            let (f, c) = prep(&src, spec).map_err(|_| not_checked())?;
            Ok((src, f, c))
        }
    }
}

/// Run git and hand back what it printed, or `None` when it refused (an unknown revision, a
/// path that is not there). Only a git that cannot be started is an error of its own.
fn git(args: &[&str]) -> Result<Option<String>, String> {
    let o = std::process::Command::new("git")
        .args(args)
        .output()
        .map_err(|e| tr!("git を起動できません: {e}", "cannot run git: {e}"))?;
    Ok(o.status.success().then(|| String::from_utf8_lossy(&o.stdout).into_owned()))
}

/// The text of a rule at a revision. `left` is either a path (it ends in `.rule`), read as
/// `git show <rev>:<path>`, or a rule's name, looked up first in the tag `rules/<name>/<rev>`
/// and then in `<rev>` itself.
fn git_source(left: &str, rev: &str) -> Result<String, String> {
    if left.ends_with(".rule") {
        return git(&["show", &format!("{rev}:{left}")])?.ok_or_else(|| {
            tr!("`{rev}` に `{left}` が無いか、`{rev}` が引けません", "no `{left}` at `{rev}`, or `{rev}` cannot be resolved")
        });
    }
    let file = format!("{left}.rule");
    let tag = format!("rules/{left}/{rev}");
    for r in [tag.as_str(), rev] {
        // Which path the rule lives at is looked up inside the tree. `-z` makes the listing
        // NUL-separated: by default git quotes non-ASCII paths in octal, so a file with a
        // Japanese name would not be found by plain string comparison.
        let Some(listing) = git(&["ls-tree", "-r", "-z", "--name-only", r])? else { continue };
        let path = listing
            .split('\0')
            .find(|p| *p == file || p.ends_with(&format!("/{file}")))
            .ok_or_else(|| tr!("`{r}` の中に {file} がありません", "no {file} in `{r}`"))?;
        return git(&["show", &format!("{r}:{path}")])?
            .ok_or_else(|| tr!("`{r}:{path}` が読めません", "cannot read `{r}:{path}`"));
    }
    Err(tr!(
        "`{left}@{rev}`: git タグ `{tag}` も、リビジョン `{rev}` も引けません",
        "`{left}@{rev}`: neither the git tag `{tag}` nor the revision `{rev}` can be resolved"
    ))
}

/// Assemble the default values used for filling from `--manifest` and `--fill` (§10.3).
/// `--fill` is a temporary override for sensitivity analysis, so it applies after the
/// manifest.
fn build_manifest(
    a: &Args,
    f: &rulec::ast::RuleFile,
    c: &rulec::types::Checked,
) -> Result<rulec::fixtures::Manifest, String> {
    let mut m = match a.get("--manifest") {
        Some(path) => {
            let src = std::fs::read_to_string(path).map_err(|_| tr!("`{path}` を読めません", "cannot read `{path}`"))?;
            rulec::fixtures::Manifest::load(&src, f, c)?
        }
        None => rulec::fixtures::Manifest::default(),
    };
    for spec in a.all("--fill") {
        m.add(spec, f, c)?;
    }
    Ok(m)
}

fn fixtures_arg(a: &Args) -> Result<(String, String), String> {
    let path = a
        .get("--fixtures")
        .ok_or_else(|| tr!("--fixtures <file.jsonl> が要ります", "--fixtures <file.jsonl> is required"))?;
    let src = std::fs::read_to_string(path).map_err(|_| tr!("`{path}` を読めません", "cannot read `{path}`"))?;
    Ok((path.to_string(), src))
}

/// §10.2: only the validation of types and ranges is done here. ETL is the user's job.
fn fixtures_lint(files: &[&String], a: &Args, json: bool) -> ExitCode {
    let (Some(jsonl), Some(rule)) = (files.first(), files.get(1)) else {
        eprintln!("error: `rulec fixtures lint <file.jsonl> <file.rule>`");
        return ExitCode::from(2);
    };
    let (_, f, c) = match load_rule(rule) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
    let m = match build_manifest(a, &f, &c) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
    let Ok(src) = std::fs::read_to_string(jsonl.as_str()) else {
        eprintln!("{}", tr!("error: `{jsonl}` を読めません", "error: cannot read `{jsonl}`"));
        return ExitCode::from(2);
    };
    let l = rulec::fixtures::load(&src, &f, &c, &m);
    if json {
        println!("{}", rulec::fixtures::render_lint_json(&l, jsonl));
    } else {
        print!("{}", rulec::fixtures::render_lint(&l, jsonl));
    }
    ExitCode::from(u8::from(!l.problems.is_empty()))
}

/// §10.3: apply the rule to past records and compare against the values produced at the time.
fn replay_cmd(files: &[&String], a: &Args, md: bool, json: bool) -> ExitCode {
    let Some(rule) = files.first() else { return refuse(tr!("規則が要ります", "a rule is required")) };
    let terse = a.has("--terse");
    if json && terse {
        return terse_with_json();
    }
    let r = (|| -> Result<(String, rulec::ast::RuleFile, rulec::types::Checked, rulec::fixtures::Manifest, String, String), String> {
        let (_, f, c) = load_rule(rule)?;
        let m = build_manifest(a, &f, &c)?;
        let (path, src) = fixtures_arg(a)?;
        Ok((String::new(), f, c, m, path, src))
    })();
    let (_, f, c, m, path, src) = match r {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
    let l = rulec::fixtures::load(&src, &f, &c, &m);
    let rep = rulec::replay::replay(&f, &c, &l, &m, &path);
    if json {
        println!("{}", rulec::report::render_json(&rep, &f, &c));
    } else if md {
        print!("{}", rulec::report::markdown(&rep, &f, &c, &tr!("過去再生", "Replay"), terse));
    } else {
        print!("{}", rulec::report::render(&rep, &f, &c, terse));
    }
    ExitCode::from(u8::from(!rep.mismatches.is_empty()))
}

/// §10.4: apply two versions to the same records and report how many change and by how much.
fn diff_cmd(files: &[&String], opts: &Args, md: bool, json: bool) -> ExitCode {
    let (Some(a), Some(b)) = (files.first(), files.get(1)) else {
        eprintln!("{}", tr!("error: `rulec diff <旧> <新> --fixtures <f.jsonl>`", "error: `rulec diff <old> <new> --fixtures <f.jsonl>`"));
        return ExitCode::from(2);
    };
    let terse = opts.has("--terse");
    if json && terse {
        return terse_with_json();
    }
    let r = (|| -> Result<_, String> {
        let (_, of, oc) = load_rule_with(a, true)?;
        let (_, nf, nc) = load_rule_with(b, true)?;
        if of.name.text != nf.name.text {
            return Err(tr!(
                "別の規則を比べようとしています（`{}` と `{}`）",
                "comparing different rules (`{}` and `{}`)",
                of.name.text, nf.name.text
            ));
        }
        let m = build_manifest(opts, &nf, &nc)?;
        let (path, src) = fixtures_arg(opts)?;
        Ok((of, oc, nf, nc, m, path, src))
    })();
    let (of, oc, nf, nc, m, path, src) = match r {
        Ok(v) => v,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
    let l = rulec::fixtures::load(&src, &nf, &nc, &m);
    let rep = rulec::replay::diff((&of, &oc), (&nf, &nc), &l, &m, (a, b));
    let _ = path;
    if json {
        println!("{}", rulec::report::render_json(&rep, &nf, &nc));
    } else if md {
        print!("{}", rulec::report::markdown(&rep, &nf, &nc, &tr!("版の差分", "Version diff"), terse));
    } else {
        print!("{}", rulec::report::render(&rep, &nf, &nc, terse));
    }
    ExitCode::from(u8::from(!rep.mismatches.is_empty()))
}

/// §9.2: decide whether the generated vectors meet the three coverage criteria. The
/// obligations are counted independently of the generator; the missing ones are named and
/// the exit code is 1.
fn coverage(files: &[&String], json: bool) -> ExitCode {
    let mut worst = 0u8;
    for path in files {
        let Ok(src) = std::fs::read_to_string(path) else {
            eprintln!("{}", tr!("error: `{path}` を読めません", "error: cannot read `{path}`"));
            return ExitCode::from(2);
        };
        let Ok((f, c)) = rulec::prepare(&src, path) else {
            eprintln!("{}", tr!("error: `{path}` は検査を通っていません", "error: `{path}` does not pass check"));
            return ExitCode::from(1);
        };
        let (a, vs, rf) = rulec::coverage::audit_file(&f, &c, path);
        if json {
            println!("{}", rulec::coverage::render_json(&a, &vs, &rf, path));
        } else {
            println!("{path}");
            print!("{}", rulec::coverage::render(&a, &vs, &rf));
        }
        if !a.ok() {
            worst = 1;
        }
    }
    ExitCode::from(worst)
}

/// §9: build the vectors from the boundaries. The reference evaluator attaches the expected
/// values and the fired rows.
fn vectors(files: &[&String], out_dir: Option<&str>) -> ExitCode {
    for path in files {
        let Ok(src) = std::fs::read_to_string(path) else {
            eprintln!("{}", tr!("error: `{path}` を読めません", "error: cannot read `{path}`"));
            return ExitCode::from(2);
        };
        let Ok((f, c)) = rulec::prepare(&src, path) else {
            eprintln!("{}", tr!("error: `{path}` は検査を通っていないのでベクタを作りません", "error: `{path}` does not pass check, so no vectors are generated"));
            return ExitCode::from(1);
        };
        let vs = rulec::vectors::generate(&f, &c);
        let body: String = vs
            .iter()
            .map(|v| rulec::vectors::to_json(&f, &c, v))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        match out_dir {
            Some(d) => {
                let alias = f.name.ascii.clone().unwrap_or_else(|| f.name.text.clone());
                let p = format!("{d}/{alias}.jsonl");
                let _ = std::fs::create_dir_all(d);
                if std::fs::write(&p, &body).is_err() {
                    eprintln!("{}", tr!("error: `{p}` に書けません", "error: cannot write `{p}`"));
                    return ExitCode::from(2);
                }
                println!("{}", tr!("ベクタ {} 件: {p}", "{} vectors: {p}", vs.len()));
            }
            None => print!("{body}"),
        }
    }
    ExitCode::from(0)
}

/// §6 of the plan: the API inventory of the generated code. An agent that has to integrate
/// the output should not have to read it first, and a human-written note about the calling
/// convention rots; this is built from the same names the generator emits.
fn api(files: &[&String]) -> ExitCode {
    for path in files {
        let Ok(src) = std::fs::read_to_string(path) else {
            eprintln!("{}", tr!("error: `{path}` を読めません", "error: cannot read `{path}`"));
            return ExitCode::from(2);
        };
        let Ok((f, c)) = rulec::prepare(&src, path) else {
            eprintln!("{}", tr!("error: `{path}` は検査を通っていません", "error: `{path}` does not pass check"));
            return ExitCode::from(1);
        };
        println!("{}", rulec::codegen::Gen::new(&f, &c, &src).api());
    }
    ExitCode::from(0)
}

/// A subcommand that only emits a string for a single rule.
fn one(
    files: &[&String],
    f: impl Fn(&rulec::ast::RuleFile, &rulec::types::Checked, &str) -> Option<String>,
) -> ExitCode {
    for path in files {
        let Ok(src) = std::fs::read_to_string(path) else {
            eprintln!("{}", tr!("error: `{path}` を読めません", "error: cannot read `{path}`"));
            return ExitCode::from(2);
        };
        let Ok((rf, c)) = rulec::prepare(&src, path) else {
            eprintln!("{}", tr!("error: `{path}` は検査を通っていません", "error: `{path}` does not pass check"));
            return ExitCode::from(1);
        };
        if let Some(s) = f(&rf, &c, path) {
            print!("{s}");
        }
    }
    ExitCode::from(0)
}

/// §10: feed the vectors through the adapter of the legacy implementation and compare.
fn verify(files: &[&String], adapter: &[String], json: bool) -> ExitCode {
    let mut worst = 0u8;
    for path in files {
        let Ok(src) = std::fs::read_to_string(path) else {
            eprintln!("{}", tr!("error: `{path}` を読めません", "error: cannot read `{path}`"));
            return ExitCode::from(2);
        };
        let Ok((f, c)) = rulec::prepare(&src, path) else {
            eprintln!("{}", tr!("error: `{path}` は検査を通っていません", "error: `{path}` does not pass check"));
            return ExitCode::from(1);
        };
        let vs = rulec::vectors::generate(&f, &c);
        match rulec::verify::run(&f, &c, adapter, &vs) {
            Ok(rep) => {
                if json {
                    println!("{}", rulec::report::render_json(&rep, &f, &c));
                } else {
                    print!("{}", rulec::report::render(&rep, &f, &c, false));
                }
                if !rep.mismatches.is_empty() {
                    worst = 1;
                }
            }
            Err(e) => {
                eprintln!("error: {e}");
                return ExitCode::from(2);
            }
        }
    }
    ExitCode::from(worst)
}
