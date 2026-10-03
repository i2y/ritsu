//! The words the five targets will not take as an identifier (E009, PLAN B.4). Every ASCII
//! alias becomes an identifier in each of them: a function, a parameter, a schema. An alias
//! that one of them refuses would stop that target's build, so `check` refuses it first.
//!
//! The words are ritsu's (`ritsu_emit::words`), each list from the standard that gives it:
//! TypeScript's are ECMAScript 2025's reserved words, the strict-mode reserved words, and the
//! names strict mode will not bind or that the globals already hold; Python's are 3.14.6's
//! keywords and soft keywords; Go's (go1.25) are its keywords and its predeclared identifiers,
//! since a parameter named like one would hide it from the body; Rust's (1.94, edition 2024)
//! are its strict and reserved keywords; PostgreSQL's (18.0) are the keywords that cannot name
//! a function or a parameter, and PL/pgSQL's reserved words, since the generated functions are
//! PL/pgSQL and their bodies name the parameters.

use ritsu_emit::words::{Words, go, postgresql, python, rust, typescript};

/// The targets, by the name a diagnostic gives them.
pub const TARGETS: &[(&str, Words)] =
    &[("TypeScript", typescript::NAMES), ("Python", python::NAMES), ("Go", go::NAMES), ("Rust", rust::NAMES), ("PostgreSQL", postgresql::NAMES)];

/// The targets that will not take `alias` as it is.
pub fn refusing(alias: &str) -> Vec<&'static str> {
    TARGETS.iter().filter(|(_, ws)| ws.contains(alias)).map(|(n, _)| *n).collect()
}
