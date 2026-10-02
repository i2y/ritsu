//! What the tests that run other programs share: a directory that is removed when the test is
//! done, finding a tool, running a program with a time limit (macOS has no `timeout`), and a
//! PostgreSQL cluster made for the test and removed after it (PLAN 0.2).

#![allow(dead_code)]

use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Every file the generated code is held to (tests/targets.rs): the examples that pass check,
/// and the fixtures made to use every operation over 1900–2100. The READMEs count their
/// vectors (tests/docs.rs).
pub const TARGET_FILES: &[&str] = &[
    "examples/calendars/東京の営業日.cal",
    "examples/calendars/民法142条の休日.cal",
    "examples/calendars/england_and_wales.cal",
    "examples/支払_20日締め翌月10日払い.cal",
    "examples/民法の期間.cal",
    "examples/締め日と支払日を受け取る.cal",
    "examples/net30.cal",
    "examples/payment_20th_close_next_10th.cal",
    "tests/fixtures/calendars/休みの書き方を全部使う.cal",
    "tests/fixtures/helpers_月を足す.cal",
    "tests/fixtures/helpers_月を引く.cal",
    "tests/fixtures/helpers_月の日と締め.cal",
    "tests/fixtures/helpers_営業日.cal",
    "tests/fixtures/helpers_断る.cal",
    "tests/fixtures/helpers_日付が一つも無い.cal",
];

/// A directory under the system's temporary directory, removed on drop.
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new(what: &str) -> TempDir {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = std::env::temp_dir().join(format!("koyomi-{what}-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
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

/// Whether `cmd` runs (`--version` succeeds).
pub fn have(cmd: &str) -> bool {
    Command::new(cmd).arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false)
}

/// Whether an executable named `name` is in a directory of the PATH.
pub fn on_path(name: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(name).is_file()))
}

/// The first line of what `cmd --version` prints.
pub fn version(cmd: &str, arg: &str) -> String {
    Command::new(cmd)
        .arg(arg)
        .output()
        .map(|o| {
            let s = if o.stdout.is_empty() { o.stderr } else { o.stdout };
            String::from_utf8_lossy(&s).lines().next().unwrap_or("").trim().to_string()
        })
        .unwrap_or_default()
}

/// A tool named by an environment variable, else at `fallback` (relative to the repository),
/// else on the PATH under `name`.
pub fn tool(var: &str, fallback: &str, name: &str) -> Option<String> {
    if let Ok(p) = std::env::var(var)
        && !p.is_empty()
    {
        return Some(p);
    }
    if !fallback.is_empty() && Path::new(fallback).exists() {
        return Some(std::fs::canonicalize(fallback).unwrap().to_string_lossy().to_string());
    }
    if have(name) { Some(name.to_string()) } else { None }
}

