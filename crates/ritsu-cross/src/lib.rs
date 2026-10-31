//! ritsu's checks across its languages (DESIGN 7), and the ledger of their codes ([`codes`]):
//! what `ritsu check` says itself, after every language's own check has said its part. Each code
//! is ritsu's, apart from the languages' ledgers (whose numbers overlap); `ritsu explain` looks
//! one up, and the headline of every finding says `ritsu` (`error[ritsu E101]`).
//!
//! A check here reads the languages through the ports only (DESIGN 3.1): ritsu-project joins them
//! and hands over the project and its index. What each check comes to is one of P5's three — shown
//! to hold, an example where it does not, or undecided — and [`Borders`] keeps each border, with
//! the check that held it to the other side ([`Check`]), its two sides and what it came to.
//!
//! The checks: the `.proto` files of a project, which no language checks on its own, read with
//! ritsu's one reader (E101); and the borders between the languages (DESIGN 7.2), where a workflow
//! calls a rule, a koyomi date or a chobo transfer: a rule's preconditions (X2, E201 and W201), the
//! days of a koyomi date given to a rule (X3 (a), E202 and W202), a rule's output given to a
//! transfer as its amount and the refusals that amount can meet (X4, E203–W204), the day given to a
//! koyomi date (X6, E205 and W205), and how long a hold of a book is held before a call its expiry
//! can refuse (X5, E206 and W206). What each decides is in [`borders`] (and X2's in its module);
//! the checks gather the facts from what dandori says a flow calls (`Flows::crossings`) and what
//! rulec, koyomi and chobo say of it. X1, the units at a border, is the languages' own check
//! (dandori's E003, rulec's E065), and X3 (b) is rulec's (`range from koyomi`, rulec's §15.174).
//!
//! The same checks hold where a module of kaname calls a rule, a koyomi date or a chobo transfer
//! (X2, X3 (a), X4, X6; kaname's DESIGN 14.3), with the same codes, over what kaname says a module
//! calls (`Programs::crossings`): [`ProgramVerdicts`] decides them, hands kaname the verdicts it
//! decides the effects of its functions with (ritsu's port `Verdicts`), and is what `ritsu check`
//! says of them.
//!
//! Two are of security (DESIGN 16): a key written in a contract of the project, a `.proto` or a
//! document a language reads, looked at once whatever reads it ([`secrets`], W901); and where a
//! flow sends a value a contract marks secret, against sakai's map ([`egress`], X14, E905 and
//! W905). Two are of authorization, sekisho's (its DESIGN 4.6; [`gates`]): every operation a
//! context of a map opens has an action of a gate to guard it (X15, E907 and W907), and what a
//! workflow calls is what the gate that names it allows it (X16, E908, W908 and W909).

pub mod borders;
pub mod codes;
mod dates;
pub mod egress;
pub mod gates;
mod holds;
mod preconditions;
mod programs;
mod protos;
pub mod secrets;
mod transfers;

pub use preconditions::UndecidedCalls;
pub use programs::ProgramVerdicts;

use ritsu_base::json::Json;
use ritsu_base::naming::Tool;
use ritsu_base::text::Lang;
use ritsu_ports::{Answer, Crossing, Crossings, Finding, Flows};
use ritsu_project::{Joined, Project};
use std::path::Path;

