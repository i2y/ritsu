//! The crates of the map's Rust code (DESIGN 7.7): what Cargo says the workspace at the map's
//! `code rust` place holds, and which crates each one depends on by path. `check` asks Cargo once
//! a map — `cargo metadata --format-version 1 --no-deps --offline`, which reads the manifests and
//! neither builds nor reaches the network — and reads the line of each dependency from the
//! manifest itself, for a diagnostic to point at. The dependencies of the tests (`[dev-dependencies]`)
//! are not read (DESIGN 3.7), nor those on a crate of a registry or a git repository, which is in
//! no context.

use crate::diag::{self, Diag};
use crate::model::Model;
use crate::owners::MANIFEST;
use crate::paths::shown;
use std::path::{Path, PathBuf};

/// A crate of the workspace.
#[derive(Clone, Debug, PartialEq)]
pub struct Crate {
    /// As its manifest names it (`ritsu-ports`).
    pub name: String,
    /// Its manifest and the directory of it, from the root.
    pub manifest: String,
    pub dir: String,
    pub deps: Vec<Dep>,
}

/// A dependency of a crate on another by its path.
#[derive(Clone, Debug, PartialEq)]
pub struct Dep {
    /// The key the manifest writes it under: the crate's name, or the name it is renamed to.
    pub key: String,
    /// `dependencies` or `build-dependencies`.
    pub table: &'static str,
    /// The directory of the crate depended on, from the root; None when it is outside the root.
    pub dir: Option<String>,
    pub line: usize,
    pub col: usize,
    /// The line as the manifest writes it.
    pub text: String,
}

/// The crates Cargo says the workspace holds, in the order it says them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Crates {
    pub list: Vec<Crate>,
}

impl Crates {
    /// The crate whose manifest is in the directory `dir` (a path from the root).
    pub fn by_dir(&self, dir: &str) -> Option<&Crate> {
        self.list.iter().find(|c| c.dir == dir)
    }

    /// The crate of the manifest `manifest` (a path from the root).
    pub fn by_manifest(&self, manifest: &str) -> Option<&Crate> {
        self.list.iter().find(|c| c.manifest == manifest)
    }
}

/// The manifest of the crate in `dir` (a path from the root).
pub fn manifest_of(dir: &str) -> String {
    if dir == "." { MANIFEST.to_string() } else { format!("{dir}/{MANIFEST}") }
}

/// A path Cargo writes, from the root; None when it is outside it.
fn from_root(root: &Path, p: &str) -> Option<String> {
    let p = ritsu_base::fs::canonicalize(p).unwrap_or_else(|_| PathBuf::from(p));
    let rel = p.strip_prefix(root).ok()?;
    let parts: Vec<String> = rel.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect();
    Some(if parts.is_empty() { ".".to_string() } else { parts.join("/") })
}

/// The line of the manifest `src` a dependency is written on, under `[<table>]`, under
/// `[target.<…>.<table>]`, or as a table of its own (`[<table>.<key>]`): its number, the column of
/// the key, and the line.
pub fn line_of(src: &str, key: &str, table: &str) -> Option<(usize, usize, String)> {
    let unquote = |s: &str| s.trim().trim_matches('"').trim_matches('\'').to_string();
    let mut current = String::new();
    let in_table = |h: &str| h == table || (h.starts_with("target.") && h.ends_with(&format!(".{table}")));
    for (i, raw) in src.lines().enumerate() {
        let line = raw.trim_start();
        let col = raw.chars().take_while(|c| c.is_whitespace()).count() + 1;
        if line.starts_with('[') {
            if line.starts_with("[[") {
                current.clear();
                continue;
            }
            current = line[1..].split(']').next().unwrap_or("").chars().filter(|c| !c.is_whitespace()).collect();
            let own = current.strip_prefix(&format!("{table}.")).map(unquote).or_else(|| {
                let at = current.find(&format!(".{table}."))?;
                current.starts_with("target.").then(|| unquote(&current[at + table.len() + 2..]))
            });
            if own.as_deref() == Some(key) {
                return Some((i + 1, col, raw.trim().to_string()));
            }
            continue;
        }
        if in_table(&current) && line.contains('=') {
            let k = unquote(line.split(['=', '.']).next().unwrap_or(""));
            if k == key {
                return Some((i + 1, col, raw.trim().to_string()));
            }
        }
    }
    None
}

