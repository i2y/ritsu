//! The languages kaname calls, read through ritsu's ports (DESIGN 0.2 P6, 6): rulec's rules
//! (`Rules`), koyomi's dates files (`Dates`), chobo's books (`Books`).
//!
//! kaname holds none of those languages: they are handed to it. The binary of kaname's own crate is
//! handed none ([`Suite::default`]), and says so once, at the first line that reads one (E202);
//! `ritsu kaname` and `ritsu check` hand it every one, joined once for the whole run (ritsu's DESIGN
//! 2.3, 3.3). What kaname reads itself is another `.kn` (`use module`).

use crate::ast::UseKind;
use ritsu_ports::{Books, Dates, Rules, Verdicts};
use std::rc::Rc;

/// The ports of the languages kaname calls, as whoever runs kaname joins them. A port that is
/// None is a language that is not joined. `verdicts` is what the checks across the borders came to
/// at a module's calls (ritsu-cross's, DESIGN 14.5): with none, a place they decide waits for them
/// (DESIGN 4.3).
#[derive(Clone, Default)]
pub struct Suite {
    pub rules: Option<Rc<dyn Rules>>,
    pub dates: Option<Rc<dyn Dates>>,
    pub books: Option<Rc<dyn Books>>,
    pub verdicts: Option<Rc<dyn Verdicts>>,
}

/// The ports `ritsu kaname` and `ritsu check` hand over (ritsu-project's `Joined::ports`), every
/// language joined.
impl From<ritsu_ports::Ports> for Suite {
    fn from(p: ritsu_ports::Ports) -> Suite {
        Suite { rules: Some(p.rules), dates: Some(p.dates), books: Some(p.books), verdicts: None }
    }
}

impl Suite {
    /// The same languages, with what the checks across the borders came to.
    pub fn with_verdicts(self, verdicts: Rc<dyn Verdicts>) -> Suite {
        Suite { verdicts: Some(verdicts), ..self }
    }

    /// The language a `use` of `kind` reads through a port; None for another module, which kaname
    /// reads itself.
    pub fn tool(kind: UseKind) -> Option<&'static str> {
        match kind {
            UseKind::Rule => Some("rulec"),
            UseKind::Dates => Some("koyomi"),
            UseKind::Book => Some("chobo"),
            UseKind::Module => None,
        }
    }

    /// Whether what a `use` of `kind` reads can be read here.
    pub fn reads(&self, kind: UseKind) -> bool {
        match kind {
            UseKind::Rule => self.rules.as_ref().is_some_and(|r| r.joined()),
            UseKind::Dates => self.dates.as_ref().is_some_and(|d| d.joined()),
            UseKind::Book => self.books.as_ref().is_some_and(|b| b.joined()),
            UseKind::Module => true,
        }
    }
}
