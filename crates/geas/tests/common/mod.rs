//! What every integration test shares: the binary, a scratch directory that removes
//! itself, copies of the examples, a run of geas that cannot hang the suite, golden
//! files, and the locks and checks for the tests that start services.

// Each test binary includes this module and uses a different part of it; the rest
// would warn as dead code in every binary that does not use it.
#![allow(dead_code)]

use std::fs;
use std::io::Read;
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant};

/// The geas binary cargo built for these tests.
pub fn geas() -> &'static str {
    env!("CARGO_BIN_EXE_geas")
}

/// The repository's root.
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// A fresh directory under `CARGO_TARGET_TMPDIR`, removed when dropped, also when
/// the test panics.
pub struct Scratch {
    dir: PathBuf,
}

static SCRATCH_SEQ: AtomicUsize = AtomicUsize::new(0);

impl Scratch {
    pub fn new(name: &str) -> Scratch {
        let n = SCRATCH_SEQ.fetch_add(1, Ordering::SeqCst);
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("{name}-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create the scratch directory");
        Scratch { dir }
    }

    pub fn path(&self) -> &Path {
        &self.dir
    }

    /// Writes a file under the scratch directory, making its directories.
    pub fn write(&self, rel: &str, text: &str) {
        let p = self.dir.join(rel);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).expect("create a directory in the scratch");
        }
        fs::write(&p, text).expect("write a file in the scratch");
    }

    /// Reads a file under the scratch directory.
    pub fn read(&self, rel: &str) -> String {
        let p = self.dir.join(rel);
        fs::read_to_string(&p).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()))
    }

    pub fn exists(&self, rel: &str) -> bool {
        self.dir.join(rel).exists()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// Copies `examples/<name>` to `<scratch>/examples/<name>`, leaving out what runs
/// leave behind (`.geas/`, `__pycache__/`), so geas run in the scratch prints
/// `examples/<name>/…` as the README does and starts with no journal or baseline.
pub fn copy_example(name: &str, scratch: &Scratch) {
    let from = root().join("examples").join(name);
    let to = scratch.path().join("examples").join(name);
    copy_tree(&from, &to);
}

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("create a directory in the scratch");
    let mut entries: Vec<_> = fs::read_dir(from)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", from.display()))
        .map(|e| e.expect("a directory entry"))
        .collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let name = e.file_name();
        if name == ".geas" || name == "__pycache__" {
            continue;
        }
        let src = e.path();
        let dst = to.join(&name);
        if src.is_dir() {
            copy_tree(&src, &dst);
        } else {
            fs::copy(&src, &dst).expect("copy a file into the scratch");
        }
    }
}

/// The variables geas reads; a run starts without them unless the test sets them,
/// so a developer's own settings cannot change what the tests see.
const GEAS_VARS: &[&str] = &[
    "GEAS_LANG",
    "GEAS_JOBS",
    "GEAS_PID_LOG",
    "GEAS_CHROME",
    "GEAS_LLVM_BIN",
];

/// How long one run of geas may take before the test kills it and fails.
const RUN_LIMIT: Duration = Duration::from_secs(120);

/// Runs geas in `cwd` and returns its stdout, its stderr and its exit status (-1
/// when a signal ended it). A run that takes longer than 120 s is killed and fails
/// the test, so a hang fails one test instead of stalling the suite.
pub fn run(cwd: &Path, args: &[&str], env: &[(&str, &str)]) -> (String, String, i32) {
    run_with_input(cwd, args, env, None)
}

