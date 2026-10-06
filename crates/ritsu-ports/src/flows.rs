//! The port of flows (DESIGN 7.4–7.8): what a workflow calls in the other languages, each call
//! with what dandori knows of the values it gives. dandori gathers what a value can be from every
//! place it is put (dandori's DESIGN 1.3), and holds the ranges of rules, tasks and koyomi's
//! integer inputs to them itself (its E014); what it does not check is what only the other
//! language can say — a rule's precondition across two inputs (X2), the days a koyomi date comes
//! to against a rule's range (X3 (a)), a rule's output as an amount chobo takes and the refusals
//! that amount can meet (X4), how long a hold is held against its expiry (X5), the day given to a
//! koyomi date against its range (X6) — which ritsu-cross asks the other language about with
//! these facts. dandori also says which calls give a value the contracts mark secret to another
//! file of the project ([`Flows::sends`]), which ritsu-cross holds to the map (X14), and which
//! calls are of an operation of a contract ([`Flows::operation_calls`]), which ritsu-cross holds to
//! what sekisho's gates allow the workflow (X16).

use crate::{Books, Dates, Rules, Said};
use ritsu_base::naming::Name;
use ritsu_base::text::Text;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// The ports a flow is read with: the rules, the dates files and the books it calls, as `ritsu
/// check` joins them once.
#[derive(Clone)]
pub struct Ports {
    pub rules: Rc<dyn Rules>,
    pub dates: Rc<dyn Dates>,
    pub books: Rc<dyn Books>,
}

/// Where a value a flow gives can come from, as far as the languages it crosses into are
/// concerned: one for each kind of place it is put in anywhere in the flow, joined as dandori
/// joins ranges (a variable given `due.day` in one arm and an input in another can be either).
#[derive(Clone, Debug, PartialEq)]
pub enum Origin {
    /// The day of a koyomi date the flow calls (`due.day`): the dates file, as dandori reaches it
    /// (what `Dates` is asked with), and the date.
    Day { file: PathBuf, date: String },
    /// A numeric output of a rule the flow calls (`fee.amount`): the rule's file, as dandori reaches
    /// it (what `Rules` is asked with), and the output.
    Output { rule: PathBuf, output: String },
    /// `now`, given where a day is taken: the moment the statement runs, which can be any day.
    Now,
    /// A number dandori knows the range of: a literal, or an input, a field or a task's answer with
    /// a `range` (both ends included; None at an open end), as it goes on the wire.
    Range(Option<i128>, Option<i128>),
    /// A place that says nothing of what the value can be (an input or a task's answer without a
    /// `range`, a date that comes in), named in both languages.
    Unknown(Text),
}

/// One call of a rule in a flow.
#[derive(Clone, Debug, PartialEq)]
pub struct RuleCall {
    /// The line of the call, from 1.
    pub line: usize,
    /// The rule's file, as dandori reaches it (what `Rules` is asked with).
    pub rule: PathBuf,
    /// The name the flow uses the rule by (`use rule <name> from …`).
    pub name: String,
    /// Each input the call gives, in the order written.
    pub args: Vec<CallArg>,
}

/// One value a call gives a rule.
#[derive(Clone, Debug, PartialEq)]
pub struct CallArg {
    /// The rule's input.
    pub input: String,
    /// The value as the flow writes it (`order.total`, `3`).
    pub shown: String,
    /// The numbers it can be, on the wire (both ends included; None at an open end), when every
    /// place it comes from has a range; None when one does not.
    pub range: Option<(Option<i128>, Option<i128>)>,
    /// A place it can come from that has no range, in both languages, when there is one.
    pub unknown: Option<Text>,
    /// Every kind of place it can come from (for a day: the koyomi dates whose days it can be).
    pub from: Vec<Origin>,
}

/// One call of a koyomi date in a flow (X6): the day given to the date's one date input. The
/// date's integer inputs are dandori's to hold to their ranges (its E014), as a rule's are.
#[derive(Clone, Debug, PartialEq)]
pub struct DateCall {
    /// The line of the call, from 1.
    pub line: usize,
    /// The dates file, as dandori reaches it (what `Dates` is asked with).
    pub file: PathBuf,
    /// The name the flow uses the file by (`use dates <name> from …`), and the date called.
    pub name: String,
    pub date: String,
    /// The date's input that takes a day, the value given to it as the flow writes it (`now`,
    /// `order.received`), and every kind of place that value can come from.
    pub input: String,
    pub shown: String,
    pub from: Vec<Origin>,
}

