//! `ritsu mod site <list> --out <dir> [--playground <URL>] [--alternate <lang>=<path>]` (DESIGN
//! 17.17): the site that lists modules, written from a list of them.
//!
//! - **The list** is written in the lexis of `ritsu.mod` (DESIGN 17.3): `title`, `description`,
//!   `playground`, `contribute` (each once), `module "<name>"` (a module of a registry), `git
//!   "<URL>" [dir "<dir>"]` (a module of a git repository, DESIGN 17.12.1), `local "<path>"` (the
//!   working tree of a module that is not published yet, from the list's directory), and `signature`
//!   lines, which are read as `ritsu.mod` reads them and check the modules the list shows (their key
//!   files are read from the list's directory). A `signature` line that covers a module of a git
//!   repository makes the list one that cannot be read: no signature of git is checked. A `module`,
//!   `git` or `local` line can end with `as "<name>"`, the name the pages tell a reader to require it
//!   by.
//! - **What is read.** For a module of a registry, the versions its registry lists (for one of a
//!   git repository, its tags), each fetched as `ritsu mod fetch <name>@<version>` fetches it, its
//!   signature checked against the list's lines that cover it, and then read as `ritsu mod diff`
//!   reads a version: as a module of its own, with its own lock, whose modules are fetched and
//!   checked against its own lines ([`fetch::listed`]).
//!   A version that does not pass is shown as not verified, and the command exits 1. For a working
//!   tree, its files, and the modules its lock pins, fetched as `ritsu mod fetch` fetches them. A
//!   working tree of a module the list also names from a registry is one more version of it, when
//!   the registry does not have its version.
//! - **What else is read.** The day a version was published: for one of a registry whose signature
//!   a line of the list checked, the day the record of its signature says it went into the
//!   transparency log; for one of a git repository, the date of its tag's commit; for any other,
//!   none. The manifest's own bytes hold no date, so that a push of the same files makes the same
//!   digest (DESIGN 17.11).
//! - **The pages** ([`pages`]): the list, each module, each version, each file (the page its
//!   language's `doc` draws, run as `ritsu <language> doc <file> --format html` in the version's
//!   directory; the source for geas and every other file), and `module.json`, the files the
//!   playground opens. A module's page is the page of its newest version.
//! - **The playground.** `module.json` holds the version's files and the modules its lock pins, in
//!   the form of a module's copy of the store (`.ritsu/modules/sha256/<hex>/…`), as the edits of a
//!   link of the playground hold them (`[[path, text], …]`; a file that is not UTF-8 as `[path,
//!   {"base64": "…"}]`). The button's script reads it from the same origin, packs it as the
//!   playground packs its edits (raw DEFLATE, base64url) and opens `<playground>#project=empty&
//!   file=<…>&edits=<…>`, on the file the tab to try opens on; past 1,000,000 characters it says that
//!   the version is too big to open in the page.
//!   A working tree that requires a module by a path has no button: the directory is not among its
//!   files.
//!
//! Everything is read and fetched before anything is written: a list that cannot be read, a
//! registry that cannot be asked, or no cosign when a line asks for one, stops the command with
//! nothing written (exit 2).

use super::{Ctx, diff, fetch, registry};
use ritsu_base::cli::Args;
use ritsu_base::modules::git as gitbase;
use ritsu_base::modules::graph::Origin;
use ritsu_base::modules::lex::{self, Kind, Token};
use ritsu_base::modules::lock as lockfile;
use ritsu_base::modules::manifest::{self, Signature, Source};
use ritsu_base::modules::names::{self, ModuleName};
use ritsu_base::modules::semver::Version;
use ritsu_base::modules::store::Store;
use ritsu_base::modules::vendor;
use ritsu_base::text::{Lang, Text, plural};
use ritsu_base::tr;
use ritsu_project::Joined;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

mod changes;
mod code;
mod holds;
mod markdown;
mod pages;

/// The playground of ritsu's site, in each language: where a version opens when neither the
/// command line nor the list says otherwise.
const PLAYGROUND_EN: &str = "https://i2y.github.io/ritsu/play/";
const PLAYGROUND_JA: &str = "https://i2y.github.io/ritsu/ja/play/";

