//! The rules this thread has checked, and read as far as their types (ritsu's DESIGN 4.15): the
//! report of each check ([`crate::report_with`]), and the rule a check read, merged with the
//! contributions of the run and typed, or the one [`crate::prepare`] read, each kept by the file,
//! its path as the run writes it, its text and what the run joins, while every file the check read
//! reads the same and the contributions of the run that come down to the files it merged are the
//! same ([`Read::holds`]), whichever files the thread reads them through (the disk, or files laid
//! over it, as ringi lays the rules it lowers a request to). A file's older text is let go when
//! another is kept. A rule asked about
//! again in the run is answered from what is kept: the check of an `enums`
//! file a rule reads, of a rule another applies, of a rule a port asks about after its own check, is
//! the one check of the file; and [`crate::prepare`] hands out a copy of a kept rule instead of
//! reading it again, so a rule that `rulec gen`, `rulec doc`, `ritsu gen` or a port's answer reads
//! after its check is read once in the run. A report is kept in the language its findings are
//! written in; what reads of a check only whether the rule passes and the codes of its errors is
//! answered from a report in either language ([`report_in_either`]).
//!
//! A check over other ranges (the inputs held to some of their values, `over::with`; a date held
//! to some days, `days::over`) reads another rule, and is neither kept nor handed out here.

use crate::ast::RuleFile;
use crate::types::Checked;
use ritsu_base::fs::Seen;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::PathBuf;

/// What a check of a rule rests on besides the files it read and the contributions that come down
/// to the files it merged: the file, the path it was asked by (the findings and the generated
/// headers name it) and the way the run writes it, its text, the port of dates the run joins
/// (§15.174), the files the thread reads (the disk, memory, a git revision: what a file's older
/// text is let go by, [`Key::let_go_for`], and not what a check is found by, [`Key::answers`]), and
/// the file the spans made now are marked as in (a rule read for a contribution's scope, §15.227).
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct Key {
    file: PathBuf,
    path: String,
    shown: String,
    text: String,
    dates: Dates,
    files: usize,
    spans: Option<(String, String)>,
}

/// The port of dates joined on the thread, held by the key that rests on it: one port is told from
/// another by where it is, and a port held here is not let go, so no other port comes to be where it
/// was.
#[derive(Clone)]
struct Dates(Option<crate::days::Port>);

impl PartialEq for Dates {
    fn eq(&self, other: &Dates) -> bool {
        match (&self.0, &other.0) {
            (None, None) => true,
            (Some(a), Some(b)) => std::sync::Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl std::fmt::Debug for Dates {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            None => write!(f, "none"),
            Some(p) => write!(f, "{:p}", std::sync::Arc::as_ptr(p)),
        }
    }
}

/// What a check read: every file, with its bytes (in the copy the checks of the thread share); the
/// files it looked for the contributions of the run of (the rule, the `enums` files it reads, the
/// rules it applies, `link::covering`); and the contribution files of the run then
/// (`link::active`), as written from the directory the run was in.
pub(crate) struct Read {
    seen: Seen,
    covers: Vec<String>,
    set: Vec<String>,
    at: PathBuf,
}

impl Read {
    /// Whether a check now would read the same: every file reads the same (asked again through the
    /// files the thread reads now, so a check watched around this one rests on them too, as it
    /// rests on the files this one merged), and so do the contributions it merged. Those are the
    /// same where the run's contribution files are written the same from the same directory: a
    /// check that looked for contributions read every contribution file of the run, and those read
    /// the same. A check that looked for none merged none. Where the run's files are others (a rule
    /// checked alone, §15.231, and the same rule merged), the contributions that come down to the
    /// files it looked for are asked of both (`link::reaching`).
    fn holds(&self) -> bool {
        let set = crate::link::active();
        let same = if self.covers.is_empty() || set == self.set {
            self.at == here() && self.seen.still()
        } else {
            self.at == here() && self.seen.still() && crate::link::reaching(&set, &self.covers) == crate::link::reaching(&self.set, &self.covers)
        };
        if same {
            crate::link::cover(&self.covers);
        }
        same
    }

