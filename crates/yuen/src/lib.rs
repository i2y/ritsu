//! yuen: a small language for where requirements come from — the sources they are read
//! from, who decided them and why, what meets them and what checks them, with the hashes that
//! stop the check when any of it moves. The design is DESIGN.md; the plan it is built by is
//! PLAN.md.

#[macro_use]
extern crate ritsu_base;

pub mod affected;
pub mod api;
pub mod ast;
pub mod check;
pub mod cli;
pub mod codes;
pub mod copies;
pub mod coverage;
pub mod date;
pub mod diag;
pub mod diff;
pub mod ends;
pub mod export;
pub mod fetch;
pub mod graph;
pub mod kw;
pub mod lex;
pub mod marks;
pub mod names;
pub mod parse;
/// ritsu's ports, as yuen answers them (ritsu's DESIGN 3.2).
pub mod ports;
pub mod project;
pub mod proto;
pub mod review;
/// The `yuen` command, as a function.
pub mod run;
pub mod sources;
/// The languages yuen reads through ritsu's ports.
pub mod suite;
pub mod trace;
