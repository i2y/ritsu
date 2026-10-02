//! Reading a Go program's counters: `go tool covdata textfmt` turns the files a
//! program built with `-cover` writes into `GOCOVERDIR` into blocks, each
//! `<import path>/<file>:<line>.<col>,<line>.<col> <statements> <count>`. The
//! import paths become files under the root through the `module` lines of the
//! `go.mod` files there.

use super::{FileLines, Report};
use crate::tree;

/// A module: its path, and the directory of its `go.mod` relative to the root
/// (`""` for the root itself).
pub struct Module {
    pub path: String,
    pub dir: String,
}

/// The module path a `go.mod` declares.
pub fn module_path(go_mod: &str) -> Option<String> {
    for line in go_mod.lines() {
        let line = line.split("//").next().unwrap_or("").trim();
        if let Some(rest) = line.strip_prefix("module") {
            if !rest.starts_with(|c: char| c.is_whitespace() || c == '"') {
                continue;
            }
            let p = rest.trim().trim_matches('"');
            if !p.is_empty() {
                return Some(p.to_string());
            }
        }
    }
    None
}

/// The file under the root an import path's file is: the longest module path that
/// is a prefix wins.
fn file_of(name: &str, modules: &[Module]) -> Option<String> {
    let mut best: Option<(&Module, &str)> = None;
    for m in modules {
        if let Some(rest) = name.strip_prefix(&m.path).and_then(|r| r.strip_prefix('/'))
            && best.is_none_or(|(b, _)| m.path.len() > b.path.len())
        {
            best = Some((m, rest));
        }
    }
    let (m, rest) = best?;
    Some(if m.dir.is_empty() { rest.to_string() } else { format!("{}/{rest}", m.dir) })
}

/// The lines of the text format: every block's lines are code, and run when its
/// count is above zero.
pub fn read(text: &str, modules: &[Module]) -> Result<Report, String> {
    let mut out = Report::new();
    for (i, line) in text.lines().enumerate() {
        if line.is_empty() || line.starts_with("mode:") {
            continue;
        }
        let bad = || format!("line {} of the text format does not read: {line}", i + 1);
        let fields: Vec<&str> = line.split_whitespace().collect();
        let [place, _statements, count] = fields.as_slice() else {
            return Err(bad());
        };
        let (name, span) = place.rsplit_once(':').ok_or_else(bad)?;
        let (from, to) = span.split_once(',').ok_or_else(bad)?;
        let line_of = |pos: &str| pos.split_once('.').and_then(|(l, _)| l.parse::<u32>().ok());
        let (Some(l1), Some(l2), Ok(n)) = (line_of(from), line_of(to), count.parse::<u64>()) else {
            return Err(bad());
        };
        let Some(rel) = file_of(name, modules) else {
            continue; // a package from outside the root
        };
        if !tree::is_source(&rel) {
            continue;
        }
        let mut f = FileLines::default();
        for l in l1..=l2.max(l1) {
            f.code.insert(l);
            if n > 0 {
                f.ran.insert(l);
            }
        }
        super::add(&mut out, rel, f);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lines::to_ranges;

    /// What `go tool covdata textfmt` wrote for `./shapes square 3`.
    #[test]
    fn a_recorded_run() {
        use crate::cover::fixtures;
        let path = module_path(&fixtures::read("go", "go.mod")).unwrap();
        let modules = vec![Module { path, dir: String::new() }];
        let r = read(&fixtures::read("go", "go.txt"), &modules).unwrap();
        fixtures::golden("go.txt", &fixtures::render(&r));
    }

    #[test]
    fn module_lines() {
        assert_eq!(module_path("// a comment\nmodule tally\n\ngo 1.22\n").as_deref(), Some("tally"));
        assert_eq!(module_path("module \"example.com/x\" // quoted\n").as_deref(), Some("example.com/x"));
        assert_eq!(module_path("go 1.22\n"), None);
        assert_eq!(module_path("modules x\n"), None);
    }

    #[test]
    fn blocks_into_lines() {
        let modules = vec![
            Module { path: "tally".into(), dir: "examples/tally-go".into() },
            Module { path: "tally/internal/x".into(), dir: "vendored/x".into() },
        ];
        let text = "mode: set\n\
                    tally/main.go:8.13,9.22 1 1\n\
                    tally/main.go:9.22,11.3 1 1\n\
                    tally/main.go:11.8,13.3 1 0\n\
                    tally/internal/x/x.go:3.1,4.2 1 1\n\
                    fmt/print.go:1.1,2.2 1 1\n";
        let r = read(text, &modules).unwrap();
        assert_eq!(r.keys().collect::<Vec<_>>(), ["examples/tally-go/main.go", "vendored/x/x.go"]);
        let m = &r["examples/tally-go/main.go"];
        assert_eq!(to_ranges(&m.code), "8-13");
        // line 11 closes the run block and opens the one that did not run
        assert_eq!(to_ranges(&m.ran), "8-11");
        assert!(read("tally/main.go:8.13 1\n", &modules).is_err());
    }
}
