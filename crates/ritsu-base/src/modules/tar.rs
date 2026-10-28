//! The tar of a module's files (DESIGN 17.11), written one way and read in that way only.
//!
//! Written: POSIX ustar, regular files only, in the order of the bytes of their paths, no entry for
//! a directory; every entry with the mode 0644, the uid and the gid 0, an empty uname and gname,
//! the mtime 0, the magic `ustar` and a NUL, the version `00`, the device numbers 0, and an empty
//! `prefix`. A path longer than 100 bytes has a pax extended header (typeflag `x`) before it, with
//! one record, `path`; that header's name is `././@PaxHeader`, and the name of the file's own entry
//! is the path cut to 100 bytes at a boundary of UTF-8's characters (a reader reads neither, but
//! the record). Two blocks of 512 zero bytes end it, and nothing comes after them.
//!
//! Read: only that. Any typeflag other than `0` (or NUL) and an `x` with `path` alone stops it — a
//! link, a device, a directory, a global header `g` — and so does a path that is not UTF-8, is
//! absolute, has an empty part, a `.` or a `..`, a `\` or a NUL, a part that starts with `.`, or a
//! first part that starts with `@`; a path twice, a file under a file's path, and anything over the
//! limits. Unpacked, every file is opened to be made new (`create_new`): on a file system that
//! takes two names for one file (case, the normal forms of Unicode), the second stops there.

use crate::text::Text;
use crate::tr;
use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;

/// The largest file a module carries.
pub const MAX_FILE: u64 = 16 * 1024 * 1024;
/// The most files a module carries.
pub const MAX_FILES: usize = 10_000;
/// The largest tar.
pub const MAX_TAR: u64 = 64 * 1024 * 1024;
/// The longest path, in bytes.
pub const MAX_PATH: usize = 1024;
/// The most parts a path has.
pub const MAX_PARTS: usize = 32;

const BLOCK: usize = 512;
const PAX_NAME: &str = "././@PaxHeader";

/// Why a tar, or the files to be one, is refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// A path that is not one a module's tar holds: the path (lossy when it is not UTF-8), and why.
    Path(String, PathWhy),
    /// The same path twice.
    Twice(String),
    /// A file under a path that is a file (`a` and `a/b`).
    UnderFile(String, String),
    /// An entry of a type other than a file: the path, and the typeflag.
    Type(String, u8),
    /// A pax extended header with a record other than `path`, or none, or two in a row.
    Pax(String),
    /// A file larger than [`MAX_FILE`].
    FileTooLarge(String, u64),
    TooManyFiles(usize),
    TooLarge(u64),
    /// A header or the end that is not as ritsu writes it: where (a byte), and what.
    Shape(usize, Shape),
    /// The file system says no (unpacking): the path, and its words.
    Disk(String, String),
    /// The file already exists when unpacked: two entries that the file system takes as one file.
    Clash(String),
}

/// What of a header or of the end is not as ritsu writes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// The bytes end before the two blocks of zeros.
    Short,
    /// A pax extended header with no file after it.
    PaxLast,
    /// Something other than two blocks of zeros at the end, or more after them.
    End,
    Checksum,
    Magic,
    Prefix,
    Size,
    /// A size that runs past the end of the bytes.
    PastEnd,
}

impl Shape {
    fn text(self) -> Text {
        match self {
            Shape::Short => tr!("二つの 0 の塊の前で終わっています", "it ends before its two blocks of zeros"),
            Shape::PaxLast => tr!("pax の拡張ヘッダのあとにファイルがありません", "a pax extended header has no file after it"),
            Shape::End => tr!("終わりが、二つの 0 の塊だけではありません", "the end is not two blocks of zeros and nothing after them"),
            Shape::Checksum => tr!("チェックサムがヘッダの和と違います", "the checksum is not the sum of the header"),
            Shape::Magic => tr!("magic が `ustar` と NUL、version が `00` ではありません", "the magic is not `ustar` and a NUL, or the version not `00`"),
            Shape::Prefix => tr!("prefix のフィールドが空ではありません", "the field prefix is not empty"),
            Shape::Size => tr!("大きさが 8 進の数ではありません", "the size is not an octal number"),
            Shape::PastEnd => tr!("大きさが tar の終わりを越えます", "the size runs past the end of the tar"),
        }
    }
}