    /// Whether a check now of the same rule would read with the same contribution files as this
    /// one: the one to keep of the two.
    fn same_run(&self, other: &Read) -> bool {
        self.set == other.set && self.at == other.at
    }
}

/// The directory the run is in, which the paths of its contribution files are written from.
fn here() -> PathBuf {
    ritsu_base::fs::current_dir().unwrap_or_default()
}

/// `f` run with what it reads watched: the files (`ritsu_base::fs::watched`), the files whose
/// contributions of the run it looked for (`link::covering`), and the run's contribution files.
pub(crate) fn watch<R>(f: impl FnOnce() -> R) -> (R, Read) {
    let set = crate::link::active();
    let at = here();
    let ((got, seen), covers) = crate::link::covering(|| ritsu_base::fs::watched(f));
    (got, Read { seen, covers, set, at })
}

/// A rule read as far as its types: as [`crate::prepare_lenient`] reads it (`file` before the
/// identifiers of what is generated are assigned), whether a pin did not match (E040, which
/// [`crate::prepare`] stops at), and what the reading read.
struct Kept {
    key: Key,
    read: Read,
    file: RuleFile,
    checked: Checked,
    pinned: bool,
}

/// A report of a rule's check, in the language its findings are written in and to the budget the
/// check was held to, and what the check read.
struct Said {
    key: Key,
    lang: crate::i18n::Lang,
    budget: i64,
    read: Read,
    report: crate::Report,
}

/// How many rules a thread keeps, of each: the rules of a run of the size of a project, the oldest
/// let go first.
const EACH_THREAD: usize = 256;

thread_local! {
    static KEPT: RefCell<VecDeque<Kept>> = const { RefCell::new(VecDeque::new()) };
    static SAID: RefCell<VecDeque<Said>> = const { RefCell::new(VecDeque::new()) };
}

/// The key of the rule of the text `src` asked by `path` on this thread now, or None where the
/// rule is read over other ranges than its own.
pub(crate) fn key(src: &str, path: &str) -> Option<Key> {
    if crate::over::holding() || crate::days::holding_over() {
        return None;
    }
    let p = std::path::Path::new(path);
    Some(Key {
        file: ritsu_base::paths::opened(p),
        path: path.to_string(),
        shown: ritsu_base::paths::show(p),
        text: crate::sha256::hex(src.as_bytes()),
        dates: Dates(crate::days::port()),
        files: ritsu_base::fs::id(),
        spans: crate::diag::spans_in(),
    })
}

impl Key {
    /// Whether what is kept under this key is let go when `newer` is kept: the same question asked
    /// again (the newer answer is the one to give), or the file asked with another text in the same
    /// files. A file reads one text at a time, so the older text is not asked again but where the
    /// text is a variant made of the file (`rulec strength` checks a rule under each change), and
    /// one kept answer for each variant would hold as many rules as the thread keeps.
    fn let_go_for(&self, newer: &Key) -> bool {
        self == newer || (self.file == newer.file && self.files == newer.files && self.text != newer.text)
    }

    /// Whether what is kept under this key answers the question asked under `asked`: the same in
    /// all but the files the thread reads. A check made through other files answers where every
    /// question it asked of them reads the same through the files of now ([`Read::holds`], which
    /// asks again what each file it read held, the files it looked for and did not find, and the
    /// directories it listed): a rule a check made on the disk, applied by a rule ringi lays over
    /// the disk, is that one check (ritsu's DESIGN 4.15).
    fn answers(&self, asked: &Key) -> bool {
        let Key { file, path, shown, text, dates, files: _, spans } = self;
        *file == asked.file && *path == asked.path && *shown == asked.shown && *text == asked.text && *dates == asked.dates && *spans == asked.spans
    }
}

/// Keeps the rule read under `key`, with what its reading read.
pub(crate) fn keep(key: Key, read: Read, file: RuleFile, checked: Checked, pinned: bool) {
    KEPT.with(|k| {
        let mut k = k.borrow_mut();
        k.retain(|e| !(e.key.let_go_for(&key) && (e.key != key || e.read.same_run(&read))));
        if k.len() >= EACH_THREAD {
            k.pop_front();
        }
        k.push_back(Kept { key, read, file, checked, pinned });
    });
}

/// A copy of the rule kept under a key that answers `key` ([`Key::answers`]) whose reading a reading
/// now would read the same ([`Read::holds`]). Only one without a pin that does not match, unless
/// `lenient`.
pub(crate) fn find(key: &Key, lenient: bool) -> Option<(RuleFile, Checked)> {
    KEPT.with(|k| {
        let k = k.borrow();
        let e = k.iter().rev().find(|e| e.key.answers(key) && (lenient || !e.pinned) && e.read.holds())?;
        Some((e.file.clone(), e.checked.clone()))
    })
}

/// Keeps the report of the check under `key`, in `lang` and to `budget`, with what the check read.
pub(crate) fn keep_report(key: Key, lang: crate::i18n::Lang, budget: i64, read: Read, report: crate::Report) {
    SAID.with(|k| {
        let mut k = k.borrow_mut();
        k.retain(|e| !(e.key.let_go_for(&key) && (e.key != key || (e.lang == lang && e.budget == budget && e.read.same_run(&read)))));
        if k.len() >= EACH_THREAD {
            k.pop_front();
        }
        k.push_back(Said { key, lang, budget, read, report });
    });
}

/// A copy of the report kept under a key that answers `key` ([`Key::answers`]) in `lang` to `budget`
/// whose check a check now would read the same ([`Read::holds`]).
pub(crate) fn report(key: &Key, lang: crate::i18n::Lang, budget: i64) -> Option<crate::Report> {
    SAID.with(|k| {
        let k = k.borrow();
        let e = k.iter().rev().find(|e| e.key.answers(key) && e.lang == lang && e.budget == budget && e.read.holds())?;
        Some(e.report.clone())
    })
}

/// [`report`] in whichever language the report was kept in: for what reads of a check only whether
/// the rule passes and the codes of its errors (`crate::verdict`), which are the same in both
/// languages. A report shown to a person is the one in the language it is shown in ([`report`]).
pub(crate) fn report_in_either(key: &Key, budget: i64) -> Option<crate::Report> {
    SAID.with(|k| {
        let k = k.borrow();
        let e = k.iter().rev().find(|e| e.key.answers(key) && e.budget == budget && e.read.holds())?;
        Some(e.report.clone())
    })
}
