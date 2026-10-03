//! The commands and their flags, in one table (DESIGN 7, rulec's §12.1). `--help` is drawn
//! from it and the command line is read against it, so a flag cannot be documented and
//! ignored, or taken and left undocumented. An unknown flag, a value outside a closed set, a
//! flag without its value and a flag given twice (unless the table says it repeats) all stop
//! the run with exit 2: an agent that is told nothing believes the flag worked.

use crate::i18n::{Lang, Text, pad, width};

pub struct Flag {
    pub name: &'static str,
    /// The placeholder of the value; None for a flag that takes none.
    pub value: Option<&'static str>,
    /// The values taken, when the set is closed.
    pub choices: &'static [&'static str],
    pub default: Option<String>,
    /// Whether it may be given more than once.
    pub repeats: bool,
    pub help: Text,
}

fn flag(name: &'static str, value: Option<&'static str>, help: Text) -> Flag {
    Flag { name, value, choices: &[], default: None, repeats: false, help }
}

impl Flag {
    fn choices(mut self, c: &'static [&'static str]) -> Flag {
        self.choices = c;
        self
    }

    fn default(mut self, d: impl Into<String>) -> Flag {
        self.default = Some(d.into());
        self
    }

    fn repeats(mut self) -> Flag {
        self.repeats = true;
        self
    }

    pub fn spelled(&self) -> String {
        let s = match self.value {
            Some(v) => format!("{} {v}", self.name),
            None => self.name.to_string(),
        };
        if self.repeats { format!("{s}...") } else { s }
    }
}

