//! The `chobo` binary: the command (`chobo::run::run`), which `ritsu chobo` runs too.

fn main() -> std::process::ExitCode {
    chobo::run::run(std::env::args().skip(1).collect())
}
