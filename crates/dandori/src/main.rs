//! The `dandori` command: crate::cli runs it. This binary holds no rulec (ritsu's DESIGN 2.3): it
//! runs a workflow that uses no rule as it always has, and where a flow reads a rule it says to run
//! it with `ritsu dandori`, which reads the rule in the same process.

use std::process::ExitCode;
use std::rc::Rc;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = dandori::cli::run(&args, Rc::new(dandori::sources::NoRules), &mut std::io::stdout(), &mut std::io::stderr());
    ExitCode::from(code)
}
