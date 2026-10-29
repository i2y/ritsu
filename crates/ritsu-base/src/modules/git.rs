//! A dependency from a git repository (DESIGN 17.12.1), as far as it is decided without running
//! git: the URLs a `require … git` takes, and the module name a URL and a `dir` give; the tag of a
//! version, and the versions among the tags `git ls-remote` lists; the name of a commit; a file that
//! is a Git LFS pointer; and the users and passwords of URLs hidden in what git says. Running git
//! is the `ritsu` crate's (`ritsu::modules::git`), which the npm package and the page in the
//! browser do not have: they read what is in the store and in the module's copy of it.
//!
//! A URL is https, ssh (`ssh://` or the scp form `<user>@<host>:<path>`), or http to a host of the
//! loopback (the git server a test starts). Every other form is E302, before git is started:
//! `git://` (neither authentication nor a check of what comes), `file://` and local paths (a
//! directory beside the module is `require … path`), `<transport>::<address>` (git runs a program
//! for it), plain http elsewhere, a value or a host that starts with `-`, a user or a password in an
//! https URL, a password in an ssh one, a query, a fragment, a space or a control character.

use super::names::ModuleName;
use super::semver::Version;
use crate::text::Text;
use crate::tr;
use std::collections::BTreeMap;

/// How a URL reaches its repository.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transport {
    Https,
    /// `ssh://[<user>@]<host>[:<port>]/<path>`.
    Ssh,
    /// `<user>@<host>:<path>`.
    Scp,
    /// `http://` to a host of the loopback.
    Http,
}

/// A URL a `require … git` takes, read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Url {
    pub transport: Transport,
    /// The host, lowercase, without its port.
    pub host: String,
    /// The path of the repository on the host, with no `/` at either end.
    pub path: String,
}

impl Url {
    /// Whether git is to be let use plain http for it (a host of the loopback).
    pub fn loopback(&self) -> bool {
        self.transport == Transport::Http
    }
}

/// What is wrong with a URL: the sentence, and whether it may carry a credential, in which case
/// neither the URL nor its line is shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    pub text: Text,
    pub secret: bool,
}

fn wrong(text: Text) -> Problem {
    Problem { text, secret: false }
}

fn secret(text: Text) -> Problem {
    Problem { text, secret: true }
}

/// Whether a host is one ritsu takes: lowercase letters, digits, `-` and `.`, its labels neither
/// empty nor starting or ending with a `-`.
fn host_ok(h: &str) -> bool {
    !h.is_empty()
        && h.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'.')
        && h.split('.').all(|l| !l.is_empty() && !l.starts_with('-') && !l.ends_with('-'))
}

fn port_ok(p: &str) -> bool {
    !p.is_empty() && p.len() <= 5 && p.bytes().all(|b| b.is_ascii_digit()) && !p.starts_with('0') && p.parse::<u32>().is_ok_and(|n| (1..=65535).contains(&n))
}

