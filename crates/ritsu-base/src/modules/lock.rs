//! `ritsu.lock` (DESIGN 17.4): the module of every node of the tree but the module at hand, with
//! where it comes from, and for one from a registry the version chosen and its two digests. ritsu
//! writes it whole, in one way; a person does not.
//!
//! ```text
//! # Written by ritsu mod. Run `ritsu mod lock` to write it again; do not edit it by hand.
//! lock 1
//! ritsu 0.37.0
//! oci example.com/acme/pricing 1.1.0 sha256:<64 hex> files sha256:<64 hex>
//! path example.com/acme/tax "../tax"
//! ```
//!
//! A lock that cannot be read is E303; one whose format is a number newer than `1` was written by
//! a newer ritsu, and reading it stops ([`Error::Newer`]).

use super::digest;
use super::lex::{self, Kind};
use super::names::ModuleName;
use super::semver::Version;
use super::{Diag, E303};
use crate::text::Text;
use crate::tr;

/// The lock's file name, beside `ritsu.mod`.
pub const FILE: &str = "ritsu.lock";

/// The format this ritsu reads and writes.
pub const FORMAT: u32 = 1;

/// The line ritsu writes first, in English whatever the language (the head of generated code is
/// the same).
pub const HEAD: &str = "# Written by ritsu mod. Run `ritsu mod lock` to write it again; do not edit it by hand.";

/// Where a module of the lock comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pinned {
    /// The version chosen, the digest of its OCI manifest, and the digest of its files (the tar).
    Oci { version: Version, digest: String, files: String },
    /// A directory, from the root of the module at hand.
    Path { path: String },
}

/// One line of the lock.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Locked {
    pub name: ModuleName,
    pub pinned: Pinned,
    /// The line it was read from; 0 for one made in memory.
    pub line: usize,
}

impl Locked {
    /// The line as the lock writes it.
    pub fn text(&self) -> String {
        match &self.pinned {
            Pinned::Oci { version, digest, files } => format!("oci {} {version} {digest} files {files}", self.name),
            Pinned::Path { path } => format!("path {} {}", self.name, lex::quote(path)),
        }
    }

    pub fn version(&self) -> Option<&Version> {
        match &self.pinned {
            Pinned::Oci { version, .. } => Some(version),
            Pinned::Path { .. } => None,
        }
    }
}

/// A `ritsu.lock` read, or made to be written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lock {
    /// The ritsu that wrote it: a record, which no run compares with itself.
    pub ritsu: Version,
    pub modules: Vec<Locked>,
}

/// Why a lock cannot be read.
#[derive(Clone, Debug)]
pub enum Error {
    /// E303, each place it is.
    Unreadable(Vec<Diag>),
    /// `lock <n>` with a number newer than [`FORMAT`]: a newer ritsu wrote it (exit 2).
    Newer(u64),
}

impl Error {
    /// The text of a stop ([`Error::Newer`]); the first diagnostic's message for the other.
    pub fn text(&self) -> Text {
        match self {
            Error::Unreadable(ds) => ds.first().map(|d| d.message.clone()).unwrap_or_default(),
            Error::Newer(n) => tr!(
                "`ritsu.lock` の書式は `lock {n}` で、この ritsu が読めるのは `lock {FORMAT}` までです。新しい ritsu が書いた lock なので、ritsu を新しくしてください",
                "`ritsu.lock` is in the format `lock {n}`, and this ritsu reads `lock {FORMAT}` at most: a newer ritsu wrote it, so run a newer ritsu"
            ),
        }
    }
}

impl Lock {
    /// A lock of these modules, written by this ritsu.
    pub fn new(modules: Vec<Locked>) -> Lock {
        Lock { ritsu: Version::parse(env!("CARGO_PKG_VERSION")).unwrap_or_else(|_| Version::new(0, 0, 0)), modules }
    }

    /// The line of a module.
    pub fn get(&self, name: &ModuleName) -> Option<&Locked> {
        self.modules.iter().find(|m| &m.name == name)
    }

    /// The text ritsu writes: the head, the format, the ritsu that wrote it, and a line a module in
    /// the order of the bytes of their names, each line ending with `\n`.
    pub fn text(&self) -> String {
        let mut ms: Vec<&Locked> = self.modules.iter().collect();
        ms.sort_by(|a, b| a.name.text().as_bytes().cmp(b.name.text().as_bytes()));
        let mut out = format!("{HEAD}\nlock {FORMAT}\nritsu {}\n", self.ritsu);
        for m in ms {
            out.push_str(&m.text());
            out.push('\n');
        }
        out
    }
}

