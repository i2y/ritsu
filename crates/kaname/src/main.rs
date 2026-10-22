//! The `kaname` binary: the command (`run.rs`) with no other language joined. A module that reads a
//! rule, a dates file or a book is told to run through `ritsu kaname` (E202, ritsu's DESIGN 2.3).

use std::io::Write;
use std::process::ExitCode;

/// Die quietly when the reader of a pipe goes away, as `cat` does.
#[cfg(unix)]
fn restore_sigpipe() {
    unsafe extern "C" {
        fn signal(signum: i32, handler: usize) -> usize;
    }
    // SIGPIPE is 13 on Linux and macOS; SIG_DFL is 0.
    unsafe { signal(13, 0) };
}

#[cfg(not(unix))]
fn restore_sigpipe() {}

/// The command runs on a thread of its own, with room for 256 MiB of stack. Reading and running a
/// value a host may give, nested 200 deep (DESIGN 28.1), recurses once a level, and a debug build
/// on x86_64 Linux overflows the stack a main thread is given (8 MiB, or less under `ulimit -s`).
/// The room is reserved, not used.
const STACK: usize = 256 << 20;

fn main() -> ExitCode {
    restore_sigpipe();
    // A panic is a bug in kaname: exit 2, as the other exits that are not the file's fault, rather
    // than Rust's 101.
    std::panic::set_hook(Box::new(|info| {
        eprintln!("kaname: a bug in kaname, please report it: {info}");
    }));
    let args: Vec<String> = std::env::args().skip(1).collect();
    let worker = std::thread::Builder::new().stack_size(STACK).spawn(move || {
        std::panic::catch_unwind(|| {
            let (mut out, mut err) = (std::io::stdout(), std::io::stderr());
            let code = kaname::run::run(&args, kaname::suite::Suite::default(), &mut out, &mut err);
            let _ = out.flush();
            code
        })
    });
    match worker.map(|w| w.join()) {
        Ok(Ok(Ok(code))) => ExitCode::from(code),
        _ => ExitCode::from(2),
    }
}
