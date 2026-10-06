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
//! - [`cedar`] and [`r#gen`]: the Cedar a file that passes compiles to, and the command that writes it;
//!   [`vectors`]: every combination, as the tests of `cedar run-tests`; [`api`]: what the file
//!   declares, as JSON; [`raw`]: the data of every combination for the code that builds the
//!   requests, with the answer of the reference evaluation (DESIGN 6.3).
//! - [`cli`] and [`run`]: the `sekisho` command, which the binary runs and so does `ritsu sekisho`.

#[macro_use]
extern crate ritsu_base;

pub mod api;
pub mod ast;
pub mod borders;
pub mod cedar;
pub mod cells;
pub mod check;
pub mod checks;
pub mod cli;
pub mod codes;
pub mod contracts;
pub mod diag;
/// `sekisho doc`: the page for people, as Markdown and as one HTML file (DESIGN 7).
pub mod doc;
pub mod eval;
/// `sekisho gen` (the file is `gen.rs`; `gen` is a keyword of Rust 2024).
pub mod r#gen;
pub mod kw;
pub mod lex;
pub mod model;
pub mod names;
pub mod parse;
/// The data the generated code is given for each combination, and the answer for it (DESIGN 6.3).
pub mod raw;
pub mod repros;
/// The `sekisho` command, as a function: the binary runs it, and so does `ritsu sekisho`.
pub mod run;
pub mod suite;
pub mod table;
pub mod types;
pub mod vectors;
pub mod walk;
