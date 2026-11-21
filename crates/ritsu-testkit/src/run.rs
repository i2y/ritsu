//! Running a program to its end with a time limit: macOS has no `timeout`, so the limit is
//! kept here, and a program that outlives it is killed with every process it started — a server
//! it runs, a runner it hands work to — and what it printed until then is kept (koyomi's, sakai's,
//! geas's and dandori's runners of the platforms).

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// What a program printed and how it ended.
#[derive(Clone, Debug, Default)]
pub struct Ran {
    pub ok: bool,
    /// None when a signal ended it, or the limit did.
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

impl Ran {
    /// Both outputs, for an assertion's message.
    pub fn both(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }
}

/// How many times [`spawn`] starts a program that Linux finds busy, at the most.
pub const BUSY_TRIES: u32 = 10;

/// Start `cmd`, and start it again, a little later each time, while Linux finds the program busy
/// (ETXTBSY, "Text file busy"). A test that has just written the program it starts (a script, a
/// stand-in for Chrome) can meet that: while the file was open for writing, another test of the
/// same binary started a program of its own, and the child it forked holds a copy of the
/// descriptor until its exec, a moment later. Linux runs no file that is open for writing; macOS
/// does. The waits come to a little over a second; any other error, or a program still busy on
/// the last try, is the answer.
pub fn spawn(cmd: &mut Command) -> std::io::Result<Child> {
    let mut tried = 1;
    loop {
        match cmd.spawn() {
            Err(e) if e.kind() == std::io::ErrorKind::ExecutableFileBusy && tried < BUSY_TRIES => {
                std::thread::sleep(Duration::from_millis(25 * u64::from(tried)));
                tried += 1;
            }
            started => return started,
        }
    }
}

/// Wait for a child, killing it after `limit` with every process it started: whether it
/// succeeded, its code, whether the limit ended it.
fn wait_limited(child: Arc<Mutex<Child>>, limit: Duration) -> (bool, Option<i32>, bool) {
    let start = Instant::now();
    loop {
        if let Some(s) = child.lock().unwrap().try_wait().unwrap() {
            return (s.success(), s.code(), false);
        }
        if start.elapsed() > limit {
            kill_tree(&mut child.lock().unwrap());
            return (false, None, true);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// The process `root` and every process under it — its children, theirs, and so on — as `ps`
/// lists them now; `root` alone where `ps` cannot be run.
fn tree(root: u32) -> Vec<u32> {
    let mut found = vec![root];
    let Ok(out) = Command::new("ps").args(["-A", "-o", "pid=,ppid="]).stdin(Stdio::null()).stderr(Stdio::null()).output() else { return found };
    let pairs: Vec<(u32, u32)> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            Some((w.next()?.parse().ok()?, w.next()?.parse().ok()?))
        })
        .collect();
    let mut i = 0;
    while i < found.len() {
        let p = found[i];
        for (c, parent) in &pairs {
            if *parent == p && !found.contains(c) {
                found.push(*c);
            }
        }
        i += 1;
    }
    found
}

fn signal(sig: &str, pids: &[u32]) {
    if !pids.is_empty() {
        let _ = Command::new("kill").arg(sig).args(pids.iter().map(u32::to_string)).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status();
    }
}

/// Kill a child and every process it started, and wait for the child. Each process of the tree
/// is held where it is first (SIGSTOP), so that none starts another while the tree is read, and
/// the tree is read again until it has no new process; then every one is killed. A process that
/// started one of its own and ended before is not in the tree (its children went to the
/// system's), and neither is one that put itself in a session of its own and left; nothing else
/// is left running. `kill` and `ps` are the system's; without them, the child alone is killed.
fn kill_tree(child: &mut Child) {
    let mut held: Vec<u32> = Vec::new();
    for _ in 0..8 {
        let new: Vec<u32> = tree(child.id()).into_iter().filter(|p| !held.contains(p)).collect();
        if new.is_empty() {
            break;
        }
        signal("-STOP", &new);
        held.extend(new);
    }
    signal("-KILL", &held);
    let _ = child.kill();
    let _ = child.wait();
}

/// How long the output of a program that has ended is read on, both pipes together: what a process
/// it left behind still holds open is not waited for longer, and the output is what came until then.
const LEFT_OPEN: Duration = Duration::from_secs(5);

/// A pipe read to its end in a thread of its own, what came kept as it comes.
struct Drained {
    bytes: Arc<Mutex<Vec<u8>>>,
    ended: std::sync::mpsc::Receiver<()>,
}

impl Drained {
    /// What came: once the pipe has ended, or at `until` while something still holds it open.
    fn take(self, until: Instant) -> String {
        let _ = self.ended.recv_timeout(until.saturating_duration_since(Instant::now()));
        String::from_utf8_lossy(&self.bytes.lock().unwrap()).to_string()
    }
}

fn drain(mut r: impl Read + Send + 'static) -> Drained {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let (done, ended) = std::sync::mpsc::channel();
    let kept = Arc::clone(&bytes);
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            match r.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => kept.lock().unwrap().extend_from_slice(&buf[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }
        let _ = done.send(());
    });
    Drained { bytes, ended }
}

/// Run a command to its end with nothing on its standard input, killing it after `limit` with
/// every process it started.
pub fn run(cmd: &mut Command, limit: Duration) -> Ran {
    run_with_input(cmd, None, limit)
}

/// The same, with `input` written to its standard input.
pub fn run_with_input(cmd: &mut Command, input: Option<&[u8]>, limit: Duration) -> Ran {
    cmd.stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() }).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = spawn(cmd).unwrap_or_else(|e| panic!("cannot run {cmd:?}: {e}"));
    if let Some(bytes) = input {
        let mut stdin = child.stdin.take().unwrap();
        let bytes = bytes.to_vec();
        std::thread::spawn(move || {
            let _ = stdin.write_all(&bytes);
        });
    }
    let o = drain(child.stdout.take().unwrap());
    let e = drain(child.stderr.take().unwrap());
    let (ok, code, timed_out) = wait_limited(Arc::new(Mutex::new(child)), limit);
    let until = Instant::now() + LEFT_OPEN;
    Ran { ok, code, stdout: o.take(until), stderr: e.take(until), timed_out }
}

