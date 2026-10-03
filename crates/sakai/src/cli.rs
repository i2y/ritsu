//! The commands and their flags, in one table (DESIGN 6). `--help` is drawn from it and the
//! command line is read against it, so a flag cannot be documented and ignored, or taken and left
//! undocumented. An unknown flag, a value outside a closed set, a flag without its value and a
//! flag given twice all stop the run with exit 2: an agent that is told nothing believes the flag
//! worked (koyomi's and chobo's `cli.rs` work the same way).

use crate::i18n::{Lang, Text, pad, width};

pub struct Flag {
    pub name: &'static str,
    /// The placeholder of the value; None for a flag that takes none.
    pub value: Option<&'static str>,
    /// The values taken, when the set is closed.
    pub choices: &'static [&'static str],
    pub default: Option<String>,
    pub help: Text,
}

fn flag(name: &'static str, value: Option<&'static str>, help: Text) -> Flag {
    Flag { name, value, choices: &[], default: None, help }
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

    pub fn spelled(&self) -> String {
        match self.value {
            Some(v) => format!("{} {v}", self.name),
            None => self.name.to_string(),
        }
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
        flag("--lang", Some("ja|en"), tr!("文面の言語。無ければ環境変数 SAKAI_LANG、それも無ければ en", "the language of the text; else the SAKAI_LANG environment variable, else en"))
            .choices(&["ja", "en"])
            .default("en"),
        flag("--help", None, tr!("この画面を出す", "print this page")),
    ]
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
    crate::codes::ledger().iter().filter(|e| e.implemented() && !e.code.starts_with("E5")).map(|e| e.code).collect()
}

pub fn commands() -> Vec<Cmd> {
    vec![
        Cmd {
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
                (2, tr!("引数の誤り、読めないファイル", "bad arguments, or a file that cannot be read")),
            ],
            examples: vec!["sakai check tests/maps/基本/基本.ctx", "sakai check tests/maps --format json --lang ja"],
            codes: check_codes(),
        },
        Cmd {
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
            examples: vec!["sakai build examples/通販/通販.ctx --target import-linter", "sakai build examples/通販/通販.ctx --target go-arch-lint --check --lang ja"],
            codes: vec!["E501", "E502"],
        },
        Cmd {
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
            examples: vec!["sakai export cml examples/通販/通販.ctx --out shop.cml"],
            codes: vec![],
        },
        Cmd {
            name: "api",
            args: "<map.ctx>",
            purpose: tr!(
                "地図、属し方、境界を越える参照を JSON で出す（yurai などほかのツールが読む形）",
                "print the map, who owns each artifact, and the references that cross a boundary, as JSON for other tools (yurai, for one)"
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

fn usage_line(c: &Cmd) -> String {
    let mut o = format!("sakai {} {}", c.name, c.args).trim_end().to_string();
    for f in &c.flags {
        o.push_str(&format!(" [{}]", f.spelled()));
    }
    o
}

/// `sakai <cmd> --help` and `sakai help <cmd>`.
pub fn help_cmd(c: &Cmd, lang: Lang) -> String {
    let t = |x: Text| x.get(lang).to_string();
    let mut o = format!("sakai {} — {}\n\n", c.name, c.purpose.get(lang));
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
        o.push_str(&format!("  {}  {}{d}\n", pad(&f.spelled(), w), f.help.get(lang)));
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
        o.push_str(&t(tr!("\n出しうる診断（`sakai explain <CODE>` が引きます）:\n  ", "\nDiagnostics it can print (`sakai explain <CODE>` looks one up):\n  ")));
        o.push_str(&c.codes.join(" "));
        o.push('\n');
    }
    o
}

/// `sakai --help`.
pub fn help_all(lang: Lang) -> String {
    let t = |x: Text| x.get(lang).to_string();
    let cs = commands();
    let mut o = format!("sakai {}\n\n", env!("CARGO_PKG_VERSION"));
    o.push_str(&t(tr!(
        "境界づけられたコンテキストと、そのあいだの関係を書く小さな言語。コンテキストマップのうち、成果物と突き合わせられる部分だけを書き、確かめる。\n\n",
        "A small language for bounded contexts and the relationships between them: the part of a context map that can be checked against the artifacts, and checked.\n\n"
    )));
    o.push_str(&t(tr!("使い方:\n", "Usage:\n")));
    let w = cs.iter().map(|c| width(&format!("{} {}", c.name, c.args))).max().unwrap_or(0);
    for c in &cs {
        o.push_str(&format!("  sakai {}  {}\n", pad(&format!("{} {}", c.name, c.args), w), c.purpose.get(lang)));
    }
    o.push_str(&t(tr!(
        "\n一つのコマンドの詳しい説明は `sakai <cmd> --help`（`sakai help <cmd>` も同じ）。\n",
        "\nFor one command in detail: `sakai <cmd> --help` (`sakai help <cmd>` is the same page).\n"
    )));
    o.push_str(&t(tr!(
        "どのコマンドにも --lang ja|en を付けられます（既定は en。環境変数 SAKAI_LANG でも指定できます）。\n",
        "Every command takes --lang ja|en (default en; the SAKAI_LANG environment variable works too).\n"
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
            return Err(tr!("知らないフラグ `{name}` です。`sakai {cmd} --help` を読んでください", "unknown flag `{name}`; run `sakai {cmd} --help`"));
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
                    Some(v) if !v.starts_with("--") => v.clone(),
                    _ => {
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
        if out.has(f.name) {
            let n = f.name;
            return Err(tr!("`{n}` が二度書かれています", "`{n}` is given twice"));
        }
        out.got.push((f.name, v));
        i += 1;
    }
    Ok(out)
}
