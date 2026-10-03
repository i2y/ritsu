//! Golden files (DESIGN 10.7): a test compares what it got with the file, or writes the file
//! instead when `RITSU_BLESS` (every crate) or the crate's own `<NAME>_BLESS` is set (`1`, or
//! set at all — the crates read either). A golden that differs says where, as a line diff.

use std::path::Path;

/// Whether the golden files are being written rather than compared.
pub fn bless() -> bool {
    let set = |v: String| std::env::var_os(v).is_some_and(|x| !x.is_empty() && x != "0");
    set("RITSU_BLESS".to_string()) || set(format!("{}_BLESS", crate::crate_var_prefix()))
}

/// Compare `actual` with the file at `path`, or write it there while blessing. Err says how
/// they differ.
pub fn check(path: &Path, actual: &str) -> Result<(), String> {
    if bless() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        return std::fs::write(path, actual).map_err(|e| e.to_string());
    }
    let want = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(_) => return Err(format!("{} is missing; write it with RITSU_BLESS=1 after reading what it would hold:\n{actual}", path.display())),
    };
    if want == actual {
        Ok(())
    } else {
        Err(format!("{} differs (- golden, + now; RITSU_BLESS=1 writes the new one):\n{}", path.display(), line_diff(&want, actual)))
    }
}

/// [`check`], failing the test.
pub fn golden(path: impl AsRef<Path>, actual: &str) {
    if let Err(e) = check(path.as_ref(), actual) {
        panic!("{e}");
    }
}

/// A line diff by the longest common subsequence (geas's), enough to read why a golden
/// failed: `  ` before a line in both, `- ` only in `a`, `+ ` only in `b`.
pub fn line_diff(a: &str, b: &str) -> String {
    let a: Vec<&str> = a.lines().collect();
    let b: Vec<&str> = b.lines().collect();
    let (n, m) = (a.len(), b.len());
    let mut lcs = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if a[i] == b[j] { lcs[i + 1][j + 1] + 1 } else { lcs[i + 1][j].max(lcs[i][j + 1]) };
        }
    }
    let mut out = String::new();
    let (mut i, mut j) = (0, 0);
    while i < n || j < m {
        if i < n && j < m && a[i] == b[j] {
            out.push_str(&format!("  {}\n", a[i]));
            i += 1;
            j += 1;
        } else if j < m && (i == n || lcs[i][j + 1] >= lcs[i + 1][j]) {
            out.push_str(&format!("+ {}\n", b[j]));
            j += 1;
        } else {
            out.push_str(&format!("- {}\n", a[i]));
            i += 1;
        }
    }
    out
}
