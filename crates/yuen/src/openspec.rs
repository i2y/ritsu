//! OpenSpec's changes, as they bear on the requirements a project pins (DESIGN 20.5): what each
//! change not yet archived does to a spec, for `yuen source outdated`, and what the delta spec a
//! diff touches does, for `yuen affected`. The reading of the Markdown is ritsu's base layer's
//! (`ritsu_base::openspec`), as OpenSpec's own.

use ritsu_base::openspec::{self, Op};
use std::path::Path;

/// One thing a change does to a requirement of a spec.
#[derive(Clone, Debug)]
pub struct Touch {
    /// The change's name, its directory under `changes/`.
    pub change: String,
    /// The delta spec, from the root.
    pub delta: String,
    /// Its line in the delta spec.
    pub line: usize,
    pub op: Op,
    /// The requirement as the spec names it now (for a rename, the name it had).
    pub name: String,
    /// For a rename, the name it is given.
    pub to: Option<String>,
    /// For MODIFIED, the block the change writes, and the block the spec has now (when it has it).
    pub new_block: Option<String>,
    pub old_block: Option<String>,
}

/// What the spec at `spec` (a path from the root) is to OpenSpec's directories, and every change
/// under them not yet archived that has a delta spec for it, with what each does: None when the
/// spec does not sit at `<dir>/openspec/specs/<capability>/spec.md`.
pub fn pending(root: &Path, spec: &str) -> Option<Vec<Touch>> {
    let (dir, cap) = openspec::layout(spec)?;
    let current = ritsu_base::fs::read(ritsu_base::paths::on_disk(root, spec)).ok().and_then(|b| openspec::read_spec(&b).ok());
    let mut out = Vec::new();
    for a in openspec::active_changes(root, &dir) {
        for (delta, c) in &a.deltas {
            if *c != cap {
                continue;
            }
            let Ok(bytes) = ritsu_base::fs::read(ritsu_base::paths::on_disk(root, delta)) else { continue };
            let Ok(d) = openspec::read_delta(&bytes) else { continue };
            out.extend(touches(&a.id, delta, &d, current.as_ref()));
        }
    }
    Some(out)
}

/// For a delta spec of a change (`<dir>/openspec/changes/<id>/specs/<capability>/spec.md`, a path
/// from the root): the spec it changes, the change's name, and what it does. None for a path of
/// another shape, or a delta spec that does not read.
pub fn of_delta(root: &Path, delta: &str) -> Option<(String, String, Vec<Touch>)> {
    let parts: Vec<&str> = delta.split('/').collect();
    let at = (0..parts.len()).rev().find(|&i| parts[i] == "changes" && i >= 1 && parts[i - 1] == "openspec")?;
    if parts.len() < at + 5 || parts[at + 2] != "specs" || parts.last() != Some(&"spec.md") || parts[at + 1] == "archive" {
        return None;
    }
    let id = parts[at + 1].to_string();
    let cap = parts[at + 3..parts.len() - 1].join("/");
    let dir = parts[..at].join("/");
    let spec = if dir.is_empty() { format!("specs/{cap}/spec.md") } else { format!("{dir}/specs/{cap}/spec.md") };
    let bytes = ritsu_base::fs::read(ritsu_base::paths::on_disk(root, delta)).ok()?;
    let d = openspec::read_delta(&bytes).ok()?;
    let current = ritsu_base::fs::read(ritsu_base::paths::on_disk(root, &spec)).ok().and_then(|b| openspec::read_spec(&b).ok());
    let t = touches(&id, delta, &d, current.as_ref());
    Some((spec, id, t))
}

fn touches(change: &str, delta: &str, d: &openspec::Delta, current: Option<&openspec::Spec>) -> Vec<Touch> {
    d.changes
        .iter()
        .map(|c| {
            let (name, to) = match c.op {
                Op::Renamed => (c.from.clone().unwrap_or_default(), Some(c.name.clone())),
                _ => (c.name.clone(), None),
            };
            let new_block = match c.op {
                Op::Modified => c.requirement.as_ref().map(|r| r.block.clone()),
                _ => None,
            };
            // A MODIFIED after a RENAMED in the same delta names the requirement by its new name.
            let renamed_from = d.changes.iter().find(|x| x.op == Op::Renamed && x.name == c.name).and_then(|x| x.from.clone());
            let now = current.and_then(|s| s.get(renamed_from.as_deref().unwrap_or(&c.name)).or_else(|| s.get(&c.name)));
            let old_block = match c.op {
                Op::Modified => now.map(|r| r.block.clone()),
                _ => None,
            };
            let name = if c.op == Op::Modified { renamed_from.unwrap_or(name) } else { name };
            Touch { change: change.to_string(), delta: delta.to_string(), line: c.line, op: c.op, name, to, new_block, old_block }
        })
        .collect()
}