/// Run a command to its end, as `Command::output` does, for no longer than `limit`: past it, the
/// command is killed with every process it started, and the test fails there, saying `what` it was
/// and what the command printed until then (the last 300 lines of each output). For a program that
/// talks to a server or to another program, which can wait without end when the other side does
/// not answer (dandori's runners of the platforms).
pub fn finish(cmd: &mut Command, limit: Duration, what: &str) -> Ran {
    finish_with_input(cmd, None, limit, what)
}

/// The same, with `input` written to its standard input.
pub fn finish_with_input(cmd: &mut Command, input: Option<&[u8]>, limit: Duration, what: &str) -> Ran {
    let ran = run_with_input(cmd, input, limit);
    if ran.timed_out {
        let tail = |s: &str| {
            let lines: Vec<&str> = s.lines().collect();
            lines[lines.len().saturating_sub(300)..].join("\n")
        };
        panic!(
            "{what}: it did not end within {} seconds, and was stopped with every process it started. What it printed until then:\n--- standard output ---\n{}\n--- standard error ---\n{}",
            limit.as_secs_f64(),
            tail(&ran.stdout),
            tail(&ran.stderr)
        );
    }
    ran
}

/// Run `cmd` with the lines `feed` writes on its standard input, and hand every line it prints
/// to `each` as it comes, keeping neither whole in memory (koyomi's, for the generated code
/// held to every input of a range). The standard output is not kept; the last twenty lines of
/// the standard error are.
pub fn pipe(cmd: &mut Command, limit: Duration, feed: impl FnOnce(&mut dyn Write) + Send, mut each: impl FnMut(&str)) -> Ran {
    let mut child = spawn(cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())).unwrap_or_else(|e| panic!("cannot run {cmd:?}: {e}"));
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
        let (ok, code, timed_out) = watch.join().unwrap();
        let err = String::from_utf8_lossy(&e.join().unwrap()).to_string();
        let tail: Vec<&str> = err.lines().rev().take(20).collect();
        let tail: Vec<&str> = tail.into_iter().rev().collect();
        Ran { ok, code, stdout: String::new(), stderr: tail.join("\n"), timed_out }
    })
}
