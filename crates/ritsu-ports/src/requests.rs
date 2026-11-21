//! What ringi knows of a request (ringi's DESIGN 6.4, 9.4): the files it lowers the request's
//! table, conditions, computations and deadlines to, each with the line of the request every line
//! of it comes from, and the request's form, for what reads a request across a border (the code
//! `ritsu gen` writes, the checks across the borders, the pages). ringi implements it.

use crate::Said;
use ritsu_base::text::Text;
use std::path::{Path, PathBuf};

/// The language a lowered file is read by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoweredTool {
    /// A rule (`route.rule`, `form.rule`, a named table, the enums of the choices).
    Rulec,
    /// A dates file (`terms.cal`, `form.cal`).
    Koyomi,
}

/// One file a request is lowered to, as it is laid where the languages read it.
#[derive(Clone, Debug, PartialEq)]
pub struct LoweredFile {
    /// Where it is laid: beside the request, in a directory named after the request's file without
    /// `.ringi` (`expense.lowered/route.rule` for `expense.ringi`, `expense.ja.lowered/…` for
    /// `expense.ja.ringi`), which is not on the disk.
    pub path: PathBuf,
    pub tool: LoweredTool,
    pub text: String,
    /// Where each line of the text comes from, from its first line: the line of the request (0 for
    /// none in particular) and what the line is there.
    pub lines: Vec<LineFrom>,
}

/// Where a line of a lowered file comes from.
#[derive(Clone, Debug, PartialEq)]
pub struct LineFrom {
    /// The line of the request, from 1; 0 for a line that comes from no line in particular (the
    /// head of the file, a blank line).
    pub line: usize,
    /// What the line is in the request: `the route's row 3`, `the range of the field amount`.
    pub what: Text,
}

/// A field of a request's form, or of a row of its `rows` field.
#[derive(Clone, Debug, PartialEq)]
pub struct RequestField {
    /// The name as written (`金額(amount)` is `金額`).
    pub name: String,
    /// The name the generated code and the lowered files use (`amount`).
    pub code_name: String,
    /// Its title in the file's language.
    pub label: String,
    /// Its type as written: `text`, `long text`, `number`, `money[JPY]`, `date`, `choice`, `choice
    /// from <alias>.<name>`, `attachment`, `person`, `team`, `bool`, `rows`.
    pub ty: String,
    /// The range of a number, an amount or a date as the integers on the wire (an amount in its
    /// smallest unit, a date as its day number), or the limits on a text's length or a `rows`
    /// field's count; None at an open end. A computed field's range is what it is held to (an
    /// application whose value falls outside is refused).
    pub lo: Option<i128>,
    pub hi: Option<i128>,
    /// The values of a `choice` written under it, in order.
    pub values: Vec<String>,
    pub optional: bool,
    pub computed: bool,
    pub approvers_only: bool,
    /// Its line in the request.
    pub line: usize,
    /// The fields of a row, for a `rows` field.
    pub fields: Vec<RequestField>,
}

/// A request's form.
#[derive(Clone, Debug, PartialEq)]
pub struct RequestForm {
    /// The request's name (`expense`).
    pub request: String,
    pub fields: Vec<RequestField>,
}

/// The days a request can be used on (ringi's DESIGN 4.6, 5.10, 8.3): from the day it takes effect
/// (`effective from`, else the first day every piece of data it reads is there), through the last
/// day every piece of it is there. None at an end no data decides: from any day, with no end.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RequestUsable {
    pub from: Option<UsableEnd>,
    pub through: Option<UsableEnd>,
}

/// An end of the days a request can be used on, and the data that decides it.
#[derive(Clone, Debug, PartialEq)]
pub struct UsableEnd {
    /// The day, as days from 1970-01-01 ([`crate::day_text`] writes it).
    pub day: i64,
    /// What decides it: one, or several that end on the same day.
    pub by: Vec<UsableBy>,
}

