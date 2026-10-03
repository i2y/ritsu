//! What a file holds and what it names outside itself (DESIGN 6.4): every language gives both, by
//! the naming of DESIGN 6.2. A project's index is made of them (ritsu-project), yuen's ends are
//! items, and sakai checks references against the map.

use crate::Said;
use ritsu_base::naming::Name;
use std::path::Path;

/// One thing a file holds.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    /// The naming of it: the tool, the file from the root, and the pairs down to it
    /// (`rulec "rules/送料.rule" table 送料表`, `rulec "a.rule" enum 区分 value 一般`).
    pub naming: Name,
    /// The first and the last line it is written on, from 1.
    pub lines: (usize, usize),
    /// Its definition, the text yuen takes the hash of: what it is, each language says in its
    /// DESIGN.md, written so that what does not change its meaning (spacing, alignment) does not
    /// change the text.
    pub text: String,
}

impl Item {
    /// The kind of the thing (the last pair's kind).
    pub fn kind(&self) -> &str {
        self.naming.items.last().map(|(k, _)| k.as_str()).unwrap_or("")
    }

    /// The name of the thing (the last pair's name).
    pub fn name(&self) -> &str {
        self.naming.items.last().map(|(_, n)| n.as_str()).unwrap_or("")
    }
}

/// One reference a file makes to something outside it.
#[derive(Clone, Debug, PartialEq)]
pub struct Reference {
    /// The line it is written on, from 1.
    pub line: usize,
    /// What it names.
    pub target: Name,
    /// How, in the words of the language that writes it (`import proto`, `shape`, `use calendar`,
    /// `source`, `apply`).
    pub how: String,
}

/// What a file holds. `root` is the root of the project (DESIGN 6.2, item 3); `file` is the file,
/// from the root.
pub trait Items {
    fn items(&self, root: &Path, file: &str) -> Result<Vec<Item>, Vec<Said>>;
}

/// What a file names outside itself, in the order written.
pub trait References {
    fn references(&self, root: &Path, file: &str) -> Result<Vec<Reference>, Vec<Said>>;
}
