//! The `geas` binary: the command (`geas::cli::run`), which `ritsu geas` runs too.

fn main() {
    // A panic is a bug in geas: exit 2 (ritsu's DESIGN 8.4) rather than Rust's 101, in the words of
    // every binary of the toolchain.
    ritsu_base::cli::on_panic("geas");
    let raw: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(std::panic::catch_unwind(|| geas::cli::run(&raw)).unwrap_or(2));
}
