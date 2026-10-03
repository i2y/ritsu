//! Areas of a few shapes, from the command line.

fn area(kind: &str, size: i64) -> i64 {
    match kind {
        "square" => size * size,
        "circle" => 3 * size * size,
        _ => panic!("{kind}"),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let size: i64 = args[2].parse().unwrap();
    println!("{}", area(&args[1], size));
}
