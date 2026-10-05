//! The process and service adapters: starting a target's program with the
//! environment it is given, reading its output by a deadline, waiting for a
//! service's port, and stopping a service. `check`, `snap` and `drift` kill a
//! service at once, since nothing needs writing; `map` asks it to stop with SIGTERM
//! first, so that its runtime writes what it ran (DESIGN §7.3).
//!
//! Every program starts in a process group of its own, and is stopped as a group:
//! what it starts in turn (the server behind `sh -c`, `npm start` or `uv run`) gets
//! the same signal, and geas waits until the whole group is gone (DESIGN §10).
//! Since a terminal's Ctrl-C reaches only geas's own group then, geas catches
//! SIGINT, SIGTERM and SIGHUP and kills the groups it started before it exits.

use crate::diag;
use ritsu_base::text::Text;
use crate::model::Pins;
use crate::run::Obs;
use crate::words;
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::io::Read;
use std::io::Write as _;
use std::net::{SocketAddr, TcpListener, TcpStream};
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

pub const STEP_TIMEOUT: Duration = Duration::from_secs(5);
/// How long a service has, after SIGTERM, to exit by itself in `map`.
pub const TERM_GRACE: Duration = Duration::from_secs(5);
/// How long geas waits for a group it killed to be gone.
const KILL_GRACE: Duration = Duration::from_secs(5);
/// How many of the last lines of a program's stderr a claim error quotes.
const STDERR_TAIL: usize = 5;

#[cfg(unix)]
unsafe extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
    fn signal(sig: i32, handler: extern "C" fn(i32)) -> usize;
    fn _exit(code: i32) -> !;
}

// Where there are no processes to signal (wasm32, in the page in the browser), no program starts
// (`Command::spawn` says so, E030), so no group is ever killed and no handler is set.
#[cfg(not(unix))]
unsafe fn kill(_pid: i32, _sig: i32) -> i32 {
    -1
}
#[cfg(not(unix))]
unsafe fn signal(_sig: i32, _handler: extern "C" fn(i32)) -> usize {
    0
}
#[cfg(not(unix))]
unsafe fn _exit(code: i32) -> ! {
    std::process::exit(code)
}
const SIGHUP: i32 = 1;
const SIGINT: i32 = 2;
const SIGKILL: i32 = 9;
const SIGTERM: i32 = 15;
/// What `kill` sets errno to when no process is left to signal.
const ESRCH: i32 = 3;

/// The process groups of the programs geas is running, for the signal handler to
/// kill: a slot holds a group's id, or 0. Atomics, since a handler may read them.
static LIVE: [AtomicI32; 256] = [const { AtomicI32::new(0) }; 256];

/// The signal that is ending geas, once one has come and a watcher has
/// directories to remove first (Chrome's profiles); 0 before.
static SIGNALLED: AtomicI32 = AtomicI32::new(0);
/// Whether a watcher is running, to remove those directories and end geas.
static WATCHING: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(sig: i32) {
    for slot in &LIVE {
        let g = slot.load(Ordering::SeqCst);
        if g > 0 {
            // SAFETY: `kill` is async-signal-safe, and `g` is a group geas started.
            unsafe {
                kill(-g, SIGKILL);
            }
        }
    }
    if WATCHING.load(Ordering::SeqCst) {
        // the watcher removes what has to go, which a handler may not, then ends geas
        SIGNALLED.store(sig, Ordering::SeqCst);
        return;
    }
    // SAFETY: `_exit` is async-signal-safe; geas ends here, as the signal asked.
    unsafe { _exit(128 + sig) }
}

/// The directories to remove if a signal ends geas: Chrome's profiles.
static DOOMED: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// The signal that came while directories were registered to be removed, if one
/// did: the run then ends as it would anyway, its programs being killed, and geas
/// prints nothing more and exits with 128 and the signal's number.
pub fn signalled() -> Option<i32> {
    match SIGNALLED.load(Ordering::SeqCst) {
        0 => None,
        s => Some(s),
    }
}

