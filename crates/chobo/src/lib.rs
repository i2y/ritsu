//! chobo: a small language for the things you count and move between accounts.
//!
//! A book (`.book`) declares units, accounts with their bounds, and the kinds of transfer
//! between them. chobo checks it, runs it in the reference interpreter, and generates the
//! scenarios that the outputs are matched against.

/// `tr!("日本語 {x}", "English {x}")`, one sentence in Japanese and English side by side, is
/// ritsu-base's: neither language can be written without the other.
#[macro_use]
extern crate ritsu_base;

pub mod api;
pub mod check;
pub mod client;
pub mod codes;
pub mod diag;
pub mod diffbase;
pub mod doc;
pub mod draw;
pub mod ids;
pub mod interp;
pub mod model;
pub mod parse;
pub mod ports;
pub mod postgres;
pub mod render;
/// The `chobo` command, as a function: the binary runs it, and so does `ritsu chobo`.
pub mod run;
pub mod scenario;
pub mod scenarios;
pub mod syntax;
pub mod target;
pub mod witness;
