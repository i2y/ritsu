//! A directory under the system's temporary directory that is removed when its value is
//! dropped, a failing test's too: `ritsu-test-<crate>-<pid>-<n>-<what>`.
//!
//! A test process that is killed does not drop its directories. The first [`TempDir`] a test
//! process makes removes those of every test process that has ended (`kill -0` says there is no
//! such process), and first stops the servers such a process noted in its directory
//! ([`TempDir::note_server`]) while they are still the program it noted (a process ID can belong
//! to another program by then). sakai and chobo did each half of this.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

/// The start of every directory this module makes.
pub const PREFIX: &str = "ritsu-test-";

/// The file in a directory that names the servers started for it, a `<pid> <program>` line
/// each.
const SERVERS: &str = ".servers";

pub struct TempDir(PathBuf);

impl TempDir {
    /// A fresh directory; `what` says what it is for (letters, digits and `-` are kept).
    pub fn new(what: &str) -> TempDir {
        static SWEPT: std::sync::Once = std::sync::Once::new();
        SWEPT.call_once(sweep);
        static N: AtomicUsize = AtomicUsize::new(0);
        let what: String = what.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' }).collect();
        let name = format!("{PREFIX}{}-{}-{}-{what}", crate::crate_name(), std::process::id(), N.fetch_add(1, Ordering::SeqCst));
        let p = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap_or_else(|e| panic!("cannot make {}: {e}", p.display()));
        // Through its symbolic links (macOS's /var is /private/var), so a path compares with
        // what a program run in it sees.
        TempDir(std::fs::canonicalize(&p).unwrap_or(p))
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    /// Write a file under the directory, making its directories. The path written.
    pub fn write(&self, rel: &str, bytes: impl AsRef<[u8]>) -> PathBuf {
        let p = self.0.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&p, bytes).unwrap_or_else(|e| panic!("cannot write {}: {e}", p.display()));
        p
    }

    /// Whether a file or a directory is there under the directory.
    pub fn exists(&self, rel: &str) -> bool {
        self.0.join(rel).exists()
    }

    /// A file under the directory.
    pub fn read(&self, rel: &str) -> String {
        let p = self.0.join(rel);
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()))
    }

    /// Note a server started for this directory, so that the first test process after this
    /// one stops it if this one is killed before it can.
    pub fn note_server(&self, pid: u32, program: &str) {
        let f = self.0.join(SERVERS);
        let mut text = std::fs::read_to_string(&f).unwrap_or_default();
        text.push_str(&format!("{pid} {program}\n"));
        let _ = std::fs::write(f, text);
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Copy a directory, every file under it, in name order.
pub fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    let mut es: Vec<_> = std::fs::read_dir(from).unwrap_or_else(|e| panic!("cannot read {}: {e}", from.display())).map(|e| e.unwrap()).collect();
    es.sort_by_key(|e| e.file_name());
    for e in es {
        let p = e.path();
        let q = to.join(e.file_name());
        if p.is_dir() {
            copy_dir(&p, &q);
        } else {
            std::fs::copy(&p, &q).unwrap_or_else(|e| panic!("cannot copy {}: {e}", p.display()));
        }
    }
}

/// Whether the process `pid` is there: `kill -0` succeeds, or fails for a reason other than
/// there being no such process (one kill may not ask about is kept).
pub fn alive(pid: u32) -> bool {
    match Command::new("kill").args(["-0", &pid.to_string()]).stdout(Stdio::null()).output() {
        Ok(o) => o.status.success() || !String::from_utf8_lossy(&o.stderr).contains("No such process"),
        Err(_) => true,
    }
}

/// The program a process runs, as `ps` names it.
fn program_of(pid: u32) -> String {
    Command::new("ps").args(["-p", &pid.to_string(), "-o", "comm="]).output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default()
}

/// The pid in a directory's name, after the prefix and the crate's name.
fn pid_of(name: &str) -> Option<u32> {
    let rest = name.strip_prefix(PREFIX)?;
    // `<crate>-<pid>-<n>-<what>`: a crate's name has no digits-only part, so the pid is the
    // first part that is digits only.
    rest.split('-').find(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())).and_then(|p| p.parse().ok())
}

/// Remove the directories of test processes that have ended, stopping first the servers they
/// noted.
pub fn sweep() {
    let me = std::process::id();
    let Ok(rd) = std::fs::read_dir(std::env::temp_dir()) else { return };
    let mut ended: std::collections::HashMap<u32, bool> = std::collections::HashMap::new();
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let Some(pid) = pid_of(&name) else { continue };
        if pid == me {
            continue;
        }
        let gone = *ended.entry(pid).or_insert_with(|| !alive(pid));
        if !gone {
            continue;
        }
        for line in std::fs::read_to_string(e.path().join(SERVERS)).unwrap_or_default().lines() {
            let mut it = line.split_whitespace();
            let (Some(Ok(spid)), Some(program)) = (it.next().map(str::parse::<u32>), it.next()) else { continue };
            if program_of(spid).contains(program) {
                let _ = Command::new("kill").args(["-9", &spid.to_string()]).stderr(Stdio::null()).status();
            }
        }
        let _ = std::fs::remove_dir_all(e.path());
    }
}
