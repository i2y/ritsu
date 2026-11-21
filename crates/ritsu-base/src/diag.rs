//! Diagnostics (DESIGN 4.2): what every language's diagnostic has — a code, how bad it is, a
//! place, the message in both languages, notes, and the fix — and how it is printed, as text
//! and as JSON.
//!
//! The text is the form dandori, koyomi, chobo, geas, yuen and sakai print:
//!
//! ```text
//! error[E302]: tests/x/民法の期間.req:41:3: 民法 第142条 changed after 法務 looked at this link
//!     41 |   from @民法 第142条
//!   = a note
//!   = The line, fixed: …
//! ```
//!
//! What led to a diagnostic is each language's own (koyomi's computation a step a line, yuen's
//! diff and chain, sakai's references, chobo's operations), so it is a type of the language's,
//! an [`Extra`], which says where its lines go and which keys it adds to the JSON.
//!
//! A place is written twice: [`Diag::file`] for a person, from where the tool runs and the way
//! the path was given (DESIGN 6.2, item 8), and [`Diag::rel`] for the JSON, from the root of
//! the project (item 9). The JSON a tool prints puts `root` beside its diagnostics.

use crate::json::Json;
use crate::text::{Lang, Text, as_written};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

impl Severity {
    /// `E…` is an error, `W…` a warning, `N…` a note.
    pub fn of(code: &str) -> Severity {
        match code.as_bytes().first() {
            Some(b'W') => Severity::Warning,
            Some(b'N') => Severity::Note,
            _ => Severity::Error,
        }
    }

    /// The word the headline starts with.
    pub fn word(self, lang: Lang) -> &'static str {
        match (self, lang) {
            (Severity::Error, Lang::En) => "error",
            (Severity::Warning, Lang::En) => "warning",
            (Severity::Note, Lang::En) => "note",
            (Severity::Error, Lang::Ja) => "エラー",
            (Severity::Warning, Lang::Ja) => "警告",
            (Severity::Note, Lang::Ja) => "備考",
        }
    }

    /// The word the JSON writes, in English whatever the language.
    pub fn key(self) -> &'static str {
        self.word(Lang::En)
    }
}

/// What fixes it: a line to write in the file, ready to paste, or a command to run once a
/// person has looked at what the diagnostic shows (yuen's `review`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fix {
    Line(String),
    Command(Text),
}

/// A language's own part of a diagnostic: what led there.
pub trait Extra: Clone + std::fmt::Debug {
    /// Lines after the notes and before the fix (yuen's chain, diff and candidates).
    fn before_fix(&self, _lang: Lang, _out: &mut String) {}
    /// Lines after the fix (koyomi's computation, sakai's references, chobo's operations).
    fn after_fix(&self, _lang: Lang, _out: &mut String) {}
    /// The keys the JSON has between `notes` and `fix`, in their order.
    fn json(&self, _lang: Lang) -> Vec<(String, Json)> {
        Vec::new()
    }
    /// How a message and a note are printed: as written by default ([`as_written`]); koyomi
    /// and sakai space and capitalize them ([`crate::text::spaced`]).
    fn say(&self, t: &Text, lang: Lang) -> String {
        as_written(t, lang)
    }
    /// Whether the JSON has a `fix` key. geas's diagnostics never carry a fixed line, and their
    /// JSON has never had the key, so geas leaves it out.
    fn fix_key(&self) -> bool {
        true
    }
}

/// A diagnostic with nothing of a language's own.
impl Extra for () {}

#[derive(Clone, Debug)]
pub struct Diag<X: Extra = ()> {
    pub code: &'static str,
    pub severity: Severity,
    /// The file as a person opens it: from where the tool runs, the way the path was given.
    pub file: String,
    /// The same file from the root of the project, for the JSON.
    pub rel: String,
    pub line: Option<usize>,
    pub col: Option<usize>,
    /// The text of that line.
    pub src: Option<String>,
    pub message: Text,
    pub notes: Vec<Text>,
    pub fix: Option<Fix>,
    /// Whether the text shows the fix on a line of its own; not when a note already says it.
    pub show_fix: bool,
    pub extra: X,
    /// The name the diagnostic says is declared nowhere, and where it is looked for (a transfer's
    /// parameters, a function's): the diagnostics of one file that say so of one name in one place
    /// are one ([`once`]). Not printed.
    pub same: Option<(String, String)>,
}

impl<X: Extra + Default> Diag<X> {
    /// At a line and a column of a file (0 for either is none). The JSON's path is the same as
    /// the text's until [`Diag::rel`] says otherwise.
    pub fn new(code: &'static str, severity: Severity, file: &str, line: usize, col: usize, message: Text) -> Diag<X> {
        Diag {
            code,
            severity,
            file: file.to_string(),
            rel: file.to_string(),
            line: (line > 0).then_some(line),
            col: (col > 0).then_some(col),
            src: None,
            message,
            notes: Vec::new(),
            fix: None,
            show_fix: true,
            extra: X::default(),
            same: None,
        }
    }

