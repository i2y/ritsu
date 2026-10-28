//! The version of a module (DESIGN 17.3): SemVer 2.0.0's `MAJOR.MINOR.PATCH`, with a pre-release
//! after a `-` if it has one, and no build metadata (an OCI tag cannot hold the `+`) and no `v`
//! before it. It is the tag of the module's version in a registry, so it is at most 128
//! characters, the longest tag. Versions are put in order as SemVer's section 11 puts them.
//!
//! The numbers are kept as their digits, which have no leading zero, and compared by their length
//! and then their digits: SemVer sets no largest number, and neither does this.

use crate::text::Text;
use crate::tr;
use std::cmp::Ordering;
use std::fmt;

/// The longest version: the longest tag the distribution-spec takes.
pub const MAX_LEN: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Version {
    pub major: String,
    pub minor: String,
    pub patch: String,
    /// The identifiers after the `-`; none for a release.
    pub pre: Vec<String>,
}

/// Why a word is not a version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Empty,
    TooLong(usize),
    /// `v1.0.0`.
    LeadingV,
    /// `1.0.0+build`.
    Build,
    /// Not three numbers with dots between them.
    Shape,
    /// `01`: a number with a leading zero.
    LeadingZero(String),
    /// An identifier of the pre-release that is empty or has a character other than `0-9A-Za-z-`.
    PreRelease(String),
}

impl Error {
    pub fn text(&self, written: &str) -> Text {
        let head = tr!("`{written}` はバージョンとして読めません", "`{written}` is not a version");
        let why = match self {
            Error::Empty => tr!("バージョンが空です", "it is empty"),
            Error::TooLong(n) => tr!("{n} 字あります。バージョンは {MAX_LEN} 字までです", "it is {n} characters long, and a version is {MAX_LEN} at most"),
            Error::LeadingV => tr!("頭に `v` を付けずに書いてください", "write it with no `v` before it"),
            Error::Build => tr!("`+` のあとのビルドのメタデータは書けません。OCI のタグには `+` を使えないからです", "build metadata after a `+` is not taken, since an OCI tag cannot hold a `+`"),
            Error::Shape => tr!("`1.4.0` のように、三つの数を `.` でつないで書いてください", "write three numbers with dots between them, as `1.4.0`"),
            Error::LeadingZero(n) => tr!("数 `{n}` の頭に 0 が付いています", "the number `{n}` has a leading zero"),
            Error::PreRelease(p) => tr!(
                "プレリリースの部分 `{p}` は、英数字と `-` でできた語を `.` でつないで書いてください",
                "write the pre-release part `{p}` as words of letters, digits and `-`, with dots between them"
            ),
        };
        Text::join(&[head, why], "。", ": ")
    }
}

fn number(s: &str) -> Result<String, Error> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Error::Shape);
    }
    if s.len() > 1 && s.starts_with('0') {
        return Err(Error::LeadingZero(s.to_string()));
    }
    Ok(s.to_string())
}

impl Version {
    pub fn parse(s: &str) -> Result<Version, Error> {
        if s.is_empty() {
            return Err(Error::Empty);
        }
        let n = s.chars().count();
        if n > MAX_LEN {
            return Err(Error::TooLong(n));
        }
        if s.starts_with('v') || s.starts_with('V') {
            return Err(Error::LeadingV);
        }
        if s.contains('+') {
            return Err(Error::Build);
        }
        let (core, pre) = match s.split_once('-') {
            Some((c, p)) => (c, Some(p)),
            None => (s, None),
        };
        let parts: Vec<&str> = core.split('.').collect();
        if parts.len() != 3 {
            return Err(Error::Shape);
        }
        let (major, minor, patch) = (number(parts[0])?, number(parts[1])?, number(parts[2])?);
        let mut ids = Vec::new();
        if let Some(p) = pre {
            for id in p.split('.') {
                if id.is_empty() || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
                    return Err(Error::PreRelease(p.to_string()));
                }
                if id.bytes().all(|b| b.is_ascii_digit()) && id.len() > 1 && id.starts_with('0') {
                    return Err(Error::LeadingZero(id.to_string()));
                }
                ids.push(id.to_string());
            }
        }
        Ok(Version { major, minor, patch, pre: ids })
    }

    /// `x.y.z` from numbers.
    pub fn new(major: u64, minor: u64, patch: u64) -> Version {
        Version { major: major.to_string(), minor: minor.to_string(), patch: patch.to_string(), pre: Vec::new() }
    }

    pub fn is_prerelease(&self) -> bool {
        !self.pre.is_empty()
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if !self.pre.is_empty() {
            write!(f, "-{}", self.pre.join("."))?;
        }
        Ok(())
    }
}

/// Two numbers with no leading zero: the longer is larger, else the digits say.
fn numbers(a: &str, b: &str) -> Ordering {
    a.len().cmp(&b.len()).then_with(|| a.cmp(b))
}

fn numeric(s: &str) -> bool {
    s.bytes().all(|b| b.is_ascii_digit())
}

impl Ord for Version {
    /// SemVer's section 11: the three numbers; a release after its pre-releases; identifiers one
    /// by one, numbers by their value and below words, words in ASCII order; more identifiers after
    /// fewer that are all the same.
    fn cmp(&self, other: &Version) -> Ordering {
        numbers(&self.major, &other.major)
            .then_with(|| numbers(&self.minor, &other.minor))
            .then_with(|| numbers(&self.patch, &other.patch))
            .then_with(|| match (self.pre.is_empty(), other.pre.is_empty()) {
                (true, true) => Ordering::Equal,
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                (false, false) => {
                    for (a, b) in self.pre.iter().zip(&other.pre) {
                        let o = match (numeric(a), numeric(b)) {
                            (true, true) => numbers(a, b),
                            (true, false) => Ordering::Less,
                            (false, true) => Ordering::Greater,
                            (false, false) => a.as_bytes().cmp(b.as_bytes()),
                        };
                        if o != Ordering::Equal {
                            return o;
                        }
                    }
                    self.pre.len().cmp(&other.pre.len())
                }
            })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Version) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
