//! `ritsu mod site <list> --out <dir> [--playground <URL>]` (DESIGN 17.17): the site that lists
//! modules, written from a list of them.
//!
//! - **The list** is written in the lexis of `ritsu.mod` (DESIGN 17.3): `title`, `description`,
//!   `playground` (each once), `module "<name>"` (a module of a registry), `local "<path>"` (the
//!   working tree of a module that is not published yet, from the list's directory), and
//!   `signature` lines, which are read as `ritsu.mod` reads them and check the modules the list
//!   shows (their key files are read from the list's directory).
//! - **What is read.** For a module of a registry, the versions its registry lists, each fetched as
//!   `ritsu mod fetch <name>@<version>` fetches it, its signature checked against the list's lines
//!   that cover it, and then read as `ritsu mod diff` reads a version: as a module of its own, with
//!   its own lock, whose modules are fetched and checked against its own lines ([`fetch::listed`]).
//!   A version that does not pass is shown as not verified, and the command exits 1. For a working
//!   tree, its files, and the modules its lock pins, fetched as `ritsu mod fetch` fetches them. A
//!   working tree of a module the list also names from a registry is one more version of it, when
//!   the registry does not have its version.
//! - **The pages** (the frame of `ritsu_base::docpage`): the list, each module (its versions, how
//!   to require it, its source and license), each version (its files, its `require` line, what
//!   changed since the version before, as `ritsu mod diff --format markdown` says it, and the button
//!   that opens it in the playground), each file (the page its language's `doc` draws, run as `ritsu
//!   <language> doc <file> --format html` in the version's directory; the source for geas and every
//!   other file), and `module.json`, the files the playground opens.
//! - **The playground.** `module.json` holds the version's files and the modules its lock pins, in
//!   the form of a module's copy of the store (`.ritsu/modules/sha256/<hex>/…`), as the edits of a
//!   link of the playground hold them (`[[path, text], …]`; a file that is not UTF-8 as `[path,
//!   {"base64": "…"}]`). The button's script reads it from the same origin, packs it as the
//!   playground packs its edits (raw DEFLATE, base64url) and opens `<playground>#project=empty&
//!   edits=<…>`; past 1,000,000 characters it says that the version is too big to open in the page.
//!   A working tree that requires a module by a path has no button: the directory is not among its
//!   files.
//!
//! Everything is read and fetched before anything is written: a list that cannot be read, a
//! registry that cannot be asked, or no cosign when a line asks for one, stops the command with
//! nothing written (exit 2).

use super::{Ctx, diff, fetch, registry};
use ritsu_base::cli::Args;
use ritsu_base::docpage::{self, esc};
use ritsu_base::json::Json;
use ritsu_base::modules::lex::{self, Kind, Token};
use ritsu_base::modules::lock::{self as lockfile, Pinned};
use ritsu_base::modules::manifest::{self, Signature, Source};
use ritsu_base::modules::names::{self, ModuleName};
use ritsu_base::modules::semver::Version;
use ritsu_base::modules::store::{self, Store};
use ritsu_base::modules::{digest, vendor};
use ritsu_base::naming::Tool;
use ritsu_base::text::{Lang, Text, plural};
use ritsu_base::tr;
use ritsu_project::Joined;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The playground of ritsu's site, in each language: where a version opens when neither the
/// command line nor the list says otherwise.
const PLAYGROUND_EN: &str = "https://i2y.github.io/ritsu/play/";
const PLAYGROUND_JA: &str = "https://i2y.github.io/ritsu/ja/play/";

/// The most characters the packed files of a version may have in the link that opens it (DESIGN
/// 17.17).
const LINK_MAX: usize = 1_000_000;

/// The most lines of a difference the page of a version shows of one table (as `ritsu mod diff`).
const ROWS: usize = 200;

/// The words a line of a list starts with.
const WORDS: [&str; 6] = ["title", "description", "playground", "module", "signature", "local"];

// ── The list ────────────────────────────────────────────────────────────

/// A module the list shows.
enum Entry {
    /// From a registry.
    Module(ModuleName),
    /// A working tree: the path as written, and the directory it leads to.
    Local(String, PathBuf),
}

