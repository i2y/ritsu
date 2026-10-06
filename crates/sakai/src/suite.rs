//! The languages sakai reads through ritsu's ports (ritsu's DESIGN 3.2, sakai's DESIGN 4.1): a
//! rule's enums, its Connect service and its names (`Rules`), what a rule, a calendar and a
//! workflow name outside themselves and what a rule holds, looked up in the project's index
//! (`Index`, which keeps every language's `References` and `Items`), and the accounts and
//! transfers of a book (`Books`) and the dates of a dates file (`Dates`), for the pages of `doc`.
//!
//! sakai holds none of those languages: they are handed to it. The binary of sakai's own crate is
//! handed none, and where a map holds what only another language reads, it says so (E104);
//! `ritsu sakai` hands it every one, joined once for the whole run (ritsu's DESIGN 2.3, 6, 8.6).
//! Each file is asked once in a run: what it names and holds by the index, rulec's facts here.

use crate::diag::{self, Diag};
use crate::model::Model;
use crate::naming::Tool;
use crate::owners::Artifact;
use crate::paths::shown;
use ritsu_ports::{BookFacts, Books, Dates, Index, Reference, RuleFacts, Rules, Said};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// The ports of the languages sakai reads, as whoever runs sakai joins them.
#[derive(Default, Clone)]
pub struct Suite {
    pub rules: Option<Rc<dyn Rules>>,
    /// What the files of rulec, koyomi, dandori and sekisho (and Cedar's written by hand) name
    /// outside themselves, and what a rule holds, by the naming of ritsu's DESIGN 6.2.
    pub index: Rc<Index>,
    pub books: Option<Rc<dyn Books>>,
    /// What koyomi knows of a dates file: its dates and the days its calendar's data covers, for
    /// the pages of `doc`.
    pub dates: Option<Rc<dyn Dates>>,
    asked: Rc<Asked>,
}

#[derive(Default)]
struct Asked {
    facts: RefCell<BTreeMap<PathBuf, Result<RuleFacts, Vec<Said>>>>,
}

impl Suite {
    /// The languages whose artifacts sakai reads the references of, by their tool words: sekisho
    /// answers for its gates and for the Cedar written by hand (the operations a schema's `@guards`
    /// names).
    pub const READ: [Tool; 5] = [Tool::Rulec, Tool::Koyomi, Tool::Dandori, Tool::Sekisho, Tool::Cedar];

    /// Whether sakai is handed what it needs to read the artifacts of `tool`.
    pub fn reads(&self, tool: Tool) -> bool {
        match tool {
            Tool::Rulec => self.rules.is_some() && self.index.reads_references(Tool::Rulec),
            Tool::Koyomi | Tool::Dandori | Tool::Sekisho | Tool::Cedar => self.index.reads_references(tool),
            Tool::Chobo => self.books.is_some(),
            _ => true,
        }
    }

    /// What rulec knows of a rule (`abs`, absolute), asked once in a run.
    pub fn facts(&self, abs: &Path) -> Option<Result<RuleFacts, Vec<Said>>> {
        let port = self.rules.as_ref()?;
        if let Some(r) = self.asked.facts.borrow().get(abs) {
            return Some(r.clone());
        }
        let r = port.facts(abs);
        self.asked.facts.borrow_mut().insert(abs.to_path_buf(), r.clone());
        Some(r)
    }

    /// What a file of `tool` names outside itself (`file` from `root`), asked once in a run.
    pub fn references(&self, tool: Tool, root: &Path, file: &str) -> Option<Result<Vec<Reference>, Vec<Said>>> {
        self.index.references(tool, root, file)
    }

    /// What chobo knows of a book (`abs`, absolute): its accounts and transfers, for `doc`.
    pub fn book(&self, abs: &Path) -> Option<Result<BookFacts, Vec<Said>>> {
        Some(self.books.as_ref()?.facts(abs))
    }
}

thread_local! {
    /// The command line `run` was given, words after the program's name, to say again with
    /// `ritsu sakai` in front when a language is not joined.
    pub static COMMAND: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
}

/// A word as a shell reads it back: as it is, or in single quotes.
fn shell_word(w: &str) -> String {
    if !w.is_empty() && w.chars().all(|c| !c.is_whitespace() && !"'\"\\$`;&|<>()*?[]#~".contains(c)) {
        w.to_string()
    } else {
        format!("'{}'", w.replace('\'', "'\\''"))
    }
}

/// The command to run instead: the one given, with `ritsu sakai` in front.
pub fn with_ritsu(m: &Model) -> String {
    let words = COMMAND.with(|c| c.borrow().clone()).unwrap_or_else(|| vec!["check".to_string(), m.map.file.clone()]);
    let shown: Vec<String> = words.iter().map(|w| shell_word(w)).collect();
    format!("ritsu sakai {}", shown.join(" "))
}

