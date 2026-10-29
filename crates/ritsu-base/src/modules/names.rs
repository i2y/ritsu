//! The names of `ritsu.mod` (DESIGN 17.3): the name of a module, the name of a dependency, the
//! start of the module names a `signature` line covers, and the name of a library in the registry
//! of its target.
//!
//! A module's name has two forms. One that is published is `<registry>/<repository>`: the host of a
//! registry (lowercase letters, digits, `-` and `.`, with a `.` in it or `localhost`, and a `:port`
//! if it has one) and a repository as the distribution-spec writes `<name>`, 255 characters at most
//! in all. One that is not is a short name, `[a-z][a-z0-9_-]*`, 64 characters at most, for a module
//! a repository uses by its path and does not publish. A name with a `/` in it is the first form.

use crate::text::Text;
use crate::tr;

/// The longest name of a module that is published: the registry, `/` and the repository.
pub const MAX_PUBLISHED: usize = 255;
/// The longest name of a module that is not, and of a dependency.
pub const MAX_SHORT: usize = 64;

/// A module's name, of either form.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ModuleName {
    /// `example.com/acme/pricing`: the registry (`example.com`, `127.0.0.1:5000`) and the
    /// repository in it (`acme/pricing`).
    Published { registry: String, repository: String },
    /// `pricing`: a module used by its path only.
    Local(String),
}

impl ModuleName {
    pub fn parse(s: &str) -> Result<ModuleName, NameError> {
        if s.contains('/') {
            let n = s.chars().count();
            if n > MAX_PUBLISHED {
                return Err(NameError::TooLong { len: n, max: MAX_PUBLISHED });
            }
            let (registry, repository) = s.split_once('/').unwrap_or((s, ""));
            registry_ok(registry)?;
            repository_ok(repository)?;
            Ok(ModuleName::Published { registry: registry.to_string(), repository: repository.to_string() })
        } else {
            short_ok(s, true)?;
            Ok(ModuleName::Local(s.to_string()))
        }
    }

    pub fn is_published(&self) -> bool {
        matches!(self, ModuleName::Published { .. })
    }

    /// The name as written.
    pub fn text(&self) -> String {
        match self {
            ModuleName::Published { registry, repository } => format!("{registry}/{repository}"),
            ModuleName::Local(s) => s.clone(),
        }
    }
}

impl std::fmt::Display for ModuleName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text())
    }
}

/// Why a name is not of its form.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NameError {
    Empty,
    TooLong { len: usize, max: usize },
    /// A character the form does not take, and where it is (a character, from 0).
    Character(char, usize),
    /// A short name that does not start with a lowercase letter.
    Start,
    /// A registry's host with neither a `.` nor the name `localhost`.
    ShortHost(String),
    /// A host whose labels are empty, or start or end with a `-`.
    Host(String),
    Port(String),
    /// A repository that is not the distribution-spec's `<name>`.
    Repository(String),
    /// A name of a dependency with a `-`.
    Dash,
    /// A short name where only a published one is taken (the start of a `signature` line).
    NotPublished,
}

impl NameError {
    /// What is wrong, after the sentence that says which name it is.
    pub fn text(&self) -> Text {
        match self {
            NameError::Empty => tr!("名前が空です", "it is empty"),
            NameError::TooLong { len, max } => tr!("{len} 字あります。{max} 字までです", "it is {len} characters long, and the most is {max}"),
            NameError::Character(c, at) => {
                let at = at + 1;
                tr!("{at} 字目の `{c}` は使えません", "the `{c}` at character {at} is not taken")
            }
            NameError::Start => tr!("英小文字で始めてください", "start it with a lowercase letter"),
            NameError::ShortHost(h) => tr!(
                "レジストリのホスト `{h}` に `.` がありません。ホストは `.` を含むか `localhost` です（短い名前と取り違えないため）",
                "the registry's host `{h}` has no `.` in it; a host has one, or is `localhost`, so that it is not taken for a short name"
            ),
            NameError::Host(h) => tr!("レジストリのホスト `{h}` は、ホスト名として読めません", "the registry's host `{h}` is not a host name"),
            NameError::Port(p) => tr!("ポート `{p}` は 1 から 65535 の数で書いてください", "the port `{p}` is a number from 1 to 65535"),
            NameError::Repository(r) => tr!(
                "リポジトリ `{r}` は、英小文字と数字を `.`、`_`、`__`、`-` でつないだ語を、`/` で区切って書いてください",
                "the repository `{r}` is words of lowercase letters and digits joined by `.`, `_`, `__` or `-`, with `/` between them"
            ),
            NameError::Dash => tr!("`-` は使えません。語を区切るときは `_` を使ってください", "a `-` is not taken; join words with `_`"),
            NameError::NotPublished => tr!("配らないモジュールの名前です。署名を求められるのは、レジストリに配るモジュールだけです", "it is the name of a module that is not published, and only one published to a registry has a signature to ask for"),
        }
    }
}