/// A list of modules, read.
struct List {
    title: Option<String>,
    description: Option<String>,
    playground: Option<String>,
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
    let mut list = List { title: None, description: None, playground: None, entries: Vec::new(), signatures: Vec::new(), dir };
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
                    "`{w}` で始まる行はありません。一覧の行は title、description、playground、module、signature、local のどれかで始めてください",
                    "no line starts with `{w}`; a line of a list starts with title, description, playground, module, signature or local"
                ),
            ));
            continue;
        };
        let args = &line.tokens[1..];
        if word == "signature" {
            match manifest::signature_line(args, n) {
                Ok(s) => list.signatures.push(s),
                Err((col, t)) => wrong.push(at(col, t)),
            }
            continue;
        }
        // every other line takes one string
        let Some(value) = one_string(args) else {
            wrong.push(at(head.col, tr!("`{word}` の行は `{word} \"…\"` の形で書いてください", "write the `{word}` line as `{word} \"…\"`")));
            continue;
        };
        let col = args[0].col;
        if matches!(word, "title" | "description" | "playground") {
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
            "module" => match ModuleName::parse(&value) {
                Ok(m) if m.is_published() => {
                    if list.entries.iter().any(|e| matches!(e, Entry::Module(x) if *x == m)) {
                        wrong.push(at(col, tr!("モジュール `{m}` はもう一覧にあります", "the module `{m}` is in the list already")));
                    } else {
                        list.entries.push(Entry::Module(m));
                    }
                }
                Ok(_) => wrong.push(at(
                    col,
                    tr!(
                        "`{value}` は配らないモジュールの名前です。`module` には `<レジストリ>/<リポジトリ>` を書いてください。まだ送り出していない作業ツリーは `local` で書きます",
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
                if list.entries.iter().any(|e| matches!(e, Entry::Local(_, x) if *x == d)) {
                    wrong.push(at(col, tr!("`local \"{value}\"` はもう一覧にあります", "`local \"{value}\"` is in the list already")));
                    continue;
                }
                list.entries.push(Entry::Local(value, d));
            }
        }
    }
    if list.entries.is_empty() && wrong.is_empty() {
        wrong.push(tr!("{file} には `module` の行も `local` の行もありません", "{file} has no `module` line and no `local` line"));
    }
    if wrong.is_empty() { Ok(list) } else { Err(wrong) }
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
    /// The digest of its OCI manifest, for a version of a registry.
    digest: Option<String>,
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
    /// Whether the list names it from a registry.
    registry: bool,
    versions: Vec<Shown>,
    /// The versions of the working trees of it that the list names and its registry has already.
    shadowed: Vec<String>,
}

impl Shelf {
    /// The newest version that was read.
    fn newest(&self) -> Option<&Shown> {
        self.versions.iter().rev().find(|v| v.module.is_some())
    }

    fn failed(&self) -> bool {
        self.versions.iter().any(|v| v.failed.is_some())
    }
}

/// The modules the lock of the module at `root` (whose files are `files`) pins from a registry,
/// each where it is read: the module's copy of the store first, then the store.
fn pinned(root: &Path, files: &BTreeMap<String, Vec<u8>>) -> Vec<(Store, String)> {
    let Some(bytes) = files.get(lockfile::FILE) else { return Vec::new() };
    let Ok(lock) = lockfile::parse(bytes, lockfile::FILE) else { return Vec::new() };
    let store = Store::located();
    lock.modules
        .iter()
        .filter_map(|l| match &l.pinned {
            Pinned::Oci { digest, .. } => vendor::find(root, digest, store.as_ref()).map(|(s, _)| (s, digest.clone())),
            Pinned::Path { .. } => None,
        })
        .collect()
}

/// Every version of the module `name` its registry lists, fetched and checked; or what stops the
/// command (exit 2) when the registry or cosign cannot be asked.
fn of_registry(list: &List, name: &ModuleName) -> Result<Vec<Shown>, Text> {
    let versions = registry::Remote::of(name, false).and_then(|r| r.versions()).map_err(|t| tr!("`{name}` の版の一覧を引けません。", "cannot list the versions of `{name}`: ").then(&t))?;
    let mut out = Vec::new();
    for v in versions {
        let shown = match fetch::listed(&list.dir, &list.signatures, name, &v) {
            Ok((store, d)) => {
                let root = store.files(&d);
                let module = diff::Module::at(&root, format!("{name}@{v}"), Some(d.clone()))?;
                let deps = pinned(&root, &module.files);
                Shown { version: v.to_string(), published: true, digest: Some(d), module: Some(module), deps, paths: false, failed: None }
            }
            Err((1, t)) => Shown { version: v.to_string(), published: true, digest: None, module: None, deps: Vec::new(), paths: false, failed: Some(t) },
            Err((_, t)) => return Err(tr!("`{name}` {v} を取り出せません。", "cannot fetch `{name}` {v}: ").then(&t)),
        };
        out.push(shown);
    }
    Ok(out)
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
    Ok((module.name.clone(), Shown { version, published: false, digest: None, module: Some(module), deps, paths, failed: None }))
}

/// Every module of the list, read, in the order the list names them; a working tree of a module
/// the list names from a registry is put among that module's versions.
fn gather(list: &List, lang: Lang) -> Result<Vec<Shelf>, Text> {
    let mut shelves: Vec<Shelf> = Vec::new();
    for e in &list.entries {
        match e {
            Entry::Module(m) => shelves.push(Shelf { name: m.to_string(), registry: true, versions: of_registry(list, m)?, shadowed: Vec::new() }),
            Entry::Local(written, dir) => {
                let (name, shown) = of_working_tree(dir, written, lang)?;
                let later = shelves.iter().position(|s| s.name == name).is_none() && list.entries.iter().any(|x| matches!(x, Entry::Module(m) if m.to_string() == name));
                if later {
                    // the registry's comes first, where the list names it
                    shelves.push(Shelf { name: format!("\u{0}{name}"), registry: false, versions: vec![shown], shadowed: Vec::new() });
                    continue;
                }
                put(&mut shelves, name, shown);
            }
        }
    }
    // a working tree read before the registry's module of its name, put among its versions
    let waiting: Vec<Shelf> = shelves.iter().position(|s| s.name.starts_with('\u{0}')).map(|_| shelves.extract_if(.., |s| s.name.starts_with('\u{0}')).collect()).unwrap_or_default();
    for s in waiting {
        let name = s.name.trim_start_matches('\u{0}').to_string();
        for v in s.versions {
            put(&mut shelves, name.clone(), v);
        }
    }
    Ok(shelves)
}

/// `shown`, a working tree of the module `name`, put on its shelf: among the versions of a module
/// of a registry of that name (in the order of the versions), unless the registry has its version;
/// else on a shelf of its own.
fn put(shelves: &mut Vec<Shelf>, name: String, shown: Shown) {
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
        None => shelves.push(Shelf { name, registry: false, versions: vec![shown], shadowed: Vec::new() }),
    }
}

// ── The command ─────────────────────────────────────────────────────────

