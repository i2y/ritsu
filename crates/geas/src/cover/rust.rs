//! Reading a Rust program's profile. A program built with `-C instrument-coverage`
//! writes a raw profile at exit, named as `LLVM_PROFILE_FILE` asks:
//! `rust-<pid>-<signature>.profraw`. `llvm-profdata merge` and `llvm-cov export
//! -format=lcov` turn the profiles and the program into lines, `SF:<file>` then
//! `DA:<line>,<count>` for each line that is code.

use super::{FileLines, Report};
use crate::tree;
use std::path::Path;

/// The pid and the signature in a profile's name, as geas asks for it
/// (`rust-%p-%m.profraw`); the signature is the same for every process of one
/// program.
pub fn profile_name(name: &str) -> Option<(u32, &str)> {
    let rest = name.strip_prefix("rust-")?.strip_suffix(".profraw")?;
    let (pid, signature) = rest.split_once('-')?;
    Some((pid.parse().ok()?, signature))
}

/// The files under `root` an lcov export names.
pub fn read_lcov(text: &str, root: &Path) -> Result<Report, String> {
    let mut out = Report::new();
    let mut file: Option<(String, FileLines)> = None;
    let mut inside = false;
    for (i, line) in text.lines().enumerate() {
        if let Some(path) = line.strip_prefix("SF:") {
            inside = true;
            file = tree::relative(root, Path::new(path)).filter(|r| tree::is_source(r)).map(|r| (r, FileLines::default()));
        } else if let Some(rest) = line.strip_prefix("DA:") {
            let mut parts = rest.split(',');
            let (Some(Ok(n)), Some(Ok(count))) =
                (parts.next().map(str::parse::<u32>), parts.next().map(str::parse::<u64>))
            else {
                return Err(format!("line {} of the lcov export does not read: {line}", i + 1));
            };
            if !inside {
                return Err(format!("line {} of the lcov export comes before any `SF:`", i + 1));
            }
            if let Some((_, f)) = file.as_mut()
                && n > 0
            {
                f.code.insert(n);
                if count > 0 {
                    f.ran.insert(n);
                }
            }
        } else if line == "end_of_record" {
            if let Some((rel, f)) = file.take() {
                super::add(&mut out, rel, f);
            }
            inside = false;
        }
    }
    if let Some((rel, f)) = file.take() {
        super::add(&mut out, rel, f);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lines::to_ranges;

    /// What `llvm-cov export -format=lcov` wrote for `./shapes square 3`.
    #[test]
    fn a_recorded_run() {
        use crate::cover::fixtures;
        let r = read_lcov(&fixtures::read("rust", "rust.lcov"), Path::new("/ROOT")).unwrap();
        fixtures::golden("rust.txt", &fixtures::render(&r));
    }

    #[test]
    fn profile_names() {
        assert_eq!(profile_name("rust-98608-9182624215952347227_0.profraw"), Some((98608, "9182624215952347227_0")));
        assert_eq!(profile_name("default_1_2.profraw"), None);
        assert_eq!(profile_name("rust-x-1.profraw"), None);
    }

    #[test]
    fn lcov_into_lines() {
        let text = "SF:/ROOT/src/main.rs\nFN:1,f\nDA:1,1\nDA:2,0\nDA:5,3\nLF:3\nend_of_record\n\
                    SF:/rustc/abc/library/std/src/lib.rs\nDA:1,1\nend_of_record\n";
        let r = read_lcov(text, Path::new("/ROOT")).unwrap();
        assert_eq!(r.keys().collect::<Vec<_>>(), ["src/main.rs"]);
        assert_eq!(to_ranges(&r["src/main.rs"].code), "1-2,5");
        assert_eq!(to_ranges(&r["src/main.rs"].ran), "1,5");
        assert!(read_lcov("DA:1,1\n", Path::new("/ROOT")).is_err());
        assert!(read_lcov("SF:/ROOT/a.rs\nDA:x,1\n", Path::new("/ROOT")).is_err());
    }
}
