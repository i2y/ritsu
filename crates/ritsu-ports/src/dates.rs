//! The port of dates (DESIGN 3.2, `Dates`): what koyomi knows of a dates file, and the questions
//! a rule or a workflow asks of it. koyomi computes a function on every input of its range, so
//! what a function can come to is known exactly, not estimated.

use crate::{Found, Said};
use std::collections::BTreeSet;
use std::path::Path;

/// A calendar day, as the number of days since 1970-01-01 (the day count koyomi and rulec both
/// keep: the day after the last of a month is one more than it).
pub type Day = i64;

/// A set of days.
pub type DaySet = BTreeSet<Day>;

/// What koyomi knows of one dates file that passes its check.
#[derive(Clone, Debug, PartialEq)]
pub struct DateFacts {
    /// The file's name and ASCII alias (`dates 支払日(payment)`).
    pub name: String,
    pub alias: String,
    pub version: String,
    pub sha256: String,
    pub inputs: Vec<DateInput>,
    /// The dates the file computes, in the order written.
    pub functions: Vec<DateFunction>,
    pub calendar: Option<DateCalendar>,
    /// The claims, each its name and text.
    pub claims: Vec<(String, String)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateKind {
    Date,
    Int,
}

/// An input: its name, alias, kind, and its range, both ends in (days for a date).
#[derive(Clone, Debug, PartialEq)]
pub struct DateInput {
    pub name: String,
    pub alias: String,
    pub kind: DateKind,
    pub min: i64,
    pub max: i64,
}

/// A date the file computes: its name and alias, the inputs it reads, and the time of day the
/// date turns into (`at 09:00`, or the end of the day), when it says one.
#[derive(Clone, Debug, PartialEq)]
pub struct DateFunction {
    pub name: String,
    pub alias: String,
    pub params: Vec<String>,
    /// Minutes after midnight; 1440 for the end of the day.
    pub at: Option<u32>,
}

/// The calendar a file uses: its name, the days its data covers, and the UTC offset its times are
/// in (minutes east of UTC), when it says one.
#[derive(Clone, Debug, PartialEq)]
pub struct DateCalendar {
    pub name: String,
    pub data: (Day, Day),
    pub offset: Option<i32>,
}

/// What a date comes to for one input.
#[derive(Clone, Debug, PartialEq)]
pub enum DateValue {
    Day(Day),
    /// The computation stopped (a day that does not exist, a day the calendar does not cover):
    /// what koyomi says of it.
    Stopped(ritsu_base::text::Text),
}

/// What koyomi answers for a dates file. `file` is the file, as the caller reaches it.
pub trait Dates {
    /// What koyomi knows of the file, when it passes check; else what check says.
    fn facts(&self, file: &Path) -> Result<DateFacts, Vec<Said>>;

    /// Every day `date` comes to over the whole range of the inputs (DESIGN 7.5): the set, or
    /// why it is not computed (more inputs than the check walks, an input where the date stops).
    fn values(&self, file: &Path, date: &str) -> Result<Found<DaySet>, Vec<Said>>;

    /// The fewest and the most days from the date input to `date`, over the whole range
    /// (DESIGN 7.7).
    fn days(&self, file: &Path, date: &str) -> Result<Found<(i64, i64)>, Vec<Said>>;

    /// Every date for one input (each input by name: a date as its day count, an integer as
    /// itself), in the order written.
    fn eval(&self, file: &Path, inputs: &[(String, i64)]) -> Result<Vec<(String, DateValue)>, Vec<Said>>;
}
