//! The commands and their flags, in one table (DESIGN 5, rulec's §12.1). `--help` is drawn
//! from it and the command line is read against it, so a flag cannot be documented and
//! ignored, or taken and left undocumented. An unknown flag, a value outside a closed set, a
//! flag without its value and a flag given twice all stop the run with exit 2: an agent that
//! is told nothing believes the flag worked.

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
        flag("--lang", Some("ja|en"), tr!("文面の言語。無ければ環境変数 KOYOMI_LANG、それも無ければ en", "the language of the text; else the KOYOMI_LANG environment variable, else en"))
            .choices(&["ja", "en"])
            .default("en"),
        flag("--help", None, tr!("この画面を出す", "print this page")),
    ]
}

fn every_code() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = crate::codes::ledger().iter().map(|e| e.code).collect();
    v.sort_by_key(|c| (c.starts_with('W'), *c));
    v
}

pub fn commands() -> Vec<Cmd> {
    vec![
        Cmd {
            name: "check",
            args: "<file.cal>...",
            purpose: tr!(
                "検査する。範囲のすべての入力で計算し、条件と例を確かめる",
                "compute on every input of the range, and hold the claims and the examples to it"
            ),
            params: vec![("<file.cal>...", tr!(".cal のファイル。ディレクトリなら下の .cal を全部（パスの順）", ".cal files; a directory stands for every .cal under it, in path order"))],
            flags: vec![
                flag("--format", Some("json"), tr!("ファイルごとに一行の JSON（成り立たない入力も並べる）", "one JSON object per file, with the inputs that fail")).choices(&["json"]),
                flag("--budget", Some("<n>"), tr!("確かめる入力の組み合わせの上限。超えたら E305", "the most input combinations to check; over it, E305"))
                    .default(crate::check::DEFAULT_BUDGET.to_string()),
            ],
            exits: vec![
                (0, tr!("エラーなし（警告はありうる）", "no errors (there may be warnings)")),
                (1, tr!("エラーが一つ以上", "at least one error")),
                (2, tr!("引数の誤り、読めないファイル", "bad arguments, or a file that cannot be read")),
            ],
            examples: vec!["koyomi check examples/", "koyomi check examples/支払_月末締め翌々月末払い.cal --format json --lang ja"],
            codes: every_code(),
        },
        Cmd {
            name: "eval",
            args: "<file.cal> <name>=<value>...",
            purpose: tr!(
                "一つの入力ですべての日付を計算し、計算の段を見せる。カレンダーなら、その日が営業日か休みか",
                "compute every date on one input and show each step; for a calendar, whether a day is open or closed"
            ),
            params: vec![
                ("<file.cal>", tr!("dates か calendar のファイル", "a dates or a calendar file")),
                (
                    "<name>=<value>...",
                    tr!(
                        "入力ごとに `受領日=2026-04-01`（名前か別名）。カレンダーなら日付を一つ（`2026-05-04`）",
                        "one `received=2026-04-01` per input (name or alias); for a calendar, one date (`2026-05-04`)"
                    ),
                ),
            ],
            flags: vec![flag("--format", Some("json"), tr!("機械向けの JSON", "machine-facing JSON")).choices(&["json"])],
            exits: vec![
                (0, tr!("計算できた", "computed")),
                (1, tr!("ファイルにエラーがある、または計算が止まった（範囲の外、表の外、無い日）", "the file has errors, or the computation stopped (outside the range, outside the table, a missing day)")),
                (2, tr!("引数の誤り、読めないファイル", "bad arguments, or a file that cannot be read")),
            ],
            examples: vec![
                "koyomi eval examples/支払_20日締め翌月10日払い.cal 受領日=2026-04-01 --lang ja",
                "koyomi eval examples/calendars/東京の営業日.cal 2026-05-04",
            ],
            codes: vec![],
        },
        Cmd {
            name: "gen",
            args: "<file.cal>...",
            purpose: tr!(
                "TypeScript・Python・Go・Rust・SQL のコードとランナーを生成する。検査を通らないファイルからは生成しない",
                "generate TypeScript, Python, Go, Rust and SQL, each with a runner; nothing is generated from a file that does not pass check"
            ),
            params: vec![("<file.cal>...", tr!(".cal のファイル。ディレクトリなら下の .cal を全部（パスの順）", ".cal files; a directory stands for every .cal under it, in path order"))],
            flags: vec![
                flag("--target", Some("typescript|python|go|rust|sql"), tr!("この出力先だけを生成する。無ければ五つ全部", "generate this target only; all five without it"))
                    .choices(&["typescript", "python", "go", "rust", "sql"]),
                flag("--out", Some("<dir>"), tr!("生成物を書くディレクトリ", "the directory the generated files go to")).default("generated"),
                flag("--check", None, tr!("書き出さずに、ディスクの生成物が、いま生成するものと一字一句同じかを見る。違えば 1（CI 用）", "write nothing; exit 1 if a file on disk differs from what gen would write (for CI)")),
            ],
            exits: vec![
                (0, tr!("生成した、または --check で全部が同じだった", "generated, or --check found every file the same")),
                (1, tr!("検査を通らないファイルがある、または --check で違う・無いファイルがある", "a file does not pass check, or --check found a file that differs or is missing")),
                (2, tr!("引数の誤り、読めないファイル、書けないファイル", "bad arguments, or a file that cannot be read or written")),
            ],
            examples: vec![
                "koyomi gen examples/支払_20日締め翌月10日払い.cal --out generated",
                "koyomi gen examples/net30.cal --target python --lang ja",
                "koyomi gen examples/net30.cal --check",
            ],
            codes: vec![],
        },
        Cmd {
            name: "vectors",
            args: "<file.cal>",
            purpose: tr!(
                "範囲のすべての入力について、参照インタプリタの結果を JSON Lines で出す（範囲の両端の外の行も）",
                "print the reference interpreter's result for every input of the range as JSON Lines, with the inputs just outside it"
            ),
            params: vec![("<file.cal>", tr!("検査を通る dates か calendar のファイル", "a dates or calendar file that passes check"))],
            flags: vec![],
            exits: vec![(0, tr!("出した", "printed")), (1, tr!("ファイルにエラーがある", "the file has errors")), (2, tr!("引数の誤り、読めないファイル", "bad arguments, or a file that cannot be read"))],
            examples: vec!["koyomi vectors examples/支払_20日締め翌月10日払い.cal", "koyomi vectors examples/calendars/東京の営業日.cal"],
            codes: vec![],
        },
        Cmd {
            name: "doc",
            args: "<file.cal>",
            purpose: tr!(
                "承認する人（経理、法務、会社のカレンダーを決める人）が読むページを出す。Markdown か、一枚の HTML",
                "print the page for whoever approves the file (accounting, legal, whoever keeps the calendar): Markdown, or one HTML file"
            ),
            params: vec![(
                "<file.cal>",
                tr!(
                    "dates か calendar のファイル。条件や例が成り立たないファイルにもページを出し、成り立たない入力の日を太字にする",
                    "a dates or calendar file; a file whose claims or examples fail still gets a page, with the input days they fail on in bold"
                ),
            )],
            flags: vec![
                flag("--format", Some("markdown|html"), tr!("html は一枚の HTML（外のファイルを読まない。明るい配色と暗い配色）", "html is one HTML file (it loads nothing else; light and dark)"))
                    .choices(&["markdown", "html"])
                    .default("markdown"),
                flag(
                    "--months",
                    Some("<YYYY-MM>..<YYYY-MM>"),
                    tr!(
                        "月の表に出す月。無ければ、dates は入力と計算した日付が入る月（24 か月を超えれば、成り立たない入力のある月とエッジケースの月だけ）、calendar はデータの範囲の最後の 24 か月",
                        "the months the tables show; without it, a dates file shows the months its inputs and computed dates fall in (over 24, only the months of the inputs claims fail on and of the edge cases), and a calendar the last 24 months of what it knows"
                    ),
                ),
            ],
            exits: vec![
                (0, tr!("ページを出した（条件が成り立たないときも）", "printed the page (also when claims fail)")),
                (1, tr!(
                    "ページを作れないエラーがある（構文、名前、出典、無い日の扱い、計算が止まる、予算）。診断は標準エラーに出す",
                    "an error keeps the page from being made (words, names, sources, a missing day, a computation that stops, the budget); the diagnostics go to standard error"
                )),
                (2, tr!("引数の誤り、読めないファイル", "bad arguments, or a file that cannot be read")),
            ],
            examples: vec![
                "koyomi doc examples/支払_月末締め翌々月末払い.cal --lang ja",
                "koyomi doc examples/calendars/東京の営業日.cal --format html --months 2026-04..2027-03",
            ],
            codes: vec![],
        },
        Cmd {
            name: "api",
            args: "<file.cal>",
            purpose: tr!(
                "関数・引数・カレンダー・データの範囲・出典のハッシュを JSON で出す（ほかのツールが読む形）",
                "print the functions, their inputs, the calendar, the data range and the sources' digests as JSON, for other tools"
            ),
            params: vec![("<file.cal>", tr!("検査を通る dates か calendar のファイル", "a dates or calendar file that passes check"))],
            flags: vec![],
            exits: vec![(0, tr!("出した", "printed")), (1, tr!("ファイルにエラーがある", "the file has errors")), (2, tr!("引数の誤り、読めないファイル", "bad arguments, or a file that cannot be read"))],
            examples: vec!["koyomi api examples/支払_20日締め翌月10日払い.cal", "koyomi api examples/calendars/東京の営業日.cal"],
            codes: vec![],
        },
        Cmd {
            name: "source",
            args: "fetch|pin|outdated <file.cal>",
            purpose: tr!(
                "出典の写しを扱う。fetch は写しを取ってきて .cal の隣に置き、pin は写しのハッシュを .cal に書き、outdated は元が変わったかを問う",
                "handle the copies of the sources: fetch brings them beside the .cal, pin writes their digests into it, outdated asks whether the originals moved on"
            ),
            params: vec![
                (
                    "fetch|pin|outdated",
                    tr!(
                        "fetch と outdated は通信する（curl を呼ぶ。表は url から、法令は e-Gov 法令 API v2 から）。pin は .cal の sha256: の 16 桁だけを書き換える（固定の無い出典や条には足す）",
                        "fetch and outdated read the network (through curl: a table from its url, a law from e-Gov law API v2); pin rewrites only the 16 digits after sha256: in the .cal, and adds a pin where there is none"
                    ),
                ),
                ("<file.cal>", tr!("`source` を宣言した calendar か dates のファイル", "a calendar or dates file that declares sources")),
            ],
            flags: vec![],
            exits: vec![
                (0, tr!("済んだ。outdated では、元が変わっていない", "done; for outdated, nothing has moved on")),
                (1, tr!("outdated: 元が変わった（表の url の先、法令の後の版）。.cal が読めないときも", "outdated: something moved on (what is at a table's url, a later revision of a law); also when the .cal cannot be parsed")),
                (2, tr!("引数の誤り、読めないファイル、curl の失敗", "bad arguments, a file that cannot be read, or curl failing")),
            ],
            examples: vec![
                "koyomi source fetch examples/calendars/東京の営業日.cal",
                "koyomi source pin examples/calendars/東京の営業日.cal",
                "koyomi source outdated examples/民法の期間.cal",
            ],
            codes: vec![],
        },
        Cmd {
            name: "explain",
            args: "<CODE>",
            purpose: tr!("診断のコードを引く。いつ出るか、どう直すか、最小の再現", "look a diagnostic code up: when it comes, how to fix it, the smallest reproduction"),
            params: vec![("<CODE>", tr!("`E301` のような診断のコード。`--all` なら要らない", "a diagnostic code such as `E301`; not needed with `--all`"))],
            flags: vec![
                flag("--all", None, tr!("全部のコードを出す", "print every code")),
                flag("--format", Some("markdown"), tr!("Markdown で出す（docs/codes.md の元）", "print Markdown (what docs/codes.md is made from)")).choices(&["markdown"]),
            ],
            exits: vec![(0, tr!("引けた", "found")), (2, tr!("そのコードが無い、または引数の誤り", "no such code, or bad arguments"))],
            examples: vec!["koyomi explain E201", "koyomi explain --all --format markdown --lang ja"],
            codes: vec![],
        },
    ]
}

