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
//! - [`docpage`]: the frame of a page for people — the HTML head and the palette.
//! - [`json`]: a JSON value whose objects keep their order and whose integers are exact.
//! - [`udiff`]: unified diffs, as `git diff` and `diff -u` write them, and whether a file on disk
//!   is one side of one.
//! - [`yaml`]: YAML (the part of YAML 1.2 that goes to JSON and back) and JSON, read into values
//!   that know where they were written.
//! - [`fs`]: where a project's files are read and written: the disk, or files held in memory (the
//!   project a page in the browser hands over).
//! - [`openspec`]: OpenSpec's specs and changes, read as OpenSpec reads them: a spec's requirements
//!   with their blocks and scenarios, and what a change adds, modifies, removes and renames.
//! - [`cedar`]: Cedar's policies and schemas, read and written as Cedar 4.13.0 reads and writes
//!   them: the policy syntax and both schema formats, the JSON policy format, `cedar format`.
//! - [`secrets`], [`urls`] and [`marks`]: what the checks of security (DESIGN 16) share across
//!   the languages: the keys of a known shape written into a file, what a URL says of how it is
//!   reached (the loopback, plain HTTP, the encrypted form of a protocol), and the marks that make
//!   a schema's property secret.
//! - [`openapi`]: the operations of an OpenAPI or AsyncAPI document: each with its id, method and
//!   path, parameters, the fields of its body with their types and ranges, who may call it, and
//!   the status codes it answers with.
//!
//! Nothing here depends on anything but std (DESIGN 3.1, P9).

pub mod cedar;
pub mod cli;
pub mod diag;
pub mod docpage;
pub mod fs;
pub mod json;
pub mod ledger;
pub mod marks;
pub mod naming;
pub mod openapi;
pub mod openspec;
pub mod paths;
pub mod secrets;
pub mod sha256;
pub mod sources;
pub mod text;
pub mod udiff;
pub mod urls;
pub mod yaml;
