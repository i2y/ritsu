//! The port of flows (DESIGN 7.4–7.8): what a workflow calls in the other languages, each call
//! with what dandori knows of the values it gives. dandori gathers what a value can be from every
//! place it is put (dandori's DESIGN 1.3), and holds the ranges of rules, tasks and koyomi's
//! integer inputs to them itself (its E014); what it does not check is what only the other
//! language can say — a rule's precondition across two inputs (X2), the days a koyomi date comes
//! to against a rule's range (X3 (a)), a rule's output as an amount chobo takes over what the runs
//! that reach the transfer give the rule, and the refusals that amount can meet (X4), how long a
//! hold is held against its expiry (X5), the day given to a koyomi date against its range (X6) —
//! which ritsu-cross asks the other language about with these facts. dandori also says which
//! calls give a value the contracts mark secret to another file of the project ([`Flows::sends`]),
//! which ritsu-cross holds to the map (X14), and which calls are of an operation of a contract
//! ([`Flows::operation_calls`]), which ritsu-cross holds to what sekisho's gates allow the workflow
//! (X16): each call with what it gives the operation and what the runs of the flow that reach it
//! hold there ([`Reached`]), which sekisho reads in the gate's terms.

use crate::{Books, Carried, Dates, Found, Rules, Said};
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
/// concerned: one for each kind of place the runs that reach the call last put it in, joined as
/// dandori joins ranges (a variable given `due.day` in one arm and an input in another can be
/// either after the arms, and is the one its arm gave it inside an arm).
#[derive(Clone, Debug, PartialEq)]
pub enum Origin {
    /// The day of a koyomi date the flow calls (`due.day`): the dates file, as dandori reaches it
    /// (what `Dates` is asked with), and the date.
    Day { file: PathBuf, date: String },
    /// A numeric output of a rule the flow calls (`fee.amount`): the rule's file, as dandori reaches
    /// it (what `Rules` is asked with), and the output.
    Output { rule: PathBuf, output: String },
    /// `now` in a workflow that does not say the period it runs in, or in its `on failure` and `on
    /// cancel`, which the period does not hold: the moment the statement runs, which can be any day.
    Now,
    /// A number dandori knows the range of: a literal, or an input, a field or a task's answer with
    /// a `range` (both ends included; None at an open end), as it goes on the wire. A date's range
    /// is its days, as day numbers (days since 1970-01-01, as koyomi, rulec and kaname count them):
    /// a date is never given where a number is taken, so the two are not taken for each other.
    Range(Option<i128>, Option<i128>),
    /// A time dandori knows the range of: an input, a field or a task's answer with a `range`, or
    /// `now` in a workflow that says the period it runs in (`now range`, `now` true), which every
    /// statement that reads `now` is held to. In seconds since 1970-01-01T00:00:00Z, both ends
    /// included (None at an open end), with the range as the `.flow` writes it. The day it is where
    /// a date takes it is the dates file's calendar's to say, by its offset.
    Moment { lo: Option<i64>, hi: Option<i64>, text: String, now: bool },
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
    /// The numbers it can be on the runs that reach the call, on the wire (both ends included; None
    /// at an open end), when every place it comes from on those runs has a range; None when one
    /// does not. A variable is what those runs last put in it: a value put in it on a way that
    /// does not reach the call is not counted.
    pub range: Option<(Option<i128>, Option<i128>)>,
    /// A place it can come from on those runs that has no range, in both languages, when there is
    /// one.
    pub unknown: Option<Text>,
    /// For a truth value or a value of an enum: every value it can be on the runs that reach the
    /// call, an enum's by the rule's names of them — what the places it comes from let in, less
    /// what the arms of the `match`es on the way keep from the call; a value written out is itself.
    /// None for a value of another type, and where the caller does not say.
    pub values: Option<Vec<crate::Value>>,
    /// Every kind of place it can come from on the runs that reach the call (for a day: the koyomi
    /// dates whose days it can be).
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
    /// `order.received`), and every kind of place that value can come from on the runs that reach
    /// the call.
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
/// every kind of place it can come from on the runs that reach the call, and the calls of rules
/// whose outputs it can be.
#[derive(Clone, Debug, PartialEq)]
pub struct Amount {
    pub param: String,
    pub shown: String,
    pub from: Vec<Origin>,
    /// The calls of rules whose outputs the amount can be on the runs of the flow that reach the
    /// transfer, each with what those runs can give the rule (X4 counts the output over that, not
    /// over every input of the rule); empty where no run that reaches the transfer gives it a
    /// rule's output. None where dandori does not follow the runs to the transfer: the outputs
    /// `from` names are then counted over every input of their rules.
    pub calls: Option<Vec<OutputCall>>,
}

/// A call of a rule whose output an amount can be (X4), with what the runs of the flow that reach
/// the transfer can give the rule there.
#[derive(Clone, Debug, PartialEq)]
pub struct OutputCall {
    /// The line of the call of the rule, from 1.
    pub line: usize,
    /// The rule's file, as dandori reaches it (what `Rules` is asked with), and the output.
    pub rule: PathBuf,
    pub output: String,
    /// Each input the call gives, in the order written.
    pub inputs: Vec<CallInput>,
}

impl OutputCall {
    /// What the call gives the rule's inputs, as `Rules::output_values_over`, `outputs_over` and
    /// `outputs_together` take it, with what rulec knows of the rule (`facts`, for the order of an
    /// enum's values): a number on the wire, an enum by the places of its values in the rule's order
    /// (a run of places as one range, a value apart from the others as the input named again), a
    /// bool as 0 and 1. An input whose value the rule does not name is held to nothing narrower.
    pub fn held_ranges(&self, facts: Option<&crate::RuleFacts>) -> Vec<(String, Option<i128>, Option<i128>)> {
        let mut out = Vec::new();
        for i in &self.inputs {
            match &i.held {
                None => {}
                Some(Held::Range(lo, hi)) => out.push((i.input.clone(), *lo, *hi)),
                Some(Held::Values(values)) => {
                    let ty = facts.and_then(|f| f.inputs.iter().find(|c| c.name == i.input)).map(|c| match &c.ty {
                        crate::ColumnType::Opt(t) => (**t).clone(),
                        t => t.clone(),
                    });
                    let order: Vec<String> = match &ty {
                        Some(crate::ColumnType::Bool) => vec!["false".into(), "true".into()],
                        Some(crate::ColumnType::Enum { name, .. }) => facts.and_then(|f| f.enums.iter().find(|e| e.name == *name)).map(|e| e.values.iter().map(|v| v.name.clone()).collect()).unwrap_or_default(),
                        _ => Vec::new(),
                    };
                    let places: Option<Vec<i128>> = values.iter().map(|v| order.iter().position(|o| o == v).map(|p| p as i128)).collect();
                    let Some(mut places) = places.filter(|p| !p.is_empty()) else { continue };
                    places.sort_unstable();
                    let mut runs: Vec<(i128, i128)> = Vec::new();
                    for p in places {
                        match runs.last_mut() {
                            Some((_, hi)) if *hi + 1 == p => *hi = p,
                            _ => runs.push((p, p)),
                        }
                    }
                    out.extend(runs.into_iter().map(|(lo, hi)| (i.input.clone(), Some(lo), Some(hi))));
                }
            }
        }
        out
    }
}

/// One value a call of a rule gives, as the runs that reach a transfer of its output see it.
#[derive(Clone, Debug, PartialEq)]
pub struct CallInput {
    /// The rule's input, and the value as the flow writes it (`kind`, `order.kind`, `workshop`).
    pub input: String,
    pub shown: String,
    /// What those runs can give it, when that is less than the rule's input takes; None for all
    /// it takes. A number is held to the range dandori gives the call of the rule for X2; an enum
    /// or a bool to the values the places it comes from can be, less those the arms of a `match`
    /// on it before the transfer leave out.
    pub held: Option<Held>,
    /// Whether the value is a variable, or a field of one, of an enum or a bool that nothing puts
    /// another value in between the call and the transfer: a `match` on it before the transfer
    /// holds what the call gives the rule to the values of the arm.
    pub matchable: bool,
}

/// What a flow can give a rule's input, when that is less than the input takes.
#[derive(Clone, Debug, PartialEq)]
pub enum Held {
    /// A number on the wire, both ends included; None at an open end.
    Range(Option<i128>, Option<i128>),
    /// Values of an enum by the rule's names of them, in the enum's order; `false` and `true` for
    /// a bool.
    Values(Vec<String>),
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
    /// What makes the most as long as it is, when koyomi's days do (a wait until a date's time, the
    /// time a call with `timeout until` waits until): each such wait, with the day that makes it the
    /// longest, in both languages (empty when nothing does, or nothing bounds the most).
    pub most_why: Vec<Text>,
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

