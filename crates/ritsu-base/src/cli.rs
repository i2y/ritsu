//! The commands and their flags, in one table (DESIGN 4.4, rulec's §12.1). `--help` is drawn
//! from it and the command line is read against it, so a flag cannot be documented and
//! ignored, or taken and left undocumented. An unknown flag, a value outside a closed set, a
//! flag without its value, and a flag given twice (unless the table says it repeats) all stop
//! the run with exit 2: an agent that is told nothing believes the flag worked.
//!
//! koyomi, yuen and sakai wrote this table three times; it differed in two places, which are
//! now [`Reading`]: whether `-5` is an argument (koyomi), and whether a value may start with
//! `--` (sakai says no).

use crate::text::{Lang, Text, pad, width};
use crate::tr;

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

pub fn flag(name: &'static str, value: Option<&'static str>, help: Text) -> Flag {
    Flag { name, value, choices: &[], default: None, repeats: false, help }
}

impl Flag {
    pub fn choices(mut self, c: &'static [&'static str]) -> Flag {
        self.choices = c;
        self
    }

    pub fn default(mut self, d: impl Into<String>) -> Flag {
        self.default = Some(d.into());
        self
    }

    pub fn repeats(mut self) -> Flag {
        self.repeats = true;
        self
    }

    /// `--format json`, `--map <spec>...`, `--help`.
    pub fn spelled(&self) -> String {
        let s = match self.value {
            Some(v) => format!("{} {v}", self.name),
            None => self.name.to_string(),
        };
        if self.repeats { format!("{s}...") } else { s }
    }
}

/// `--lang ja|en`, whose help names the variables read without it, in the order
/// [`crate::text::Lang::pick`] reads them: the tool's own (`KOYOMI_LANG`), then `RITSU_LANG`.
pub fn lang_flag(var: &str) -> Flag {
    // ritsu's own command reads RITSU_LANG alone, and says it once
    let what = if var == "RITSU_LANG" {
        tr!("文面の言語。無ければ環境変数 RITSU_LANG、それも無ければ en", "the language of the text; else the RITSU_LANG environment variable, else en")
    } else {
        tr!(
            "文面の言語。無ければ環境変数 {var}、次に RITSU_LANG、どちらも無ければ en",
            "the language of the text; else the {var} environment variable, then RITSU_LANG, else en"
        )
    };
    flag("--lang", Some("ja|en"), what)
    .choices(&["ja", "en"])
    .default("en")
}

/// `--help`.
pub fn help_flag() -> Flag {
    flag("--help", None, tr!("この画面を出す", "print this page"))
}

