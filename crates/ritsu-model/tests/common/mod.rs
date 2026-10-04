//! What the tests of this crate share: a koyomi file written as `KoyomiModel.readFile` reads it,
//! for the comparison of koyomi's dates (`koyomi.rs`) and of the days the checks across the
//! languages read (`cross.rs`).

use koyomi::ast::Conv;
use koyomi::calendar::{Calendar, Reason};
use koyomi::date::{Day, Missing};
use koyomi::resolve::{A, Model, ROp, Start};
use koyomi::vectors;
use serde_json::{Value, json};

fn a(x: &A) -> Value {
    match x {
        A::Lit(n) => json!({"lit": n}),
        A::Input(k) => json!({"input": k}),
    }
}

fn missing(m: &Option<Missing>) -> Value {
    match m {
        Some(Missing::EndOfMonth) => json!("end_of_month"),
        Some(Missing::StartOfNextMonth) => json!("start_of_next_month"),
        Some(Missing::Reject) => json!("reject"),
        Some(Missing::Never) | None => Value::Null,
    }
}

fn op(o: &ROp) -> Value {
    match o {
        ROp::Days(s, n) => json!({"days": {"sign": s, "n": a(n)}}),
        ROp::Business(f, n) => json!({"business": {"forward": f, "n": a(n)}}),
        ROp::Months { sign, n, per, missing: m } => json!({"months": {"sign": sign, "n": a(n), "per": per, "missing": missing(m)}}),
        ROp::DayOfMonth { n, sign, k, missing: m } => json!({"day_of_month": {"n": a(n), "sign": sign, "k": a(k), "missing": missing(m)}}),
        ROp::StartOfMonth(s, k) => json!({"start_of_month": {"sign": s, "k": a(k)}}),
        ROp::EndOfMonth(s, k) => json!({"end_of_month": {"sign": s, "k": a(k)}}),
        ROp::CloseDay(n, m) => json!({"close_day": {"n": a(n), "missing": missing(m)}}),
        ROp::CloseEndOfMonth => json!({"close_end_of_month": true}),
        ROp::Roll(c) => json!({"roll": match c {
            Conv::Following => "following",
            Conv::Preceding => "preceding",
            Conv::ModifiedFollowing => "modified_following",
            Conv::ModifiedPreceding => "modified_preceding",
        }}),
        ROp::IfClosed(inner) => json!({"if_closed": op(inner)}),
    }
}

/// A calendar as `KoyomiModel.readRules` reads it. The days its tables close are read back
/// through `reasons`, over its data: a calendar that reads no table has none.
pub fn calendar(c: &Calendar) -> Value {
    let span = |s: &koyomi::calendar::Stretch| json!([s.from.0, s.to.0]);
    let mut holidays = Vec::new();
    if !c.tables.is_empty() {
        for z in c.data.0.0..=c.data.1.0 {
            if c.reasons(Day(z)).iter().any(|r| matches!(r, Reason::Holiday { .. })) {
                holidays.push(z);
            }
        }
    }
    json!({
        "data": [c.data.0.0, c.data.1.0],
        "offset": c.offset,
        "weekly": c.weekly,
        "every": c.every.iter().map(|e| json!([e.from.0, e.from.1, e.to.0, e.to.1])).collect::<Vec<_>>(),
        "days": c.days.iter().map(span).collect::<Vec<_>>(),
        "opens": c.opens.iter().map(span).collect::<Vec<_>>(),
        "holidays": holidays,
    })
}

/// A dates file as `KoyomiModel.readFile` reads it.
pub fn dates(m: &Model) -> Value {
    json!({
        "kind": "dates",
        "inputs": m.inputs.iter().enumerate().map(|(k, i)| json!({"lo": i.lo, "hi": i.hi, "taken": m.dates.iter().any(|d| d.params.contains(&k))})).collect::<Vec<_>>(),
        "date_input": m.date_input,
        "dates": m.dates.iter().map(|d| json!({
            "start": match d.start { Start::Input => json!({"input": true}), Start::Date(s) => json!({"date": s}) },
            "ops": d.ops.iter().map(|o| op(&o.op)).collect::<Vec<_>>(),
            "at": d.at.map(|(at, _)| vectors::at_minutes(at)),
        })).collect::<Vec<_>>(),
        "order": m.order,
        "calendar": m.cal.as_ref().map(calendar),
    })
}