/// Which of ritsu's checks a border is (DESIGN 7.2): the word `ritsu check --format json` writes
/// it with. A word is never changed, nor given to another check; a new check takes a new word,
/// after the others (the records are in this order). The marks of DESIGN (X2 to X16) are no
/// words of the output (DESIGN 4.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Check {
    /// A rule's precondition, at a call that gives it values (X2: E201, W201).
    Precondition,
    /// The day given to a rule's date input, against the range the rule declares (X3 (a): E202,
    /// W202).
    RuleDateInput,
    /// A rule's input whose range is a koyomi date (`range from koyomi`, X3 (b)): rulec's own
    /// check holds it, over the days koyomi hands it.
    RangeFromKoyomi,
    /// The amount given to a chobo transfer (X4: E203, W203).
    TransferAmount,
    /// The refusals a `do` or a `hold` can meet at the amounts given, against what the call
    /// handles (X4: E204, W204).
    TransferRefusals,
    /// A hold's expiry, against how long after it a call on it comes (X5: E206, W206).
    HoldExpiry,
    /// The day given to a koyomi date's input, against the input's range (X6: E205, W205).
    KoyomiDateInput,
    /// Where a call sends a secret value, against the map (X14: E905, W905).
    SecretDestination,
    /// An operation a context of a map opens, and the action of a gate that guards it (X15: E907,
    /// W907).
    OperationGuard,
    /// A call a workflow makes of an operation a gate guards, against what the gate allows it
    /// (X16: E908, W909).
    WorkflowPermission,
}

impl Check {
    /// Every check, in the order of the records.
    pub const ALL: [Check; 10] = [
        Check::Precondition,
        Check::RuleDateInput,
        Check::RangeFromKoyomi,
        Check::TransferAmount,
        Check::TransferRefusals,
        Check::HoldExpiry,
        Check::KoyomiDateInput,
        Check::SecretDestination,
        Check::OperationGuard,
        Check::WorkflowPermission,
    ];

    pub fn word(self) -> &'static str {
        match self {
            Check::Precondition => "precondition",
            Check::RuleDateInput => "rule_date_input",
            Check::RangeFromKoyomi => "range_from_koyomi",
            Check::TransferAmount => "transfer_amount",
            Check::TransferRefusals => "transfer_refusals",
            Check::HoldExpiry => "hold_expiry",
            Check::KoyomiDateInput => "koyomi_date_input",
            Check::SecretDestination => "secret_destination",
            Check::OperationGuard => "operation_guard",
            Check::WorkflowPermission => "workflow_permission",
        }
    }

    /// The check a verdict of kaname's is of, whose word kaname writes in its own JSON
    /// (`Crossing::check`, the same word).
    pub fn of(c: Crossing) -> Check {
        match c {
            Crossing::Precondition => Check::Precondition,
            Crossing::RuleDay => Check::RuleDateInput,
            Crossing::DateDay => Check::KoyomiDateInput,
            Crossing::Amount => Check::TransferAmount,
            Crossing::Refusals => Check::TransferRefusals,
        }
    }
}

/// What a border came to (P5): shown to hold, an example where it does not, or undecided.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Came {
    Held,
    Failed,
    Undecided,
}

impl Came {
    /// What an answer of a check comes to.
    pub fn of<E>(a: &Answer<E>) -> Came {
        match a {
            Answer::Holds => Came::Held,
            Answer::Fails(_) => Came::Failed,
            Answer::Undecided(_) => Came::Undecided,
        }
    }

    /// The word of the JSON, the key of its count: `held`, `failed`, `undecided`.
    pub fn word(self) -> &'static str {
        match self {
            Came::Held => "held",
            Came::Failed => "failed",
            Came::Undecided => "undecided",
        }
    }
}

/// One side of a border: the language, the file from the root (None when it is outside the root),
/// and the line, from 1 (None when the port does not say it: the side of a declaration, such as a
/// rule's precondition or a book's transfer).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Side {
    pub tool: Tool,
    pub file: Option<String>,
    pub line: Option<usize>,
}

impl Side {
    pub fn new(tool: Tool, file: Option<String>, line: Option<usize>) -> Side {
        Side { tool, file, line }
    }

    /// The side of a file of the project, by its path from the root.
    pub fn at(tool: Tool, rel: &str, line: usize) -> Side {
        Side { tool, file: Some(rel.to_string()), line: Some(line) }
    }