/// `ritsu mod site <list>`: the exit code. `--out` is required, and this function is the one to say
/// so.
pub fn site(ctx: &Ctx, a: &Args) -> u8 {
    if cfg!(target_os = "wasi") {
        return ctx.refuse(ritsu_base::wasi::cannot_start("curl"));
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
    let shelves = match gather(&list, ctx.lang) {
        Ok(s) => s,
        Err(t) => return ctx.refuse(t.then(&tr!("。何も書いていません", "; nothing was written"))),
    };
    let mut w = Writer { out: PathBuf::from(out), lang: ctx.lang, list: &list, playground, joined: Joined::new(), written: 0 };
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
                    "注: `{name}` {v} はレジストリにもうあるので、作業ツリーではなくレジストリの版を載せました",
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

// ── The pages ───────────────────────────────────────────────────────────

/// What writes the pages.
struct Writer<'a> {
    out: PathBuf,
    lang: Lang,
    list: &'a List,
    playground: String,
    joined: Joined,
    /// How many pages were written.
    written: usize,
}

/// `s` with every byte a path of a URL does not take as it is written `%XX`: how a link names a
/// file of a module (`税/規則/付加価値税率.rule`) or a module whose name has a port in it.
fn url_path(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~' | b'/') {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{b:02X}"));
        }
    }
    o
}

/// `../` once for each part of `rel` (a directory, `/` between its parts): from it back up.
fn up(rel: &str) -> String {
    "../".repeat(rel.split('/').filter(|p| !p.is_empty()).count())
}

/// The directory of a file's path (`""` for one at the root).
fn parent_of(rel: &str) -> &str {
    rel.rsplit_once('/').map_or("", |(d, _)| d)
}

/// A count with its thousands apart, as both languages print it.
fn count(n: usize) -> String {
    ritsu_base::text::count(n as u64)
}

/// The name a module's `require` line gives it on its page: the last part of its name, in the
/// letters a dependency's name takes.
fn short_name(module: &str) -> String {
    let last = module.rsplit('/').next().unwrap_or(module);
    let mut s: String = last.chars().map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' }).collect();
    if !s.starts_with(|c: char| c.is_ascii_lowercase()) {
        s.insert(0, 'm');
    }
    if names::dependency_ok(&s).is_ok() { s } else { "dependency".to_string() }
}

/// The head of a digest a page shows, the whole in its title.
fn digest_cell(d: &str) -> String {
    format!("<code title=\"{}\">{}</code>", esc(d), esc(&diff::digest_head(d)))
}

/// The stylesheet of the pages, after the palette's variables.
const CSS: &str = "body { margin: 0; background: var(--bg); color: var(--fg); font: 15px/1.55 system-ui, -apple-system, \"Segoe UI\", sans-serif; }
main { max-width: 960px; margin: 0 auto; padding: 16px 16px 48px; }
nav.crumbs { font-size: 13px; color: var(--dim); margin-bottom: 8px; overflow-wrap: anywhere; }
h1 { font-size: 24px; margin: 8px 0 4px; overflow-wrap: anywhere; }
h2 { font-size: 18px; margin: 28px 0 8px; border-bottom: 1px solid var(--line); padding-bottom: 4px; }
h3, h4 { font-size: 15px; margin: 20px 0 6px; }
p.lead { color: var(--dim); margin-top: 0; }
a { color: var(--accent); }
code, pre { font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; font-size: 13px; }
code { background: var(--code); border-radius: 4px; padding: 1px 4px; overflow-wrap: anywhere; }
pre { background: var(--panel); border: 1px solid var(--line); border-radius: 6px; padding: 10px 12px; overflow-x: auto; }
pre code { background: none; padding: 0; overflow-wrap: normal; }
.scroll { overflow-x: auto; }
table { border-collapse: collapse; margin: 8px 0; font-size: 14px; }
th, td { border: 1px solid var(--line); padding: 4px 8px; text-align: left; vertical-align: top; }
th { background: var(--soft); }
.ok { color: var(--ok); }
.warn { color: var(--warn); }
.bad { color: var(--bad); }
.badge { display: inline-block; font-size: 12px; border: 1px solid currentColor; border-radius: 10px; padding: 0 8px; margin-left: 6px; vertical-align: middle; }
button.open { font: inherit; padding: 6px 14px; border-radius: 6px; border: 1px solid var(--accent); background: var(--accent); color: var(--bg); cursor: pointer; }
button.open:disabled { background: var(--soft); color: var(--dim); border-color: var(--line); cursor: default; }
details { border: 1px solid var(--line); border-radius: 6px; padding: 4px 10px; margin: 6px 0; }
summary { cursor: pointer; }
footer { margin-top: 40px; font-size: 12px; color: var(--dim); }
";

