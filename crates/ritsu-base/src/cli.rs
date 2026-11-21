//! The commands and their flags, in one table (DESIGN 4.4, rulec's §12.1). `--help` is drawn
//! from it and the command line is read against it, so a flag cannot be documented and
//! ignored, or taken and left undocumented. An unknown flag, a value outside a closed set, a
//! flag without its value, and a flag given twice (unless the table says it repeats) all stop
//! the run with exit 2: an agent that is told nothing believes the flag worked. So do a value given
//! twice to a flag that takes each value once, and a value a flag took up to an earlier version,
//! which is named with the value it is now (DESIGN 9.6).
//!
//! koyomi, yuen and sakai wrote this table three times; it differed in two places, which are
//! now [`Reading`]: whether `-5` is an argument (koyomi), and whether a value may start with
//! `--` (sakai says no).

use crate::diag::Severity;
use crate::json::Json;
use crate::text::{Lang, Text, pad, width};
use crate::tr;
use std::cell::RefCell;
use std::io;

pub struct Flag {
    pub name: &'static str,
    /// The placeholder of the value; None for a flag that takes none.
    pub value: Option<&'static str>,
    /// The values taken, when the set is closed.
    pub choices: &'static [&'static str],
    pub default: Option<String>,
    /// Whether it may be given more than once.
    pub repeats: bool,
    /// Of a flag that repeats, whether each value is given once at most (`--target`, DESIGN 9.6):
    /// the same target twice does no harm, but is the sign of a line written in error.
    pub once_each: bool,
    /// The values it took up to an earlier version and takes no more, each with what is written
    /// now and the version that changed it (`("temporal", "temporal-typescript", "0.41.0")`). Given
    /// one, the run stops and says what to write instead (DESIGN 9.6): no old value is read as the
    /// new one.
    pub renamed: &'static [(&'static str, &'static str, &'static str)],
    /// Whether the command cannot run without it (`sakai gen --target`): its usage line writes it
    /// without brackets, as the sentence that stops a line without it says (`needs`).
    pub needed: bool,
    pub help: Text,
}

