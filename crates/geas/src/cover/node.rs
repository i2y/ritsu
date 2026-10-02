//! Reading V8's coverage JSON, which Node writes into `NODE_V8_COVERAGE`: one
//! `coverage-<pid>-<time>-<n>.json` a process, and one more each time geas's hook
//! takes coverage on SIGTERM. V8 reports ranges of characters with counts, nested,
//! the innermost one deciding; geas turns them into lines.

use super::{FileLines, Report};
use crate::json::{self, J};
use crate::tree;
use std::path::{Path, PathBuf};

/// One range V8 reports: UTF-16 offsets into the script, end not included.
struct Range {
    start: u64,
    end: u64,
    count: u64,
}

/// The files under `root` one coverage file reports. `source` reads a file the way
/// the run saw it (the tests hand in recorded sources).
pub fn read(text: &str, root: &Path, source: &dyn Fn(&Path) -> Option<String>) -> Result<Report, String> {
    let v = json::parse(text)?;
    let Some(J::Arr(scripts)) = get(&v, "result") else {
        return Err("no `result` list".into());
    };
    let mut out = Report::new();
    for script in scripts {
        let Some(J::Str(url)) = get(script, "url") else {
            continue;
        };
        let Some(path) = file_url(url) else {
            continue; // node:internal and the like
        };
        let Some(rel) = tree::relative(root, &path) else {
            continue;
        };
        if !tree::is_source(&rel) {
            continue;
        }
        let Some(src) = source(&path) else {
            continue;
        };
        let mut ranges = Vec::new();
        if let Some(J::Arr(functions)) = get(script, "functions") {
            for f in functions {
                if let Some(J::Arr(rs)) = get(f, "ranges") {
                    for r in rs {
                        let n = |k: &str| match get(r, k) {
                            Some(J::Num(x)) if *x >= 0.0 => Ok(*x as u64),
                            _ => Err(format!("a range of {rel} has no `{k}`")),
                        };
                        ranges.push(Range { start: n("startOffset")?, end: n("endOffset")?, count: n("count")? });
                    }
                }
            }
        }
        super::add(&mut out, rel, lines(&src, &ranges));
    }
    Ok(out)
}

fn get<'a>(v: &'a J, key: &str) -> Option<&'a J> {
    match v {
        J::Obj(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
        _ => None,
    }
}

/// The file a script's URL names: an ES module's `file://` URL, percent-decoded,
/// or a CommonJS script's plain absolute path. None for Node's own scripts.
fn file_url(url: &str) -> Option<PathBuf> {
    if url.starts_with('/') {
        return Some(PathBuf::from(url));
    }
    let rest = url.strip_prefix("file://")?;
    let bytes = rest.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    let hex = |b: u8| (b as char).to_digit(16).map(|d| d as u8);
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2]))
        {
            out.push(h * 16 + l);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok().map(PathBuf::from)
}

/// Every non-blank line is code; it ran when the innermost range holding its first
/// non-blank character (the shortest; on a tie, the later one) has a count above 0.
fn lines(src: &str, ranges: &[Range]) -> FileLines {
    let mut f = FileLines::default();
    let mut offset: u64 = 0; // UTF-16 units, as V8 counts
    for (i, line) in src.split('\n').enumerate() {
        let mut first = None;
        let mut at = offset;
        for c in line.chars() {
            if !c.is_whitespace() {
                first = Some(at);
                break;
            }
            at += c.len_utf16() as u64;
        }
        if let Some(pos) = first {
            let n = i as u32 + 1;
            f.code.insert(n);
            let mut best: Option<&Range> = None;
            for r in ranges {
                if r.start <= pos && pos < r.end && best.is_none_or(|b| r.end - r.start <= b.end - b.start) {
                    best = Some(r);
                }
            }
            if best.is_some_and(|r| r.count > 0) {
                f.ran.insert(n);
            }
        }
        offset += line.encode_utf16().count() as u64 + 1;
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lines::to_ranges;

    /// What V8 wrote for `node shapes.ts square 3`, read against the TypeScript
    /// file itself: Node blanks the types out, so the offsets fall on its lines.
    #[test]
    fn a_recorded_run() {
        use crate::cover::fixtures;
        let source = |p: &Path| {
            let rel = p.strip_prefix("/ROOT").ok()?;
            std::fs::read_to_string(fixtures::dir("node").join(rel)).ok()
        };
        let r = read(&fixtures::read("node", "coverage-1.json"), Path::new("/ROOT"), &source).unwrap();
        fixtures::golden("node.txt", &fixtures::render(&r));
    }

    #[test]
    fn urls_are_decoded() {
        assert_eq!(file_url("file:///ROOT/a%20b/s.ts"), Some(PathBuf::from("/ROOT/a b/s.ts")));
        assert_eq!(file_url("node:internal/main"), None);
        // a CommonJS script is named by its path, as it is
        assert_eq!(file_url("/ROOT/a%20b.js"), Some(PathBuf::from("/ROOT/a%20b.js")));
        assert_eq!(file_url("file:///ROOT/%E6%97%A5.js"), Some(PathBuf::from("/ROOT/日.js")));
    }

    #[test]
    fn the_innermost_range_decides() {
        // line 1 in the top range (run), line 3 in a block that did not run, line 4
        // in the function again, line 6 blank
        let src = "a();\nfunction f() {\n  if (x) { y(); }\n  return 1;\n}\n\n";
        let start_if = src.find("if").unwrap() as u64;
        let end_if = start_if + "if (x) { y(); }".len() as u64;
        let ranges = vec![
            Range { start: 0, end: src.len() as u64, count: 1 },
            Range { start: 5, end: 47, count: 1 },
            Range { start: start_if, end: end_if, count: 0 },
        ];
        let f = lines(src, &ranges);
        assert_eq!(to_ranges(&f.code), "1-5");
        assert_eq!(to_ranges(&f.ran), "1-2,4-5");
    }

    #[test]
    fn offsets_count_utf16() {
        // "é" is one unit and "😀" two: the second line starts at 5 + 1 + 1
        let src = "//é😀\nx();\n";
        let second = "//é😀\n".encode_utf16().count() as u64;
        let ranges = vec![Range { start: 0, end: 100, count: 1 }, Range { start: second, end: second + 4, count: 0 }];
        let f = lines(src, &ranges);
        assert_eq!(to_ranges(&f.ran), "1");
        assert_eq!(to_ranges(&f.code), "1-2");
    }
}
