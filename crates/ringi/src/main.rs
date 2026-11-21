//! The `ringi` binary: the command (`run.rs`) with no other language joined. A request, whose form
//! and route are lowered to rules and whose ticket is lowered to a module of kaname's, is told to
//! run through `ritsu ringi` (E008, ritsu's DESIGN 2.3); an organization and masters need no other
//! language.

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

/// The command runs on a thread of its own, with room for 256 MiB of stack, as `kaname`'s does: an
/// expression nested deep is read and typed by recursion, once a level, and a debug build on
/// x86_64 Linux overflows the stack a main thread is given (8 MiB, or less under `ulimit -s`). The
/// room is reserved, not used.
const STACK: usize = 256 << 20;

fn main() -> ExitCode {
    restore_sigpipe();
    // A panic is a bug in ringi: said as every binary of the toolchain says it, and exit 2, as the
    // other exits that are not the file's fault, rather than Rust's 101 (ritsu's DESIGN 6.2, item 10)
    ritsu_base::cli::on_panic("ringi");
    let args: Vec<String> = std::env::args().skip(1).collect();
    let worker = std::thread::Builder::new().stack_size(STACK).spawn(move || {
        std::panic::catch_unwind(|| {
            let (mut out, mut err) = (std::io::stdout(), std::io::stderr());
            let code = ringi::run::run(&args, ringi::suite::Suite::default(), &mut out, &mut err);
            let _ = out.flush();
            code
        })
    });
    match worker.map(|w| w.join()) {
        Ok(Ok(Ok(code))) => ExitCode::from(code),
        _ => ExitCode::from(2),
    }
}