/// The words a line of a list starts with.
const WORDS: [&str; 8] = ["title", "description", "playground", "contribute", "module", "git", "signature", "local"];

// ── The list ────────────────────────────────────────────────────────────

/// A module the list shows.
enum Entry {
    /// From a registry; the name its `as` gives it.
    Module(ModuleName, Option<String>),
    /// From a git repository: the name its URL and `dir` give, the URL and the `dir`; the name its
    /// `as` gives it.
    Git(ModuleName, String, Option<String>, Option<String>),
    /// A working tree: the path as written, and the directory it leads to; the name its `as` gives it.
    Local(String, PathBuf, Option<String>),
}

/// A list of modules, read.
struct List {
    title: Option<String>,
    description: Option<String>,
    playground: Option<String>,
    /// Where a module is added to the list; a list without it does not offer it.
    contribute: Option<String>,
    entries: Vec<Entry>,
    signatures: Vec<Signature>,
    /// Where it is: what its `local` paths and its key files are read from.
    dir: PathBuf,
}

/// The text of the list at `file` (as the command line writes it), and the directory it is in.
fn list_text(file: &str) -> Result<(String, PathBuf), Text> {
    let path = Path::new(file);
    let bytes = std::fs::read(path).map_err(|e| tr!("{file} を読めません（{e}）", "cannot read {file} ({e})"))?;
    let text = String::from_utf8(bytes).map_err(|_| tr!("{file} は UTF-8 のテキストではありません", "{file} is not UTF-8 text"))?;
    let dir = std::fs::canonicalize(path).ok().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_else(|| PathBuf::from("."));
    Ok((text, dir))
}

