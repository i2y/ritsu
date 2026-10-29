//! Digests (DESIGN 17.11): `sha256:` and 64 lowercase hex digits. The digest of a module's files
//! is the digest of the tar of them, and the tar is written one way only ([`super::tar`]), so the
//! files in a directory give the digest without a tar on the disk: the store and the copies of a
//! module are checked so.
//!
//! What a module carries ([`module_files`]): every file under its root but the names a walk passes
//! over (DESIGN 4.7), the names that start with `.`, a module below it with its own `ritsu.mod`, and
//! what its `exclude` lines name. The files of a commit in a git repository are chosen by the same
//! rule ([`carried_of_tree`]).

use super::manifest::Manifest;
use super::tar;
use crate::fs::{self, Kind};
use crate::paths;
use crate::text::Text;
use crate::tr;
use std::path::Path;

/// `sha256:` and the hex of the bytes' SHA-256.
pub fn of(bytes: &[u8]) -> String {
    format!("sha256:{}", crate::sha256::hex(bytes))
}

/// Whether `s` is a digest as ritsu writes one.
pub fn is_digest(s: &str) -> bool {
    hex(s).is_some()
}

/// The 64 hex digits of a digest.
pub fn hex(s: &str) -> Option<&str> {
    let h = s.strip_prefix("sha256:")?;
    (h.len() == 64 && h.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))).then_some(h)
}

/// Every file under `dir`, by its path from `dir` with `/` between the parts, in the order of the
/// bytes of the paths, read through [`crate::fs::raw`] (a copy of a module held in memory is read
/// as one on the disk is). A symbolic link, or anything else that is neither a file nor a
/// directory, is refused: a tar of a module holds files only.
pub fn files_in(dir: &Path) -> Result<Vec<(String, Vec<u8>)>, Text> {
    let mut out = Vec::new();
    gather(dir, "", &mut out, &|_, _| false)?;
    out.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    Ok(out)
}

/// The digest of the files under `dir`: the digest of their tar.
pub fn files(dir: &Path) -> Result<String, Text> {
    let fs = files_in(dir)?;
    let t = tar::write(&fs).map_err(|e| e.text())?;
    Ok(of(&t))
}

/// The paths a module's `exclude` lines name, from its root.
pub fn excludes_of(manifest: &Manifest) -> Vec<String> {
    manifest.excludes.iter().filter_map(|e| paths::join(".", &e.value).ok()).collect()
}

/// Whether a module passes over `rel`, a file or a directory (`is_dir`) under its root, when it
/// gathers the files it carries (DESIGN 17.12, step 2): a name a walk passes over (DESIGN 4.7), a
/// path one of its `exclude` lines names, or a directory below the root with a `ritsu.mod` of its
/// own (`has_mod`), which is another module. What is under a directory passed over is passed over.
pub fn passed_over(rel: &str, is_dir: bool, excludes: &[String], has_mod: &dyn Fn(&str) -> bool) -> bool {
    let name = rel.rsplit('/').next().unwrap_or(rel);
    paths::skipped_name(name) || excludes.iter().any(|e| paths::contains(e, rel)) || (is_dir && has_mod(rel))
}

/// The files a module carries (DESIGN 17.12, step 2), read from its root `root` through
/// [`crate::fs::raw`]: its `ritsu.mod` and `ritsu.lock` among them.
pub fn module_files(root: &Path, manifest: &Manifest) -> Result<Vec<(String, Vec<u8>)>, Text> {
    let excludes = excludes_of(manifest);
    let has_mod = |rel: &str| fs::raw::metadata(&root.join(rel).join(super::manifest::FILE)).is_ok_and(|m| m.is_file());
    let mut out = Vec::new();
    let skip = |rel: &str, kind: Kind| -> bool { passed_over(rel, kind == Kind::Dir, &excludes, &has_mod) };
    gather(root, "", &mut out, &skip)?;
    out.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    at_root(out.iter().map(|(p, _)| p.as_str()))?;
    Ok(out)
}

