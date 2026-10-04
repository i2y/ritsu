//! The ends of a link (DESIGN 3.2, 4.1, PLAN B.5): what a person looked at, as bytes, and the
//! first sixteen digits of its SHA-256.
//!
//! A requirement's end is its text, a line for every upper end it comes from — an article's
//! copy, a file source, a requirement it is read from, each with its hash — and its period,
//! the lines after the first in the order of their UTF-8 bytes. Putting the upper ends' hashes
//! in is what carries a change downstream: when an article changes, so does every requirement
//! that cites it, and every link below those.

use crate::ast::FromWhat;
use crate::names::{Name, Tool};
use crate::project::Project;
use ritsu_base::sha256;
use crate::sources::{Cited, Sources};
use ritsu_ports::Lookup;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct End {
    pub bytes: Vec<u8>,
    pub hash: String,
}

impl End {
    pub fn of(bytes: Vec<u8>) -> End {
        let hash = sha256::short(&bytes);
        End { bytes, hash }
    }
}

/// The upper ends of one `from` line, when they can be read.
pub enum Upper {
    Cited(Vec<Cited>),
    Req(usize),
}

/// The upper ends of every `from` line of a requirement version, None where a line's cannot
/// be read (a copy missing or not matching its pin, a source not declared, a requirement that
/// does not resolve).
pub fn uppers(p: &Project, s: &Sources, r: usize) -> Vec<Option<Upper>> {
    let fi = p.reqs[r].file;
    let d = p.decl(r);
    d.from
        .iter()
        .enumerate()
        .map(|(i, f)| match &f.what {
            FromWhat::Cite { source, fragments, .. } => s.cited(fi, source, fragments).map(Upper::Cited),
            FromWhat::Req(_) => p.names.from[r][i].map(Upper::Req),
        })
        .collect()
}

/// Every requirement version's end, in an order where a requirement comes after those it is
/// read from. None for a requirement whose end cannot be made: one of its upper ends cannot be
/// read, it is in a cycle of `from`s, or it is read from one that has no end.
pub fn requirement_ends(p: &Project, s: &Sources, in_cycle: &[bool]) -> Vec<Option<End>> {
    let n = p.reqs.len();
    let mut ends: Vec<Option<End>> = vec![None; n];
    let mut done = vec![false; n];
    // Repeat until nothing more can be made: a requirement waits for those it is read from.
    loop {
        let mut progressed = false;
        for r in 0..n {
            if done[r] {
                continue;
            }
            if in_cycle[r] {
                done[r] = true;
                progressed = true;
                continue;
            }
            let ups = uppers(p, s, r);
            let waiting = ups.iter().any(|u| matches!(u, Some(Upper::Req(t)) if !done[*t]));
            if waiting {
                continue;
            }
            done[r] = true;
            progressed = true;
            ends[r] = requirement_end(p, r, &ups, &ends);
        }
        if !progressed || done.iter().all(|d| *d) {
            break;
        }
    }
    ends
}

/// One requirement version's end (DESIGN 4.1), given its upper ends and the ends made so far.
pub fn requirement_end(p: &Project, r: usize, ups: &[Option<Upper>], ends: &[Option<End>]) -> Option<End> {
    let d = p.decl(r);
    let (text, _) = d.text.as_ref()?;
    let mut lines: Vec<String> = Vec::new();
    for u in ups {
        match u.as_ref()? {
            Upper::Cited(cs) => lines.extend(cs.iter().map(|c| c.line.clone())),
            Upper::Req(t) => {
                let e = ends[*t].as_ref()?;
                let v = &p.reqs[*t];
                lines.push(format!("from requirement {} v{} sha256:{}", v.name, v.version, e.hash));
            }
        }
    }
    if let Some((period, _)) = &d.in_force {
        lines.push(format!("in force {period}"));
    }
    lines.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
    let mut out = format!("text {text}\n");
    for l in lines {
        out.push_str(&l);
        out.push('\n');
    }
    Some(End::of(out.into_bytes()))
}

