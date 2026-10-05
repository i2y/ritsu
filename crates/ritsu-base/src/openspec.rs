//! OpenSpec's specs and changes, read as OpenSpec 1.14 reads them (Fission-AI's OpenSpec, the npm
//! package `@fission-ai/openspec`): the requirements of a spec, each with its block and its
//! scenarios; what a change's delta spec adds, modifies, removes and renames; and where a
//! project's specs and changes sit. yuen pins the requirements of a spec as the articles of a
//! source (yuen's DESIGN 20), and geas holds the scenarios of a spec to the claims of the same
//! names (geas's DESIGN 17). What a requirement means is neither's, nor this module's.
//!
//! The rules are OpenSpec's own (`src/core/parsers/` in its repository at v1.14.0):
//!
//! - A UTF-8 byte order mark is dropped, and CR LF and CR are read as LF.
//! - A line inside a fenced code block (``` or ~~~, three or more, closed by a fence of the same
//!   character at least as long) is never structure.
//! - A spec's requirements are the `### Requirement: <name>` headers of its `## Requirements`
//!   section, which runs to the next `## ` header.
//! - A requirement's block runs from its header to the next requirement header or `## ` header,
//!   with the white space at its end removed: the text `openspec archive` puts in place of the old
//!   one when a change modifies the requirement, and leaves alone when the change touches another.
//! - A requirement's name is the header's text after `Requirement:`, without a closing run of `#`
//!   and trimmed, and is compared as written: the name archive matches a MODIFIED, REMOVED or
//!   RENAMED header against.
//! - A scenario is a `#### ` header of a requirement whose body is not empty; its name drops the
//!   closing run of `#` and a leading `Scenario:`, and its body runs to the next header of level 4
//!   or above.
//! - A delta spec's sections are `## ADDED Requirements`, `## MODIFIED Requirements`,
//!   `## REMOVED Requirements` and `## RENAMED Requirements`, their titles compared without case.
//!   A removal is a requirement header or a bullet holding one; a rename is a `FROM:` line and the
//!   `TO:` line after it. Archive applies them in the order RENAMED, REMOVED, MODIFIED, ADDED.

use crate::paths;
use std::path::Path;

/// One scenario of a requirement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scenario {
    pub name: String,
    /// Its header's line in the file, from 1.
    pub line: usize,
    /// The lines of its body, as written, without the blank lines at either end.
    pub body: Vec<String>,
}

/// One requirement of a spec, or of a delta spec's ADDED or MODIFIED section.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Requirement {
    pub name: String,
    /// Its header's line in the file, from 1.
    pub line: usize,
    /// The last line of its block.
    pub last: usize,
    /// The block: the header and every line after it to the next requirement or section, the
    /// white space at its end removed, its lines joined by LF.
    pub block: String,
    pub scenarios: Vec<Scenario>,
}

/// The requirements of a spec, in the order it writes them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spec {
    pub requirements: Vec<Requirement>,
}

impl Spec {
    pub fn get(&self, name: &str) -> Option<&Requirement> {
        self.requirements.iter().find(|r| r.name == name)
    }

    /// The names of the spec that differ from `name` only in case or in the spaces inside it:
    /// what OpenSpec takes for a mistake, never for another requirement.
    pub fn near(&self, name: &str) -> Vec<&str> {
        let f = fold(name);
        self.requirements.iter().filter(|r| r.name != name && fold(&r.name) == f).map(|r| r.name.as_str()).collect()
    }
}

/// Why a file does not read as a spec.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpecError {
    /// The file is not UTF-8.
    NotUtf8,
    /// It has no `## Requirements` section outside a code block (a delta spec has none either).
    NoRequirements { delta: bool },
    /// Two requirements of one name: archive could keep one block and drop the other.
    Twice { name: String, first: usize, line: usize },
}

/// One thing a delta spec does to a requirement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Op {
    Renamed,
    Removed,
    Modified,
    Added,
}

impl Op {
    /// As OpenSpec writes the section: `RENAMED`, `REMOVED`, `MODIFIED`, `ADDED`.
    pub fn word(self) -> &'static str {
        match self {
            Op::Renamed => "RENAMED",
            Op::Removed => "REMOVED",
            Op::Modified => "MODIFIED",
            Op::Added => "ADDED",
        }
    }
}