impl Writer<'_> {
    fn say(&self, t: Text) -> String {
        t.get(self.lang).to_string()
    }

    fn title(&self) -> String {
        self.list.title.clone().unwrap_or_else(|| self.say(tr!("ritsu のモジュール", "ritsu modules")))
    }

    /// A file at `rel` (from the site's root), its directories made.
    fn write(&mut self, rel: &str, body: &str) -> Result<(), Text> {
        let p = self.out.join(rel);
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| {
                let d = d.display();
                tr!("{d} を作れません（{e}）", "cannot make {d} ({e})")
            })?;
        }
        std::fs::write(&p, body).map_err(|e| {
            let p = p.display();
            tr!("{p} に書けません（{e}）", "cannot write {p} ({e})")
        })?;
        if rel.ends_with(".html") {
            self.written += 1;
        }
        Ok(())
    }

    /// A page of the site: the frame, the trail of links to the pages above it (`crumbs`: each a
    /// link from this page, and its text), and the body.
    fn page(&self, title: &str, crumbs: &[(String, String)], body: &str) -> String {
        let css = format!("{}{CSS}", docpage::Palette::default().css());
        let mut o = docpage::html_head(self.lang, &format!("ritsu mod site {}", env!("CARGO_PKG_VERSION")), title, &css);
        o.push_str("<main>\n");
        o.push_str(&crumbs_nav(crumbs, false));
        o.push_str(body);
        let made = self.say(tr!("`ritsu mod site` {} が書きました", "Written by `ritsu mod site` {}", env!("CARGO_PKG_VERSION")));
        o.push_str(&format!("<footer>{}</footer>\n</main>\n", inline(&made)));
        o.push_str(docpage::HTML_TAIL);
        o
    }

    /// Every page.
    fn all(&mut self, shelves: &[Shelf]) -> Result<(), Text> {
        let index = self.index(shelves);
        self.write("index.html", &index)?;
        for s in shelves {
            let page = self.module_page(s);
            self.write(&format!("{}/index.html", s.name), &page)?;
            let mut before: Option<&Shown> = None;
            for v in s.versions.iter().filter(|v| v.module.is_some()) {
                self.version_pages(s, v, before)?;
                before = Some(v);
            }
        }
        Ok(())
    }

    /// The list's `signature` lines that cover the module `name`.
    fn lines_for(&self, name: &str) -> Vec<&Signature> {
        self.list.signatures.iter().filter(|s| names::covers(&s.prefix, name)).collect()
    }

    /// What the list's lines say of a module, as a cell.
    fn verified_cell(&self, s: &Shelf) -> String {
        if s.failed() {
            return format!("<span class=\"bad\">{}</span>", esc(&self.say(tr!("確かめられない版があります", "a version could not be verified"))));
        }
        if !s.registry {
            return format!("<span class=\"warn\">{}</span>", esc(&self.say(tr!("まだ公開していません", "not published yet"))));
        }
        if self.lines_for(&s.name).is_empty() {
            inline(&self.say(tr!("一覧の `signature` の行が覆っていません", "no `signature` line of the list covers it")))
        } else {
            format!("<span class=\"ok\">{}</span>", inline(&self.say(tr!("一覧の `signature` の行で確かめました", "checked against the list's `signature` lines"))))
        }
    }

    fn index(&self, shelves: &[Shelf]) -> String {
        let title = self.title();
        let mut b = format!("<h1>{}</h1>\n", esc(&title));
        if let Some(d) = &self.list.description {
            b.push_str(&format!("<p class=\"lead\">{}</p>\n", esc(d)));
        }
        b.push_str("<div class=\"scroll\"><table>\n<tr>");
        for h in [tr!("モジュール", "Module"), tr!("説明", "Description"), tr!("いちばん新しい版", "Latest version"), tr!("署名", "Signature")] {
            b.push_str(&format!("<th>{}</th>", esc(&self.say(h))));
        }
        b.push_str("</tr>\n");
        for s in shelves {
            let newest = s.newest();
            let description = newest.and_then(|v| v.module.as_ref()).and_then(|m| m.manifest.description.as_ref()).map(|d| d.value.clone()).unwrap_or_default();
            let latest = match newest {
                Some(v) if v.published => esc(&v.version),
                Some(v) => format!("{} <span class=\"badge warn\">{}</span>", esc(&v.version), esc(&self.say(tr!("未公開", "unpublished")))),
                None => "—".to_string(),
            };
            b.push_str(&format!(
                "<tr><td><a href=\"./{}/index.html\"><code>{}</code></a></td><td>{}</td><td>{latest}</td><td>{}</td></tr>\n",
                url_path(&s.name),
                esc(&s.name),
                esc(&description),
                self.verified_cell(s)
            ));
        }
        b.push_str("</table></div>\n");
        if !self.list.signatures.is_empty() {
            b.push_str(&format!(
                "<h2>{}</h2>\n<p>{}</p>\n",
                esc(&self.say(tr!("署名の決まり", "Signatures"))),
                inline(&self.say(tr!(
                    "このサイトは、載せたモジュールの署名を次の行で確かめました。使う側の `ritsu.mod` に同じ行を書けば、取り出すときに同じように確かめます。",
                    "The site checked the signatures of the modules it shows against these lines. The same lines in a `ritsu.mod` check them the same way when they are fetched."
                )))
            ));
            let lines: Vec<String> = self.list.signatures.iter().map(|s| esc(&s.text())).collect();
            b.push_str(&format!("<pre><code>{}</code></pre>\n", lines.join("\n")));
        }
        self.page(&title, &[], &b)
    }

    fn module_page(&self, s: &Shelf) -> String {
        let crumbs = vec![(format!("{}index.html", up(&s.name)), self.title())];
        let mut b = format!("<h1><code>{}</code></h1>\n", esc(&s.name));
        let newest = s.newest();
        let manifest = newest.and_then(|v| v.module.as_ref()).map(|m| &m.manifest);
        if let Some(d) = manifest.and_then(|m| m.description.as_ref()) {
            b.push_str(&format!("<p class=\"lead\">{}</p>\n", esc(&d.value)));
        }
        if !s.registry {
            b.push_str(&format!(
                "<p class=\"warn\">{}</p>\n",
                esc(&self.say(tr!(
                    "このモジュールは、まだレジストリに送り出していません。ページは作業ツリーのファイルから作りました。",
                    "This module is not on a registry yet: the pages are made from the files of its working tree."
                )))
            ));
        }
        // the versions, newest first
        b.push_str(&format!("<h2>{}</h2>\n<div class=\"scroll\"><table>\n<tr>", esc(&self.say(tr!("版", "Versions")))));
        for h in [tr!("版", "Version"), tr!("ダイジェスト", "Digest"), tr!("ファイル", "Files"), tr!("確かめたこと", "Verified")] {
            b.push_str(&format!("<th>{}</th>", esc(&self.say(h))));
        }
        b.push_str("</tr>\n");
        for v in s.versions.iter().rev() {
            let version = if v.module.is_some() { format!("<a href=\"./{}/index.html\">{}</a>", url_path(&v.version), esc(&v.version)) } else { esc(&v.version) };
            let digest = v.digest.as_deref().map(digest_cell).unwrap_or_else(|| "—".to_string());
            let files = v.module.as_ref().map(|m| count(m.files.len())).unwrap_or_else(|| "—".to_string());
            let state = match (&v.failed, v.published) {
                (Some(t), _) => format!("<span class=\"bad\">{}</span>", inline(&self.say(tr!("確かめられません。{}", "not verified: {}", t.ja; t.en)))),
                (None, false) => format!("<span class=\"warn\">{}</span>", esc(&self.say(tr!("まだ公開していません", "not published yet")))),
                (None, true) if self.lines_for(&s.name).is_empty() => esc(&self.say(tr!("ダイジェスト", "its digests"))),
                (None, true) => format!("<span class=\"ok\">{}</span>", esc(&self.say(tr!("ダイジェストと署名", "its digests and its signature")))),
            };
            b.push_str(&format!("<tr><td>{version}</td><td>{digest}</td><td>{files}</td><td>{state}</td></tr>\n"));
        }
        b.push_str("</table></div>\n");
        // how to require it
        if let Some(v) = newest {
            let short = short_name(&s.name);
            b.push_str(&format!("<h2>{}</h2>\n", esc(&self.say(tr!("足し方", "Requiring it")))));
            let name = &s.name;
            if v.published {
                b.push_str(&format!("<pre><code>{}</code></pre>\n", esc(&format!("ritsu mod add {short} {name}@{}", v.version))));
                b.push_str(&format!(
                    "<p>{}</p>\n",
                    inline(&self.say(tr!(
                        "`ritsu.mod` に次の `require` の行が入り、ファイルからは `@{short}/…` で指せます。署名を確かめるなら、`signature` の行も書いてください。",
                        "It writes this `require` line in `ritsu.mod`, and the files name the module's files `@{short}/…`. To check its signature, write the `signature` line too."
                    )))
                ));
                let mut lines = vec![format!("require {short} oci {} {}", lex::quote(name), v.version)];
                lines.extend(self.lines_for(name).iter().map(|l| l.text()));
                b.push_str(&format!("<pre><code>{}</code></pre>\n", esc(&lines.join("\n"))));
            } else {
                b.push_str(&format!(
                    "<p>{}</p>\n",
                    inline(&self.say(tr!("公開したあとは `ritsu mod add {short} {name}@<版>` で足せます。", "Once it is published, `ritsu mod add {short} {name}@<version>` requires it.")))
                ));
            }
        }
        // its source and its license
        let source = manifest.and_then(|m| m.source.as_ref()).map(|x| x.value.clone());
        let license = manifest.and_then(|m| m.license.as_ref()).map(|x| x.value.clone());
        if source.is_some() || license.is_some() {
            b.push_str(&format!("<h2>{}</h2>\n<ul>\n", esc(&self.say(tr!("ソースとライセンス", "Source and license")))));
            if let Some(u) = source {
                b.push_str(&format!("<li>{}: <a href=\"{}\">{}</a></li>\n", esc(&self.say(tr!("ソース", "Source"))), esc(&u), esc(&u)));
            }
            if let Some(l) = license {
                b.push_str(&format!("<li>{}: <code>{}</code></li>\n", esc(&self.say(tr!("ライセンス", "License"))), esc(&l)));
            }
            b.push_str("</ul>\n");
        }
        self.page(&s.name, &crumbs, &b)
    }

    /// The page of a version, the pages of its files, and its `module.json`.
    fn version_pages(&mut self, s: &Shelf, v: &Shown, before: Option<&Shown>) -> Result<(), Text> {
        let Some(m) = &v.module else { return Ok(()) };
        let dir = format!("{}/{}", s.name, v.version);
        let crumbs = vec![(format!("{}index.html", up(&dir)), self.title()), ("../index.html".to_string(), s.name.clone())];
        let mut b = format!("<h1><code>{}</code> {}", esc(&s.name), esc(&v.version));
        if !v.published {
            b.push_str(&format!(" <span class=\"badge warn\">{}</span>", esc(&self.say(tr!("未公開", "unpublished")))));
        }
        b.push_str("</h1>\n");
        if let Some(d) = &m.manifest.description {
            b.push_str(&format!("<p class=\"lead\">{}</p>\n", esc(&d.value)));
        }
        if let Some(d) = &v.digest {
            b.push_str(&format!("<p>{}: <code>{}@{}</code></p>\n", esc(&self.say(tr!("参照", "Reference"))), esc(&s.name), esc(d)));
            let short = short_name(&s.name);
            b.push_str(&format!("<pre><code>{}</code></pre>\n", esc(&format!("require {short} oci {} {}", lex::quote(&s.name), v.version))));
        }
        b.push_str(&self.button(v));
        // the files
        b.push_str(&format!("<h2>{}</h2>\n<div class=\"scroll\"><table>\n<tr>", esc(&self.say(tr!("ファイル", "Files")))));
        for h in [tr!("ファイル", "File"), tr!("言語", "Language"), tr!("バイト", "Bytes")] {
            b.push_str(&format!("<th>{}</th>", esc(&self.say(h))));
        }
        b.push_str("</tr>\n");
        for (rel, bytes) in &m.files {
            let language = ritsu_project::project::kind_of(rel).map(|t| t.word()).unwrap_or("—");
            b.push_str(&format!(
                "<tr><td><a href=\"./files/{}.html\"><code>{}</code></a></td><td>{}</td><td>{}</td></tr>\n",
                url_path(rel),
                esc(rel),
                esc(language),
                count(bytes.len())
            ));
        }
        b.push_str("</table></div>\n");
        // what changed since the version before
        match before.and_then(|p| p.module.as_ref().map(|pm| (p, pm))) {
            Some((p, pm)) => {
                let d = diff::modules(pm, m, None, self.lang, &self.joined);
                let pv = &p.version;
                b.push_str(&format!("<h2>{}</h2>\n", esc(&self.say(tr!("{pv} からの差", "Changes since {pv}")))));
                b.push_str(&md_html(&diff::body_markdown(&d, self.lang, 3, ROWS)));
            }
            None => b.push_str(&format!(
                "<h2>{}</h2>\n<p>{}</p>\n",
                esc(&self.say(tr!("差", "Changes"))),
                esc(&self.say(tr!("このサイトに載せたいちばん古い版です。", "This is the oldest version the site shows.")))
            )),
        }
        let page = self.page(&format!("{} {}", s.name, v.version), &crumbs, &b);
        self.write(&format!("{dir}/index.html"), &page)?;
        for (rel, bytes) in &m.files {
            let page = self.file_page(s, v, m, rel, bytes);
            self.write(&format!("{dir}/files/{rel}.html"), &page)?;
        }
        if !v.paths {
            let json = module_json(m, &v.deps)?;
            self.write(&format!("{dir}/module.json"), &json)?;
        }
        Ok(())
    }

    /// The button that opens a version in the playground, with its script; or why it has none.
    fn button(&self, v: &Shown) -> String {
        if v.paths {
            return format!(
                "<p class=\"warn\">{}</p>\n",
                inline(&self.say(tr!(
                    "この作業ツリーは `require … path` でディレクトリのモジュールを読むので、ブラウザで試すページでは開けません。そのディレクトリは、このモジュールのファイルに入っていないからです。",
                    "This working tree requires a module by a path, so the playground cannot open it: the directory is not among the module's files."
                )))
            );
        }
        let label = self.say(tr!("ブラウザで試すページで開く", "Open in the playground"));
        let big = self.say(tr!("大きすぎて、ブラウザで試すページでは開けません", "Too big to open in the playground"));
        let failed = self.say(tr!("ブラウザで試すページに渡せませんでした", "Could not hand the files to the playground"));
        let note = self.say(tr!(
            "ファイルはリンクの `#` のあとに入れて運びます。ブラウザはそこをサーバーに送りません。",
            "The files go in the link, after its `#`, which a browser does not send to a server."
        ));
        format!(
            "<p><button type=\"button\" class=\"open\" id=\"open\" data-playground=\"{}\" data-big=\"{}\" data-failed=\"{}\">{}</button></p>\n<p class=\"lead\">{}</p>\n<script>\n{}</script>\n",
            esc(&self.playground),
            esc(&big),
            esc(&failed),
            esc(&label),
            inline(&note),
            docpage::script_text(&OPEN.replace("LINK_MAX", &LINK_MAX.to_string()))
        )
    }

    /// The page of one file: the page its language's `doc` draws, or its source.
    fn file_page(&self, s: &Shelf, v: &Shown, m: &diff::Module, rel: &str, bytes: &[u8]) -> String {
        let in_files = format!("files/{}", parent_of(rel));
        let to_version = up(&in_files);
        let crumbs = vec![
            (format!("{}index.html", up(&format!("{}/{}/{in_files}", s.name, v.version))), self.title()),
            (format!("{to_version}../index.html"), s.name.clone()),
            (format!("{to_version}index.html"), v.version.clone()),
        ];
        let mut said = String::new();
        if let Some(t) = ritsu_project::project::kind_of(rel).filter(|t| has_doc(*t)) {
            match doc_of(&m.root, t, rel, self.lang) {
                // the page the language draws, with the way back to the version above it
                Ok(html) => return html.replacen("<body>\n", &format!("<body>\n{}", crumbs_nav(&crumbs, true)), 1),
                Err(e) => said = e,
            }
        }
        let mut b = format!("<h1><code>{}</code></h1>\n", esc(rel));
        if !said.is_empty() {
            b.push_str(&format!(
                "<p class=\"bad\">{}</p>\n<pre><code>{}</code></pre>\n",
                esc(&self.say(tr!("このファイルのページを作れませんでした。言語の出力：", "The page of this file could not be made. Its language said:"))),
                esc(&said)
            ));
        }
        match std::str::from_utf8(bytes) {
            Ok(text) => b.push_str(&format!("<pre><code>{}</code></pre>\n", esc(text))),
            Err(_) => {
                let (n, d) = (count(bytes.len()), digest::of(bytes));
                b.push_str(&format!(
                    "<p>{}</p>\n",
                    inline(&self.say(tr!(
                        "UTF-8 のテキストではないファイルです（{n} バイト、`{d}`）。ブラウザで試すページには、バイトのまま渡します。",
                        "This file is not UTF-8 text ({n} bytes, `{d}`). The playground is given its bytes as they are."
                    )))
                ));
            }
        }
        self.page(rel, &crumbs, &b)
    }
}