/// `run`, with `input` on geas's stdin (a diff for `geas affected … -`).
pub fn run_with_input(cwd: &Path, args: &[&str], env: &[(&str, &str)], input: Option<&[u8]>) -> (String, String, i32) {
    let mut cmd = Command::new(geas());
    cmd.args(args)
        .current_dir(cwd)
        .stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for v in GEAS_VARS {
        cmd.env_remove(v);
    }
    for (k, v) in env {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().expect("start geas");
    if let Some(bytes) = input {
        use std::io::Write as _;
        let mut stdin = child.stdin.take().expect("piped stdin");
        stdin.write_all(bytes).expect("write geas's stdin");
    }
    let out = drain(child.stdout.take().expect("piped stdout"));
    let err = drain(child.stderr.take().expect("piped stderr"));
    let deadline = Instant::now() + RUN_LIMIT;
    let status = loop {
        if let Some(st) = child.try_wait().expect("wait for geas") {
            break st;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("geas {args:?} did not finish within {} s", RUN_LIMIT.as_secs());
        }
        thread::sleep(Duration::from_millis(10));
    };
    let out = String::from_utf8(out.join().expect("stdout reader")).expect("stdout is UTF-8");
    let err = String::from_utf8(err.join().expect("stderr reader")).expect("stderr is UTF-8");
    (out, err, status.code().unwrap_or(-1))
}

fn drain(mut r: impl Read + Send + 'static) -> thread::JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut v = Vec::new();
        let _ = r.read_to_end(&mut v);
        v
    })
}

/// Compares `actual` with `tests/golden/<name>`. With `GEAS_BLESS` set, writes it
/// there instead.
pub fn golden(name: &str, actual: &str) {
    let path = root().join("tests/golden").join(name);
    if std::env::var_os("GEAS_BLESS").is_some() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create the golden directory");
        }
        fs::write(&path, actual).expect("write the golden file");
        return;
    }
    let want = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => panic!(
            "tests/golden/{name} is missing; record it with GEAS_BLESS=1, after reading what it would hold:\n{actual}"
        ),
    };
    if want != actual {
        panic!(
            "the output differs from tests/golden/{name} (- golden, + now):\n{}",
            line_diff(&want, actual)
        );
    }
}

/// A line diff, enough to read why a golden failed.
pub fn line_diff(a: &str, b: &str) -> String {
    let a: Vec<&str> = a.lines().collect();
    let b: Vec<&str> = b.lines().collect();
    let (n, m) = (a.len(), b.len());
    let mut lcs = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
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

/// Whether `tool args…` runs and succeeds.
pub fn have(tool: &str, args: &[&str]) -> bool {
    Command::new(tool)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Says on stderr why a test did not run what it is about; the test then passes.
pub fn skip(reason: &str) {
    eprintln!("SKIP: {reason}");
}

/// Whether python3 is on PATH; prints the SKIP line when it is not.
pub fn python3(what: &str) -> bool {
    if have("python3", &["--version"]) {
        true
    } else {
        skip(&format!("python3 is not on PATH; {what} is not run"));
        false
    }
}

/// The directory holding llvm-profdata and llvm-cov in the Rust toolchain's sysroot,
/// where rustup's llvm-tools component puts them; None without them.
pub fn llvm_bin() -> Option<PathBuf> {
    let out = |args: &[&str]| -> Option<String> {
        let o = Command::new("rustc").args(args).stderr(Stdio::null()).output().ok()?;
        o.status.success().then(|| String::from_utf8_lossy(&o.stdout).into_owned())
    };
    let sysroot = out(&["--print", "sysroot"])?;
    let host = out(&["-vV"])?.lines().find_map(|l| l.strip_prefix("host: ").map(str::to_string))?;
    let bin = PathBuf::from(sysroot.trim()).join("lib/rustlib").join(host.trim()).join("bin");
    (bin.join("llvm-profdata").is_file() && bin.join("llvm-cov").is_file()).then_some(bin)
}

static FIXED_PORTS: Mutex<()> = Mutex::new(());

/// Held by every test that starts a service on a fixed port (the greeter's 8123).
/// Test binaries run one after another, so one lock per binary keeps two such
/// services from meeting. A test that panicked while holding it does not poison it
/// for the others.
pub fn port_lock() -> MutexGuard<'static, ()> {
    FIXED_PORTS.lock().unwrap_or_else(|e| e.into_inner())
}

/// Whether nothing answers on the port: after a run, the service geas started for
/// it has been stopped.
pub fn port_closed(port: u16) -> bool {
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().expect("an address");
    TcpStream::connect_timeout(&addr, Duration::from_millis(300)).is_err()
}

/// Reads a file of the repository.
pub fn repo_file(rel: &str) -> String {
    let p = root().join(rel);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()))
}

unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}

/// Reads the log geas writes when `GEAS_PID_LOG` names a file: every process it
/// started was reaped, and none is alive. Returns how many it started, so a test can
/// tell that the log was written at all.
pub fn no_process_left(log: &Path) -> usize {
    let text = fs::read_to_string(log).unwrap_or_default();
    let mut started: Vec<(i32, String)> = Vec::new();
    let mut stopped: Vec<i32> = Vec::new();
    for line in text.lines() {
        let parts: Vec<&str> = line.split(' ').collect();
        match parts.as_slice() {
            ["start", pid, target] => started.push((pid.parse().expect("a pid"), target.to_string())),
            ["stop", pid] => stopped.push(pid.parse().expect("a pid")),
            _ => panic!("GEAS_PID_LOG has a line geas does not write: {line:?}"),
        }
    }
    for (pid, target) in &started {
        assert!(stopped.contains(pid), "`{target}` ({pid}) was started and never reaped");
        // kill(pid, 0) succeeds while the process exists, a zombie included
        let alive = unsafe { kill(*pid, 0) } == 0;
        assert!(!alive, "`{target}` ({pid}) is still alive");
    }
    started.len()
}

/// A JSON value, read by `json()`: enough for the tests to look into what geas
/// prints with `--json`.
#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    pub fn get(&self, key: &str) -> &Json {
        match self {
            Json::Obj(pairs) => pairs
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, v)| v)
                .unwrap_or_else(|| panic!("no `{key}` in {self:?}")),
            _ => panic!("`{key}` looked up in {self:?}"),
        }
    }

    pub fn str(&self) -> &str {
        match self {
            Json::Str(s) => s,
            _ => panic!("not a string: {self:?}"),
        }
    }

    pub fn num(&self) -> f64 {
        match self {
            Json::Num(n) => *n,
            _ => panic!("not a number: {self:?}"),
        }
    }

    pub fn arr(&self) -> &[Json] {
        match self {
            Json::Arr(items) => items,
            _ => panic!("not an array: {self:?}"),
        }
    }
}

/// Parses one JSON value; panics on anything else, which is a failed test.
pub fn json(text: &str) -> Json {
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let v = json_value(&chars, &mut i);
    json_ws(&chars, &mut i);
    assert_eq!(i, chars.len(), "trailing text after the JSON value in {text:?}");
    v
}

fn json_ws(c: &[char], i: &mut usize) {
    while *i < c.len() && c[*i].is_whitespace() {
        *i += 1;
    }
}