/// One entry of a delta spec.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub op: Op,
    /// The requirement's name: for a rename, the name it is given (`TO:`).
    pub name: String,
    /// For a rename, the name it had (`FROM:`).
    pub from: Option<String>,
    /// Its line in the delta spec, from 1.
    pub line: usize,
    /// For ADDED and MODIFIED, the requirement as the change writes it.
    pub requirement: Option<Requirement>,
}

/// What a delta spec does, in the order archive applies it (RENAMED, REMOVED, MODIFIED, ADDED),
/// and in each section in the order the file writes it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Delta {
    pub changes: Vec<Change>,
}

/// The text with a byte order mark dropped and its lines ended by LF (OpenSpec's
/// `normalizeContent`).
pub fn normalize(text: &str) -> String {
    let t = text.strip_prefix('\u{feff}').unwrap_or(text);
    t.replace("\r\n", "\n").replace('\r', "\n")
}

/// Which lines are inside a fenced code block, the fences included (OpenSpec's
/// `buildCodeFenceMask`).
pub fn fence_mask(lines: &[&str]) -> Vec<bool> {
    let mut mask = vec![false; lines.len()];
    let mut open: Option<(char, usize)> = None;
    for (i, l) in lines.iter().enumerate() {
        let t = l.trim_start();
        let run = |c: char| t.chars().take_while(|x| *x == c).count();
        match open {
            None => {
                for c in ['`', '~'] {
                    let n = run(c);
                    if n >= 3 {
                        open = Some((c, n));
                        mask[i] = true;
                        break;
                    }
                }
            }
            Some((c, n)) => {
                mask[i] = true;
                let k = run(c);
                if k >= n && t[k * c.len_utf8()..].trim().is_empty() {
                    open = None;
                }
            }
        }
    }
    mask
}

/// A requirement's name as archive matches it: a closing run of `#` (one with a space or a tab
/// before it) removed, and trimmed (OpenSpec's `normalizeRequirementName`).
pub fn normalize_name(name: &str) -> String {
    let t = name.trim_end_matches([' ', '\t']);
    let hashes = t.len() - t.trim_end_matches('#').len();
    let without = &t[..t.len() - hashes];
    let s = if hashes > 0 && without.ends_with([' ', '\t']) { without } else { name };
    s.trim().to_string()
}

/// A name folded for telling a mistake from another name: lower case, every run of white space
/// one space (OpenSpec's `foldRequirementName`).
pub fn fold(name: &str) -> String {
    normalize_name(name).to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `###` (white space after it or not), `Requirement:` in any case, and a name.
fn requirement_header(line: &str) -> Option<String> {
    let rest = line.strip_prefix("###")?;
    let rest = rest.trim_start();
    let word = rest.get(..12)?;
    if !word.eq_ignore_ascii_case("requirement:") {
        return None;
    }
    let name = rest[12..].trim();
    if name.is_empty() {
        return None;
    }
    Some(normalize_name(name))
}

/// `##` and white space: where a section starts, and the one before it ends. `###` is not one.
fn is_section(line: &str) -> bool {
    line.strip_prefix("##").is_some_and(|rest| rest.starts_with(char::is_whitespace))
}

/// The title of a section's header (`## ` and a title).
fn section_title(line: &str) -> Option<&str> {
    if !is_section(line) {
        return None;
    }
    let t = line[2..].trim();
    if t.is_empty() { None } else { Some(t) }
}

/// A header of level `1..=max` with white space after its `#`s.
fn header_up_to(line: &str, max: usize) -> bool {
    let n = line.chars().take_while(|c| *c == '#').count();
    (1..=max).contains(&n) && line[n..].starts_with(char::is_whitespace)
}

fn scenario_header(line: &str) -> bool {
    line.starts_with("####") && line[4..].starts_with(char::is_whitespace)
}

/// A scenario's name from its header line (OpenSpec's `scenarioNameAt`).
fn scenario_name(line: &str) -> String {
    let t = line[4..].trim_start();
    let n = normalize_name(t);
    let lower = n.to_lowercase();
    let n = if lower.starts_with("scenario:") { n["scenario:".len()..].trim_start().to_string() } else { n };
    n.trim().to_string()
}

/// The scenarios of one requirement's block (its lines from the header, `first` the header's line
/// in the file).
fn scenarios(lines: &[&str], first: usize) -> Vec<Scenario> {
    let mask = fence_mask(lines);
    let mut out = Vec::new();
    for i in 1..lines.len() {
        if mask[i] || !scenario_header(lines[i]) {
            continue;
        }
        let mut end = i + 1;
        while end < lines.len() && (mask[end] || !header_up_to(lines[end], 4)) {
            end += 1;
        }
        let mut body: Vec<String> = lines[i + 1..end].iter().map(|l| l.trim_end().to_string()).collect();
        while body.first().is_some_and(|l| l.trim().is_empty()) {
            body.remove(0);
        }
        while body.last().is_some_and(|l| l.trim().is_empty()) {
            body.pop();
        }
        if body.is_empty() {
            continue;
        }
        out.push(Scenario { name: scenario_name(lines[i]), line: first + i, body });
    }
    out
}

/// The requirement blocks among `lines` (`at` the first one's line in the file, from 1): every
/// requirement header outside a code block, to the next one.
fn blocks(lines: &[&str], mask: &[bool], at: usize) -> Vec<Requirement> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let name = if mask[i] { None } else { requirement_header(lines[i]) };
        let Some(name) = name else {
            i += 1;
            continue;
        };
        let start = i;
        i += 1;
        while i < lines.len() && (mask[i] || (requirement_header(lines[i]).is_none() && !is_section(lines[i]))) {
            i += 1;
        }
        let block = lines[start..i].join("\n").trim_end().to_string();
        let count = block.split('\n').count();
        let own: Vec<&str> = lines[start..start + count].to_vec();
        out.push(Requirement { name, line: at + start, last: at + start + count - 1, scenarios: scenarios(&own, at + start), block });
    }
    out
}