/// What is wrong with a path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathWhy {
    NotUtf8,
    Empty,
    Absolute,
    EmptyPart,
    Dot,
    DotDot,
    Backslash,
    Nul,
    DotStart,
    AtStart,
    TooLong,
    TooDeep,
}

impl Error {
    pub fn text(&self) -> Text {
        match self {
            Error::Path(p, why) => {
                let w = match why {
                    PathWhy::NotUtf8 => tr!("UTF-8 ではありません", "it is not UTF-8"),
                    PathWhy::Empty => tr!("空です", "it is empty"),
                    PathWhy::Absolute => tr!("絶対パスです", "it is absolute"),
                    PathWhy::EmptyPart => tr!("空の段があります", "it has an empty part"),
                    PathWhy::Dot => tr!("`.` の段があります", "it has a part `.`"),
                    PathWhy::DotDot => tr!("`..` の段があります", "it has a part `..`"),
                    PathWhy::Backslash => tr!("`\\` があります", "it has a `\\` in it"),
                    PathWhy::Nul => tr!("NUL があります", "it has a NUL in it"),
                    PathWhy::DotStart => tr!("`.` で始まる段があります", "it has a part that starts with `.`"),
                    PathWhy::AtStart => tr!("最初の段が `@` で始まります", "its first part starts with `@`"),
                    PathWhy::TooLong => tr!("{MAX_PATH} バイトを超えます", "it is longer than {MAX_PATH} bytes"),
                    PathWhy::TooDeep => tr!("段が {MAX_PARTS} を超えます", "it has more than {MAX_PARTS} parts"),
                };
                tr!("モジュールのファイルのパス `{p}` は受け付けません。", "the path `{p}` of a module's file is refused: ").then(&w)
            }
            Error::Twice(p) => tr!("パス `{p}` が二度あります", "the path `{p}` is there twice"),
            Error::UnderFile(f, p) => tr!("ファイル `{f}` の下に `{p}` があります", "`{p}` is under `{f}`, which is a file"),
            Error::Type(p, t) => {
                let t = if t.is_ascii_graphic() { format!("`{}`", *t as char) } else { format!("0x{t:02x}") };
                tr!(
                    "`{p}` の種類 {t} は受け付けません。モジュールの tar が持つのは、ふつうのファイルだけです",
                    "the type {t} of `{p}` is refused: a module's tar holds files only"
                )
            }
            Error::Pax(p) => tr!(
                "`{p}` の pax の拡張ヘッダは受け付けません。持てるのは `path` の記録一つだけです",
                "the pax extended header of `{p}` is refused: it holds one record, `path`, and nothing else"
            ),
            Error::FileTooLarge(p, n) => {
                let max = MAX_FILE;
                tr!("`{p}` は {n} バイトあります。一つのファイルは {max} バイトまでです", "`{p}` is {n} bytes, and a file is {max} bytes at most")
            }
            Error::TooManyFiles(n) => tr!("ファイルが {n} あります。モジュールのファイルは {MAX_FILES} までです", "there are {n} files, and a module has {MAX_FILES} at most"),
            Error::TooLarge(n) => {
                let max = MAX_TAR;
                tr!("tar が {n} バイトになります。{max} バイトまでです", "the tar is {n} bytes, and it is {max} bytes at most")
            }
            Error::Shape(at, what) => {
                let w = what.text();
                tr!("tar の {at} バイト目が、ritsu の書く形ではありません。{}", "the tar is not as ritsu writes one at byte {at}: {}", w.ja; w.en)
            }
            Error::Disk(p, e) => tr!("`{p}` を書けません（{e}）", "cannot write `{p}` ({e})"),
            Error::Clash(p) => tr!(
                "`{p}` は、tar のほかの項目とファイルシステムの上で同じファイルになります（大文字と小文字、Unicode の正規化など）",
                "`{p}` is the same file as another entry of the tar on this file system (by case, or by the normal forms of Unicode)"
            ),
        }
    }
}

