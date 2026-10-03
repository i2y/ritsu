//! The one type of units the languages of ritsu share (DESIGN 5). rulec's way of writing a unit
//! is the ground: a dimension and a unit (`mass[kg]`), money in a currency with its tax
//! (`money[円, incl_tax]`), a rate with its step (`rate[step 0.1%]`), and a number with no unit.
//!
//! - [`Rat`]: an exact rational, which every conversion is done in.
//! - [`table`]: the closed table of currencies and units, each with its dimension and its exact
//!   factor ([`table::unit`]); ℉'s offset ([`table::offset`]); the ISO 4217 codes
//!   ([`table::CURRENCIES`]).
//! - [`Unit`]: a unit as written, with its [`Dim`], its [`Tax`] and its step. It answers whether
//!   two units are the same ([`Unit::same`]: `JPY` is `円`), whether a value converts exactly and
//!   to a whole number ([`Unit::convert`], [`Unit::whole`]: `1lb` is no whole number of grams),
//!   whether a dimension is ordered only ([`Dim::compares_only`]), and the table's own spelling
//!   ([`Unit::canonical`]).
//!
//! The rules of arithmetic on units — that grams are not added to yen, that a date takes no sum,
//! that an output is rounded — are each language's checks, and stay with the languages.

mod rat;
pub mod table;
mod unit;

pub use rat::Rat;
pub use unit::{Dim, Problem, Tax, Unit};