/// Whether a text has a section of a delta spec.
pub fn is_delta(text: &str) -> bool {
    let t = normalize(text);
    let lines: Vec<&str> = t.split('\n').collect();
    let mask = fence_mask(&lines);
    lines.iter().zip(&mask).any(|(l, m)| !m && section_title(l).is_some_and(|t| delta_op(t).is_some()))
}

fn delta_op(title: &str) -> Option<Op> {
    match title.to_lowercase().as_str() {
        "added requirements" => Some(Op::Added),
        "modified requirements" => Some(Op::Modified),
        "removed requirements" => Some(Op::Removed),
        "renamed requirements" => Some(Op::Renamed),
        _ => None,
    }
}

/// Read a spec (`openspec/specs/<capability>/spec.md`).
pub fn read_spec(bytes: &[u8]) -> Result<Spec, SpecError> {
    let text = std::str::from_utf8(bytes).map_err(|_| SpecError::NotUtf8)?;
    let t = normalize(text);
    let lines: Vec<&str> = t.split('\n').collect();
    let mask = fence_mask(&lines);
    let Some(head) = (0..lines.len()).find(|&i| !mask[i] && section_title(lines[i]).is_some_and(|t| t.eq_ignore_ascii_case("requirements"))) else {
        return Err(SpecError::NoRequirements { delta: is_delta(&t) });
    };
    let end = (head + 1..lines.len()).find(|&i| !mask[i] && is_section(lines[i])).unwrap_or(lines.len());
    let requirements = blocks(&lines[head + 1..end], &mask[head + 1..end], head + 2);
    for (k, r) in requirements.iter().enumerate() {
        if let Some(first) = requirements[..k].iter().find(|x| x.name == r.name) {
            return Err(SpecError::Twice { name: r.name.clone(), first: first.line, line: r.line });
        }
    }
    Ok(Spec { requirements })
}

/// `FROM:` or `TO:` (a bullet before it or not), and a requirement header, in backquotes or not.
fn rename_line(line: &str, word: &str) -> Option<String> {
    let t = line.trim_start();
    let t = t.strip_prefix(['-', '*', '+']).unwrap_or(t).trim_start();
    let t = t.strip_prefix(word)?.trim_start();
    let t = t.strip_prefix('`').unwrap_or(t);
    let t = t.trim_end().strip_suffix('`').unwrap_or(t.trim_end());
    requirement_header(t)
}

/// A removal written as a bullet holding a requirement header.
fn removed_bullet(line: &str) -> Option<String> {
    let t = line.trim_start();
    let t = t.strip_prefix(['-', '*', '+'])?.trim_start();
    let t = t.strip_prefix('`').unwrap_or(t);
    let t = t.trim_end().strip_suffix('`').unwrap_or(t.trim_end());
    requirement_header(t)
}