/// `[a-z][a-z0-9_-]*` (a module's short name, `dash` true) or `[a-z][a-z0-9_]*` (a dependency's),
/// 64 characters at most.
fn short_ok(s: &str, dash: bool) -> Result<(), NameError> {
    if s.is_empty() {
        return Err(NameError::Empty);
    }
    let n = s.chars().count();
    if n > MAX_SHORT {
        return Err(NameError::TooLong { len: n, max: MAX_SHORT });
    }
    for (i, c) in s.chars().enumerate() {
        let ok = c.is_ascii_lowercase() || (i > 0 && (c.is_ascii_digit() || c == '_' || (dash && c == '-')));
        if !ok {
            return Err(if i == 0 && (c.is_ascii_digit() || c == '_' || c == '-') {
                NameError::Start
            } else if c == '-' {
                NameError::Dash
            } else {
                NameError::Character(c, i)
            });
        }
    }
    Ok(())
}

/// A name of a dependency (DESIGN 17.3): `[a-z][a-z0-9_]*`, 64 characters at most. It is written
/// `@<name>/` on a command line, and is the name of a Python package and of a directory of Go.
pub fn dependency_ok(s: &str) -> Result<(), NameError> {
    short_ok(s, false)
}

/// The registry of a published name: a host, and a `:port` if it has one.
pub fn registry_ok(s: &str) -> Result<(), NameError> {
    if s.is_empty() {
        return Err(NameError::Empty);
    }
    let (host, port) = match s.split_once(':') {
        Some((h, p)) => (h, Some(p)),
        None => (s, None),
    };
    for (i, c) in host.chars().enumerate() {
        if !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '.') {
            return Err(NameError::Character(c, i));
        }
    }
    if host.split('.').any(|l| l.is_empty() || l.starts_with('-') || l.ends_with('-')) {
        return Err(NameError::Host(host.to_string()));
    }
    if !host.contains('.') && host != "localhost" {
        return Err(NameError::ShortHost(host.to_string()));
    }
    if let Some(p) = port {
        let ok = !p.is_empty() && p.len() <= 5 && p.bytes().all(|b| b.is_ascii_digit()) && !p.starts_with('0') && p.parse::<u32>().is_ok_and(|n| (1..=65535).contains(&n));
        if !ok {
            return Err(NameError::Port(p.to_string()));
        }
    }
    Ok(())
}

/// A repository as the distribution-spec writes `<name>`:
/// `[a-z0-9]+((\.|_|__|-+)[a-z0-9]+)*(\/[a-z0-9]+((\.|_|__|-+)[a-z0-9]+)*)*`.
pub fn repository_ok(s: &str) -> Result<(), NameError> {
    let bad = || NameError::Repository(s.to_string());
    if s.is_empty() {
        return Err(NameError::Empty);
    }
    for part in s.split('/') {
        let b = part.as_bytes();
        let alnum = |c: u8| c.is_ascii_lowercase() || c.is_ascii_digit();
        let mut i = 0;
        // a run of letters and digits, then a separator and another run, and so on
        loop {
            let start = i;
            while i < b.len() && alnum(b[i]) {
                i += 1;
            }
            if i == start {
                return Err(bad());
            }
            if i == b.len() {
                break;
            }
            match b[i] {
                b'.' => i += 1,
                b'_' => i += if b.get(i + 1) == Some(&b'_') { 2 } else { 1 },
                b'-' => {
                    while i < b.len() && b[i] == b'-' {
                        i += 1;
                    }
                }
                _ => return Err(bad()),
            }
        }
    }
    Ok(())
}