/// What is wrong with a path a module's tar would hold, if anything.
pub fn path_why(p: &str) -> Option<PathWhy> {
    if p.is_empty() {
        return Some(PathWhy::Empty);
    }
    if p.len() > MAX_PATH {
        return Some(PathWhy::TooLong);
    }
    if p.contains('\0') {
        return Some(PathWhy::Nul);
    }
    if p.contains('\\') {
        return Some(PathWhy::Backslash);
    }
    if p.starts_with('/') || crate::paths::is_absolute(p) {
        return Some(PathWhy::Absolute);
    }
    let parts: Vec<&str> = p.split('/').collect();
    if parts.len() > MAX_PARTS {
        return Some(PathWhy::TooDeep);
    }
    for (i, part) in parts.iter().enumerate() {
        match *part {
            "" => return Some(PathWhy::EmptyPart),
            "." => return Some(PathWhy::Dot),
            ".." => return Some(PathWhy::DotDot),
            _ if part.starts_with('.') => return Some(PathWhy::DotStart),
            _ if i == 0 && part.starts_with('@') => return Some(PathWhy::AtStart),
            _ => {}
        }
    }
    None
}

/// The checks of a set of files that do not need their bytes: each path, a path twice, a file under
/// a file, how many. `files` is in the order of the bytes of their paths.
fn check_set(paths: &[&str]) -> Result<(), Error> {
    if paths.len() > MAX_FILES {
        return Err(Error::TooManyFiles(paths.len()));
    }
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for p in paths {
        if let Some(why) = path_why(p) {
            return Err(Error::Path(p.to_string(), why));
        }
        if !seen.insert(p) {
            return Err(Error::Twice(p.to_string()));
        }
    }
    for p in paths {
        let mut at = 0;
        while let Some(i) = p[at..].find('/') {
            let head = &p[..at + i];
            if seen.contains(head) {
                return Err(Error::UnderFile(head.to_string(), p.to_string()));
            }
            at += i + 1;
        }
    }
    Ok(())
}

/// The tar of `files` (any order; it is written in the order of the bytes of the paths).
pub fn write(files: &[(String, Vec<u8>)]) -> Result<Vec<u8>, Error> {
    let mut fs: Vec<&(String, Vec<u8>)> = files.iter().collect();
    fs.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    let paths: Vec<&str> = fs.iter().map(|(p, _)| p.as_str()).collect();
    check_set(&paths)?;
    let mut out: Vec<u8> = Vec::new();
    for (p, bytes) in fs {
        if bytes.len() as u64 > MAX_FILE {
            return Err(Error::FileTooLarge(p.clone(), bytes.len() as u64));
        }
        if p.len() > 100 {
            let rec = pax_record("path", p);
            out.extend(header(PAX_NAME.as_bytes(), rec.len() as u64, b'x'));
            out.extend(&rec);
            pad(&mut out);
        }
        out.extend(header(cut(p, 100).as_bytes(), bytes.len() as u64, b'0'));
        out.extend(bytes);
        pad(&mut out);
        if out.len() as u64 > MAX_TAR {
            return Err(Error::TooLarge(out.len() as u64 + 2 * BLOCK as u64));
        }
    }
    out.extend([0u8; 2 * BLOCK]);
    if out.len() as u64 > MAX_TAR {
        return Err(Error::TooLarge(out.len() as u64));
    }
    Ok(out)
}