/// A piece of data that decides an end of the days a request can be used on.
#[derive(Clone, Debug, PartialEq)]
pub struct UsableBy {
    /// `effective from`, `organization`, `master`, `calendar` (the deadlines counted in it) or `form
    /// date` (a date of the form counted in business days in it).
    pub kind: String,
    /// What the request reads it by: the alias of its `use` line (`office`), with the master's name
    /// (`m.accounts`), or the field (`back_on`).
    pub name: String,
    /// The file the data is in, as the caller reaches it; for an organization, the directory of its
    /// data (the directories of its versions).
    pub file: String,
    /// The line of that file, when a row decides it.
    pub line: Option<usize>,
    /// The day the data itself starts or ends on, when it is not the end itself (a calendar's data
    /// ends past the last day a stage can start on).
    pub data: Option<i64>,
    /// What it is, in words.
    pub text: Text,
}

/// What ringi knows of the ticket of a request (ringi's DESIGN 9.3, 9.4), for the checks across
/// the borders (the gate against the route, X18; a hold against how long a round takes, X19; what
/// a flow is given against what it takes, X20) and for `ringi api`, which writes the same.
#[derive(Clone, Debug, PartialEq)]
pub struct RequestTicket {
    /// The request's name (`expense`), in ASCII.
    pub request: String,
    /// Its name as the file writes it (`経費`, of `request 経費(expense_ja)`).
    pub named: String,
    /// Whether a requester who is among a stage's approvers answers it (`self approval allowed`).
    pub self_approval: bool,
    /// The line that says so; 0 where the request writes none, which is `not allowed`.
    pub self_approval_line: usize,
    /// The gates the request reads (`use gate`), in the order written.
    pub gates: Vec<TicketGate>,
    /// The operations the server asks a gate about for the request, by the actions it asks: those a
    /// ticket of the request has, and those of the server's administration.
    pub operations: Vec<TicketOperation>,
    /// The ways a ticket ends, or is taken somewhere an `on` line can act on, besides `submitted`:
    /// of `approved`, `sent back`, `rejected`, `withdrawn` and `expired`, the ones it has.
    pub endings: Vec<String>,
    pub time: TicketTime,
    /// The holds it makes when it is submitted (`on submitted hold …`), in the order written.
    pub holds: Vec<TicketHold>,
    /// The flows it starts (`on … launch`) and asks (an agent's stage, a summary), in the order
    /// written.
    pub flows: Vec<TicketFlow>,
}

/// A gate a request reads.
#[derive(Clone, Debug, PartialEq)]
pub struct TicketGate {
    /// The file, as the caller reaches it.
    pub file: PathBuf,
    /// The line of the `use gate`.
    pub line: usize,
}

/// An operation the server asks a gate about (ringi's DESIGN 7.2 and 14), by the name a gate
/// declares its action with.
#[derive(Clone, Debug, PartialEq)]
pub struct TicketOperation {
    /// Of a ticket: `submit`, `withdraw`, `approve`, `send_back`, `reject`, `pause`, `retract`,
    /// `add_approver`, `hand_over`, `comment` or `see`. Of the server's administration:
    /// `change_request_type` and `read_audit` (of a version of a request type),
    /// `import_organization` and `read_organization_audit` (of the organization).
    pub name: String,
    /// The line of the request that gives its tickets the operation (`pause at most …`); 0 for
    /// one every ticket has.
    pub line: usize,
    /// Whether an approver of a stage does it (`approve`, `send_back`, `reject`, `pause`,
    /// `retract`): what the approvers the route decides have to be allowed.
    pub by_approver: bool,
    /// Whether it decides the stage (`approve`, `send_back`, `reject`): what a requester does not do
    /// to their own request where the route does not allow it.
    pub decides: bool,
}

/// How long a ticket takes, as ringi's check counts it (ringi's DESIGN 7.6), in business days.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TicketTime {
    /// The most of one round: from `submit` to the round's ending, a pause not counted. None where
    /// a stage has no end.
    pub round: Option<u64>,
    /// How many times a ticket goes back at the most (`send back … at most N times`).
    pub backs: u32,
    /// What its requester has each time it goes back (`resubmit within`).
    pub resubmit: Option<u32>,
    /// What a ticket is paused for at the most, in all (`pause at most`).
    pub pause: Option<u32>,
    /// The most of a ticket. None where it may never end.
    pub total: Option<u64>,
    /// The most a hold made at `submit` stays open before the round's ending posts or voids it: one
    /// round and the pause; and where `send back` can send a ticket to an earlier stage, which
    /// starts no new round, each time it walks the round again. None where a stage has no end.
    pub held: Option<u64>,
    /// What has no end; empty where every ticket ends.
    pub no_end: Vec<TicketNoEnd>,
    /// The calendars the days of a round are counted in (the deadlines of its stages, the pause):
    /// each by the alias the request reads it by, with its file as the caller reaches it. Empty
    /// where no calendar is read.
    pub calendars: Vec<TicketCalendar>,
}

