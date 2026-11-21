//! The `rulec` binary: the command (`rulec::cli::run`), which `ritsu rulec` runs too.

use std::process::ExitCode;

/// Die quietly when the reader of a pipe goes away, the way `cat` does. Rust ignores SIGPIPE
/// at startup, so `rulec vectors | head` would have `println!` see EPIPE and panic instead.
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
    // A panic is a bug in rulec: exit 2 (ritsu's DESIGN 8.4) rather than Rust's 101, in the words of
    // every binary of the toolchain.
    ritsu_base::cli::on_panic("rulec");
    std::panic::catch_unwind(|| rulec::cli::run(std::env::args().skip(1).collect())).unwrap_or(ExitCode::from(2))
}
