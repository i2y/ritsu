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
use crate::sha256;
use crate::sources::{Cited, Sources};

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
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unread {
    /// The file is not there, or is a directory (E201).
    Missing { dir: bool },
    /// Reading this tool's artifacts is not built yet: the suite's JSON is read by the next
    /// stage of yurai (PLAN C).
    NotYet(Tool),
}

/// The end of an artifact this stage reads: a `file`, its bytes.
pub fn artifact_end(p: &Project, n: &Name) -> Result<End, Unread> {
    match n.tool {
        Tool::File => {
            let abs = p.root.join(&n.path);
            if abs.is_dir() || n.path == "." {
                return Err(Unread::Missing { dir: true });
            }
            std::fs::read(&abs).map(End::of).map_err(|_| Unread::Missing { dir: false })
        }
        t => Err(Unread::NotYet(t)),
    }
}
