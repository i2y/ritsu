//! What the files of a project name of other files (DESIGN 17.8), each naming as the path from the
//! root it lands on: every reference a file's language gives (DESIGN 6.4; a `.req`'s through yuen's
//! port, which the index does not join), the `to` and each `after` of a contribution, which no
//! language's references hold, and each import of a `.proto`, read as ritsu-proto reads it (the
//! file's own module first, then its module's dependencies; DESIGN 17.6). `ritsu check` reads it for
//! the dependencies a module uses (W301) and the paths into a dependency's dependency (W303), and
//! `ritsu gen` for the files of the dependencies the files given reach ([`Project::reached`]).

use crate::joined::Joined;
use crate::project::{File, Project, rank};
use ritsu_base::naming::Tool;
use ritsu_base::paths;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

impl Project {
    /// What each file of the project names outside itself, with the line it is named on and the
    /// path from the root it lands on; and the files whose language could not say what they name
    /// (they do not read).
    pub fn named(&self, joined: &Joined) -> (Vec<(File, usize, String)>, Vec<String>) {
        named_in(self, &self.files, joined)
    }

    /// The project as `ritsu gen` makes a package of it (DESIGN 17.8): the files given, and each
    /// file of a dependency they name ([`Project::named`]), and each the files reached name, through
    /// every level, a dependency's dependencies too. A dependency's files are the ones a check of the
    /// whole module reads ([`Project::with_dependencies`]: what its `exclude` lines leave out is not
    /// there, and its `.geas` files are left out to be counted), and the files are in the order that
    /// gives them. A root with no `ritsu.mod`, and a tree that cannot be read, come back as they are.
    pub fn reached(mut self, joined: &Joined) -> Project {
        let Some(tree) = self.tree.clone() else { return self };
        let Ok(tree) = tree.as_ref() else { return self };
        // each file of a dependency, by its path from the root, with its language
        let theirs: BTreeMap<String, Tool> = self.dependency_files(tree).into_iter().map(|(t, r)| (r, t)).collect();
        let mut held: BTreeSet<String> = self.files.iter().map(|f| f.rel.clone()).collect();
        let mut rels: Vec<(Tool, String)> = self.files.iter().map(|f| (f.tool, f.rel.clone())).collect();
        let mut todo: Vec<File> = self.files.clone();
        while !todo.is_empty() {
            let (named, _) = named_in(&self, &todo, joined);
            todo = Vec::new();
            for (_, _, target) in named {
                let Some(tool) = theirs.get(&target).copied() else { continue };
                if held.insert(target.clone()) {
                    rels.push((tool, target.clone()));
                    todo.push(File { tool, shown: self.shown.path(&target), rel: target });
                }
            }
        }
        rels.sort_by(|(ta, a), (tb, b)| (rank(*ta), a).cmp(&(rank(*tb), b)));
        rels.dedup();
        self.files = rels.into_iter().map(|(tool, rel)| File { tool, shown: self.shown.path(&rel), rel }).collect();
        self.leave_out_claims();
        self
    }
}

/// [`Project::named`], of the files `files` of the project.
fn named_in(project: &Project, files: &[File], joined: &Joined) -> (Vec<(File, usize, String)>, Vec<String>) {
    let yuen = yuen::ports::Engine::with(joined.yuen());
    let mut out: Vec<(File, usize, String)> = Vec::new();
    let mut refused = Vec::new();
    for f in files.iter().filter(|f| f.tool != Tool::Proto) {
        let refs = match f.tool {
            Tool::Yuen => Some(ritsu_ports::References::references(&yuen, &project.root, &f.rel)),
            t => joined.index.references(t, &project.root, &f.rel),
        };
        match refs {
            None => {}
            Some(Err(_)) => refused.push(f.rel.clone()),
            Some(Ok(refs)) => out.extend(refs.into_iter().map(|r| (f.clone(), r.line, r.target.path))),
        }
        if f.tool == Tool::Rulec {
            for (line, shown) in rulec::link::names_of(&f.shown) {
                if let Some(rel) = paths::from_root(&project.root, Path::new(&shown)) {
                    out.push((f.clone(), line, rel));
                }
            }
        }
    }
    // the imports of the `.proto` files, each from the root of its module (`-I .`), and in a module
    // from the roots of the module's dependencies after its own
    let protos: Vec<&File> = files.iter().filter(|f| f.tool == Tool::Proto).collect();
    if !protos.is_empty() {
        let rels: Vec<String> = protos.iter().map(|f| f.rel.clone()).collect();
        let (ps, issues) = ritsu_proto::load(&project.root, &rels, &[".".to_string()]);
        for f in protos {
            let Some(pf) = ps.files.get(&f.rel) else {
                refused.push(f.rel.clone());
                continue;
            };
            let at = ps.imports.get(&f.rel).cloned().unwrap_or_default();
            for (i, imp) in pf.imports.iter().enumerate() {
                // a file found outside the files read is told, with where it was found
                let found = at.get(i).cloned().flatten().or_else(|| {
                    issues.iter().find_map(|x| match x {
                        ritsu_proto::Issue::OutOfScope { file, import, at } if *file == f.rel && import == imp => Some(at.clone()),
                        _ => None,
                    })
                });
                if let Some(t) = found {
                    out.push((f.clone(), imp.line, t));
                }
            }
        }
    }
    (out, refused)
}