/// Read a delta spec (`openspec/changes/<id>/specs/<capability>/spec.md`). What OpenSpec refuses
/// to apply (a `FROM:` without its `TO:`) is left out.
pub fn read_delta(bytes: &[u8]) -> Result<Delta, SpecError> {
    let text = std::str::from_utf8(bytes).map_err(|_| SpecError::NotUtf8)?;
    let t = normalize(text);
    let lines: Vec<&str> = t.split('\n').collect();
    let mask = fence_mask(&lines);
    let heads: Vec<(usize, &str)> = (0..lines.len()).filter(|&i| !mask[i]).filter_map(|i| section_title(lines[i]).map(|t| (i, t))).collect();
    let mut changes = Vec::new();
    for (k, (h, title)) in heads.iter().enumerate() {
        let Some(op) = delta_op(title) else { continue };
        let end = heads.get(k + 1).map(|(i, _)| *i).unwrap_or(lines.len());
        let (body, bmask, at) = (&lines[h + 1..end], &mask[h + 1..end], h + 2);
        match op {
            Op::Added | Op::Modified => {
                for r in blocks(body, bmask, at) {
                    changes.push(Change { op, name: r.name.clone(), from: None, line: r.line, requirement: Some(r) });
                }
            }
            Op::Removed => {
                for (i, l) in body.iter().enumerate() {
                    if bmask[i] {
                        continue;
                    }
                    if let Some(name) = requirement_header(l).or_else(|| removed_bullet(l)) {
                        changes.push(Change { op, name, from: None, line: at + i, requirement: None });
                    }
                }
            }
            Op::Renamed => {
                let mut pending: Option<(String, usize)> = None;
                for (i, l) in body.iter().enumerate() {
                    if bmask[i] {
                        continue;
                    }
                    if let Some(from) = rename_line(l, "FROM:") {
                        pending = Some((from, at + i));
                    } else if let Some(to) = rename_line(l, "TO:")
                        && let Some((from, line)) = pending.take()
                    {
                        changes.push(Change { op, name: to, from: Some(from), line, requirement: None });
                    }
                }
            }
        }
    }
    changes.sort_by_key(|c| c.op);
    Ok(Delta { changes })
}

/// Where a spec sits among OpenSpec's directories: for `<dir>/openspec/specs/<capability>/spec.md`
/// (a path from the root, `/` between its parts), `<dir>/openspec` and the capability, which may
/// hold a `/` (`identity/user-auth`).
pub fn layout(spec: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = spec.split('/').collect();
    if parts.last() != Some(&"spec.md") {
        return None;
    }
    let at = (0..parts.len().saturating_sub(1)).rev().find(|&i| parts[i] == "specs" && i >= 1 && parts[i - 1] == "openspec")?;
    if at + 2 >= parts.len() {
        return None;
    }
    let dir = parts[..at].join("/");
    let cap = parts[at + 1..parts.len() - 1].join("/");
    Some((dir, cap))
}

/// One change under `<openspec>/changes/` that is not archived.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Active {
    /// The change's name: its directory.
    pub id: String,
    /// Its directory, from the root.
    pub dir: String,
    /// Each delta spec it holds, from the root, with the capability it changes, in path order.
    pub deltas: Vec<(String, String)>,
}

/// The changes under `<openspec>/changes/` (a path from the root), leaving out `archive/`, in the
/// order of their names.
pub fn active_changes(root: &Path, openspec: &str) -> Vec<Active> {
    let changes = if openspec.is_empty() || openspec == "." { "changes".to_string() } else { format!("{openspec}/changes") };
    let Ok(rd) = crate::fs::read_dir(paths::on_disk(root, &changes)) else { return vec![] };
    let mut names: Vec<String> = rd.filter_map(|e| e.ok()).filter(|e| e.file_type().is_ok_and(|t| t.is_dir())).map(|e| e.file_name().to_string_lossy().to_string()).collect();
    names.sort();
    let mut out = Vec::new();
    for id in names {
        if id == "archive" || paths::skipped_name(&id) {
            continue;
        }
        let dir = format!("{changes}/{id}");
        let specs = format!("{dir}/specs");
        let mut files = Vec::new();
        paths::walk(root, &specs, &[], &mut files);
        let deltas = files
            .into_iter()
            .filter(|f| f.ends_with("/spec.md"))
            .filter_map(|f| {
                let cap = f.strip_prefix(&format!("{specs}/"))?.strip_suffix("/spec.md")?.to_string();
                Some((f, cap))
            })
            .collect();
        out.push(Active { id, dir, deltas });
    }
    out
}