/// From here on, a signal that ends geas removes `dir` first (DESIGN §8.4). A
/// signal handler may not walk a directory, so the handler kills the groups and
/// returns; the run, its programs gone, ends at once and removes what it made on
/// the way out (`signalled` tells it to say nothing more). A thread watches as
/// well, and should the run not have ended 2 s after the signal, it removes what
/// is registered and ends geas itself.
pub fn remove_on_signal(dir: &Path) {
    DOOMED.lock().unwrap_or_else(|e| e.into_inner()).push(dir.to_path_buf());
    static WATCHER: OnceLock<()> = OnceLock::new();
    WATCHER.get_or_init(|| {
        WATCHING.store(true, Ordering::SeqCst);
        thread::spawn(|| {
            loop {
                let sig = SIGNALLED.load(Ordering::SeqCst);
                if sig != 0 {
                    thread::sleep(Duration::from_secs(2));
                    // whatever started since the handler ran goes too
                    for slot in &LIVE {
                        let g = slot.load(Ordering::SeqCst);
                        if g > 0 {
                            // SAFETY: `kill` on a group geas started.
                            unsafe {
                                kill(-g, SIGKILL);
                            }
                        }
                    }
                    let dirs = DOOMED.lock().unwrap_or_else(|e| e.into_inner()).clone();
                    // the killed programs may still be writing for a moment
                    for _ in 0..50 {
                        for d in &dirs {
                            let _ = std::fs::remove_dir_all(d);
                        }
                        if dirs.iter().all(|d| !d.exists()) {
                            break;
                        }
                        thread::sleep(Duration::from_millis(20));
                    }
                    // SAFETY: ends geas as the signal asked, as the handler would have.
                    unsafe { _exit(128 + sig) }
                }
                thread::sleep(Duration::from_millis(10));
            }
        });
    });
}

/// `dir` is gone by other means: a signal no longer needs to remove it.
pub fn forget_on_signal(dir: &Path) {
    DOOMED.lock().unwrap_or_else(|e| e.into_inner()).retain(|d| d != dir);
}

/// From here on, SIGINT, SIGTERM and SIGHUP to geas kill the process groups of the
/// programs it is running, then end geas: those groups are not geas's own, so a
/// terminal's Ctrl-C would not reach them.
pub fn stop_groups_on_signals() {
    for sig in [SIGHUP, SIGINT, SIGTERM] {
        // SAFETY: the handler only reads atomics and calls `kill` and `_exit`.
        unsafe {
            signal(sig, on_signal);
        }
    }
}

/// A program's process group: its id is the program's pid, and the program is the
/// group's first member. Known to the signal handler while it lives.
struct Group {
    pgid: i32,
    slot: Option<usize>,
}

impl Group {
    fn of(child: &Child) -> Group {
        let pgid = child.id() as i32;
        let slot = LIVE.iter().position(|s| s.compare_exchange(0, pgid, Ordering::SeqCst, Ordering::SeqCst).is_ok());
        Group { pgid, slot }
    }

    fn signal(&self, sig: i32) {
        // SAFETY: `kill` on a group geas started; one already gone is ESRCH, harmless.
        unsafe {
            kill(-self.pgid, sig);
        }
    }

    /// Whether no process is left in the group.
    fn gone(&self) -> bool {
        // SAFETY: signal 0 delivers nothing; it asks whether the group has a member.
        let r = unsafe { kill(-self.pgid, 0) };
        r == -1 && std::io::Error::last_os_error().raw_os_error() == Some(ESRCH)
    }
}

impl Drop for Group {
    fn drop(&mut self) {
        if let Some(i) = self.slot {
            LIVE[i].store(0, Ordering::SeqCst);
        }
    }
}

