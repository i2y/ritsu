//! The port of rules (DESIGN 3.2, `Rules`): what rulec knows of a rule, as types, and the
//! questions a workflow asks of it. Everything dandori reads of a rule today from three JSON
//! documents (`rulec schema`, `certificate` and `api`) is here, with the units the documents
//! only described in words.

use crate::{Answer, Said};
use ritsu_base::text::{Lang, Text};
use ritsu_units::Unit;
use std::path::Path;

/// What rulec knows of one rule that passes its check.
#[derive(Clone, Debug, PartialEq)]
pub struct RuleFacts {
    /// The rule's name (`送料`) and its ASCII alias (`shipping_fee`).
    pub rule: String,
    pub alias: String,
    /// The rule's own version (`v4`).
    pub version: String,
    /// The SHA-256 of the source, in hex.
    pub sha256: String,
    /// The version of rulec that read it.
    pub rulec: String,
    /// In the order declared.
    pub inputs: Vec<Column>,
    pub outputs: Vec<Column>,
    /// The list the rule walks (`elements 明細(lines)`), when it walks one: its name and alias,
    /// and the fields of one element.
    pub elements: Option<(String, String, Vec<Column>)>,
    /// Every enum the rule knows (its own, the ones it takes from a contract, a machine's
    /// states), by name.
    pub enums: Vec<RuleEnum>,
    pub machine: Option<Machine>,
    /// What the generated code holds a caller to that the shape of an input cannot say.
    pub preconditions: Vec<Precondition>,
    /// The rule's Connect service.
    pub connect: Option<Connect>,
    /// How the generated code is called in the three languages a workflow calls it from.
    pub typescript: Call,
    pub python: Call,
    pub go: Call,
}

