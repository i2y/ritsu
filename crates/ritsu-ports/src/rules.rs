//! The port of rules (DESIGN 3.2, `Rules`): what rulec knows of a rule, as types, and the
//! questions a workflow asks of it. Everything dandori reads of a rule today from three JSON
//! documents (`rulec schema`, `certificate` and `api`) is here, with the units the documents
//! only described in words.

use crate::{Answer, Said};
use ritsu_base::text::{Lang, Text};
use ritsu_units::{Rat, Unit};
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
    /// The lists the rule walks (`elements 明細(lines)`), in the order the rule declares them: each
    /// with its name and alias, and the fields of one element. Empty for a rule that walks none; a
    /// rule may walk more than one (rulec's §15.219).
    pub elements: Vec<(String, String, Vec<Column>)>,
    /// Every enum the rule knows (its own, the ones it takes from a contract, a machine's
    /// states), by name.
    pub enums: Vec<RuleEnum>,
    pub machine: Option<Machine>,
    /// What the generated code holds a caller to that the shape of an input cannot say.
    pub preconditions: Vec<Precondition>,
    /// The rule's Connect service. None for an `enums` file (rulec's §15.212), which is made into
    /// modules of types and nothing else: no service is generated from it (rulec's §15.258).
    pub connect: Option<Connect>,
    /// How the generated code is called in the three languages a workflow calls it from, and in
    /// Rust, which kaname's code of a package calls it in.
    pub typescript: Call,
    pub python: Call,
    pub go: Call,
    pub rust: Call,
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
    /// A calendar day, `YYYY-MM-DD` on the wire, and its range as day numbers (days since
    /// 1970-01-01, the count rulec's code holds a date in): an input's and a field of an element's as
    /// its line declares it, or the hull of the days of the koyomi date an input's range is; an
    /// output's the hull of the days its answers can be, where rulec's check works them out (the rows
    /// that decide it, what a `min` or `max` over a list answers with). None at an end not known: the
    /// calendar's own, 0001-01-01 and 9999-12-31.
    Date { min: Option<i64>, max: Option<i64> },
    /// One of the rule's enums, by its name; and for an input of an ordered enum that declares a
    /// `range` over its values (`range >=basic <=gold`, rulec's §15.210), the values the range
    /// lets in, by the rule's names of them, in the enum's order: what the generated code takes at
    /// its door, and the reference evaluator (`Rules::eval`) too. None where every value of the
    /// enum is taken: an input with no range, an output, a field of an element.
    Enum { name: String, range: Option<Vec<String>> },
    /// A number: the type as rulec spells it (`money[JPY, incl_tax]`, `rate`); its unit, with a
    /// rate's step (None for a spelling the table of units does not have, which rulec lets
    /// through); and its range, as the integers that go on the wire: an input's as declared, an
    /// output's the one its answers lie in, written or not, as `rulec schema` and `rulec api` say
    /// it (rulec's §15.271). `round`: for an output, the step its `round` puts its answers on, in
    /// its unit (`round down(0.5%)` is 1/200), which can be coarser than the step a rate travels
    /// in (`rate[step 0.25%]`, the unit's); None for an input and a field of an element.
    Num { written: String, unit: Option<Unit>, min: Option<i128>, max: Option<i128>, round: Option<Rat> },
    /// `T?`: the value may be `none`.
    Opt(Box<ColumnType>),
}

impl ColumnType {
    /// A date with no range written.
    pub const DATE: ColumnType = ColumnType::Date { min: None, max: None };

    /// The range of a date, or of a date that may be none (`date?`), as day numbers; None for a
    /// column of any other type. The one place a rule's date says its range: the port has no other
    /// question of it.
    pub fn date_range(&self) -> Option<(Option<i64>, Option<i64>)> {
        match self {
            ColumnType::Date { min, max } => Some((*min, *max)),
            ColumnType::Opt(t) => t.date_range(),
            _ => None,
        }
    }

    /// Whether the column holds a date, or a date that may be none (`date?`), whatever its range.
    pub fn is_date(&self) -> bool {
        self.date_range().is_some()
    }
}