pub struct Cmd {
    pub name: &'static str,
    pub args: &'static str,
    pub purpose: Text,
    pub params: Vec<(&'static str, Text)>,
    pub flags: Vec<Flag>,
    pub exits: Vec<(u8, Text)>,
    pub examples: Vec<&'static str>,
    pub codes: Vec<&'static str>,
}

pub fn global_flags() -> Vec<Flag> {
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

fn every_code() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = crate::codes::ledger().iter().map(|e| e.code).collect();
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
            name: "check",
            args: "<path>...",
            purpose: tr!(
                "検査する。名前、出典の写しと固定、確かめた記録と今のハッシュ、版の期間、カバレッジ、範囲",
                "check the names, the copies of the sources against their pins, every record against the hashes now, the periods, the coverage and the scope"
            ),
            params: vec![("<path>...", tr!("{}", "{}", PATHS.0; PATHS.1))],
            flags: vec![flag("--format", Some("json"), tr!("プロジェクトに一つの JSON（診断と差分も）", "one JSON object for the project, the diagnostics and their diffs in it")).choices(&["json"])],
            exits: vec![
                (0, tr!("エラーなし（警告はありうる）", "no errors (there may be warnings)")),
                (1, tr!("エラーが一つ以上（印の付いたリンクも）", "at least one error (a marked link is one)")),
                (2, tr!("引数の誤り、読めないファイル、まだ読めない成果物", "bad arguments, a file that cannot be read, or an artifact yuen does not read yet")),
            ],
            examples: vec!["yuen check tests/fixtures/period", "yuen check tests/fixtures/period --format json --lang ja"],
            codes: every_code(),
        },
        Cmd {
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
        },
        Cmd {
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
                (1, tr!("構文、名前、出典、成果物にエラーがある（端が決まらないので、何も書かない）", "the words, names, sources or artifacts have errors (some end cannot be made, so nothing is written)")),
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
            name: "source",
            args: "fetch|pin|outdated <path>...",
            purpose: tr!(
                "出典の写しを取る（fetch）、写しのハッシュを固定の行に書く（pin）、元が変わったかを問う（outdated）。通信するのは fetch と outdated だけ",
                "fetch the copies of the sources, pin their hashes, or ask whether the originals moved on (only fetch and outdated read the network)"
            ),
            params: vec![
                (
                    "fetch|pin|outdated",
                    tr!(
                        "fetch は e-Gov か eCFR か url から写しを取る。pin は写しの SHA-256 の先頭 16 桁を書く（ほかは一字も変えない）。outdated は asof より後の版と url を問う",
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
                        "引数の誤り、読めないファイル、書けないファイル、curl の失敗、まだ読めない借りた出典（outdated）",
                        "bad arguments, a file that cannot be read or written, curl failing, or a borrowed source yuen does not read yet (outdated)"
                    ),
                ),
            ],
            examples: vec!["yuen source fetch tests/fixtures/period", "yuen source pin tests/fixtures/period", "yuen source outdated tests/fixtures/period --lang ja"],
            codes: vec![],
        },
        Cmd {
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

fn usage_line(c: &Cmd) -> String {
    let mut o = format!("yuen {} {}", c.name, c.args).trim_end().to_string();
    for f in &c.flags {
        o.push_str(&format!(" [{}]", f.spelled()));
    }
    o
}

/// `yuen <cmd> --help` and `yuen help <cmd>`.
pub fn help_cmd(c: &Cmd, lang: Lang) -> String {
    let t = |x: Text| x.get(lang).to_string();
    let mut o = format!("yuen {} — {}\n\n", c.name, c.purpose.get(lang));
    o.push_str(&t(tr!("使い方:\n", "Usage:\n")));
    o.push_str(&format!("  {}\n", usage_line(c)));
    if !c.params.is_empty() {
        o.push_str(&t(tr!("\n引数:\n", "\nArguments:\n")));
        let w = c.params.iter().map(|(n, _)| width(n)).max().unwrap_or(0);
        for (n, h) in &c.params {
            o.push_str(&format!("  {}  {}\n", pad(n, w), h.get(lang)));
        }
    }
    o.push_str(&t(tr!("\nフラグ:\n", "\nFlags:\n")));
    let globals = global_flags();
    let flags: Vec<&Flag> = c.flags.iter().chain(globals.iter()).collect();
    let w = flags.iter().map(|f| width(&f.spelled())).max().unwrap_or(0);
    for f in &flags {
        let d = match &f.default {
            Some(d) => t(tr!("（既定 {d}）", " (default {d})")),
            None => String::new(),
        };
        let r = if f.repeats { t(tr!("（何度でも書ける）", " (may be given more than once)")) } else { String::new() };
        o.push_str(&format!("  {}  {}{d}{r}\n", pad(&f.spelled(), w), f.help.get(lang)));
    }
    o.push_str(&t(tr!("\nexit code:\n", "\nExit codes:\n")));
    for (n, h) in &c.exits {
        o.push_str(&format!("  {n}  {}\n", h.get(lang)));
    }
    o.push_str(&t(tr!("\n例:\n", "\nExamples:\n")));
    for e in &c.examples {
        o.push_str(&format!("  $ {e}\n"));
    }
    if !c.codes.is_empty() {
        o.push_str(&t(tr!("\n出しうる診断（`yuen explain <CODE>` が引きます）:\n  ", "\nDiagnostics it can print (`yuen explain <CODE>` looks one up):\n  ")));
        o.push_str(&c.codes.join(" "));
        o.push('\n');
    }
    o
}

/// `yuen --help`.
pub fn help_all(lang: Lang) -> String {
    let t = |x: Text| x.get(lang).to_string();
    let cs = commands();
    let mut o = format!("yuen {}\n\n", env!("CARGO_PKG_VERSION"));
    o.push_str(&t(tr!(
        "要件の出どころを書く。満たすものにつなぐ。どこかが変われば止める。\n\n",
        "Write where each requirement comes from. Link what meets it. Stop when anything moves.\n\n"
    )));
    o.push_str(&t(tr!("使い方:\n", "Usage:\n")));
    let w = cs.iter().map(|c| width(&format!("{} {}", c.name, c.args))).max().unwrap_or(0);
    for c in &cs {
        o.push_str(&format!("  yuen {}  {}\n", pad(&format!("{} {}", c.name, c.args), w), c.purpose.get(lang)));
    }
    o.push_str(&t(tr!(
        "\n一つのコマンドの詳しい説明は `yuen <cmd> --help`（`yuen help <cmd>` も同じ）。\n",
        "\nFor one command in detail: `yuen <cmd> --help` (`yuen help <cmd>` is the same page).\n"
    )));
    o.push_str(&t(tr!(
        "どのコマンドにも --lang ja|en（既定は en。環境変数 YUEN_LANG でも指定できる）と --root <dir> を付けられます。\n",
        "Every command takes --lang ja|en (default en; the YUEN_LANG environment variable works too) and --root <dir>.\n"
    )));
    o.push_str(&t(tr!(
        "exit code: 0 エラーなし / 1 エラーあり / 2 引数の誤りか、読めないファイル\n",
        "Exit codes: 0 no errors / 1 errors / 2 bad arguments or a file that cannot be read\n"
    )));
    o
}

/// What one command line said.
pub struct Args {
    pub got: Vec<(&'static str, String)>,
    pub pos: Vec<String>,
}

impl Args {
    pub fn has(&self, n: &str) -> bool {
        self.got.iter().any(|(k, _)| *k == n)
    }

    pub fn get(&self, n: &str) -> Option<&str> {
        self.got.iter().find(|(k, _)| *k == n).map(|(_, v)| v.as_str())
    }

    pub fn all(&self, n: &str) -> Vec<&str> {
        self.got.iter().filter(|(k, _)| *k == n).map(|(_, v)| v.as_str()).collect()
    }
}

/// Read a command line against the command's flags.
pub fn parse(c: &Cmd, argv: &[String]) -> Result<Args, Text> {
    let globals = global_flags();
    let find = |name: &str| c.flags.iter().chain(globals.iter()).find(|f| f.name == name);
    let mut out = Args { got: Vec::new(), pos: Vec::new() };
    let mut i = 0;
    while i < argv.len() {
        let a = &argv[i];
        if a == "-h" {
            out.got.push(("--help", String::new()));
            i += 1;
            continue;
        }
        if !a.starts_with('-') || a == "-" {
            out.pos.push(a.clone());
            i += 1;
            continue;
        }
        let (name, inline) = match a.split_once('=') {
            Some((k, v)) => (k.to_string(), Some(v.to_string())),
            None => (a.clone(), None),
        };
        let Some(f) = find(&name) else {
            let cmd = c.name;
            return Err(tr!("知らないフラグ `{name}` です。`yuen {cmd} --help` を読んでください", "unknown flag `{name}`; run `yuen {cmd} --help`"));
        };
        let v = match (f.value, inline) {
            (None, Some(v)) => {
                let n = f.name;
                return Err(tr!("`{n}` は値を取りません（`={v}` が付いています）", "`{n}` takes no value (it was given `={v}`)"));
            }
            (None, None) => String::new(),
            (Some(_), Some(v)) => v,
            (Some(_), None) => {
                i += 1;
                match argv.get(i) {
                    Some(v) => v.clone(),
                    None => {
                        let s = f.spelled();
                        return Err(tr!("`{s}` に値がありません", "`{s}` is missing its value"));
                    }
                }
            }
        };
        if !f.choices.is_empty() && !f.choices.contains(&v.as_str()) {
            let (n, cs) = (f.name, f.choices.join(" | "));
            return Err(tr!("`{n} {v}` は知らない値です。書けるのは {cs} だけです", "`{n} {v}` is not a value this flag takes; it takes only {cs}"));
        }
        if out.has(f.name) && !f.repeats {
            let n = f.name;
            return Err(tr!("`{n}` が二度書かれています", "`{n}` is given twice"));
        }
        out.got.push((f.name, v));
        i += 1;
    }
    Ok(out)
}