/// Waits, at most `within`, for every process of the group to be gone, reaping the
/// first member when it exits (the others are reaped by the system once their
/// parent is gone). Whether the group is gone.
fn wait_gone(child: &mut Child, group: &Group, within: Duration) -> bool {
    let deadline = Instant::now() + within;
    loop {
        let _ = child.try_wait();
        if group.gone() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(10));
    }
}

/// Stops a program and everything in its group, and reaps it. With `term`, the
/// group is asked with SIGTERM first and given 5 s to be gone; true when it had to
/// be killed after that.
fn stop(child: &mut Child, group: &Group, term: bool) -> bool {
    let mut killed = false;
    if term && !group.gone() {
        group.signal(SIGTERM);
        killed = !wait_gone(child, group, TERM_GRACE);
    }
    if !group.gone() {
        group.signal(SIGKILL);
        wait_gone(child, group, KILL_GRACE);
    }
    let _ = child.wait();
    killed
}

/// The ports geas has handed to `port auto` instances and not taken back.
static HANDED: Mutex<BTreeSet<u16>> = Mutex::new(BTreeSet::new());

/// Held while geas starts a program, and while `AutoPort::take` has the socket it reads a free
/// port from open (DESIGN §10). A program being started holds a copy of every descriptor geas
/// has open until it execs. A program started on one worker while another worker had that
/// socket open kept the port listening after geas closed it: the claim given the port found its
/// service ready on a connection to the copy, which nothing accepts, and its request was cut off
/// when the copy went (E033). With the two kept apart, no program starts with such a copy.
static STARTING: Mutex<()> = Mutex::new(());

pub fn starting() -> std::sync::MutexGuard<'static, ()> {
    STARTING.lock().unwrap_or_else(|e| e.into_inner())
}

/// `cmd.output()`, the program started under `starting`. The command sets its stdin, stdout and
/// stderr itself: `Command::output` pipes stdout and stderr when it does not, `spawn` does not.
pub fn output(cmd: &mut Command) -> std::io::Result<std::process::Output> {
    let child = {
        let _starting = starting();
        cmd.spawn()?
    };
    child.wait_with_output()
}

/// A free port on 127.0.0.1 for one instance of a `port auto` service, held until
/// dropped (DESIGN §10): geas binds port 0, reads the number, closes it, and skips
/// numbers it has handed to an instance still running.
pub struct AutoPort(pub u16);

impl AutoPort {
    pub fn take() -> std::io::Result<AutoPort> {
        for _ in 0..100 {
            let port = {
                let _starting = starting();
                TcpListener::bind("127.0.0.1:0")?.local_addr()?.port()
            };
            if HANDED.lock().unwrap_or_else(|e| e.into_inner()).insert(port) {
                return Ok(AutoPort(port));
            }
        }
        Err(std::io::Error::other("every free port the system offered is held by another instance"))
    }
}

impl Drop for AutoPort {
    fn drop(&mut self) {
        HANDED.lock().unwrap_or_else(|e| e.into_inner()).remove(&self.0);
    }
}

/// Why a `when` got no observation.
pub struct Failure {
    pub code: &'static str,
    pub msg: Text,
    pub notes: Vec<Text>,
}

/// How a target's processes are started: in the spec's directory, with the
/// environment its pins give it (DESIGN §12), and in `map` the coverage switches.
pub struct Launch<'a> {
    pub dir: &'a Path,
    pub env: &'a Env,
    /// In `map`: a program's group is asked to stop with SIGTERM before it is killed.
    pub term: bool,
}

/// The environment a process starts with: geas's own, or nothing with `env
/// clean`, then these variables, the later of two with one name winning.
#[derive(Debug, Clone)]
pub struct Env {
    pub clear: bool,
    pub vars: Vec<(OsString, OsString)>,
}

