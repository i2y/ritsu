//! koyomi: a small language for closing days, payment days, business days and month
//! arithmetic. The design is DESIGN.md; the plan it is built by is PLAN.md.

#[macro_use]
extern crate ritsu_base;

pub mod api;
pub mod ast;
pub mod calendar;
pub mod check;
pub mod cli;
pub mod codes;
pub mod date;
pub mod diag;
pub mod doc;
pub mod eval;
pub mod fetch;
pub mod codegen;
pub mod holidays;
pub mod interp;
pub mod kw;
pub mod lex;
pub mod naming;
pub mod paraphrase;
pub mod parse;
/// ritsu's ports, as koyomi answers them (ritsu's DESIGN 3.2).
pub mod ports;
pub mod reserved;
pub mod resolve;
/// The `koyomi` command, as a function: the binary runs it, and so does `ritsu koyomi`.
pub mod run;
pub mod sjis;
pub mod sjis_table;
pub mod sources;
pub mod vectors;
