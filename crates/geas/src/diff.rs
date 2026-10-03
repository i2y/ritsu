//! Unified diffs, from `git diff` or `diff -u`, and the hashes of the two sides of
//! each file (DESIGN §7.5). A git diff names the sides' blobs on its `index` lines;
//! a plain diff is held to the file on disk, which has to be one of its sides, and
//! the other side is the disk with the hunks undone or done.

use ritsu_base::text::Text;
use crate::hash;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Context,
    Removed,
    Added,
}

#[derive(Clone, Debug)]
pub struct HunkLine {
    pub mark: Mark,
    pub text: Vec<u8>,
    /// Followed by `\ No newline at end of file`.
    pub no_newline: bool,
    /// Its line in the diff, from 1.
    pub at: usize,
}

#[derive(Clone, Debug)]
pub struct Hunk {
    pub old_start: u32,
    pub old_len: u32,
    pub new_start: u32,
    pub new_len: u32,
    pub lines: Vec<HunkLine>,
}

impl Hunk {
    /// The hunk's lines on one side: context and removed lines before, context and
    /// added lines after.
    fn side(&self, after: bool) -> Vec<&HunkLine> {
        let other = if after { Mark::Removed } else { Mark::Added };
        self.lines.iter().filter(|l| l.mark != other).collect()
    }

    /// Where the hunk's lines on one side start, counted from 0. A side with no
    /// lines names the line before the place.
    fn start0(&self, after: bool) -> usize {
        let (start, len) = if after { (self.new_start, self.new_len) } else { (self.old_start, self.old_len) };
        if len == 0 { start as usize } else { start as usize - 1 }
    }

    /// Whether the hunk ends where the file ends. As `patch` reads a hunk: one with
    /// fewer lines of context after its changes than before them ran into the end
    /// of the file, since a diff gives as much context on both sides as there is.
    fn at_the_end(&self) -> bool {
        let leading = self.lines.iter().take_while(|l| l.mark == Mark::Context).count();
        let trailing = self.lines.iter().rev().take_while(|l| l.mark == Mark::Context).count();
        leading < self.lines.len() && trailing < leading
    }
}

#[derive(Clone, Debug, Default)]
pub struct FileDiff {
    /// The path before, relative to the root; None for a file the diff adds.
    pub old_path: Option<String>,
    /// The path after; None for a file the diff deletes.
    pub new_path: Option<String>,
    /// The blobs on a git diff's `index` line, as written (often abbreviated);
    /// all zeros for a side that does not exist.
    pub old_blob: Option<String>,
    pub new_blob: Option<String>,
    pub binary: bool,
    pub hunks: Vec<Hunk>,
    /// The line of the file's first header in the diff.
    pub at: usize,
}

impl FileDiff {
    /// The path the file has on disk after the change, or before it for a deletion.
    pub fn path(&self) -> &str {
        self.new_path.as_deref().or(self.old_path.as_deref()).unwrap_or("")
    }

    pub fn added(&self) -> bool {
        self.old_path.is_none()
    }

    pub fn deleted(&self) -> bool {
        self.new_path.is_none()
    }
}

/// Why a diff does not read: its line (from 1, 0 for the whole) and the message.
pub type DiffError = (usize, Text);

fn err(at: usize, en: impl Into<String>, ja: impl Into<String>) -> DiffError {
    (at, Text::new(ja, en))
}

/// A path as git writes it: plain, or in double quotes with C escapes, the bytes
/// of a non-ASCII name in octal.
fn unquote(s: &str) -> Option<String> {
    let Some(inner) = s.strip_prefix('"').and_then(|r| r.strip_suffix('"')) else {
        return Some(s.to_string());
    };
    let b = inner.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'\\' {
            out.push(b[i]);
            i += 1;
            continue;
        }
        let c = *b.get(i + 1)?;
        i += 2;
        match c {
            b'n' => out.push(b'\n'),
            b't' => out.push(b'\t'),
            b'r' => out.push(b'\r'),
            b'a' => out.push(7),
            b'b' => out.push(8),
            b'f' => out.push(12),
            b'v' => out.push(11),
            b'"' | b'\\' => out.push(c),
            b'0'..=b'7' => {
                let digits = std::str::from_utf8(b.get(i - 1..i + 2)?).ok()?;
                out.push(u8::from_str_radix(digits, 8).ok()?);
                i += 2;
            }
            _ => return None,
        }
    }
    String::from_utf8(out).ok()
}

