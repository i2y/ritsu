//! The days a date input takes when its range is a date of a koyomi file (`range from koyomi
//! "<file>" date <name>`, §15.174; ritsu's DESIGN 7.5 (b)). rulec has no arithmetic on days
//! (E048), and does not compute a payment day; koyomi does, on every input of its range, so the
//! set of days a date comes to is known exactly. rulec reads that set through ritsu's port of
//! dates (`ritsu_ports::Dates`) and checks the tables over it: completeness, overlaps and the
//! rows nothing reaches are all about the days in the set, and the generated code refuses any
//! other day at its door.
//!
//! The port is handed over for a run, on the thread that checks (`with`): `ritsu rulec`,
//! `ritsu check` and the languages that read rules through rulec's engine join koyomi; the
//! binary of rulec's own crate does not (ritsu's DESIGN 2.3), and a rule that takes its range
//! from koyomi is refused there (E129) rather than checked over the whole of its days.

use ritsu_base::text::Text;
use ritsu_ports::{Dates, Found, Said};
use std::cell::RefCell;
use std::path::Path;
use std::sync::Arc;

/// The port of dates, as a run hands it over.
pub type Port = Arc<dyn Dates + Send + Sync>;

thread_local! {
    static PORT: RefCell<Option<Port>> = const { RefCell::new(None) };
    /// The days `checked_over` holds one input to, in place of what the rule declares.
    static OVER: RefCell<Option<(String, Vec<i64>)>> = const { RefCell::new(None) };
}

/// Puts the old value back however the closure ends.
struct Restore<T: 'static> {
    key: &'static std::thread::LocalKey<RefCell<Option<T>>>,
    old: Option<T>,
}

impl<T: 'static> Drop for Restore<T> {
    fn drop(&mut self) {
        let old = self.old.take();
        self.key.with(|c| *c.borrow_mut() = old);
    }
}

/// Runs `f` with `port` as the dates a rule's range can be read from on this thread.
pub fn with<R>(port: Option<Port>, f: impl FnOnce() -> R) -> R {
    let old = PORT.with(|c| std::mem::replace(&mut *c.borrow_mut(), port));
    let _restore = Restore { key: &PORT, old };
    f()
}

/// Runs `f` with the date input `input` taking only `days` (ritsu's `Rules::checked_over`).
pub fn over<R>(input: &str, days: Vec<i64>, f: impl FnOnce() -> R) -> R {
    let old = OVER.with(|c| std::mem::replace(&mut *c.borrow_mut(), Some((input.to_string(), days))));
    let _restore = Restore { key: &OVER, old };
    f()
}

/// The days `checked_over` holds `input` to, if it holds this one.
pub fn held_over(input: &str) -> Option<Vec<i64>> {
    OVER.with(|c| c.borrow().as_ref().filter(|(n, _)| n == input).map(|(_, d)| d.clone()))
}

/// Whether koyomi is joined on this thread.
pub fn joined() -> bool {
    PORT.with(|c| c.borrow().is_some())
}

/// The port joined on this thread, to hand to another thread that checks for it.
pub fn port() -> Option<Port> {
    PORT.with(|c| c.borrow().clone())
}

/// What reading the days of a koyomi file came to.
pub enum Read {
    /// The days, in order, as day numbers (days since 1970-01-01, rulec's ordinal of a date),
    /// with the SHA-256 of the koyomi file they were computed from.
    Days { sha256: String, days: Vec<i64> },
    /// No koyomi is joined: the binary of rulec's own crate.
    NotJoined,
    /// koyomi cannot answer for the file (it is not there, does not pass koyomi's check, or has
    /// no such date): what koyomi says.
    Refused(Vec<Said>),
    /// koyomi does not compute the set: why.
    Undecided(Text),
}

/// The days the date `date` of the koyomi file at `file` comes to over its whole range.
pub fn read(file: &Path, date: &str) -> Read {
    let Some(port) = port() else { return Read::NotJoined };
    let facts = match port.facts(file) {
        Ok(f) => f,
        Err(said) => return Read::Refused(said),
    };
    match port.values(file, date) {
        Ok(Found::Value(set)) => Read::Days { sha256: facts.sha256, days: set.into_iter().collect() },
        Ok(Found::Undecided(why)) => Read::Undecided(why),
        Err(said) => Read::Refused(said),
    }
}
