//! What the tests share: the books, and the command; the rest is ritsu-testkit's.
#![allow(dead_code)]

pub use std::path::{Path, PathBuf};

pub mod runners;
pub mod servers;

pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The root of ritsu's workspace, two directories above this crate.
pub fn repo() -> PathBuf {
    root().parent().and_then(Path::parent).expect("the crate is in crates/ of the workspace").to_path_buf()
}

/// chobo's site, which ritsu's site holds at website/chobo (its English pages in docs, the
/// Japanese ones in docs-ja).
pub fn site() -> PathBuf {
    repo().join("website/chobo")
}

/// Every `.book` directly under `dir`, in name order, without the `.before.book` ones.
pub fn books_in(dir: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(root().join(dir))
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "book") && !p.to_string_lossy().ends_with(".before.book"))
        .collect();
    v.sort();
    v
}

/// Every `.book` of the examples, one directory after another.
pub fn example_books() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root().join("examples")).unwrap().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    dirs.sort();
    dirs.iter().flat_map(|d| books_in(&d.strip_prefix(root()).unwrap().to_string_lossy())).collect()
}

pub fn stem(p: &Path) -> String {
    p.file_stem().unwrap().to_string_lossy().to_string()
}

/// Golden files, temporary directories, SKIP lines and the levels are ritsu-testkit's (the golden
/// files are written again under `CHOBO_BLESS=1` or `RITSU_BLESS=1`).
#[allow(unused_imports)]
pub use ritsu_testkit::golden::{bless, golden};
#[allow(unused_imports)]
pub use ritsu_testkit::{Need, TempDir, need, skip};

/// A program that talks to a database is run to its end with a limit (ritsu-testkit's `finish`):
/// past it, it is stopped with every process it started, and the test fails there with what it
/// printed, not at the limit of the job with nothing said.
#[allow(unused_imports)]
pub use ritsu_testkit::run::finish;

/// How long psql or mysql may take with one file or one script: a second or two each.
pub const SQL_LIMIT: std::time::Duration = std::time::Duration::from_secs(5 * 60);

/// How long a client of a book may take against its database or ledger: a runner of the
/// scenarios, or the calls at once under REPEATABLE READ, with `go run` building it first — a
/// minute at most on a busy machine.
pub const CLIENT_LIMIT: std::time::Duration = std::time::Duration::from_secs(10 * 60);

pub fn chobo() -> std::process::Command {
    let mut c = std::process::Command::new(env!("CARGO_BIN_EXE_chobo"));
    c.env_remove("CHOBO_LANG").env_remove("RITSU_LANG");
    c
}
