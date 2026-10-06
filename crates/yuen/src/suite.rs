//! The languages yuen reads through ritsu's ports (ritsu's DESIGN 3.2, yuen's DESIGN 3.1): what a
//! file of rulec, koyomi, chobo, geas, dandori, sekisho or sakai holds and the definition of each
//! thing in it, looked up in the project's index (`Index`, which keeps every language's `Items`), the
//! sources a rule or a calendar pins (`Sources`), the claims of a geas spec, the record of what
//! they ran and what a diff comes to for them (`Claims`), and the aliases of a rule's and a dates
//! file's names (`Rules`, `Dates`), for when a link is written with one.
//!
//! yuen holds none of those languages: they are handed to it. The binary of yuen's own crate is
//! handed none, and says so when a project names something only another language can read; `ritsu
//! yuen` hands it every one, joined once for the whole run (ritsu's DESIGN 2.3, 6, 8.6). Each file
//! is asked once in a run: what it holds by the index, the sources it pins here.

use crate::names::Tool;
use ritsu_ports::{Claims, Dates, Index, Rules, Said, Source, Sources};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// The ports of the languages yuen reads, as whoever runs yuen joins them.
#[derive(Default, Clone)]
pub struct Suite {
    /// What the files of the languages hold, by the naming of ritsu's DESIGN 6.2.
    pub index: Rc<Index>,
    pub sources: BTreeMap<String, Rc<dyn Sources>>,
    pub rules: Option<Rc<dyn Rules>>,
    pub dates: Option<Rc<dyn Dates>>,
    pub claims: Option<Rc<dyn Claims>>,
    asked: Rc<Asked>,
}

/// What a run has asked, by file; and the documents and Cedar files it has read (DESIGN 3.6), so
/// that a large document many links name is read once.
#[derive(Default)]
struct Asked {
    sources: RefCell<BTreeMap<(String, PathBuf), Result<Vec<Source>, Vec<Said>>>>,
    documents: RefCell<BTreeMap<(PathBuf, Tool, String), Rc<Result<crate::documents::Read, ritsu_base::text::Text>>>>,
}

impl Suite {
    /// The languages whose things yuen reads through the index, by their tool words.
    pub const READ: [Tool; 7] = [Tool::Rulec, Tool::Koyomi, Tool::Chobo, Tool::Geas, Tool::Dandori, Tool::Sekisho, Tool::Sakai];

    /// Whether yuen is handed what it needs to read the files of `tool`.
    pub fn reads(&self, tool: Tool) -> bool {
        match tool {
            // the standard formats, which yuen reads itself (DESIGN 3.4, 3.6)
            Tool::File | Tool::Proto | Tool::Openapi | Tool::Asyncapi | Tool::Cedar => true,
            Tool::Rulec | Tool::Koyomi => self.index.reads_items(tool) && self.sources.contains_key(tool.word()),
            Tool::Geas => self.index.reads_items(tool) && self.claims.is_some(),
            t => self.index.reads_items(t),
        }
    }

    /// The file of a reference of `openapi`, `asyncapi` or `cedar` (`file`, from the root), read
    /// once in a run (DESIGN 3.6).
    pub fn document(&self, root: &Path, tool: Tool, file: &str) -> Rc<Result<crate::documents::Read, ritsu_base::text::Text>> {
        let key = (root.to_path_buf(), tool, file.to_string());
        if let Some(r) = self.asked.documents.borrow().get(&key) {
            return r.clone();
        }
        let r = Rc::new(crate::documents::load(root, tool, file));
        self.asked.documents.borrow_mut().insert(key, r.clone());
        r
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