    /// The side of a file a language reached at `path` (as it reached it), written from the
    /// project's root.
    pub fn reached(project: &Project, tool: Tool, path: &Path, line: Option<usize>) -> Side {
        Side { tool, file: ritsu_base::paths::from_root(&project.root, path), line }
    }
}

/// One border a check of ritsu's held a language to another (DESIGN 7.1): which check, what it is
/// about, its place and its other side, what it came to, and the code ritsu said it with.
#[derive(Clone, Debug, PartialEq)]
pub struct Border {
    pub check: Check,
    /// What it is about, as ritsu's diagnostic of it names it: the precondition as the rule writes
    /// it, the input, the parameter, the operation.
    pub about: String,
    /// Its place: the side ritsu says its diagnostic at (the call of a flow, the place in a module
    /// of kaname, the line of a context that opens an operation, the line of a rule's `range from
    /// koyomi`).
    pub at: Side,
    /// The other side; None when there is none (an operation no action guards).
    pub to: Option<Side>,
    pub came: Came,
    /// The code of the diagnostic ritsu said of it, when it said one: always None for one that
    /// holds.
    pub code: Option<String>,
    /// What the check says of the border of its own, written after the rest, in this order.
    pub detail: Vec<(String, Json)>,
}

impl Border {
    pub fn new(check: Check, about: impl Into<String>, at: Side, to: Option<Side>, came: Came, code: Option<&str>) -> Border {
        Border { check, about: about.into(), at, to, came, code: code.map(str::to_string), detail: Vec::new() }
    }
}

/// What the checks of the borders between the languages came to (DESIGN 7.1, P5): each border,
/// shown to hold, with an example where it does not, or undecided. The counts are the records',
/// one a border.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Borders {
    records: Vec<Border>,
}

impl Borders {
    pub fn add(&mut self, b: Border) {
        self.records.push(b);
    }

    /// Each border, in the order [`Borders::sort`] puts them in once the checks are done.
    pub fn records(&self) -> &[Border] {
        &self.records
    }

    fn count(&self, came: Came) -> usize {
        self.records.iter().filter(|b| b.came == came).count()
    }

    pub fn held(&self) -> usize {
        self.count(Came::Held)
    }

    pub fn failed(&self) -> usize {
        self.count(Came::Failed)
    }

    pub fn undecided(&self) -> usize {
        self.count(Came::Undecided)
    }

    /// Every border checked, whatever it came to.
    pub fn checked(&self) -> usize {
        self.records.len()
    }

    /// The order of the records: by the check ([`Check::ALL`]), then the file and the line of the
    /// place; the borders of the same place in the order they were found, which the same project
    /// finds in the same order.
    pub fn sort(&mut self) {
        self.records.sort_by(|a, b| (a.check, &a.at.file, a.at.line).cmp(&(b.check, &b.at.file, b.at.line)));
    }
}

/// What ritsu says of a project itself.
#[derive(Clone, Debug, Default)]
pub struct Crossed {
    /// In ritsu's codes, in the order of the ledger, each as `ritsu check` prints it.
    pub findings: Vec<Finding>,
    pub borders: Borders,
}

/// Every check ritsu makes of a project across its languages, in `lang`. `joined` is what every
/// check of a border will read the languages through (PLAN E.4).
pub fn check(project: &Project, joined: &Joined, lang: Lang) -> Crossed {
    check_with(project, joined, &ProgramVerdicts::new(joined), lang)
}

/// [`check`], with the verdicts at the calls of the project's modules of kaname that kaname was
/// handed when the project's files were checked: each module is decided once in a run.
pub fn check_with(project: &Project, joined: &Joined, programs: &ProgramVerdicts, lang: Lang) -> Crossed {
    let mut findings = Vec::new();
    let mut borders = Borders::default();
    findings.extend(protos::unread(project, lang));
    findings.extend(preconditions::check(project, joined, lang, &mut borders));
    days_held(project, joined, &mut borders);
    let flows = flows(project, joined);
    findings.extend(dates::days_to_rules(project, &flows, joined, lang, &mut borders));
    findings.extend(transfers::check(project, &flows, joined, lang, &mut borders));
    findings.extend(dates::days_to_dates(project, &flows, joined, lang, &mut borders));
    findings.extend(holds::check(project, &flows, joined, lang, &mut borders));
    findings.extend(programs::check(project, programs, lang, &mut borders));
    findings.extend(secrets::keys(project, joined, lang));
    findings.extend(egress::check(project, joined, lang, &mut borders));
    findings.extend(gates::check(project, joined, lang, &mut borders));
    borders.sort();
    Crossed { findings, borders }
}

