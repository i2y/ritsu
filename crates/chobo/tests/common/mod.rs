//! What the tests share: the books, and the command; the rest is ritsu-testkit's.
#![allow(dead_code)]

pub use std::path::{Path, PathBuf};

pub mod runners;
pub mod servers;

pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
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

pub fn chobo() -> std::process::Command {
    let mut c = std::process::Command::new(env!("CARGO_BIN_EXE_chobo"));
    c.env_remove("CHOBO_LANG").env_remove("RITSU_LANG");
    c
}