/// One call of an operation of a chobo transfer in a flow (X4): a task that runs it.
#[derive(Clone, Debug, PartialEq)]
pub struct TransferCall {
    /// The line of the call, from 1.
    pub line: usize,
    /// The book, as dandori reaches it (what `Books` is asked with), and the transfer.
    pub book: PathBuf,
    pub transfer: String,
    /// `do`, `hold`, `post` or `void`.
    pub op: String,
    /// The task that runs it, and the errors it declares: the reasons it handles.
    pub task: String,
    pub handles: Vec<String>,
    /// Each amount the call gives, in the transfer's order.
    pub amounts: Vec<Amount>,
}

/// One amount a call gives a transfer: the transfer's parameter, the value as the flow writes it,
/// and every kind of place it can come from.
#[derive(Clone, Debug, PartialEq)]
pub struct Amount {
    pub param: String,
    pub shown: String,
    pub from: Vec<Origin>,
}

/// How long it can be from a hold being made to a call that the hold's expiry can refuse (X5): a
/// `post` or a `void` of a case that follows a book's transfer, on every way the flow can reach
/// the call. The hold is made somewhere inside the call that makes it, and the call on it acts
/// somewhere inside its own run, so the fewest seconds run from the end of the one to the start
/// of the other, and the most from the start of the one to the end of the other.
#[derive(Clone, Debug, PartialEq)]
pub struct HoldSpan {
    /// The case, the book (as dandori reaches it) and the transfer it follows.
    pub case: String,
    pub book: PathBuf,
    pub transfer: String,
    /// The line of the call that makes the hold (the first, when more than one can).
    pub made: usize,
    /// The line of the call on it, its task, and the operation (`post`, `void`).
    pub line: usize,
    pub task: String,
    pub op: String,
    /// The fewest seconds from the hold to the call, and what makes them up, a statement at a
    /// time, in both languages (empty when nothing does: the call can come at once).
    pub least: u64,
    pub least_why: Vec<Text>,
    /// The most seconds, when something bounds them; else the first thing that does not, in both
    /// languages (a task with no `timeout`, a wait until a time nothing bounds).
    pub most: Result<u64, Text>,
}

/// Everything a flow that passes dandori's check calls in the other languages.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Crossings {
    pub rules: Vec<RuleCall>,
    pub dates: Vec<DateCall>,
    pub transfers: Vec<TransferCall>,
    pub holds: Vec<HoldSpan>,
}

/// Where a call of a flow sends what it gives, inside the project (X14, DESIGN 16.8). dandori
/// says the parties outside the project itself (its E906), so they are not here.
#[derive(Clone, Debug, PartialEq)]
pub enum Destination {
    /// A file of the project that holds the other side: an OpenAPI document a task calls an
    /// operation of, a `.proto` a `connect` task calls, a rule called at its Connect service, a
    /// child `.flow`, a book, a dates file. As dandori reaches it.
    File(PathBuf),
}

/// A value marked secret, as the flow gives it, and where the mark is written.
#[derive(Clone, Debug, PartialEq)]
pub struct Secret {
    /// The value as the flow writes it, down to the field that is secret: `account.number`.
    pub shown: String,
    /// The file that marks it (a `.proto`, an OpenAPI or AsyncAPI document, or the `.flow`
    /// itself, for `secret`), as dandori reaches it; the line of the mark; and the mark as the
    /// file writes it (`debug_redact = true`, `x-data-classification`, `secret`).
    pub marked_in: PathBuf,
    pub line: usize,
    pub mark: String,
}

/// One call of a flow that gives a secret value to something in the project (X14).
#[derive(Clone, Debug, PartialEq)]
pub struct Send {
    /// The line of the call, from 1, and the task (or the rule) called.
    pub line: usize,
    pub task: String,
    pub to: Destination,
    /// Each secret the call gives, with the parameter that carries it.
    pub secrets: Vec<(String, Secret)>,
    /// The parameters the task says it discloses (`discloses`), with the reason written.
    pub disclosed: Vec<(String, String)>,
}

/// What dandori answers for a flow.
pub trait Flows {
    /// The calls the flow at `file` makes to rules, read with the rules through `rules`, when the
    /// flow passes dandori's check; else what the check says.
    fn rule_calls(&self, file: &Path, rules: Rc<dyn Rules>) -> Result<Vec<RuleCall>, Vec<Said>>;

