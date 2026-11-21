//! Numbers as the checks' sentences say them (DESIGN 6.2 item 8, 7.1): each value of a language
//! with the unit and the digits it is written with ([`ritsu_units::written`]), the unit taken from
//! the side that writes the value in the sentence — a rule's input or output, a book's parameter.
//! A field of the JSON a program reads (a border's `about`, `takes`, `gives`, `example`) keeps the
//! whole number on the wire.

use ritsu_ports::{day_text, BookFacts, ColumnType, RuleFacts, Value};
use ritsu_units::written::{self, End};
use ritsu_units::Unit;

/// The unit of a numeric type (through `T?`); None for any other, and for a spelling the table of
/// units does not have.
pub(crate) fn unit_of(ty: &ColumnType) -> Option<&Unit> {
    match ty {
        ColumnType::Num { unit, .. } => unit.as_ref(),
        ColumnType::Opt(t) => unit_of(t),
        _ => None,
    }
}

/// The unit of a rule's input `name`, or of a field of the elements of a list it walks (the column
/// a total adds up).
pub(crate) fn input_unit(facts: &RuleFacts, name: &str) -> Option<Unit> {
    facts.inputs.iter().chain(facts.elements.iter().flat_map(|(_, _, cols)| cols.iter())).find(|c| c.name == name).and_then(|c| unit_of(&c.ty).cloned())
}

/// The unit of a rule's output `name`.
pub(crate) fn output_unit(facts: &RuleFacts, name: &str) -> Option<Unit> {
    facts.outputs.iter().find(|c| c.name == name).and_then(|c| unit_of(&c.ty).cloned())
}

/// The unit of an amount parameter of a book's transfer, as ritsu's languages share it (`unit USD
/// scale 2` is `money[USDc]`): what an amount given to it is counted in.
pub(crate) fn param_unit(facts: &BookFacts, transfer: &str, param: &str) -> Option<Unit> {
    let t = facts.transfers.iter().find(|t| t.name == transfer)?;
    let p = t.params.iter().find(|p| p.name == param)?;
    facts.unit(p.unit.as_deref()?).cloned()
}

/// A value on the wire as a sentence says it: a number in `unit` (`10_000JPY`, `0.5%`, `5000`).
pub(crate) fn value(v: &Value, unit: Option<&Unit>) -> String {
    match v {
        Value::Int(n) => written::value(*n, unit),
        Value::Bool(b) => b.to_string(),
        Value::Str(s) => format!("\"{s}\""),
        Value::Date(d) | Value::Enum(d) => d.clone(),
        Value::None => "none".into(),
        Value::List(xs) => format!("[{} elements]", written::digits(xs.len() as i128)),
    }
}

/// [`value`] as a sentence in one language says it: a list by how many elements it has, in that
/// language (`[0 件]`, `[0 elements]`).
pub(crate) fn value_in(v: &Value, unit: Option<&Unit>, ja: bool) -> String {
    match v {
        Value::List(xs) if ja => format!("[{} 件]", written::digits(xs.len() as i128)),
        other => value(other, unit),
    }
}

/// A range on the wire as a sentence says it: a number's ends in `unit` (`>=1JPY <=10_000JPY`), a
/// date's day numbers as days (`>=2026-01-01 <=2026-12-31`); `any` with neither end.
pub(crate) fn range(r: (Option<i128>, Option<i128>), date: bool, unit: Option<&Unit>) -> String {
    let text = if date {
        written::range(r.0.map(End::Closed), r.1.map(End::Closed), |v| i64::try_from(v).map(day_text).unwrap_or_else(|_| written::digits(v)))
    } else {
        written::between(r.0, r.1, unit)
    };
    if text.is_empty() { "any".into() } else { text }
}

/// What a rule's inputs are given, as a sentence says it, each in the input's unit: `asked =
/// 10_000JPY, paid = 5000JPY`. `facts` is None where the rule does not answer.
pub(crate) fn inputs(facts: Option<&RuleFacts>, values: &[(String, Value)]) -> String {
    values.iter().map(|(n, v)| format!("{n} = {}", value(v, facts.and_then(|f| input_unit(f, n)).as_ref()))).collect::<Vec<_>>().join(", ")
}