/// An enum of the rule.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuleEnum {
    pub name: String,
    /// The enum's ASCII name in the generated code (`MemberKind`).
    pub alias: String,
    pub values: Vec<EnumValue>,
    /// The enum's public name as the rule writes it: its alias in parentheses (`緊急度(urgency)`
    /// is `urgency`), else its name. What a module of kaname names it by beside its name (kaname's
    /// DESIGN 2.6). An enum the rule does not declare in words of its own (one of a `.proto`, a
    /// machine's states) has its name.
    pub public: String,
    /// Whether its values are ordered as written (`enum era = ancient < medieval`, rulec's
    /// §15.210), so that two of them compare. False for a port that does not say.
    pub ordered: bool,
    /// Where it is declared, when the rule reads it whole from an `enums` file (`import enum
    /// <name> from "<file>"`, rulec's §15.212): the file as the caller reaches it (the rule's
    /// directory joined with the path written), and the enum's name there. Two rules that read the
    /// same enum of the same file hold one type (kaname's DESIGN 3.5). None for an enum the rule
    /// declares itself, and for a port that does not say.
    pub origin: Option<(std::path::PathBuf, String)>,
}

/// A value of a rule's enum.
#[derive(Clone, Debug, PartialEq)]
pub struct EnumValue {
    /// As the rule names it (`上限まで`).
    pub name: String,
    /// The member in the generated code (`WithinLimit`).
    pub alias: String,
    /// The value's public name as the rule writes it: its alias in parentheses
    /// (`上限まで(within_limit)` is `within_limit`), else its name. What a gate writes into Cedar,
    /// and what `Rules::outputs_over` answers an enum's value with.
    pub public: String,
    /// The display name the rule writes after the value (`goods "Goods and services"`, rulec's
    /// §15.211), in the words the rule is written in; None for a value with none. For people and
    /// screens only: no check and no wire reads it.
    pub title: Option<String>,
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
    /// For each coordinate, the first and the last value it stands for, written as the coordinates
    /// are: a coordinate of numbers is a set of values the table answers alike (`0.1%` stands for
    /// `0.1%` to `0.4%` where the table cuts at 0% and 0.5% on a step of 0.1%), and any value
    /// between its ends is one of them. None for a coordinate that is no set of numbers (a word,
    /// `true`, the absent value); None inside the pair for an end the input's range leaves open.
    pub ends: Vec<Option<(Option<String>, Option<String>)>>,
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
    /// `constraint <a> + <b> <op> <c>`: a relation where one side adds inputs up (rulec's
    /// §15.216). Each side is the inputs it adds, in the order the rule writes them; at most one
    /// side has more than one.
    SumRelation { left: Vec<String>, op: String, right: Vec<String> },
    /// The total of one column over the list the rule walks, bounded above (on the wire), and
    /// below where the rule's range for it starts above zero (rulec's §15.314): a list that adds up
    /// to less, an empty one to begin with, is refused as one that passes the top is. `unit` is the
    /// total's, with what one integer of `max` and `min` counts where that is less than a whole unit
    /// (`Unit::step`), so that a sentence says the ends in it (rulec's §15.333); None for a spelling
    /// the table of units does not have.
    Sum { name: String, over: String, of: String, max: i128, min: Option<i128>, unit: Option<Unit> },
    /// How many elements of the list the rule counts under `name`, bounded below, where the rule's
    /// range for the count starts above zero (rulec's §15.314). The top of that range is `Length`.
    Count { name: String, over: String, min: i128 },
    /// How many elements the list may have.
    Length { sequence: String, max: i128 },
    /// No two elements of the list with the same value of `field` (rulec's §15.290).
    Unique { sequence: String, field: String },
    /// A date input that takes only the days of a koyomi date (`range from koyomi`, rulec's
    /// §15.174): the file and the date as the rule names them, and the days.
    Days { input: String, file: String, date: String, days: crate::DaySet },
    /// `never <conditions>`: a combination of values the rule says does not happen, which the code
    /// generated from it refuses (rulec's §15.282). The line as the rule writes it, and what it
    /// tests. The precondition is that not all of these hold at once. A line on `elements`
    /// summaries alone is not here: nothing a caller gives reaches them.
    Never { said: String, conds: Vec<NeverCond> },
    /// `define <name> … range <range>`: a value the rule works out, which the code generated from
    /// it refuses a call for when it falls outside the range written for it (rulec's §15.313). The
    /// range as the rule writes it (the value is never on the wire); the inputs, and the fields of
    /// one element, it is worked out from, in the order the rule declares them; and, for a value
    /// worked out on each element of a list, that list.
    Define { name: String, range: String, from: Vec<String>, over: Option<String> },
}

