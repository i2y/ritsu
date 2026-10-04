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
//! ritsu's one reader (E101); and the borders between the languages (DESIGN 7.2). Of those, X2
//! (a rule's preconditions where a workflow calls it, E201 and W201) reaches a project now; X3 (a),
//! X4 and X6 ([`borders`]) are decided over what koyomi, chobo and rulec hand over, and reach a
//! project once dandori calls koyomi and chobo (PLAN E.5). X1, the units at a border, is the
//! languages' own check (dandori's E003, rulec's E065), and X3 (b) is rulec's (`range from
//! koyomi`, rulec's §15.174).

pub mod borders;
pub mod codes;
mod preconditions;
mod protos;

use ritsu_base::text::Lang;
use ritsu_ports::Finding;
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
    Crossed { findings, borders }
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
