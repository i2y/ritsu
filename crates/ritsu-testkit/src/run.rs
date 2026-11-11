//! Running a program to its end with a time limit: macOS has no `timeout`, so the limit is
//! kept here and a program that outlives it is killed (koyomi's, sakai's and geas's).

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

/// Wait for a child, killing it after `limit`: whether it succeeded, its code, whether the
/// limit ended it.
fn wait_limited(child: Arc<Mutex<Child>>, limit: Duration) -> (bool, Option<i32>, bool) {
    let start = Instant::now();
    loop {
        if let Some(s) = child.lock().unwrap().try_wait().unwrap() {
            return (s.success(), s.code(), false);
        }
        if start.elapsed() > limit {
            let mut c = child.lock().unwrap();
            let _ = c.kill();
            let _ = c.wait();
            return (false, None, true);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn drain(mut r: impl Read + Send + 'static) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut v = Vec::new();
        let _ = r.read_to_end(&mut v);
        v
    })
}

/// Run a command to its end with nothing on its standard input, killing it after `limit`.
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
    Ran {
        ok,
        code,
        stdout: String::from_utf8_lossy(&o.join().unwrap()).to_string(),
        stderr: String::from_utf8_lossy(&e.join().unwrap()).to_string(),
        timed_out,
    }
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