/// The path on a `---` or `+++` line: up to a tab (a plain diff's date follows
/// one), unquoted; None for `/dev/null`.
fn header_path(rest: &str) -> Option<Option<String>> {
    let p = rest.split('\t').next().unwrap_or("").trim_end_matches('\r');
    let p = if p.starts_with('"') { p.to_string() } else { p.trim_end().to_string() };
    let p = unquote(&p)?;
    Some(if p == "/dev/null" { None } else { Some(p) })
}

/// `@@ -a[,b] +c[,d] @@`
fn hunk_header(line: &str) -> Option<(u32, u32, u32, u32)> {
    let rest = line.strip_prefix("@@ -")?;
    let (ranges, _) = rest.split_once(" @@")?;
    let (old, new) = ranges.split_once(" +")?;
    let range = |r: &str| -> Option<(u32, u32)> {
        match r.split_once(',') {
            Some((a, b)) => Some((a.parse().ok()?, b.parse().ok()?)),
            None => Some((r.parse().ok()?, 1)),
        }
    };
    let (a, b) = range(old)?;
    let (c, d) = range(new)?;
    Some((a, b, c, d))
}

/// The two paths of `diff --git a/x b/y` when nothing else names them: a binary
/// file or a change of mode, where both are the same.
fn git_line_paths(rest: &str) -> Option<(String, String)> {
    if let Some(inner) = rest.strip_prefix('"') {
        // the first path ends at the first quote no backslash escapes
        let mut escaped = false;
        let end = inner.char_indices().find_map(|(i, c)| {
            let close = c == '"' && !escaped;
            escaped = c == '\\' && !escaped;
            close.then_some(i)
        })?;
        let a = unquote(&rest[..end + 2])?;
        let b = unquote(rest[end + 2..].trim_start())?;
        return Some((a, b));
    }
    // `a/P b/P`: the same path twice
    let n = rest.len().checked_sub(1)? / 2;
    let (a, b) = (rest.get(..n)?, rest.get(n + 1..)?);
    if rest.as_bytes().get(n) == Some(&b' ') && a.get(2..) == b.get(2..) {
        return Some((a.to_string(), b.to_string()));
    }
    None
}

fn strip(p: Option<String>, prefix: &str) -> Option<String> {
    p.map(|p| p.strip_prefix(prefix).map(str::to_string).unwrap_or(p))
}