impl Env {
    /// What a target's pins give its processes, `{port}` in a value replaced by the
    /// instance's port. `env clean` keeps `PATH`, so the program can be found, and
    /// what `env pass` names, as geas has them.
    pub fn of(pins: &Pins, port: Option<u16>) -> Env {
        let mut vars: Vec<(OsString, OsString)> = Vec::new();
        if pins.clean {
            for name in std::iter::once("PATH").chain(pins.pass.iter().map(String::as_str)) {
                if let Some(v) = std::env::var_os(name) {
                    vars.push((name.into(), v));
                }
            }
        }
        for (k, v) in pins.vars() {
            let v = match port {
                Some(p) => v.replace(words::PORT, &p.to_string()),
                None => v,
            };
            vars.push((k.into(), v.into()));
        }
        Env { clear: pins.clean, vars }
    }

    /// The value a process started with this environment sees for a variable.
    pub fn get(&self, name: &str) -> Option<OsString> {
        match self.vars.iter().rev().find(|(k, _)| k == name) {
            Some((_, v)) => Some(v.clone()),
            None if self.clear => None,
            None => std::env::var_os(name),
        }
    }

    pub fn set(&mut self, name: &str, value: impl Into<OsString>) {
        self.vars.push((name.into(), value.into()));
    }
}

/// Reads a pipe on a thread, so that what was read can be taken by a deadline even
/// when something still holds the pipe open.
pub struct Collector {
    buf: Arc<Mutex<Vec<u8>>>,
    done: Arc<AtomicBool>,
}

impl Collector {
    fn start(mut r: impl Read + Send + 'static) -> Collector {
        let buf = Arc::new(Mutex::new(Vec::new()));
        let done = Arc::new(AtomicBool::new(false));
        let (b, d) = (buf.clone(), done.clone());
        thread::spawn(move || {
            let mut chunk = [0u8; 8192];
            loop {
                match r.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(n) => b.lock().unwrap_or_else(|e| e.into_inner()).extend_from_slice(&chunk[..n]),
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                }
            }
            d.store(true, Ordering::SeqCst);
        });
        Collector { buf, done }
    }

    /// What has been read so far.
    fn peek(&self) -> String {
        String::from_utf8_lossy(&self.buf.lock().unwrap_or_else(|e| e.into_inner())).into_owned()
    }

    /// What was read, once the pipe closed or the deadline passed.
    fn take(self, deadline: Instant) -> String {
        while !self.done.load(Ordering::SeqCst) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        let bytes = std::mem::take(&mut *self.buf.lock().unwrap_or_else(|e| e.into_inner()));
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

/// A running service.
pub struct Server {
    child: Child,
    group: Group,
    stderr: Collector,
}

/// With `GEAS_PID_LOG` naming a file, appends a line for every process geas starts
/// and every one it has reaped. Tests read it to hold geas to stopping everything.
fn pid_log(line: String) {
    static LOCK: Mutex<()> = Mutex::new(());
    let Some(path) = std::env::var_os("GEAS_PID_LOG") else {
        return;
    };
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = f.write_all(line.as_bytes());
    }
}

