//! ritsu's checks across its languages (DESIGN 7), and the ledger of their codes ([`codes`]):
//! what `ritsu check` says itself, after every language's own check has said its part. Each code
//! is ritsu's, apart from the languages' ledgers (whose numbers overlap); `ritsu explain` looks
//! one up, and the headline of every finding says `ritsu` (`error[ritsu E101]`).
//!
//! A check here reads the languages through the ports only (DESIGN 3.1): ritsu-project joins them
//! and hands over the project and its index. What each check comes to is one of P5's three — shown
//! to hold, an example where it does not, or undecided — and [`Borders`] counts them.
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

pub mod borders;
pub mod codes;
mod dates;
mod holds;
mod preconditions;
mod protos;
mod transfers;

use ritsu_base::naming::Tool;
use ritsu_base::text::Lang;
use ritsu_ports::{Crossings, Finding, Flows};
use ritsu_project::{Joined, Project};

/// What the checks of the borders between the languages came to (DESIGN 7.1, P5): each border
/// shown to hold, with an example where it does not, or undecided.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Borders {
    pub held: usize,
    pub failed: usize,
    pub undecided: usize,
}

impl Borders {
    /// Every border checked, whatever it came to.
    pub fn checked(&self) -> usize {
        self.held + self.failed + self.undecided
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
fn days_held(project: &Project, joined: &Joined, borders: &mut Borders) {
    use ritsu_ports::Rules;
    for f in project.of(ritsu_base::naming::Tool::Rulec) {
        let disk = ritsu_base::paths::on_disk(&project.root, &f.rel);
        if let Ok(facts) = joined.rulec.facts(&disk) {
            borders.held += facts.preconditions.iter().filter(|p| matches!(p, ritsu_ports::Precondition::Days { .. })).count();
        }
    }
}
