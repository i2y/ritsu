//! A headless Chrome for a test (dandori's, koyomi's, chobo's and geas's): found in the order
//! the five crates looked, run with a profile and a temporary directory of its own in a
//! [`TempDir`], and stopped as soon as its page or picture is out — headless Chrome can take a
//! long while to end by itself.

use crate::tmp::TempDir;
use crate::tools;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Where macOS installs Google Chrome.
pub const MAC: &str = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

/// Chrome: `RITSU_CHROME` or the crate's own `<CRATE>_CHROME` (named and there: one that is
/// named but not there is not found either), else macOS's Google Chrome, else a Chrome or a
/// Chromium on the PATH.
pub fn find() -> Option<PathBuf> {
    if let Some(c) = tools::from_vars("CHROME") {
        let p = PathBuf::from(c);
        return p.is_file().then_some(p);
    }
    if Path::new(MAC).is_file() {
        return Some(PathBuf::from(MAC));
    }
    ["google-chrome", "google-chrome-stable", "chromium", "chromium-browser"].iter().find_map(|n| tools::on_path(n))
}

/// The flags every run takes: headless, no first-run pages, files may read files.
const FLAGS: &[&str] = &["--headless=new", "--disable-gpu", "--no-first-run", "--no-default-browser-check", "--allow-file-access-from-files"];

/// Chrome with the flags every run takes, its profile in `work`, and its temporary directory
/// under `work` too. A Chrome that is stopped before it ends leaves the directory of its
/// singleton socket (`com.google.Chrome.*`) in its temporary directory; under `work`, it goes
/// with the profile. macOS's Chrome takes that directory from `MAC_CHROMIUM_TMPDIR` (it does not
/// read `TMPDIR`), and Chrome elsewhere from `TMPDIR`.
fn command(chrome: &Path, work: &TempDir) -> Command {
    let tmp = crate::tmp::tmpdir_in(work.path());
    let mut c = Command::new(chrome);
    c.args(FLAGS)
        .arg(format!("--user-data-dir={}", work.path().display()))
        .env("MAC_CHROMIUM_TMPDIR", &tmp)
        .env("TMPDIR", &tmp);
    c
}

/// The DOM of a page after its scripts ran (up to `budget_ms` of virtual time), as headless
/// Chrome dumps it. Chrome is stopped as soon as the page is out, or after `limit`.
pub fn dump_dom(chrome: &Path, url: &str, budget_ms: u32, limit: Duration) -> String {
    let profile = TempDir::new("chrome-profile");
    let mut child = command(chrome, &profile)
        .arg(format!("--virtual-time-budget={budget_ms}"))
        .args(["--dump-dom", url])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap_or_else(|e| panic!("cannot run {}: {e}", chrome.display()));
    let mut out = child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut all = Vec::new();
        let mut buf = [0u8; 65536];
        loop {
            match out.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    all.extend_from_slice(&buf[..n]);
                    if all.windows(7).any(|w| w == b"</html>") {
                        break;
                    }
                }
            }
        }
        let _ = tx.send(all);
    });
    let got = rx.recv_timeout(limit).unwrap_or_default();
    let _ = child.kill();
    let _ = child.wait();
    String::from_utf8_lossy(&got).into_owned()
}

/// A picture of a page, `width` by `height`, written to `png`. Chrome is stopped once the
/// file stops growing, or after `limit`; Err says why there is no picture.
pub fn screenshot(chrome: &Path, url: &str, png: &Path, width: u32, height: u32, limit: Duration) -> Result<(), String> {
    let profile = TempDir::new("chrome-profile");
    let mut child = command(chrome, &profile)
        .arg("--hide-scrollbars")
        .arg(format!("--window-size={width},{height}"))
        .arg(format!("--screenshot={}", png.display()))
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("cannot run {}: {e}", chrome.display()))?;
    let start = Instant::now();
    let mut last = 0u64;
    let result = loop {
        std::thread::sleep(Duration::from_millis(200));
        let size = std::fs::metadata(png).map(|m| m.len()).unwrap_or(0);
        if size > 0 && size == last {
            break Ok(());
        }
        last = size;
        if let Ok(Some(_)) = child.try_wait() {
            break if size > 0 { Ok(()) } else { Err("Chrome ended without a picture".to_string()) };
        }
        if start.elapsed() > limit {
            break Err(format!("Chrome took more than {} s", limit.as_secs()));
        }
    };
    let _ = child.kill();
    let _ = child.wait();
    result
}

/// The width and the height of a PNG, from its header.
pub fn png_size(b: &[u8]) -> Option<(u32, u32)> {
    if b.len() < 24 || &b[..8] != b"\x89PNG\r\n\x1a\n" || &b[12..16] != b"IHDR" {
        return None;
    }
    Some((u32::from_be_bytes(b[16..20].try_into().ok()?), u32::from_be_bytes(b[20..24].try_into().ok()?)))
}

/// After a run: no process whose command line holds `dir` (a profile is under it).
pub fn none_left(dir: &Path) -> Result<(), String> {
    let out = Command::new("pgrep").args(["-f", &dir.to_string_lossy()]).output().map_err(|e| e.to_string())?;
    let pids = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if pids.is_empty() { Ok(()) } else { Err(format!("processes still name {}: {pids}", dir.display())) }
}