/// An input, an output, or a field of an element.
#[derive(Clone, Debug, PartialEq)]
pub struct Column {
    pub name: String,
    pub alias: String,
    pub ty: ColumnType,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ColumnType {
    Bool,
    Str,
    /// A calendar day, `YYYY-MM-DD` on the wire.
    Date,
    /// One of the rule's enums, by its name.
    Enum(String),
    /// A number: the type as rulec spells it (`money[JPY, incl_tax]`, `rate`); its unit, with a
    /// rate's step (None for a spelling the table of units does not have, which rulec lets
    /// through); and its declared range, as the integers that go on the wire.
    Num { written: String, unit: Option<Unit>, min: Option<i128>, max: Option<i128> },
    /// `T?`: the value may be `none`.
    Opt(Box<ColumnType>),
}

/// An enum of the rule.
#[derive(Clone, Debug, PartialEq)]
pub struct RuleEnum {
    pub name: String,
    /// The enum's ASCII name in the generated code (`MemberKind`).
    pub alias: String,
    pub values: Vec<EnumValue>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EnumValue {
    pub name: String,
    pub alias: String,
}

/// The state machine a rule is one step of (rulec's §15.148): the table that decides the next
/// state, its axes, and for each row the coordinates it takes, where it goes and what it writes.
/// chobo describes the life of a hold the same way ([`crate::BookFacts`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Machine {
    pub name: String,
    /// The input that carries the state in, the output that carries it out, and their enum.
    pub carry_in: String,
    pub carry_out: String,
    pub state_enum: String,
    pub states: Vec<String>,
    pub initial: usize,
    pub finals: Vec<usize>,
    /// The inputs a case passes unchanged from its first call to its last.
    pub held: Vec<String>,
    /// The policy of the deciding table (`first`, `unique`, `all`).
    pub policy: String,
    pub axes: Vec<Axis>,
    /// Which axis is the state; None when the table does not read the state as a column.
    pub state_axis: Option<usize>,
    /// The outputs the table decides, in the order a row writes them.
    pub decides: Vec<String>,
    pub rows: Vec<MachineRow>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Axis {
    pub column: String,
    pub coords: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MachineRow {
    /// The row's number in the table, from 1.
    pub row: usize,
    /// For each axis, the coordinates the row takes.
    pub accepts: Vec<Vec<usize>>,
    /// The state the row goes to; None when it keeps the state.
    pub to: Option<usize>,
    /// What the row writes, in the order of `decides`; None where it computes the value.
    pub produces: Vec<Option<String>>,
}

/// What the generated code holds a caller to beyond the shape of each input (rulec's §15.116).
#[derive(Clone, Debug, PartialEq)]
pub enum Precondition {
    /// `constraint <left> <op> <right>`: a relation between two inputs.
    Relation { left: String, op: String, right: String },
    /// The total of one column over the list the rule walks, bounded above (on the wire).
    Sum { name: String, over: String, of: String, max: i128 },
    /// How many elements the list may have.
    Length { sequence: String, max: i128 },
    /// A date input that takes only the days of a koyomi date (`range from koyomi`, rulec's
    /// §15.174): the file and the date as the rule names them, and the days.
    Days { input: String, file: String, date: String, days: crate::DaySet },
}

/// The rule's Connect service (rulec's §15.112).
#[derive(Clone, Debug, PartialEq)]
pub struct Connect {
    /// What a call POSTs to: `/rulec.urgency.v1.UrgencyService/Decide`.
    pub path: String,
    /// How the service writes a field's name in JSON (`lowerCamelCase`).
    pub json_names: String,
    /// How it writes a 64-bit integer (`string`).
    pub json_int64: String,
    pub request: Vec<WireField>,
    pub response: Vec<WireField>,
    /// The fields of one element of the list the rule walks.
    pub elements: Option<Vec<WireField>>,
    pub enums: Vec<WireEnum>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WireField {
    /// The rule's name for it.
    pub name: String,
    /// The `.proto`'s field.
    pub field: String,
    /// The `.proto`'s type (`int64`, `bool`, `rulec.urgency.v1.Carrier`).
    pub ty: String,
    pub optional: bool,
    /// For an enum, the rule's name of the enum.
    pub enumeration: Option<String>,
}

/// One enum on the wire: the rule's name of it, the `.proto`'s type, the contract it comes from
/// (the file the rule names, and the path the generated `.proto` imports it by), the value that
/// says a field is not set, and each value of the rule's with its name and number on the wire.
#[derive(Clone, Debug, PartialEq)]
pub struct WireEnum {
    pub name: String,
    pub alias: String,
    pub contract: Option<(String, String)>,
    pub unset: Option<String>,
    pub values: Vec<(String, String, i64)>,
}

/// How the generated code is called in one language (what `rulec api` lists under it).
#[derive(Clone, Debug, PartialEq)]
pub struct Call {
    /// The module (`shipping_fee.ts`, `shipping_fee`), or for Go the package.
    pub module: String,
    /// The function (`shipping_fee`, Go's `ShippingFee`).
    pub function: String,
    /// The struct the inputs go in (Go's `Input`); empty where the inputs are arguments.
    pub input_type: String,
    pub params: Vec<Param>,
    pub outputs: Vec<Param>,
    pub enums: Vec<CallEnum>,
}

/// A parameter or an output, as the language's code names and types it.
#[derive(Clone, Debug, PartialEq)]
pub struct Param {
    /// The rule's name.
    pub name: String,
    /// The name in the code.
    pub alias: String,
    /// The type in the code (`JPYInclTax`, `bigint`, `Carrier`).
    pub ty: String,
    pub optional: bool,
}

/// An enum, as the language's code names it and its members.
#[derive(Clone, Debug, PartialEq)]
pub struct CallEnum {
    pub name: String,
    pub alias: String,
    /// Each value: the rule's name, and the member in the code (`CouponKind.PERCENT`).
    pub values: Vec<(String, String)>,
}

/// What a numeric output of a rule comes to (`Rules::output_values`), on the wire.
#[derive(Clone, Debug, PartialEq)]
pub struct OutputValues {
    /// The fewest and the most, as rulec's intervals read them; None at an end it has none for.
    pub min: Option<i128>,
    pub max: Option<i128>,
    /// Each number the output comes to, when every row that decides it writes one; None when
    /// some row computes it.
    pub values: Option<Vec<i128>>,
    /// For a value the output comes to, an input that reaches it (from rulec's vectors), in the
    /// order of the values; the fewest and the most first.
    pub examples: Vec<(i128, Values)>,
}

/// A value across the border, as it goes on the wire.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    /// A whole number counted in its declared unit; a rate, in its step.
    Int(i128),
    Str(String),
    /// `YYYY-MM-DD`.
    Date(String),
    /// An enum's value, by the rule's name of it.
    Enum(String),
    /// The list a rule walks: each element, its fields by name.
    List(Vec<Vec<(String, Value)>>),
    /// `none`, for a value that may be absent.
    None,
}

/// Values by name.
pub type Values = Vec<(String, Value)>;

/// Why a rule gave no answer.
#[derive(Clone, Debug, PartialEq)]
pub enum RuleError {
    /// The rule does not pass its check, or cannot be read.
    Unread(Vec<Said>),
    /// An input the generated code refuses at its door: not of its type, outside its range, or
    /// breaking a precondition.
    Input(Text),
    /// No row answers, or two answer differently (a rule that passes its check never does this
    /// for an input it takes).
    Contradiction(Text),
}

/// What rulec answers for a rule (DESIGN 3.2). `rule` is the rule's file, as the caller reaches
/// it.
pub trait Rules {
    /// What rulec knows of the rule, when it passes check; else what check says.
    fn facts(&self, rule: &Path) -> Result<RuleFacts, Vec<Said>>;

    /// Whether each precondition holds for every value the inputs may take, each input (or a
    /// field of the list the rule walks) within the range given (the integers on the wire), or
    /// the rule's own range for one left out (DESIGN 7.4, X2); and the list the rule walks no
    /// longer than `max_len`, when the caller knows how long it can be (None: not known, and a
    /// bound on the list's total or length is not decided).
    fn preconditions_hold(&self, rule: &Path, ranges: &[(String, Option<i128>, Option<i128>)], max_len: Option<i128>) -> Result<Vec<(Precondition, Answer<Values>)>, Vec<Said>>;

    /// What a numeric output comes to over every input the rule takes (DESIGN 7.6, X4): its
    /// fewest and most, and, where every row that decides it writes a number, each of those
    /// numbers with an input that reaches it.
    fn output_values(&self, rule: &Path, output: &str) -> Result<crate::Found<OutputValues>, Vec<Said>>;

    /// Whether the rule's tables are complete, without overlap and without a row nothing
    /// reaches, when the date input `input` takes only the days in `days` (DESIGN 7.5 (b), X3).
    fn checked_over(&self, rule: &Path, input: &str, days: &crate::DaySet) -> Result<Answer<Text>, Vec<Said>>;

    /// The range a date input declares, as day numbers (DESIGN 7.5 (a), X3): both ends, or None
    /// at an open one; for an input whose range is a koyomi date, the hull of its days. The
    /// default answers nothing known, for a port that does not read ranges of dates.
    fn date_range(&self, rule: &Path, input: &str) -> Result<(Option<i64>, Option<i64>), Vec<Said>> {
        let _ = (rule, input);
        Ok((None, None))
    }

    /// The outputs for these inputs (rulec's reference evaluator), refusing what the generated
    /// code refuses.
    fn eval(&self, rule: &Path, inputs: &Values) -> Result<Values, RuleError>;

    /// The approver's page, as `rulec doc` draws it — Markdown, or with `html` the page that
    /// tries a case — in `lang`, naming the file `shown` (dandori names the file alone).
    fn doc(&self, rule: &Path, shown: &str, html: bool, lang: Lang) -> Result<String, Vec<Said>>;

    /// Whether rulec is joined at all: false for the port the binary of a receiving language's own
    /// crate holds, which reads no rule (ritsu's DESIGN 2.3), so that the language can say so once,
    /// in its own code, rather than for each rule it asks of.
    fn joined(&self) -> bool {
        true
    }
}