/// `p` cut to at most `n` bytes at a boundary of its characters.
fn cut(p: &str, n: usize) -> &str {
    if p.len() <= n {
        return p;
    }
    let mut end = n;
    while !p.is_char_boundary(end) {
        end -= 1;
    }
    &p[..end]
}

/// `<length> <key>=<value>\n`, the length counting itself.
fn pax_record(key: &str, value: &str) -> Vec<u8> {
    let body = format!(" {key}={value}\n");
    let mut n = body.len() + 1;
    loop {
        let len = n.to_string().len() + body.len();
        if len == n {
            break;
        }
        n = len;
    }
    format!("{n}{body}").into_bytes()
}

fn pad(out: &mut Vec<u8>) {
    let r = out.len() % BLOCK;
    if r != 0 {
        out.extend(std::iter::repeat_n(0u8, BLOCK - r));
    }
}

/// `n` in octal, `width - 1` digits and a NUL.
fn octal(n: u64, width: usize) -> Vec<u8> {
    let mut s = format!("{n:0w$o}", w = width - 1).into_bytes();
    s.push(0);
    s
}

/// One header block as ritsu writes it.
fn header(name: &[u8], size: u64, typeflag: u8) -> [u8; BLOCK] {
    let mut h = [0u8; BLOCK];
    h[..name.len()].copy_from_slice(name);
    h[100..108].copy_from_slice(&octal(0o644, 8));
    h[108..116].copy_from_slice(&octal(0, 8));
    h[116..124].copy_from_slice(&octal(0, 8));
    h[124..136].copy_from_slice(&octal(size, 12));
    h[136..148].copy_from_slice(&octal(0, 12));
    h[156] = typeflag;
    h[257..263].copy_from_slice(b"ustar\0");
    h[263..265].copy_from_slice(b"00");
    h[329..337].copy_from_slice(&octal(0, 8));
    h[337..345].copy_from_slice(&octal(0, 8));
    // the checksum: the sum of the bytes with its own field as spaces, six octal digits, a NUL and a space
    h[148..156].copy_from_slice(b"        ");
    let sum: u32 = h.iter().map(|b| u32::from(*b)).sum();
    let mut c = format!("{sum:06o}").into_bytes();
    c.push(0);
    c.push(b' ');
    h[148..156].copy_from_slice(&c);
    h
}

/// An octal field: digits, then NULs or spaces.
fn read_octal(f: &[u8]) -> Option<u64> {
    let end = f.iter().position(|b| *b == 0 || *b == b' ').unwrap_or(f.len());
    if end == 0 || !f[end..].iter().all(|b| *b == 0 || *b == b' ') {
        return None;
    }
    let s = std::str::from_utf8(&f[..end]).ok()?;
    u64::from_str_radix(s, 8).ok()
}

