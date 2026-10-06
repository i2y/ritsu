//! sekisho (関所): a small language for who may do what. A `.gate` says which principals may take
//! which actions on which resources, with roles, attributes and relations, and with the answers of
//! rulec's rules and koyomi's dates as conditions; sekisho checks it on every combination of the
//! values it declares, and generates Cedar, which evaluates it. The design is DESIGN.md.
//!
//! The parts, in the order a file goes through them:
//!
//! - [`lex`] and [`parse`]: a `.gate` into its syntax tree ([`ast`]), with the words of [`kw`].
//! - [`names`]: what each name refers to, its alias, and the types and units of the values
//!   ([`types`]), reading the rules and the dates files through the ports of [`suite`].
//! - [`check`]: a file from its text to its diagnostics ([`diag`], the codes of [`codes`]).
//! - [`cli`] and [`run`]: the `sekisho` command, which the binary runs and so does `ritsu sekisho`.

#[macro_use]
extern crate ritsu_base;

pub mod ast;
pub mod borders;
pub mod cells;
pub mod check;
pub mod checks;
pub mod cli;
pub mod codes;
pub mod contracts;
pub mod diag;
pub mod eval;
pub mod kw;
pub mod lex;
pub mod model;
pub mod names;
pub mod parse;
pub mod repros;
/// The `sekisho` command, as a function: the binary runs it, and so does `ritsu sekisho`.
pub mod run;
pub mod suite;
pub mod table;
pub mod types;
pub mod walk;