    /// How bad it is, from the code's first letter ([`Severity::of`]).
    pub fn at(code: &'static str, file: &str, line: usize, col: usize, message: Text) -> Diag<X> {
        Diag::new(code, Severity::of(code), file, line, col, message)
    }

    pub fn error(code: &'static str, file: &str, line: usize, col: usize, message: Text) -> Diag<X> {
        Diag::new(code, Severity::Error, file, line, col, message)
    }

    pub fn warning(code: &'static str, file: &str, line: usize, col: usize, message: Text) -> Diag<X> {
        Diag::new(code, Severity::Warning, file, line, col, message)
    }

    /// About a whole file: no line.
    pub fn whole(code: &'static str, file: &str, message: Text) -> Diag<X> {
        Diag::new(code, Severity::of(code), file, 0, 0, message)
    }
}

impl<X: Extra> Diag<X> {
    /// The file from the root, for the JSON.
    pub fn rel(mut self, rel: &str) -> Diag<X> {
        self.rel = rel.to_string();
        self
    }

    /// The line's text, from the file's source, with any key on it masked
    /// ([`crate::secrets::mask`]): a diagnostic's line must not carry a key into the logs.
    pub fn source(mut self, src: &str) -> Diag<X> {
        if let Some(l) = self.line {
            self.src = src.lines().nth(l - 1).map(|s| crate::secrets::mask(s.trim_end()));
        }
        self
    }

    pub fn note(mut self, t: Text) -> Diag<X> {
        self.notes.push(t);
        self
    }

    pub fn fix_line(mut self, line: impl Into<String>) -> Diag<X> {
        self.fix = Some(Fix::Line(line.into()));
        self
    }

    /// The fixed line, which a note has already spelled out: the JSON has it, the text does not
    /// say it twice.
    pub fn fix_in_notes(mut self, line: impl Into<String>) -> Diag<X> {
        self.fix = Some(Fix::Line(line.into()));
        self.show_fix = false;
        self
    }

    pub fn fix_command(mut self, c: Text) -> Diag<X> {
        self.fix = Some(Fix::Command(c));
        self
    }

    /// The language's own part.
    pub fn with(mut self, extra: X) -> Diag<X> {
        self.extra = extra;
        self
    }

    /// The diagnostic says that `name` is declared nowhere in the file ([`once`]).
    pub fn same(mut self, name: impl Into<String>) -> Diag<X> {
        self.same = Some((String::new(), name.into()));
        self
    }

    /// [`Diag::same`] for a diagnostic already pushed.
    pub fn same_mut(&mut self, name: impl Into<String>) -> &mut Diag<X> {
        self.same = Some((String::new(), name.into()));
        self
    }

    /// [`Diag::same_in`] for a diagnostic already pushed.
    pub fn same_in_mut(&mut self, scope: impl Into<String>, name: impl Into<String>) -> &mut Diag<X> {
        self.same = Some((scope.into(), name.into()));
        self
    }

    /// The diagnostic says that `name` is declared nowhere in `scope` (a transfer's parameters, a
    /// function's): the same name missing from another is another mistake ([`once`]).
    pub fn same_in(mut self, scope: impl Into<String>, name: impl Into<String>) -> Diag<X> {
        self.same = Some((scope.into(), name.into()));
        self
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }

    /// The line to write instead, when the fix is a line.
    pub fn fixed_line(&self) -> Option<&str> {
        match &self.fix {
            Some(Fix::Line(l)) => Some(l),
            _ => None,
        }
    }

    /// `file:line:col`, `file:line`, or the file.
    pub fn place(&self) -> String {
        match (self.line, self.col) {
            (Some(l), Some(c)) => format!("{}:{l}:{c}", self.file),
            (Some(l), None) => format!("{}:{l}", self.file),
            _ => self.file.clone(),
        }
    }

    /// What a person reads. A diagnostic with no file (geas's about its command line) has no
    /// place before its message.
    pub fn render(&self, lang: Lang) -> String {
        let place = self.place();
        let at = if place.is_empty() { String::new() } else { format!("{place}: ") };
        let mut out = format!("{}[{}]: {at}{}\n", self.severity.word(lang), self.code, self.extra.say(&self.message, lang));
        if let (Some(s), Some(l)) = (&self.src, self.line) {
            out.push_str(&format!("  {l:>4} | {s}\n"));
        }
        for n in &self.notes {
            out.push_str(&format!("  = {}\n", self.extra.say(n, lang)));
        }
        self.extra.before_fix(lang, &mut out);
        match &self.fix {
            Some(Fix::Line(l)) if self.show_fix => {
                let head = if lang == Lang::Ja { "直した行" } else { "The line, fixed" };
                out.push_str(&format!("  = {head}: {}\n", l.trim()));
            }
            Some(Fix::Command(c)) if self.show_fix => {
                let head = if lang == Lang::Ja { "確かめたら" } else { "Once a person has looked" };
                out.push_str(&format!("  = {head}: {}\n", c.get(lang)));
            }
            _ => {}
        }
        self.extra.after_fix(lang, &mut out);
        out
    }

