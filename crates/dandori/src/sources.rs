//! Where dandori reads what a `.flow` names: the child `.flow`s it runs, the descriptions of the
//! APIs it calls, the rules, through ritsu's port of rules (`ritsu_ports::Rules`, ritsu's DESIGN
//! 3.2), and the dates files and the books, through the ports of dates and books (`Dates`,
//! `Books`). dandori holds no rulec, koyomi or chobo: the program that runs it hands it the ports.
//!
//! - `Disk`: the disk (through ritsu_base::fs, which ritsu's playground holds in memory), and the
//!   ports it is handed. `ritsu dandori`, ritsu's playground and the tests hand it rulec's own
//!   answer (`rulec::ports::Engine`); the dandori binary of this crate hands it `NoRules`, which
//!   reads no rule and says to run the flow with `ritsu dandori` (ritsu's DESIGN 2.3).

use crate::diag::Lang;
use ritsu_base::text::Text;
use ritsu_ports::{Answer, BookFacts, Books, DateFacts, DateValue, Dates, DaySet, Found, Ledger, Precondition, RuleError, RuleFacts, Rules, Said, Values};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

pub trait Sources {
    /// The text of a file, or why it cannot be read.
    fn read(&self, path: &Path) -> Result<String, String>;
    /// One name for a file however the path to it is written, to tell a flow that runs itself.
    fn canonical(&self, path: &Path) -> PathBuf;
    /// The port the rules are read through.
    fn rules(&self) -> &dyn Rules;
    /// The port the dates files are read through: none, unless the program that runs dandori
    /// hands one over.
    fn dates(&self) -> &dyn Dates {
        &NoDates
    }
    /// The port the books are read through, likewise.
    fn books(&self) -> &dyn Books {
        &NoBooks
    }
    /// What the checks across the borders could not decide of the flow's calls of rules (ritsu's
    /// X2), which the code dandori writes checks when the workflow runs (DESIGN 1.17): nothing,
    /// unless the program that runs dandori hands it over.
    fn undecided(&self) -> Option<&dyn ritsu_ports::Undecided> {
        None
    }
}

thread_local! {
    static CURRENT: RefCell<Option<Rc<dyn Sources>>> = const { RefCell::new(None) };
}

/// Run `f` reading from `s`, then read from what was read from before.
pub fn with<R>(s: Rc<dyn Sources>, f: impl FnOnce() -> R) -> R {
    struct Restore(Option<Rc<dyn Sources>>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let before = self.0.take();
            CURRENT.with(|c| *c.borrow_mut() = before);
        }
    }
    let _restore = Restore(CURRENT.with(|c| c.borrow_mut().replace(s)));
    f()
}

/// Run `f` reading the disk, and the rules through `rules`.
pub fn with_rules<R>(rules: Rc<dyn Rules>, f: impl FnOnce() -> R) -> R {
    with(Rc::new(Disk::new(rules)), f)
}

/// Run `f` reading the disk, the rules through `rules`, the dates files through `dates` and the
/// books through `books` (what `ritsu dandori` hands over when it joins koyomi and chobo).
pub fn with_ports<R>(rules: Rc<dyn Rules>, dates: Rc<dyn Dates>, books: Rc<dyn Books>, f: impl FnOnce() -> R) -> R {
    with(Rc::new(Disk { rules, dates: Some(dates), books: Some(books), undecided: None }), f)
}

/// Run `f` as `with_ports`, and with what the checks across the borders could not decide of a
/// flow's calls of rules (`undecided`), which `build`, `run`, `scenarios` and `doc` put into the flow
/// as checks (DESIGN 1.17): what `ritsu dandori` hands over.
pub fn with_undecided<R>(rules: Rc<dyn Rules>, dates: Rc<dyn Dates>, books: Rc<dyn Books>, undecided: Rc<dyn ritsu_ports::Undecided>, f: impl FnOnce() -> R) -> R {
    with(Rc::new(Disk { rules, dates: Some(dates), books: Some(books), undecided: Some(undecided) }), f)
}

/// The preconditions the program that runs dandori says ritsu could not decide at the calls of
/// the flow at `path`: none, unless it hands them over.
pub fn undecided(path: &Path) -> Vec<ritsu_ports::UndecidedPrecondition> {
    let s = current();
    s.undecided().map(|u| u.preconditions(path)).unwrap_or_default()
}

