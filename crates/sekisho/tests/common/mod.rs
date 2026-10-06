//! What the tests share: the languages joined as `ritsu sekisho` joins them, and a file checked
//! with them.

#![allow(dead_code)]

use sekisho::check::{Options, Outcome};
use sekisho::suite::Suite;
use std::path::{Path, PathBuf};
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

/// The root the references of a file are written from (DESIGN 2.6), as the tests give it: an
/// example is a project of its own, run with its directory as `--root` (as yuen's and sakai's
/// examples are); a gate under `tests/` reads the example's documents, so its root is the crate's
/// directory; any other file (a copy in a temporary directory) finds its root as the command does.
pub fn root_of(path: &str) -> Option<PathBuf> {
    if let Some((dir, _)) = path.strip_prefix("examples/").and_then(|p| p.split_once('/')) {
        return Some(Path::new("examples").join(dir));
    }
    path.starts_with("tests/").then(|| PathBuf::from("."))
}

/// The options of a check of the file at `path`, its root as [`root_of`] gives it.
pub fn options(path: &str) -> Options {
    Options { root: root_of(path), ..Options::default() }
}

/// The file at `path`, checked with every language joined.
pub fn check(path: &str) -> Outcome {
    sekisho::check::check_file(path, &joined(), &options(path)).unwrap()
}

/// What `sekisho check` prints for it.
pub fn shown(o: &Outcome, lang: ritsu_base::text::Lang) -> String {
    sekisho::check::render(o, lang)
}