/// One condition of a `never` line.
#[derive(Clone, Debug, PartialEq)]
pub struct NeverCond {
    pub name: String,
    pub test: NeverTest,
}

/// What a condition of a `never` line tests, on the wire.
#[derive(Clone, Debug, PartialEq)]
pub enum NeverTest {
    /// One of these values, or with `yes` false, none of them.
    Among { values: Vec<Value>, yes: bool },
    /// All of these comparisons (`<=`, `<`, `>=`, `>`) against a whole number on the wire.
    Compare(Vec<(String, i128)>),
    /// A test only the rule's own door reads: `= none`, `starts_with`, a comparison of dates.
    Other,
}

impl NeverTest {
    /// Whether a value given meets the test; None for a test only the rule's own door reads.
    pub fn holds(&self, v: &Value) -> Option<bool> {
        match self {
            NeverTest::Among { values, yes } => Some(values.contains(v) == *yes),
            NeverTest::Compare(ops) => Some(match v {
                Value::Int(n) => ops.iter().all(|(op, k)| match op.as_str() {
                    "<=" => n <= k,
                    "<" => n < k,
                    ">=" => n >= k,
                    ">" => n > k,
                    _ => false,
                }),
                _ => false,
            }),
            NeverTest::Other => None,
        }
    }
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
    /// The fields of one element of each list the rule walks, by the list's name, in the order
    /// the rule declares them (rulec's §15.219).
    pub elements: Vec<(String, Vec<WireField>)>,
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
    /// The fewest and the most: the range the output's answers lie in, the facts' (rulec's
    /// §15.271); None at an end it has none for.
    pub min: Option<i128>,
    pub max: Option<i128>,
    /// Each number the output comes to, when every row that decides it writes one; None when
    /// some row computes it.
    pub values: Option<Vec<i128>>,
    /// A value the output comes to with an input that reaches it, from rulec's vectors: the two
    /// ends first, then the values of the rows, then the fewest and the most the vectors come to
    /// (rulec's §15.270). A value is given once, and one no vector reaches is left out.
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
    /// An enum's value, by the rule's name of it (by its public name in what
    /// `Rules::outputs_over` answers, `EnumValue::public`).
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
    /// An input the generated code refuses at its door: not of its type, outside its range (a
    /// number's or a date's, or the values an enum's range lets in), or breaking a precondition;
    /// and what it refuses as an input while it walks a list: two elements with the same value of
    /// a `unique` field, weights of a pick by weight whose total passes int64, a total or a count
    /// outside its range, and a drawn value not below the total of the weights.
    Input(Text),
    /// No row answers, or two answer differently (a rule that passes its check never does this
    /// for an input it takes), or two elements both take under a `take_unique`.
    Contradiction(Text),
}

/// A value the rule works out that falls outside the range it accepts for it, where rulec's
/// reference evaluator refuses a call for it ([`Rules::eval_outside`]): what a caller reads to tell
/// this refusal from the others at the door, and which value it is, by what the evaluator found,
/// not by its words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outside {
    /// A value of a `define … range` (`Precondition::Define`): its name, and for one worked out on
    /// each element of a list, the list's name and the element it falls outside on, counted from 0
    /// as the list on the wire counts it (rulec's §15.324). One step is all a place can be: each
    /// field of an element is one value, and a rule declares its lists at its top, none within an
    /// element (rulec's §3.3).
    Define { name: String, at: Option<(String, usize)> },
    /// A total or a count over a list (`Precondition::Sum`, `Precondition::Count`) that ends
    /// outside the range the rule writes for it.
    Summary { name: String },
}

impl Outside {
    /// The value as a sentence names it: `subtotal`, or at its place, `lines[1].subtotal`.
    pub fn said(&self) -> String {
        match self {
            Outside::Define { name, at: Some((over, index)) } => format!("{over}[{index}].{name}"),
            Outside::Define { name, at: None } | Outside::Summary { name } => name.clone(),
        }
    }

