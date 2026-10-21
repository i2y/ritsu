//! The characters a unit is written with (DESIGN 5.1), one reading for every language of ritsu that
//! writes units. rulec's lexer is the ground (rulec's reference §2):
//!
//! - Right after a number, a unit is a run that starts with an ASCII letter (a currency's code,
//!   `kg`), `円`, `銭`, `坪` or one of the signs `%`, `℃`, `℉` ([`is_unit_char`]), and goes on with
//!   those and ASCII digits ([`is_unit_tail`]: `m2`). The full-width `％` is `%`
//!   ([`table_spelling`]).
//! - Where a type takes its unit as a word (`temperature[℃]`), ℃ and ℉ stand as a word of their own
//!   ([`is_word_sign`]): Unicode files them as symbols, not as letters, so a lexer that starts a
//!   word only at a letter cannot read them there. `%` does not stand as a word: the type of a rate
//!   writes its step (`rate[step 1%]`), and a `%` with no number before it is a word of no language.
//!
//! What a language does with what follows a number and is no unit of the table (an error, or the
//! rest of a name, as sekisho's `3日以内`), and where it lets a sign stand as a word (dandori and
//! sekisho: right after a type's `[`; chobo: where a unit's name goes), stay with the language.

/// The signs a unit is written with that are no letters: `%` and the full-width `％`, `℃`, `℉`.
pub fn is_sign(c: char) -> bool {
    matches!(c, '%' | '％' | '℃' | '℉')
}

/// The signs that stand as a word of their own where a type takes its unit: `℃` and `℉`
/// (`temperature[℃]`).
pub fn is_word_sign(c: char) -> bool {
    matches!(c, '℃' | '℉')
}

/// A character a unit written right after a number starts with: an ASCII letter, `円`, `銭`, `坪`,
/// or a sign ([`is_sign`]).
pub fn is_unit_char(c: char) -> bool {
    c.is_ascii_alphabetic() || matches!(c, '円' | '銭' | '坪') || is_sign(c)
}

/// A character a unit goes on with after its first: those of [`is_unit_char`], and an ASCII digit
/// (`m2`, `km3`). A digit cannot start one: the digits before it are the number's.
pub fn is_unit_tail(c: char) -> bool {
    is_unit_char(c) || c.is_ascii_digit()
}

/// A unit as the table spells it: the full-width `％` is `%`, and every other spelling is the
/// table's as it is written.
pub fn table_spelling(written: &str) -> String {
    written.replace('％', "%")
}
