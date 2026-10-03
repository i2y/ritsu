//! The line a test prints when it does not run what it is about, and passes (DESIGN 10.3):
//! `SKIP: <crate>: <reason>`. With `RITSU_SKIP_LOG` naming a file, the same goes into it too, a
//! line each: the crate, the test, why (`level` when `RITSU_TEST_LEVEL` left it out, `missing`
//! when what it needs is not on the machine), the reason, between tabs. `cargo xtask test`
//! gathers the file into a table and holds the `missing` ones to the SKIPs the level allows
//! (`ci/skips/<level>.txt`).

use std::io::Write;

/// The test that is running: libtest names each test's thread after it.
pub fn test_name() -> String {
    std::thread::current().name().unwrap_or("?").to_string()
}

/// Why a test did not run what it is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Why {
    /// `RITSU_TEST_LEVEL` leaves it to a higher level (DESIGN 10.2).
    Level,
    /// What it needs is not on the machine.
    Missing,
}

impl Why {
    pub fn word(self) -> &'static str {
        match self {
            Why::Level => "level",
            Why::Missing => "missing",
        }
    }
}

/// Say why the test does not run what it is about, a program or a server it needs not being
/// there; the test then returns and passes.
pub fn skip(reason: &str) {
    skip_for(Why::Missing, reason);
}

/// The same, saying why.
pub fn skip_for(why: Why, reason: &str) {
    let krate = crate::crate_name();
    // On the standard output, where `cargo test -- --nocapture` shows it with the test's own
    // lines; the crates printed theirs on either, and a reader looks for `SKIP:` in both.
    println!("SKIP: {krate}: {reason}");
    if let Some(log) = std::env::var_os("RITSU_SKIP_LOG").filter(|v| !v.is_empty()) {
        let line = format!("{krate}\t{}\t{}\t{}\n", test_name(), why.word(), reason.replace(['\t', '\n'], " "));
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(log) {
            let _ = f.write_all(line.as_bytes());
        }
    }
}

/// One line of a SKIP log.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Logged {
    pub krate: String,
    pub test: String,
    pub why: Why,
    pub reason: String,
}

/// The lines of a SKIP log; a line that is not four fields is passed over.
pub fn read_log(text: &str) -> Vec<Logged> {
    text.lines()
        .filter_map(|l| {
            let mut it = l.splitn(4, '\t');
            let (krate, test, why, reason) = (it.next()?, it.next()?, it.next()?, it.next()?);
            let why = match why {
                "level" => Why::Level,
                "missing" => Why::Missing,
                _ => return None,
            };
            Some(Logged { krate: krate.to_string(), test: test.to_string(), why, reason: reason.to_string() })
        })
        .collect()
}
