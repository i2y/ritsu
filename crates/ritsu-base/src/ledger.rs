//! The ledger of a language's diagnostic codes (DESIGN 4.3): for each code, a one-line title,
//! when it is printed, how to get rid of it, and the smallest input that prints it. `explain`
//! reads it, `docs/codes.md` is its Markdown, and a test runs every reproduction and requires
//! its code to come out ([`check_every`]), so a reproduction cannot go stale while the prose
//! around it still reads well.
//!
//! The entries are the language's own; how they are written out is here. A reproduction is a
//! file with what has to be beside it (koyomi's and yuen's), or files laid in one directory and
//! a command run there (sakai's). An entry can hold two reproductions, one in English names and
//! one in Japanese ones (an entry's [`Entry::repro_ja`]): `explain` shows the one of the language
//! it is asked in, and [`check_every`] runs both.

use crate::diag::Severity;
use crate::json::Json;
use crate::text::{Lang, Text};
use std::path::Path;

/// What prints the code.
#[derive(Clone, Debug)]
pub enum Repro {
    /// The smallest file that prints it (written to [`Ledger::example_file`]), and what has to
    /// be beside it, as (path, contents).
    File { body: &'static str, beside: &'static [(&'static str, &'static [u8])] },
    /// Files laid in one directory, and the command run there, after the tool's name.
    Dir { files: Vec<(&'static str, &'static str)>, command: Vec<&'static str> },
    /// None yet: the code is not printed yet, or what prints it is read in a later stage.
    Later,
    /// None: the code is retired. Nothing prints it any more, and its number is given to
    /// nothing else (DESIGN 7.10); why, and since when.
    Retired(Text),
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub code: &'static str,
    pub severity: Severity,
    /// One line, like the diagnostic's own first line.
    pub title: Text,
    /// When it is printed.
    pub when: Text,
    /// How to get rid of it, down to what to write.
    pub fix: Text,
    /// The reproduction shown in English, and shown in Japanese too when [`Entry::repro_ja`] is
    /// none.
    pub repro: Repro,
    /// The reproduction shown in Japanese, when its names are Japanese where `repro`'s are English.
    pub repro_ja: Option<Repro>,
    pub related: &'static [&'static str],
}

impl Entry {
    /// An entry whose severity is read from its code's first letter.
    pub fn new(code: &'static str, title: Text, when: Text, fix: Text, repro: Repro, related: &'static [&'static str]) -> Entry {
        Entry { code, severity: Severity::of(code), title, when, fix, repro, repro_ja: None, related }
    }

    /// The same entry, its reproduction in Japanese being `repro` and the one it had until now,
    /// shown in English only: `english` is the reproduction with English names. The output of
    /// `explain` in Japanese is what it was.
    pub fn english(mut self, english: Repro) -> Entry {
        self.repro_ja = Some(std::mem::replace(&mut self.repro, english));
        self
    }

    /// The reproduction shown in a language.
    pub fn repro_in(&self, lang: Lang) -> &Repro {
        match (lang, &self.repro_ja) {
            (Lang::Ja, Some(ja)) => ja,
            _ => &self.repro,
        }
    }

    /// The entry with the reproduction shown in `lang` as its only one: what a test runs.
    pub fn shown_in(&self, lang: Lang) -> Entry {
        let mut e = self.clone();
        e.repro = self.repro_in(lang).clone();
        e.repro_ja = None;
        e
    }

    /// The same entry with what has to be beside its file (koyomi's and yuen's `with`). An entry
    /// whose reproduction is not a file is left as it is.
    pub fn beside(mut self, files: &'static [(&'static str, &'static [u8])]) -> Entry {
        if let Repro::File { beside, .. } = &mut self.repro {
            *beside = files;
        }
        self
    }

    /// The same entry with no reproduction yet ([`Repro::Later`]).
    pub fn later(mut self) -> Entry {
        self.repro = Repro::Later;
        self
    }

    /// The same entry, retired ([`Repro::Retired`]): why, and since when.
    pub fn retired(mut self, why: Text) -> Entry {
        self.repro = Repro::Retired(why);
        self
    }

    /// Whether the code is retired.
    pub fn is_retired(&self) -> bool {
        matches!(self.repro, Repro::Retired(_))
    }
}

/// What a retired entry says where its reproduction would be: why it was retired, and when.
fn retired_text(why: &Text, lang: Lang) -> String {
    why.get(lang).to_string()
}

/// A language's ledger and how it is written out.
#[derive(Clone, Debug)]
pub struct Ledger {
    /// The tool's command: `koyomi`.
    pub tool: &'static str,
    /// The file a [`Repro::File`] is written to: `example.cal`.
    pub example_file: &'static str,
    /// The language of a Markdown fence around a [`Repro::File`]: `cal`.
    pub fence: &'static str,
    /// The heading over the reproduction: 再現 / Example (sakai's English is Reproduction).
    pub repro_heading: Text,
    /// What a [`Repro::Later`] says, in the text and in Markdown.
    pub later_text: Text,
    pub later_markdown: Text,
    pub entries: Vec<Entry>,
}

fn headings(lang: Lang) -> (&'static str, &'static str, &'static str) {
    match lang {
        Lang::En => ("When", "Fix", "See also"),
        Lang::Ja => ("いつ出るか", "直し方", "関連"),
    }
}

/// The fence a file of a [`Repro::Dir`] is shown in: its extension, for the languages of the
/// suite and `.proto`.
fn fence_of(name: &str) -> &'static str {
    const KNOWN: &[&str] = &["rule", "flow", "cal", "book", "geas", "req", "ctx", "proto"];
    let ext = name.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    KNOWN.iter().find(|k| **k == ext).copied().unwrap_or("")
}