/// Reads a unified diff. Lines outside any file (a commit message, `diff -r`'s
/// own lines, `Only in …`) are passed over.
pub fn parse(text: &[u8]) -> Result<Vec<FileDiff>, DiffError> {
    let lines: Vec<&[u8]> = {
        let mut v: Vec<&[u8]> = text.split(|b| *b == b'\n').collect();
        if v.last().is_some_and(|l| l.is_empty()) {
            v.pop();
        }
        v
    };
    let s = |i: usize| String::from_utf8_lossy(lines[i]).into_owned();
    let unreadable = |at: usize| err(at, "a path that does not read", "パスが読めません");
    let mut files: Vec<FileDiff> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = s(i);
        if let Some(rest) = line.strip_prefix("diff --git ") {
            let mut f = FileDiff { at: i + 1, ..Default::default() };
            let prefixed = rest.starts_with("a/") || rest.starts_with("\"a/");
            let line_paths = git_line_paths(rest.trim_end_matches('\r'));
            let (mut old, mut new) = (None, None);
            let (mut from, mut to) = (None, None);
            let (mut added, mut deleted) = (false, false);
            i += 1;
            while i < lines.len() {
                let h = s(i);
                if h.starts_with("diff --git ") || h.starts_with("@@ ") {
                    break;
                } else if let Some(r) = h.strip_prefix("--- ") {
                    old = Some(header_path(r).ok_or_else(|| unreadable(i + 1))?);
                } else if let Some(r) = h.strip_prefix("+++ ") {
                    new = Some(header_path(r).ok_or_else(|| unreadable(i + 1))?);
                } else if let Some(r) = h.strip_prefix("rename from ").or_else(|| h.strip_prefix("copy from ")) {
                    from = unquote(r.trim_end_matches('\r'));
                } else if let Some(r) = h.strip_prefix("rename to ").or_else(|| h.strip_prefix("copy to ")) {
                    to = unquote(r.trim_end_matches('\r'));
                } else if h.starts_with("new file mode") {
                    added = true;
                } else if h.starts_with("deleted file mode") {
                    deleted = true;
                } else if let Some(r) = h.strip_prefix("index ") {
                    let ids = r.split_whitespace().next().unwrap_or("");
                    let (a, b) = ids
                        .split_once("..")
                        .ok_or_else(|| err(i + 1, "an `index` line without `..`", "`..` のない `index` の行です"))?;
                    f.old_blob = Some(a.to_string());
                    f.new_blob = Some(b.to_string());
                } else if h.starts_with("Binary files ") || h.starts_with("GIT binary patch") {
                    f.binary = true;
                } else if !(h.starts_with("old mode")
                    || h.starts_with("new mode")
                    || h.starts_with("similarity index")
                    || h.starts_with("dissimilarity index")
                    || f.binary)
                {
                    break;
                }
                i += 1;
            }
            let (pa, pb) = if prefixed { ("a/", "b/") } else { ("", "") };
            let fallback = line_paths.map(|(a, b)| (strip(Some(a), pa), strip(Some(b), pb)));
            f.old_path = match (old, &from) {
                (Some(p), _) => strip(p, pa),
                (None, Some(p)) => Some(p.clone()),
                (None, None) => fallback.as_ref().and_then(|x| x.0.clone()),
            };
            f.new_path = match (new, &to) {
                (Some(p), _) => strip(p, pb),
                (None, Some(p)) => Some(p.clone()),
                (None, None) => fallback.as_ref().and_then(|x| x.1.clone()),
            };
            if added {
                f.old_path = None;
            }
            if deleted {
                f.new_path = None;
            }
            if f.old_path.is_none() && f.new_path.is_none() {
                return Err(err(f.at, "a file whose path does not read", "パスが読めないファイルがあります"));
            }
            i = hunks(&lines, i, &mut f)?;
            files.push(f);
        } else if let Some(r) = line.strip_prefix("--- ") {
            let at = i + 1;
            let old = header_path(r).ok_or_else(|| unreadable(at))?;
            let next = if i + 1 < lines.len() { s(i + 1) } else { String::new() };
            let Some(r2) = next.strip_prefix("+++ ") else {
                return Err(err(at + 1, "a `---` line not followed by `+++`", "`---` の行の次に `+++` の行がありません"));
            };
            let new = header_path(r2).ok_or_else(|| unreadable(at + 1))?;
            // A plain diff has no renames: its two names are one file, as `patch` reads
            // them (`diff -u calc.py.orig calc.py`), and the file is the `+++` one.
            // `--- a/x` and `+++ b/x`, as Mercurial writes them, lose their prefixes.
            let both = old.as_deref().is_some_and(|p| p.starts_with("a/"))
                && new.as_deref().is_some_and(|p| p.starts_with("b/"));
            let (old, new) = if both { (strip(old, "a/"), strip(new, "b/")) } else { (old, new) };
            let old = if old.is_some() && new.is_some() { new.clone() } else { old };
            if old.is_none() && new.is_none() {
                return Err(err(at, "a file whose path does not read", "パスが読めないファイルがあります"));
            }
            let mut f = FileDiff { old_path: old, new_path: new, at, ..Default::default() };
            i = hunks(&lines, i + 2, &mut f)?;
            // `diff -N` writes an added or a deleted file against an empty one
            if let [h] = f.hunks.as_slice()
                && f.old_path.is_some()
                && f.new_path.is_some()
            {
                if h.old_start == 0 && h.old_len == 0 {
                    f.old_path = None;
                } else if h.new_start == 0 && h.new_len == 0 {
                    f.new_path = None;
                }
            }
            files.push(f);
        } else if line.starts_with("@@ ") {
            return Err(err(i + 1, "a hunk before any file's header", "ファイルのヘッダーより前にハンクがあります"));
        } else {
            i += 1;
        }
    }
    if files.is_empty() && lines.iter().any(|l| !l.iter().all(|b| b.is_ascii_whitespace())) {
        return Err(err(
            0,
            "no file in it: a diff names its files on `diff --git` lines or on `---` and `+++` lines",
            "ファイルが一つもありません。差分はファイルを `diff --git` の行か、`---` と `+++` の行で示します",
        ));
    }
    Ok(files)
}

