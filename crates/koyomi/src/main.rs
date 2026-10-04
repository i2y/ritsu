//! The `koyomi` binary: the command (`koyomi::run::run`), which `ritsu koyomi` runs too.

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
    // A panic is a bug in koyomi: exit 2, as DESIGN 4.1 says, rather than Rust's 101.
    std::panic::set_hook(Box::new(|info| {
        eprintln!("koyomi: a bug in koyomi, please report it: {info}");
    }));
    let args: Vec<String> = std::env::args().skip(1).collect();
    match std::panic::catch_unwind(|| koyomi::run::run(args)) {
        Ok(code) => code,
        Err(_) => ExitCode::from(2),
    }
}