/// The trail of links to the pages above a page (`crumbs`: each a link from the page, and its
/// text), styled where it stands (`own`) on a page a language drew, whose stylesheet has none for
/// it.
fn crumbs_nav(crumbs: &[(String, String)], own: bool) -> String {
    if crumbs.is_empty() {
        return String::new();
    }
    let links: Vec<String> = crumbs.iter().map(|(href, text)| format!("<a href=\"{}\">{}</a>", esc(href), esc(text))).collect();
    let style = if own { " style=\"max-width: 960px; margin: 8px auto 0; padding: 0 16px; font: 13px/1.5 system-ui, sans-serif; overflow-wrap: anywhere;\"" } else { "" };
    format!("<nav class=\"crumbs\"{style}>{}</nav>\n", links.join(" / "))
}

/// Whether a language draws a page of a file with `doc` (DESIGN 4.8).
fn has_doc(t: Tool) -> bool {
    matches!(t, Tool::Rulec | Tool::Dandori | Tool::Koyomi | Tool::Chobo | Tool::Yuen | Tool::Sakai | Tool::Sekisho | Tool::Kaname)
}

/// The HTML page `ritsu <language> doc <rel> --format html` prints, run in `root`; or what it said
/// when it drew none.
fn doc_of(root: &Path, t: Tool, rel: &str, lang: Lang) -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut c = std::process::Command::new(exe);
    c.current_dir(root).args([t.word(), "doc", rel, "--format", "html", "--lang", lang.code()]);
    if t == Tool::Sekisho {
        c.args(["--root", "."]);
    }
    let o = c.output().map_err(|e| e.to_string())?;
    let out = String::from_utf8_lossy(&o.stdout).into_owned();
    if o.status.success() && out.trim_start().to_ascii_lowercase().starts_with("<!doctype html>") {
        Ok(out)
    } else {
        Err(format!("{out}{}", String::from_utf8_lossy(&o.stderr)))
    }
}

