//! What the tests share: the books, goldens, and temporary directories that are cleaned up.
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

pub fn bless() -> bool {
    std::env::var("CHOBO_BLESS").is_ok_and(|v| v == "1")
}

/// Compare with the golden file, or write it under CHOBO_BLESS=1.
pub fn golden(path: &Path, actual: &str) {
    if bless() {
        std::fs::write(path, actual).unwrap();
        return;
    }
    let want = std::fs::read_to_string(path).unwrap_or_else(|_| panic!("no golden {}; run with CHOBO_BLESS=1", path.display()));
    if want != actual {
        let (w, a): (Vec<&str>, Vec<&str>) = (want.lines().collect(), actual.lines().collect());
        let at = w.iter().zip(&a).position(|(x, y)| x != y).unwrap_or(w.len().min(a.len()));
        panic!(
            "{} differs at line {}:\n  golden: {:?}\n  now:    {:?}\n(CHOBO_BLESS=1 writes the new one)",
            path.display(),
            at + 1,
            w.get(at),
            a.get(at)
        );
    }
}

/// A directory for one test, under the system's temporary directory, removed when dropped.
/// The first one a test process makes also removes what earlier test processes left behind.
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new(name: &str) -> TempDir {
        static SWEPT: std::sync::Once = std::sync::Once::new();
        SWEPT.call_once(sweep);
        let p = std::env::temp_dir().join(format!("chobo-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        TempDir(p)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Is the process `pid` still there.
pub fn alive(pid: u32) -> bool {
    std::process::Command::new("kill").arg("-0").arg(pid.to_string()).stderr(std::process::Stdio::null()).status().is_ok_and(|s| s.success())
}

/// Remove `chobo-test-<pid>-…` directories whose process has ended, stopping first the servers
/// it left running (their process IDs are in the directory's `servers`).
fn sweep() {
    let Ok(rd) = std::fs::read_dir(std::env::temp_dir()) else { return };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let Some(rest) = name.strip_prefix("chobo-test-") else { continue };
        let Some(pid) = rest.split('-').next().and_then(|p| p.parse::<u32>().ok()) else { continue };
        if pid == std::process::id() || alive(pid) {
            continue;
        }
        for server in std::fs::read_to_string(e.path().join("servers")).unwrap_or_default().lines() {
            let Ok(spid) = server.trim().parse::<u32>() else { continue };
            // only if it is still one of ours: the ID may belong to another process by now
            let comm = std::process::Command::new("ps").args(["-p", &spid.to_string(), "-o", "comm="]).output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
            if comm.contains("postgres") || comm.contains("tigerbeetle") {
                let _ = std::process::Command::new("kill").arg("-9").arg(spid.to_string()).stderr(std::process::Stdio::null()).status();
            }
        }
        let _ = std::fs::remove_dir_all(e.path());
    }
}

pub fn chobo() -> std::process::Command {
    let mut c = std::process::Command::new(env!("CARGO_BIN_EXE_chobo"));
    c.env_remove("CHOBO_LANG");
    c
}