    /// What the workflow at `file` declares of itself (ringi's DESIGN 9.3, X20): its name, and each
    /// input and output with its type and range, the same as `dandori api` writes them, read with
    /// the other languages through `ports` (an enum a rule declares), when the flow passes
    /// dandori's check; else what the check says. Undecided by default, for a port that does not
    /// say: nothing is then held against it.
    fn declared(&self, file: &Path, ports: &Ports) -> Result<Found<FlowDeclared>, Vec<Said>> {
        let _ = (file, ports);
        Ok(Found::Undecided(ritsu_base::tr!("この口は、ワークフローの入力と出力を答えません", "this port does not say what a workflow takes and gives")))
    }

    /// What the input, the variable or the field `path` names (`amount`, `order.total`) holds in
    /// the flow at `file` (from `root`), wherever it is read: its type, and for a number the range
    /// dandori knows it in from every place a value is put in it (its E014). None when the flow
    /// has no such value; what the check says when the flow does not pass it. None by default,
    /// until dandori answers it.
    fn value(&self, root: &Path, file: &str, ports: &Ports, path: &str) -> Result<Option<FlowValue>, Vec<Said>> {
        let _ = (root, file, ports, path);
        Ok(None)
    }

    /// The names a `path` of [`Flows::value`] goes on with in the flow at `file` (from `root`), for
    /// a message that says what can be written: with `under` empty, the flow's inputs and
    /// variables; else the fields of the record the value `under` is (`order`), none when it is no
    /// record or no value. What the check says when the flow does not pass it. None by default,
    /// until dandori answers it.
    fn value_names(&self, root: &Path, file: &str, ports: &Ports, under: &str) -> Result<Vec<String>, Vec<Said>> {
        let _ = (root, file, ports, under);
        Ok(Vec::new())
    }
}

/// A place a workflow declares: an input or an output, as `dandori api` writes it.
#[derive(Clone, Debug, PartialEq)]
pub struct FlowPlace {
    pub name: String,
    /// Its type as the `.flow` writes it (`money[JPY]`, `Check`, `string?`).
    pub ty: String,
    /// Whether it may be absent (`T?`).
    pub optional: bool,
    /// Its range as `dandori api` writes it, when the `.flow` writes one: a number's ends are the
    /// whole numbers on the wire (`>=1 <=30000` for `money[JPY]`), as its `min` and `max` are.
    pub range: Option<String>,
    /// The type with the range, as a check across the languages compares it (the port of `fed
    /// by`'s [`crate::Carried`]).
    pub carried: crate::Carried,
    /// Its line, from 1; 0 where the flow does not say.
    pub line: usize,
}

/// What a workflow declares of itself ([`Flows::declared`]).
#[derive(Clone, Debug, PartialEq)]
pub struct FlowDeclared {
    /// The workflow's name, and its version (`v1`).
    pub name: String,
    pub version: u32,
    /// The workflow's type on Temporal, as dandori writes it (`contract_summary_v1`): what a run is
    /// started by, and the task queue of its worker.
    pub workflow_type: String,
    /// In the order written.
    pub inputs: Vec<FlowPlace>,
    pub outputs: Vec<FlowPlace>,
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
    /// Each value the call gives, by the task's parameter, in the order written.
    pub args: Vec<(String, FlowValue)>,
    /// What the runs of the flow that reach the call hold there; None where dandori does not
    /// follow the runs to the call, which says nothing of them.
    pub reached: Option<Reached>,
}

impl OperationCall {
    /// A call nothing more is said of: no value it gives, and no runs followed to it.
    pub fn new(line: usize, task: &str, operation: Name, denied: Option<&str>) -> OperationCall {
        OperationCall { line, task: task.to_string(), operation, denied: denied.map(str::to_string), args: Vec::new(), reached: None }
    }
}

/// A value of a flow, as the checks across the borders read it where the flow makes a call (X16).
#[derive(Clone, Debug, PartialEq)]
pub struct FlowValue {
    /// The value as the flow writes it (`order.total`, `3`, `urgent`).
    pub shown: String,
    /// The input, the variable or the field it reads (`amount`, `order.total`), while nothing has
    /// put another value in it between where it was read and the call; None for a value written
    /// out, and for one that has been given another value since.
    pub path: Option<String>,
    /// What it holds, as the port of `fed by` says what a place holds: a number with its unit and
    /// the range dandori knows it in (None at an end nothing says), an enum with the values the
    /// places it comes from let in (the value itself, for one written out), a date with its days.
    /// A number written out where a parameter of a number's type takes it is of the parameter's
    /// type and unit, as dandori's check takes it.
    pub holds: Carried,
    /// For a bool that is one value, that value: one written out, or a variable every value put
    /// in which is the same (where the call is made, on the runs that reach it).
    pub constant: Option<bool>,
}

/// What the runs of a flow that reach a call hold there (X16), as dandori follows them: through
/// the arms of each `match` and the handlers of each call, and through a loop round after round
/// until what its rounds start from no longer changes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reached {
    /// Each input, variable or field a `match` on the way to the call has read (`kind`,
    /// `order.kind`, `r.band`), with the values it can be at the call, as the arms write them: the
    /// values of an enum, `true` and `false`, and for a value that may be none, `none` and (where
    /// it is no enum or bool) `some`.
    pub held: Vec<(String, Vec<String>)>,
    /// Each variable that holds the answer of a call of a rule on these runs (`let r =
    /// limit(amount: amount)`), with every call whose answer it can hold.
    pub answers: Vec<(String, Vec<AnswerCall>)>,
    /// The inputs and the variables every run that reaches the call has given a value.
    pub set: Vec<String>,
    /// Each input or variable that holds less on these runs than [`Flows::value`] says of it
    /// (which is what every value put in it anywhere in the flow can be), with what it holds at
    /// the call: what the runs that reach the call last put in it. One not here is what
    /// [`Flows::value`] says.
    pub values: Vec<(String, FlowValue)>,
}

/// A call of a rule whose answer a variable holds where a flow calls an operation (X16).
#[derive(Clone, Debug, PartialEq)]
pub struct AnswerCall {
    /// The line of the call of the rule, from 1, and the rule's file, as dandori reaches it (what
    /// `Rules` is asked with).
    pub line: usize,
    pub rule: PathBuf,
    /// Each input the call gave, by the rule's input, in the order written.
    pub inputs: Vec<(String, FlowValue)>,
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
/// The code dandori writes for the workflow checks it as soon as the values are made, and so do
/// `ritsu run` and `ritsu dandori run` (dandori's DESIGN 1.17).
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
