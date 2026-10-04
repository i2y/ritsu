//! The `sakai` binary: the command (`run.rs`) with no other language joined. Where a map holds
//! what only another language reads, it says so (E104), and to run through `ritsu sakai`
//! (ritsu's DESIGN 2.3).

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
    // A panic is a bug in sakai: exit 2, as DESIGN 5.1 says, rather than Rust's 101.
    std::panic::set_hook(Box::new(|info| {
        eprintln!("sakai: a bug in sakai, please report it: {info}");
    }));
    let args: Vec<String> = std::env::args().skip(1).collect();
    match std::panic::catch_unwind(|| {
        let (mut out, mut err) = (std::io::stdout(), std::io::stderr());
        let code = sakai::run::run(&args, sakai::suite::Suite::default(), &mut out, &mut err);
        let _ = out.flush();
        code
    }) {
        Ok(code) => ExitCode::from(code),
        Err(_) => ExitCode::from(2),
    }
}