impl Ledger {
    /// The entry of a code, written in either case.
    pub fn find(&self, code: &str) -> Option<&Entry> {
        let code = code.to_ascii_uppercase();
        self.entries.iter().find(|e| e.code == code)
    }

    /// The sentence that says how to run a reproduction. A command that starts with `ritsu`
    /// (`ritsu sakai check .`, with every language joined) is given in full; any other is the
    /// words after the tool's command.
    fn run_sentence(&self, command: &[&str], lang: Lang) -> String {
        let cmd = if command.first() == Some(&"ritsu") { command.join(" ") } else { format!("{} {}", self.tool, command.join(" ")) };
        match lang {
            Lang::Ja => format!("下のファイルを一つのディレクトリに置き、そこで `{cmd}` を走らせます"),
            Lang::En => format!("put the files below in one directory, and run `{cmd}` there"),
        }
    }

    /// `<tool> explain <CODE>`, for a terminal.
    pub fn render_text(&self, e: &Entry, lang: Lang) -> String {
        let (when, fix, also) = headings(lang);
        let repro = self.repro_heading.get(lang);
        let mut o = format!("{} ({}) — {}\n\n", e.code, e.severity.word(lang), e.title.get(lang));
        o.push_str(&format!("{when}: {}\n\n{fix}: {}\n\n{repro}:\n", e.when.get(lang), e.fix.get(lang)));
        match e.repro_in(lang) {
            Repro::Later => o.push_str(&format!("    {}\n", self.later_text.get(lang))),
            Repro::Retired(why) => o.push_str(&format!("    {}\n", retired_text(why, lang))),
            Repro::File { body, beside } => {
                for l in body.lines() {
                    o.push_str(&format!("    {l}\n"));
                }
                for (name, bytes) in beside.iter() {
                    let label = if lang == Lang::Ja { "隣に置くファイル" } else { "beside it" };
                    o.push_str(&format!("\n  {label}: {name}\n"));
                    if let Ok(t) = std::str::from_utf8(bytes)
                        && bytes.len() < 400
                    {
                        for l in t.lines() {
                            o.push_str(&format!("    {l}\n"));
                        }
                    }
                }
            }
            Repro::Dir { files, command } => {
                o.push_str(&format!("  ({})\n", self.run_sentence(command, lang)));
                for (name, body) in files {
                    o.push_str(&format!("\n  {name}:\n"));
                    for l in body.lines() {
                        o.push_str(format!("    {l}").trim_end());
                        o.push('\n');
                    }
                }
            }
        }
        if !e.related.is_empty() {
            o.push_str(&format!("\n{also}: {}\n", e.related.join(" ")));
        }
        o
    }

    /// One code in Markdown, under an anchor of its own (`#e302`).
    pub fn render_markdown_one(&self, e: &Entry, lang: Lang) -> String {
        let (when, fix, also) = headings(lang);
        let repro = self.repro_heading.get(lang);
        let mut o = format!("<a id=\"{}\"></a>\n\n## {} — {}\n\n", e.code.to_lowercase(), e.code, e.title.get(lang));
        o.push_str(&format!("**{when}**: {}\n\n**{fix}**: {}\n\n**{repro}**:", e.when.get(lang), e.fix.get(lang)));
        match e.repro_in(lang) {
            Repro::Later => o.push_str(&format!(" {}\n", self.later_markdown.get(lang))),
            Repro::Retired(why) => o.push_str(&format!(" {}\n", retired_text(why, lang))),
            Repro::File { body, beside } => {
                o.push_str(&format!("\n\n```{}\n{body}```\n", self.fence));
                for (name, bytes) in beside.iter() {
                    if let Ok(t) = std::str::from_utf8(bytes)
                        && bytes.len() < 400
                    {
                        o.push_str(&format!("\n`{name}`:\n\n```\n{t}```\n"));
                    }
                }
            }
            Repro::Dir { files, command } => {
                let end = if lang == Lang::Ja { "。" } else { "." };
                o.push_str(&format!(" {}{end}\n", self.run_sentence(command, lang)));
                for (name, body) in files {
                    o.push_str(&format!("\n`{name}`:\n\n```{}\n{body}```\n", fence_of(name)));
                }
            }
        }
        if !e.related.is_empty() {
            let links: Vec<String> = e.related.iter().map(|c| format!("[{c}](#{})", c.to_lowercase())).collect();
            o.push_str(&format!("\n{also}: {}\n", links.join(", ")));
        }
        o
    }

