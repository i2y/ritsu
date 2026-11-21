//! What the files the links name read (DESIGN 8): the files their answers are made of, so that a
//! change of one reaches the requirements that name something in a file that reads it, though no
//! byte of that file changes. Each language says what its files read (ritsu's `References::reads`,
//! looked up in the project's index, which asks each file once); yuen reads the standard formats
//! itself, as it reads them for their ends: what a `.proto` imports (3.4), and the other files a
//! document's `$ref`s reach (3.6). A file read by a file read is read too, through every level.

use crate::names::{Name, Tool};
use crate::project::Project;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// One step of the way down from a file to a file it reads: the file that reads, the line that
/// names what it reads (None for what it reads without naming it), how, in the words of its
/// language, and what it reads, as the language names it.
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub reader: Name,
    pub line: Option<usize>,
    pub how: String,
    pub read: Name,
}

/// What the files of a run read, each file asked once.
#[derive(Default)]
pub struct Reads {
    asked: RefCell<BTreeMap<(Tool, String), Vec<Step>>>,
}

impl Reads {
    /// What the file reads itself: by its language, through the index; a `.proto`, the files it
    /// imports; a document, the other files its `$ref`s reach. Nothing for a file whose language
    /// says nothing of what it reads (a book, a spec, a file of bytes), or cannot read it.
    fn of(&self, p: &Project, file: &Name) -> Vec<Step> {
        let key = (file.tool, file.path.clone());
        if let Some(s) = self.asked.borrow().get(&key) {
            return s.clone();
        }
        let steps = match p.suite.index.reads(file.tool, &p.root, &file.path) {
            Some(Ok(reads)) => reads.into_iter().map(|r| Step { reader: file.clone(), line: r.line, how: r.how, read: r.target }).collect(),
            Some(Err(_)) => Vec::new(),
            None => match file.tool {
                Tool::Proto => {
                    self.imports(p, file);
                    return self.asked.borrow().get(&key).cloned().unwrap_or_default();
                }
                Tool::Openapi | Tool::Asyncapi | Tool::Jsonschema => refs(p, file),
                _ => Vec::new(),
            },
        };
        self.asked.borrow_mut().insert(key, steps.clone());
        steps
    }

    /// What a `.proto` imports, as yuen reads it (3.4): the file and every file it imports, read
    /// once, each file's imports kept for when it is asked.
    fn imports(&self, p: &Project, file: &Name) {
        let ps = crate::proto::load(&p.root, &file.path).ok();
        let mut asked = self.asked.borrow_mut();
        let Some(ps) = ps else {
            asked.insert((file.tool, file.path.clone()), Vec::new());
            return;
        };
        for (f, found) in &ps.imports {
            let Some(pf) = ps.files.get(f) else { continue };
            let reader = Name::file(Tool::Proto, f.clone());
            let steps = pf
                .imports
                .iter()
                .zip(found)
                .filter_map(|(imp, at)| Some(Step { reader: reader.clone(), line: Some(imp.line), how: "import".into(), read: Name::file(Tool::Proto, at.clone()?) }))
                .collect();
            asked.entry((Tool::Proto, f.clone())).or_insert(steps);
        }
        asked.entry((file.tool, file.path.clone())).or_default();
    }

    /// Every file `start` reads, through every level, each once, with the shortest way down to it:
    /// level by level, and in each file in the order its language gives what it reads. A file read
    /// on two ways is followed once; `start` itself is not among them.
    pub fn closure(&self, p: &Project, start: &Name) -> Vec<Vec<Step>> {
        let top = Name::file(start.tool, start.path.clone());
        let mut seen: BTreeSet<(Tool, String)> = BTreeSet::from([(top.tool, top.path.clone())]);
        let mut out: Vec<Vec<Step>> = Vec::new();
        let mut todo: VecDeque<(Name, Vec<Step>)> = VecDeque::from([(top, Vec::new())]);
        while let Some((at, way)) = todo.pop_front() {
            for s in self.of(p, &at) {
                let file = Name::file(s.read.tool, s.read.path.clone());
                if !seen.insert((file.tool, file.path.clone())) {
                    continue;
                }
                let mut w = way.clone();
                w.push(s);
                out.push(w.clone());
                todo.push_back((file, w));
            }
        }
        out
    }
}

/// The other files a document's `$ref`s reach, each once, at the line of the first `$ref` to it.
/// A document that does not read reads none.
fn refs(p: &Project, file: &Name) -> Vec<Step> {
    let Ok(text) = ritsu_base::fs::read_to_string(ritsu_base::paths::on_disk(&p.root, &file.path)) else { return Vec::new() };
    let Ok(top) = ritsu_base::yaml::read_file_text(&file.path, &text) else { return Vec::new() };
    let mut found = Vec::new();
    ritsu_base::document::refs_in(&top, "", &mut found);
    let mut out: Vec<Step> = Vec::new();
    for (_, line, _, written) in found {
        let Ok((t, _)) = ritsu_base::document::target(&file.path, &written) else { continue };
        if t == file.path || out.iter().any(|s| s.read.path == t) {
            continue;
        }
        out.push(Step { reader: file.clone(), line: Some(line), how: "$ref".into(), read: Name::file(file.tool, t) });
    }
    out
}

/// Whether what a step reads is the file at `path` (from the root), or a directory it is under.
pub fn reaches(read: &Name, path: &str) -> bool {
    let dir = read.path.trim_end_matches('/');
    path == dir || path.strip_prefix(dir).is_some_and(|rest| rest.starts_with('/'))
}
