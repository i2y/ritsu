//! The `sekisho` binary: the command (`run.rs`) with no other language joined. A file that reads a
//! rule, a dates file, a calendar, a book or a flow is told to run through `ritsu sekisho` (E209,
//! ritsu's DESIGN 2.3).

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

fn main() -> ExitCode {
    restore_sigpipe();
    // A panic is a bug in sekisho: exit 2, as the other exits that are not the file's fault, rather
    // than Rust's 101.
    std::panic::set_hook(Box::new(|info| {
        eprintln!("sekisho: a bug in sekisho, please report it: {info}");
    }));
    let args: Vec<String> = std::env::args().skip(1).collect();
    match std::panic::catch_unwind(|| {
        let (mut out, mut err) = (std::io::stdout(), std::io::stderr());
        let code = sekisho::run::run(&args, sekisho::suite::Suite::default(), &mut out, &mut err);
        let _ = out.flush();
        code
    }) {
        Ok(code) => ExitCode::from(code),
        Err(_) => ExitCode::from(2),
    }
}
