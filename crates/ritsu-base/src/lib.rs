//! The ground the seven languages of ritsu stand on (DESIGN 4): what each of them wrote for
//! itself before, written once. None of it knows what a rule, a date, an account, a workflow,
//! a claim, a requirement or a context is; each module is a tool a language uses.
//!
//! - [`text`]: a sentence in Japanese and English side by side (`tr!`), the language a run
//!   prints in, and the widths and spaces of Japanese text.
//! - [`diag`]: a diagnostic — its code, place, message, notes and fix — as text and as JSON,
//!   with a part of its own for each language ([`diag::Extra`]).
//! - [`ledger`]: the entries of `explain`, each with the smallest input that prints its code.
//! - [`cli`]: one table of commands and flags, which `--help` is drawn from and the command
//!   line is read against.
//! - [`sha256`]: FIPS 180-4.
//! - [`naming`] and [`paths`]: `<tool> "<path>" [<kind> <name>]...` (DESIGN 6.2), the root of a
//!   project, and paths from it and from where a tool runs.
//! - [`sources`]: the copies of a law's articles, their text, their pins, and the requests to
//!   e-Gov and the eCFR that bring them.
//! - [`docpage`]: the frame of an approver's page — the HTML head and the palette.
//! - [`json`]: a JSON value whose objects keep their order and whose integers are exact.
//! - [`udiff`]: unified diffs, as `git diff` and `diff -u` write them, and whether a file on disk
//!   is one side of one.
//!
//! Nothing here depends on anything but std (DESIGN 3.1, P9).

pub mod cli;
pub mod diag;
pub mod docpage;
pub mod json;
pub mod ledger;
pub mod naming;
pub mod paths;
pub mod sha256;
pub mod sources;
pub mod text;
pub mod udiff;
