//! The languages a `.ringi` reads and is lowered to, through ritsu's ports (DESIGN 9.1): rulec's
//! rules (`Rules`), koyomi's calendars and dates files (`Dates`), chobo's books (`Books`),
//! dandori's flows (`Flows`), sekisho's gates (`Gates`) and kaname's modules (`Programs`).
//!
//! ringi holds none of those languages: they are handed to it. The binary of ringi's own crate is
//! handed none ([`Suite::default`]), and says so for a file that needs one, once for each language
//! at the first line that needs it (E008, DESIGN 16.3); `ritsu ringi` and `ritsu check` hand it
//! every one (ritsu's DESIGN 2.3, 3.3). What ringi reads itself is another `.ringi`: an
//! organization, masters, a shared route.

use ritsu_ports::{Books, Dates, Flows, Gates, Programs, Rules};
use std::rc::Rc;

/// The ports of the languages ringi reads and is lowered to, as whoever runs ringi joins them. A
/// port that is None is a language that is not joined.
#[derive(Clone, Default)]
pub struct Suite {
    pub rules: Option<Rc<dyn Rules>>,
    pub dates: Option<Rc<dyn Dates>>,
    pub books: Option<Rc<dyn Books>>,
    pub flows: Option<Rc<dyn Flows>>,
    pub gates: Option<Rc<dyn Gates>>,
    pub programs: Option<Rc<dyn Programs>>,
}

impl Suite {
    /// Whether the language of that word is joined here: `rulec`, `koyomi`, `chobo`, `dandori`,
    /// `sekisho` or `kaname`.
    pub fn reads(&self, tool: &str) -> bool {
        match tool {
            "rulec" => self.rules.as_ref().is_some_and(|r| r.joined()),
            "koyomi" => self.dates.as_ref().is_some_and(|d| d.joined()),
            "chobo" => self.books.as_ref().is_some_and(|b| b.joined()),
            "dandori" => self.flows.is_some(),
            "sekisho" => self.gates.as_ref().is_some_and(|g| g.joined()),
            "kaname" => self.programs.as_ref().is_some_and(|p| p.joined()),
            _ => true,
        }
    }
}