/// What a program printed and how it ended.
pub struct Ran {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

/// Wait for a child, killing it after `limit`.
fn wait_limited(child: Arc<Mutex<Child>>, limit: Duration) -> (bool, bool) {
    let start = Instant::now();
    loop {
        if let Some(s) = child.lock().unwrap().try_wait().unwrap() {
            return (s.success(), false);
        }
        if start.elapsed() > limit {
            let mut c = child.lock().unwrap();
            let _ = c.kill();
            let _ = c.wait();
            return (false, true);
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Run a command to its end, with a time limit.
pub fn run(cmd: &mut Command, limit: Duration) -> Ran {
    let mut child = cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap_or_else(|e| panic!("cannot run {cmd:?}: {e}"));
    let mut out = child.stdout.take().unwrap();
    let mut err = child.stderr.take().unwrap();
    let o = std::thread::spawn(move || {
        let mut s = Vec::new();
        let _ = out.read_to_end(&mut s);
        s
    });
    let e = std::thread::spawn(move || {
        let mut s = Vec::new();
        let _ = err.read_to_end(&mut s);
        s
    });
    let (ok, timed_out) = wait_limited(Arc::new(Mutex::new(child)), limit);
    Ran {
        ok,
        stdout: String::from_utf8_lossy(&o.join().unwrap()).to_string(),
        stderr: String::from_utf8_lossy(&e.join().unwrap()).to_string(),
        timed_out,
    }
}

/// Run `cmd` with the lines `feed` writes on its standard input, and hand every line it prints
/// to `each` as it comes, without keeping the whole of either in memory. Returns how the
/// program ended and the end of its standard error.
pub fn pipe(cmd: &mut Command, limit: Duration, feed: impl FnOnce(&mut dyn Write) + Send, mut each: impl FnMut(&str)) -> Ran {
    let mut child = cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap_or_else(|e| panic!("cannot run {cmd:?}: {e}"));
    let stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let child = Arc::new(Mutex::new(child));
    let watch = {
        let child = Arc::clone(&child);
        std::thread::spawn(move || wait_limited(child, limit))
    };
    std::thread::scope(|s| {
        s.spawn(move || {
            let mut w = std::io::BufWriter::with_capacity(1 << 16, stdin);
            feed(&mut w);
            let _ = w.flush();
        });
        let e = s.spawn(move || {
            let mut v = Vec::new();
            let _ = stderr.read_to_end(&mut v);
            v
        });
        for line in BufReader::with_capacity(1 << 16, stdout).lines() {
            match line {
                Ok(l) => each(&l),
                Err(_) => break,
            }
        }
        let (ok, timed_out) = watch.join().unwrap();
        let err = String::from_utf8_lossy(&e.join().unwrap()).to_string();
        let tail: String = err.lines().rev().take(20).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
        Ran { ok, stdout: String::new(), stderr: tail, timed_out }
    })
}

/// A PostgreSQL cluster made for one test: `initdb` into a temporary directory, a server on
/// a Unix socket only (`listen_addresses=''`), stopped and removed on drop.
pub struct Cluster {
    pub bin: PathBuf,
    pub socket: PathBuf,
    pub port: u16,
    dir: TempDir,
}

/// Where the PostgreSQL programs are: `KOYOMI_PG_BIN`, else the PATH.
pub fn pg_bin() -> Option<PathBuf> {
    let dir = std::env::var("KOYOMI_PG_BIN").ok().filter(|s| !s.is_empty()).map(PathBuf::from);
    let initdb = dir.as_ref().map(|d| d.join("initdb")).unwrap_or_else(|| PathBuf::from("initdb"));
    if have(&initdb.to_string_lossy()) {
        return Some(dir.unwrap_or_default());
    }
    None
}

impl Cluster {
    pub fn start(bin: &Path) -> Result<Cluster, String> {
        let dir = TempDir::new("pg");
        let socket = std::env::var("KOYOMI_PG_SOCKET_DIR").ok().filter(|s| !s.is_empty()).map(PathBuf::from).unwrap_or_else(|| dir.path().join("s"));
        std::fs::create_dir_all(&socket).map_err(|e| e.to_string())?;
        // A port no one listens on: the socket's name has it in it, so two clusters in one
        // socket directory need two.
        let port = std::net::TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr()).map(|a| a.port()).map_err(|e| e.to_string())?;
        let user = std::env::var("USER").unwrap_or_else(|_| "postgres".into());
        let data = dir.path().join("data");
        let r = run(
            Command::new(bin.join("initdb")).args(["-D", &data.to_string_lossy(), "-A", "trust", "-U", &user, "-E", "UTF8", "--no-locale"]),
            Duration::from_secs(120),
        );
        if !r.ok {
            return Err(format!("initdb: {}", r.stderr));
        }
        let opts = format!("-k {} -p {port} -c listen_addresses='' -c fsync=off", socket.display());
        let r = run(
            Command::new(bin.join("pg_ctl")).args(["-D", &data.to_string_lossy(), "-o", &opts, "-l", &dir.path().join("pg.log").to_string_lossy(), "-w", "start"]),
            Duration::from_secs(120),
        );
        let c = Cluster { bin: bin.to_path_buf(), socket, port, dir };
        if !r.ok {
            return Err(format!("pg_ctl start: {}{}", r.stdout, r.stderr));
        }
        Ok(c)
    }

    /// `psql` against a database of the cluster, quiet, stopping at the first error.
    pub fn psql(&self, db: &str) -> Command {
        let mut c = Command::new(self.bin.join("psql"));
        c.args(["-X", "-q", "-v", "ON_ERROR_STOP=1", "-d", db])
            .env("PGHOST", &self.socket)
            .env("PGPORT", self.port.to_string())
            .env_remove("PGDATABASE")
            .env_remove("PGUSER");
        c
    }
}

impl Drop for Cluster {
    fn drop(&mut self) {
        let data = self.dir.path().join("data");
        let _ = run(Command::new(self.bin.join("pg_ctl")).args(["-D", &data.to_string_lossy(), "-m", "immediate", "-w", "stop"]), Duration::from_secs(60));
        let _ = std::fs::remove_file(self.socket.join(format!(".s.PGSQL.{}", self.port)));
        let _ = std::fs::remove_file(self.socket.join(format!(".s.PGSQL.{}.lock", self.port)));
    }
}