    /// Every call the flow at `file` makes to rules, koyomi dates and chobo transfers, and every
    /// span from a hold to a call its expiry can refuse, read with the other languages through
    /// `ports`, when the flow passes dandori's check; else what the check says.
    fn crossings(&self, file: &Path, ports: &Ports) -> Result<Crossings, Vec<Said>>;

    /// Every call of the flow at `file` that gives a secret value to a file of the project, read
    /// with the other languages through `ports`, when the flow passes dandori's check; else what
    /// the check says. None by default, until dandori answers it.
    fn sends(&self, file: &Path, ports: &Ports) -> Result<Vec<Send>, Vec<Said>> {
        let _ = (file, ports);
        Ok(Vec::new())
    }

    /// The workflow's name, and every call the flow at `file` (from `root`) makes of an operation of
    /// a contract, read with the other languages through `ports`, when the flow passes dandori's
    /// check; else what the check says. An operation whose contract is outside the root has no
    /// reference, and its calls are not here. None by default, until dandori answers it.
    fn operation_calls(&self, root: &Path, file: &str, ports: &Ports) -> Result<(String, Vec<OperationCall>), Vec<Said>> {
        let _ = (root, file, ports);
        Ok((String::new(), Vec::new()))
    }
}

/// One call a flow makes of an operation of a contract (sekisho's X16): a task bound to an operation
/// of an OpenAPI document (`http` on a `use openapi`) or to a method of a service of a `.proto`
/// (`connect` on a `use proto`), at a line where the flow calls it. sekisho's gates guard the same
/// operations by the same references, so the checks across the borders hold what a workflow calls
/// to what the gate that names it allows it.
#[derive(Clone, Debug, PartialEq)]
pub struct OperationCall {
    /// The line of the call, from 1, and the task called.
    pub line: usize,
    pub task: String,
    /// The operation, as a reference from the root (ritsu's DESIGN 6.2): `openapi "api/orders.json"
    /// operation refundOrder` (its `operationId`, else its method and path), `proto
    /// "shop/v1/orders.proto" service Orders method Refund` (the service from the file's package).
    pub operation: Name,
    /// The error the task declares for a denial, when it declares one: the one that comes back with
    /// the HTTP status 403 (`errors denied = 403`), or for a method called by Connect with the code
    /// `permission_denied` (`errors denied = permission_denied`), whose status is 403 too.
    pub denied: Option<String>,
}

/// A length of time as a message says it, in its largest units: `17 days 9 hours`, `17 日 9 時間`.
pub fn seconds_text(secs: u64) -> Text {
    let parts: Vec<(u64, &str, &str, &str)> = [(86_400, "日", "day", "days"), (3_600, "時間", "hour", "hours"), (60, "分", "minute", "minutes"), (1, "秒", "second", "seconds")]
        .into_iter()
        .scan(secs, |left, (per, ja, one, many)| {
            let n = *left / per;
            *left %= per;
            Some((n, ja, one, many))
        })
        .filter(|(n, ..)| *n > 0)
        .collect();
    if parts.is_empty() {
        return Text { ja: "0 秒".into(), en: "0 seconds".into() };
    }
    Text {
        ja: parts.iter().map(|(n, ja, ..)| format!("{n} {ja}")).collect::<Vec<_>>().join(" "),
        en: parts.iter().map(|(n, _, one, many)| format!("{n} {}", if *n == 1 { one } else { many })).collect::<Vec<_>>().join(" "),
    }
}

/// One precondition of a rule at one call that the check across the border (X2, DESIGN 7.4) could
/// not decide: neither shown to hold for every value the call can give, nor broken by an example.
/// The code dandori writes for the workflow checks it when the workflow runs, as soon as the values
/// are made (dandori's DESIGN 1.17).
#[derive(Clone, Debug, PartialEq)]
pub struct UndecidedPrecondition {
    /// The line of the call, from 1, as `RuleCall::line` gives it.
    pub line: usize,
    /// The rule's file, as dandori reaches it.
    pub rule: PathBuf,
    pub precondition: crate::Precondition,
}

/// What the checks across the borders could not decide of a flow's calls of rules (DESIGN 7.4,
/// item 3), for the code dandori writes: `ritsu dandori` hands ritsu-cross's answer over, and the
/// binary of dandori's own crate hands over one that says nothing (it reads no rule).
pub trait Undecided {
    /// The preconditions left undecided at the calls of the flow at `file`, in the order of the
    /// calls; none when the flow does not pass dandori's check.
    fn preconditions(&self, file: &Path) -> Vec<UndecidedPrecondition>;
}
