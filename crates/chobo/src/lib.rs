//! chobo: a small language for the things you count and move between accounts.
//!
//! A book (`.book`) declares units, accounts with their bounds, and the kinds of transfer
//! between them. chobo checks it, runs it in the reference interpreter, and generates the
//! scenarios that the outputs are matched against.

/// One sentence in Japanese and English, side by side: `tr!("日本語 {x}", "English {x}")`.
/// Neither language can be written without the other.
#[macro_export]
macro_rules! tr {
    ($ja:literal, $en:literal) => {
        $crate::diag::Text { ja: ::std::format!($ja), en: ::std::format!($en) }
    };
    ($ja:literal, $en:literal, $($arg:tt)+) => {
        $crate::diag::Text { ja: ::std::format!($ja, $($arg)+), en: ::std::format!($en, $($arg)+) }
    };
}

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
pub mod postgres;
pub mod render;
pub mod scenario;
pub mod scenarios;
pub mod syntax;
pub mod target;
pub mod witness;
