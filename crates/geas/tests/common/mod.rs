//! What every integration test shares: the binary, a scratch directory that removes
//! itself, copies of the examples, a run of geas that cannot hang the suite, golden
//! files, and the locks and checks for the tests that start services. The scratch
//! directory, the run with a time limit, the golden files, finding a tool and Chrome,
//! the SKIP line and the JSON reader are ritsu-testkit's and ritsu-base's; what is
//! here is geas's own.

// Each test binary includes this module and uses a different part of it; the rest
// would warn as dead code in every binary that does not use it.
#![allow(dead_code)]

use std::fs;
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

/// A fresh directory under the system's temporary directory, removed when dropped, also when
/// the test panics (ritsu-testkit's).
pub use ritsu_testkit::TempDir as Scratch;
/// Says why a test did not run what it is about; the test then passes (ritsu-testkit's
/// `SKIP: geas: <reason>`).
pub use ritsu_testkit::skip;
#[allow(unused_imports)]
pub use ritsu_testkit::golden::line_diff;

/// The geas binary cargo built for these tests.
pub fn geas() -> &'static str {
    env!("CARGO_BIN_EXE_geas")
}

/// The repository's root.
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
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
    "RITSU_LANG",
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
    cmd.args(args).current_dir(cwd);
    for v in GEAS_VARS {
        cmd.env_remove(v);
    }
    for (k, v) in env {
        cmd.env(k, v);
    }
    let r = ritsu_testkit::run::run_with_input(&mut cmd, input, RUN_LIMIT);
    assert!(!r.timed_out, "geas {args:?} did not finish within {} s", RUN_LIMIT.as_secs());
    (r.stdout, r.stderr, r.code.unwrap_or(-1))
}

/// Compares `actual` with `tests/golden/<name>`. With `GEAS_BLESS` (or `RITSU_BLESS`) set,
/// writes it there instead.
pub fn golden(name: &str, actual: &str) {
    ritsu_testkit::golden(root().join("tests/golden").join(name), actual);
}

/// Whether `tool args…` runs and succeeds.
pub fn have(tool: &str, args: &[&str]) -> bool {
    ritsu_testkit::tools::runs(tool, args)
}

/// Whether python3 is on PATH; prints the SKIP line when it is not.
pub fn python3(what: &str) -> bool {
    if !ritsu_testkit::need(ritsu_testkit::Need::Python) {
        return false;
    }
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

/// Parses one JSON value with ritsu-base's reader; panics on anything else, which is a
/// failed test.
pub fn json(text: &str) -> Json {
    fn from(v: ritsu_base::json::Json) -> Json {
        use ritsu_base::json::Json as B;
        match v {
            B::Null => Json::Null,
            B::Bool(b) => Json::Bool(b),
            B::Int(n) => Json::Num(n as f64),
            B::Frac(s) => Json::Num(s.parse().unwrap_or_else(|_| panic!("not a number: {s:?}"))),
            B::Str(s) => Json::Str(s),
            B::Arr(a) => Json::Arr(a.into_iter().map(from).collect()),
            B::Obj(o) => Json::Obj(o.into_iter().map(|(k, v)| (k, from(v))).collect()),
        }
    }
    from(ritsu_base::json::parse(text).unwrap_or_else(|e| panic!("not JSON ({}): {text:?}", e.message.en)))
}

/// A fresh `GEAS_PID_LOG` in the scratch directory, as an absolute path.
pub fn pid_log(s: &Scratch) -> PathBuf {
    s.path().join("pids.log")
}

/// The Chrome the tests use, found by ritsu-testkit (`RITSU_CHROME` or `GEAS_CHROME`, the
/// macOS application, then `google-chrome`, `chromium` or `chromium-browser` on PATH). The
/// tests hand it to geas as `GEAS_CHROME`, so geas starts the same one. None after printing
/// the SKIP line.
pub fn chrome(what: &str) -> Option<PathBuf> {
    if !ritsu_testkit::need(ritsu_testkit::Need::Chrome) {
        return None;
    }
    let found = chrome_path();
    if found.is_none() {
        skip(&format!("Chrome is not at hand (GEAS_CHROME, the macOS application, or google-chrome, chromium or chromium-browser on PATH); {what} is not run"));
    }
    found
}

/// The Chrome the tests use, without a word when there is none.
pub fn chrome_path() -> Option<PathBuf> {
    ritsu_testkit::chrome::find()
}

/// After a run with Chrome: no process whose command line holds the scratch path
/// (Chrome's profile is under it), and no profile directory left under `.geas/`.
pub fn no_chrome_left(s: &Scratch) {
    if let Err(e) = ritsu_testkit::chrome::none_left(s.path()) {
        panic!("{e}");
    }
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
