//! sakai: a small language for the part of a context map that can be checked against the
//! artifacts. The design is DESIGN.md; the plan it is built by is PLAN.md.

#[macro_use]
extern crate ritsu_base;

pub mod naming;
pub mod paths;
pub mod ast;
pub mod diag;
pub mod kw;
pub mod lex;
pub mod parse;
pub mod proto;
/// The crates of the map's Rust code, as Cargo says them (DESIGN 7.7).
pub mod cargo;
pub mod check;
/// OpenAPI and AsyncAPI documents (DESIGN 15).
pub mod contracts;
pub mod elements;
pub mod mapping;
pub mod model;
pub mod owners;
pub mod patterns;
/// ritsu's ports, as sakai answers them (ritsu's DESIGN 3.2).
pub mod ports;
pub mod refs;
pub mod resolve;
/// The checks of security: keys in a `.ctx`, servers that do not encrypt, operations and
/// channels that say no authentication (DESIGN 16).
pub mod security;
/// The `sakai` command, as a function.
pub mod run;
/// The languages sakai reads through ritsu's ports.
pub mod suite;
pub mod terms;
pub mod api;
pub mod build;
pub mod cli;
pub mod cml;
pub mod codes;
/// The page `sakai doc` writes (DESIGN 10).
pub mod doc;
