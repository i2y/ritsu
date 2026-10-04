//! What a file copies and pins of the sources it cites (DESIGN 4.6). rulec and koyomi keep the
//! copies of a law beside the file, an article at a time (`sources/law/<id>@<asof>/<element>.xml`),
//! and pin each with the first sixteen digits of its SHA-256; a document is copied whole and pinned
//! the same way. yuen borrows a source a rule or a calendar already pins rather than copying it a
//! second time (yuen's DESIGN 1.4, 3.3), and compares the copies of one article that two files
//! hold.

use crate::Said;
use std::path::Path;

/// A source a file declares: its name, the line it is declared on, and what it is.
#[derive(Clone, Debug, PartialEq)]
pub struct Source {
    pub name: String,
    pub line: usize,
    pub kind: SourceKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SourceKind {
    /// A law: its database (`egov` or `ecfr`), its ID, the day it is read as of (`YYYY-MM-DD`),
    /// and each article pinned, as it is written (`第142条`, `§1910.157`), with its pin.
    Law { db: String, id: String, asof: String, pins: Vec<(String, String)> },
    /// A document copied whole: its path from the file's directory, as written, where it is
    /// fetched from, and its pin.
    File { path: String, url: Option<String>, pin: Option<String> },
}

/// What a file's language answers of the sources it pins. `file` is the file, as the caller
/// reaches it.
pub trait Sources {
    /// The sources the file declares, when it passes its language's check (which holds every copy
    /// to its pin); else what the check says.
    fn sources(&self, file: &Path) -> Result<Vec<Source>, Vec<Said>>;
}