/// What is read from now: the disk with no rules, unless `with` says otherwise.
fn current() -> Rc<dyn Sources> {
    CURRENT.with(|c| c.borrow().clone()).unwrap_or_else(|| Rc::new(Disk::new(Rc::new(NoRules))))
}

pub fn read(path: &Path) -> Result<String, String> {
    current().read(path)
}

pub fn canonical(path: &Path) -> PathBuf {
    current().canonical(path)
}

/// What rulec knows of a rule, or what is said instead.
pub fn rule(path: &Path) -> Result<RuleFacts, Vec<Said>> {
    current().rules().facts(path)
}

/// What koyomi knows of a dates file, or what is said instead.
pub fn dates(path: &Path) -> Result<DateFacts, Vec<Said>> {
    current().dates().facts(path)
}

/// The fewest and the most days from a dates file's date input to one of its dates, over the
/// whole range, as koyomi counts them (ritsu's DESIGN 7.7: how long a wait until the date's time
/// lasts, for the span of a hold, `crossings`).
pub fn date_span(path: &Path, date: &str) -> Result<Found<ritsu_ports::DaySpan>, Vec<Said>> {
    current().dates().span(path, date)
}

/// What chobo knows of a book, or what is said instead.
pub fn book(path: &Path) -> Result<BookFacts, Vec<Said>> {
    current().books().facts(path)
}

/// The page `rulec doc` draws for a rule, in a language: Markdown, or with `html` the page on which
/// a reader of the rule tries a case. It names the file alone, not where this machine keeps
/// it, and is shown as rulec drew it.
pub fn rule_doc(path: &Path, html: bool, lang: Lang) -> Result<String, Vec<Said>> {
    let shown = path.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
    current().rules().doc(path, &shown, html, lang)
}

/// What was said of a rule, as notes of the diagnostic that names it: each thing rulec says, with
/// its code and where it is.
pub fn said_notes(said: &[Said]) -> Vec<Text> {
    said.iter()
        .map(|s| {
            let at = match s.line {
                Some(l) => Text::new(format!("{} の {l} 行目", s.file), format!("{}, line {l}", s.file)),
                None => Text::same(s.file.clone()),
            };
            match s.code.as_str() {
                "" => s.message.clone(),
                code => Text::new(format!("{code}: {}（{}）", s.message.ja, at.ja), format!("{code}: {} ({})", s.message.en, at.en)),
            }
        })
        .collect()
}

/// What a port said of a file a flow reads, as notes, with every file under the flow's directory
/// named from there, as the flow names it (`books/stock.book`), not by the path it was reached by.
pub fn said_notes_from(said: &[Said], base: &std::path::Path) -> Vec<Text> {
    let dir = base.to_string_lossy().to_string();
    if dir.is_empty() {
        return said_notes(said);
    }
    let prefix = format!("{}/", dir.trim_end_matches('/'));
    let shown: Vec<Said> = said
        .iter()
        .map(|s| Said {
            file: s.file.strip_prefix(&prefix).map(str::to_string).unwrap_or_else(|| s.file.clone()),
            message: Text::new(s.message.ja.replace(&prefix, ""), s.message.en.replace(&prefix, "")),
            ..s.clone()
        })
        .collect();
    said_notes(&shown)
}

/// The disk, and the ports the program that runs dandori hands over.
pub struct Disk {
    rules: Rc<dyn Rules>,
    dates: Option<Rc<dyn Dates>>,
    books: Option<Rc<dyn Books>>,
    undecided: Option<Rc<dyn ritsu_ports::Undecided>>,
}

impl Disk {
    pub fn new(rules: Rc<dyn Rules>) -> Disk {
        Disk { rules, dates: None, books: None, undecided: None }
    }
}

impl Sources for Disk {
    fn read(&self, path: &Path) -> Result<String, String> {
        ritsu_base::fs::read_to_string(path).map_err(|e| e.to_string())
    }

    fn canonical(&self, path: &Path) -> PathBuf {
        ritsu_base::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
    }

    fn rules(&self) -> &dyn Rules {
        &*self.rules
    }

    fn dates(&self) -> &dyn Dates {
        match &self.dates {
            Some(d) => &**d,
            None => &NoDates,
        }
    }

    fn books(&self) -> &dyn Books {
        match &self.books {
            Some(b) => &**b,
            None => &NoBooks,
        }
    }

    fn undecided(&self) -> Option<&dyn ritsu_ports::Undecided> {
        self.undecided.as_deref()
    }
}

