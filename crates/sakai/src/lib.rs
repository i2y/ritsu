//! sakai: a small language for the part of a context map that can be checked against the
//! artifacts. The design is DESIGN.md; the plan it is built by is PLAN.md.

#[macro_use]
pub mod i18n;

pub mod naming;
pub mod paths;
pub mod sha256;
pub mod ast;
pub mod diag;
pub mod kw;
pub mod lex;
pub mod parse;
pub mod proto;
pub mod check;
pub mod elements;
pub mod mapping;
pub mod model;
pub mod owners;
pub mod patterns;
pub mod refs;
pub mod resolve;
pub mod terms;
pub mod api;
pub mod build;
pub mod cli;
pub mod cml;
pub mod codes;
