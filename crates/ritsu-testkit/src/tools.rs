//! Finding a program a test needs (DESIGN 10.8): `RITSU_<TOOL>`, then the crate's own variable
//! (`KOYOMI_<TOOL>`, the names the crates had before ritsu), then a place in the crate (a venv,
//! `tools/node_modules`), then the PATH. A variable that is set but empty counts as not set.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The variables that may name a tool, in the order they are read: `RITSU_<TOOL>` and the
/// crate's own `<CRATE>_<TOOL>` (`tool` is written as in the variable: `PG_BIN`, `CHROME`).
pub fn vars(tool: &str) -> Vec<String> {
    let mut v = vec![format!("RITSU_{tool}")];
    let own = format!("{}_{tool}", crate::crate_var_prefix());
    if own != v[0] {
        v.push(own);
    }
    v
}

/// The first of the variables that is set and not empty.
pub fn from_vars(tool: &str) -> Option<String> {
    vars(tool).into_iter().find_map(|v| std::env::var(v).ok().filter(|s| !s.is_empty()))
}

/// An executable named `name` in a directory of the PATH.
pub fn on_path(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|p| std::env::split_paths(&p).map(|d| d.join(name)).find(|f| f.is_file()))
}

/// Whether `cmd args…` runs and succeeds.
pub fn runs(cmd: impl AsRef<std::ffi::OsStr>, args: &[&str]) -> bool {
    Command::new(cmd).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
}

/// The first line a program prints for `arg` (`--version`), on its standard output or, when
/// that is empty, its standard error.
pub fn version(cmd: impl AsRef<std::ffi::OsStr>, arg: &str) -> String {
    Command::new(cmd)
        .arg(arg)
        .output()
        .map(|o| {
            let s = if o.stdout.is_empty() { o.stderr } else { o.stdout };
            String::from_utf8_lossy(&s).lines().next().unwrap_or("").trim().to_string()
        })
        .unwrap_or_default()
}

/// A program: named by [`vars`], else at `fallback` (relative to the crate's directory, where
/// the tests run), else `name` on the PATH when `name probe…` runs.
pub fn find(tool: &str, fallback: Option<&Path>, name: &str, probe: &[&str]) -> Option<PathBuf> {
    if let Some(p) = from_vars(tool) {
        return Some(PathBuf::from(p));
    }
    if let Some(f) = fallback
        && f.exists()
    {
        return Some(std::fs::canonicalize(f).unwrap_or_else(|_| f.to_path_buf()));
    }
    if runs(name, probe) { Some(on_path(name).unwrap_or_else(|| PathBuf::from(name))) } else { None }
}