/// Reads the hunks from line `i`; returns the line after them.
fn hunks(lines: &[&[u8]], mut i: usize, f: &mut FileDiff) -> Result<usize, DiffError> {
    while i < lines.len() {
        let line = String::from_utf8_lossy(lines[i]).into_owned();
        if !line.starts_with("@@ ") {
            break;
        }
        let at = i + 1;
        let Some((old_start, old_len, new_start, new_len)) = hunk_header(&line) else {
            let shown = crate::diag::cut(line.trim_end(), 60);
            return Err(err(at, format!("a hunk header that does not read: {shown}"), format!("ハンクのヘッダーが読めません: {shown}")));
        };
        if (old_len > 0 && old_start == 0) || (new_len > 0 && new_start == 0) {
            return Err(err(at, "a hunk that starts at line 0", "0 行目から始まるハンクがあります"));
        }
        let mut h = Hunk { old_start, old_len, new_start, new_len, lines: vec![] };
        let (mut old_left, mut new_left) = (old_len, new_len);
        i += 1;
        while old_left > 0 || new_left > 0 {
            let Some(l) = lines.get(i) else {
                return Err(err(at, "the hunk ends before the lines its header counts", "ハンクが、ヘッダーの数える行より前に終わっています"));
            };
            let (mark, text) = match l.first() {
                Some(b' ') => (Mark::Context, &l[1..]),
                Some(b'-') => (Mark::Removed, &l[1..]),
                Some(b'+') => (Mark::Added, &l[1..]),
                // an empty context line, its blank cut off by an editor
                None => (Mark::Context, &l[..]),
                Some(b'\\') => {
                    i += 1;
                    continue;
                }
                _ => {
                    return Err(err(
                        i + 1,
                        "a line in a hunk that starts with neither ` `, `-` nor `+`",
                        "ハンクの中に、` `・`-`・`+` のどれでも始まらない行があります",
                    ));
                }
            };
            match mark {
                Mark::Context if old_left > 0 && new_left > 0 => {
                    old_left -= 1;
                    new_left -= 1;
                }
                Mark::Removed if old_left > 0 => old_left -= 1,
                Mark::Added if new_left > 0 => new_left -= 1,
                _ => {
                    return Err(err(i + 1, "the hunk has more lines than its header counts", "ハンクの行が、ヘッダーの数より多くなっています"));
                }
            }
            h.lines.push(HunkLine { mark, text: text.to_vec(), no_newline: false, at: i + 1 });
            i += 1;
            if lines.get(i).is_some_and(|l| l.starts_with(b"\\")) {
                if let Some(last) = h.lines.last_mut() {
                    last.no_newline = true;
                }
                i += 1;
            }
        }
        f.hunks.push(h);
    }
    Ok(i)
}

/// A side of a file: its lines, and whether the last ends in a newline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Content {
    pub lines: Vec<Vec<u8>>,
    pub final_newline: bool,
}

impl Content {
    pub fn of(bytes: &[u8]) -> Content {
        if bytes.is_empty() {
            return Content { lines: vec![], final_newline: true };
        }
        let final_newline = bytes.ends_with(b"\n");
        let body = if final_newline { &bytes[..bytes.len() - 1] } else { bytes };
        Content { lines: body.split(|b| *b == b'\n').map(<[u8]>::to_vec).collect(), final_newline }
    }

    pub fn bytes(&self) -> Vec<u8> {
        let mut out = self.lines.join(&b'\n');
        if self.final_newline && !self.lines.is_empty() {
            out.push(b'\n');
        }
        out
    }
}

/// Whether `c` is the file on one side of the diff: every hunk's lines on that side
/// are where the hunk puts them, the final newline included, and an added or a
/// deleted file is exactly its hunk.
fn fits(c: &Content, f: &FileDiff, after: bool) -> bool {
    let whole = if after { f.added() } else { f.deleted() };
    let mut cursor = 0;
    let mut total = 0;
    for h in &f.hunks {
        let start = h.start0(after);
        let side = h.side(after);
        if start < cursor || start + side.len() > c.lines.len() {
            return false;
        }
        for (k, l) in side.iter().enumerate() {
            if c.lines[start + k] != l.text {
                return false;
            }
        }
        let reaches_end = start + side.len() == c.lines.len();
        if h.at_the_end() && !reaches_end {
            return false;
        }
        let marked = side.last().is_some_and(|l| l.no_newline);
        if side.iter().rev().skip(1).any(|l| l.no_newline) || (marked && (!reaches_end || c.final_newline)) {
            return false;
        }
        if reaches_end && !side.is_empty() && !c.final_newline && !marked {
            return false;
        }
        cursor = start + side.len();
        total += side.len();
    }
    !whole || total == c.lines.len()
}

