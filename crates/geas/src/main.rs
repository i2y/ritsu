//! The `geas` binary: the command (`geas::cli::run`), which `ritsu geas` runs too.

fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(geas::cli::run(&raw));
}