/// The list `file` holds (`text`, in `dir`): the list, or what is wrong with it, a sentence each,
/// after its line and its column.
fn read_list(file: &str, text: &str, dir: PathBuf) -> Result<List, Vec<Text>> {
    let mut list = List { title: None, description: None, playground: None, contribute: None, entries: Vec::new(), signatures: Vec::new(), dir };
    let mut wrong: Vec<Text> = Vec::new();
    let mut once: Vec<(&str, usize)> = Vec::new();
    for (i, (src, _)) in lex::split(text).into_iter().enumerate() {
        let n = i + 1;
        let at = |col: usize, t: Text| tr!("{file}:{n}:{col}: {}", "{file}:{n}:{col}: {}", t.ja; t.en);
        let line = match lex::line(src) {
            Ok(l) => l,
            Err(e) => {
                wrong.push(at(e.col(), e.text()));
                continue;
            }
        };
        let Some(head) = line.tokens.first() else { continue };
        let Some(word) = WORDS.into_iter().find(|w| head.kind == Kind::Word && head.text == *w) else {
            let w = &head.text;
            wrong.push(at(
                head.col,
                tr!(
                    "`{w}` で始まる行は書けません。一覧の行は title、description、playground、contribute、module、git、signature、local のどれかで始めてください",
                    "no line starts with `{w}`; a line of a list starts with title, description, playground, contribute, module, git, signature or local"
                ),
            ));
            continue;
        };
        // `as "<name>"` at the end of a line that names a module
        let mut args = &line.tokens[1..];
        let mut short: Option<String> = None;
        if matches!(word, "module" | "git" | "local")
            && let [before @ .., w, name] = args
            && w.kind == Kind::Word
            && w.text == "as"
        {
            if name.kind != Kind::Str {
                wrong.push(at(name.col, tr!("`as` のあとには文字列を書いてください", "`as` takes a string")));
                continue;
            }
            if let Err(e) = names::dependency_ok(&name.text) {
                let (v, why) = (&name.text, e.text());
                wrong.push(at(name.col, tr!("`as \"{v}\"` は依存の名前として読めません。{}", "`as \"{v}\"` is not a dependency's name: {}", why.ja; why.en)));
                continue;
            }
            short = Some(name.text.clone());
            args = before;
        }
        if word == "signature" {
            match manifest::signature_line(args, n) {
                Ok(s) => list.signatures.push(s),
                Err((col, t)) => wrong.push(at(col, t)),
            }
            continue;
        }
        if word == "git" {
            match git_line(args) {
                Ok((m, url, dir)) => {
                    if list.entries.iter().any(|e| matches!(e, Entry::Module(x, _) | Entry::Git(x, ..) if *x == m)) {
                        wrong.push(at(args[0].col, tr!("モジュール `{m}` はもう一覧にあります", "the module `{m}` is in the list already")));
                    } else {
                        list.entries.push(Entry::Git(m, url, dir, short));
                    }
                }
                Err((col, t)) => wrong.push(at(col.unwrap_or(head.col), t)),
            }
            continue;
        }
        // every other line takes one string
        let Some(value) = one_string(args) else {
            wrong.push(at(head.col, tr!("`{word}` の行は `{word} \"…\"` の形で書いてください", "write the `{word}` line as `{word} \"…\"`")));
            continue;
        };
        let col = args[0].col;
        if matches!(word, "title" | "description" | "playground" | "contribute") {
            if let Some((_, l)) = once.iter().find(|(w, _)| *w == word) {
                let l = *l;
                wrong.push(at(head.col, tr!("`{word}` は一度しか書けません。{l} 行目にもう書いてあります", "`{word}` is written once, and it is already on line {l}")));
                continue;
            }
            once.push((word, n));
        }
        match word {
            "title" => list.title = Some(value),
            "description" => list.description = Some(value),
            "playground" => match playground_problem(&value) {
                None => list.playground = Some(value),
                Some(t) => wrong.push(at(col, t)),
            },
            "contribute" => {
                if ritsu_base::urls::scheme_and_host(&value).is_some_and(|(s, _)| s == "https") {
                    list.contribute = Some(value);
                } else {
                    wrong.push(at(col, tr!("`contribute` には https の URL を書いてください", "`contribute` takes an https URL")));
                }
            }
            "module" => match ModuleName::parse(&value) {
                Ok(m) if m.is_published() => {
                    if list.entries.iter().any(|e| matches!(e, Entry::Module(x, _) | Entry::Git(x, ..) if *x == m)) {
                        wrong.push(at(col, tr!("モジュール `{m}` はもう一覧にあります", "the module `{m}` is in the list already")));
                    } else {
                        list.entries.push(Entry::Module(m, short));
                    }
                }
                Ok(_) => wrong.push(at(
                    col,
                    tr!(
                        "`{value}` は配らないモジュールの名前です。`module` には `<レジストリ>/<リポジトリ>` を書いてください。まだ送り出していない作業ツリーなら、`local` の行で書いてください",
                        "`{value}` is the name of a module that is not published; `module` takes `<registry>/<repository>`, and a working tree not pushed yet is written with `local`"
                    ),
                )),
                Err(e) => {
                    let why = e.text();
                    wrong.push(at(col, tr!("`{value}` はモジュールの名前として読めません。{}", "`{value}` is not a module's name: {}", why.ja; why.en)));
                }
            },
            _ => {
                if ritsu_base::paths::is_absolute(&value) {
                    wrong.push(at(
                        col,
                        tr!("`{value}` は絶対パスです。一覧のファイルのあるディレクトリからの相対パスで書いてください", "`{value}` is an absolute path; write it from the directory of the list"),
                    ));
                    continue;
                }
                if let Some(t) = manifest::path_problem(&value, false) {
                    wrong.push(at(col, t));
                    continue;
                }
                let d = list.dir.join(&value);
                if !d.join(manifest::FILE).is_file() {
                    wrong.push(at(col, tr!("`local \"{value}\"` の指すディレクトリに `ritsu.mod` がありません", "the directory `local \"{value}\"` names has no `ritsu.mod`")));
                    continue;
                }
                let d = std::fs::canonicalize(&d).unwrap_or(d);
                if list.entries.iter().any(|e| matches!(e, Entry::Local(_, x, _) if *x == d)) {
                    wrong.push(at(col, tr!("`local \"{value}\"` はもう一覧にあります", "`local \"{value}\"` is in the list already")));
                    continue;
                }
                list.entries.push(Entry::Local(value, d, short));
            }
        }
    }
    if list.entries.is_empty() && wrong.is_empty() {
        wrong.push(tr!("{file} には `module` の行も `git` の行も `local` の行もありません", "{file} has no `module` line, no `git` line and no `local` line"));
    }
    // a line that would check the signature of a module from git, which has none to check (E309)
    for s in &list.signatures {
        for e in &list.entries {
            if let Entry::Git(m, ..) = e
                && names::covers(&s.prefix, &m.to_string())
            {
                let n = s.line;
                let t = ritsu_base::modules::graph::e309_text(&s.prefix, &m.to_string());
                wrong.push(tr!("{file}:{n}:1: {}", "{file}:{n}:1: {}", t.ja; t.en));
            }
        }
    }
    if wrong.is_empty() { Ok(list) } else { Err(wrong) }
}

