//! The `dandori` command: crate::cli runs it. This binary holds no rulec (ritsu's DESIGN 2.3): it
//! runs a workflow that uses no rule as it always has, and where a flow reads a rule it says to run
//! it with `ritsu dandori`, which reads the rule in the same process.

use std::process::ExitCode;
use std::rc::Rc;

fn main() -> ExitCode {
    // A panic is a bug in dandori: exit 2 (ritsu's DESIGN 8.4) rather than Rust's 101, in the words of
    // every binary of the toolchain.
    ritsu_base::cli::on_panic("dandori");
    let args: Vec<String> = std::env::args().skip(1).collect();
    let ran = std::panic::catch_unwind(|| dandori::cli::run(&args, Rc::new(dandori::sources::NoRules), &mut std::io::stdout(), &mut std::io::stderr()));
    ExitCode::from(ran.unwrap_or(2))
}