/// What an artifact is, when it cannot be read.
#[derive(Clone, Debug, PartialEq)]
pub enum Unread {
    /// The file is not there, or is a directory (E201).
    Missing { dir: bool },
    /// The language of the file is not handed to yuen (the binary of yuen's own crate):
    /// `ritsu yuen` runs it with every language joined (exit 2).
    NoPort(Tool),
    /// The language cannot answer for the file: it does not pass the language's check, or does
    /// not read (E203). What the language says.
    Refused(Vec<ritsu_ports::Said>),
    /// The file holds no such thing (E202): the things of the same kind it holds, each with its
    /// end, for the candidates.
    NoSuchName { same_kind: Vec<(Name, End)> },
    /// A `.proto` that does not read (E205): the file, and why.
    Proto(String, ritsu_base::text::Text),
}

/// The end of an artifact (DESIGN 3.2): a file's bytes; a thing of a file of rulec, koyomi,
/// chobo, geas, dandori or sakai, its definition as its language gives it (`Items`, found in the
/// project's index); an element
/// of a `.proto`, the text of DESIGN 3.4. A file of rulec or koyomi is held to its language's
/// check first (`Sources`, which answers only for a file that passes it), and a file of the
/// others to its language reading it (`Items`).
pub fn artifact_end(p: &Project, n: &Name) -> Result<End, Unread> {
    let abs = if n.path == "." { p.root.clone() } else { p.root.join(&n.path) };
    if abs.is_dir() {
        return Err(Unread::Missing { dir: true });
    }
    if !abs.is_file() {
        return Err(Unread::Missing { dir: false });
    }
    let bytes = || std::fs::read(&abs).map(End::of).map_err(|_| Unread::Missing { dir: false });
    match n.tool {
        Tool::File => bytes(),
        Tool::Proto => {
            let ps = crate::proto::load(&p.root, &n.path).map_err(|(f, why)| Unread::Proto(f, why))?;
            if n.items.is_empty() {
                return bytes();
            }
            crate::proto::end_text(&ps, n).map(|t| End::of(t.into_bytes())).map_err(|_| {
                let kind = n.kind().unwrap_or("");
                let same_kind = crate::proto::gather(&ps, &Name { tool: n.tool, path: n.path.clone(), items: n.items[..n.items.len() - 1].to_vec() }, kind)
                    .into_iter()
                    .filter_map(|m| crate::proto::end_text(&ps, &m).ok().map(|t| (m, End::of(t.into_bytes()))))
                    .collect();
                Unread::NoSuchName { same_kind }
            })
        }
        t if crate::suite::Suite::READ.contains(&t) => {
            if !p.suite.reads(t) {
                return Err(Unread::NoPort(t));
            }
            if matches!(t, Tool::Rulec | Tool::Koyomi)
                && let Some(Err(said)) = p.suite.sources(t, &abs)
            {
                return Err(Unread::Refused(said));
            }
            // looked up in the project's index, which asks the language of the file once in a run
            // (ritsu's DESIGN 6.4, X10); what is wrong, yuen says in its own codes
            match p.suite.index.find(&p.root, n) {
                Lookup::NotJoined => Err(Unread::NoPort(t)),
                Lookup::Refused(said) => Err(Unread::Refused(said)),
                Lookup::Found(None) => bytes(),
                // a definition with nothing in it is no end to hold a link to (ritsu's PLAN 7.6)
                Lookup::Found(Some(i)) if i.text.is_empty() => Err(Unread::Refused(vec![ritsu_ports::Said {
                    code: String::new(),
                    file: n.path.clone(),
                    line: Some(i.lines.0),
                    message: tr!("{} は定義の文を渡しません", "{} gives no definition", n.text()),
                }])),
                Lookup::Found(Some(i)) => Ok(End::of(i.text.into_bytes())),
                Lookup::Missing(same) => {
                    let same_kind = same.into_iter().filter(|i| !i.text.is_empty()).map(|i| (i.naming, End::of(i.text.into_bytes()))).collect();
                    Err(Unread::NoSuchName { same_kind })
                }
            }
        }
        // yuen's own namings are refused with E012 before anything is read
        t => Err(Unread::NoPort(t)),
    }
}