/// A `git` line of the list: the module's name, the URL and the `dir`.
type GitLine = (ModuleName, String, Option<String>);

/// The words after `git` on a line of the list: `"<URL>"`, then `dir "<dir>"` or nothing. The
/// module's name, the URL and the `dir`; or the column (when one is to say) and what is wrong.
fn git_line(args: &[Token]) -> Result<GitLine, (Option<usize>, Text)> {
    let form = || (None, tr!("`git` の行は `git \"<URL>\"` か `git \"<URL>\" dir \"<ディレクトリ>\"` の形で書いてください", "write the `git` line as `git \"<URL>\"` or `git \"<URL>\" dir \"<dir>\"`"));
    let (url, dir) = match args {
        [u] if u.kind == Kind::Str => (u, None),
        [u, w, d] if u.kind == Kind::Str && w.kind == Kind::Word && w.text == "dir" && d.kind == Kind::Str => (u, Some(d)),
        _ => return Err(form()),
    };
    if let Some(d) = dir
        && let Some(t) = gitbase::dir_problem(&d.text)
    {
        return Err((Some(d.col), t));
    }
    // a URL that may carry a credential is said without itself
    let m = gitbase::name_of(&url.text, dir.map(|d| d.text.as_str())).map_err(|p| (Some(url.col), p.text))?;
    Ok((m, url.text.clone(), dir.map(|d| d.text.clone())))
}

/// The one string of a line's words.
fn one_string(args: &[Token]) -> Option<String> {
    match args {
        [t] if t.kind == Kind::Str => Some(t.text.clone()),
        _ => None,
    }
}

/// What is wrong with the URL of a playground, if anything: an https URL, or an http one on this
/// machine (a playground served to try a site out), with no user, no query and no fragment.
fn playground_problem(url: &str) -> Option<Text> {
    let ok = match ritsu_base::urls::scheme_and_host(url) {
        Some((s, _)) if s == "https" => true,
        Some((s, h)) if s == "http" => ritsu_base::urls::is_loopback(&h),
        _ => false,
    };
    if !ok || ritsu_base::urls::has_userinfo(url) || ritsu_base::urls::has_query(url) || url.contains('#') {
        return Some(tr!(
            "ブラウザで試すページの URL `{url}` は、https の URL（この機械の上なら http も）で、ユーザー、クエリ、`#` を含まないものを書いてください",
            "the playground's URL `{url}` is an https URL (or an http one on this machine) with no user, no query and no `#`"
        ));
    }
    None
}

// ── What is read ────────────────────────────────────────────────────────

/// A version of a module, as the site shows it.
struct Shown {
    /// Its version, as its `ritsu.mod` says it (`unversioned` for a working tree that says none).
    version: String,
    /// False for a working tree.
    published: bool,
    /// The day it was published (`2026-10-29`, in UTC), when a record says it.
    published_on: Option<String>,
    /// The digest of its OCI manifest, for a version of a registry or a git repository.
    digest: Option<String>,
    /// The commit of a version of a git repository.
    commit: Option<String>,
    /// Its files, read; None when it could not be verified.
    module: Option<diff::Module>,
    /// The modules its lock pins: the store each one's entry is read from, and its digest.
    deps: Vec<(Store, String)>,
    /// Whether it requires a module by a path, which the playground cannot be given.
    paths: bool,
    /// Why it could not be verified.
    failed: Option<Text>,
}

/// A module the list shows, with its versions, oldest first.
struct Shelf {
    name: String,
    /// The name the list's `as` gives it, which the pages tell a reader to require it by.
    short: Option<String>,
    /// Whether the list names it from a registry or a git repository.
    registry: bool,
    /// For a module of a git repository: its URL and its `dir`.
    git: Option<(String, Option<String>)>,
    versions: Vec<Shown>,
    /// The versions of the working trees of it that the list names and its registry has already.
    shadowed: Vec<String>,
}

