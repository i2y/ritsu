//! The words of the language, all of them, in one table (DESIGN 1.2).
//!
//! Every keyword has one English spelling and no synonym. The lexer cuts every word out as a
//! name; the parser matches names against the constants here by position. The check that a
//! name or an alias is not a keyword (E009) reads [`RESERVED`], and the table in the reference
//! reads [`TABLE`], so a word added here reaches both.

// Words that start a line.
pub const CALENDAR: &str = "calendar";
pub const DATES: &str = "dates";
pub const DESCRIPTION: &str = "description";
pub const OFFSET: &str = "offset";
pub const SOURCE: &str = "source";
pub const CLOSED: &str = "closed";
pub const OPEN: &str = "open";
pub const USE: &str = "use";
pub const INPUTS: &str = "inputs";
pub const DATE: &str = "date";
pub const CLAIMS: &str = "claims";
pub const EXAMPLES: &str = "examples";

// A source line.
pub const FILE: &str = "file";
pub const URL: &str = "url";
pub const LAW: &str = "law";
pub const ASOF: &str = "asof";
/// Written with the digest after it, `sha256:cec37a743c96995c`; the lexer reads the two as one.
pub const SHA256: &str = "sha256";

// The lines under a source.
pub const FORMAT: &str = "format";
pub const COVERS: &str = "covers";
pub const CSV: &str = "csv";
pub const GOVUK: &str = "govuk";
pub const UTF8: &str = "utf8";
pub const SHIFT_JIS: &str = "shift_jis";
pub const LISTED: &str = "listed";
pub const YEARS: &str = "years";

// A calendar.
pub const WEEKLY: &str = "weekly";
pub const EVERY: &str = "every";
/// Monday first, the order `date.rs` numbers the days of the week in.
pub const WEEKDAYS: [&str; 7] = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

// Types and ranges.
pub const INT: &str = "int";
pub const RANGE: &str = "range";

// The operations.
pub const DAY: &str = "day";
pub const DAYS: &str = "days";
pub const BUSINESS: &str = "business";
pub const MONTH: &str = "month";
pub const MONTHS: &str = "months";
pub const YEAR: &str = "year";
pub const OF: &str = "of";
pub const START: &str = "start";
pub const END: &str = "end";
pub const CLOSE: &str = "close";
pub const ROLL: &str = "roll";
pub const FOLLOWING: &str = "following";
pub const PRECEDING: &str = "preceding";
pub const MODIFIED: &str = "modified";
pub const IF: &str = "if";
pub const ELSE: &str = "else";
pub const AT: &str = "at";

// What to do with a day the month does not have (DESIGN 1.7).
pub const END_OF_MONTH: &str = "end_of_month";
pub const START_OF_NEXT_MONTH: &str = "start_of_next_month";
pub const REJECT: &str = "reject";

// Claims.
pub const IS: &str = "is";
pub const MONOTONIC: &str = "monotonic";

/// The words, by where they are written, as DESIGN 1.2 lists them.
pub const TABLE: &[(&str, &[&str])] = &[
    ("line", &[CALENDAR, DATES, DESCRIPTION, OFFSET, SOURCE, CLOSED, OPEN, USE, INPUTS, DATE, CLAIMS, EXAMPLES]),
    ("source", &[FILE, URL, LAW, ASOF, "sha256:"]),
    ("under a source", &[FORMAT, COVERS, CSV, GOVUK, UTF8, SHIFT_JIS, LISTED, YEARS]),
    ("calendar", &[WEEKLY, EVERY, "mon", "tue", "wed", "thu", "fri", "sat", "sun"]),
    ("type and range", &[DATE, INT, RANGE]),
    (
        "operation",
        &[
            "+", "-", DAY, DAYS, BUSINESS, MONTH, MONTHS, YEAR, YEARS, OF, START, END, CLOSE, ROLL, FOLLOWING,
            PRECEDING, MODIFIED, IF, CLOSED, ELSE, AT,
        ],
    ),
    ("a day the month does not have", &[END_OF_MONTH, START_OF_NEXT_MONTH, REJECT]),
    ("claim", &[IS, OPEN, MONOTONIC, "=", "<", "<=", ">", ">="]),
];

/// The words a name or an alias cannot be (E009): every word of [`TABLE`].
pub const RESERVED: &[&str] = &[
    CALENDAR, DATES, DESCRIPTION, OFFSET, SOURCE, CLOSED, OPEN, USE, INPUTS, DATE, CLAIMS, EXAMPLES, FILE, URL, LAW,
    ASOF, SHA256, FORMAT, COVERS, CSV, GOVUK, UTF8, SHIFT_JIS, LISTED, YEARS, WEEKLY, EVERY, "mon", "tue", "wed",
    "thu", "fri", "sat", "sun", INT, RANGE, DAY, DAYS, BUSINESS, MONTH, MONTHS, YEAR, OF, START, END, CLOSE, ROLL,
    FOLLOWING, PRECEDING, MODIFIED, IF, ELSE, AT, END_OF_MONTH, START_OF_NEXT_MONTH, REJECT, IS, MONOTONIC,
];

pub fn is_reserved(w: &str) -> bool {
    RESERVED.contains(&w)
}

/// The day of the week a word names, Monday as 0.
pub fn weekday(w: &str) -> Option<u32> {
    WEEKDAYS.iter().position(|d| *d == w).map(|i| i as u32)
}