/// The other side of `c`, which fits the side `after` says: the hunks undone when
/// `c` is after the change, done when it is before.
fn other_side(c: &Content, f: &FileDiff, after: bool) -> Content {
    let mut out = Vec::new();
    let mut cursor = 0;
    let mut final_newline = c.final_newline;
    let mut reached_end = false;
    for h in &f.hunks {
        let start = h.start0(after);
        out.extend(c.lines[cursor..start].iter().cloned());
        let from = h.side(after);
        let to = h.side(!after);
        out.extend(to.iter().map(|l| l.text.clone()));
        cursor = start + from.len();
        if cursor == c.lines.len() {
            reached_end = true;
            final_newline = !to.last().is_some_and(|l| l.no_newline);
        }
    }
    out.extend(c.lines[cursor..].iter().cloned());
    if !reached_end && f.hunks.iter().any(|h| h.side(!after).iter().any(|l| l.no_newline)) {
        final_newline = false;
    }
    Content { lines: out, final_newline }
}

/// One side's blob: absent, or its hash (from an `index` line, perhaps abbreviated,
/// or computed in full).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Blob {
    Absent,
    Hash(String),
}

impl Blob {
    /// Whether a recorded hash (or a file the record does not have) is this side:
    /// an abbreviated hash matches by prefix.
    pub fn matches(&self, recorded: Option<&str>) -> bool {
        match (self, recorded) {
            (Blob::Absent, None) => true,
            (Blob::Hash(h), Some(r)) => !h.is_empty() && r.starts_with(h.as_str()),
            _ => false,
        }
    }

    /// Seven hex digits, as a person reads a blob.
    pub fn short(&self) -> String {
        match self {
            Blob::Absent => "-".into(),
            Blob::Hash(h) => h.chars().take(7).collect(),
        }
    }
}

fn from_index(h: &str) -> Blob {
    if h.chars().all(|c| c == '0') { Blob::Absent } else { Blob::Hash(h.to_ascii_lowercase()) }
}