/// The files of a tar ritsu wrote, by their paths, in the order they are in it.
pub fn read(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, Error> {
    if bytes.len() as u64 > MAX_TAR {
        return Err(Error::TooLarge(bytes.len() as u64));
    }
    let mut out: Vec<(String, Vec<u8>)> = Vec::new();
    let mut at = 0;
    let mut pending: Option<String> = None;
    loop {
        if at + BLOCK > bytes.len() {
            return Err(Error::Shape(at, Shape::Short));
        }
        let h = &bytes[at..at + BLOCK];
        if h.iter().all(|b| *b == 0) {
            if pending.is_some() {
                return Err(Error::Shape(at, Shape::PaxLast));
            }
            // the end: two blocks of zeros, and nothing after them
            if bytes.len() != at + 2 * BLOCK || !bytes[at..].iter().all(|b| *b == 0) {
                return Err(Error::Shape(at, Shape::End));
            }
            break;
        }
        // the checksum, its own field taken as spaces
        let want = read_octal(&h[148..156]).ok_or(Error::Shape(at + 148, Shape::Checksum))?;
        let sum: u64 = h.iter().enumerate().map(|(i, b)| if (148..156).contains(&i) { 32 } else { u64::from(*b) }).sum();
        if want != sum {
            return Err(Error::Shape(at + 148, Shape::Checksum));
        }
        if &h[257..263] != b"ustar\0" || &h[263..265] != b"00" {
            return Err(Error::Shape(at + 257, Shape::Magic));
        }
        if h[345..500].iter().any(|b| *b != 0) {
            return Err(Error::Shape(at + 345, Shape::Prefix));
        }
        let size = read_octal(&h[124..136]).ok_or(Error::Shape(at + 124, Shape::Size))?;
        let name_end = h[..100].iter().position(|b| *b == 0).unwrap_or(100);
        let name = String::from_utf8_lossy(&h[..name_end]).into_owned();
        let body_at = at + BLOCK;
        let padded = size.div_ceil(BLOCK as u64) * BLOCK as u64;
        if body_at as u64 + padded > bytes.len() as u64 {
            return Err(Error::Shape(at + 124, Shape::PastEnd));
        }
        let body = &bytes[body_at..body_at + size as usize];
        let typeflag = h[156];
        match typeflag {
            b'x' => {
                if pending.is_some() {
                    return Err(Error::Pax(name));
                }
                pending = Some(pax_path(body).ok_or_else(|| Error::Pax(name.clone()))?);
            }
            b'0' | 0 => {
                let path = match pending.take() {
                    Some(p) => p,
                    None => match std::str::from_utf8(&h[..name_end]) {
                        Ok(s) => s.to_string(),
                        Err(_) => return Err(Error::Path(name, PathWhy::NotUtf8)),
                    },
                };
                if let Some(why) = path_why(&path) {
                    return Err(Error::Path(path, why));
                }
                if size > MAX_FILE {
                    return Err(Error::FileTooLarge(path, size));
                }
                out.push((path, body.to_vec()));
                if out.len() > MAX_FILES {
                    return Err(Error::TooManyFiles(out.len()));
                }
            }
            t => {
                let p = pending.take().unwrap_or(name);
                return Err(Error::Type(p, t));
            }
        }
        at = body_at + padded as usize;
    }
    let mut sorted: Vec<&str> = out.iter().map(|(p, _)| p.as_str()).collect();
    sorted.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
    check_set(&sorted)?;
    Ok(out)
}

/// The path of a pax extended header that holds one record, `path`, and nothing else.
fn pax_path(body: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(body).ok()?;
    let (len, rest) = text.split_once(' ')?;
    let n: usize = len.parse().ok()?;
    if n != text.len() || !text.ends_with('\n') || len.starts_with('0') {
        return None;
    }
    let value = rest.strip_prefix("path=")?.strip_suffix('\n')?;
    Some(value.to_string())
}

/// The files written under `dir`, which is there and empty, each opened to be made new. A
/// directory the file system already has when it is made (two entries the file system takes as
/// one) stops it, as a file does.
pub fn unpack(files: &[(String, Vec<u8>)], dir: &Path) -> Result<(), Error> {
    let mut made: BTreeSet<String> = BTreeSet::new();
    for (p, bytes) in files {
        if let Some(why) = path_why(p) {
            return Err(Error::Path(p.clone(), why));
        }
        let parts: Vec<&str> = p.split('/').collect();
        for k in 1..parts.len() {
            let d = parts[..k].join("/");
            if made.contains(&d) {
                continue;
            }
            match std::fs::create_dir(dir.join(&d)) {
                Ok(()) => {
                    made.insert(d);
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return Err(Error::Clash(d)),
                Err(e) => return Err(Error::Disk(d, e.to_string())),
            }
        }
        let mut f = match std::fs::OpenOptions::new().write(true).create_new(true).open(dir.join(p)) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return Err(Error::Clash(p.clone())),
            Err(e) => return Err(Error::Disk(p.clone(), e.to_string())),
        };
        f.write_all(bytes).map_err(|e| Error::Disk(p.clone(), e.to_string()))?;
    }
    Ok(())
}
