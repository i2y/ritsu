//! The languages yuen reads through ritsu's ports (ritsu's DESIGN 3.2, yuen's DESIGN 3.1): what
//! a file of rulec, koyomi, chobo, geas, dandori or sakai holds and the definition of each thing
//! in it (`Items`), the sources a rule or a calendar pins (`Sources`), the claims of a geas spec,
//! the record of what they ran and what a diff comes to for them (`Claims`), and the aliases of a
//! rule's and a dates file's names (`Rules`, `Dates`), for when a link is written with one.
//!
//! yuen holds none of those languages: they are handed to it. The binary of yuen's own crate is
//! handed none, and says so when a project names something only another language can read; `ritsu
//! yuen` hands it every one (ritsu's DESIGN 2.3, 8.6). Each file is asked once in a run.

use crate::names::Tool;
use ritsu_ports::{Claims, Dates, Item, Items, Rules, Said, Source, Sources};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// The ports of the languages yuen reads, as whoever runs yuen joins them.
#[derive(Default, Clone)]
pub struct Suite {
    pub items: BTreeMap<String, Rc<dyn Items>>,
    pub sources: BTreeMap<String, Rc<dyn Sources>>,
    pub rules: Option<Rc<dyn Rules>>,
    pub dates: Option<Rc<dyn Dates>>,
    pub claims: Option<Rc<dyn Claims>>,
    asked: Rc<Asked>,
}

/// What a run has asked, by file.
#[derive(Default)]
struct Asked {
    items: RefCell<BTreeMap<(String, PathBuf), Result<Vec<Item>, Vec<Said>>>>,
    sources: RefCell<BTreeMap<(String, PathBuf), Result<Vec<Source>, Vec<Said>>>>,
}

impl Suite {
    /// The languages whose things yuen reads through `Items`, by their tool words.
    pub const READ: [Tool; 6] = [Tool::Rulec, Tool::Koyomi, Tool::Chobo, Tool::Geas, Tool::Dandori, Tool::Sakai];

    /// Whether yuen is handed what it needs to read the files of `tool`.
    pub fn reads(&self, tool: Tool) -> bool {
        match tool {
            Tool::File | Tool::Proto => true,
            Tool::Rulec | Tool::Koyomi => self.items.contains_key(tool.word()) && self.sources.contains_key(tool.word()),
            Tool::Geas => self.items.contains_key(tool.word()) && self.claims.is_some(),
            t => self.items.contains_key(t.word()),
        }
    }

    /// What the file holds (`file` from the root `root`), asked of its language once in a run.
    /// None when the language is not handed to yuen.
    pub fn items(&self, tool: Tool, root: &Path, file: &str) -> Option<Result<Vec<Item>, Vec<Said>>> {
        let port = self.items.get(tool.word())?;
        let key = (tool.word().to_string(), root.join(file));
        if let Some(r) = self.asked.items.borrow().get(&key) {
            return Some(r.clone());
        }
        let r = port.items(root, file);
        self.asked.items.borrow_mut().insert(key, r.clone());
        Some(r)
    }

    /// The sources a rule or a calendar pins (`abs`, absolute), asked once in a run. None when the
    /// language is not handed to yuen.
    pub fn sources(&self, tool: Tool, abs: &Path) -> Option<Result<Vec<Source>, Vec<Said>>> {
        let port = self.sources.get(tool.word())?;
        let key = (tool.word().to_string(), abs.to_path_buf());
        if let Some(r) = self.asked.sources.borrow().get(&key) {
            return Some(r.clone());
        }
        let r = port.sources(abs);
        self.asked.sources.borrow_mut().insert(key, r.clone());
        Some(r)
    }
}
