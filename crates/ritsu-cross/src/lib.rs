//! ritsu's checks across its languages (DESIGN 7), and the ledger of their codes ([`codes`]):
//! what `ritsu check` says itself, after every language's own check has said its part. Each code
//! is ritsu's, apart from the languages' ledgers (whose numbers overlap); `ritsu explain` looks
//! one up, and the headline of every finding says `ritsu` (`error[ritsu E101]`).
//!
//! A check here reads the languages through the ports only (DESIGN 3.1): ritsu-project joins them
//! and hands over the project and its index. What each check comes to is one of P5's three — shown
//! to hold, an example where it does not, or undecided — and [`Borders`] counts them.
//!
//! In the first part of stage E there is one check: the `.proto` files of a project, which no
//! language checks on its own, read with ritsu's one reader ([`codes`]'s E101). The checks of the
//! borders between the languages (DESIGN 7.2, X1–X7) come in its second part (PLAN E.4).

pub mod codes;
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
pub fn check(project: &Project, _joined: &Joined, lang: Lang) -> Crossed {
    let mut findings = Vec::new();
    findings.extend(protos::unread(project, lang));
    Crossed { findings, borders: Borders::default() }
}
