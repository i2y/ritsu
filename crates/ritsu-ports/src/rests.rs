//! What a file's answers rest on, as `Rules::rests_on` and `Dates::rests_on` say it (DESIGN 17.9.4).
//! Two files whose strings are equal answer alike, so only what can change an answer goes in:
//!
//! - where each file read is, seen from the file asked about, and not from the root of the disk: a
//!   copy of a tree at another place rests on the same.
//! - what a rule or a dates file says, as [`ritsu_base::definition`] reads its lines, without its
//!   comments, its alignment and its `description` lines. Any other file (a table of holidays) goes
//!   in byte for byte.
//! - not `ritsu.mod` and `ritsu.lock`: what they can change is which files a dependency's path comes
//!   to, and those files are in with their paths and what they hold. So raising a module's version
//!   changes nothing here.

use ritsu_base::definition;
use ritsu_base::modules::{lock, manifest};
use std::path::Path;

/// A digest of what the file at `file` rests on: `text`, when the reads do not hold the file itself,
/// and each file read with what it held (None: it could not be read), in the order of their paths.
pub fn rests_on<'a>(file: &Path, text: Option<&str>, reads: impl IntoIterator<Item = (&'a Path, Option<&'a [u8]>)>) -> String {
    let mut read: Vec<(String, Option<Vec<u8>>)> =
        reads.into_iter().filter(|(p, _)| !module_file(p)).map(|(p, b)| (ritsu_base::paths::seen_from(file, p), b.map(|b| what_it_says(p, b)))).collect();
    read.sort();
    read.dedup_by(|a, b| a.0 == b.0);
    fn put(all: &mut Vec<u8>, b: &[u8]) {
        all.extend_from_slice(&(b.len() as u64).to_le_bytes());
        all.extend_from_slice(b);
    }
    let mut all = Vec::new();
    match text {
        Some(t) => {
            all.push(1);
            put(&mut all, source(t).as_bytes());
        }
        None => all.push(0),
    }
    for (p, b) in read {
        put(&mut all, p.as_bytes());
        match b {
            Some(b) => {
                all.push(1);
                put(&mut all, &b);
            }
            None => all.push(0),
        }
    }
    ritsu_base::sha256::hex(&all)
}

/// Whether a file only says where a module's files are.
fn module_file(p: &Path) -> bool {
    p.file_name().is_some_and(|n| n == manifest::FILE || n == lock::FILE)
}

fn what_it_says(p: &Path, bytes: &[u8]) -> Vec<u8> {
    let language = matches!(p.extension().and_then(|e| e.to_str()), Some("rule" | "cal"));
    match std::str::from_utf8(bytes) {
        Ok(text) if language => source(text).into_bytes(),
        _ => bytes.to_vec(),
    }
}

/// The text of a rule or a dates file as far as its answers go.
fn source(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let block = definition::block(&lines, 1, lines.len());
    let said = block.lines().filter(|l| {
        let l = l.trim_start();
        !(l.starts_with("description \"") && l.ends_with('"'))
    });
    said.collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    const RULE: &str = "rule fee v1\ndescription \"The fee\"\n\ninputs\n  size : size\n\ntable fee\n| size | -> fee |\n| a    | 1      |\n";

    fn digest(file: &str, text: &str, reads: &[(&str, Option<&str>)]) -> String {
        rests_on(Path::new(file), Some(text), reads.iter().map(|(p, b)| (Path::new(*p), b.map(str::as_bytes))))
    }

    #[test]
    fn a_copy_at_another_place_rests_on_the_same() {
        let here = digest("/a/rules/fee.rule", RULE, &[("/a/rules/sizes.rule", Some("x")), ("/a/shared/days.csv", Some("y"))]);
        let there = digest("/b/c/d/rules/fee.rule", RULE, &[("/b/c/d/shared/days.csv", Some("y")), ("/b/c/d/rules/sizes.rule", Some("x"))]);
        assert_eq!(here, there);
        // the same file spelled two ways is one file
        assert_eq!(here, digest("/a/rules/fee.rule", RULE, &[("/a/rules/./sizes.rule", Some("x")), ("/a/rules/../shared/days.csv", Some("y")), ("/a/rules/sizes.rule", Some("x"))]));
    }

    #[test]
    fn where_a_file_read_is_from_the_file_asked_about_counts() {
        let here = digest("/a/fee.rule", RULE, &[("/a/sizes.rule", Some("x"))]);
        assert_ne!(here, digest("/a/fee.rule", RULE, &[("/a/other/sizes.rule", Some("x"))]));
        // two files that hold each other's text
        let swapped = |x, y| digest("/a/fee.rule", RULE, &[("/a/b.rule", Some(x)), ("/a/c.rule", Some(y))]);
        assert_ne!(swapped("one", "two"), swapped("two", "one"));
        // a file that could not be read is not a file that holds nothing
        assert_ne!(digest("/a/fee.rule", RULE, &[("/a/b.csv", Some(""))]), digest("/a/fee.rule", RULE, &[("/a/b.csv", None)]));
        assert_ne!(digest("/a/fee.rule", RULE, &[("/a/b.csv", None)]), digest("/a/fee.rule", RULE, &[]));
    }

    #[test]
    fn what_cannot_change_an_answer_changes_nothing() {
        let here = digest("/a/fee.rule", RULE, &[]);
        let noted = RULE.replace("rule fee v1\n", "# about the fee\nrule fee v1  # one\n").replace("description \"The fee\"", "description \"The fee of an order\"").replace("| a    | 1      |", "|  a  |  1  | # note");
        assert_ne!(noted, RULE);
        assert_eq!(here, digest("/a/fee.rule", &noted, &[]));
        // the same in a file read
        let read = |t| digest("/a/fee.rule", RULE, &[("/a/sizes.rule", Some(t))]);
        assert_eq!(read("enums sizes v1\ndescription \"a\"\nenum size = a | b\n"), read("enums sizes v1\n\ndescription \"b\"\n# c\nenum size  =  a | b   # d\n"));
        // the files that say where a module's files are
        assert_eq!(here, digest("/a/fee.rule", RULE, &[("/a/ritsu.mod", Some("module a\nversion 1.0.0\n")), ("/a/ritsu.lock", None)]));
    }

    #[test]
    fn what_can_change_an_answer_changes_it() {
        let here = digest("/a/fee.rule", RULE, &[]);
        assert_ne!(here, digest("/a/fee.rule", &RULE.replace("| a    | 1      |", "| a    | 2      |"), &[]));
        // a `#` in a string is not a comment, and a description is not any line that says the word
        let read = |t| digest("/a/fee.rule", RULE, &[("/a/sizes.rule", Some(t))]);
        assert_ne!(read("label \"a # b\"\n"), read("label \"a # c\"\n"));
        assert_ne!(read("  description : string\n"), read("  description : int\n"));
        // the depth of a line under another counts
        assert_ne!(read("a\n  b\n"), read("a\nb\n"));
        // a file of another kind goes in as it is
        let csv = |t| digest("/a/fee.rule", RULE, &[("/a/days.csv", Some(t))]);
        assert_ne!(csv("2026-01-01,a\n"), csv("2026-01-01,a # b\n"));
    }
}