/// A wait of a ticket that has no end.
#[derive(Clone, Debug, PartialEq)]
pub struct TicketNoEnd {
    /// What waits, as a sentence with no period.
    pub text: Text,
    /// What gives it an end, as a sentence with no period.
    pub fix: Text,
    /// True for a wait of a round (a stage that stays, a stage with no deadline); false for the
    /// requester's turn between two rounds.
    pub in_round: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TicketCalendar {
    /// The alias as written, which a diagnostic says.
    pub alias: String,
    /// The alias in ASCII (`office` for `本社(office)`), which the JSON takes.
    pub code_name: String,
    pub file: PathBuf,
}

/// A hold a ticket makes when it is submitted, which the round's ending posts or voids.
#[derive(Clone, Debug, PartialEq)]
pub struct TicketHold {
    /// The alias the request reads the book by, and the book's file as the caller reaches it.
    pub alias: String,
    pub book: PathBuf,
    /// The transfer (`reserve` of `budget.reserve`).
    pub transfer: String,
    /// The line of the `on submitted hold`, and the column of the call.
    pub line: usize,
    pub col: usize,
    /// The arguments that read where the requester is in the organization (`dept:
    /// requester.department`), which not every requester has a value for.
    pub args: Vec<HoldArg>,
}

/// An argument of a hold whose value comes from the requester's place in the organization.
#[derive(Clone, Debug, PartialEq)]
pub struct HoldArg {
    /// The parameter of the transfer (`dept`).
    pub name: String,
    /// The value as written (`requester.department`), and its place.
    pub written: String,
    pub line: usize,
    pub col: usize,
    /// For each version of the organization, who of the people who may submit the request has no
    /// value for it; undecided, with the reason, where the organization has no data.
    pub without: crate::Found<Vec<VersionWithout>>,
}

/// The requesters of a version of the organization who have no value for an argument.
#[derive(Clone, Debug, PartialEq)]
pub struct VersionWithout {
    /// The day the version is in force from, as days from 1970-01-01.
    pub from: i64,
    /// How many people may submit the request in the version.
    pub requesters: usize,
    /// Those of them with no value, by their IDs, in the data's order.
    pub without: Vec<String>,
    /// Each of them as a sentence names a person, in the order of `without`: the ID with the name
    /// the organization's data gives it (`e0101 (Head of Sales Division)`).
    pub named: Vec<Text>,
}

/// A flow a ticket starts or asks.
#[derive(Clone, Debug, PartialEq)]
pub struct TicketFlow {
    /// `launch` (`on approved launch pay(…)`), `agent` (a way to choose an approver, `agent
    /// <flow>, else …`) or `summary` (a stage `note from <flow>`).
    pub kind: String,
    /// The alias the request reads the flow by, and the flow's file as the caller reaches it.
    pub alias: String,
    pub flow: PathBuf,
    /// Where it is written: the `on` line, the line of the way to choose, the stage of the summary.
    pub line: usize,
    /// For a `launch`, the endings it is started on; for an agent, the way to choose it is; for a
    /// summary, its stage.
    pub at: Vec<String>,
    /// What the flow is given, by the names of its inputs. A `launch` gives what it writes; an
    /// agent and a summary are given the ticket's ID (`request_id`) and every field of the form, of
    /// which the flow takes those it declares.
    pub given: Vec<FlowGiven>,
}

/// Words that may name lines of the request, kept with the lines apart: `ringi api` writes the
/// words without the lines and the lines as numbers (`text`, `lines`), so that what the words say
/// does not change with where they are written, and a message says the two together (`said`).
#[derive(Clone, Debug, PartialEq)]
pub struct Lined {
    /// The words without the lines (`on the rows of the route`).
    pub text: Text,
    /// The lines the words name, in order: none for words that name no line.
    pub lines: Vec<usize>,
    /// The words with the lines, as a message says them (`on the rows of the route on lines 3, 4`).
    pub said: Text,
}

impl Lined {
    /// Words that name no line.
    pub fn plain(t: Text) -> Lined {
        Lined { text: t.clone(), lines: Vec::new(), said: t }
    }