/// The entries of a tree of a git repository a module carries (DESIGN 17.12.1): `entries` is each
/// entry under the module's root, by its path from the root and whether it is a tree, as
/// `git ls-tree -r -t` lists them; what comes back is the indexes of the entries that are no tree
/// and that the walk of [`module_files`] would gather from a copy of the commit on the disk. A
/// directory with an entry `ritsu.mod` of its own is another module, as on the disk.
pub fn carried_of_tree(entries: &[(&str, bool)], excludes: &[String]) -> Result<Vec<usize>, Text> {
    let with_mod: std::collections::BTreeSet<&str> =
        entries.iter().filter(|(_, tree)| !tree).filter_map(|(p, _)| p.strip_suffix(super::manifest::FILE).and_then(|d| d.strip_suffix('/'))).collect();
    let has_mod = |rel: &str| with_mod.contains(rel);
    let mut out = Vec::new();
    'entries: for (i, (p, tree)) in entries.iter().enumerate() {
        if *tree {
            continue;
        }
        // every directory above it, as the walk enters them
        let mut at = 0;
        while let Some(j) = p[at..].find('/') {
            if passed_over(&p[..at + j], true, excludes, &has_mod) {
                continue 'entries;
            }
            at += j + 1;
        }
        if !passed_over(p, false, excludes, &has_mod) {
            out.push(i);
        }
    }
    out.sort_by(|&a, &b| entries[a].0.as_bytes().cmp(entries[b].0.as_bytes()));
    at_root(out.iter().map(|&i| entries[i].0))?;
    Ok(out)
}

/// A name right under a module's root that starts with `@`, which names a dependency.
fn at_root<'a>(mut carried: impl Iterator<Item = &'a str>) -> Result<(), Text> {
    if let Some(p) = carried.find(|p| p.starts_with('@')) {
        let top = p.split('/').next().unwrap_or(p);
        return Err(tr!(
            "モジュールのルートに `{top}` があります。ルートのすぐ下の `@` で始まる名前は依存を指すので、ファイルやディレクトリには付けられません",
            "the root of the module has `{top}` in it; a name that starts with `@` right under the root names a dependency, so no file or directory takes one"
        ));
    }
    Ok(())
}

/// The files under `dir`/`rel`, those `skip` passes over left out.
fn gather(dir: &Path, rel: &str, out: &mut Vec<(String, Vec<u8>)>, skip: &dyn Fn(&str, Kind) -> bool) -> Result<(), Text> {
    let here = if rel.is_empty() { dir.to_path_buf() } else { dir.join(rel) };
    let entries = fs::raw::read_dir(&here).map_err(|e| {
        let d = here.display();
        tr!("ディレクトリ {d} を読めません（{e}）", "cannot read the directory {d} ({e})")
    })?;
    for (name, kind) in entries {
        let Some(name) = name.to_str().map(str::to_string) else {
            let d = here.display();
            return Err(tr!("{d} に、UTF-8 でない名前のファイルがあります", "{d} has a file whose name is not UTF-8"));
        };
        let p = if rel.is_empty() { name.clone() } else { format!("{rel}/{name}") };
        if skip(&p, kind) {
            continue;
        }
        match kind {
            Kind::Dir => gather(dir, &p, out, skip)?,
            Kind::File => {
                let bytes = fs::raw::read(&dir.join(&p)).map_err(|e| tr!("`{p}` を読めません（{e}）", "cannot read `{p}` ({e})"))?;
                out.push((p, bytes));
            }
            Kind::Other => {
                return Err(tr!(
                    "`{p}` はふつうのファイルでもディレクトリでもありません（シンボリックリンクなど）。モジュールが運ぶのはふつうのファイルだけです",
                    "`{p}` is neither a file nor a directory (a symbolic link, say); a module carries files only"
                ));
            }
        }
    }
    Ok(())
}
