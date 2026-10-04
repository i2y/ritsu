//! The map, resolved (PLAN B.3): the map file and its contexts with every path made a path
//! from the root and every relationship pointing at its partner. What the checks read.

use crate::ast::{ContextFile, EnumMap, MapFile, Pos, Role, TermMap};
use crate::naming::Tool;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Model {
    pub root: PathBuf,
    pub map: MapInfo,
    pub contexts: Vec<Ctx>,
}

#[derive(Clone, Debug)]
pub struct MapInfo {
    /// From the root.
    pub file: String,
    pub dir: String,
    pub src: String,
    pub ast: MapFile,
    pub covers: Vec<String>,
    pub except: Vec<String>,
    pub proto_roots: Vec<String>,
    pub code: Vec<CodeDir>,
}

#[derive(Clone, Debug)]
pub struct CodeDir {
    pub language: String,
    pub path: String,
    pub test: Option<String>,
}

/// A file or a directory of `owns`, `layer` or `shared kernel with`, its path from the root.
#[derive(Clone, Debug, PartialEq)]
pub struct Own {
    pub path: String,
    /// None for `dir`.
    pub tool: Option<Tool>,
    pub pos: Pos,
}

impl Own {
    pub fn is_dir(&self) -> bool {
        self.tool.is_none()
    }

    /// As the `.ctx` writes it, with the path from the root: `dir "proto/warehouse"`.
    pub fn text(&self) -> String {
        format!("{} {}", self.tool.map(|t| t.word()).unwrap_or("dir"), crate::naming::quote(&self.path))
    }


    /// Whether the entry holds the file or directory `p`.
    pub fn holds(&self, p: &str) -> bool {
        if self.is_dir() { crate::paths::contains(&self.path, p) } else { self.path == p }
    }
}

/// `published language <package>`, its files from the root.
#[derive(Clone, Debug)]
pub struct Pub {
    pub package: String,
    pub pos: Pos,
    pub package_pos: Pos,
    /// The proto files, and where each is written.
    pub protos: Vec<(String, Pos)>,
    pub rulec: Option<(String, Pos)>,
    /// The directory of a Rust crate (DESIGN 1.4, 7.7).
    pub krate: Option<(String, Pos)>,
    pub services: Vec<(String, Pos)>,
    pub generated: Vec<(String, Pos)>,
}

#[derive(Clone, Debug)]
pub struct Ctx {
    pub name: String,
    pub alias: String,
    /// From the root.
    pub file: String,
    pub dir: String,
    pub src: String,
    pub ast: ContextFile,
    pub owns: Vec<Own>,
    pub published: Vec<Pub>,
    pub rels: Vec<Rel>,
}

#[derive(Clone, Debug)]
pub struct Rel {
    /// The index of the partner in `Model::contexts`.
    pub partner: usize,
    pub kind: RelK,
    pub pos: Pos,
    pub partner_pos: Pos,
}

#[derive(Clone, Debug)]
pub enum RelK {
    Upstream { roles: Vec<(Role, Pos)>, through: Vec<(String, Pos)>, layer: Vec<Own>, enums: Vec<EnumMap>, terms: Vec<TermMap> },
    /// `downstream <partner> supplier`
    Downstream,
    Kernel(Vec<Own>),
    Partnership,
    Separate,
}

impl RelK {
    /// The words the `.ctx` starts the relationship with.
    pub fn words(&self) -> &'static str {
        match self {
            RelK::Upstream { .. } => "upstream",
            RelK::Downstream => "downstream",
            RelK::Kernel(_) => "shared kernel with",
            RelK::Partnership => "partnership with",
            RelK::Separate => "separate ways from",
        }
    }
}

impl Rel {
    pub fn roles(&self) -> Vec<Role> {
        match &self.kind {
            RelK::Upstream { roles, .. } => roles.iter().map(|(r, _)| *r).collect(),
            _ => vec![],
        }
    }

    pub fn has(&self, r: Role) -> bool {
        self.roles().contains(&r)
    }
}

impl Model {
    /// The context of a name, by its index.
    pub fn ctx(&self, name: &str) -> Option<usize> {
        self.contexts.iter().position(|c| c.name == name)
    }

    /// The relationships `a` writes toward `b`.
    pub fn rels(&self, a: usize, b: usize) -> impl Iterator<Item = &Rel> {
        self.contexts[a].rels.iter().filter(move |r| r.partner == b)
    }

    /// `a`'s `upstream b …`, if it writes one.
    pub fn upstream(&self, a: usize, b: usize) -> Option<&Rel> {
        self.rels(a, b).find(|r| matches!(r.kind, RelK::Upstream { .. }))
    }

    pub fn writes(&self, a: usize, b: usize, f: impl Fn(&RelK) -> bool) -> bool {
        self.rels(a, b).any(|r| f(&r.kind))
    }

    /// The text of the line a relationship starts on.
    pub fn rel_line(&self, c: usize, r: &Rel) -> String {
        self.contexts[c].src.lines().nth(r.pos.line - 1).unwrap_or("").trim().to_string()
    }
}
