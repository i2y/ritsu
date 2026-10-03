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

/// What geas answers for a spec. `file` is the spec, as the caller reaches it.
pub trait Claims {
    /// The claims, when the spec reads; else what geas says.
    fn claims(&self, file: &Path) -> Result<Vec<Claim>, Vec<Said>>;

    /// The record `geas map` wrote for the spec, where it writes it by default; None when there
    /// is none.
    fn map_record(&self, file: &Path) -> Result<Option<MapRecord>, Vec<Said>>;
}