/// A flow of the project that passes dandori's check, with what it calls across the borders.
struct Flow<'a> {
    file: &'a ritsu_project::File,
    src: String,
    calls: Crossings,
}

/// Every flow of the project that passes dandori's check, read with the rules, the dates files and
/// the books through the ports. A flow that does not pass is dandori's to say.
fn flows<'a>(project: &'a Project, joined: &Joined) -> Vec<Flow<'a>> {
    let ports = joined.ports();
    project
        .of(Tool::Dandori)
        .into_iter()
        .filter_map(|f| {
            let disk = ritsu_base::paths::on_disk(&project.root, &f.rel);
            let calls = joined.dandori.crossings(&disk, &ports).ok()?;
            Some(Flow { file: f, src: ritsu_base::fs::read_to_string(&disk).unwrap_or_default(), calls })
        })
        .collect()
}

/// Japanese with a space after a piece that ends in code, as the Japanese of the suite writes it:
/// `` 入力 `received` `` and `には` make `` 入力 `received` には ``, `` `count` の結果 `` and `に` make
/// `` `count` の結果に ``.
fn then_ja(piece: &str, rest: &str) -> String {
    if piece.ends_with('`') { format!("{piece} {rest}") } else { format!("{piece}{rest}") }
}

/// A file a flow reaches, as the project names it: from the root, else as dandori reached it.
fn named(project: &Project, path: &std::path::Path) -> String {
    ritsu_base::paths::from_root(&project.root, path).unwrap_or_else(|| path.to_string_lossy().to_string())
}

/// X3 (b): each rule input of the project whose range is a koyomi date (`range from koyomi`,
/// rulec's §15.174) is a border rulec checked in its own check, over the days koyomi handed it
/// through the port; a rule that passes counts it as held. One that does not is rulec's to say
/// (E129, E130, or the table's own errors over the days), and is not counted here.
///
/// The border is at the line of the rule's `range from koyomi`, and its other side is the dates
/// file, as the rule names them to the index (rulec's references `range from koyomi`, of the same
/// date, taken in the order written).
fn days_held(project: &Project, joined: &Joined, borders: &mut Borders) {
    use ritsu_ports::Rules;
    for f in project.of(Tool::Rulec) {
        let disk = ritsu_base::paths::on_disk(&project.root, &f.rel);
        let Ok(facts) = joined.rulec.facts(&disk) else { continue };
        let mut refs: Vec<ritsu_ports::Reference> = match joined.index.references(Tool::Rulec, &project.root, &f.rel) {
            Some(Ok(r)) => r.into_iter().filter(|r| r.how == "range from koyomi").collect(),
            _ => Vec::new(),
        };
        for p in &facts.preconditions {
            let ritsu_ports::Precondition::Days { input, date, .. } = p else { continue };
            let found = refs.iter().position(|r| r.target.items.iter().any(|(k, v)| k == "date" && v == date)).map(|i| refs.remove(i));
            let (line, to) = match found {
                Some(r) => (Some(r.line), Some(r.target.path)),
                None => (None, None),
            };
            let at = Side::new(Tool::Rulec, Some(f.rel.clone()), line);
            borders.add(Border::new(Check::RangeFromKoyomi, input.clone(), at, Some(Side::new(Tool::Koyomi, to, None)), Came::Held, None));
        }
    }
}
