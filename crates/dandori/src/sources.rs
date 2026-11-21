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
    fn read(&self, path: &Path) -> Result<String, std::io::Error>;
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
    /// The ports themselves, as the program that runs dandori handed them over: what a check kept
    /// by `ports::Engine` was made with. None where nothing was handed over.
    fn held(&self) -> Option<Held> {
        None
    }
}

/// The ports a check reads the rules, the dates files and the books through, as they were handed
/// over. Two are the same when they are the same ports (the same objects): a check kept with one
/// is not taken for a check with another (`ports::Engine`). Holding them keeps their places from
/// being taken by others.
#[derive(Clone)]
pub struct Held {
    rules: Rc<dyn Rules>,
    dates: Option<Rc<dyn Dates>>,
    books: Option<Rc<dyn Books>>,
}

impl Held {
    /// The three ports, when the program that runs dandori handed over all three (`ritsu dandori`):
    /// what `dandori run` and `dandori scenarios` compute the calls of rules, dates and books with.
    pub fn ports(&self) -> Option<(Rc<dyn Rules>, Rc<dyn Dates>, Rc<dyn Books>)> {
        Some((self.rules.clone(), self.dates.clone()?, self.books.clone()?))
    }

    /// The three ports, each as it was handed over, or, where it was not (the dates and the books
    /// of a program that joins rulec only), one that answers every question with what to run
    /// instead (`NoDates`, `NoBooks`).
    pub fn or_unread(&self) -> (Rc<dyn Rules>, Rc<dyn Dates>, Rc<dyn Books>) {
        let dates: Rc<dyn Dates> = self.dates.clone().unwrap_or_else(|| Rc::new(NoDates));
        let books: Rc<dyn Books> = self.books.clone().unwrap_or_else(|| Rc::new(NoBooks));
        (self.rules.clone(), dates, books)
    }

    pub fn same(&self, o: &Held) -> bool {
        fn at<T: ?Sized>(x: &Rc<T>) -> *const () {
            Rc::as_ptr(x) as *const ()
        }
        at(&self.rules) == at(&o.rules) && self.dates.as_ref().map(at) == o.dates.as_ref().map(at) && self.books.as_ref().map(at) == o.books.as_ref().map(at)
    }
}

/// The ports the checks read through now: none, unless the program that runs dandori handed them over.
pub fn held() -> Option<Held> {
    CURRENT.with(|c| c.borrow().as_ref().and_then(|s| s.held()))
}

thread_local! {
    static CURRENT: RefCell<Option<Rc<dyn Sources>>> = const { RefCell::new(None) };
}

/// The file a path written in a `.flow` names, from the directory `dir` of the `.flow`, as dandori
/// reaches it (ritsu's DESIGN 17.6): outside a module of ritsu's, `dir.join(written)`; in one, the
/// path held to the module, and `@<name>/…` a file of a dependency, read from the module's root.
pub fn follow(dir: &Path, written: &str) -> Result<PathBuf, ritsu_base::modules::Refused> {
    ritsu_base::modules::refer_shown(dir, written)
}

/// [`follow`], where what a refused path is has been said once, where the file is read: the
/// directory joined to it, as written.
pub fn reach(dir: &Path, written: &str) -> PathBuf {
    follow(dir, written).unwrap_or_else(|_| dir.join(written))
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
/// flow's calls of rules (`undecided`), which `gen`, `run`, `scenarios` and `doc` put into the flow
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

pub fn read(path: &Path) -> Result<String, std::io::Error> {
    current().read(path)
}

pub fn canonical(path: &Path) -> PathBuf {
    current().canonical(path)
}

/// What rulec knows of a rule, or what is said instead.
pub fn rule(path: &Path) -> Result<RuleFacts, Vec<Said>> {
    current().rules().facts(path)
}

/// What a call of a rule comes to, through the port of rules the program that runs dandori hands
/// over, as `ritsu run` answers it (`computed::rule_answer`).
pub fn rule_answer(m: &crate::model::Model, r: usize, args: &serde_json::Map<String, serde_json::Value>) -> Result<serde_json::Value, ritsu_base::text::Text> {
    let s = current();
    crate::computed::rule_answer(s.rules(), m, r, args)
}

/// The same, as the record of the rule's outputs the flow reads, or why the call is refused at the
/// door (`computed::rule_outputs`).
pub fn rule_outputs(m: &crate::model::Model, r: usize, args: &serde_json::Map<String, serde_json::Value>) -> Result<Result<serde_json::Value, String>, ritsu_base::text::Text> {
    let s = current();
    crate::computed::rule_outputs(s.rules(), m, r, args)
}

/// What the enum and truth-value outputs of a rule come to together with its inputs held to
/// `ranges` (`Rules::outputs_together`): each combination, with an input that comes to it, exact or
/// undecided.
pub fn outputs_together(path: &Path, outputs: &[String], ranges: &[(String, Option<i128>, Option<i128>)]) -> Result<Found<Vec<(Vec<ritsu_ports::Value>, ritsu_ports::Values)>>, Vec<Said>> {
    current().rules().outputs_together(path, outputs, ranges)
}

/// The inputs of the vectors rulec makes of the rule `r`, as a call of the flow gives them
/// (`computed::rule_vectors`); none without rulec.
pub fn rule_vectors(m: &crate::model::Model, r: usize) -> Vec<serde_json::Map<String, serde_json::Value>> {
    let s = current();
    crate::computed::rule_vectors(s.rules(), m, r)
}

/// What koyomi knows of a dates file, or what is said instead.
pub fn dates(path: &Path) -> Result<DateFacts, Vec<Said>> {
    current().dates().facts(path)
}

/// Every day a date of a dates file comes to over the whole range of its inputs, as koyomi counts
/// them (the day of the date, where dandori holds it to a range: `ranges`).
pub fn date_values(path: &Path, date: &str) -> Result<Found<DaySet>, Vec<Said>> {
    current().dates().values(path, date)
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

/// What was said of a file a flow reads (a rule, a dates file, a book), as notes of the diagnostic
/// that names it: each thing its language says, with its code and where it is, the file named the
/// way the run writes its paths, as the language's port names it (ritsu's DESIGN 6.2, item 8).
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
    fn held(&self) -> Option<Held> {
        Some(Held { rules: self.rules.clone(), dates: self.dates.clone(), books: self.books.clone() })
    }

    fn read(&self, path: &Path) -> Result<String, std::io::Error> {
        ritsu_base::fs::read_to_string(path)
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

    fn preconditions_hold(&self, rule: &Path, _: &[(String, Option<i128>, Option<i128>)], _: Option<i128>, _: Option<i128>) -> Result<Vec<(Precondition, Answer<Values>)>, Vec<Said>> {
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
