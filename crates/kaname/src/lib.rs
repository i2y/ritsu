//! kaname (要): a small general-purpose language for writing a functional core, the part of a program
//! that only computes and decides. A `.kn` module is functions over records, enums with fields, lists,
//! dicts, sets and strings, with generics, `result` and function values; it calls rulec's rules,
//! koyomi's dates and chobo's transfers in ritsu's types too, and every computation is checked by the
//! same rules of types, units, ranges, rounding and int64. It has no input or output: what it is
//! given, it computes from. The design is DESIGN.md.
//!
//! The parts, in the order a file goes through them:
//!
//! - [`lex`] and [`parse`]: a `.kn` into its syntax tree ([`ast`]), with the words of [`kw`].
//! - [`names`]: the files its `use` lines read, through the ports of [`suite`] (rules, dates files,
//!   books) or as modules of their own, and its declarations with their types ([`ty`]).
//! - [`types`]: every expression's type, and whether each value fits where it goes; [`tried`], each
//!   `let … = try …` read as the `match` it means once the types pass; [`wrapped`], each function
//!   written as a value read as the lambda that calls it.
//! - [`check`]: a file from its text to its diagnostics ([`diag`], the codes of [`codes`]), its
//!   `examples` run last ([`examples`]), and a key written in the file looked for in its whole text
//!   ([`security`]).
//! - [`ranges`]: the analysis of ranges, what each expression's values can be and whether each place
//!   where the generated code can fail is shown to keep what it takes; [`witness`], the search for an
//!   input that breaks a place that is not shown; [`effects`], what each function can end in; and
//!   [`hatch`], the warning of a decision written in code.
//! - [`interp`]: the reference interpreter, which says what a module means: its values
//!   ([`value`]), its numbers ([`num`]: exact, held to int64, rounded rulec's five ways), its
//!   dates ([`date`]) and its strings ([`text`]: code points); and [`vectors`], the inputs a function
//!   is tried with and its answers.
//! - [`cli`] and [`run`]: the `kaname` command, which the binary runs and so does `ritsu kaname`;
//!   [`api`], the module as the tools outside read it; [`doc`], the page for people.
//! - [`ports`]: kaname as `ritsu check` joins it, and as ritsu's checks across the borders ask it
//!   about a module's calls into the other languages (`Programs`), with their verdicts handed back
//!   through [`suite`] (`Verdicts`).
//! - `r#gen`: the module as TypeScript, Python and Go under the `glue/` of the package `ritsu gen`
//!   writes, checked where the analysis does not show a place; and the harness that holds that code
//!   to the reference interpreter.

#[macro_use]
extern crate ritsu_base;

pub mod api;
pub mod ast;
pub mod check;
pub mod cli;
pub mod codes;
pub mod date;
pub mod diag;
pub mod doc;
pub mod effects;
pub mod examples;
// The generated code (DESIGN 10): `r#gen`, as `gen` is a word of Rust 2024.
pub mod r#gen;
pub mod hatch;
pub mod interp;
pub mod kw;
pub mod lex;
pub mod names;
pub mod num;
pub mod parse;
pub mod ports;
pub mod ranges;
/// The `kaname` command, as a function: the binary runs it, and so does `ritsu kaname`.
pub mod run;
pub mod security;
pub mod suite;
pub mod text;
pub mod tried;
pub mod ty;
pub mod types;
pub mod value;
pub mod vectors;
pub mod witness;
pub mod wrapped;