/// `module.json` (DESIGN 17.17): the files of the version, and the modules its lock pins in the form
/// of a module's copy of the store, `[[path, text], …]`, a file that is not UTF-8 as `[path,
/// {"base64": "…"}]`.
fn module_json(m: &diff::Module, deps: &[(Store, String)]) -> Result<String, Text> {
    let mut items: Vec<Json> = m.files.iter().map(|(p, b)| item(p, b)).collect();
    for (from, d) in deps {
        let hex = d.strip_prefix("sha256:").unwrap_or(d);
        let at = format!("{}/sha256/{hex}", store::VENDOR);
        let unreadable = || tr!("置き場の項目 {d} を読めません", "cannot read the entry {d} of the store");
        let entry = from.entry(d);
        let manifest_bytes = std::fs::read(entry.join("manifest.json")).map_err(|_| unreadable())?;
        let module = from.module_bytes(d).ok_or_else(unreadable)?;
        let record = std::fs::read(entry.join("record.json")).map_err(|_| unreadable())?;
        items.push(item(&format!("{at}/manifest.json"), &manifest_bytes));
        items.push(item(&format!("{at}/{}", manifest::FILE), &module));
        items.push(item(&format!("{at}/record.json"), &record));
        for (p, b) in digest::files_in(&from.files(d))? {
            items.push(item(&format!("{at}/files/{p}"), &b));
        }
    }
    Ok(Json::arr(items).compact() + "\n")
}