/// The blobs of a file's two sides, before and after: from the `index` line when
/// the diff has one; else from the file on disk, which has to be one of the sides.
pub fn sides(f: &FileDiff, root: &Path) -> Result<(Blob, Blob), Text> {
    if let (Some(a), Some(b)) = (&f.old_blob, &f.new_blob) {
        return Ok((from_index(a), from_index(b)));
    }
    let read = |p: &str| std::fs::read(root.join(p)).ok().map(|b| Content::of(&b));
    let disk = read(f.path());
    if f.hunks.is_empty() {
        // a rename or a change of mode: the same bytes on both sides
        let other = f.old_path.as_deref().and_then(read);
        return match disk.or(other) {
            Some(c) => {
                let h = Blob::Hash(hash::blob(&c.bytes()));
                Ok((if f.added() { Blob::Absent } else { h.clone() }, if f.deleted() { Blob::Absent } else { h }))
            }
            None => Err(tr!(
                "{} がディスクになく、差分にもその blob がありません",
                "{} is not on disk, and the diff names no blob for it",
                f.path(),
            )),
        };
    }
    let blob = |c: &Content| Blob::Hash(hash::blob(&c.bytes()));
    let empty = Content { lines: vec![], final_newline: true };
    match disk {
        Some(c) if fits(&c, f, true) => {
            let before = if f.added() { Blob::Absent } else { blob(&other_side(&c, f, true)) };
            Ok((before, blob(&c)))
        }
        Some(c) if fits(&c, f, false) => {
            let after = if f.deleted() { Blob::Absent } else { blob(&other_side(&c, f, false)) };
            Ok((blob(&c), after))
        }
        None if f.deleted() => Ok((blob(&other_side(&empty, f, true)), Blob::Absent)),
        None if f.added() => Ok((Blob::Absent, blob(&other_side(&empty, f, false)))),
        _ => Err(tr!(
            "{} の差分が、ディスクのファイルにも、変更前のそのファイルにも合いません",
            "the diff of {} fits neither the file on disk nor that file before the change",
            f.path(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GIT: &str = "diff --git a/src/app.py b/src/app.py
index 3b18e51..a1b2c3d 100644
--- a/src/app.py
+++ b/src/app.py
@@ -2,3 +2,4 @@ def f():
 a
-b
+B
+C
 c
diff --git a/new.py b/new.py
new file mode 100644
index 0000000..e69de29
diff --git a/gone.py b/gone.py
deleted file mode 100644
index 1234567..0000000
--- a/gone.py
+++ /dev/null
@@ -1,2 +0,0 @@
-x
-y
diff --git a/old name.py b/new name.py
similarity index 90%
rename from old name.py
rename to new name.py
index 1111111..2222222 100644
--- a/old name.py
+++ b/new name.py
@@ -1 +1 @@
-p
+q
\\ No newline at end of file
diff --git a/logo.png b/logo.png
index 3333333..4444444 100644
Binary files a/logo.png and b/logo.png differ
diff --git \"a/\\346\\227\\245.py\" \"b/\\346\\227\\245.py\"
index 5555555..6666666 100644
--- \"a/\\346\\227\\245.py\"
+++ \"b/\\346\\227\\245.py\"
@@ -1 +1 @@
-x
+y
";

    #[test]
    fn a_git_diff() {
        let files = parse(GIT.as_bytes()).unwrap();
        let paths: Vec<(Option<&str>, Option<&str>)> =
            files.iter().map(|f| (f.old_path.as_deref(), f.new_path.as_deref())).collect();
        assert_eq!(
            paths,
            [
                (Some("src/app.py"), Some("src/app.py")),
                (None, Some("new.py")),
                (Some("gone.py"), None),
                (Some("old name.py"), Some("new name.py")),
                (Some("logo.png"), Some("logo.png")),
                (Some("日.py"), Some("日.py")),
            ]
        );
        let app = &files[0];
        assert_eq!((app.old_blob.as_deref(), app.new_blob.as_deref()), (Some("3b18e51"), Some("a1b2c3d")));
        let h = &app.hunks[0];
        assert_eq!((h.old_start, h.old_len, h.new_start, h.new_len), (2, 3, 2, 4));
        assert_eq!(h.lines.iter().filter(|l| l.mark == Mark::Added).count(), 2);
        assert_eq!(h.lines[2].at, 8);
        assert!(files[1].hunks.is_empty());
        assert!(files[3].hunks[0].lines[1].no_newline);
        assert!(files[4].binary);
        assert_eq!(from_index("0000000"), Blob::Absent);
    }

    #[test]
    fn a_plain_diff() {
        let text = "Only in b: extra\n\
                    diff -u a/calc.py b/calc.py\n\
                    --- calc.py.orig\t2026-10-03 01:00:00.000000000 +0900\n\
                    +++ calc.py\t2026-10-03 01:01:00.000000000 +0900\n\
                    @@ -1,2 +1,2 @@\n x\n-y\n+z\n\
                    --- /dev/null\n+++ added.py\n@@ -0,0 +1 @@\n+n\n\
                    --- empty.py\t2026\n+++ empty.py\t2026\n@@ -0,0 +1 @@\n+m\n";
        let files = parse(text.as_bytes()).unwrap();
        assert_eq!(files.len(), 3);
        assert_eq!((files[0].old_path.as_deref(), files[0].new_path.as_deref()), (Some("calc.py"), Some("calc.py")));
        assert!(files[0].old_blob.is_none());
        assert!(files[1].added() && files[2].added());
    }

    #[test]
    fn quoted_paths_on_the_git_line() {
        let q = git_line_paths;
        assert_eq!(q("\"a/x \\\"y\\\".py\" \"b/x \\\"y\\\".py\""), Some(("a/x \"y\".py".into(), "b/x \"y\".py".into())));
        assert_eq!(q("a/m.png b/m.png"), Some(("a/m.png".into(), "b/m.png".into())));
        assert_eq!(q("a/x b/y"), None);
    }

    #[test]
    fn diffs_that_do_not_read() {
        assert_eq!(parse(b"hello\n").unwrap_err().0, 0);
        assert!(parse(b"").unwrap().is_empty());
        assert_eq!(parse(b"--- a\nnot plus\n").unwrap_err().0, 2);
        assert_eq!(parse(b"--- a\n+++ a\n@@ -1,2 +1,2 @@\n x\n").unwrap_err().0, 3);
        assert_eq!(parse(b"--- a\n+++ a\n@@ -1 +1 @@\n?x\n").unwrap_err().0, 4);
        assert_eq!(parse(b"--- a\n+++ a\n@@ bad @@\n").unwrap_err().0, 3);
        assert_eq!(parse(b"@@ -1 +1 @@\n x\n").unwrap_err().0, 1);
    }

    fn diff_of(text: &str) -> FileDiff {
        parse(text.as_bytes()).unwrap().remove(0)
    }

    #[test]
    fn the_disk_after_the_change_and_before_it() {
        let f = diff_of("--- a.py\n+++ a.py\n@@ -1,3 +1,3 @@\n one\n-two\n+TWO\n three\n");
        let before = b"one\ntwo\nthree\nfour\n";
        let after = b"one\nTWO\nthree\nfour\n";
        let c = Content::of(after);
        assert!(fits(&c, &f, true) && !fits(&c, &f, false));
        assert_eq!(other_side(&c, &f, true).bytes(), before);
        let c = Content::of(before);
        assert!(fits(&c, &f, false) && !fits(&c, &f, true));
        assert_eq!(other_side(&c, &f, false).bytes(), after);
        assert!(!fits(&Content::of(b"one\nzwei\nthree\n"), &f, true));
    }

    #[test]
    fn the_newline_at_the_end() {
        // the change adds the missing final newline
        let f = diff_of("--- a\n+++ a\n@@ -1,2 +1,2 @@\n x\n-y\n\\ No newline at end of file\n+y\n");
        let after = Content::of(b"x\ny\n");
        assert!(fits(&after, &f, true));
        assert_eq!(other_side(&after, &f, true).bytes(), b"x\ny");
        let before = Content::of(b"x\ny");
        assert!(fits(&before, &f, false) && !fits(&before, &f, true));
        assert_eq!(other_side(&before, &f, false).bytes(), b"x\ny\n");
    }

    #[test]
    fn a_hunk_that_runs_into_the_end_of_the_file() {
        // the last line removed: the file after the change ends where the hunk does,
        // so a file that still has the line is the one before the change
        let f = diff_of("--- s\n+++ s\n@@ -3,4 +3,3 @@\n c\n d\n e\n-f\n");
        let before = Content::of(b"a\nb\nc\nd\ne\nf\n");
        assert!(!fits(&before, &f, true) && fits(&before, &f, false));
        assert!(fits(&Content::of(b"a\nb\nc\nd\ne\n"), &f, true));
        // as much context on both sides: nothing says where the file ends
        let g = diff_of("--- s\n+++ s\n@@ -2,3 +2,3 @@\n b\n-c\n+C\n d\n");
        assert!(fits(&Content::of(b"a\nb\nC\nd\ne\n"), &g, true));
    }

    #[test]
    fn pure_additions_and_removals() {
        // a line added after line 1, and the last line removed
        let f = diff_of("--- a\n+++ a\n@@ -1,0 +2 @@\n+new\n@@ -3 +3,0 @@\n-last\n");
        let before = b"1\n2\nlast\n";
        let after = b"1\nnew\n2\n";
        assert!(fits(&Content::of(after), &f, true));
        assert_eq!(other_side(&Content::of(after), &f, true).bytes(), before);
        assert_eq!(other_side(&Content::of(before), &f, false).bytes(), after);
    }

    #[test]
    fn added_and_deleted_files_from_the_disk() {
        let root = Path::new("/nonexistent-geas-test-root");
        let added = diff_of("--- /dev/null\n+++ n.py\n@@ -0,0 +1,2 @@\n+a\n+b\n");
        let (b, a) = sides(&added, root).unwrap();
        assert_eq!(b, Blob::Absent);
        assert_eq!(a, Blob::Hash(hash::blob(b"a\nb\n")));
        let deleted = diff_of("--- d.py\n+++ /dev/null\n@@ -1 +0,0 @@\n-gone\n");
        let (b, a) = sides(&deleted, root).unwrap();
        assert_eq!((b, a), (Blob::Hash(hash::blob(b"gone\n")), Blob::Absent));
        let changed = diff_of("--- c.py\n+++ c.py\n@@ -1 +1 @@\n-x\n+y\n");
        assert!(sides(&changed, root).is_err());
    }

    #[test]
    fn abbreviated_hashes_match_by_prefix() {
        let b = Blob::Hash("ce01362".into());
        assert!(b.matches(Some("ce013625030ba8dba906f756967f9e9ca394464a")));
        assert!(!b.matches(Some("e69de29bb2d1d6434b8b29ae775ad8c2e48c5391")));
        assert!(!b.matches(None));
        assert!(Blob::Absent.matches(None));
    }
}
