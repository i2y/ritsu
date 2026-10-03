//! geas: hold agent-written code to claims a person has read. The command is `src/main.rs`; this
//! library is what it runs, and what ritsu's ports read of a spec (`ports`, ritsu's DESIGN 3.2).

#[macro_use]
extern crate ritsu_base;

pub mod affected;
pub mod cdp;
pub mod check;
pub mod codes;
pub mod cover;
pub mod diag;
pub mod diff;
pub mod drift;
pub mod driver;
pub mod gui;
pub mod hash;
pub mod http;
pub mod json;
pub mod lex;
pub mod lines;
pub mod map;
pub mod model;
pub mod parse;
pub mod pins;
pub mod pixie;
pub mod proc;
pub mod regex;
pub mod screen;
pub mod report;
pub mod run;
pub mod sched;
pub mod skill;
pub mod tree;
pub mod words;
pub mod ws;
/// ritsu's ports, as geas answers them (ritsu's DESIGN 3.2).
pub mod ports;
