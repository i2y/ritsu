//! The port of claims (DESIGN 3.2, `Claims`): the claims a geas spec makes, and the record
//! `geas map` keeps of the lines each claim ran, which yuen's `affected` reads.

use crate::Said;
use std::path::Path;

/// A claim: its name, the line it starts on, and its steps as written.
#[derive(Clone, Debug, PartialEq)]
pub struct Claim {
    pub name: String,
    pub line: usize,
    pub steps: Vec<String>,
}

/// The record `geas map` wrote for a spec (`.geas/<stem>.map.jsonl`).
#[derive(Clone, Debug, PartialEq)]
pub struct MapRecord {
    /// The spec's path from the root the record was made at, and that root from the spec's
    /// directory.
    pub spec: String,
    pub root: String,
    pub claims: Vec<RecordClaim>,
    pub files: Vec<RecordFile>,
    pub ran: Vec<RecordRan>,
}

/// A claim as the record has it: how it ended in the mapped run, and the targets it started.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordClaim {
    pub name: String,
    pub status: String,
    pub targets: Vec<String>,
}

/// A file a runtime reported: its path from the root and its git blob.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordFile {
    pub path: String,
    pub blob: String,
}

/// The lines one claim ran in one file, through one target, as ranges (first, last).
#[derive(Clone, Debug, PartialEq)]
pub struct RecordRan {
    pub claim: String,
    pub target: String,
    pub file: String,
    pub lines: Vec<(usize, usize)>,
}

/// What a diff comes to for a spec's claims: geas's `affected` (geas's DESIGN 7.5), read from the
/// records of the lines each claim ran, each record held to the code it was made on.
#[derive(Clone, Debug, PartialEq)]
pub struct Affected {
    /// Each record read, and which side of the change it is of: `before`, `after`, `either` or
    /// `mixed`.
    pub records: Vec<(String, String)>,
    /// The claims the change touches, in the order of the spec.
    pub claims: Vec<Touched>,
    /// The changed lines no claim runs.
    pub unclaimed: Vec<Untouched>,
    /// The source files the diff deletes, each with whether a record knew it.
    pub deleted: Vec<(String, bool)>,
    /// The files of the diff that are not source code.
    pub outside: Vec<String>,
    /// The spec itself, when the diff changes it.
    pub spec_changed: Vec<String>,
    /// Whether the diff changes the spec's baseline.
    pub baseline_changed: bool,
}

/// A claim the change touches: its name, how it ended in the mapped run, and the lines of the
/// change it ran.
#[derive(Clone, Debug, PartialEq)]
pub struct Touched {
    pub name: String,
    pub status: String,
    pub lines: Vec<TouchedLines>,
}

/// Lines of one file the claim ran: the side of the change (`after`, `before`), the lines as
/// ranges (`26`, `3-5`), how they were attributed (`ran`, or `near` for a removed line read by
/// the lines around it), and the target whose every claim runs them, when they are startup code.
#[derive(Clone, Debug, PartialEq)]
pub struct TouchedLines {
    pub file: String,
    pub side: String,
    pub lines: String,
    pub how: String,
    pub startup: Option<String>,
}

/// Changed lines of one file that no claim runs, and why: `not run` (a runtime reported the file
/// and no claim ran them) or `not reported` (no runtime reported the file).
#[derive(Clone, Debug, PartialEq)]
pub struct Untouched {
    pub file: String,
    pub side: String,
    pub lines: String,
    pub why: String,
}

/// What geas answers for a spec. `file` is the spec, as the caller reaches it.
pub trait Claims {
    /// The claims, when the spec reads; else what geas says.
    fn claims(&self, file: &Path) -> Result<Vec<Claim>, Vec<Said>>;

    /// The record `geas map` wrote for the spec, where it writes it by default; None when there
    /// is none.
    fn map_record(&self, file: &Path) -> Result<Option<MapRecord>, Vec<Said>>;

    /// What the diff (its bytes, and the name to show it by) comes to for the spec's claims, read
    /// from `records` (the record beside the spec when none are given), with the paths of the
    /// diff and the records read from `root` (the nearest directory with a `.git` when None).
    /// When geas cannot answer — no record, a record of other code, a diff it cannot read — what
    /// it says.
    fn affected(&self, file: &Path, root: Option<&Path>, diff: &[u8], diff_shown: &str, records: &[String]) -> Result<Affected, Vec<Said>>;
}