pub fn flag(name: &'static str, value: Option<&'static str>, help: Text) -> Flag {
    Flag { name, value, choices: &[], default: None, repeats: false, once_each: false, renamed: &[], needed: false, help }
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

    /// It may be given more than once, each value once at most.
    pub fn once_each(mut self) -> Flag {
        self.repeats = true;
        self.once_each = true;
        self
    }

    /// The command cannot run without it.
    pub fn needed(mut self) -> Flag {
        self.needed = true;
        self
    }

    /// The values it took up to an earlier version: `(old, new, version)`.
    pub fn renamed(mut self, r: &'static [(&'static str, &'static str, &'static str)]) -> Flag {
        self.renamed = r;
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

/// `--lang ja|en` of a command that writes what others read and keep (the code it generates, a
/// page, the JSON of `api`, the vectors): its help says that the flag alone gives the language of
/// what the command writes ([`crate::text::Lang::made`]: en without it, whatever the environment
/// says), and that the text the command prints reads the variables, as with [`lang_flag`]. A
/// command takes it in place of the `--lang` every command of its tool takes.
pub fn made_lang_flag(var: &str) -> Flag {
    let what = if var == "RITSU_LANG" {
        tr!(
            "書く物と出す文の言語。無ければ、書く物は en（環境変数では変わらない）、出す文は環境変数 RITSU_LANG、それも無ければ en",
            "the language of what it writes and of the text it prints. Without it, what it writes is in en, whatever the environment says, and the text follows the RITSU_LANG environment variable, else en"
        )
    } else {
        tr!(
            "書く物と出す文の言語。無ければ、書く物は en（環境変数では変わらない）、出す文は環境変数 {var}、次に RITSU_LANG、どちらも無ければ en",
            "the language of what it writes and of the text it prints. Without it, what it writes is in en, whatever the environment says, and the text follows the {var} environment variable, then RITSU_LANG, else en"
        )
    };
    flag("--lang", Some("ja|en"), what).choices(&["ja", "en"]).default("en")
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
    /// A flag that takes each value once, given one value twice.
    SameTwice { flag: &'static str, value: String },
    /// A value the flag took up to `since`, which is `new` from that version on.
    Renamed { flag: &'static str, old: String, new: &'static str, since: &'static str },
}

impl Misuse {
    /// In ritsu-base's words, for `<tool> <cmd>`.
    pub fn text(&self, tool: &str, cmd: &str) -> Text {
        self.text_among(tool, cmd, &[])
    }

    /// [`Misuse::text`], given the flags the command takes (`names`): an unknown flag says the one it
    /// was most likely meant to be ([`nearest_flag`]).
    pub fn text_among(&self, tool: &str, cmd: &str, names: &[&str]) -> Text {
        match self {
            Misuse::UnknownFlag(name) => {
                let hint = match nearest_flag(name, names) {
                    Some(n) => tr!("（`{n}` のことですか）", " (did you mean `{n}`?)"),
                    None if !name.starts_with("--") => tr!("（フラグは長い形だけです）", " (every flag is spelled out in full)"),
                    None => Text::same(""),
                };
                let (hj, he) = (hint.ja, hint.en);
                Text::new(
                    format!("`{name}` というフラグはありません{hj}。`{tool} {cmd} --help` を読んでください"),
                    format!("unknown flag `{name}`{he}; run `{tool} {cmd} --help`"),
                )
            }
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
            Misuse::SameTwice { flag, value } => tr!("`{flag} {value}` が二度書かれています", "`{flag} {value}` is given twice"),
            Misuse::Renamed { old, new, since, .. } => renamed(old, new, since),
        }
    }
}

/// The flag an unknown one (`typed`) was most likely meant to be, among `names`: `-o` is taken as the
/// start of a long name (`--out`), `--outt` as a slip of at most two letters. Only an answer that is
/// the one best is given; two as near would be a guess.
pub fn nearest_flag<'a>(typed: &str, names: &[&'a str]) -> Option<&'a str> {
    if !typed.starts_with("--") {
        let head = typed.trim_start_matches('-');
        let hits: Vec<&str> = names.iter().copied().filter(|n| !head.is_empty() && n.trim_start_matches('-').starts_with(head)).collect();
        return (hits.len() == 1).then(|| hits[0]);
    }
    fn distance(a: &str, b: &str) -> usize {
        let b: Vec<char> = b.chars().collect();
        let mut row: Vec<usize> = (0..=b.len()).collect();
        for (i, ca) in a.chars().enumerate() {
            let mut prev = row[0];
            row[0] = i + 1;
            for (j, cb) in b.iter().enumerate() {
                let here = row[j + 1];
                row[j + 1] = (prev + usize::from(ca != *cb)).min(row[j] + 1).min(here + 1);
                prev = here;
            }
        }
        row[b.len()]
    }
    let mut best: Vec<(usize, &str)> = names.iter().map(|n| (distance(typed, n), *n)).filter(|(d, _)| *d <= 2).collect();
    best.sort();
    match best.as_slice() {
        [(d, n), rest @ ..] if rest.first().is_none_or(|(e, _)| e > d) => Some(n),
        _ => None,
    }
}

/// What stops a word that was one thing up to `since` and is another from that version on: a
/// value of a flag (`temporal`), or a command (`chobo build`, which is `chobo gen`). The old word
/// is not read as the new one (DESIGN 9.6).
pub fn renamed(old: &str, new: &str, since: &str) -> Text {
    tr!("`{old}` は {since} から `{new}` です", "`{old}` is `{new}` from {since}")
}

