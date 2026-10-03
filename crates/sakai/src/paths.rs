//! Paths (DESIGN 2.4, 1.3): how a path written in a `.ctx` becomes a path from the root, where
//! the root is, and which files of the scope are artifacts.
//!
//! A path from the root is a string: the components joined with `/`, with no `.` or `..`, and
//! `.` for the root itself. Every path sakai holds has this form, and so does every path of
//! `api` and of the JSON. A diagnostic writes it from where sakai was run ([`shown`]).
//!
//! How a path is joined, folded, shown and walked is ritsu's, written once (`ritsu_base::paths`);
//! what is sakai's is the words a diagnostic says a written path is wrong in, and the one way
//! a run shows its paths, said once by the command line.

use ritsu_base::text::Text;

pub use ritsu_base::paths::{PathError, Shown, SKIPPED, absolute, between, contains, depth, find_root, from_root, has_skipped_part, is_absolute, join, on_disk, parent, relative, skipped_name, walk};

/// Why a written path cannot be a path from the root (E012), in sakai's words.
pub fn error_text(e: PathError, written: &str) -> Text {
    match e {
        PathError::Absolute => tr!(
            "絶対パス \"{written}\" は書けません。パスは、書いたファイルのディレクトリからの相対で書きます",
            "the absolute path \"{written}\" cannot be written; a path is written from the directory of the file it is in"
        ),
        PathError::Outside => tr!("パス \"{written}\" はルートの外に出ます", "the path \"{written}\" goes outside the root"),
        PathError::Empty => tr!("パスが空です。書いたファイルのディレクトリそのものは \".\" と書きます", "the path is empty; the directory of the file itself is written \".\""),
    }
}

static SHOWN: std::sync::OnceLock<Shown> = std::sync::OnceLock::new();

/// Said once, by the command line, before anything is checked. The library and the tests that
/// call it leave it unsaid, and their paths are written from the root: the same paths, when the
/// command is run at the root.
pub fn show_from(s: Shown) {
    let _ = SHOWN.set(s);
}

/// A path from the root, as the diagnostics write it.
pub fn shown(p: &str) -> String {
    match SHOWN.get() {
        Some(s) => s.path(p),
        None => p.to_string(),
    }
}

/// The root as the run writes it, for the JSON: `.` when sakai runs there.
pub fn shown_root() -> String {
    shown(".")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_written_path_becomes_a_path_from_the_root() {
        assert_eq!(join("contexts", "../inventory"), Ok("inventory".into()));
        assert_eq!(join(".", "./calendars/../支払条件.cal"), Ok("支払条件.cal".into()));
        assert_eq!(join(".", "."), Ok(".".into()));
        assert_eq!(join("a/b", "../../.."), Err(PathError::Outside));
        assert_eq!(join(".", "/etc/hosts"), Err(PathError::Absolute));
        assert_eq!(join(".", "C:/x"), Err(PathError::Absolute));
    }

    #[test]
    fn containment_and_depth() {
        assert!(contains(".", "a/b"));
        assert!(contains("a", "a/b"));
        assert!(contains("a", "a"));
        assert!(!contains("a", "ab/c"));
        assert_eq!(depth("."), 0);
        assert_eq!(depth("a/b"), 2);
        assert_eq!(relative("ctx", "proto/x.proto"), "../proto/x.proto");
        assert_eq!(relative(".", "proto"), "proto");
    }
}