/// What the other languages said of the map's artifacts.
#[derive(Default)]
pub struct Read {
    /// rulec's facts of each rule it answers for, by the rule's path from the root.
    pub facts: BTreeMap<String, RuleFacts>,
    /// The project's index, where the things a rule holds are looked up.
    pub index: Rc<Index>,
    /// What each artifact of a rule, a calendar or a workflow names outside itself: the artifact,
    /// its tool, and the references.
    pub refs: Vec<(String, Tool, Vec<Reference>)>,
    /// Whether every artifact of another language was read: false while a language is not joined
    /// or did not answer for a file, when what crosses is not known in full.
    pub complete: bool,
}

impl Read {
    /// What one artifact names outside itself.
    pub fn refs_of(&self, path: &str) -> &[Reference] {
        self.refs.iter().find(|(p, _, _)| p == path).map(|(_, _, r)| r.as_slice()).unwrap_or(&[])
    }
}

/// The diagnostic of an artifact its language cannot answer for (E105): at the artifact, with what
/// the language says.
fn refused(m: &Model, a: &Artifact, said: &[Said]) -> Diag {
    let (t, f) = (a.tool.word(), shown(&a.path));
    let line = said.iter().find_map(|s| s.line).unwrap_or(1);
    let src = ritsu_base::fs::read_to_string(crate::paths::on_disk(&m.root, &a.path)).unwrap_or_default();
    let mut d = diag::at("E105", &a.path, line, 1, tr!("{f} が {t} の検査を通らないか、読めません", "The file {f} does not pass {t}'s check, or cannot be read")).source(&src);
    for s in said.iter().take(5) {
        let at = match s.line {
            Some(l) => format!("{f}:{l}"),
            None => f.clone(),
        };
        let code = if s.code.is_empty() { String::new() } else { format!("[{}] ", s.code) };
        d = d.note(tr!("{t} の診断: {code}{at}: {}", "what {t} says: {code}{at}: {}", s.message.ja; s.message.en));
    }
    if said.len() > 5 {
        let more = said.len() - 5;
        d = d.note(tr!("ほかに {more} 件", "and {more} more"));
    }
    d.note(tr!(
        "そのファイルを、{t} の検査を通るように直してください。検査を通らないファイルや読めないファイルからは参照を読み取れないので、sakai はその参照を確かめられません。",
        "Make the file pass {t}'s check; the references of a file that cannot be read cannot be checked."
    ))
}

/// Read what the other languages say of the map's artifacts (DESIGN 4.1): rulec's facts and the
/// references of each rule, and the references of each calendar and workflow. A language not
/// joined is E104 once, at the first of its artifacts; a file its language cannot answer for is
/// E105.
pub fn read(m: &Model, arts: &[Artifact], suite: &Suite) -> (Read, Vec<Diag>) {
    let mut out = Read { complete: true, index: suite.index.clone(), ..Read::default() };
    let mut diags = Vec::new();
    for tool in Suite::READ {
        let mine: Vec<&Artifact> = arts.iter().filter(|a| a.tool == tool).collect();
        let Some(first) = mine.first() else { continue };
        if !suite.reads(tool) {
            out.complete = false;
            let (t, n, f) = (tool.word(), mine.len(), shown(&first.path));
            let cmd = with_ritsu(m);
            let (ci, oi) = first.owner.expect("every artifact has an owner past stage 2");
            let c = &m.contexts[ci];
            let pos = c.owns[oi].pos;
            diags.push(
                diag::at("E104", &c.file, pos.line, pos.col, tr!("この sakai は {t} の成果物を読めません（{n} 件。最初は {f}）", "This sakai cannot read {t} artifacts ({n} of them, the first {f})"))
                    .source(&c.src)
                    .note(tr!(
                        "sakai 単独のバイナリには、ほかの言語が入っていません。ほかの言語の成果物まで確かめるには、すべての言語をつないだ ritsu で、`{cmd}` のように走らせてください。",
                        "The binary of sakai's own crate holds no other language; run it with every language joined, through ritsu: `{cmd}`."
                    )),
            );
            continue;
        }
        for a in mine {
            let abs = m.root.join(&a.path);
            if tool == Tool::Rulec {
                match suite.facts(&abs) {
                    Some(Ok(f)) => {
                        out.facts.insert(a.path.clone(), f);
                    }
                    Some(Err(said)) => {
                        out.complete = false;
                        diags.push(refused(m, a, &said));
                        continue;
                    }
                    None => continue,
                }
            }
            match suite.references(tool, &m.root, &a.path) {
                Some(Ok(rs)) => out.refs.push((a.path.clone(), tool, rs)),
                Some(Err(said)) => {
                    out.complete = false;
                    diags.push(refused(m, a, &said));
                }
                None => {}
            }
        }
    }
    (out, diags)
}

/// The accounts and the transfers of a book, by their names, as the pages of `doc` list them
/// (DESIGN 4.5): what chobo answers through `Books`, None when chobo is not joined.
pub fn book_names(suite: &Suite, abs: &Path) -> Option<Result<(Vec<String>, Vec<String>), Vec<Said>>> {
    suite.book(abs).map(|r| r.map(|f| (f.accounts.iter().map(|a| a.name.clone()).collect(), f.transfers.iter().map(|t| t.name.clone()).collect())))
}