    /// `<tool> explain --all --format markdown`: every code, what `docs/codes.md` is.
    pub fn render_markdown(&self, lang: Lang) -> String {
        let tool = self.tool;
        let mut o = match lang {
            Lang::En => format!("# Diagnostic codes\n\nWritten by `{tool} explain --all --format markdown`; do not edit.\n"),
            Lang::Ja => format!("# 診断のコード\n\n`{tool} explain --all --format markdown --lang ja` の出力です。手で直しません。\n"),
        };
        for e in &self.entries {
            o.push('\n');
            o.push_str(&self.render_markdown_one(e, lang));
        }
        o
    }

    /// One code for a program: `code`, `severity`, `title`, `when`, `fix`, `repro` and
    /// `related`, in that order.
    pub fn to_json(&self, e: &Entry, lang: Lang) -> Json {
        let repro = match e.repro_in(lang) {
            Repro::Later => Json::Null,
            Repro::Retired(why) => Json::obj([("retired", Json::str(why.get(lang)))]),
            Repro::File { body, beside } => {
                let mut files = vec![Json::obj([("path", Json::str(self.example_file)), ("text", Json::str(*body))])];
                for (name, bytes) in beside.iter() {
                    files.push(Json::obj([("path", Json::str(*name)), ("text", Json::str(String::from_utf8_lossy(bytes)))]));
                }
                Json::obj([("files", Json::Arr(files)), ("command", Json::Null)])
            }
            Repro::Dir { files, command } => Json::obj([
                ("files", Json::arr(files.iter().map(|(p, t)| Json::obj([("path", Json::str(*p)), ("text", Json::str(*t))])))),
                ("command", Json::arr(command.iter().map(|c| Json::str(*c)))),
            ]),
        };
        Json::obj([
            ("code", Json::str(e.code)),
            ("severity", Json::str(e.severity.key())),
            ("title", Json::str(e.title.get(lang))),
            ("when", Json::str(e.when.get(lang))),
            ("fix", Json::str(e.fix.get(lang))),
            ("repro", repro),
            ("related", Json::arr(e.related.iter().map(|c| Json::str(*c)))),
        ])
    }

    /// Write the reproduction of `e` into `dir`, making directories. Nothing for a
    /// [`Repro::Later`].
    pub fn lay_out(&self, e: &Entry, dir: &Path) -> std::io::Result<()> {
        let write = |rel: &str, bytes: &[u8]| -> std::io::Result<()> {
            let p = dir.join(rel);
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(p, bytes)
        };
        match &e.repro {
            Repro::Later | Repro::Retired(_) => Ok(()),
            Repro::File { body, beside } => {
                for (name, bytes) in beside.iter() {
                    write(name, bytes)?;
                }
                write(self.example_file, body.as_bytes())
            }
            Repro::Dir { files, .. } => {
                for (name, body) in files {
                    write(name, body.as_bytes())?;
                }
                Ok(())
            }
        }
    }
}

/// Every reproduction prints its code: each is laid out in a fresh directory under `scratch`
/// and handed to `run`, which answers the codes it printed. An entry with a reproduction in
/// Japanese as well ([`Entry::repro_ja`]) is handed to `run` twice, the second time as an entry
/// whose reproduction is the Japanese one. The failures, one a line (an entry that is
/// [`Repro::Later`] or [`Repro::Retired`] is passed over). The directories are removed.
pub fn check_every(ledger: &Ledger, scratch: &Path, mut run: impl FnMut(&Entry, &Path) -> Vec<String>) -> Vec<String> {
    let mut failures = Vec::new();
    for e in &ledger.entries {
        if matches!(e.repro, Repro::Later | Repro::Retired(_)) {
            continue;
        }
        let mut variants = vec![(e.shown_in(Lang::En), "")];
        if e.repro_ja.is_some() {
            variants.push((e.shown_in(Lang::Ja), "-ja"));
        }
        for (entry, tag) in variants {
            let dir = scratch.join(format!("{}{tag}", e.code));
            let _ = std::fs::remove_dir_all(&dir);
            if let Err(err) = ledger.lay_out(&entry, &dir) {
                failures.push(format!("{}{tag}: cannot lay out the reproduction: {err}", e.code));
                continue;
            }
            let codes = run(&entry, &dir);
            if !codes.iter().any(|c| c == e.code) {
                failures.push(format!("{}{tag}: the reproduction printed {codes:?}", e.code));
            }
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
    failures
}

/// The codes that are in a ledger more than once: what a test holds a ledger to (none).
pub fn duplicates(ledger: &Ledger) -> Vec<&'static str> {
    let mut seen = std::collections::BTreeSet::new();
    ledger.entries.iter().map(|e| e.code).filter(|c| !seen.insert(*c)).collect()
}