/// A word of a command line as a shell reads it back: as it is when it holds nothing a shell would
/// take apart, else in single quotes. What stops a run given no target writes the line that writes
/// what the earlier version wrote with it (DESIGN 9.6), the words given among it.
pub fn shell_word(w: &str) -> String {
    let plain = !w.is_empty() && w.chars().all(|c| c.is_alphanumeric() || "_./:=@%+,-".contains(c));
    if plain { w.to_string() } else { format!("'{}'", w.replace('\'', "'\\''")) }
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

    /// The values of the flags named, each a path, held as the run holds a path a flag takes
    /// ([`crate::paths::fold_given`], DESIGN 4.15): `--out ./generated` is `generated`.
    pub fn fold_paths(&mut self, flags: &[&str]) {
        for (k, v) in &mut self.got {
            if flags.contains(k) {
                *v = crate::paths::fold_given(v);
            }
        }
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
        // a flag the arguments already name is one way to call the command (`<CODE>... | --all`),
        // not said again
        for f in c.flags.iter().filter(|f| !c.args.split_whitespace().any(|w| w == f.name)) {
            o.push_str(&if f.needed { format!(" {}", f.spelled()) } else { format!(" [{}]", f.spelled()) });
        }
        o
    }

    /// `<tool> <cmd> --help` and `<tool> help <cmd>`. The commands it names are named the way the
    /// person ran this one ([`command_name`]): `ritsu koyomi check …` inside [`through`].
    pub fn help_cmd(&self, c: &Cmd, lang: Lang) -> String {
        let t = |x: Text| x.get(lang).to_string();
        let run = command_name(self.tool);
        let mut o = format!("{run} {} — {}\n\n", c.name, c.purpose.get(lang));
        o.push_str(&t(tr!("使い方:\n", "Usage:\n")));
        o.push_str(&format!("  {}\n", as_run(&self.usage_line(c))));
        if !c.params.is_empty() {
            o.push_str(&t(tr!("\n引数:\n", "\nArguments:\n")));
            let w = c.params.iter().map(|(n, _)| width(n)).max().unwrap_or(0);
            for (n, h) in &c.params {
                o.push_str(&format!("  {}  {}\n", pad(n, w), h.get(lang)));
            }
        }
        o.push_str(&t(tr!("\nフラグ:\n", "\nFlags:\n")));
        // a flag every command takes, which the command takes in a form of its own (the `--lang`
        // of a command that writes what others keep, `made_lang_flag`), is said once, as its own
        let flags: Vec<&Flag> = c.flags.iter().chain(self.globals.iter().filter(|g| !c.flags.iter().any(|f| f.name == g.name))).collect();
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
            o.push_str(&format!("  $ {}\n", as_run_of(self.tool, e)));
        }
        if !c.codes.is_empty() {
            o.push_str(&t(tr!("\n出しうる診断（`{run} explain <CODE>` が引きます）:\n  ", "\nDiagnostics it can print (`{run} explain <CODE>` looks one up):\n  ")));
            o.push_str(&c.codes.join(" "));
            o.push('\n');
        }
        o
    }

    /// `<tool> --help`, the commands named as [`Table::help_cmd`] names them.
    pub fn help_all(&self, lang: Lang) -> String {
        let tool = self.tool;
        let run = command_name(tool);
        let mut o = format!("{tool} {}\n\n{}\n\n", self.version, self.summary.get(lang));
        o.push_str(tr!("使い方:\n", "Usage:\n").get(lang));
        let w = self.commands.iter().map(|c| width(&format!("{} {}", c.name, c.args))).max().unwrap_or(0);
        for c in &self.commands {
            o.push_str(&format!("  {run} {}  {}\n", pad(&format!("{} {}", c.name, c.args), w), c.purpose.get(lang)));
        }
        o.push_str(
            tr!(
                "\n一つのコマンドの詳しい説明は `{run} <cmd> --help`（`{run} help <cmd>` も同じ）。\n",
                "\nFor one command in detail: `{run} <cmd> --help` (`{run} help <cmd>` is the same page).\n"
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
        let names: Vec<&str> = c.flags.iter().chain(self.globals.iter()).map(|f| f.name).collect();
        self.read(c, argv).map_err(|m| m.text_among(&command_name(self.tool), c.name, &names))
    }

    /// What stops a command given arguments it does not take, or none ([`takes`]): what it takes,
    /// and its usage line.
    pub fn takes(&self, c: &Cmd) -> Text {
        takes(&format!("{} {}", command_name(self.tool), c.name), c.args, &self.usage_line(c))
    }

    /// What stops a command without a flag it needs ([`needs`]), and its usage line.
    pub fn needs(&self, c: &Cmd, flag: &str) -> Text {
        needs(&format!("{} {}", command_name(self.tool), c.name), flag, &self.usage_line(c))
    }

    /// [`Table::takes`] of the command named `name`.
    pub fn takes_of(&self, name: &str) -> Text {
        self.takes(self.command(name).expect("the command is in the table"))
    }

    /// [`Table::needs`] of the command named `name`.
    pub fn needs_of(&self, name: &str, flag: &str) -> Text {
        self.needs(self.command(name).expect("the command is in the table"), flag)
    }

    /// What stops the command named `name` given none of `flags`, or more than one ([`one_of`]).
    pub fn one_of(&self, name: &str, flags: &[&str]) -> Text {
        let c = self.command(name).expect("the command is in the table");
        one_of(&format!("{} {}", command_name(self.tool), c.name), flags, &self.usage_line(c))
    }

    /// What stops the command named `name` given none of `flags`, which it takes together ([`needs_any`]).
    pub fn needs_any(&self, name: &str, flags: &[&str]) -> Text {
        let c = self.command(name).expect("the command is in the table");
        needs_any(&format!("{} {}", command_name(self.tool), c.name), flags, &self.usage_line(c))
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
                if let Some((_, new, since)) = f.renamed.iter().find(|(old, _, _)| *old == v) {
                    return Err(Misuse::Renamed { flag: f.name, old: v, new, since });
                }
                return Err(Misuse::NotAChoice { flag: f.name, value: v, choices: f.choices });
            }
            if out.has(f.name) && !f.repeats {
                return Err(Misuse::Twice(f.name));
            }
            if f.once_each && out.all(f.name).contains(&v.as_str()) {
                return Err(Misuse::SameTwice { flag: f.name, value: v });
            }
            out.got.push((f.name, v));
            i += 1;
        }
        Ok(out)
    }
}