fn usage_line(c: &Cmd) -> String {
    let mut o = format!("koyomi {} {}", c.name, c.args).trim_end().to_string();
    for f in &c.flags {
        o.push_str(&format!(" [{}]", f.spelled()));
    }
    o
}

/// `koyomi <cmd> --help` and `koyomi help <cmd>`.
pub fn help_cmd(c: &Cmd, lang: Lang) -> String {
    let t = |x: Text| x.get(lang).to_string();
    let mut o = format!("koyomi {} — {}\n\n", c.name, c.purpose.get(lang));
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
        o.push_str(&t(tr!("\n出しうる診断（`koyomi explain <CODE>` が引きます）:\n  ", "\nDiagnostics it can print (`koyomi explain <CODE>` looks one up):\n  ")));
        o.push_str(&c.codes.join(" "));
        o.push('\n');
    }
    o
}

/// `koyomi --help`.
pub fn help_all(lang: Lang) -> String {
    let t = |x: Text| x.get(lang).to_string();
    let cs = commands();
    let mut o = format!("koyomi {}\n\n", env!("CARGO_PKG_VERSION"));
    o.push_str(&t(tr!(
        "締めと支払、営業日、月の足し算を書く小さな言語。書いた条件を範囲のすべての日で確かめる。\n\n",
        "A small language for closing days, payment days, business days and month arithmetic, checked on every day of its range.\n\n"
    )));
    o.push_str(&t(tr!("使い方:\n", "Usage:\n")));
    let w = cs.iter().map(|c| width(&format!("{} {}", c.name, c.args))).max().unwrap_or(0);
    for c in &cs {
        o.push_str(&format!("  koyomi {}  {}\n", pad(&format!("{} {}", c.name, c.args), w), c.purpose.get(lang)));
    }
    o.push_str(&t(tr!(
        "\n一つのコマンドの詳しい説明は `koyomi <cmd> --help`（`koyomi help <cmd>` も同じ）。\n",
        "\nFor one command in detail: `koyomi <cmd> --help` (`koyomi help <cmd>` is the same page).\n"
    )));
    o.push_str(&t(tr!(
        "どのコマンドにも --lang ja|en を付けられます（既定は en。環境変数 KOYOMI_LANG でも指定できます）。\n",
        "Every command takes --lang ja|en (default en; the KOYOMI_LANG environment variable works too).\n"
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
        if !a.starts_with('-') || a == "-" || is_negative_number(a) {
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
            return Err(tr!("知らないフラグ `{name}` です。`koyomi {cmd} --help` を読んでください", "unknown flag `{name}`; run `koyomi {cmd} --help`"));
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
        if out.has(f.name) {
            let n = f.name;
            return Err(tr!("`{n}` が二度書かれています", "`{n}` is given twice"));
        }
        out.got.push((f.name, v));
        i += 1;
    }
    Ok(out)
}

fn is_negative_number(a: &str) -> bool {
    a.len() > 1 && a[1..].chars().all(|c| c.is_ascii_digit())
}