    /// What `Rules::eval` refuses the call with: the same words wherever it is said.
    pub fn text(&self) -> Text {
        let name = self.said();
        match self {
            Outside::Define { .. } => ritsu_base::tr!("計算した `{name}` が、受け付ける範囲の外です", "the `{name}` it works out is outside the range the rule accepts for it"),
            Outside::Summary { .. } => ritsu_base::tr!("並びから求めた `{name}` が、範囲の外です", "the `{name}` worked out over a list is outside its range"),
        }
    }
}

/// What rulec answers for a rule (DESIGN 3.2). `rule` is the rule's file, as the caller reaches
/// it.
pub trait Rules {
    /// What rulec knows of the rule, when it passes check; else what check says.
    fn facts(&self, rule: &Path) -> Result<RuleFacts, Vec<Said>>;

    /// Whether each precondition holds for every value the inputs may take, each input (or a
    /// field of the list the rule walks) within the range given (the integers on the wire), or
    /// the rule's own range for one left out (DESIGN 7.4, X2); and the list the rule walks no
    /// shorter than `min_len` and no longer than `max_len`, when the caller knows how short and
    /// how long it can be (None: not known). A bound on the list's total or length is not decided
    /// without `max_len`, and the bottom of a total (rulec's §15.314) not without `min_len`: the
    /// least a total comes to is the shortest list with every element at the bottom of its range.
    fn preconditions_hold(&self, rule: &Path, ranges: &[(String, Option<i128>, Option<i128>)], min_len: Option<i128>, max_len: Option<i128>) -> Result<Vec<(Precondition, Answer<Values>)>, Vec<Said>>;

    /// As [`Rules::preconditions_hold`], for a caller that also knows every value it gives some
    /// inputs that are truth values or values of an enum (`values`: the input, and the values it
    /// can be, an enum's by the rule's names of them). A `never` line is decided over them too
    /// (X2): a condition none of an input's values meets keeps the line, and one every one of them
    /// meets is met, as a number's is inside a closed range. Values that are not the input's own
    /// say nothing of it. The default does not read them.
    fn preconditions_hold_given(
        &self,
        rule: &Path,
        ranges: &[(String, Option<i128>, Option<i128>)],
        values: &[(String, Vec<Value>)],
        min_len: Option<i128>,
        max_len: Option<i128>,
    ) -> Result<Vec<(Precondition, Answer<Values>)>, Vec<Said>> {
        let _ = values;
        self.preconditions_hold(rule, ranges, min_len, max_len)
    }

    /// What a numeric output comes to over every input the rule takes (DESIGN 7.6, X4): its
    /// fewest and most, and, where every row that decides it writes a number, each of those
    /// numbers with an input that reaches it.
    fn output_values(&self, rule: &Path, output: &str) -> Result<crate::Found<OutputValues>, Vec<Said>>;

    /// Each value the output `output` (an enum or a bool) can come to when the inputs named in
    /// `ranges` are held to them (by name, as the integers on the wire, both ends in; None at an
    /// open end, where the rule's own range holds) and every other input takes its whole range,
    /// each with an input that reaches it (sekisho's DESIGN 3.2: the computed values a policy
    /// reads, counted over the intervals its conditions cut and the values of the enums and the
    /// bools it reads). An enum input is held by the places of its values, from 0 in the order
    /// `RuleFacts::enums` lists them, and a bool input as 0 for false and 1 for true; an enum or a
    /// bool input named more than once takes the values of each. An enum input that declares a
    /// range of its own takes only the values inside it (`ColumnType::Enum`'s `range`), whatever
    /// places it is held to, as a number takes only its own range. An input that may be left out
    /// is given when it is named in `ranges`: what the rule comes to with it left out is not among
    /// the answers, whatever the ends (a caller that may leave it out does not name it). An enum's
    /// value is its public name
    /// (`EnumValue::public`, what a gate writes into Cedar); the input is as `eval` takes it.
    /// Exact, or undecided with the reason: a value the rule's own analysis cannot rule out and
    /// no input is found to reach. An empty list when no input is inside the ranges. The default
    /// decides nothing, for a port that does not analyse rules.
    fn outputs_over(&self, rule: &Path, output: &str, ranges: &[(String, Option<i128>, Option<i128>)]) -> Result<crate::Found<Vec<(Value, Values)>>, Vec<Said>> {
        let _ = (rule, output, ranges);
        Ok(crate::Found::Undecided(ritsu_base::tr!(
            "この口は、入力を範囲に限ったときに出力がとる値を答えません",
            "this port does not say what an output comes to over ranges of the inputs"
        )))
    }

