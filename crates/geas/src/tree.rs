//! The tree a record describes (DESIGN §7.4): its root, the source files under it,
//! and the language each is written in, by its extension. Every path a record holds
//! is relative to the root and written with `/`, so a record names no directory of
//! the machine it was made on.

use std::io;
use std::path::{Component, Path, PathBuf};

/// Path components whose files are not the project's own source: geas's and git's
/// directories, installed packages, Python's byte code, and Cargo's build directory
/// (which holds generated `.rs` files and the test scratch of a Rust project).
pub const EXCLUDED: &[&str] = &[".geas", ".git", "node_modules", "site-packages", "__pycache__", "target"];

/// The runtimes geas records, each named after what reports the lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Runtime {
    Python,
    Node,
    Go,
    Rust,
}

impl Runtime {
    /// The runtime of a source file, by its extension; None for any other file.
    pub fn of(path: &str) -> Option<Runtime> {
        let ext = path.rsplit_once('.').map(|(_, e)| e)?;
        if path.ends_with('/') || ext.contains('/') {
            return None;
        }
        match ext {
            "py" => Some(Runtime::Python),
            "js" | "mjs" | "cjs" | "ts" | "mts" | "cts" => Some(Runtime::Node),
            "go" => Some(Runtime::Go),
            "rs" => Some(Runtime::Rust),
            _ => None,
        }
    }

    /// The word the record writes.
    pub fn word(self) -> &'static str {
        match self {
            Runtime::Python => "python",
            Runtime::Node => "node",
            Runtime::Go => "go",
            Runtime::Rust => "rust",
        }
    }

    pub fn parse(s: &str) -> Option<Runtime> {
        [Runtime::Python, Runtime::Node, Runtime::Go, Runtime::Rust].into_iter().find(|r| r.word() == s)
    }
}

/// Whether a path relative to the root has a component the record leaves out.
pub fn excluded(rel: &str) -> bool {
    rel.split('/').any(|c| EXCLUDED.contains(&c))
}

/// A source file the record covers: a mapped extension, no excluded component.
pub fn is_source(rel: &str) -> bool {
    Runtime::of(rel).is_some() && !excluded(rel)
}

/// The nearest directory at or above `start` for which `is_root` holds.
pub fn nearest(start: &Path, is_root: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        if is_root(d) {
            return Some(d.to_path_buf());
        }
        dir = d.parent();
    }
    None
}

/// The root of a spec in `spec_dir` (absolute): the nearest directory holding
/// `.git`, a directory or the file a worktree has, found by looking, not by running
/// git; else the spec's own directory.
pub fn root_of(spec_dir: &Path) -> PathBuf {
    nearest(spec_dir, |d| d.join(".git").exists()).unwrap_or_else(|| spec_dir.to_path_buf())
}

/// `p` relative to `root`, with `/`; None when it is not under the root. Both are
/// absolute and canonical.
pub fn relative(root: &Path, p: &Path) -> Option<String> {
    let rest = p.strip_prefix(root).ok()?;
    let mut parts = Vec::new();
    for c in rest.components() {
        match c {
            Component::Normal(s) => parts.push(s.to_str()?.to_string()),
            _ => return None,
        }
    }
    Some(parts.join("/"))
}

/// How to get from `from` up to `root`: `../..`, or `.` when they are the same
/// directory. `from` is under `root`.
pub fn up_to(rel_from: &str) -> String {
    if rel_from.is_empty() {
        ".".to_string()
    } else {
        vec![".."; rel_from.split('/').count()].join("/")
    }
}

/// Every source file under the root, as relative paths, sorted. Symbolic links are
/// not followed: git keeps a link as the path it points to, not as that file.
pub fn sources(root: &Path) -> io::Result<Vec<String>> {
    let mut out = Vec::new();
    walk(root, "", &mut out)?;
    out.sort();
    Ok(out)
}

fn walk(dir: &Path, rel: &str, out: &mut Vec<String>) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(String::from) else {
            continue;
        };
        if EXCLUDED.contains(&name.as_str()) {
            continue;
        }
        let r = if rel.is_empty() { name } else { format!("{rel}/{name}") };
        let kind = entry.file_type()?;
        if kind.is_dir() {
            walk(&entry.path(), &r, out)?;
        } else if kind.is_file() && Runtime::of(&r).is_some() {
            out.push(r);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtimes_by_extension() {
        assert_eq!(Runtime::of("a/b.py"), Some(Runtime::Python));
        for f in ["s.js", "s.mjs", "s.cjs", "s.ts", "s.mts", "s.cts"] {
            assert_eq!(Runtime::of(f), Some(Runtime::Node), "{f}");
        }
        assert_eq!(Runtime::of("main.go"), Some(Runtime::Go));
        assert_eq!(Runtime::of("src/main.rs"), Some(Runtime::Rust));
        assert_eq!(Runtime::of("README.md"), None);
        assert_eq!(Runtime::of("Makefile"), None);
        assert_eq!(Runtime::of("a.d/Makefile"), None);
        assert_eq!(Runtime::parse("node"), Some(Runtime::Node));
    }

    #[test]
    fn excluded_components() {
        assert!(excluded(".geas/hook/sitecustomize.py"));
        assert!(excluded("web/node_modules/x/index.js"));
        assert!(excluded("venv/lib/python3.13/site-packages/a.py"));
        assert!(excluded("target/debug/build/x/out/gen.rs"));
        assert!(!excluded("src/targets.rs"));
        assert!(is_source("examples/greeter/server.py"));
        assert!(!is_source("examples/greeter/greeter.geas"));
    }

    #[test]
    fn the_root_is_the_nearest_directory_with_git() {
        let has = |dirs: &'static [&'static str]| move |d: &Path| dirs.iter().any(|x| d == Path::new(x));
        let start = Path::new("/w/repo/examples/greeter");
        // a `.git` above the spec
        assert_eq!(nearest(start, has(&["/w/repo"])), Some(PathBuf::from("/w/repo")));
        // the nearest wins over one further up, and the spec's own directory counts
        assert_eq!(nearest(start, has(&["/w/repo", "/w/repo/examples/greeter"])), Some(start.to_path_buf()));
        // neither: the caller falls back to the spec's directory
        assert_eq!(nearest(start, has(&[])), None);
    }

    #[test]
    fn relative_paths_and_the_way_up() {
        let root = Path::new("/w/repo");
        assert_eq!(relative(root, Path::new("/w/repo/a/b.py")).as_deref(), Some("a/b.py"));
        assert_eq!(relative(root, Path::new("/w/repo")).as_deref(), Some(""));
        assert_eq!(relative(root, Path::new("/w/other/b.py")), None);
        assert_eq!(up_to("examples/greeter"), "../..");
        assert_eq!(up_to(""), ".");
    }
}
