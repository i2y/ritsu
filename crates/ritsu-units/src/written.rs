//! Numbers as the languages write them (ritsu's DESIGN 6.2): what a message puts in its text for a
//! value of a language. A value travels as a whole number — in its unit, a rate as the count of its
//! step, a book's amount in its unit's smallest step — and its author wrote it with its unit and its
//! digits in groups: `10_000_000JPY`, `0.5%`, `12.50USD`. A message says it the way it is written,
//! so that the reader does not count digits again or guess the unit (yen or sen), and can find it
//! in the file. A check that says a number of a language in a sentence writes it with these
//! functions, and keeps the whole number where a program reads it (a field of the JSON).
//!
//! - [`digits`]: a whole number with its digits grouped: `5000`, `10_000`, `-12_345`.
//! - [`decimal`]: a rational with its whole part so grouped: `12_345.5`, `0.5`, `1/3`.
//! - [`value`]: a whole number on the wire, in its unit: `10_000_000JPY`, `1250USDc`, `0.5%`, `5000`.
//! - [`amount`]: an amount of a book's unit with the unit's decimal places: `12.50USD`, `1000円`.
//! - [`range`] and [`between`]: a range with its ends so written: `>=1JPY <=100_000JPY`, `>0 <100`.
//!
//! The Japanese of a message writes them the same way: the digits as the languages take them, and
//! the unit as the file spells it (`円` where the rule writes `円`).

use crate::{Dim, Rat, Unit};

/// A whole number as the languages write one: from five digits on, in groups of three joined by
/// `_` (`10_000`, `-12_345`); four digits or fewer as they are (`5000`), as the corpus writes
/// `5000JPY` and `10_000JPY` and rulec writes an amount back.
pub fn digits(n: i128) -> String {
    let d = n.unsigned_abs().to_string();
    let sign = if n < 0 { "-" } else { "" };
    if d.len() < 5 {
        return format!("{sign}{d}");
    }
    let mut out = String::with_capacity(d.len() + d.len() / 3 + 1);
    out.push_str(sign);
    for (i, c) in d.chars().enumerate() {
        if i > 0 && (d.len() - i) % 3 == 0 {
            out.push('_');
        }
        out.push(c);
    }
    out
}

/// A rational as [`Rat`] writes it, with its whole part grouped as [`digits`] groups a whole number:
/// `12_345.5`, `0.5`, `-0.48`. A value no decimal writes stays the fraction `Rat` writes (`1/3`).
/// kaname's sentences write its values, which need not be whole, so.
pub fn decimal(r: Rat) -> String {
    let t = r.to_string();
    let (sign, body) = match t.strip_prefix('-') {
        Some(b) => ("-", b),
        None => ("", t.as_str()),
    };
    let (whole, frac) = match body.split_once('.') {
        Some((w, f)) => (w, Some(f)),
        None => (body, None),
    };
    match (whole.parse::<i128>(), frac) {
        (Ok(w), Some(f)) => format!("{sign}{}.{f}", digits(w)),
        (Ok(w), None) => format!("{sign}{}", digits(w)),
        _ => t,
    }
}

/// A whole number on the wire, in `unit`, as the languages write a value of its type:
///
/// - money and a quantity with the unit as the file spells it: `10_000_000JPY`, `1000円`, `1250USDc`,
///   `30_000g`, `25℃`;
/// - a rate as the percentage the count of its step comes to: `rate[step 0.1%]`'s 5 is `0.5%`; a
///   rate with no step counts the whole rate (what one integer counts, [`Unit::step`]), 1 is `100%`;
/// - a number, a count (a name and nothing else, as chobo's `unit seat`), and a value whose unit
///   nothing says (`None`: a spelling the table of units does not have) as its digits: `5000`, `-20`.
pub fn value(n: i128, unit: Option<&Unit>) -> String {
    let Some(u) = unit else { return digits(n) };
    match &u.dim {
        Dim::Number | Dim::Count(_) => digits(n),
        Dim::Rate => {
            let step = u.step.unwrap_or(Rat::int(1));
            match Rat::int(n).checked_mul(step).and_then(|r| r.checked_mul(Rat::int(100))) {
                Some(r) => format!("{}%", decimal(r)),
                // past what 128 bits hold: the count, which no wire carries
                None => digits(n),
            }
        }
        _ => format!("{}{}", digits(n), u.unit),
    }
}

/// An amount of a book's unit, counted in the unit's smallest step, as a unit of `places` decimal
/// places writes it, with the unit's name after it: `12.50USD`, `-0.48USD`, `1000円`,
/// `1_234_567.50USD` (chobo's `unit USD scale 2`). A count — a unit with a name and nothing else
/// (`unit seat`, or a unit of the table with decimal places of its own) — is digits only, as the
/// languages write a value of one.
pub fn amount(n: i128, places: u32, name: &str, unit: &Unit) -> String {
    let text = match 10u128.checked_pow(places) {
        Some(p) if places > 0 => {
            let a = n.unsigned_abs();
            let whole = i128::try_from(a / p).unwrap_or(i128::MAX);
            format!("{}{}.{:0width$}", if n < 0 { "-" } else { "" }, digits(whole), a % p, width = places as usize)
        }
        _ => digits(n),
    };
    match unit.dim {
        Dim::Count(_) | Dim::Number => text,
        _ => format!("{text}{name}"),
    }
}

/// One end of a range: a value the range takes (`>=`, `<=`), or one it stops short of (`>`, `<`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    Closed(i128),
    Open(i128),
}

/// A range as the languages write one, each end written by `show`: `>=1JPY <=100_000JPY`, `>=0%`,
/// `<=10_000`, `>0 <100`. Empty where it has neither end; what to say then is the sentence's.
pub fn range(lo: Option<End>, hi: Option<End>, show: impl Fn(i128) -> String) -> String {
    let mut parts = Vec::new();
    match lo {
        Some(End::Closed(n)) => parts.push(format!(">={}", show(n))),
        Some(End::Open(n)) => parts.push(format!(">{}", show(n))),
        None => {}
    }
    match hi {
        Some(End::Closed(n)) => parts.push(format!("<={}", show(n))),
        Some(End::Open(n)) => parts.push(format!("<{}", show(n))),
        None => {}
    }
    parts.join(" ")
}

/// A range of whole numbers on the wire whose ends it takes, in `unit` ([`value`]):
/// `>=1JPY <=100_000JPY`, `>=0 <=10_000`, `<=50%`. Empty where it has neither end.
pub fn between(lo: Option<i128>, hi: Option<i128>, unit: Option<&Unit>) -> String {
    range(lo.map(End::Closed), hi.map(End::Closed), |n| value(n, unit))
}