    /// Each combination of values the outputs `outputs` (each an enum or a bool) can come to in one
    /// call, the inputs held as `outputs_over` takes them: the values in the order of `outputs`,
    /// each combination with an input that reaches it (sekisho's DESIGN 4.1: two values a gate
    /// takes from one call of a rule are counted together, so that a pair the rule never gives is
    /// not counted). Exact — every combination listed happens, and no other does — or undecided
    /// with the reason: a combination the rule's own analysis cannot rule out and no input is found
    /// to reach (two outputs decided by two tables, where the rows of the one say nothing of the
    /// rows of the other). An empty list when no input is inside the ranges. The default decides
    /// nothing, for a port that does not analyse rules.
    fn outputs_together(&self, rule: &Path, outputs: &[String], ranges: &[(String, Option<i128>, Option<i128>)]) -> Result<crate::Found<Vec<(Vec<Value>, Values)>>, Vec<Said>> {
        let _ = (rule, outputs, ranges);
        Ok(crate::Found::Undecided(ritsu_base::tr!(
            "この口は、入力を範囲に限ったときに二つ以上の出力が一緒にとる値を答えません",
            "this port does not say what several outputs come to together over ranges of the inputs"
        )))
    }

    /// Whether the rule's tables are complete, without overlap and without a row nothing
    /// reaches, when the date input `input` takes only the days in `days` (DESIGN 7.5 (b), X3).
    fn checked_over(&self, rule: &Path, input: &str, days: &crate::DaySet) -> Result<Answer<Text>, Vec<Said>>;

    /// What `output_values` answers for a numeric output when the inputs named in `ranges` are
    /// held to them, as `outputs_over` takes them (the integers on the wire, an enum by the places
    /// of its values, a bool as 0 and 1, an input named more than once taking the values of each),
    /// and every other input takes its whole range (DESIGN 7.6, X4: what a flow can give the call
    /// whose output it gives a transfer). With no input held to less than the rule declares, it is
    /// `output_values`'s answer. Otherwise the fewest and the most are what decides the output can
    /// come to there (not a range written on the output's line, which is said of every input), the
    /// numbers are those of the rows an input there may reach, and the examples are inputs inside
    /// the ranges from the rule's vectors, taken as `output_values` takes them. Undecided with the
    /// reason where no input is inside the ranges, or where a vector inside them comes to a value
    /// outside what the analysis says. The default decides nothing, for a port that does not
    /// analyse rules.
    fn output_values_over(&self, rule: &Path, output: &str, ranges: &[(String, Option<i128>, Option<i128>)]) -> Result<crate::Found<OutputValues>, Vec<Said>> {
        let _ = (rule, output, ranges);
        Ok(crate::Found::Undecided(ritsu_base::tr!(
            "この口は、入力を範囲に限ったときに数の出力がとる値を答えません",
            "this port does not say what a numeric output comes to over ranges of the inputs"
        )))
    }

    /// The outputs for these inputs (rulec's reference evaluator), refusing what the generated
    /// code refuses at its door, an enum's value outside the range its input declares among it
    /// (`RuleError::Input`).
    fn eval(&self, rule: &Path, inputs: &Values) -> Result<Values, RuleError>;

    /// [`Rules::eval`], with the value the evaluator refuses the call for when it is one the rule
    /// works out outside the range it accepts for it ([`Outside`]); None beside any other refusal.
    /// The default says none, for a port whose evaluator does not say which.
    fn eval_outside(&self, rule: &Path, inputs: &Values) -> Result<Values, (RuleError, Option<Outside>)> {
        self.eval(rule, inputs).map_err(|e| (e, None))
    }

    /// The inputs of the rule's vectors, as `rulec vectors` makes them: for each class of values
    /// the rule's tables tell apart, an input at its ends that keeps the rule's preconditions, as
    /// `eval` takes it, in the order the vectors come. dandori's scenarios choose among them the
    /// values a call gives that are left open, so that a run comes to each answer the rule can give
    /// there (its DESIGN 36). The default makes none, for a port that does not make vectors.
    fn vector_inputs(&self, rule: &Path) -> Result<Vec<Values>, Vec<Said>> {
        let _ = rule;
        Ok(Vec::new())
    }

