//! The `chobo` binary: the command (`chobo::run::run`), which `ritsu chobo` runs too.

fn main() -> std::process::ExitCode {
    // A panic is a bug in chobo: exit 2 (ritsu's DESIGN 8.4) rather than Rust's 101, in the words of
    // every binary of the toolchain.
    ritsu_base::cli::on_panic("chobo");
    std::panic::catch_unwind(|| chobo::run::run(std::env::args().skip(1).collect())).unwrap_or(std::process::ExitCode::from(2))
}