/// One file of `module.json`.
fn item(path: &str, bytes: &[u8]) -> Json {
    match std::str::from_utf8(bytes) {
        Ok(t) => Json::arr([Json::str(path), Json::str(t)]),
        Err(_) => Json::arr([Json::str(path), Json::obj([("base64", Json::str(ritsu_base::sources::base64_encode(bytes)))])]),
    }
}

/// The script of the button of a version's page: `module.json` read from beside the page, packed as
/// the playground packs the edits of its links (its JSON compressed as raw DEFLATE by the browser's
/// CompressionStream, in base64url with no padding), and the playground opened on it; past
/// `LINK_MAX` characters the button says the version is too big.
const OPEN: &str = r##"document.getElementById("open").addEventListener("click", async (e) => {
  const b = e.currentTarget;
  b.disabled = true;
  try {
    const files = await (await fetch("module.json")).json();
    const stream = new Blob([JSON.stringify(files)]).stream().pipeThrough(new CompressionStream("deflate-raw"));
    const bytes = new Uint8Array(await new Response(stream).arrayBuffer());
    let bin = "";
    for (let i = 0; i < bytes.length; i += 0x8000) bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
    const packed = btoa(bin).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
    if (packed.length > LINK_MAX) {
      b.textContent = b.dataset.big;
      return;
    }
    location.assign(b.dataset.playground + "#project=empty&edits=" + packed);
  } catch (err) {
    b.textContent = b.dataset.failed;
  }
});
"##;

// ── Markdown, as the pages show it ──────────────────────────────────────

/// A text of a page with its `` `code` `` spans made code, its `**strong**` strong, and the rest
/// escaped.
fn inline(s: &str) -> String {
    let mut o = String::new();
    for (i, part) in s.split('`').enumerate() {
        if i % 2 == 1 {
            o.push_str(&format!("<code>{}</code>", esc(part)));
            continue;
        }
        let e = esc(part);
        let mut rest = e.as_str();
        while let Some(a) = rest.find("**") {
            let Some(z) = rest[a + 2..].find("**") else { break };
            o.push_str(&rest[..a]);
            o.push_str(&format!("<strong>{}</strong>", &rest[a + 2..a + 2 + z]));
            rest = &rest[a + 2 + z + 2..];
        }
        o.push_str(rest);
    }
    o
}