    /// The page for people, as `rulec doc` draws it — Markdown, or with `html` the page that
    /// tries a case — in `lang`, naming the file `shown` (dandori names the file alone).
    fn doc(&self, rule: &Path, shown: &str, html: bool, lang: Lang) -> Result<String, Vec<Said>>;

    /// What the rule's answers rest on, as one string ([`crate::rests_on`]): its text, and every
    /// file its check read with what the file held (what it imports and applies, the contributions
    /// to it, the days of a koyomi date). Two rules whose strings are equal answer alike, whatever
    /// files each was read through and wherever they are held (sekisho's diff takes a value two
    /// versions of a gate compute so as one, ritsu's DESIGN 17.9.4). The default is None, for a port
    /// that cannot say: then no two are taken to answer alike.
    fn rests_on(&self, rule: &Path) -> Option<String> {
        let _ = rule;
        None
    }

    /// The pin of the rule's meaning, as an `apply` of it holds the rule (`meaning:<16 hex digits>`,
    /// what `rulec source pin` writes): a language that lowers what it holds into a rule which
    /// applies this one writes it (ringi's DESIGN 6.4). None where the file does not read, and for a
    /// port that cannot say.
    fn meaning(&self, rule: &Path) -> Option<String> {
        let _ = rule;
        None
    }

    /// Every diagnostic `rulec check` gives the rule, in `lang`, each as `rulec check --format json`
    /// writes it (its code, its line, the inputs of an example, the rows, how to fix it) with its
    /// text: what a language that lowers what it holds into a rule reads to say the rule's
    /// diagnostics again in its own words, at its own lines (ringi's DESIGN 6.5). The finding's
    /// file is the rule as the caller named it. Err when the file cannot be read; the default
    /// decides nothing, for a port that does not check rules.
    fn findings(&self, rule: &Path, lang: Lang) -> Result<crate::Found<Vec<crate::Finding>>, Vec<Said>> {
        let _ = (rule, lang);
        Ok(crate::Found::Undecided(ritsu_base::tr!("この口は、規則の診断を渡しません", "this port does not hand over a rule's diagnostics")))
    }

    /// What two versions of a rule answer differently over every input either takes, as `rulec
    /// diff` finds it (its §15.122) and its JSON says it, each side read and checked through its own
    /// files ([`DiffSide`]); the walk held to `budget` cells, rulec's default when None. A language
    /// that lowers what it holds into a rule says it again in its own words (ringi's DESIGN 11.2).
    /// Err with what the check says of a side that cannot be read or does not pass, and when the two
    /// are not one rule; the default decides nothing, for a port that does not compare rules.
    fn diff(&self, old: &DiffSide, new: &DiffSide, budget: Option<usize>) -> Result<crate::Found<RuleDiff>, Vec<Said>> {
        let _ = (old, new, budget);
        Ok(crate::Found::Undecided(ritsu_base::tr!("この口は、規則の二つの版を比べません", "this port does not compare two versions of a rule")))
    }

    /// Whether rulec is joined at all: false for the port the binary of a receiving language's own
    /// crate holds, which reads no rule (ritsu's DESIGN 2.3), so that the language can say so once,
    /// in its own code, rather than for each rule it asks of.
    fn joined(&self) -> bool {
        true
    }
}

// ── What two versions of a rule answer differently ──────────────────────

/// One side of [`Rules::diff`]: the rule, and the files it is read through — an overlay a language
/// laid what it lowered in, a git revision, a module's store (ritsu's DESIGN 17.10); None reads the
/// files as they are.
#[derive(Clone)]
pub struct DiffSide {
    pub rule: std::path::PathBuf,
    pub files: Option<std::rc::Rc<dyn ritsu_base::fs::Files>>,
}

impl DiffSide {
    pub fn new(rule: impl Into<std::path::PathBuf>, files: Option<std::rc::Rc<dyn ritsu_base::fs::Files>>) -> DiffSide {
        DiffSide { rule: rule.into(), files }
    }
}