/// `bytes`, read as the `ritsu.lock` at `file` (a path from the root, as diagnostics write it).
pub fn parse(bytes: &[u8], file: &str) -> Result<Lock, Error> {
    let mut diags: Vec<Diag> = Vec::new();
    let mut put = |line: usize, col: usize, src: &str, message: Text| {
        let mut d = Diag::at(E303, file, line, col, message).rel(file);
        if line > 0 {
            d = d.source(&format!("{}{src}", "\n".repeat(line - 1)));
        }
        diags.push(d);
    };
    let text = match std::str::from_utf8(bytes) {
        Ok(t) => t,
        Err(e) => {
            let line = bytes[..e.valid_up_to()].iter().filter(|b| **b == b'\n').count() + 1;
            put(line, 0, "", tr!("UTF-8 として読めないバイトがあります", "there are bytes that are not UTF-8"));
            return Err(Error::Unreadable(diags));
        }
    };
    let mut format: Option<usize> = None;
    let mut ritsu: Option<Version> = None;
    let mut modules: Vec<Locked> = Vec::new();
    for (i, (src, _)) in lex::split(text).into_iter().enumerate() {
        let n = i + 1;
        let lexed = match lex::line(src) {
            Ok(l) => l,
            Err(e) => {
                put(n, e.col(), src, e.text());
                continue;
            }
        };
        let ts = &lexed.tokens;
        let Some(head) = ts.first() else { continue };
        let words: Vec<&str> = ts.iter().map(|t| t.text.as_str()).collect();
        let kinds_ok = |want: &[Kind]| ts.len() == want.len() && ts.iter().zip(want).all(|(t, k)| t.kind == *k);
        if format.is_none() {
            // the first statement is the format
            if !(head.is_word("lock") && ts.len() == 2 && ts[1].kind == Kind::Word) {
                put(n, head.col, src, tr!("`ritsu.lock` は `lock {FORMAT}` の行で始まります", "`ritsu.lock` starts with the line `lock {FORMAT}`"));
                return Err(Error::Unreadable(diags));
            }
            let w = words[1];
            match w.parse::<u64>() {
                Ok(v) if w.len() == 1 || !w.starts_with('0') => {
                    if v > u64::from(FORMAT) {
                        return Err(Error::Newer(v));
                    }
                    if v != u64::from(FORMAT) {
                        put(n, ts[1].col, src, tr!("書式 `lock {w}` はありません。`lock {FORMAT}` です", "there is no format `lock {w}`; it is `lock {FORMAT}`"));
                        return Err(Error::Unreadable(diags));
                    }
                    format = Some(n);
                }
                _ => {
                    put(n, ts[1].col, src, tr!("書式 `lock {w}` は数ではありません", "the format `lock {w}` is not a number"));
                    return Err(Error::Unreadable(diags));
                }
            }
            continue;
        }
        if ritsu.is_none() {
            if !(head.is_word("ritsu") && kinds_ok(&[Kind::Word, Kind::Word])) {
                put(n, head.col, src, tr!("`lock {FORMAT}` の次の行は `ritsu <版>` です", "the line after `lock {FORMAT}` is `ritsu <version>`"));
                return Err(Error::Unreadable(diags));
            }
            match Version::parse(words[1]) {
                Ok(v) => ritsu = Some(v),
                Err(e) => {
                    put(n, ts[1].col, src, e.text(words[1]));
                    return Err(Error::Unreadable(diags));
                }
            }
            continue;
        }
        let shape_oci = [Kind::Word, Kind::Word, Kind::Word, Kind::Word, Kind::Word, Kind::Word];
        let line = match words[0] {
            "oci" if kinds_ok(&shape_oci) && words[4] == "files" => {
                let name = match ModuleName::parse(words[1]) {
                    Ok(m) if m.is_published() => m,
                    _ => {
                        put(n, ts[1].col, src, tr!("`{}` はレジストリのモジュールの名前として読めません", "`{}` is not the name of a module in a registry", words[1]));
                        continue;
                    }
                };
                let version = match Version::parse(words[2]) {
                    Ok(v) => v,
                    Err(e) => {
                        put(n, ts[2].col, src, e.text(words[2]));
                        continue;
                    }
                };
                if let Some(k) = [3usize, 5].into_iter().find(|k| !digest::is_digest(words[*k])) {
                    let w = words[k];
                    put(n, ts[k].col, src, tr!("`{w}` はダイジェスト（`sha256:` と 64 桁の 16 進の小文字）ではありません", "`{w}` is not a digest (`sha256:` and 64 lowercase hex digits)"));
                    continue;
                }
                Locked { name, pinned: Pinned::Oci { version, digest: words[3].to_string(), files: words[5].to_string() }, line: n }
            }
            "path" if kinds_ok(&[Kind::Word, Kind::Word, Kind::Str]) => {
                let name = match ModuleName::parse(words[1]) {
                    Ok(m) => m,
                    Err(e) => {
                        put(n, ts[1].col, src, tr!("`{}` はモジュールの名前として読めません", "`{}` is not a module's name", words[1]).then(&Text::new(format!("。{}", e.text().ja), format!(": {}", e.text().en))));
                        continue;
                    }
                };
                let p = &ts[2].text;
                if p.is_empty() || crate::paths::is_absolute(p) || p.contains('\\') {
                    put(n, ts[2].col, src, tr!("`{p}` は、モジュールのルートからの相対パスではありません", "`{p}` is not a path from the root of the module"));
                    continue;
                }
                Locked { name, pinned: Pinned::Path { path: p.clone() }, line: n }
            }
            _ => {
                put(n, head.col, src, tr!(
                    "この行は読めません。`ritsu.lock` の行は `oci <名前> <版> <ダイジェスト> files <ダイジェスト>` か `path <名前> \"<パス>\"` です",
                    "this line cannot be read: a line of `ritsu.lock` is `oci <name> <version> <digest> files <digest>` or `path <name> \"<path>\"`"
                ));
                continue;
            }
        };
        if let Some(other) = modules.iter().find(|m| m.name == line.name) {
            let (name, l) = (line.name.to_string(), other.line);
            put(n, ts[1].col, src, tr!("`{name}` の行は、{l} 行目にもうあります", "`{name}` already has a line, line {l}"));
            continue;
        }
        modules.push(line);
    }
    if format.is_none() {
        put(0, 0, "", tr!("`ritsu.lock` に `lock {FORMAT}` の行がありません", "`ritsu.lock` has no line `lock {FORMAT}`"));
    } else if ritsu.is_none() {
        put(0, 0, "", tr!("`ritsu.lock` に `ritsu <版>` の行がありません", "`ritsu.lock` has no line `ritsu <version>`"));
    }
    match (diags.is_empty(), ritsu) {
        (true, Some(ritsu)) => Ok(Lock { ritsu, modules }),
        _ => Err(Error::Unreadable(diags)),
    }
}