/// The cells of a row of a table, `\|` a `|` in a cell.
fn cells(line: &str) -> Vec<String> {
    let inner = line.trim().trim_start_matches('|');
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                cur.push('|');
                chars.next();
            }
            '|' => out.push(std::mem::take(&mut cur).trim().to_string()),
            c => cur.push(c),
        }
    }
    out.push(cur.trim().to_string());
    out
}

/// What [`md_html`] holds until a line ends it.
#[derive(Default)]
struct Held {
    para: Vec<String>,
    items: Vec<(usize, String)>,
    rows: Vec<Vec<String>>,
}

impl Held {
    fn para(&mut self, o: &mut String) {
        if !self.para.is_empty() {
            o.push_str(&format!("<p>{}</p>\n", inline(&self.para.join(" "))));
            self.para.clear();
        }
    }

    fn items(&mut self, o: &mut String) {
        if self.items.is_empty() {
            return;
        }
        o.push_str("<ul>\n");
        let mut deep = false;
        for (depth, text) in &self.items {
            if *depth > 0 && !deep {
                o.push_str("<ul>\n");
                deep = true;
            } else if *depth == 0 && deep {
                o.push_str("</ul>\n");
                deep = false;
            }
            o.push_str(&format!("<li>{}</li>\n", inline(text)));
        }
        if deep {
            o.push_str("</ul>\n");
        }
        o.push_str("</ul>\n");
        self.items.clear();
    }

    fn rows(&mut self, o: &mut String) {
        if self.rows.is_empty() {
            return;
        }
        o.push_str("<div class=\"scroll\"><table>\n");
        for (i, r) in self.rows.iter().enumerate() {
            let tag = if i == 0 { "th" } else { "td" };
            o.push_str("<tr>");
            for c in r {
                o.push_str(&format!("<{tag}>{}</{tag}>", inline(c)));
            }
            o.push_str("</tr>\n");
        }
        o.push_str("</table></div>\n");
        self.rows.clear();
    }

    fn all(&mut self, o: &mut String) {
        self.para(o);
        self.items(o);
        self.rows(o);
    }
}

/// The Markdown `ritsu mod diff` writes (headings, paragraphs, lists of two levels, tables, fenced
/// code, and `<details>` lines, which are HTML already), as HTML.
fn md_html(md: &str) -> String {
    let mut o = String::new();
    let mut h = Held::default();
    let mut fence: Option<Vec<String>> = None;
    for line in md.lines() {
        if let Some(code) = fence.as_mut() {
            if line.trim_start().starts_with("```") {
                o.push_str(&format!("<pre><code>{}</code></pre>\n", esc(&code.join("\n"))));
                fence = None;
            } else {
                code.push(line.to_string());
            }
            continue;
        }
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            h.all(&mut o);
            fence = Some(Vec::new());
        } else if line.starts_with("<details>") || line.starts_with("</details>") {
            h.all(&mut o);
            o.push_str(line);
            o.push('\n');
        } else if trimmed.is_empty() {
            h.all(&mut o);
        } else if line.starts_with('#') {
            h.all(&mut o);
            let level = line.chars().take_while(|c| *c == '#').count().min(6);
            o.push_str(&format!("<h{level}>{}</h{level}>\n", inline(line[level..].trim())));
        } else if line.starts_with('|') {
            h.para(&mut o);
            h.items(&mut o);
            let c = cells(line);
            if !c.iter().all(|x| !x.is_empty() && x.chars().all(|ch| matches!(ch, '-' | ':'))) {
                h.rows.push(c);
            }
        } else if let Some(text) = trimmed.strip_prefix("- ") {
            h.para(&mut o);
            h.rows(&mut o);
            h.items.push((usize::from(line.len() - trimmed.len() >= 2), text.to_string()));
        } else {
            h.items(&mut o);
            h.rows(&mut o);
            h.para.push(trimmed.to_string());
        }
    }
    if let Some(code) = fence {
        o.push_str(&format!("<pre><code>{}</code></pre>\n", esc(&code.join("\n"))));
    }
    h.all(&mut o);
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_markdown_of_a_diff_as_html() {
        let md = "- Files: 2 changed\n  - one `a|b`\n\n| Change | File |\n|---|---|\n| changed | `x\\|y` |\n\n<details><summary><code>r.rule</code> (rulec): differ</summary>\n\n**Version diff**\n\nweight >=2g\n\n</details>\n";
        let h = md_html(md);
        assert!(h.contains("<ul>\n<li>Files: 2 changed</li>\n<ul>\n<li>one <code>a|b</code></li>\n</ul>\n</ul>\n"), "{h}");
        assert!(h.contains("<tr><th>Change</th><th>File</th></tr>\n<tr><td>changed</td><td><code>x|y</code></td></tr>"), "{h}");
        assert!(h.contains("<details><summary><code>r.rule</code> (rulec): differ</summary>\n<p><strong>Version diff</strong></p>\n<p>weight &gt;=2g</p>\n</details>"), "{h}");
    }

    #[test]
    fn names_and_paths_as_links_take_them() {
        assert_eq!(url_path("税/a b.rule"), "%E7%A8%8E/a%20b.rule");
        assert_eq!(url_path("127.0.0.1:5000/acme/tax"), "127.0.0.1%3A5000/acme/tax");
        assert_eq!(up("a/b/c"), "../../../");
        assert_eq!(parent_of("rules/a.rule"), "rules");
        assert_eq!(parent_of("a.rule"), "");
        assert_eq!(short_name("ghcr.io/i2y/ritsu-holidays"), "ritsu_holidays");
        assert_eq!(short_name("example.com/acme/tax"), "tax");
        assert_eq!(count(1_234_567), "1,234,567");
        assert_eq!(count(12), "12");
    }
}
