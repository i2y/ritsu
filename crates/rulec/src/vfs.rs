//! Reading a file as of a git revision, and following a path a rule writes.
//!
//! `check --diff-base <rev>` reports what the base revision reported, and `replay` and `diff`
//! run a rule as it was at a revision; a rule that applies another (§15.69) has to read that
//! other rule from the same revision, not from the working tree. The read goes through
//! here, and a caller that works at a revision says so around the call.
//!
//! A path written in a rule (`apply`, `import`, `shape`, `source … file`, `contribution … to`,
//! `range from koyomi`) is followed from the directory of the file that wrote it ([`follow`]): in
//! a module of ritsu's — a directory with a `ritsu.mod` — it stays in the module, and `@<name>/…`
//! is a file of a dependency, read from the module's root (ritsu's DESIGN 17.6). Elsewhere it is
//! the directory joined to it, as it always was.

use ritsu_base::modules::Refused;
use std::cell::RefCell;
use std::path::{Path, PathBuf};

/// The directory a path written in the file at `base` is followed from: the file's directory,
/// else `dot`.
pub fn dir_of<'a>(base: &'a str, dot: &'static str) -> &'a Path {
    Path::new(base).parent().unwrap_or(Path::new(dot))
}

/// The file a path written in a file of the directory `dir` names (ritsu's DESIGN 17.6): outside a
/// module of ritsu's, `dir.join(written)`, byte for byte; in one, the path held to the module, a
/// dependency's file written from the module's root (`@pricing/rules/fee.rule`).
pub fn follow(dir: &Path, written: &str) -> Result<PathBuf, Refused> {
    ritsu_base::modules::refer_shown(dir, written)
}

/// What a rule says, with its own code, of a path written in it that the module of ritsu's it is
/// in does not let it read (ritsu's DESIGN 17.20): one the module refuses (into a dependency it
/// does not require, out of the module, into a module below without an `@`), or a dependency's
/// file that is not there. None for any other path, which is read, or said as before.
pub fn refused(followed: &Result<PathBuf, Refused>) -> Option<String> {
    match followed {
        Err(r) => Some(crate::i18n::text(&r.text())),
        Ok(p) if !ritsu_base::fs::exists(p) => ritsu_base::modules::missing(p).map(|t| crate::i18n::text(&t)),
        Ok(_) => None,
    }
}

/// The note a "cannot read" of a path written in a file of the directory `dir` takes when the path
/// starts with `@`, no module of ritsu's is above `dir`, where it is a plain relative path, and no
/// file is there (ritsu's DESIGN 17.6).
pub fn outside_note(dir: &Path, written: &str) -> Option<String> {
    let note = ritsu_base::modules::outside_note_for(dir, written)?;
    (!ritsu_base::fs::exists(dir.join(written))).then(|| crate::i18n::text(&note))
}

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
    let Some(rev) = rev else { return ritsu_base::fs::read(path) };
    // a dependency's file is the one the lock pins, which no revision of this repository holds
    if ritsu_base::modules::map(path).is_some() {
        return ritsu_base::fs::read(path);
    }
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