/// The user of an ssh URL: letters, digits, `.`, `_` and `-`, not starting with a `-`.
fn user_ok(u: &str) -> bool {
    !u.is_empty() && !u.starts_with('-') && u.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// `host[:port]`, read: the host, or what is wrong with it.
fn host_port(url: &str, hp: &str) -> Result<String, Problem> {
    let (host, port) = match hp.rsplit_once(':') {
        Some((h, p)) => (h, Some(p)),
        None => (hp, None),
    };
    if host.starts_with('-') {
        return Err(wrong(tr!("git の URL `{url}` のホストが `-` で始まっています", "the host of the git URL `{url}` starts with a `-`")));
    }
    if host.bytes().any(|b| b.is_ascii_uppercase()) {
        return Err(wrong(tr!(
            "git の URL `{url}` のホストに大文字があります。小文字で書いてください（モジュールの名前になるので、小文字に限っています）",
            "the host of the git URL `{url}` has an uppercase letter in it; write it in lowercase (it is part of the module's name, which is lowercase)"
        )));
    }
    if !host_ok(host) {
        return Err(wrong(tr!("git の URL `{url}` のホスト `{host}` は、ホスト名として読めません", "the host `{host}` of the git URL `{url}` is not a host name")));
    }
    if let Some(p) = port
        && !port_ok(p)
    {
        return Err(wrong(tr!("git の URL `{url}` のポート `{p}` は、1 から 65535 の数で書いてください", "the port `{p}` of the git URL `{url}` is a number from 1 to 65535")));
    }
    Ok(host.to_string())
}

/// The path of a repository from a URL, with no `/` at either end, or what is wrong with it.
fn repo_path(url: &str, p: &str) -> Result<String, Problem> {
    let path = p.trim_matches('/');
    if path.is_empty() {
        return Err(wrong(tr!("git の URL `{url}` に、リポジトリのパスがありません", "the git URL `{url}` names no repository on its host")));
    }
    if path.starts_with('-') {
        return Err(wrong(tr!("git の URL `{url}` のパスが `-` で始まっています", "the path of the git URL `{url}` starts with a `-`")));
    }
    Ok(path.to_string())
}

/// The URL of a `require … git`, read; or what is wrong with it (E302).
pub fn parse(url: &str) -> Result<Url, Problem> {
    if url.is_empty() {
        return Err(wrong(tr!("git の URL が空です", "the git URL is empty")));
    }
    if url.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(secret(tr!("git の URL に空白か制御文字があります。URL はそのまま一語で書いてください", "the git URL has a space or a control character in it; write the URL as one word")));
    }
    if url.starts_with('-') {
        return Err(wrong(tr!("git の URL `{url}` が `-` で始まっています。git がオプションとして読むので、書けません", "the git URL `{url}` starts with a `-`, which git reads as an option")));
    }
    if url.contains('?') || url.contains('#') {
        return Err(secret(tr!("git の URL にクエリかフラグメント（`?` か `#` から後）があります。`require … git` には書けません", "the git URL has a query or a fragment (from the `?` or the `#` on) in it, which a `require … git` cannot have")));
    }
    // `<transport>::<address>`: git runs `git-remote-<transport>` for it
    if let Some((t, _)) = url.split_once("::")
        && !t.is_empty()
        && !t.contains('/')
        && t.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
    {
        return Err(wrong(tr!(
            "git の URL `{url}` は `{t}::` の形です。git が外のプログラムを走らせる書き方なので、受け付けません。https か ssh の URL で書いてください",
            "the git URL `{url}` is of the form `{t}::`, for which git runs a program of its own; write an https or an ssh URL"
        )));
    }
    if let Some((scheme, rest)) = url.split_once("://") {
        let scheme = scheme.to_ascii_lowercase();
        let (authority, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
        let transport = match scheme.as_str() {
            "https" => Transport::Https,
            "ssh" => Transport::Ssh,
            "http" => Transport::Http,
            "git" => {
                return Err(wrong(tr!(
                    "`git://` の URL は受け付けません（`{url}`）。git のプロトコルには認証も、受け取ったものの改ざんを見つける仕組みもありません。https か ssh の URL で書いてください",
                    "a `git://` URL is not taken (`{url}`): the git protocol has neither authentication nor a check of what it sends; write an https or an ssh URL"
                )));
            }
            "file" => {
                return Err(wrong(tr!(
                    "`file://` の URL は受け付けません（`{url}`）。手元のディレクトリのモジュールは `require … path` で書いてください",
                    "a `file://` URL is not taken (`{url}`); a module in a directory of this machine is required with `require … path`"
                )));
            }
            _ => {
                return Err(wrong(tr!("git の URL `{url}` のスキーム `{scheme}` は受け付けません。https か ssh の URL で書いてください", "the scheme `{scheme}` of the git URL `{url}` is not taken; write an https or an ssh URL")));
            }
        };
        let (user, hp) = match authority.rsplit_once('@') {
            Some((u, h)) => (Some(u), h),
            None => (None, authority),
        };
        match (transport, user) {
            (Transport::Https | Transport::Http, Some(_)) => {
                return Err(secret(tr!(
                    "git の URL にユーザーかパスワードが入っています。資格情報は `ritsu.mod` に書かず、git の資格情報のヘルパーか ssh の鍵に任せてください",
                    "the git URL has a user or a password in it; keep credentials out of `ritsu.mod`, and leave them to git's credential helper or an ssh key"
                )));
            }
            (Transport::Ssh, Some(u)) if u.contains(':') => {
                return Err(secret(tr!(
                    "git の URL にパスワードが入っています。ssh の URL にはユーザーだけを書き、認証は ssh の鍵に任せてください",
                    "the git URL has a password in it; an ssh URL names the user alone, and leaves the rest to an ssh key"
                )));
            }
            (Transport::Ssh, Some(u)) if !user_ok(u) => {
                return Err(wrong(tr!("git の URL `{url}` のユーザー `{u}` は、ユーザーの名前として読めません", "the user `{u}` of the git URL `{url}` is not a user's name")));
            }
            _ => {}
        }
        let host = host_port(url, hp)?;
        if transport == Transport::Http && !crate::urls::is_loopback(&host) {
            return Err(wrong(tr!(
                "git の URL `{url}` は平文の http です。この機械の上のサーバーのほかは、https か ssh の URL で書いてください",
                "the git URL `{url}` is plain http, which is taken for a server on this machine only; write an https or an ssh URL"
            )));
        }
        let path = repo_path(url, path)?;
        return Ok(Url { transport, host, path });
    }
    // the scp form: `<user>@<host>:<path>`, with no `/` before the `:`
    if let Some((before, path)) = url.split_once(':')
        && !before.contains('/')
    {
        let Some((user, host)) = before.rsplit_once('@') else {
            return Err(wrong(tr!(
                "git の URL `{url}` にユーザーがありません。scp の形は `git@example.com:acme/rates.git` のようにユーザーを付けて書くか、`ssh://` か `https://` の URL で書いてください",
                "the git URL `{url}` has no user; write the scp form with a user, as `git@example.com:acme/rates.git`, or an `ssh://` or `https://` URL"
            )));
        };
        if !user_ok(user) {
            return Err(wrong(tr!("git の URL `{url}` のユーザー `{user}` は、ユーザーの名前として読めません", "the user `{user}` of the git URL `{url}` is not a user's name")));
        }
        let host = host_port(url, host)?;
        let path = repo_path(url, path)?;
        return Ok(Url { transport: Transport::Scp, host, path });
    }
    Err(wrong(tr!(
        "`{url}` は git の URL ではありません。https か ssh の URL で書いてください。手元のディレクトリのモジュールは `require … path` で書いてください",
        "`{url}` is no git URL; write an https or an ssh URL, and a module in a directory of this machine with `require … path`"
    )))
}

/// What is wrong with the `dir` of a `require … git`, if anything: a path in the repository, `/`
/// between its parts, with neither `.` nor `..`.
pub fn dir_problem(dir: &str) -> Option<Text> {
    let bad = dir.is_empty() || dir.starts_with('/') || dir.ends_with('/') || dir.contains('\\') || dir.split('/').any(|p| p.is_empty() || p == "." || p == "..");
    bad.then(|| {
        tr!(
            "`dir` の `{dir}` は、リポジトリのルートからのディレクトリで、`/` で区切り、`.` と `..` を含まないパスで書いてください",
            "the `dir` `{dir}` is a directory from the root of the repository, `/` between its parts, with neither `.` nor `..`"
        )
    })
}

/// The name of the module a URL and a `dir` give (DESIGN 17.12.1): the host, the path without a
/// `.git` at its end, and the `dir`, with `/` between them; as text, before it is held to the form
/// of a published name.
pub fn name_text(url: &Url, dir: Option<&str>) -> String {
    let path = url.path.strip_suffix(".git").unwrap_or(&url.path).trim_end_matches('/');
    match dir {
        Some(d) => format!("{}/{path}/{d}", url.host),
        None => format!("{}/{path}", url.host),
    }
}

/// The module name a URL and a `dir` give, held to the form of a published name; or what is wrong
/// (E302).
pub fn name_of(url: &str, dir: Option<&str>) -> Result<ModuleName, Problem> {
    let u = parse(url)?;
    if let Some(d) = dir
        && let Some(t) = dir_problem(d)
    {
        return Err(wrong(t));
    }
    let text = name_text(&u, dir);
    if text.bytes().any(|b| b.is_ascii_uppercase()) {
        return Err(wrong(tr!(
            "git の URL と `dir` から決まるモジュールの名前 `{text}` に大文字があります。モジュールの名前は小文字なので、小文字で書いてください。大文字と小文字を区別しないホストなら、小文字で同じリポジトリを指せます。区別するホストにある、大文字を含むリポジトリは git の依存にできません",
            "the module name `{text}` the git URL and `dir` give has an uppercase letter in it, and a module's name is lowercase: write them in lowercase. A host that does not tell the cases apart takes the lowercase path for the same repository; a repository with an uppercase letter on a host that tells them apart cannot be a git dependency"
        )));
    }
    match ModuleName::parse(&text) {
        Ok(m) if m.is_published() => Ok(m),
        Ok(_) => Err(wrong(tr!("git の URL `{url}` から、配るモジュールの名前が決まりません", "the git URL `{url}` gives no published module name"))),
        Err(e) => {
            let why = e.text();
            Err(wrong(tr!(
                "git の URL と `dir` から決まるモジュールの名前 `{text}` は、配るモジュールの名前として読めません。{}",
                "the module name `{text}` the git URL and `dir` give is not a published module's name: {}",
                why.ja; why.en
            )))
        }
    }
}

/// The `dir` of a git module, from its name and its URL: none for a module at the root of its
/// repository; None when the name is not one the URL gives.
pub fn dir_of(name: &ModuleName, url: &str) -> Option<Option<String>> {
    let u = parse(url).ok()?;
    let base = name_text(&u, None);
    let n = name.to_string();
    if n == base {
        return Some(None);
    }
    n.strip_prefix(&format!("{base}/")).map(|d| Some(d.to_string()))
}

/// The tag of a version (DESIGN 17.12.1): `v<version>`, after `<dir>/` for a module in a
/// directory of its repository.
pub fn tag_of(dir: Option<&str>, version: &Version) -> String {
    match dir {
        Some(d) => format!("{d}/v{version}"),
        None => format!("v{version}"),
    }
}

/// The pattern `git ls-remote` is given for the tags of the versions: `refs/tags/[<dir>/]v*`. git
/// matches it at the end of each name, and ritsu chooses again ([`versions_of`]).
pub fn tag_pattern(dir: Option<&str>) -> String {
    match dir {
        Some(d) => format!("refs/tags/{d}/v*"),
        None => "refs/tags/v*".to_string(),
    }
}

/// Whether `s` is the name of a commit: 40 lowercase hex digits (SHA-1) or 64 (SHA-256).
pub fn is_commit(s: &str) -> bool {
    matches!(s.len(), 40 | 64) && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// The versions among the tags `git ls-remote --tags` printed (`out`), lowest first, each with the
/// commit its tag leads to: a tag `v<version>` (`<dir>/v<version>`), whose version is one ritsu
/// writes (no `+`, no leading zero), and nothing else. An annotated tag leads to the commit its
/// `^{}` line names.
pub fn versions_of(out: &str, dir: Option<&str>) -> Vec<(Version, String)> {
    let prefix = match dir {
        Some(d) => format!("refs/tags/{d}/v"),
        None => "refs/tags/v".to_string(),
    };
    let mut found: BTreeMap<String, (Option<String>, Option<String>)> = BTreeMap::new();
    for line in out.lines() {
        let Some((oid, name)) = line.split_once('\t') else { continue };
        if !is_commit(oid) {
            continue;
        }
        let (name, peeled) = match name.strip_suffix("^{}") {
            Some(n) => (n, true),
            None => (name, false),
        };
        let Some(v) = name.strip_prefix(&prefix) else { continue };
        let e = found.entry(v.to_string()).or_default();
        if peeled {
            e.1 = Some(oid.to_string());
        } else {
            e.0 = Some(oid.to_string());
        }
    }
    let mut out: Vec<(Version, String)> = found
        .into_iter()
        .filter_map(|(v, (own, peeled))| {
            let version = Version::parse(&v).ok().filter(|x| x.to_string() == v)?;
            Some((version, peeled.or(own)?))
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Whether a file is a Git LFS pointer, which names the file rather than holding it: under 1024
/// bytes, and starting with the line of the pointer's version (the one Git LFS writes, or the one of
/// its first releases, which it still reads).
pub fn is_lfs_pointer(bytes: &[u8]) -> bool {
    bytes.len() < 1024 && (bytes.starts_with(b"version https://git-lfs.github.com/spec/v1") || bytes.starts_with(b"version https://hawser.github.com/spec/v1"))
}

/// `text` with the user and the password of each URL in it hidden (`https://***@example.com/…`):
/// what git says goes to a terminal and to logs.
pub fn redacted(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(at) = rest.find("://") {
        let (head, tail) = rest.split_at(at + 3);
        out.push_str(head);
        let end = tail.find(|c: char| c == '/' || c.is_whitespace() || matches!(c, '\'' | '"' | '<' | '>')).unwrap_or(tail.len());
        let authority = &tail[..end];
        match authority.rfind('@') {
            Some(k) => {
                out.push_str("***");
                out.push_str(&authority[k..]);
            }
            None => out.push_str(authority),
        }
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(url: &str, dir: Option<&str>) -> Result<String, String> {
        name_of(url, dir).map(|n| n.to_string()).map_err(|p| p.text.en)
    }

    #[test]
    fn the_names_of_urls() {
        assert_eq!(name("https://example.com/acme/rates", None).as_deref(), Ok("example.com/acme/rates"));
        assert_eq!(name("https://example.com/acme/rates.git/", None).as_deref(), Ok("example.com/acme/rates"));
        assert_eq!(name("ssh://git@example.com:2222/acme/mono.git", Some("modules/tax")).as_deref(), Ok("example.com/acme/mono/modules/tax"));
        assert_eq!(name("git@example.com:acme/rates.git", None).as_deref(), Ok("example.com/acme/rates"));
        assert_eq!(name("http://127.0.0.1:4100/acme/rates.git", None).as_deref(), Ok("127.0.0.1/acme/rates"));
        assert!(name("https://example.com/Acme/rates", None).unwrap_err().contains("uppercase"));
        assert!(name("https://Example.com/acme/rates", None).unwrap_err().contains("lowercase"));
        assert!(name("https://example.com/acme/rates", Some("../x")).unwrap_err().contains("neither `.` nor `..`"));
        assert!(name("https://example.com/~acme/rates", None).unwrap_err().contains("not a published module's name"));
        assert_eq!(dir_of(&ModuleName::parse("example.com/acme/mono/modules/tax").unwrap(), "git@example.com:acme/mono.git"), Some(Some("modules/tax".to_string())));
        assert_eq!(dir_of(&ModuleName::parse("example.com/acme/rates").unwrap(), "https://example.com/acme/rates"), Some(None));
        assert_eq!(dir_of(&ModuleName::parse("example.com/other").unwrap(), "https://example.com/acme/rates"), None);
    }

    #[test]
    fn the_urls_refused() {
        let refused = |u: &str| parse(u).unwrap_err();
        for u in ["git://example.com/acme/rates", "file:///srv/rates", "../rates", "/srv/rates", "ext::ssh-proxy", "fd::17", "http://example.com/acme/rates", "-oProxyCommand=x", "git@-oProxyCommand=x:a", "example.com:acme/rates", "ftp://example.com/a"] {
            assert!(!refused(u).secret, "{u}");
        }
        for u in ["https://user:token@example.com/acme/rates", "https://token@example.com/acme/rates", "ssh://git:pw@example.com/a", "https://example.com/a?private_token=x", "https://example.com/a#x", "https://example.com/a b", "https://example.com/a\u{1b}[31m"] {
            let p = refused(u);
            assert!(p.secret && !p.text.en.contains("example.com"), "{u}: {:?}", p.text);
        }
        assert_eq!(parse("ssh://git@example.com/acme/rates.git").map(|u| u.transport), Ok(Transport::Ssh));
        assert!(parse("http://localhost:8080/acme/rates").is_ok_and(|u| u.loopback()));
    }

    #[test]
    fn the_versions_of_the_tags() {
        let c = |n: u8| format!("{:040x}", n);
        let out = [
            format!("{}\trefs/tags/v1.0.0", c(1)),
            format!("{}\trefs/tags/v1.1.0", c(2)),
            format!("{}\trefs/tags/v1.1.0^{{}}", c(3)),
            format!("{}\trefs/tags/v01.0.0", c(4)),
            format!("{}\trefs/tags/v1.2.0+build.5", c(5)),
            format!("{}\trefs/tags/1.3.0", c(6)),
            format!("{}\trefs/tags/modules/tax/v2.0.0", c(7)),
            format!("{}\trefs/tags/v2.0.0-rc.1", c(8)),
        ]
        .join("\n");
        let vs: Vec<(String, String)> = versions_of(&out, None).into_iter().map(|(v, o)| (v.to_string(), o)).collect();
        assert_eq!(vs, [("1.0.0".to_string(), c(1)), ("1.1.0".to_string(), c(3)), ("2.0.0-rc.1".to_string(), c(8))]);
        let vs: Vec<String> = versions_of(&out, Some("modules/tax")).into_iter().map(|(v, _)| v.to_string()).collect();
        assert_eq!(vs, ["2.0.0"]);
        assert_eq!(tag_of(Some("modules/tax"), &Version::parse("2.0.0").unwrap()), "modules/tax/v2.0.0");
        assert_eq!(tag_pattern(None), "refs/tags/v*");
    }

    #[test]
    fn lfs_pointers_and_what_git_says() {
        assert!(is_lfs_pointer(b"version https://git-lfs.github.com/spec/v1\noid sha256:4d7a\nsize 12345\n"));
        assert!(!is_lfs_pointer(b"version 1\n"));
        assert_eq!(redacted("fatal: unable to access 'https://user:tok@example.com/a/': 401"), "fatal: unable to access 'https://***@example.com/a/': 401");
        assert_eq!(redacted("fatal: 'ssh://git@example.com/a' x"), "fatal: 'ssh://***@example.com/a' x");
        assert_eq!(redacted("no url here"), "no url here");
        assert!(is_commit(&"a".repeat(40)) && is_commit(&"0".repeat(64)) && !is_commit("abc") && !is_commit(&"A".repeat(40)));
    }
}
