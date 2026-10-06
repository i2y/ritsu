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

/// What koyomi knows of one calendar file that passes its check (`Dates::calendar`): its name and
/// alias, the days its data covers (both ends in), the UTC offset its times are in (minutes east
/// of UTC) when it says one, and every day it closes within the data.
#[derive(Clone, Debug, PartialEq)]
pub struct CalendarFacts {
    pub name: String,
    pub alias: String,
    pub data: (Day, Day),
    pub offset: Option<i32>,
    pub closed: DaySet,
}

/// The fewest and the most days from the date input to a date over the whole range of the
/// inputs (`Dates::span`), each with the date input's day that first comes to it, when the answer
/// says one.
#[derive(Clone, Debug, PartialEq)]
pub struct DaySpan {
    pub fewest: i64,
    pub most: i64,
    pub fewest_at: Option<Day>,
    pub most_at: Option<Day>,
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

    /// The fewest and the most days from the date input to `date`, as `days` answers, with the date
    /// input's day that first comes to each (DESIGN 7.7, X5: the example of a hold that has always
    /// expired). The default has no days to give with them.
    fn span(&self, file: &Path, date: &str) -> Result<Found<DaySpan>, Vec<Said>> {
        Ok(match self.days(file, date)? {
            Found::Value((fewest, most)) => Found::Value(DaySpan { fewest, most, fewest_at: None, most_at: None }),
            Found::Undecided(why) => Found::Undecided(why),
        })
    }

    /// The first input of the walk over the whole range at which `date` comes to `day` (each input
    /// by name: a date as its day count, an integer as itself), to show where a day of the date
    /// comes from (DESIGN 7.1: an example names koyomi's input). None when no input does; the
    /// default walks nothing and says none.
    fn input_for(&self, file: &Path, date: &str, day: Day) -> Result<Option<Vec<(String, i64)>>, Vec<Said>> {
        let _ = (file, date, day);
        Ok(None)
    }

    /// Every date for one input (each input by name: a date as its day count, an integer as
    /// itself), in the order written.
    fn eval(&self, file: &Path, inputs: &[(String, i64)]) -> Result<Vec<(String, DateValue)>, Vec<Said>>;

    /// What koyomi knows of a calendar file (sekisho's DESIGN 3.3: `today is open in <calendar>`),
    /// when it passes check; else what check says. The default reads no calendar.
    fn calendar(&self, file: &Path) -> Result<CalendarFacts, Vec<Said>> {
        Err(unanswered(file, ritsu_base::tr!("この口はカレンダーを読みません", "this port reads no calendar")))
    }

    /// The page for people, as `koyomi doc` draws it — Markdown, or with `html` the one HTML file —
    /// in `lang`, naming the file `shown`, for a dates file or a calendar, as `Rules::doc` gives
    /// rulec's. The default draws no page.
    fn doc(&self, file: &Path, shown: &str, html: bool, lang: ritsu_base::text::Lang) -> Result<String, Vec<Said>> {
        let _ = (shown, html, lang);
        Err(unanswered(file, ritsu_base::tr!("この口はページを作りません", "this port draws no page")))
    }

    /// Whether koyomi is joined at all: false for the port the binary of a receiving language's own
    /// crate holds, which reads no dates file (DESIGN 2.3), so that the language can say so once,
    /// as with rules (`Rules::joined`).
    fn joined(&self) -> bool {
        true
    }
}

/// What a port that does not answer a question says of the file it was asked about.
fn unanswered(file: &Path, message: ritsu_base::text::Text) -> Vec<Said> {
    vec![Said { code: String::new(), file: file.display().to_string(), line: None, message }]
}

/// A day number as a date, `YYYY-MM-DD` (days since 1970-01-01, as rulec and koyomi count them).
pub fn day_text(d: Day) -> String {
    // Howard Hinnant's civil_from_days
    let z = d + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let dd = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{m:02}-{dd:02}", if m <= 2 { y + 1 } else { y })
}
