//! The languages sekisho reads through ritsu's ports (DESIGN 8.1): rulec's rules (`Rules`),
//! koyomi's dates files and calendars (`Dates`), chobo's books (`Books`), and dandori's flows
//! (`Flows`: whether a flow passes dandori's check, and what it calls).
//!
//! sekisho holds none of those languages: they are handed to it. The binary of sekisho's own crate
//! is handed none ([`Suite::default`]), and says so once, at the first line that reads one (E209);
//! `ritsu sekisho` hands it every one, joined once for the whole run (ritsu's DESIGN 2.3, 3.3).
//! What sekisho reads itself is not here: another `.gate` (`use gate`), and the contracts
//! (OpenAPI and AsyncAPI documents, `.proto` files), whose readers are in ritsu's base layer.

use crate::ast::UseKind;
use ritsu_ports::{Books, Dates, Flows, Rules};
use std::rc::Rc;

/// The ports of the languages sekisho reads, as whoever runs sekisho joins them. A port that is
/// None is a language that is not joined.
#[derive(Clone, Default)]
pub struct Suite {
    pub rules: Option<Rc<dyn Rules>>,
    pub dates: Option<Rc<dyn Dates>>,
    pub books: Option<Rc<dyn Books>>,
    /// dandori's flows: a workflow's `from` (E208), and later what the flow calls (X16).
    pub flows: Option<Rc<dyn Flows>>,
}

/// A language sekisho reads through a port.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    Rulec,
    Koyomi,
    Chobo,
    Dandori,
}

impl Language {
    pub fn word(self) -> &'static str {
        match self {
            Language::Rulec => "rulec",
            Language::Koyomi => "koyomi",
            Language::Chobo => "chobo",
            Language::Dandori => "dandori",
        }
    }

    /// The language a `use` of `kind` reads through a port; None for what sekisho reads itself.
    pub fn of_use(kind: UseKind) -> Option<Language> {
        match kind {
            UseKind::Rule => Some(Language::Rulec),
            UseKind::Dates | UseKind::Calendar => Some(Language::Koyomi),
            UseKind::Book => Some(Language::Chobo),
            UseKind::Openapi | UseKind::Proto | UseKind::Asyncapi | UseKind::Gate => None,
        }
    }
}

/// The ports `ritsu sekisho` and `ritsu check` hand over (ritsu-project's `Joined::sekisho`), every
/// language joined. Its `items` (what a flow holds) is not read: whether a flow passes dandori's
/// check is asked through `Flows`.
impl From<ritsu_ports::GatePorts> for Suite {
    fn from(g: ritsu_ports::GatePorts) -> Suite {
        Suite { rules: Some(g.rules), dates: Some(g.dates), books: Some(g.books), flows: Some(g.flows) }
    }
}

impl Suite {
    /// Whether the language is joined.
    pub fn joins(&self, l: Language) -> bool {
        match l {
            Language::Rulec => self.rules.as_ref().is_some_and(|r| r.joined()),
            Language::Koyomi => self.dates.as_ref().is_some_and(|d| d.joined()),
            Language::Chobo => self.books.as_ref().is_some_and(|b| b.joined()),
            Language::Dandori => self.flows.is_some(),
        }
    }

    /// Whether what a `use` of `kind` names can be read here.
    pub fn reads(&self, kind: UseKind) -> bool {
        Language::of_use(kind).is_none_or(|l| self.joins(l))
    }
}