/// The start of the names a `signature` line covers (DESIGN 17.3): a module's published name, or a
/// start of one that ends with `/` (`"example.com/acme/"`, `"example.com/"`).
pub fn prefix_ok(s: &str) -> Result<(), NameError> {
    match s.strip_suffix('/') {
        Some(head) => match head.split_once('/') {
            Some((registry, repository)) => {
                registry_ok(registry)?;
                repository_ok(repository)
            }
            None => registry_ok(head),
        },
        None => ModuleName::parse(s).and_then(|m| if m.is_published() { Ok(()) } else { Err(NameError::NotPublished) }),
    }
}

/// Whether a `signature` line whose start is `prefix` covers the module `name`.
pub fn covers(prefix: &str, name: &str) -> bool {
    if prefix.ends_with('/') { name.starts_with(prefix) } else { name == prefix }
}

/// The targets a module is published to as a library (DESIGN 17.16).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Target {
    Typescript,
    Python,
    Go,
}

impl Target {
    pub const ALL: [Target; 3] = [Target::Typescript, Target::Python, Target::Go];

    pub fn word(self) -> &'static str {
        match self {
            Target::Typescript => "typescript",
            Target::Python => "python",
            Target::Go => "go",
        }
    }

    pub fn from_word(w: &str) -> Option<Target> {
        Target::ALL.into_iter().find(|t| t.word() == w)
    }
}

/// Why a library's name is not one its registry takes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibraryError(pub Text);

/// A library's name as its target's registry takes it: an npm package (`name` or `@scope/name`), a
/// Python distribution (PEP 508's name), a Go module path with no `/vN` at its end (ritsu adds it
/// from version 2 on, DESIGN 17.16).
pub fn library_ok(t: Target, s: &str) -> Result<(), LibraryError> {
    let fail = |ja: &str, en: &str| Err(LibraryError(Text::new(ja, en)));
    match t {
        Target::Typescript => {
            // npm: lowercase, URL-safe, not starting with `.` or `_`, 214 characters at most
            let part = |p: &str| !p.is_empty() && !p.starts_with('.') && !p.starts_with('_') && p.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'.' | b'_' | b'~'));
            let ok = s.len() <= 214
                && match s.strip_prefix('@') {
                    Some(rest) => rest.split_once('/').is_some_and(|(scope, name)| part(scope) && part(name)),
                    None => part(s),
                };
            if ok { Ok(()) } else { fail("npm のパッケージの名前（`name` か `@scope/name`。英小文字、数字、`-`、`.`、`_`、`~` で 214 字まで）として読めません", "it is not an npm package's name (`name` or `@scope/name`: lowercase letters, digits, `-`, `.`, `_` and `~`, 214 characters at most)") }
        }
        Target::Python => {
            // PEP 508: ^([A-Z0-9]|[A-Z0-9][A-Z0-9._-]*[A-Z0-9])$, in either case
            let b = s.as_bytes();
            let end = |c: u8| c.is_ascii_alphanumeric();
            let ok = !b.is_empty() && end(b[0]) && end(b[b.len() - 1]) && b.iter().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'));
            if ok { Ok(()) } else { fail("Python のディストリビューションの名前（英数字で始まり英数字で終わり、間に `.`、`_`、`-` も使える）として読めません", "it is not a Python distribution's name (letters and digits, with `.`, `_` and `-` between, starting and ending with a letter or a digit)") }
        }
        Target::Go => {
            let parts: Vec<&str> = s.split('/').collect();
            let elem = |p: &str| !p.is_empty() && p != "." && p != ".." && !p.starts_with('.') && !p.ends_with('.') && p.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~'));
            let first = parts[0];
            let domain = first.contains('.') && !first.starts_with('-') && first.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'.'));
            if !(parts.iter().all(|p| elem(p)) && domain) {
                return fail("Go のモジュールのパス（`.` を含むホストで始め、英数字と `-`、`.`、`_`、`~` の要素を `/` でつなぐ）として読めません", "it is not a Go module path (a host with a `.` in it first, then parts of letters, digits, `-`, `.`, `_` and `~`, with `/` between them)");
            }
            let last = parts[parts.len() - 1];
            let major = last.strip_prefix('v').is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) && !n.starts_with('0') && n != "1");
            if parts.len() > 1 && major {
                return fail("最後の `/vN` は書かないでください。版 2 からは ritsu が付けます", "leave out the `/vN` at its end; ritsu adds it from version 2 on");
            }
            Ok(())
        }
    }
}
