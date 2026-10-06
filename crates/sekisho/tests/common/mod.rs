//! What the tests share: the languages joined as `ritsu sekisho` joins them, and a file checked
//! with them.

#![allow(dead_code)]

use sekisho::check::{Options, Outcome};
use sekisho::suite::Suite;
use std::rc::Rc;
use std::sync::Arc;

/// The ports as ritsu-project's `Joined::sekisho` makes them: rulec, koyomi, chobo and dandori,
/// each through its own engine (rulec reads the days of a koyomi date through koyomi's).
pub fn ports() -> ritsu_ports::GatePorts {
    let dandori = Rc::new(dandori::ports::Engine);
    ritsu_ports::GatePorts {
        rules: Rc::new(rulec::ports::Engine::with_dates(Arc::new(koyomi::ports::Engine))),
        dates: Rc::new(koyomi::ports::Engine),
        books: Rc::new(chobo::ports::Engine),
        flows: dandori.clone(),
        items: dandori,
    }
}

/// Every language joined, as `ritsu sekisho` hands them over.
pub fn joined() -> Suite {
    Suite::from(ports())
}

/// The file at `path`, checked with every language joined.
pub fn check(path: &str) -> Outcome {
    sekisho::check::check_file(path, &joined(), &Options::default()).unwrap()
}

/// What `sekisho check` prints for it.
pub fn shown(o: &Outcome, lang: ritsu_base::text::Lang) -> String {
    sekisho::check::render(o, lang)
}
