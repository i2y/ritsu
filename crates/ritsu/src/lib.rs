//! The commands of `ritsu` as a library: what the binary runs (`src/main.rs`), and what the page
//! in the browser runs on the files the reader edits (ritsu-wasm, DESIGN 8.7), so that the two
//! answer alike.
//!
//! - [`check`]: `ritsu check` (DESIGN 8.1, 8.3, 8.4), writing to the writers it is given.
//! - [`explain`]: `ritsu explain`, from ritsu's ledger.
//! - [`cli`]: the table of ritsu's commands and flags, and the seven languages.
//! - [`languages`]: `ritsu dandori`, with the languages a flow reads joined.
//! - [`run`]: `ritsu run` (DESIGN 7.9), a workflow run with its rules, dates and books computed.

pub mod check;
pub mod cli;
pub mod explain;
pub mod languages;
pub mod run;