    /// The same finding for a program. The keys are English whatever the language, in this
    /// order: `code`, `severity`, `file` (from the root; null when there is none), `line`, `col`,
    /// `message`, `notes`, the language's own, `fix` (unless the language has none,
    /// [`Extra::fix_key`]).
    pub fn to_json(&self, lang: Lang) -> Json {
        let mut o: Vec<(String, Json)> = vec![
            ("code".into(), Json::str(self.code)),
            ("severity".into(), Json::str(self.severity.key())),
            ("file".into(), if self.rel.is_empty() { Json::Null } else { Json::str(&self.rel) }),
            ("line".into(), self.line.into()),
            ("col".into(), self.col.into()),
            ("message".into(), Json::str(self.extra.say(&self.message, lang))),
            ("notes".into(), Json::arr(self.notes.iter().map(|n| Json::str(self.extra.say(n, lang))))),
        ];
        o.extend(self.extra.json(lang));
        if self.extra.fix_key() {
            o.push((
                "fix".into(),
                match &self.fix {
                    Some(Fix::Line(l)) => Json::str(l),
                    Some(Fix::Command(c)) => Json::str(c.get(lang)),
                    None => Json::Null,
                },
            ));
        }
        Json::Obj(o)
    }
}

/// How many of each.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub errors: usize,
    pub warnings: usize,
    pub notes: usize,
}

pub fn count<X: Extra>(diags: &[Diag<X>]) -> Counts {
    let mut c = Counts::default();
    for d in diags {
        match d.severity {
            Severity::Error => c.errors += 1,
            Severity::Warning => c.warnings += 1,
            Severity::Note => c.notes += 1,
        }
    }
    c
}

pub fn has_errors<X: Extra>(diags: &[Diag<X>]) -> bool {
    diags.iter().any(Diag::is_error)
}

/// One name declared nowhere is one diagnostic (DESIGN 8.3): of the diagnostics of one file that
/// say so of one name ([`Diag::same`]), the first in the file is kept, with a note of the other
/// lines the name is written on, and the others are dropped. The reader fixes the declaration, or
/// the lines the note lists.
pub fn once<X: Extra>(diags: &mut Vec<Diag<X>>) {
    once_with(diags, written_too);
}

/// [`once`], with the note a language writes: [`written_too`], or, where notes end with no stop
/// (chobo, geas), [`written_too`] through [`unstopped`].
pub fn once_with<X: Extra>(diags: &mut Vec<Diag<X>>, note: impl Fn(&str, &[usize]) -> Text) {
    let mut order: Vec<usize> = (0..diags.len()).collect();
    order.sort_by_key(|&i| (diags[i].line.unwrap_or(usize::MAX), diags[i].col.unwrap_or(0)));
    let mut firsts: Vec<((String, String, String), usize, Vec<usize>)> = Vec::new();
    let mut drop = vec![false; diags.len()];
    for i in order {
        let Some((scope, name)) = diags[i].same.clone() else { continue };
        let key = (diags[i].file.clone(), scope, name);
        match firsts.iter_mut().find(|(k, ..)| *k == key) {
            Some((_, _, lines)) => {
                drop[i] = true;
                lines.extend(diags[i].line);
            }
            None => firsts.push((key, i, Vec::new())),
        }
    }
    for ((_, _, name), i, mut lines) in firsts {
        lines.retain(|l| Some(*l) != diags[i].line);
        lines.sort_unstable();
        lines.dedup();
        if !lines.is_empty() {
            diags[i].notes.push(note(&name, &lines));
        }
    }
    let mut k = 0;
    diags.retain(|_| {
        k += 1;
        !drop[k - 1]
    });
}

/// The note [`once`] adds: `` `x` is written on line 12 too. ``, `` `x` is written on lines 12 and
/// 15 too. ``
pub fn written_too(name: &str, lines: &[usize]) -> Text {
    let ja = lines.iter().map(|l| l.to_string()).collect::<Vec<_>>().join("、");
    let en = match lines {
        [l] => format!("line {l}"),
        [rest @ .., last] => format!("lines {} and {last}", rest.iter().map(|l| l.to_string()).collect::<Vec<_>>().join(", ")),
        [] => String::new(),
    };
    Text::new(format!("`{name}` は {ja} 行目にも書かれています。"), format!("`{name}` is written on {en} too."))
}

/// A note as the languages whose notes end with no stop write it: the last `.` and `。` dropped.
pub fn unstopped(t: Text) -> Text {
    Text::new(t.ja.strip_suffix('。').unwrap_or(&t.ja).to_string(), t.en.strip_suffix('.').unwrap_or(&t.en).to_string())
}
