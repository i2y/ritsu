//! Unified diffs, from `git diff` or `diff -u`, and the hashes of the two sides of
//! each file (DESIGN §7.5). A git diff names the sides' blobs on its `index` lines;
//! a plain diff is held to the file on disk, which has to be one of its sides, and
//! the other side is the disk with the hunks undone or done. Reading a diff is ritsu's
//! (`ritsu_base::udiff`, which yuen reads diffs with too); the blobs are geas's.

use ritsu_base::text::Text;
use crate::hash;
use std::path::Path;

pub use ritsu_base::udiff::{Content, DiffError, FileDiff, Hunk, HunkLine, Mark, parse};
use ritsu_base::udiff::{fits, other_side};

/// One side's blob: absent, or its hash (from an `index` line, perhaps abbreviated,
/// or computed in full).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Blob {
    Absent,
    Hash(String),
}

impl Blob {
    /// Whether a recorded hash (or a file the record does not have) is this side:
    /// an abbreviated hash matches by prefix.
    pub fn matches(&self, recorded: Option<&str>) -> bool {
        match (self, recorded) {
            (Blob::Absent, None) => true,
            (Blob::Hash(h), Some(r)) => !h.is_empty() && r.starts_with(h.as_str()),
            _ => false,
        }
    }

    /// Seven hex digits, as a person reads a blob.
    pub fn short(&self) -> String {
        match self {
            Blob::Absent => "-".into(),
            Blob::Hash(h) => h.chars().take(7).collect(),
        }
    }
}

fn from_index(h: &str) -> Blob {
    if h.chars().all(|c| c == '0') { Blob::Absent } else { Blob::Hash(h.to_ascii_lowercase()) }
}

/// The blobs of a file's two sides, before and after: from the `index` line when
/// the diff has one; else from the file on disk, which has to be one of the sides.
pub fn sides(f: &FileDiff, root: &Path) -> Result<(Blob, Blob), Text> {
    if let (Some(a), Some(b)) = (&f.old_blob, &f.new_blob) {
        return Ok((from_index(a), from_index(b)));
    }
    let read = |p: &str| std::fs::read(root.join(p)).ok().map(|b| Content::of(&b));
    let disk = read(f.path());
    if f.hunks.is_empty() {
        // a rename or a change of mode: the same bytes on both sides
        let other = f.old_path.as_deref().and_then(read);
        return match disk.or(other) {
            Some(c) => {
                let h = Blob::Hash(hash::blob(&c.bytes()));
                Ok((if f.added() { Blob::Absent } else { h.clone() }, if f.deleted() { Blob::Absent } else { h }))
            }
            None => Err(tr!(
                "{} がディスクになく、差分にもその blob がありません",
                "{} is not on disk, and the diff names no blob for it",
                f.path(),
            )),
        };
    }
    let blob = |c: &Content| Blob::Hash(hash::blob(&c.bytes()));
    let empty = Content { lines: vec![], final_newline: true };
    match disk {
        Some(c) if fits(&c, f, true) => {
            let before = if f.added() { Blob::Absent } else { blob(&other_side(&c, f, true)) };
            Ok((before, blob(&c)))
        }
        Some(c) if fits(&c, f, false) => {
            let after = if f.deleted() { Blob::Absent } else { blob(&other_side(&c, f, false)) };
            Ok((blob(&c), after))
        }
        None if f.deleted() => Ok((blob(&other_side(&empty, f, true)), Blob::Absent)),
        None if f.added() => Ok((Blob::Absent, blob(&other_side(&empty, f, false)))),
        _ => Err(tr!(
            "{} の差分が、ディスクのファイルにも、変更前のそのファイルにも合いません",
            "the diff of {} fits neither the file on disk nor that file before the change",
            f.path(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diff_of(text: &str) -> FileDiff {
        parse(text.as_bytes()).unwrap().remove(0)
    }

    #[test]
    fn added_and_deleted_files_from_the_disk() {
        let root = Path::new("/nonexistent-geas-test-root");
        let added = diff_of("--- /dev/null\n+++ n.py\n@@ -0,0 +1,2 @@\n+a\n+b\n");
        let (b, a) = sides(&added, root).unwrap();
        assert_eq!(b, Blob::Absent);
        assert_eq!(a, Blob::Hash(hash::blob(b"a\nb\n")));
        let deleted = diff_of("--- d.py\n+++ /dev/null\n@@ -1 +0,0 @@\n-gone\n");
        let (b, a) = sides(&deleted, root).unwrap();
        assert_eq!((b, a), (Blob::Hash(hash::blob(b"gone\n")), Blob::Absent));
        let changed = diff_of("--- c.py\n+++ c.py\n@@ -1 +1 @@\n-x\n+y\n");
        assert!(sides(&changed, root).is_err());
    }

    #[test]
    fn abbreviated_hashes_match_by_prefix() {
        let b = Blob::Hash("ce01362".into());
        assert!(b.matches(Some("ce013625030ba8dba906f756967f9e9ca394464a")));
        assert!(!b.matches(Some("e69de29bb2d1d6434b8b29ae775ad8c2e48c5391")));
        assert!(!b.matches(None));
        assert!(Blob::Absent.matches(None));
        assert_eq!(from_index("0000000"), Blob::Absent);
    }
}