impl Shelf {
    fn failed(&self) -> bool {
        self.versions.iter().any(|v| v.failed.is_some())
    }
}

/// The modules the lock of the module at `root` (whose files are `files`) pins from a registry or a
/// git repository, each where it is read: the module's copy of the store first, then the store.
fn pinned(root: &Path, files: &BTreeMap<String, Vec<u8>>) -> Vec<(Store, String)> {
    let Some(bytes) = files.get(lockfile::FILE) else { return Vec::new() };
    let Ok(lock) = lockfile::parse(bytes, lockfile::FILE) else { return Vec::new() };
    let store = Store::located();
    lock.modules
        .iter()
        .filter_map(|l| l.pinned.digests().and_then(|(_, digest, _)| vendor::find(root, digest, store.as_ref()).map(|(s, _)| (s, digest.to_string()))))
        .collect()
}

/// Every version of the module `name` its registry (`origin`: or its git repository's tags) lists,
/// fetched and checked; or what stops the command (exit 2) when the registry, the repository or
/// cosign cannot be asked.
fn of_registry(list: &List, name: &ModuleName, origin: &Origin) -> Result<Vec<Shown>, Text> {
    let versions = match origin {
        Origin::Oci => registry::Remote::of(name, false).and_then(|r| r.versions()),
        Origin::Git { url, dir } => super::git::Git::new().versions(url, dir.as_deref()).map(|vs| vs.into_iter().map(|(v, _)| v).collect()).map_err(super::git::Failed::text),
    }
    .map_err(|t| tr!("`{name}` の版の一覧を引けません。", "cannot list the versions of `{name}`: ").then(&t))?;
    let mut out = Vec::new();
    for v in versions {
        let shown = match fetch::listed(&list.dir, &list.signatures, name, origin, &v) {
            Ok((store, d)) => {
                let root = store.files(&d);
                let module = diff::Module::at(&root, format!("{name}@{v}"), Some(d.clone()))?;
                let deps = pinned(&root, &module.files);
                let commit = store.record(&d).and_then(|r| r.git).map(|(_, c)| c).filter(|_| matches!(origin, Origin::Git { .. }));
                let published_on = published_on(list, name, origin, &d, commit.as_deref());
                Shown { version: v.to_string(), published: true, published_on, digest: Some(d), commit, module: Some(module), deps, paths: false, failed: None }
            }
            Err((1, t)) => Shown { version: v.to_string(), published: true, published_on: None, digest: None, commit: None, module: None, deps: Vec::new(), paths: false, failed: Some(t) },
            Err((_, t)) => return Err(tr!("`{name}` {v} を取り出せません。", "cannot fetch `{name}` {v}: ").then(&t)),
        };
        out.push(shown);
    }
    Ok(out)
}

/// The day a version was published, when a record says it: the day its signature went into the
/// transparency log, for a version whose signature a line of the list checked; the date of its
/// commit, for a version of a git repository. A record that cannot be read leaves no date: it stops
/// nothing, and the pages say only what they know.
fn published_on(list: &List, name: &ModuleName, origin: &Origin, digest: &str, commit: Option<&str>) -> Option<String> {
    match origin {
        Origin::Oci => {
            if !list.signatures.iter().any(|s| names::covers(&s.prefix, &name.to_string())) {
                return None;
            }
            let at = registry::Remote::of(name, false).ok()?.signed_at(digest)?;
            Some(day_of(at))
        }
        Origin::Git { url, .. } => super::git::Git::new().committed_at(url, commit?).map(day_of),
    }
}