// ── What a command says when it cannot start its work (DESIGN 6.2, item 10) ──────────────────────
//
// Every entry of the toolchain — `ritsu` and the ten languages, run by their own names or as
// `ritsu <language>` — says these things in these words, after the word of an error in the language
// it prints in (`error:`, `エラー:`), and exits 2 (DESIGN 8.4): a file it was given is not there, is
// not UTF-8 or cannot be read; what it writes cannot be made or written; the command line is not one
// it takes. No entry writes these sentences itself, so that what reads one language's (a person, a
// script, an agent) reads every language's alike.

thread_local! {
    /// The command a language's command runs through (`ritsu`, in `ritsu koyomi …`).
    static THROUGH: RefCell<Option<String>> = const { RefCell::new(None) };
    /// What the command stopped with, in the language it printed it in, for its JSON ([`answering`]).
    static REFUSED: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Run `f` as a language's command run through `outer` (`ritsu koyomi check …`): the commands the
/// words of the run name (`run `koyomi check --help``) are named the way the person ran this one
/// (`ritsu koyomi check --help`), which runs where only `ritsu` is installed.
pub fn through<R>(outer: &str, f: impl FnOnce() -> R) -> R {
    struct Back(Option<String>);
    impl Drop for Back {
        fn drop(&mut self) {
            let before = self.0.take();
            THROUGH.with(|t| *t.borrow_mut() = before);
        }
    }
    let _back = Back(THROUGH.with(|t| t.borrow_mut().replace(outer.to_string())));
    f()
}

/// `tool` as the person runs it: `koyomi`, or `ritsu koyomi` inside [`through`].
pub fn command_name(tool: &str) -> String {
    THROUGH.with(|t| match t.borrow().as_deref() {
        Some(outer) => format!("{outer} {tool}"),
        None => tool.to_string(),
    })
}

/// A command line written out for the person to run (a usage line, `koyomi check <file.cal>...`),
/// named as [`command_name`] names its tool.
pub fn as_run(line: &str) -> String {
    THROUGH.with(|t| match t.borrow().as_deref() {
        Some(outer) => format!("{outer} {line}"),
        None => line.to_string(),
    })
}

/// [`as_run`] of a line that may not start with `tool` (an example of a help page that sets a
/// variable first, `KOYOMI_LANG=ja koyomi …`): such a line is as it is.
pub fn as_run_of(tool: &str, line: &str) -> String {
    match line.strip_prefix(tool) {
        Some(rest) if rest.is_empty() || rest.starts_with(' ') => format!("{}{rest}", command_name(tool)),
        _ => line.to_string(),
    }
}

/// The line a command stops with: `error: <said>` (`エラー: <said>`), the head the word of an error
/// in the language the run prints in. A sentence of more than one line has the head on its first.
pub fn refusal(said: &str, lang: Lang) -> String {
    format!("{}: {said}", Severity::Error.word(lang))
}

/// [`refusal`] of `msg` written on `err`, kept for the JSON ([`answering`]), and 2, the exit code of
/// a command that did not do its work (DESIGN 8.4).
pub fn refuse(err: &mut dyn io::Write, msg: &Text, lang: Lang) -> u8 {
    refuse_said(err, msg.get(lang), lang)
}

/// [`refuse`] of a sentence already in the language of the run (rulec's, which chooses the language
/// as it makes the sentence). A command that goes on to the next file after one it cannot read keeps
/// the first sentence for its JSON.
pub fn refuse_said(err: &mut dyn io::Write, said: &str, lang: Lang) -> u8 {
    let _ = writeln!(err, "{}", refusal(said, lang));
    REFUSED.with(|r| {
        let mut r = r.borrow_mut();
        if r.is_none() {
            *r = Some(said.to_string());
        }
    });
    2
}

/// Whether a command line asks for JSON: `--format json` or `--format=json`, wherever it stands
/// (a line the table refuses asks for it too).
pub fn asks_json(args: &[String]) -> bool {
    args.windows(2).any(|w| w[0] == "--format" && w[1] == "json") || args.iter().any(|a| a == "--format=json")
}

/// The value of `error` in the JSON of a command that stopped before its work (DESIGN 6.2, item
/// 10): the exit code and the sentence it printed on the standard error, without its head.
pub fn error_value(said: &str) -> Json {
    Json::obj([("code", Json::int(2)), ("message", Json::str(said))])
}

/// What the command stopped with, taken: a command that writes one JSON document puts it in the
/// document under `error` ([`error_value`]), and [`answering`] then writes nothing more.
pub fn take_refusal() -> Option<String> {
    REFUSED.with(|r| r.borrow_mut().take())
}

/// Run a command (`f`, given `out`): when the line asks for JSON and the command stopped before its
/// work, `{"error": …}` ([`error_value`]) is the last value written on `out`, after what the command
/// wrote before it stopped (the files it had checked when the next was not there).
pub fn answering<C>(args: &[String], out: &mut dyn io::Write, f: impl FnOnce(&mut dyn io::Write) -> C) -> C {
    let before = REFUSED.with(|r| r.borrow_mut().take());
    let code = f(out);
    let said = REFUSED.with(|r| std::mem::replace(&mut *r.borrow_mut(), before));
    if let Some(s) = said
        && asks_json(args)
    {
        let _ = writeln!(out, "{}", Json::obj([("error", error_value(&s))]).compact());
    }
    code
}

/// The system's words for `e`, as the native binary says them: built for WASI, with the system's
/// number rather than WASI's ([`crate::fs::renumbered`]).
pub fn reason(e: &io::Error) -> String {
    let words = e.to_string();
    #[cfg(target_os = "wasi")]
    if let Some(native) = e.raw_os_error().and_then(|n| crate::fs::renumbered(&words, n)) {
        return native;
    }
    words
}

/// Why a file given to a command (or to a flag of it) could not be read: it is not there, it is not
/// UTF-8, or the system's reason. `shown` is the path as the run writes it ([`crate::paths::show_given`],
/// [`crate::paths::fold_given`]).
pub fn cannot_read(shown: &str, e: &io::Error) -> Text {
    match e.kind() {
        io::ErrorKind::NotFound => not_there(shown),
        io::ErrorKind::InvalidData => not_utf8(shown),
        _ => {
            let why = reason(e);
            tr!("`{shown}` を読めません: {why}", "cannot read `{shown}`: {why}")
        }
    }
}

/// Why the standard input, given as `-`, could not be read.
pub fn cannot_read_stdin(e: &io::Error) -> Text {
    match e.kind() {
        io::ErrorKind::InvalidData => tr!("標準入力は UTF-8 ではありません", "the standard input is not UTF-8"),
        _ => {
            let why = reason(e);
            tr!("標準入力を読めません: {why}", "cannot read the standard input: {why}")
        }
    }
}

/// A program the command starts (git, an adapter given on the line) that does not start: the
/// system's reason, or, built for WASI, that no program starts there ([`crate::wasi::cannot_start`]).
pub fn cannot_start(program: &str, e: &io::Error) -> Text {
    if crate::wasi::unsupported(e) {
        return crate::wasi::cannot_start(program);
    }
    let why = reason(e);
    tr!("`{program}` を起動できません: {why}", "cannot start `{program}`: {why}")
}

/// A file given (a scenario, an input) that is read but is not JSON: `why` is the reader's words.
pub fn not_json(shown: &str, why: &str) -> Text {
    tr!("`{shown}` は JSON として読めません: {why}", "`{shown}` is not JSON: {why}")
}

/// A file given that is not there.
pub fn not_there(shown: &str) -> Text {
    tr!("`{shown}` がありません", "`{shown}` is not there")
}

/// A file given that is not UTF-8.
pub fn not_utf8(shown: &str) -> Text {
    tr!("`{shown}` は UTF-8 ではありません", "`{shown}` is not UTF-8")
}

/// A directory the command cannot make: `--out`, or one under it.
pub fn cannot_create(shown: &str, e: &io::Error) -> Text {
    match read_only(e) {
        true => tr!("`{shown}` を作れません: 依存のファイルは読むだけです", "cannot create `{shown}`: it is in a dependency, whose files are read only"),
        false => {
            let why = reason(e);
            tr!("`{shown}` を作れません: {why}", "cannot create `{shown}`: {why}")
        }
    }
}

/// A file the command cannot write.
pub fn cannot_write(shown: &str, e: &io::Error) -> Text {
    match read_only(e) {
        true => tr!("`{shown}` に書けません: 依存のファイルは読むだけです", "cannot write `{shown}`: it is in a dependency, whose files are read only"),
        false => {
            let why = reason(e);
            tr!("`{shown}` に書けません: {why}", "cannot write `{shown}`: {why}")
        }
    }
}

/// Whether `e` is what writing under a module's dependency answers ([`crate::fs::ReadOnly`]).
fn read_only(e: &io::Error) -> bool {
    e.get_ref().is_some_and(|inner| inner.is::<crate::fs::ReadOnly>())
}

/// What stops a command given a directory that holds none of the files it reads: `exts` their
/// extensions (`.cal`), more than one for ritsu's own commands, which read every language's.
pub fn no_files_under(shown_dir: &str, exts: &[&str]) -> Text {
    let en = match exts {
        [] => String::new(),
        [one] => one.to_string(),
        [init @ .., last] => format!("{} or {last}", init.join(", ")),
    };
    let ja = exts.join("、");
    Text::new(format!("`{shown_dir}` の下に {ja} のファイルがありません"), format!("there is no {en} file under `{shown_dir}`"))
}

/// What stops a command line whose command `tool` (`koyomi`, or a command with commands of its own,
/// `ritsu mod`) does not have.
pub fn no_command(tool: &str, name: &str) -> Text {
    let t = command_name(tool);
    tr!("`{name}` というコマンドはありません。`{t} --help` を読んでください", "there is no command `{name}`; run `{t} --help`")
}

/// What stops `explain` given a code `tool` does not have.
pub fn no_code(tool: &str, code: &str) -> Text {
    let t = command_name(tool);
    tr!("`{code}` というコードはありません。`{t} explain --all` を読んでください", "there is no code `{code}`; run `{t} explain --all`")
}

/// What stops `explain` given codes `tool` does not have: every one of them in one sentence, in the
/// order given, each once ([`no_code`] when there is one), so that one run names them all.
pub fn no_codes(tool: &str, codes: &[String]) -> Text {
    let mut once: Vec<&str> = Vec::new();
    for c in codes {
        if !once.contains(&c.as_str()) {
            once.push(c);
        }
    }
    if let [one] = once.as_slice() {
        return no_code(tool, one);
    }
    let t = command_name(tool);
    let quoted: Vec<String> = once.iter().map(|c| format!("`{c}`")).collect();
    let (ja, en) = (quoted.join("、"), quoted.join(", "));
    tr!("{ja} というコードはありません。`{t} explain --all` を読んでください", "there are no codes {en}; run `{t} explain --all`")
}

/// What stops a command given `flag` with a value that is not a whole number of 1 or more (`--budget
/// 0`, `--jobs x`).
pub fn not_positive(flag: &str, value: &str) -> Text {
    tr!("`{flag} {value}` は正の整数ではありません", "`{flag} {value}` is not a positive integer")
}

/// What stops a command given `flag` with a value that is not a whole number of 0 or more
/// (`--at-most -1`): [`not_positive`]'s, for a flag that takes 0.
pub fn not_whole(flag: &str, value: &str) -> Text {
    tr!("`{flag} {value}` は 0 以上の整数ではありません", "`{flag} {value}` is not a whole number of 0 or more")
}

/// What stops `command` (`koyomi api`, as [`command_name`] names it) given arguments other than it
/// takes, or none: what it takes (`args`, the arguments of its line in the table, `<file.cal>`), and
/// on the next line its usage line.
pub fn takes(command: &str, args: &str, usage: &str) -> Text {
    let args = args.trim();
    let said = if args.is_empty() {
        tr!("`{command}` は引数を取りません", "`{command}` takes no arguments")
    } else {
        tr!("`{command}` に渡すのは {args} です", "`{command}` takes {args}")
    };
    said.then(&usage_line(usage))
}

/// What stops `command` without the flag it needs (`--scenario <file.json>`, as the table spells
/// it), and on the next line its usage line.
pub fn needs(command: &str, flag: &str, usage: &str) -> Text {
    tr!("`{command}` には `{flag}` が要ります", "`{command}` needs `{flag}`").then(&usage_line(usage))
}

/// What stops `command` given none of `flags`, or more than one: it takes one of them.
pub fn one_of(command: &str, flags: &[&str], usage: &str) -> Text {
    let quoted: Vec<Text> = flags.iter().map(|f| Text::same(format!("`{f}`"))).collect();
    let (ja, en) = (Text::join(&quoted, "、", ", ").ja, Text::list(&quoted).en);
    Text::new(format!("`{command}` には {ja} のどれか一つを渡してください"), format!("`{command}` takes one of {en}")).then(&usage_line(usage))
}

/// What stops `command` given none of `flags`, any number of which it takes together: it needs one
/// of them at least.
pub fn needs_any(command: &str, flags: &[&str], usage: &str) -> Text {
    let quoted: Vec<String> = flags.iter().map(|f| format!("`{f}`")).collect();
    let en = match quoted.as_slice() {
        [init @ .., last] if !init.is_empty() => format!("{} or {last}", init.join(", ")),
        _ => quoted.concat(),
    };
    let ja = quoted.join("、");
    Text::new(format!("`{command}` には {ja} のどれかが要ります"), format!("`{command}` needs {en}")).then(&usage_line(usage))
}

/// `usage: <line>` on a line of its own.
fn usage_line(usage: &str) -> Text {
    let u = as_run(usage);
    tr!("\n使い方: {u}", "\nusage: {u}")
}

/// What a panic says, alike in every binary of the toolchain: a bug of `tool`, to be reported. Its
/// `main` catches the panic and exits 2 (DESIGN 8.4), not Rust's 101. Built for WASI, a reader that
/// went away (`| head`) is an error of the write, which std's printing panics on: the binary goes
/// quietly then, with the code a shell gives the death the native binary dies of that signal (DESIGN 8.8).
pub fn on_panic(tool: &'static str) {
    std::panic::set_hook(Box::new(move |info| {
        #[cfg(target_os = "wasi")]
        if info.payload_as_str().is_some_and(|s| s.starts_with("failed printing to std") && s.contains("Broken pipe")) {
            std::process::exit(128 + 13);
        }
        eprintln!("{tool}: a bug in {tool}, please report it: {info}");
    }));
}