/// Words as a shell would read them back: plain when they are plain, else in
/// single quotes. `{port}`, as an auto port's command shows it, is plain: a shell
/// keeps braces without a comma as they are.
pub fn shell_words(words: &[String]) -> String {
    words
        .iter()
        .map(|w| {
            let rest = w.replace(words::PORT, "");
            if !w.is_empty() && rest.chars().all(|c| c.is_ascii_alphanumeric() || "_./:=@%+,-".contains(c)) {
                w.clone()
            } else {
                format!("'{}'", w.replace('\'', "'\\''"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn command_note(words: &[String]) -> Text {
    let w = shell_words(words);
    tr!("コマンド: {w}", "command: {w}")
}

/// The last lines of a program's stderr, one note each.
pub fn stderr_tail(stderr: &str) -> Vec<Text> {
    let lines: Vec<&str> = stderr.lines().filter(|l| !l.trim().is_empty()).collect();
    let from = lines.len().saturating_sub(STDERR_TAIL);
    lines[from..].iter().map(|l| Text::same(format!("stderr: {}", diag::cut(l, 120)))).collect()
}

/// How a process ended, for a message.
fn ended(st: ExitStatus) -> Text {
    #[cfg(unix)]
    let signal = {
        use std::os::unix::process::ExitStatusExt;
        st.signal()
    };
    #[cfg(not(unix))]
    let signal: Option<i32> = None;
    match (st.code(), signal) {
        (Some(c), _) => tr!("終了コード {c}", "exit {c}"),
        (None, Some(s)) => tr!("シグナル {s} で終了", "killed by signal {s}"),
        (None, None) => tr!("終了", "ended"),
    }
}

/// The program a command's first word starts, as the OS would find it: a word with
/// a `/` is a path from the spec's directory, any other is looked for on PATH.
pub fn resolve(word: &str, dir: &Path) -> Option<PathBuf> {
    let p = if word.contains('/') { dir.join(word) } else { crate::cover::on_path(word)? };
    std::fs::canonicalize(p).ok()
}

/// Starts a program in a process group of its own. `shown` is the command as a
/// message gives it.
fn spawn(target: &str, words: &[String], shown: &[String], launch: &Launch, stdout: Stdio) -> Result<(Child, Group), Failure> {
    spawn_with(target, words, shown, launch, Stdio::null(), stdout)
}

fn spawn_with(target: &str, words: &[String], shown: &[String], launch: &Launch, stdin: Stdio, stdout: Stdio) -> Result<(Child, Group), Failure> {
    let prog = &words[0];
    // after a signal the run only winds down: nothing new starts
    if signalled().is_some() {
        return Err(Failure {
            code: "E030",
            msg: tr!("geas は止まるところなので、`{prog}` を起動しません", "geas is stopping, and does not start `{prog}`"),
            notes: vec![],
        });
    }
    let mut cmd = Command::new(prog);
    cmd.args(&words[1..])
        .current_dir(launch.dir)
        .stdin(stdin)
        .stdout(stdout)
        .stderr(Stdio::piped());
    #[cfg(unix)]
    cmd.process_group(0);
    if launch.env.clear {
        cmd.env_clear();
    }
    for (k, v) in &launch.env.vars {
        cmd.env(k, v);
    }
    let spawned = {
        let _starting = starting();
        cmd.spawn()
    };
    let child = spawned.map_err(|e| Failure {
        code: "E030",
        msg: tr!("`{prog}` を起動できません: {e}", "cannot start `{prog}`: {e}"),
        notes: vec![command_note(shown)],
    })?;
    pid_log(format!("start {} {}\n", child.id(), target));
    let group = Group::of(&child);
    Ok((child, group))
}

/// Runs a `run` target's command, its words with the `when`'s arguments appended,
/// for at most 5 s. `on_start` hears the pid and the words of the process once it
/// runs. The parser has made sure the command has a first word (E010). A program
/// the command leaves running in its group is stopped when the command ends.
pub fn run_process(
    target: &str,
    base: &[String],
    args: &[String],
    launch: &Launch,
    on_start: &mut dyn FnMut(u32, &[String]),
) -> Result<Obs, Failure> {
    let mut words: Vec<String> = base.to_vec();
    words.extend(args.iter().cloned());
    let prog = words[0].clone();
    let (mut child, group) = spawn(target, &words, &words, launch, Stdio::piped())?;
    on_start(child.id(), &words);
    let so = Collector::start(child.stdout.take().expect("piped"));
    let se = Collector::start(child.stderr.take().expect("piped"));
    let deadline = Instant::now() + STEP_TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(10)),
            other => {
                stop(&mut child, &group, false);
                pid_log(format!("stop {}\n", child.id()));
                let stderr = se.take(Instant::now() + Duration::from_secs(1));
                let mut notes = vec![command_note(&words)];
                notes.extend(stderr_tail(&stderr));
                let msg = match other {
                    Err(e) => tr!("`{prog}` の終了を待てませんでした: {e}", "waiting for `{prog}` failed: {e}"),
                    _ => tr!(
                        "`{prog}` が 5 秒のうちに終わらなかったので、geas が止めました",
                        "`{prog}` did not finish within 5 s, so geas stopped it",
                    ),
                };
                return Err(Failure { code: "E031", msg, notes });
            }
        }
    };
    stop(&mut child, &group, launch.term);
    pid_log(format!("stop {}\n", child.id()));
    let stdout = so.take(deadline);
    let stderr = se.take(deadline);
    Ok(Obs::Proc { stdout, stderr, exit: status.code().unwrap_or(-1) })
}


/// Starts a `serve` target, `{port}` in its words replaced by the port, and waits,
/// at most 5 s, for the port to open. With `port auto`, messages and `on_start`
/// get the words as written, `{port}` in them, so that what geas prints does not
/// change with the port.
pub fn start_server(
    name: &str,
    written: &[String],
    port: u16,
    auto: bool,
    launch: &Launch,
    on_start: &mut dyn FnMut(u32, &[String]),
) -> Result<Server, Failure> {
    let words = words::with_port(written, port);
    let shown: &[String] = if auto { written } else { &words };
    // a fixed port is named by its number; an auto one by where it came from,
    // since its number changes from run to run
    let pe = if auto { "the port geas gave it".to_string() } else { format!("port {port}") };
    let (mut child, group) = spawn(name, &words, shown, launch, Stdio::null())?;
    on_start(child.id(), shown);
    let se = Collector::start(child.stderr.take().expect("piped"));
    let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().expect("addr");
    let deadline = Instant::now() + STEP_TIMEOUT;
    loop {
        if TcpStream::connect_timeout(&addr, Duration::from_millis(100)).is_ok() {
            break;
        }
        if let Ok(Some(st)) = child.try_wait() {
            stop(&mut child, &group, false);
            pid_log(format!("stop {}\n", child.id()));
            let stderr = se.take(Instant::now() + Duration::from_secs(1));
            let how = ended(st);
            let mut notes = vec![command_note(shown)];
            notes.extend(stderr_tail(&stderr));
            return Err(Failure {
                code: "E032",
                msg: Text::new(
                    if auto {
                        format!("サービス `{name}` が、geas が渡したポートを開く前に終了しました（{}）", how.ja)
                    } else {
                        format!("サービス `{name}` がポート {port} を開く前に終了しました（{}）", how.ja)
                    },
                    format!("the service `{name}` exited before opening {pe} ({})", how.en),
                ),
                notes,
            });
        }
        if Instant::now() >= deadline {
            stop(&mut child, &group, false);
            pid_log(format!("stop {}\n", child.id()));
            let stderr = se.take(Instant::now() + Duration::from_secs(1));
            let mut notes = vec![command_note(shown)];
            notes.extend(stderr_tail(&stderr));
            return Err(Failure {
                code: "E032",
                msg: Text::new(
                    if auto {
                        format!("サービス `{name}` が、geas が渡したポートを 5 秒のうちに開かなかったので、geas が止めました")
                    } else {
                        format!("サービス `{name}` が 5 秒のうちにポート {port} を開かなかったので、geas が止めました")
                    },
                    format!("the service `{name}` did not open {pe} within 5 s, so geas stopped it"),
                ),
                notes,
            });
        }
        thread::sleep(Duration::from_millis(25));
    }
    Ok(Server { child, group, stderr: se })
}

/// Stops a service, its whole process group, and returns its stderr, and whether
/// it had to be killed after SIGTERM. With `term`, the group is asked first and
/// given 5 s to be gone; without, it is killed at once.
pub fn stop_server(mut s: Server, term: bool) -> (String, bool) {
    let killed = stop(&mut s.child, &s.group, term);
    pid_log(format!("stop {}\n", s.child.id()));
    (s.stderr.take(Instant::now() + Duration::from_secs(1)), killed)
}

/// A program geas talks to or waits for: a GUI driver, a pixie app, Chrome. It runs
/// in a process group of its own like every program geas starts, its stderr is
/// collected, and its stdin and stdout are the caller's when asked for.
pub struct Proc {
    child: Child,
    group: Group,
    pub stdin: Option<ChildStdin>,
    pub stdout: Option<ChildStdout>,
    stderr: Option<Collector>,
    /// Stopped and reaped: dropping it has nothing left to do.
    done: bool,
}

/// Starts a program for a target (`label` names it in `GEAS_PID_LOG`), with its
/// stdin and stdout piped when asked for, else null.
pub fn start(label: &str, words: &[String], launch: &Launch, stdin: bool, stdout: bool) -> Result<Proc, Failure> {
    let pipe = |p: bool| if p { Stdio::piped() } else { Stdio::null() };
    let (mut child, group) = spawn_with(label, words, words, launch, pipe(stdin), pipe(stdout))?;
    let stderr = child.stderr.take().map(Collector::start);
    Ok(Proc { stdin: child.stdin.take(), stdout: child.stdout.take(), child, group, stderr, done: false })
}

impl Proc {
    /// How it ended, if it has by `deadline`.
    pub fn wait_until(&mut self, deadline: Instant) -> Option<ExitStatus> {
        loop {
            match self.child.try_wait() {
                Ok(Some(st)) => return Some(st),
                Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(5)),
                _ => return None,
            }
        }
    }

    /// What it has written on stderr so far.
    pub fn stderr_so_far(&self) -> String {
        self.stderr.as_ref().map(Collector::peek).unwrap_or_default()
    }

    /// Stops it and its whole group, reaps it, and returns its stderr.
    pub fn finish(mut self) -> String {
        self.stop_now();
        match self.stderr.take() {
            Some(c) => c.take(Instant::now() + Duration::from_secs(1)),
            None => String::new(),
        }
    }

    fn stop_now(&mut self) {
        if self.done {
            return;
        }
        self.stdin = None;
        stop(&mut self.child, &self.group, false);
        pid_log(format!("stop {}\n", self.child.id()));
        self.done = true;
    }
}

/// A program dropped without `finish`, on an error or a panic, is stopped all the
/// same: nothing geas starts outlives it.
impl Drop for Proc {
    fn drop(&mut self) {
        self.stop_now();
    }
}

/// How a program ended, for a message: `exit 1`, `killed by signal 9`.
pub fn how_it_ended(st: ExitStatus) -> Text {
    ended(st)
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_read_back_as_a_shell_would() {
        let w = |v: &[&str]| shell_words(&v.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        assert_eq!(w(&["python3", "calc.py", "2", "+", "3"]), "python3 calc.py 2 + 3");
        assert_eq!(w(&["a b", "", "it's"]), "'a b' '' 'it'\\''s'");
        assert_eq!(w(&["server", "--port={port}", "{a,b}"]), "server --port={port} '{a,b}'");
    }

    #[test]
    fn the_tail_of_stderr() {
        let err = "1\n\n2\n3\n4\n5\n6\n";
        let notes: Vec<String> = stderr_tail(err).into_iter().map(|n| n.en).collect();
        assert_eq!(notes, ["stderr: 2", "stderr: 3", "stderr: 4", "stderr: 5", "stderr: 6"]);
    }

    #[test]
    fn auto_ports_are_never_handed_out_twice_while_held() {
        let held: Vec<AutoPort> = (0..20).map(|_| AutoPort::take().expect("a free port")).collect();
        let mut numbers: Vec<u16> = held.iter().map(|p| p.0).collect();
        numbers.sort();
        numbers.dedup();
        assert_eq!(numbers.len(), 20);
        let first = held[0].0;
        drop(held);
        assert!(!HANDED.lock().unwrap_or_else(|e| e.into_inner()).contains(&first));
    }
}
