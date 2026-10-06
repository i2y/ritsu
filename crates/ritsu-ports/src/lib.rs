//! The ports of ritsu (DESIGN 3.2): what the languages hand each other across a border, as types,
//! and the questions one language asks another, as traits. The language that knows a thing
//! implements the port it is asked through (`rulec` implements [`Rules`], `koyomi` [`Dates`]);
//! the language that asks holds a `&dyn` of the trait and knows nothing of the other crate.
//! What is handed over carries its units ([`ritsu_units::Unit`]), so a fact one language checked
//! reaches the next one as the fact, not as a string in a JSON document.
//!
//! - [`Rules`]: a rule's inputs and outputs with their units and ranges, its enums, its state
//!   machine, its preconditions, its Connect service, how its generated code is called, the page
//!   `rulec doc` draws, the questions a workflow asks of it, and the values an output comes to
//!   when the inputs are held to ranges (what a gate counts).
//! - [`Dates`]: a dates file's functions, inputs, calendar and claims, the values a function
//!   takes over its whole range, the days from an input to a value, a calendar's closed days, and
//!   the page `koyomi doc` draws.
//! - [`Books`]: a book's units, accounts and transfers, the life of a hold, and a [`Ledger`] to
//!   run operations on.
//! - [`Claims`]: a geas spec's claims, the record `geas map` keeps of the lines they ran, and
//!   what a diff comes to for them (geas's `affected`).
//! - [`Items`] and [`References`]: what a file holds and what it names outside itself, by the
//!   naming of DESIGN 6.2; every language gives them, and an [`Index`] keeps them for a run, so
//!   that a naming is looked up in one place.
//! - [`Sources`]: the sources a rule or a calendar copies and pins, which yuen borrows and holds
//!   its own copies to.
//! - [`Flows`]: the calls a workflow makes to rules, koyomi dates and chobo transfers, with what
//!   dandori knows of each value it gives (its range, the places it can come from), and how long
//!   each hold can be held before a call its expiry can refuse, for the checks across the borders;
//!   and [`Undecided`], the preconditions those checks could not decide, which the code dandori
//!   writes checks when the workflow runs.
//!   [`Flows::sends`] adds the calls that give a value the contracts mark secret to another
//!   file of the project ([`Send`]), for the check of where secrets go (X14), and
//!   [`Flows::operation_calls`] the calls of an operation of a contract ([`OperationCall`]), for
//!   the check of what a workflow is allowed (X16).
//! - [`Maps`]: sakai's maps — their contexts and relationships ([`MapFacts`]), the context a file
//!   belongs to, and the operations each context opens ([`PublishedOperation`]) — as the checks
//!   across the borders read them (X14, X15).
//! - [`Checked`]: what a language's own `check` prints for a unit it checks, a diagnostic at a
//!   time, the text and the JSON, as `ritsu check` prints it again (DESIGN 8.3).
//! - [`Gates`]: a `.gate`'s actions with the operations they guard, its workflows, policies and
//!   expectations, and how far someone is allowed an action, for the checks across the borders;
//!   and [`GatePorts`], the ports sekisho reads a `.gate` with.
//!
//! Every check across a border answers with an [`Answer`]: shown to hold, an example where it
//! does not, or why it cannot be decided (P5). A question that asks for a value answers with a
//! [`Found`]: the value, or why it cannot be had. Nothing undecided passes as decided.

mod books;
mod check;
mod claims;
mod dates;
mod flows;
mod gates;
mod index;
mod maps;
mod rules;
mod sources;

pub use books::{Account, Balance, BookCall, BookClient, BookFacts, BookOutcome, BookUnit, Bound, Books, ClientTransfer, Expiry, Ledger, Move, MoveAmount, MoveRef, Transfer, TransferParam};
pub use check::{Checked, Finding, Part, Verdict};
pub use claims::{Affected, Claim, Claims, MapRecord, RecordClaim, RecordFile, RecordRan, Touched, TouchedLines, Untouched};
pub use sources::{Source, SourceKind, Sources};
pub use dates::{day_text, DateCalendar, DateFacts, DateFunction, DateInput, DateKind, DateValue, Dates, Day, DaySet, DaySpan};
pub use dates::CalendarFacts;
pub use gates::{Allowance, Asker, GateAction, GateFacts, GatePolicy, GatePorts, GateWorkflow, Gates};
pub use index::{Index, Item, Items, Lookup, Reference, References};
pub use flows::{
    seconds_text, Amount, CallArg, Crossings, DateCall, Destination, Flows, HoldSpan, OperationCall, Origin, Ports, RuleCall, Secret, Send,
    TransferCall, Undecided, UndecidedPrecondition,
};
pub use maps::{MapFacts, MapRelationship, Maps, PublishedOperation};
pub use rules::{Axis, Call, CallEnum, Column, ColumnType, Connect, EnumValue, Machine, MachineRow, OutputValues, Param, Precondition, RuleEnum, RuleError, RuleFacts, Rules, Value, Values, WireEnum, WireField};

use ritsu_base::text::Text;

/// What a check across a border comes to (DESIGN P5).
#[derive(Clone, Debug, PartialEq)]
pub enum Answer<E> {
    /// It holds: shown, for every value it is about.
    Holds,
    /// It does not: a value where it fails, and what leads to it.
    Fails(E),
    /// It cannot be decided. Why, in both languages.
    Undecided(Text),
}

/// What a question that asks for a value comes to: the value, or why it cannot be had.
#[derive(Clone, Debug, PartialEq)]
pub enum Found<T> {
    Value(T),
    Undecided(Text),
}

/// One thing a language says is wrong when it cannot answer for a file: its code, where it is,
/// and what it says, as its own diagnostics say it. A receiving language names these where it
/// refers to the file (DESIGN 6.1).
#[derive(Clone, Debug, PartialEq)]
pub struct Said {
    /// The language's own code (`E101`); empty when the file could not be read at all.
    pub code: String,
    /// The file, as the caller named it.
    pub file: String,
    pub line: Option<usize>,
    pub message: Text,
}

impl Said {
    /// One of the diagnostics of a language that writes them with ritsu-base (koyomi, chobo,
    /// geas, yuen, sakai).
    pub fn of<X: ritsu_base::diag::Extra>(d: &ritsu_base::diag::Diag<X>) -> Said {
        Said { code: d.code.to_string(), file: d.file.clone(), line: d.line, message: d.message.clone() }
    }

    /// A file that could not be read at all.
    pub fn unreadable(file: &str, why: &str) -> Said {
        Said { code: String::new(), file: file.to_string(), line: None, message: ritsu_base::tr!("`{file}` を読めません: {why}", "cannot read `{file}`: {why}") }
    }
}