/// What two versions of a rule answer differently ([`Rules::diff`]), as `rulec diff --format json`
/// says it: the boxes of inputs where they answer differently, each with what each side answers;
/// the boxes the walk could not settle, each with why; and what the two declare differently. Each
/// box holds one [`DiffHeld`] for each of the [`columns`](RuleDiff::columns).
#[derive(Clone, Debug, PartialEq)]
pub struct RuleDiff {
    pub rule: String,
    pub old_version: String,
    pub new_version: String,
    /// The columns the boxes are drawn over, in the order the walk cuts them.
    pub columns: Vec<DiffColumn>,
    /// What the two declare differently: an input added or taken out, its type or range, an
    /// output's presence, type, step, rounding or range (the JSON's `domain`).
    pub declared: Vec<DiffDeclared>,
    pub changes: Vec<DiffChange>,
    /// The boxes the walk could not settle.
    pub unknown: Vec<DiffUnknown>,
    /// The cells of the walk: every one, those an input can reach, those the two answer alike,
    /// those they answer differently, those it could not settle, and those no input could be built
    /// for (which were never compared).
    pub cells: usize,
    pub feasible: usize,
    pub same: usize,
    pub differing: usize,
    pub unsettled: usize,
    pub unrealized: usize,
    /// The space is larger than the budget, and nothing was compared.
    pub over_budget: bool,
    /// Why the two cannot be compared cell by cell at all.
    pub blocked: Option<Text>,
}

impl RuleDiff {
    /// Whether anything differs: a box the two answer differently, or a declaration (`rulec diff`
    /// exits 1). A box the walk could not settle is not one.
    pub fn any(&self) -> bool {
        !self.changes.is_empty() || !self.declared.is_empty()
    }

    /// Whether "outside the boxes, every input gets the same answer" was earned: nothing over the
    /// budget, blocked, unsettled or unrealized.
    pub fn total(&self) -> bool {
        !self.over_budget && self.blocked.is_none() && self.unknown.is_empty() && self.unrealized == 0
    }
}

/// A column a box is drawn over.
#[derive(Clone, Debug, PartialEq)]
pub struct DiffColumn {
    pub name: String,
    pub kind: DiffKind,
    /// The ends of a run of a column of days are day numbers ([`crate::day_text`]).
    pub date: bool,
    /// A rate: the ends are fractions, 0.3 for 30%.
    pub rate: bool,
    /// The unit of its numbers as the rule writes it (`JPY`), empty for none.
    pub unit: String,
}

/// What a column of the walk is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffKind {
    /// An input a caller gives.
    Input,
    /// A value computed from the columns before it (`derive`, `define`, an output of a rule the
    /// rule applies).
    Derived,
    /// A total or a count over the sequence the rule walks.
    Walk,
}

/// What a box holds of one column.
#[derive(Clone, Debug, PartialEq)]
pub enum DiffHeld {
    /// Every value: the box does not narrow the column.
    Any,
    /// These values of an enum, `true` or `false`, or the strings that begin with them; with
    /// `other`, a string that begins with none of the words the rule's cells name too.
    Words { words: Vec<String>, other: bool },
    /// Runs of numbers or days, each with both ends inside; None where a run goes on to the
    /// column's own end.
    Runs(Vec<(Option<ritsu_units::Rat>, Option<ritsu_units::Rat>)>),
}

/// A box the two versions answer differently.
#[derive(Clone, Debug, PartialEq)]
pub struct DiffChange {
    /// What the box holds of each column.
    pub held: Vec<DiffHeld>,
    /// Each output, with what the old version and the new answer at the witness, as the wire writes
    /// them (`none` for an optional output that has no value).
    pub outputs: Vec<(String, String, String)>,
    /// Whether every input of the box moves the same way; false where the values come out of an
    /// expression, and the outputs are the witness's own.
    pub uniform: bool,
    /// The rows that answered, as (table, row), on each side.
    pub old_rows: Vec<(String, usize)>,
    pub new_rows: Vec<(String, usize)>,
    /// One input that shows it, each value as `rulec vectors` writes it.
    pub witness: Vec<(String, String)>,
    /// How many cells of the walk the box holds.
    pub cells: usize,
}

/// A box the walk could not settle, and why.
#[derive(Clone, Debug, PartialEq)]
pub struct DiffUnknown {
    pub held: Vec<DiffHeld>,
    pub why: Text,
}

/// One thing the two versions declare differently: `what` is the key of the JSON's `domain`
/// (`input_added`, `input_range`, `output_removed`, …), with the name and each side's words.
#[derive(Clone, Debug, PartialEq)]
pub struct DiffDeclared {
    pub what: String,
    pub name: String,
    pub old: Option<String>,
    pub new: Option<String>,
}
