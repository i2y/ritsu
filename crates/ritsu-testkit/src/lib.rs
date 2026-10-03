//! What the tests of ritsu's crates share (DESIGN 10.8): what koyomi, chobo, geas, yuen and
//! sakai each kept in `tests/common`, and dandori, koyomi and chobo in their `tests/doc.rs`.
//!
//! - [`tmp`]: a directory that removes itself, and the directories of test processes that
//!   ended without removing theirs (and the servers they left running).
//! - [`tools`]: finding a program: `RITSU_<TOOL>`, the crate's own variable, a place in the
//!   crate, the PATH.
//! - [`run`]: running a program with a time limit (macOS has no `timeout`).
//! - [`golden`]: comparing with a golden file, or writing it under `RITSU_BLESS` or the crate's
//!   own `<NAME>_BLESS`.
//! - [`skip`] and [`level`]: the `SKIP: <crate>: <reason>` line, `RITSU_SKIP_LOG`, and the three
//!   levels of `RITSU_TEST_LEVEL` (DESIGN 10.2, 10.3).
//! - [`pg`], [`tigerbeetle`], [`chrome`], [`mermaid`], [`http`]: what a test starts — a
//!   throwaway PostgreSQL cluster, one TigerBeetle replica, a headless Chrome, Mermaid in that
//!   Chrome, a small HTTP server — each stopped when its value is dropped.
//!
//! Nothing here depends on anything but std (DESIGN 3.1). Every crate takes it as a
//! dev-dependency only.

pub mod chrome;
pub mod golden;
pub mod http;
pub mod level;
pub mod mermaid;
pub mod pg;
pub mod run;
pub mod skip;
pub mod tigerbeetle;
pub mod tmp;
pub mod tools;

pub use golden::golden;
pub use level::{Level, Need, need, ready};
pub use run::{Ran, run};
pub use skip::skip;
pub use tmp::TempDir;

/// The crate whose tests are running: cargo gives the test process `CARGO_PKG_NAME`.
pub fn crate_name() -> String {
    std::env::var("CARGO_PKG_NAME").unwrap_or_else(|_| "?".to_string())
}

/// The prefix of the crate's own variables: `koyomi` gives `KOYOMI`, `ritsu-base` gives
/// `RITSU_BASE`.
pub fn crate_var_prefix() -> String {
    crate_name().to_ascii_uppercase().replace('-', "_")
}