/// The ports of dates and books when none is handed over (the dandori binary of this crate, and a
/// program that joins rulec only): every question is answered with what to run instead.
pub struct NoDates;
pub struct NoBooks;

fn unread(file: &Path, ja: &str, en: &str) -> Vec<Said> {
    vec![Said {
        code: String::new(),
        file: file.display().to_string(),
        line: None,
        message: ritsu_base::tr!(
            "この dandori は{ja}を読めません。{ja}を使うワークフローは `ritsu dandori …` で走らせてください",
            "this dandori does not read {en}; run a workflow that uses {en} with `ritsu dandori …`"
        ),
    }]
}

impl Dates for NoDates {
    fn facts(&self, file: &Path) -> Result<DateFacts, Vec<Said>> {
        Err(unread(file, "日付のファイル", "dates files"))
    }

    fn values(&self, file: &Path, _: &str) -> Result<Found<DaySet>, Vec<Said>> {
        Err(unread(file, "日付のファイル", "dates files"))
    }

    fn days(&self, file: &Path, _: &str) -> Result<Found<(i64, i64)>, Vec<Said>> {
        Err(unread(file, "日付のファイル", "dates files"))
    }

    fn eval(&self, file: &Path, _: &[(String, i64)]) -> Result<Vec<(String, DateValue)>, Vec<Said>> {
        Err(unread(file, "日付のファイル", "dates files"))
    }

    fn joined(&self) -> bool {
        false
    }
}

impl Books for NoBooks {
    fn facts(&self, file: &Path) -> Result<BookFacts, Vec<Said>> {
        Err(unread(file, "帳簿", "books"))
    }

    fn refusals(&self, file: &Path, _: &str, _: (i128, i128)) -> Result<Found<Vec<(String, Vec<String>)>>, Vec<Said>> {
        Err(unread(file, "帳簿", "books"))
    }

    fn open(&self, file: &Path) -> Result<Box<dyn Ledger>, Vec<Said>> {
        Err(unread(file, "帳簿", "books"))
    }

    fn joined(&self) -> bool {
        false
    }
}

/// The port of the dandori binary of this crate, which holds no rulec (ritsu's DESIGN 2.3): every
/// question about a rule is answered with what to run instead. A flow that uses no rule never asks.
pub struct NoRules;

fn no_rules(rule: &Path) -> Vec<Said> {
    vec![Said {
        code: String::new(),
        file: rule.display().to_string(),
        line: None,
        message: ritsu_base::tr!(
            "この dandori は規則を読めません。規則を使うワークフローは `ritsu dandori …` で走らせてください",
            "this dandori does not read rules; run a workflow that uses rules with `ritsu dandori …`"
        ),
    }]
}

impl Rules for NoRules {
    fn facts(&self, rule: &Path) -> Result<RuleFacts, Vec<Said>> {
        Err(no_rules(rule))
    }

    fn preconditions_hold(&self, rule: &Path, _: &[(String, Option<i128>, Option<i128>)], _: Option<i128>) -> Result<Vec<(Precondition, Answer<Values>)>, Vec<Said>> {
        Err(no_rules(rule))
    }

    fn output_values(&self, rule: &Path, _: &str) -> Result<ritsu_ports::Found<ritsu_ports::OutputValues>, Vec<Said>> {
        Err(no_rules(rule))
    }

    fn checked_over(&self, rule: &Path, _: &str, _: &DaySet) -> Result<Answer<Text>, Vec<Said>> {
        Err(no_rules(rule))
    }

    fn eval(&self, rule: &Path, _: &Values) -> Result<Values, RuleError> {
        Err(RuleError::Unread(no_rules(rule)))
    }

    fn doc(&self, rule: &Path, _: &str, _: bool, _: Lang) -> Result<String, Vec<Said>> {
        Err(no_rules(rule))
    }

    fn joined(&self) -> bool {
        false
    }
}

/// Whether the rules a flow uses can be read here: false in the dandori binary of this crate,
/// which holds no rulec (E018).
pub fn rules_joined() -> bool {
    current().rules().joined()
}

/// Whether the dates files a flow uses can be read here, likewise (koyomi).
pub fn dates_joined() -> bool {
    current().dates().joined()
}

/// Whether the books a flow uses can be read here, likewise (chobo).
pub fn books_joined() -> bool {
    current().books().joined()
}
