//! The one type of units the languages of ritsu share (DESIGN 5). rulec's way of writing a unit
//! is the ground: a dimension and a unit (`mass[kg]`), money in a currency with its tax
//! (`money[円, incl_tax]`), a rate with its step (`rate[step 0.1%]`), and a number with no unit.
//!
//! - [`Rat`]: an exact rational, which every conversion is done in.
//! - [`table`]: the closed table of currencies and units, each with its dimension and its exact
//!   factor ([`table::unit`]); ℉'s offset ([`table::offset`]); the ISO 4217 codes
//!   ([`table::CURRENCIES`]); the name generated code gives a unit's type ([`table::type_name`]:
//!   `kg` is `Kilogram`).
//! - [`chars`]: the characters a unit is written with, as rulec reads them, for every language's
//!   lexer: what may follow a number as its unit, and ℃ and ℉, which stand as a word of their own
//!   where a type takes its unit (`temperature[℃]`).
//! - [`Unit`]: a unit as written, with its [`Dim`], its [`Tax`] and its step. It answers whether
//!   two units are the same ([`Unit::same`]: `JPY` is `円`), whether a value converts exactly and
//!   to a whole number ([`Unit::convert`], [`Unit::whole`]: `1lb` is no whole number of grams),
//!   whether a dimension is ordered only ([`Dim::compares_only`]), the table's own spelling
//!   ([`Unit::canonical`]), the spelling the table gives the unit first ([`Unit::first_spelling`]:
//!   `mL` is `cm3`), and the name of its type in generated code ([`Unit::type_name`]).
//!
//! The rules of arithmetic on units — that grams are not added to yen, that a date takes no sum,
//! that an output is rounded — are each language's checks, and stay with the languages.

pub mod chars;
mod rat;
pub mod table;
mod unit;

pub use rat::Rat;
pub use unit::{Dim, Problem, Tax, Unit};