fn json_value(c: &[char], i: &mut usize) -> Json {
    json_ws(c, i);
    match c.get(*i) {
        Some('{') => {
            *i += 1;
            let mut pairs = Vec::new();
            json_ws(c, i);
            if c.get(*i) == Some(&'}') {
                *i += 1;
                return Json::Obj(pairs);
            }
            loop {
                json_ws(c, i);
                let Json::Str(k) = json_value(c, i) else { panic!("a key that is not a string") };
                json_ws(c, i);
                assert_eq!(c.get(*i), Some(&':'));
                *i += 1;
                pairs.push((k, json_value(c, i)));
                json_ws(c, i);
                match c.get(*i) {
                    Some(',') => *i += 1,
                    Some('}') => {
                        *i += 1;
                        return Json::Obj(pairs);
                    }
                    other => panic!("expected `,` or `}}`, found {other:?}"),
                }
            }
        }
        Some('[') => {
            *i += 1;
            let mut items = Vec::new();
            json_ws(c, i);
            if c.get(*i) == Some(&']') {
                *i += 1;
                return Json::Arr(items);
            }
            loop {
                items.push(json_value(c, i));
                json_ws(c, i);
                match c.get(*i) {
                    Some(',') => *i += 1,
                    Some(']') => {
                        *i += 1;
                        return Json::Arr(items);
                    }
                    other => panic!("expected `,` or `]`, found {other:?}"),
                }
            }
        }
        Some('"') => {
            *i += 1;
            let mut s = String::new();
            loop {
                let ch = c[*i];
                *i += 1;
                match ch {
                    '"' => return Json::Str(s),
                    '\\' => {
                        let e = c[*i];
                        *i += 1;
                        match e {
                            'n' => s.push('\n'),
                            't' => s.push('\t'),
                            'r' => s.push('\r'),
                            'b' => s.push('\u{8}'),
                            'f' => s.push('\u{c}'),
                            'u' => {
                                let hex = |at: usize| -> u32 {
                                    u32::from_str_radix(&c[at..at + 4].iter().collect::<String>(), 16).expect("four hex digits")
                                };
                                let mut cp = hex(*i);
                                *i += 4;
                                if (0xD800..0xDC00).contains(&cp) {
                                    *i += 2; // the second half's backslash and letter
                                    cp = 0x10000 + ((cp - 0xD800) << 10) + (hex(*i) - 0xDC00);
                                    *i += 4;
                                }
                                s.push(char::from_u32(cp).expect("a character"));
                            }
                            other => s.push(other),
                        }
                    }
                    other => s.push(other),
                }
            }
        }
        Some('t') => {
            *i += 4;
            Json::Bool(true)
        }
        Some('f') => {
            *i += 5;
            Json::Bool(false)
        }
        Some('n') => {
            *i += 4;
            Json::Null
        }
        Some(_) => {
            let start = *i;
            while *i < c.len() && matches!(c[*i], '0'..='9' | '-' | '+' | '.' | 'e' | 'E') {
                *i += 1;
            }
            let s: String = c[start..*i].iter().collect();
            Json::Num(s.parse().unwrap_or_else(|_| panic!("not a number: {s:?}")))
        }
        None => panic!("the JSON ended early"),
    }
}

/// A fresh `GEAS_PID_LOG` in the scratch directory, as an absolute path.
pub fn pid_log(s: &Scratch) -> PathBuf {
    s.path().join("pids.log")
}

/// The Chrome geas would start, as geas finds it: `GEAS_CHROME`, the macOS
/// application, then `google-chrome`, `chromium` or `chromium-browser` on PATH.
/// None after printing the SKIP line.
pub fn chrome(what: &str) -> Option<PathBuf> {
    let found = chrome_path();
    if found.is_none() {
        skip(&format!("Chrome is not at hand (GEAS_CHROME, the macOS application, or google-chrome, chromium or chromium-browser on PATH); {what} is not run"));
    }
    found
}

/// The Chrome geas would start, without a word when there is none.
pub fn chrome_path() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("GEAS_CHROME") {
        Some(PathBuf::from(p)).filter(|p| p.is_file())
    } else {
        let mac = PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome");
        if mac.is_file() {
            Some(mac)
        } else {
            ["google-chrome", "chromium", "chromium-browser"].iter().find_map(|name| {
                std::env::var_os("PATH").and_then(|path| std::env::split_paths(&path).map(|d| d.join(name)).find(|p| p.is_file()))
            })
        }
    }
}

/// After a run with Chrome: no process whose command line holds the scratch path
/// (Chrome's profile is under it), and no profile directory left under `.geas/`.
pub fn no_chrome_left(s: &Scratch) {
    let out = Command::new("pgrep").args(["-f", &s.path().to_string_lossy()]).output().expect("pgrep");
    let pids = String::from_utf8_lossy(&out.stdout).trim().to_string();
    assert!(pids.is_empty(), "processes still name the scratch directory: {pids}");
    let mut stack = vec![s.path().to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in fs::read_dir(&dir).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                let name = e.file_name().to_string_lossy().into_owned();
                assert!(!name.starts_with("chrome-"), "a Chrome profile is left at {}", p.display());
                stack.push(p);
            }
        }
    }
}
