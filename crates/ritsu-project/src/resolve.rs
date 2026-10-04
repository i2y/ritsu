//! The references between files, resolved through the index (DESIGN 6.1, step 2; 6.4): for every
//! file of the project whose language says what it names outside itself (`References`), each
//! naming it writes, the file it lands on, whether that file is part of the project, and what the
//! language of that file says of the thing named (`Items`). What is wrong with a reference, each
//! language says in its own check, in its own codes (DESIGN 7.10): this is the one picture of the
//! project the checks across languages and the LSP read.

use crate::joined::Joined;
use crate::project::{File, Project};
use ritsu_base::naming::{Name, Tool};
use ritsu_base::text::Text;
use ritsu_ports::{Lookup, Said};

/// Where a reference lands.
#[derive(Clone, Debug, PartialEq)]
pub enum Landing {
    /// On no file: nothing is at the path.
    Missing,
    /// On a file whose things the index reads: what the language of that file says of the naming.
    Thing(Lookup),
    /// On a `.proto`, read with ritsu-proto: whether it holds the element named (the file itself
    /// when the naming names no element), or why it does not read.
    Proto(Result<bool, Text>),
    /// On a file whose things the index does not read: any file (`file "…"`), a directory, a
    /// requirements file.
    File,
}

/// One reference of a file of the project.
#[derive(Clone, Debug, PartialEq)]
pub struct Resolved {
    /// The file that writes it.
    pub from: File,
    pub line: usize,
    /// How, in the words of the language that writes it (`use rule … connect`, `import proto`).
    pub how: String,
    pub target: Name,
    /// Whether the file it lands on is one of the project's files (and so checked by its language
    /// in a `ritsu check` of the project).
    pub in_project: bool,
    pub landing: Landing,
}

impl Project {
    /// Every reference the files of the project make, in the order of the files and of the lines,
    /// each resolved through the index of `joined`; and, for each file whose language could not say
    /// what it names (it does not read), what the language says.
    pub fn references(&self, joined: &Joined) -> (Vec<Resolved>, Vec<(File, Vec<Said>)>) {
        let mut out = Vec::new();
        let mut refused = Vec::new();
        for f in &self.files {
            let refs = match joined.index.references(f.tool, &self.root, &f.rel) {
                None => continue,
                Some(Err(said)) => {
                    refused.push((f.clone(), said));
                    continue;
                }
                Some(Ok(refs)) => refs,
            };
            for r in refs {
                let landing = self.land(joined, &r.target);
                let in_project = self.holds(&r.target.path).is_some();
                out.push(Resolved { from: f.clone(), line: r.line, how: r.how, target: r.target, in_project, landing });
            }
        }
        (out, refused)
    }

    /// Where a naming lands.
    fn land(&self, joined: &Joined, n: &Name) -> Landing {
        let disk = ritsu_base::paths::on_disk(&self.root, &n.path);
        if !disk.exists() {
            return Landing::Missing;
        }
        if disk.is_dir() {
            return Landing::File;
        }
        match n.tool {
            Tool::Proto => Landing::Proto(proto_holds(&disk, n)),
            t if joined.index.reads_items(t) => Landing::Thing(joined.index.find(&self.root, n)),
            _ => Landing::File,
        }
    }
}

/// Whether a `.proto` holds what a naming names (DESIGN 6.2, item 1: `message M [field f]`,
/// `enum E [value V]`, `service S [method M]`, the names from the file's package).
fn proto_holds(disk: &std::path::Path, n: &Name) -> Result<bool, Text> {
    let src = std::fs::read_to_string(disk).map_err(|e| Text::same(e.to_string()))?;
    let f = ritsu_proto::read(&n.path, &src).map_err(|e| e.message("ritsu"))?;
    let Some((k, name)) = n.items.first() else { return Ok(true) };
    let child = n.items.get(1).map(|(_, c)| c.as_str());
    Ok(match k.as_str() {
        "message" => f.message(name).is_some_and(|m| child.is_none_or(|c| m.fields.iter().any(|x| x.name == c))),
        "enum" => f.enumeration(name).is_some_and(|e| child.is_none_or(|c| e.values.iter().any(|x| x.name == c))),
        "service" => f.service(name).is_some_and(|s| child.is_none_or(|c| s.methods.iter().any(|x| x.name == c))),
        _ => false,
    })
}