/// The day (`2026-10-29`, in UTC) of a time in seconds since the epoch.
fn day_of(seconds: i64) -> String {
    // the days since 0000-03-01, in years of 400 of the Gregorian calendar
    let z = seconds.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era = (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let m = (5 * day_of_year + 2) / 153;
    let (day, month) = (day_of_year - (153 * m + 2) / 5 + 1, if m < 10 { m + 3 } else { m - 9 });
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// The working tree at `dir` (`written` in the list): its name and the version shown, with the
/// modules its lock pins fetched first, as `ritsu mod fetch` fetches them (what was fetched said on
/// the standard error).
fn of_working_tree(dir: &Path, written: &str, lang: Lang) -> Result<(String, Shown), Text> {
    let done = fetch::tree(dir).map_err(|t| tr!("`local \"{written}\"` が読むモジュールを取り出せません。", "cannot fetch the modules `local \"{written}\"` reads: ").then(&t))?;
    fetch::told(lang, &done);
    let module = diff::Module::at(dir, written.to_string(), None)?;
    let paths = module.manifest.requires.iter().any(|r| matches!(r.source, Source::Path { .. }));
    let deps = pinned(dir, &module.files);
    let version = module.version.clone().unwrap_or_else(|| "unversioned".to_string());
    Ok((module.name.clone(), Shown { version, published: false, published_on: None, digest: None, commit: None, module: Some(module), deps, paths, failed: None }))
}

/// Every module of the list, read, in the order the list names them; a working tree of a module
/// the list names from a registry is put among that module's versions.
fn gather(list: &List, lang: Lang) -> Result<Vec<Shelf>, Text> {
    let mut shelves: Vec<Shelf> = Vec::new();
    for e in &list.entries {
        match e {
            Entry::Module(m, short) => shelves.push(Shelf { name: m.to_string(), short: short.clone(), registry: true, git: None, versions: of_registry(list, m, &Origin::Oci)?, shadowed: Vec::new() }),
            Entry::Git(m, url, dir, short) => {
                let origin = Origin::Git { url: url.clone(), dir: dir.clone() };
                shelves.push(Shelf { name: m.to_string(), short: short.clone(), registry: true, git: Some((url.clone(), dir.clone())), versions: of_registry(list, m, &origin)?, shadowed: Vec::new() });
            }
            Entry::Local(written, dir, short) => {
                let (name, shown) = of_working_tree(dir, written, lang)?;
                let later = shelves.iter().position(|s| s.name == name).is_none() && list.entries.iter().any(|x| matches!(x, Entry::Module(m, _) | Entry::Git(m, ..) if m.to_string() == name));
                if later {
                    // the registry's comes first, where the list names it
                    shelves.push(Shelf { name: format!("\u{0}{name}"), short: short.clone(), registry: false, git: None, versions: vec![shown], shadowed: Vec::new() });
                    continue;
                }
                put(&mut shelves, name, short.clone(), shown);
            }
        }
    }
    // a working tree read before the registry's module of its name, put among its versions
    let waiting: Vec<Shelf> = shelves.iter().position(|s| s.name.starts_with('\u{0}')).map(|_| shelves.extract_if(.., |s| s.name.starts_with('\u{0}')).collect()).unwrap_or_default();
    for s in waiting {
        let name = s.name.trim_start_matches('\u{0}').to_string();
        for v in s.versions {
            put(&mut shelves, name.clone(), s.short.clone(), v);
        }
    }
    Ok(shelves)
}

/// `shown`, a working tree of the module `name`, put on its shelf: among the versions of a module
/// of a registry of that name (in the order of the versions), unless the registry has its version;
/// else on a shelf of its own.
fn put(shelves: &mut Vec<Shelf>, name: String, short: Option<String>, shown: Shown) {
    match shelves.iter_mut().find(|s| s.name == name) {
        Some(s) if s.versions.iter().any(|v| v.version == shown.version) => s.shadowed.push(shown.version),
        Some(s) => {
            let at = Version::parse(&shown.version).ok();
            let pos = s.versions.iter().position(|v| match (&at, Version::parse(&v.version).ok()) {
                (Some(a), Some(b)) => b > *a,
                _ => false,
            });
            match pos {
                Some(p) => s.versions.insert(p, shown),
                None => s.versions.push(shown),
            }
        }
        None => shelves.push(Shelf { name, short, registry: false, git: None, versions: vec![shown], shadowed: Vec::new() }),
    }
}

// ── The command ─────────────────────────────────────────────────────────

/// `--alternate <lang>=<path>`: the same site in another language, by the path from this site's root.
fn alternate_of(spelled: &str) -> Result<(Lang, String), Text> {
    let wrong = || tr!("`--alternate` は `ja=<パス>` か `en=<パス>` の形で渡してください", "`--alternate` takes `ja=<path>` or `en=<path>`");
    let (lang, path) = spelled.split_once('=').ok_or_else(wrong)?;
    let lang = Lang::parse(lang).ok_or_else(wrong)?;
    // a path from this site to the other, which each page writes in a link
    if path.is_empty() || path.contains(':') || path.starts_with('/') || path.chars().any(|c| c.is_whitespace() || matches!(c, '"' | '<' | '>')) {
        return Err(wrong());
    }
    Ok((lang, path.to_string()))
}

/// `ritsu mod site <list>`: the exit code. `--out` is required, and this function is the one to say
/// so.
pub fn site(ctx: &Ctx, a: &Args) -> u8 {
    // a site reads registries with curl, git repositories with git, and signatures with cosign
    if cfg!(target_os = "wasi") {
        return ctx.refuse(ritsu_base::wasi::cannot_start_programs());
    }
    let Some(out) = a.get("--out") else {
        return ctx.refuse(tr!("書く先のディレクトリを `--out <ディレクトリ>` で渡してください", "say where the site goes with `--out <dir>`"));
    };
    let file = a.pos[0].as_str();
    let (text, dir) = match list_text(file) {
        Ok(x) => x,
        Err(t) => return ctx.refuse(t.then(&tr!("。何も書いていません", "; nothing was written"))),
    };
    let list = match read_list(file, &text, dir) {
        Ok(l) => l,
        Err(ts) => {
            let head = if ctx.lang == Lang::Ja { "エラー" } else { "error" };
            for t in &ts {
                eprintln!("{head}: {}", ctx.say(t));
            }
            return ctx.refuse(tr!("一覧を読めません。何も書いていません", "the list cannot be read; nothing was written"));
        }
    };
    let playground = match a.get("--playground") {
        Some(u) => match playground_problem(u) {
            None => u.to_string(),
            Some(t) => return ctx.refuse(t),
        },
        None => list.playground.clone().unwrap_or_else(|| if ctx.lang == Lang::Ja { PLAYGROUND_JA } else { PLAYGROUND_EN }.to_string()),
    };
    let alternate = match a.get("--alternate").map(alternate_of) {
        None => None,
        Some(Ok(x)) => Some(x),
        Some(Err(t)) => return ctx.refuse(t),
    };
    let shelves = match gather(&list, ctx.lang) {
        Ok(s) => s,
        Err(t) => return ctx.refuse(t.then(&tr!("。何も書いていません", "; nothing was written"))),
    };
    let mut w = pages::Writer { out: PathBuf::from(out), lang: ctx.lang, list: &list, playground, alternate, joined: Joined::new(), written: 0 };
    if let Err(t) = w.all(&shelves) {
        return ctx.refuse(t);
    }
    let head = if ctx.lang == Lang::Ja { "エラー" } else { "error" };
    let mut failed = false;
    for s in &shelves {
        for v in &s.versions {
            if let Some(t) = &v.failed {
                failed = true;
                let (name, version, why) = (&s.name, &v.version, ctx.say(t));
                eprintln!("{head}: {}", ctx.say(&tr!("`{name}` {version} を確かめられません。{why}", "`{name}` {version} could not be verified: {why}")));
            }
        }
        for v in &s.shadowed {
            let name = &s.name;
            eprintln!(
                "{}",
                ctx.say(&tr!(
                    "備考: `{name}` {v} はレジストリにもうあるので、作業ツリーではなくレジストリの版を載せました",
                    "note: `{name}` {v} is on its registry already, so the site shows the registry's and not the working tree's"
                ))
            );
        }
    }
    let (pages, modules) = (w.written, shelves.len());
    let (pages_said, modules_said) = (plural(pages, "page", "pages"), plural(modules, "module", "modules"));
    println!("{}", ctx.say(&tr!("ritsu mod site: モジュール {modules} 個のページを {pages} 枚、{out} に書きました", "ritsu mod site: wrote {pages_said} of {modules_said} in {out}")));
    u8::from(failed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_time_is_read_as_its_day() {
        assert_eq!(day_of(0), "1970-01-01");
        assert_eq!(day_of(1_793_312_527), "2026-10-29");
        assert_eq!(day_of(951_782_400), "2000-02-29");
        assert_eq!(day_of(1_709_164_800), "2024-02-29");
        assert_eq!(day_of(-86_400), "1969-12-31");
    }
}
