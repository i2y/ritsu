//! The files a project's requests are lowered to, laid where the languages read them (ringi's
//! DESIGN 6.4, 12.1, 12.3), for the package `ritsu gen` makes: each request's lowered rules and
//! dates, the module of kaname that joins it to the machine of its tickets (its glue, in the same
//! lowered directory, with the files of business days it counts by), and the machine itself, the
//! module `ticket` that ringi's crate holds, laid once for the project at [`TICKET`] under its root
//! with the day numbers it reads ([`DAYS`]). None of them is on the disk: they are laid over the
//! files the languages read ([`Laid::overlay`]), and are files of the project only for the package
//! (`ritsu check` reads a request's lowered files through ringi's check, which says what their
//! languages say of them at the request's lines).

use crate::{File, Joined, Project};
use ritsu_base::naming::Tool;
use ritsu_base::text::Text;
use std::path::Path;
use std::rc::Rc;

/// Where a project lays the machine of the tickets, from its root: a directory no file of the
/// project is in, named as the lowered directories are.
pub const TICKET: &str = ".lowered/ticket.kn";

/// Where a project lays the day numbers the machine reads, beside it.
pub const DAYS: &str = ".lowered/days.cal";

/// What the project's requests are lowered to, laid.
pub struct Laid {
    /// Each lowered file of each request whose glue is written, the glue, and the machine, as files
    /// of the project, in the order of the languages and of their paths.
    pub files: Vec<File>,
    /// The files laid over the files the languages read.
    pub overlay: Rc<dyn ritsu_base::fs::Files>,
    /// Each request whose code is not written, as the run writes it, and why: what of it the glue
    /// does not lower yet.
    pub left: Vec<(File, Text)>,
    /// Each kind of request whose code is written, in the order of their names: what a package's
    /// list of them says (ringi's DESIGN 12.2).
    pub kinds: Vec<Kind>,
}

/// A kind of request whose code is written: its ASCII name, its version, its glue's path from the
/// root (the module of kaname whose `start` and `next` a host calls), and its request's file, which
/// `--target requests` describes.
#[derive(Clone, Debug)]
pub struct Kind {
    pub name: String,
    pub version: u32,
    pub glue: String,
    pub request: File,
}

impl Project {
    /// The files the project's requests are lowered to, laid (ringi's DESIGN 12.3): the requests
    /// that pass their check, each with its glue; a request whose glue is not written is left out
    /// whole, with why. None laid when the project has no request.
    pub fn laid(&self, joined: &Joined) -> Laid {
        // a gate a request reads is named from the project's root, as the project's check names it
        joined.ringi.set_root(&self.root);
        let mut ov = ritsu_base::fs::Overlay::new(ritsu_base::fs::current());
        let mut files: Vec<File> = Vec::new();
        let mut left = Vec::new();
        let mut kinds = Vec::new();
        let ticket = ritsu_base::paths::absolute(&self.root.join(TICKET));
        let requests = joined.requests();
        for f in self.of(Tool::Ringi) {
            let path = Path::new(&f.shown);
            // an organization, masters and a shared route lower to no glue
            let Ok(ritsu_ports::Found::Value(_)) = requests.ticket(path) else { continue };
            let Ok(lowered) = requests.lowered(path) else { continue };
            let glue = match joined.ringi.glue(path, &ticket) {
                Ok(g) => g,
                Err(why) => {
                    left.push((f.clone(), why));
                    continue;
                }
            };
            let dir = ringi::lower::dir_of(&f.shown);
            let mut laid: Vec<(std::path::PathBuf, String)> = lowered.into_iter().map(|l| (l.path, l.text)).collect();
            if let Some(rel) = ritsu_base::paths::from_root(&self.root, &dir.join(&glue.name)) {
                kinds.push(Kind { name: glue.request.clone(), version: glue.version, glue: rel, request: f.clone() });
            }
            laid.push((dir.join(&glue.name), glue.text));
            laid.extend(glue.files.into_iter().map(|(name, text)| (dir.join(name), text)));
            for (p, text) in laid {
                let Some(rel) = ritsu_base::paths::from_root(&self.root, &p) else { continue };
                let Some(tool) = crate::project::kind_of(&rel) else { continue };
                ov.put(&ritsu_base::paths::absolute(&p), text);
                files.push(File { tool, shown: self.shown.path(&rel), rel });
            }
        }
        if !files.is_empty() {
            ov.put(&ticket, ringi::lower::glue::TICKET);
            files.push(File { tool: Tool::Kaname, shown: self.shown.path(TICKET), rel: TICKET.to_string() });
            ov.put(&ritsu_base::paths::absolute(&self.root.join(DAYS)), ringi::lower::glue::DAYS);
            files.push(File { tool: Tool::Koyomi, shown: self.shown.path(DAYS), rel: DAYS.to_string() });
        }
        files.sort_by(|a, b| (crate::project::rank(a.tool), &a.rel).cmp(&(crate::project::rank(b.tool), &b.rel)));
        kinds.sort_by(|a: &Kind, b: &Kind| a.name.cmp(&b.name));
        Laid { files, overlay: Rc::new(ov), left, kinds }
    }

    /// The project with the files `laid` holds among its own, in the order of the languages and of
    /// their paths: what `ritsu gen` makes the packages of.
    pub fn with_laid(&self, laid: &Laid) -> Project {
        let mut p = self.clone();
        p.files.extend(laid.files.iter().cloned());
        p.files.sort_by(|a, b| (crate::project::rank(a.tool), &a.rel).cmp(&(crate::project::rank(b.tool), &b.rel)));
        p
    }

    /// The project of the files `laid` holds alone: what is checked of them before their package is
    /// made, as the project's own files are.
    pub fn only_laid(&self, laid: &Laid) -> Project {
        let mut p = self.clone();
        p.files = laid.files.clone();
        p
    }
}
