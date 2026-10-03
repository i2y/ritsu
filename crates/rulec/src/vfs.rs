//! Reading a file as of a git revision.
//!
//! `check --diff-base <rev>` reports what the base revision reported, and `replay` and `diff`
//! run a rule as it was at a revision; a rule that applies another (§15.69) has to read that
//! other rule from the same revision, not from the working tree. The read goes through
//! here, and a caller that works at a revision says so around the call.

use std::cell::RefCell;
use std::path::Path;

thread_local! {
    static REV: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Run `f` with every read through this module taken from `rev` (`git show rev:path`).
pub fn with_git_rev<R>(rev: &str, f: impl FnOnce() -> R) -> R {
    let before = REV.with(|r| r.replace(Some(rev.to_string())));
    let out = f();
    REV.with(|r| *r.borrow_mut() = before);
    out
}

/// The bytes of a file, from the working tree or from the revision in force.
pub fn read(path: &Path) -> std::io::Result<Vec<u8>> {
    let rev = REV.with(|r| r.borrow().clone());
    let Some(rev) = rev else { return std::fs::read(path) };
    // git resolves `rev:./p` against the current directory, as the working tree read does.
    let spec = if path.is_absolute() {
        format!("{rev}:{}", path.display())
    } else {
        format!("{rev}:./{}", path.display())
    };
    let out = std::process::Command::new("git").args(["show", &spec]).output()?;
    if out.status.success() {
        Ok(out.stdout)
    } else {
        Err(std::io::Error::new(std::io::ErrorKind::NotFound, format!("no `{spec}`")))
    }
}

pub fn read_to_string(path: &Path) -> std::io::Result<String> {
    let bytes = read(path)?;
    String::from_utf8(bytes).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}