    /// The words put in a sentence `f` makes of them, with the lines and without.
    #[must_use]
    pub fn within(&self, f: impl Fn(&Text) -> Text) -> Lined {
        Lined { text: f(&self.text), lines: self.lines.clone(), said: f(&self.said) }
    }
}

/// One value a ticket gives a flow.
#[derive(Clone, Debug, PartialEq)]
pub struct FlowGiven {
    /// The input of the flow it is given as.
    pub name: String,
    /// The value as the request writes it (`ticket.id`, `amount`); for a field given to an agent or
    /// a summary, the field.
    pub written: String,
    /// What can be given: the type with its range; why ringi cannot say, otherwise.
    pub carried: crate::Found<crate::Carried>,
    /// Whether it can have no value where the flow is given it: None for one that always has one;
    /// an example in words for one that can lack it, with the row of the route it is on; undecided,
    /// with the reason, otherwise.
    pub none: crate::Found<Option<Lined>>,
    /// Where the range is held to less than the form's, as the flow is given it, in words to follow
    /// "is … " (`on the rows of the route on lines 3, 4`, `where the `when` of the `on` line
    /// holds`): the rows of the route that give an agent its stage, a `when` whose comparisons of
    /// the field with constants ringi follows. None for the whole range of the form.
    pub narrowed: Option<Lined>,
    /// What the value may be held to less by, where ringi does not follow it, in words (a `when`
    /// with a part that is no comparison of a field with a constant, or that reads a field the value
    /// is counted with): `carried` then says no less than can be given, and may say more. None
    /// where `carried` is what can be given.
    pub may_be_less: Option<Text>,
    /// The line of the value: the `on` line, or the field's.
    pub line: usize,
}

/// What ringi answers for a request file. `file` is the file, as the caller reaches it.
pub trait Requests {
    /// The files ringi lowers the request to, in the order it lowers them; else what ringi's check
    /// says of the file, which does not pass.
    fn lowered(&self, file: &Path) -> Result<Vec<LoweredFile>, Vec<Said>>;

    /// The request's form; else what ringi's check says of the file.
    fn form(&self, file: &Path) -> Result<RequestForm, Vec<Said>>;

    /// The days the request can be used on, and the data that decides each end: what the code
    /// `ritsu gen` writes refuses a request past, and what an administrator is told of before
    /// the day; else what ringi's check says of the file.
    fn usable(&self, file: &Path) -> Result<RequestUsable, Vec<Said>>;

    /// What ringi knows of the request's ticket: its operations and endings, how long it takes,
    /// the holds it makes and the flows it starts and asks; else what ringi's check says of the
    /// file. Undecided by default, for a port that does not say: nothing is then held against it.
    fn ticket(&self, file: &Path) -> Result<crate::Found<RequestTicket>, Vec<Said>> {
        let _ = file;
        Ok(crate::Found::Undecided(ritsu_base::tr!("この口は、チケットの事実を答えません", "this port does not say what a request's ticket does")))
    }

    /// The server's API for the requests at `files`, as `ritsu gen --target requests` would write
    /// its `openapi.json` of them now, with their paths from `root` (ringi's DESIGN 12.4): what a
    /// document a flow reads is held to (X20, where a flow opens a ticket). Else what ringi's check
    /// says of a file that does not pass. Undecided by default, for a port that does not say:
    /// nothing is then held against it.
    fn openapi(&self, files: &[std::path::PathBuf], root: &Path) -> Result<crate::Found<String>, Vec<Said>> {
        let _ = (files, root);
        Ok(crate::Found::Undecided(ritsu_base::tr!("この口は、サーバーの API を答えません", "this port does not say what the server's API is")))
    }
}
