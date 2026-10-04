//! The commands of `ritsu` as a library: what the binary runs (`src/main.rs`), and what the page
//! in the browser runs on the files the reader edits (ritsu-wasm, DESIGN 8.7), so that the two
//! answer alike.
//!
//! - [`check`]: `ritsu check` (DESIGN 8.1, 8.3, 8.4), writing to the writers it is given.
//! - [`explain`]: `ritsu explain`, from ritsu's ledger.
//! - [`cli`]: the table of ritsu's commands and flags, and the seven languages.

pub mod check;
pub mod cli;
pub mod explain;
