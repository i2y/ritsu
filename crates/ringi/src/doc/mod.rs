//! The pages of `ritsu ringi doc` (DESIGN 11.1, 11.3): one page for a file that passes its check, in
//! the words of the people who approve the requests and the people who manage them, in English or
//! Japanese, as Markdown or as one HTML file.
//!
//! - A request: when it can be used and what decides that, who may submit it, its form (what each
//!   field accepts, and what a computed field's computation comes to), its route case by case (`上長
//!   → 部長と経理 → 本部長`), how each approver is chosen, the deadlines and reminders with dates,
//!   the rules of a ticket, comments, proxies, what it does to books and flows, its claims, its
//!   examples and its scenarios ([`request`]).
//! - With `--longest`, the longest a ticket of the request can take (DESIGN 7.6), with the day it is
//!   submitted and each deadline after it, and what shortens it ([`longest`]).
//! - A shared route, an organization and masters: a page of their own ([`others`]).
//!
//! The page is built from what the check read and decided, not from the pages of the lowered files:
//! the route's table is the table the request writes, its cells said with the ways to choose as the
//! file names them. The lowered `route.rule` holds the same table with a column of its own for each
//! stage's value, whose page names those values and the lowered file; swapping the text of its cells
//! for the ways' names would follow rulec's page as it is drawn, and break where it changes.

pub mod longest;
pub mod others;
pub mod request;
pub mod words;
pub mod write;

use crate::check::{Outcome, Run};
use crate::names::Scope;
use ritsu_base::text::{Lang, Text};

/// A page: the file it was drawn from as the page names it (from the root), the file's SHA-256 (the
/// short form), the language, the title, the description and the blocks.
#[derive(Clone, Debug)]
pub struct Page {
    pub file: String,
    pub sha: String,
    pub lang: Lang,
    pub title: String,
    pub description: Option<String>,
    pub blocks: Vec<Block>,
}

/// A block of a page. Text holds code in backquotes.
#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    Heading { level: u8, text: String, id: String },
    Para(String),
    Note(String),
    List(Vec<String>),
    Table { head: Vec<String>, rows: Vec<Vec<String>> },
}

/// What a page is asked for.
#[derive(Clone, Debug, Default)]
pub struct Ask {
    /// `--longest`: the longest a ticket of the request can take, with dates.
    pub longest: bool,
    /// `--on`: the day the ticket of `--longest` is submitted on.
    pub on: Option<i64>,
    /// The root of the project the file is in, when there is one (`--root`, or the directory above
    /// the file that holds a `.git` or a `ritsu.mod`, as `check` finds it): where the page of a
    /// shared route finds the requests that take it.
    pub project: Option<std::path::PathBuf>,
}

/// The root `root` (found as `check` finds it, [`crate::check::root_of`]) as the root of a project:
/// when `--root` gave it, or it holds a `.git` or a `ritsu.mod`. None when the file is in no project,
/// and the root is only the directory it is in.
pub fn project_of(root: &std::path::Path, given: bool) -> Option<std::path::PathBuf> {
    let found = given || ritsu_base::fs::exists(root.join(".git")) || ritsu_base::fs::is_file(root.join(ritsu_base::modules::manifest::FILE));
    found.then(|| root.to_path_buf())
}

/// The page of the file `o` (which passes its check), named as `shown`.
pub fn page(o: &Outcome, run: &Run, shown: &str, lang: Lang, ask: &Ask) -> Result<Page, Text> {
    let Some(file) = &o.file else { return Err(tr!("検査が通りません", "the file does not pass its check")) };
    let src = ritsu_base::fs::read_to_string(&o.path).map_err(|e| ritsu_base::cli::cannot_read(shown, &e))?;
    let mut said = Vec::new();
    let got = crate::names::read_uses(file, &o.path, &src, run, &mut said);
    let s = Scope { file, path: &o.path, src: &src, got };
    let sha = ritsu_base::sha256::short(src.as_bytes());
    let mut say = words::Say::new(&s, lang);
    if let (crate::ast::FileKind::Organization, Some(crate::org::Data::Org(org))) = (file.kind, o.data.as_deref()) {
        // an organization's page names its people and teams from its own data
        (say.org, say.named_from) = (Some(org), org.versions.last());
    }
    if ask.longest {
        if file.kind != crate::ast::FileKind::Request {
            return Err(tr!("`--longest` が見せるのは申請の種類のチケットです", "`--longest` shows a ticket of a request"));
        }
        return longest::page(o, run, &say, shown, &sha, ask.on);
    }
    let blocks = match file.kind {
        crate::ast::FileKind::Request => request::blocks(o, run, &say, shown, &sha),
        crate::ast::FileKind::Route => others::route(o, run, &say, shown, &sha, ask.project.as_deref()),
        crate::ast::FileKind::Organization => others::organization(o, &say, shown, &sha),
        crate::ast::FileKind::Masters => others::masters(o, &say, shown, &sha),
    };
    Ok(Page { file: shown.to_string(), sha, lang, title: say.title(), description: say.description(), blocks })
}

impl words::Say<'_, '_> {
    /// The file's description, as written (a description has no label).
    pub fn description(&self) -> Option<String> {
        self.s.file.description.as_ref().map(|d| d.0.clone())
    }
}

/// A section's heading.
pub fn h2(text: String, id: &str) -> Block {
    Block::Heading { level: 2, text, id: id.to_string() }
}

/// A heading under a section.
pub fn h3(text: String, id: String) -> Block {
    Block::Heading { level: 3, text, id }
}

/// An ID for a heading made of a name: ASCII letters and digits kept, the rest `-`.
pub fn id_of(prefix: &str, name: &str) -> String {
    let body: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' }).collect();
    let body = body.trim_matches('-').to_string();
    if body.is_empty() {
        // a name with no ASCII letter: its bytes, so that two names are two IDs
        let hex: String = name.bytes().map(|b| format!("{b:02x}")).collect();
        format!("{prefix}-{hex}")
    } else {
        format!("{prefix}-{body}")
    }
}
