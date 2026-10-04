//! What a file holds and what it names outside itself (DESIGN 6.4): every language gives both, by
//! the naming of DESIGN 6.2. A project's index is made of them ([`Index`], which ritsu-project
//! joins), yuen's ends are items, and sakai checks references against the map.

use crate::Said;
use ritsu_base::naming::{Name, Tool};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

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

/// What a run has asked of each file: by the tool, the root and the file from it (a naming's path
/// is from its root, so the same file under two roots is two answers).
type Asked<T> = RefCell<BTreeMap<(Tool, PathBuf, String), Result<Vec<T>, Vec<Said>>>>;

/// The index of a project (DESIGN 6.4, X10): what each file holds and names outside itself, by the
/// naming of DESIGN 6.2, asked of the file's language once in a run and kept. ritsu-project joins
/// every language's answer into one index; yuen and sakai look their namings up in it rather than
/// asking each language themselves, so a file both of them name, and the file a check across
/// languages reads, is read once. What is wrong with a naming each of them says in its own codes
/// and words: the index only finds things.
#[derive(Default)]
pub struct Index {
    items: BTreeMap<Tool, Rc<dyn Items>>,
    references: BTreeMap<Tool, Rc<dyn References>>,
    asked_items: Asked<Item>,
    asked_refs: Asked<Reference>,
}

/// What a naming comes to in an [`Index`].
#[derive(Clone, Debug, PartialEq)]
pub enum Lookup {
    /// The language of the naming's tool is not joined (the binary of a receiving language's own
    /// crate).
    NotJoined,
    /// The language cannot answer for the file (it does not read, or does not pass what the
    /// language asks of a file before it answers): what the language says.
    Refused(Vec<Said>),
    /// The file holds it; None for a naming of the file itself, which the language reads.
    Found(Option<Item>),
    /// The file holds no such thing: the things it holds of the same kind, under the same pairs.
    Missing(Vec<Item>),
}

impl Index {
    pub fn new() -> Index {
        Index::default()
    }

    /// The index with the language of `tool` answering what its files hold.
    pub fn with_items(mut self, tool: Tool, port: Rc<dyn Items>) -> Index {
        self.items.insert(tool, port);
        self
    }

    /// The index with the language of `tool` answering what its files name outside themselves.
    pub fn with_references(mut self, tool: Tool, port: Rc<dyn References>) -> Index {
        self.references.insert(tool, port);
        self
    }

    /// Whether the index reads what the files of `tool` hold.
    pub fn reads_items(&self, tool: Tool) -> bool {
        self.items.contains_key(&tool)
    }

    /// Whether the index reads what the files of `tool` name outside themselves.
    pub fn reads_references(&self, tool: Tool) -> bool {
        self.references.contains_key(&tool)
    }

    /// What the file (`file`, from `root`) holds, asked of its language once in a run. None when
    /// the language of `tool` is not joined.
    pub fn items(&self, tool: Tool, root: &Path, file: &str) -> Option<Result<Vec<Item>, Vec<Said>>> {
        let port = self.items.get(&tool)?;
        let key = (tool, root.to_path_buf(), file.to_string());
        if let Some(r) = self.asked_items.borrow().get(&key) {
            return Some(r.clone());
        }
        let r = port.items(root, file);
        self.asked_items.borrow_mut().insert(key, r.clone());
        Some(r)
    }

    /// What the file names outside itself, asked of its language once in a run. None when the
    /// language of `tool` is not joined.
    pub fn references(&self, tool: Tool, root: &Path, file: &str) -> Option<Result<Vec<Reference>, Vec<Said>>> {
        let port = self.references.get(&tool)?;
        let key = (tool, root.to_path_buf(), file.to_string());
        if let Some(r) = self.asked_refs.borrow().get(&key) {
            return Some(r.clone());
        }
        let r = port.references(root, file);
        self.asked_refs.borrow_mut().insert(key, r.clone());
        Some(r)
    }

    /// The thing a naming names, under the root `root`: asked of the language of its tool, which
    /// must answer for the file even when the naming names the file itself.
    pub fn find(&self, root: &Path, n: &Name) -> Lookup {
        let items = match self.items(n.tool, root, &n.path) {
            None => return Lookup::NotJoined,
            Some(Err(said)) => return Lookup::Refused(said),
            Some(Ok(items)) => items,
        };
        if n.items.is_empty() {
            return Lookup::Found(None);
        }
        if let Some(i) = items.iter().find(|i| i.naming == *n) {
            return Lookup::Found(Some(i.clone()));
        }
        let (parent, kind) = (&n.items[..n.items.len() - 1], n.items.last().map(|(k, _)| k.as_str()).unwrap_or(""));
        Lookup::Missing(items.into_iter().filter(|i| i.naming.items.len() == n.items.len() && i.naming.items[..parent.len()] == *parent && i.kind() == kind).collect())
    }
}