/// The crates of the map's Rust code, or None when the map writes no `code rust`, or when Cargo
/// cannot say them (E107): then no dependency of a crate is checked, and no crate held to a
/// published language's heading.
pub fn read(m: &Model) -> (Option<Crates>, Vec<Diag>) {
    let Some(code) = m.map.code.iter().find(|c| c.language == "rust") else { return (None, vec![]) };
    let at = m.map.ast.code.iter().find(|c| c.language == "rust").map(|c| c.pos).unwrap_or_default();
    let place = crate::naming::quote(&shown(&code.path));
    let fail = |why: ritsu_base::text::Text, note: Option<ritsu_base::text::Text>| {
        let mut d = diag::at("E107", &m.map.file, at.line, at.col, tr!("{place} の Rust のクレートを、Cargo から読めません: {}", "Cargo cannot say the crates of the Rust code at {place}: {}", why.ja; why.en)).source(&m.map.src);
        if let Some(n) = note {
            d = d.note(n);
        }
        d.note(tr!(
            "sakai は、`code rust` の場所のワークスペースのクレートとその依存を、`cargo metadata --format-version 1 --no-deps --offline` で読みます。マニフェストを読むだけで、ビルドもネットワークへの接続もしません。",
            "The crates of the workspace at the `code rust` place and their dependencies are asked of `cargo metadata --format-version 1 --no-deps --offline`, which reads the manifests, and neither builds nor reaches the network."
        ))
    };
    let disk = crate::paths::on_disk(&m.root, &code.path);
    let manifest = disk.join(MANIFEST);
    if !ritsu_base::fs::is_file(&manifest) {
        let note = tr!(
            "`code rust` には、ワークスペースの（クレートが一つなら、そのクレートの）`Cargo.toml` のあるディレクトリを書いてください。",
            "`code rust` names the directory of the workspace's `Cargo.toml` (or, for one crate, of its own)."
        );
        return (None, vec![fail(tr!("Cargo.toml がありません", "there is no Cargo.toml"), Some(note))]);
    }
    let root = ritsu_base::fs::canonicalize(&m.root).unwrap_or_else(|_| m.root.clone());
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let out = std::process::Command::new(&cargo).args(["metadata", "--format-version", "1", "--no-deps", "--offline", "--manifest-path"]).arg(&manifest).current_dir(&disk).output();
    let out = match out {
        Ok(o) => o,
        Err(e) => {
            let e = e.to_string();
            return (None, vec![fail(tr!("cargo を走らせられません（{e}）", "cargo cannot be run ({e})"), None)]);
        }
    };
    if !out.status.success() {
        // Cargo's first line, with the root's absolute path taken off what it names.
        let err = String::from_utf8_lossy(&out.stderr).to_string();
        let mut first = err.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim().trim_start_matches("error:").trim().to_string();
        for r in [root.to_string_lossy().to_string(), m.root.to_string_lossy().to_string()] {
            if !r.is_empty() {
                first = first.replace(&format!("{r}/"), "");
            }
        }
        return (None, vec![fail(ritsu_base::text::Text::same(first), None)]);
    }
    let v: serde_json::Value = match serde_json::from_slice(&out.stdout) {
        Ok(v) => v,
        Err(e) => {
            let e = e.to_string();
            return (None, vec![fail(tr!("cargo metadata の JSON を読めません（{e}）", "the JSON of cargo metadata does not read ({e})"), None)]);
        }
    };
    let mut list = Vec::new();
    for p in v["packages"].as_array().into_iter().flatten() {
        let (Some(name), Some(mp)) = (p["name"].as_str(), p["manifest_path"].as_str()) else { continue };
        let Some(manifest) = from_root(&root, mp) else { continue };
        let dir = crate::paths::parent(&manifest);
        let src = ritsu_base::fs::read_to_string(crate::paths::on_disk(&m.root, &manifest)).unwrap_or_default();
        let mut deps = Vec::new();
        for d in p["dependencies"].as_array().into_iter().flatten() {
            let table = match d["kind"].as_str() {
                None => "dependencies",
                Some("build") => "build-dependencies",
                _ => continue,
            };
            let Some(path) = d["path"].as_str() else { continue };
            let key = d["rename"].as_str().or(d["name"].as_str()).unwrap_or("").to_string();
            let (line, col, text) = line_of(&src, &key, table).unwrap_or((1, 1, String::new()));
            deps.push(Dep { key, table, dir: from_root(&root, path), line, col, text });
        }
        list.push(Crate { name: name.to_string(), manifest, dir, deps });
    }
    (Some(Crates { list }), vec![])
}

#[cfg(test)]
mod tests {
    use super::line_of;

    #[test]
    fn the_line_of_a_dependency() {
        let src = "[package]\nname = \"a\"\n\n[dependencies]\nb = { path = \"../b\" }\n\"c\".path = \"../c\"\n\n[build-dependencies]\nb = { path = \"../b\" }\n\n[dependencies.d]\npath = \"../d\"\n\n[target.'cfg(unix)'.dependencies]\n  e = { path = \"../e\" }\n";
        assert_eq!(line_of(src, "b", "dependencies"), Some((5, 1, "b = { path = \"../b\" }".into())));
        assert_eq!(line_of(src, "c", "dependencies"), Some((6, 1, "\"c\".path = \"../c\"".into())));
        assert_eq!(line_of(src, "b", "build-dependencies"), Some((9, 1, "b = { path = \"../b\" }".into())));
        assert_eq!(line_of(src, "d", "dependencies"), Some((11, 1, "[dependencies.d]".into())));
        assert_eq!(line_of(src, "e", "dependencies"), Some((15, 3, "e = { path = \"../e\" }".into())));
        assert_eq!(line_of(src, "name", "dependencies"), None);
    }
}