pub struct Cmd {
    pub name: &'static str,
    /// The usage line written by hand, when the arguments and the flags cannot say it: a flag the
    /// command needs, or one of two ways to call it (chobo's `explain <code> | --all`). None for
    /// the line drawn from them ([`Table::usage_line`]).
    pub usage: Option<&'static str>,
    pub args: &'static str,
    pub purpose: Text,
    pub params: Vec<(&'static str, Text)>,
    pub flags: Vec<Flag>,
    pub exits: Vec<(u8, Text)>,
    pub examples: Vec<&'static str>,
    /// The diagnostic codes it can print, for its `--help`.
    pub codes: Vec<&'static str>,
}

/// How a command line is read where the tools differed.
#[derive(Clone, Copy, Debug, Default)]
pub struct Reading {
    /// `-5` is an argument, not a flag (koyomi takes negative numbers).
    pub negative_numbers: bool,
    /// A value that starts with `--` is not taken: the flag is missing its value (sakai, chobo).
    pub no_dashes_in_values: bool,
    /// `--format=json` is one word, an unknown flag, not a flag and its value (chobo).
    pub no_inline_values: bool,
    /// Only what starts with `--` is a flag; `-x` and `-5` are arguments (chobo).
    pub single_dash_args: bool,
}

/// What stops a command line, before it is said in words ([`Misuse::text`]). A tool that words
/// it its own way reads the kind (chobo).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Misuse {
    /// A flag neither the command nor every command takes, as written.
    UnknownFlag(String),
    /// `--all=yes`: a flag that takes no value, given one.
    TakesNoValue { flag: &'static str, value: String },
    /// A flag at the end, or before a word it does not take as its value.
    MissingValue { flag: &'static str, placeholder: &'static str },
    /// A value outside the flag's closed set.
    NotAChoice { flag: &'static str, value: String, choices: &'static [&'static str] },
    /// A flag that does not repeat, given twice.
    Twice(&'static str),
}

impl Misuse {
    /// In ritsu-base's words, for `<tool> <cmd>`.
    pub fn text(&self, tool: &str, cmd: &str) -> Text {
        match self {
            Misuse::UnknownFlag(name) => tr!("`{name}` というフラグはありません。`{tool} {cmd} --help` を読んでください", "unknown flag `{name}`; run `{tool} {cmd} --help`"),
            Misuse::TakesNoValue { flag, value } => tr!("`{flag}` は値を取りません（`={value}` が付いています）", "`{flag}` takes no value (it was given `={value}`)"),
            Misuse::MissingValue { flag, placeholder } => {
                let s = format!("{flag} {placeholder}");
                tr!("`{s}` に値がありません", "`{s}` is missing its value")
            }
            Misuse::NotAChoice { flag, value, choices } => {
                let cs = choices.join(" | ");
                tr!("`{flag}` に `{value}` は使えません。使えるのは {cs} だけです", "`{flag} {value}` is not a value this flag takes; it takes only {cs}")
            }
            Misuse::Twice(flag) => tr!("`{flag}` が二度書かれています", "`{flag}` is given twice"),
        }
    }
}

/// A tool's commands, its flags for every command, and the lines of its `--help`.
pub struct Table {
    /// The command: `koyomi`.
    pub tool: &'static str,
    pub version: &'static str,
    /// What the tool is, under the version in `--help`.
    pub summary: Text,
    /// The flags every command takes, after its own.
    pub globals: Vec<Flag>,
    pub commands: Vec<Cmd>,
    /// The lines at the end of `--help`: what every command takes, the exit codes.
    pub footer: Vec<Text>,
    pub reading: Reading,
}

/// What one command line said.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
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

    /// Every value of a flag that repeats, in the order given.
    pub fn all(&self, n: &str) -> Vec<&str> {
        self.got.iter().filter(|(k, _)| *k == n).map(|(_, v)| v.as_str()).collect()
    }
}

fn is_negative_number(a: &str) -> bool {
    a.len() > 1 && a[1..].chars().all(|c| c.is_ascii_digit())
}

impl Table {
    pub fn command(&self, name: &str) -> Option<&Cmd> {
        self.commands.iter().find(|c| c.name == name)
    }

    /// `koyomi check <file.cal>... [--format json]`: the command's own line when it has one.
    pub fn usage_line(&self, c: &Cmd) -> String {
        if let Some(u) = c.usage {
            return u.to_string();
        }
        let mut o = format!("{} {} {}", self.tool, c.name, c.args).trim_end().to_string();
        for f in &c.flags {
            o.push_str(&format!(" [{}]", f.spelled()));
        }
        o
    }

    /// `<tool> <cmd> --help` and `<tool> help <cmd>`.
    pub fn help_cmd(&self, c: &Cmd, lang: Lang) -> String {
        let t = |x: Text| x.get(lang).to_string();
        let mut o = format!("{} {} — {}\n\n", self.tool, c.name, c.purpose.get(lang));
        o.push_str(&t(tr!("使い方:\n", "Usage:\n")));
        o.push_str(&format!("  {}\n", self.usage_line(c)));
        if !c.params.is_empty() {
            o.push_str(&t(tr!("\n引数:\n", "\nArguments:\n")));
            let w = c.params.iter().map(|(n, _)| width(n)).max().unwrap_or(0);
            for (n, h) in &c.params {
                o.push_str(&format!("  {}  {}\n", pad(n, w), h.get(lang)));
            }
        }
        o.push_str(&t(tr!("\nフラグ:\n", "\nFlags:\n")));
        let flags: Vec<&Flag> = c.flags.iter().chain(self.globals.iter()).collect();
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
            let tool = self.tool;
            o.push_str(&t(tr!("\n出しうる診断（`{tool} explain <CODE>` が引きます）:\n  ", "\nDiagnostics it can print (`{tool} explain <CODE>` looks one up):\n  ")));
            o.push_str(&c.codes.join(" "));
            o.push('\n');
        }
        o
    }

    /// `<tool> --help`.
    pub fn help_all(&self, lang: Lang) -> String {
        let tool = self.tool;
        let mut o = format!("{tool} {}\n\n{}\n\n", self.version, self.summary.get(lang));
        o.push_str(tr!("使い方:\n", "Usage:\n").get(lang));
        let w = self.commands.iter().map(|c| width(&format!("{} {}", c.name, c.args))).max().unwrap_or(0);
        for c in &self.commands {
            o.push_str(&format!("  {tool} {}  {}\n", pad(&format!("{} {}", c.name, c.args), w), c.purpose.get(lang)));
        }
        o.push_str(
            tr!(
                "\n一つのコマンドの詳しい説明は `{tool} <cmd> --help`（`{tool} help <cmd>` も同じ）。\n",
                "\nFor one command in detail: `{tool} <cmd> --help` (`{tool} help <cmd>` is the same page).\n"
            )
            .get(lang),
        );
        for f in &self.footer {
            o.push_str(f.get(lang));
            o.push('\n');
        }
        o
    }

    /// Read a command line against the command's flags and the flags every command takes.
    pub fn parse(&self, c: &Cmd, argv: &[String]) -> Result<Args, Text> {
        self.read(c, argv).map_err(|m| m.text(self.tool, c.name))
    }

    /// The same, saying what stopped it as a [`Misuse`].
    pub fn read(&self, c: &Cmd, argv: &[String]) -> Result<Args, Misuse> {
        let find = |name: &str| c.flags.iter().chain(self.globals.iter()).find(|f| f.name == name);
        let r = self.reading;
        let mut out = Args::default();
        let mut i = 0;
        while i < argv.len() {
            let a = &argv[i];
            if a == "-h" {
                out.got.push(("--help", String::new()));
                i += 1;
                continue;
            }
            let argument = if r.single_dash_args { !a.starts_with("--") } else { !a.starts_with('-') || a == "-" || (r.negative_numbers && is_negative_number(a)) };
            if argument {
                out.pos.push(a.clone());
                i += 1;
                continue;
            }
            let (name, inline) = match a.split_once('=').filter(|_| !r.no_inline_values) {
                Some((k, v)) => (k.to_string(), Some(v.to_string())),
                None => (a.clone(), None),
            };
            let Some(f) = find(&name) else {
                return Err(Misuse::UnknownFlag(name));
            };
            let v = match (f.value, inline) {
                (None, Some(v)) => return Err(Misuse::TakesNoValue { flag: f.name, value: v }),
                (None, None) => String::new(),
                (Some(_), Some(v)) => v,
                (Some(placeholder), None) => {
                    i += 1;
                    match argv.get(i) {
                        Some(v) if !(r.no_dashes_in_values && v.starts_with("--")) => v.clone(),
                        _ => return Err(Misuse::MissingValue { flag: f.name, placeholder }),
                    }
                }
            };
            if !f.choices.is_empty() && !f.choices.contains(&v.as_str()) {
                return Err(Misuse::NotAChoice { flag: f.name, value: v, choices: f.choices });
            }
            if out.has(f.name) && !f.repeats {
                return Err(Misuse::Twice(f.name));
            }
            out.got.push((f.name, v));
            i += 1;
        }
        Ok(out)
    }
}
