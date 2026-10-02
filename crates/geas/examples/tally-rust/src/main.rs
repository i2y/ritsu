//! A tally on the command line: `tally add 5 7` prints 12, `tally max 5 7`
//! prints 7. Anything that is not a whole number is refused with exit 2.

use std::process::ExitCode;

fn numbers(args: &[String]) -> Result<Vec<i64>, String> {
    args.iter().map(|a| a.parse::<i64>().map_err(|_| format!("not a number: {a}"))).collect()
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((command, rest)) = args.split_first() else {
        eprintln!("usage: tally add|max <number>...");
        eprintln!("       the numbers are whole, and may be negative");
        return ExitCode::from(2);
    };
    let xs = match numbers(rest) {
        Ok(xs) => xs,
        Err(why) => {
            eprintln!("{why}");
            return ExitCode::from(2);
        }
    };
    match command.as_str() {
        "add" => {
            let sum: i64 = xs.iter().sum();
            println!("{sum}");
        }
        "max" => match xs.iter().max() {
            Some(m) => println!("{m}"),
            None => {
                eprintln!("max needs a number");
                return ExitCode::from(2);
            }
        },
        other => {
            eprintln!("unknown command: {other}");
            return ExitCode::from(2);
        }
    }
    ExitCode::SUCCESS
}
