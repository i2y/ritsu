//! ritsu's site (website/, DESIGN 13.2): ritsu's own pages in English and Japanese, and below them
//! the site of each language that website/build.sh builds, each with its own configuration in a
//! directory of its own (website/rulec and website/dandori, the sites the two languages had in
//! their own repositories, and website/chobo), all built into one tree, website/build, which is
//! published at https://i2y.github.io/ritsu/. This holds:
//!
//! - every relative link of ritsu's pages leads to one of its pages, or to the site of a language
//!   that build.sh builds, read as Zensical reads it (a link that names no `.md` from the directory
//!   of the page's file, and none climbing out of the site), and every `#anchor` to a heading of the
//!   page it names, or, on a link to the playground, to a project the playground lists in the
//!   page's language (`#project=shop&file=gates/orders.gate`); a page may be an HTML page the site
//!   publishes as it is, as the page of the playground alone (website/docs/play/index.html and the
//!   Japanese one), whose own links lead to the page that explains the playground and to its files;
//! - the English pages and the Japanese ones are the same pages, the two navs list them in the same
//!   order (all but the page of yuen's namespace), and each home page links to every other page its
//!   nav lists;
//! - the pages of sekisho (website/docs/sekisho.md and website/docs-ja/sekisho.md) show sekisho's
//!   example as the tools have it: the lines of its files, what the commands print (a `gen` into a
//!   scratch directory), the Cedar and the code `ritsu gen` writes, the table `sekisho doc` draws
//!   and the count of combinations it says, what `sekisho check` says of each edit a ```diff
//!   shows, and what `sekisho diff` prints of two versions of a gate of its tests; the two show the
//!   same kinds of blocks and tables, as many of each; and sekisho's READMEs link to them;
//! - the pages of koyomi (website/docs/languages/koyomi.md and the Japanese one) show what
//!   `koyomi diff` prints of two versions of a calendar of its tests, and the commands their prose
//!   names; the two show the same kinds of blocks;
//! - the guide to modules (website/docs/modules.md and website/docs-ja/modules.md) shows the modules
//!   of ritsu's tests and the module of holidays as the tools have them: the lines of the files, a
//!   block marked with the file it is (`<!-- … -->` above its fence), what the commands print, and
//!   the commands its prose names; the two pages show the same kinds of blocks and tables, as many
//!   of each, under the same eight headings;
//! - the pages of kaname (website/docs/kaname/ and website/docs-ja/kaname/) show its examples and the
//!   materials of its tests as the tools have them: the lines of the `.kn` files, what the commands
//!   print (a `gen` into a scratch directory), and the lines of the hosts of its examples in
//!   TypeScript, Python and Go with what they print (`host/output.txt`), the page of the examples
//!   showing every host of the four functional cores; a page in English and the same page in
//!   Japanese show the same kinds of blocks and tables, as many of each; and kaname's READMEs and the
//!   READMEs at the root link to them;
//! - every link into ritsu's repository, on ritsu's pages and on the pages of every language's
//!   site, names a file or a directory that is there, and no page links to the repository of one
//!   language by itself, its releases included (ritsu's releases hand out every language);
//! - the code on the index pages is the lines of files of the languages, `ritsu check` prints what
//!   they show under it, and a command the prose names is one;
//! - build.sh builds rulec's, dandori's and chobo's sites; both index pages link to the site of
//!   every language build.sh builds, and each such site is there, with its own build.sh and
//!   configurations;
//! - the configurations name the URL each site is published at, under /ritsu/, and ritsu's
//!   repository;
//! - .github/workflows/docs.yml runs on a push to main that changes what the site is built from,
//!   and by hand, and installs the cosign tools.yml installs, which the list of modules checks the
//!   seed module's signatures with;
//! - the nav and the home page of each language's site lead to ritsu's playground in the site's
//!   language, on a project the playground lists in that language; and the pages the playgrounds of
//!   rulec's and dandori's sites had (`playground.md` in each language) send a reader on to the same
//!   place (tests/playground.rs follows them in Chrome);
//! - the pages of a language's namespace (website/docs/ns/yuen.md and website/docs-ja/ns/yuen.md,
//!   which the IRIs of yuen's PROV words open: `https://i2y.github.io/ritsu/ns/yuen#Requirement`)
//!   say every word yuen writes and no other, each once, under the heading of its kind, with the
//!   types it is written on, as the id the IRI's `#` names; and the example on them is what the
//!   command prints;
//! - and, when Zensical is in website/.venv, build.sh builds the tree that is published, in a copy
//!   of website/: the index pages of ritsu's site and of each language's, in both languages, the
//!   playground, the guide to modules and the list of modules that `ritsu mod site` writes from
//!   website/modules.list (with the ritsu of these tests, and the seed module's working tree in
//!   place of its version on ghcr.io, as the test has no network), the pages of sekisho, the pages
//!   of kaname (with a set of tabs for the hosts of each functional core on the page of the
//!   examples), the pages the languages' playgrounds had, each with nothing beside it and its link
//!   and the nav of its site leading to the playground in the tree, the marketplace of Claude Code
//!   as it is written, and a file of the tree for every relative link of ritsu's built pages, none
//!   of which climbs out of /ritsu/; and the code of the
//!   languages on ritsu's pages is coloured by their lexers (website/tools/knlexer.py,
//!   gatelexer.py and cedarlexer.py, and the lexers of rulec's, dandori's and chobo's sites): every
//!   block of `kn`, `rule`, `flow`, `book`, `gate`, `cedar` and `cedarschema` is built as its
//!   language, with Pygments' classes on its words and none the lexer could not read, and so is
//!   every block of a gate or of Cedar on the pages of a language's site; the pages of kaname show
//!   a keyword, a type, a number with its unit, a date, a comment, a string and an operator, each in
//!   its class, and the pages of sekisho the kinds of words of a gate, of Cedar's policies and of a
//!   schema, each in its class. Without Zensical the test says SKIP;
//! - and the lexers of a gate and of Cedar read every `.gate` of the repository that sekisho's lexer
//!   reads, every policy and schema the official Cedar CLI reads among the materials of the tests,
//!   what it wrote for them (`cedar format`), and the Cedar sekisho writes for its examples, with
//!   nothing they could not read; and of the policies and schemas the CLI does not read, they mark
//!   what it could not read in each it says has an invalid token, and nothing in the others, whose
//!   tokens it read. Without Pygments in website/.venv the test says SKIP.
//!
//! (tests/common/mod.rs holds what this file shares with tests/readme.rs.)

mod common;

use common::{
    Block, Page, code_from_files, commands_named, hosts_from_files, inline_code, kaname_hosts, lines_in_order, lines_of, link_targets, links, marked_files, page,
    root, run_the_consoles, sh, shown_lines, trimmed,
};
use ritsu_testkit::{Need, TempDir, ready};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use yuen::export::prov::{Kind, TERMS, YUEN_NS};

/// Where the site is published.
const PUBLISHED: &str = "https://i2y.github.io/ritsu/";

/// The sites of the languages that ritsu's site holds: the ones rulec and dandori had in their own
/// repositories, moved into website/<language> (DESIGN 13.2), and chobo's, written in website/chobo.
const HELD: [&str; 3] = ["rulec", "dandori", "chobo"];

fn website() -> PathBuf {
    root().join("website")
}

fn read(p: &Path) -> String {
    fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// The sites of the languages that build.sh builds: the names of its `sites=(…)`.
fn sites() -> Vec<String> {
    let text = read(&website().join("build.sh"));
    let line = text.lines().find_map(|l| l.trim().strip_prefix("sites=(")).expect("build.sh names the sites it builds as sites=(…)");
    let names: Vec<String> = line.trim_end_matches(')').split_whitespace().map(String::from).collect();
    assert!(!names.is_empty(), "build.sh builds no site of a language");
    names
}

/// ritsu's pages, each with the path of its directory in the built site (`/`, `/playground/`,
/// `/ns/yuen/`, `/ja/`, …). A page in a directory of `docs` is published under that directory
/// (`ns/yuen.md` at `/ns/yuen/`).
fn pages() -> Vec<(Page, String)> {
    let mut out = Vec::new();
    for (dir, at) in [("docs", "/"), ("docs-ja", "/ja/")] {
        let mut todo = vec![PathBuf::new()];
        while let Some(under) = todo.pop() {
            for e in fs::read_dir(website().join(dir).join(&under)).unwrap().flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if e.file_type().unwrap().is_dir() {
                    todo.push(under.join(&name));
                    continue;
                }
                let Some(stem) = name.strip_suffix(".md") else { continue };
                let path = under.join(stem).to_string_lossy().into_owned();
                let url = if stem == "index" { format!("{at}{}", if under.as_os_str().is_empty() { String::new() } else { format!("{}/", under.display()) }) } else { format!("{at}{path}/") };
                out.push((page(&format!("website/{dir}/{}", under.join(&name).display())), url));
            }
        }
    }
    out.sort_by(|a, b| a.0.name.cmp(&b.0.name));
    out
}

/// A path with `.` and `..` worked out, from the root: `/a/b/../c/` is `/a/c/`.
fn normal(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for p in path.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            p => parts.push(p),
        }
    }
    let mut out = format!("/{}", parts.join("/"));
    if path.ends_with('/') && out != "/" {
        out.push('/');
    }
    out
}

/// A relative path followed from a directory of the built site (`/`, `/ja/`, `/sekisho/`): the
/// path it leads to, or `None` when it climbs out of the site, which is published under /ritsu/ and
/// has nothing of its own above its root.
fn follow(base: &str, rel: &str) -> Option<String> {
    let mut parts: Vec<&str> = base.split('/').filter(|p| !p.is_empty()).collect();
    for p in rel.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            p => parts.push(p),
        }
    }
    let mut out = format!("/{}", parts.join("/"));
    if rel.ends_with('/') && out != "/" {
        out.push('/');
    }
    Some(out)
}

/// The directory of the built site that a page's file is in (`/`, `/ja/`, `/ns/`): Zensical reads a
/// relative link that names no page (`rulec/`, `playground/#project=…`) from there, and writes it,
/// in a page it publishes a directory deeper (`sekisho.md` at `/sekisho/`), with one more `../`.
fn dir_of(name: &str) -> String {
    let (rest, at) = name.strip_prefix("website/docs-ja/").map(|r| (r, "/ja/")).or_else(|| name.strip_prefix("website/docs/").map(|r| (r, "/"))).unwrap_or_else(|| panic!("{name} is no page of ritsu's site"));
    rest.rsplit_once('/').map_or(at.to_string(), |(d, _)| format!("{at}{d}/"))
}

/// The Markdown a path of the built site is made from: ritsu's pages at the root and under `/ja/`,
/// a language's site under `/<language>/` and `/<language>/ja/`; and the list of modules under
/// `/modules/list/` and `/ja/modules/list/`, which `ritsu mod site` writes from website/modules.list
/// as build.sh builds the site (DESIGN 17.17).
fn source_of(url: &str, sites: &[String]) -> Option<PathBuf> {
    let mut parts: Vec<&str> = url.split('/').filter(|p| !p.is_empty()).collect();
    if parts.starts_with(&["modules", "list"]) || parts.starts_with(&["ja", "modules", "list"]) {
        return Some(website().join("modules.list"));
    }
    let mut base = website();
    if let Some(first) = parts.first().copied()
        && sites.iter().any(|s| s == first)
    {
        base = base.join(first);
        parts.remove(0);
    }
    let docs = if parts.first() == Some(&"ja") {
        parts.remove(0);
        "docs-ja"
    } else {
        "docs"
    };
    let d = base.join(docs);
    let rest = parts.join("/");
    let candidates = if rest.is_empty() { vec![d.join("index.md")] } else { vec![d.join(format!("{rest}.md")), d.join(&rest).join("index.md"), d.join(&rest).join("index.html"), d.join(&rest)] };
    candidates.into_iter().find(|p| p.is_file())
}

/// The headings of a page, anchored as the site anchors them (the same as GitHub, `toc.slugify` in
/// the configurations).
fn anchors_of(file: &Path) -> Vec<String> {
    page(&file.strip_prefix(root()).unwrap().display().to_string()).anchors
}

#[test]
fn the_links_of_ritsus_pages_lead_to_pages_headings_and_sites() {
    let sites = sites();
    let mut wrong = Vec::new();
    let mut seen = 0;
    for (p, _) in pages() {
        let dir = Path::new(&p.name).parent().unwrap().to_path_buf();
        for (line, target) in link_targets(&p.text) {
            if target.starts_with("https://") || target.starts_with("http://") {
                continue;
            }
            seen += 1;
            let (file, frag) = target.split_once('#').map_or((target.as_str(), None), |(f, a)| (f, Some(a)));
            let to = if file.is_empty() {
                Some(root().join(&p.name))
            } else if file.ends_with(".md") {
                // A page of the same site, as the Markdown names it.
                let at = root().join(normal(&format!("/{}/{file}", dir.display())).trim_start_matches('/'));
                at.is_file().then_some(at)
            } else {
                // A path of the built site, from the directory of the page's file, as Zensical
                // reads it; one that climbs out of the site leads nowhere.
                follow(&dir_of(&p.name), file).and_then(|at| source_of(&at, &sites))
            };
            match (to, frag) {
                (None, _) => wrong.push(format!("{}:{line}: `{target}` leads to no page of the site", p.name)),
                // A link that opens the playground on a project (`#project=shop&file=…`), which
                // the playground reads and no heading is anchored by.
                (Some(at), Some(a)) if a.contains('=') => {
                    let ja = p.name.starts_with("website/docs-ja/");
                    let playground = website().join(if ja { "docs-ja/playground.md" } else { "docs/playground.md" });
                    let alone = website().join(if ja { "docs-ja/play/index.html" } else { "docs/play/index.html" });
                    if (at != playground && at != alone) || !opens_a_project(a, ja) {
                        wrong.push(format!("{}:{line}: `{target}` opens no project the playground of the page's language lists", p.name));
                    }
                }
                (Some(at), Some(a)) if !anchors_of(&at).iter().any(|h| h == a) => {
                    wrong.push(format!("{}:{line}: `{target}`: no heading of {} is anchored `#{a}`", p.name, at.strip_prefix(root()).unwrap().display()))
                }
                _ => {}
            }
        }
    }
    // The playground and the site of each language, from the two index pages.
    assert!(seen >= 4, "{seen} relative links on ritsu's pages: were they changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The page of the playground alone, in each language: its file and the directory the site publishes
/// it at (it is copied as it is).
const ALONE: [(&str, &str); 2] = [("website/docs/play/index.html", "/play/"), ("website/docs-ja/play/index.html", "/ja/play/")];

/// The page of the playground alone is an HTML page of its own, which the site publishes as it is:
/// every relative link and source of it leads, from where it is published, to the page that explains
/// the playground in its language or to a file of the playground (the Japanese site's are the English
/// site's, which sync.sh copies), and the pages that explain the playground link to it.
#[test]
fn the_page_of_the_playground_alone_leads_to_the_one_that_explains_it() {
    let sites = sites();
    let mut seen = 0;
    for (file, at) in ALONE {
        let html = read(&root().join(file));
        let ja = at.starts_with("/ja/");
        let mut explained = false;
        for target in targets_in(&html) {
            assert!(!target.contains("://") && !target.starts_with('/'), "{file}: `{target}` leaves the site; the page loads nothing from elsewhere");
            let path = target.split(['#', '?']).next().unwrap();
            let to = follow(at, path).unwrap_or_else(|| panic!("{file}: `{target}` climbs out of the site"));
            let found = source_of(&to, &sites).or_else(|| to.strip_prefix("/ja/playground/").map(|f| website().join("docs/playground").join(f)).filter(|p| p.is_file()));
            let found = found.unwrap_or_else(|| panic!("{file}: `{target}` leads to {to}, which is no page or file of the site"));
            explained |= found == website().join(if ja { "docs-ja/playground.md" } else { "docs/playground.md" });
            seen += 1;
        }
        assert!(explained, "{file} does not link to the page that explains the playground");
        let page = if ja { "website/docs-ja/playground.md" } else { "website/docs/playground.md" };
        assert!(link_targets(&read(&root().join(page))).iter().any(|(_, t)| t.split('#').next() == Some("play/")), "{page} does not link to the page of the playground alone");
    }
    // the stylesheet, the script and the page that explains, in each language
    assert!(seen >= 6, "{seen} links on the pages of the playground alone: were they changed?");
}

/// The Markdown pages of ritsu's site and of the site of every language build.sh builds.
fn every_page() -> Vec<PathBuf> {
    let mut dirs = vec![website().join("docs"), website().join("docs-ja")];
    for s in sites() {
        dirs.push(website().join(&s).join("docs"));
        dirs.push(website().join(&s).join("docs-ja"));
    }
    let mut out = Vec::new();
    while let Some(d) = dirs.pop() {
        for e in fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if e.file_type().unwrap().is_dir() {
                dirs.push(p);
            } else if p.extension().is_some_and(|x| x == "md") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

#[test]
fn the_links_into_the_repository_name_what_is_there() {
    let into = ["https://github.com/i2y/ritsu/blob/main/", "https://github.com/i2y/ritsu/tree/main/"];
    let mut wrong = Vec::new();
    let mut seen = 0;
    for f in every_page() {
        let name = f.strip_prefix(root()).unwrap().display().to_string();
        for (n, line) in read(&f).lines().enumerate() {
            for start in into {
                let mut rest = line;
                while let Some(i) = rest.find(start) {
                    let after = &rest[i + start.len()..];
                    let path: String = after.chars().take_while(|c| !matches!(c, ')' | '>' | '"' | '\'' | '#' | '`' | ' ' | '?')).collect();
                    seen += 1;
                    if !root().join(&path).exists() {
                        wrong.push(format!("{name}:{}: {start}{path} is no file or directory of the repository", n + 1));
                    }
                    rest = &after[path.len()..];
                }
            }
            // The repository of a language by itself is not where the language is: only ritsu's is
            // named. Its releases neither: ritsu's releases hand out every language, rulec's binary,
            // packages and action among them (rulec's install page, DESIGN 13.2).
            let mut rest = line;
            while let Some(i) = rest.find("github.com/i2y/") {
                let repo: String = rest[i + 15..].chars().take_while(|c| c.is_alphanumeric() || *c == '-').collect();
                if repo != "ritsu" {
                    wrong.push(format!("{name}:{}: names github.com/i2y/{repo}", n + 1));
                }
                rest = &rest[i + 15..];
            }
        }
    }
    // The examples and tests dandori's pages link to, and the READMEs on ritsu's index pages.
    assert!(seen >= 50, "{seen} links into the repository: were they changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

fn index_pages() -> Vec<Page> {
    vec![page("website/docs/index.md"), page("website/docs-ja/index.md")]
}

#[test]
fn the_code_on_the_index_is_from_the_files_and_ritsu_check_prints_what_it_shows() {
    let pages = index_pages();
    let (seen, wrong) = code_from_files(&pages);
    // The refund rule and flow, on two pages.
    assert!(seen >= 4, "{seen} blocks of code on the index pages: were the fences changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    let (ran, wrong) = run_the_consoles(&pages);
    // `ritsu check .` on the two files, as text and as JSON, on each of the two pages.
    assert!(ran >= 4, "{ran} commands run: were the fences changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    let (named, wrong) = commands_named(&pages);
    // `check` of three languages, `doc` of four and `yuen trace`, on two pages.
    assert!(named >= 2 * 8, "{named} commands named in the prose: were they changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn every_site_build_sh_builds_is_there_and_the_index_pages_link_to_it() {
    let sites = sites();
    for held in HELD {
        assert!(sites.iter().any(|s| s == held), "build.sh does not build {held}'s site, which ritsu's site holds at website/{held}");
    }
    for site in sites {
        let dir = website().join(&site);
        for f in ["build.sh", "zensical.toml", "zensical.ja.toml", "docs/index.md", "docs-ja/index.md"] {
            assert!(dir.join(f).is_file(), "build.sh builds the site {site}, and website/{site}/{f} is not there");
        }
        for (index, link) in [("website/docs/index.md", format!("]({site}/)")), ("website/docs-ja/index.md", format!("](../{site}/ja/)"))] {
            assert!(read(&root().join(index)).contains(&link), "{index} does not link to the site {site} as {link}");
        }
    }
}

/// The value of `key = "…"` in a configuration.
fn value(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|l| l.strip_prefix(&format!("{key} = \""))?.strip_suffix('"').map(String::from))
}

#[test]
fn the_configurations_name_where_each_site_is_published() {
    let cargo = read(&root().join("Cargo.toml"));
    let repository = cargo.lines().find_map(|l| l.strip_prefix("repository = \"")).and_then(|l| l.strip_suffix('"')).expect("the workspace names its repository");
    let mut configured = vec![(String::new(), website())];
    configured.extend(sites().into_iter().map(|s| (format!("{s}/"), website().join(s))));
    for (under, dir) in configured {
        for (file, ja) in [("zensical.toml", false), ("zensical.ja.toml", true)] {
            let at = dir.join(file);
            let name = at.strip_prefix(root()).unwrap().display().to_string();
            let text = read(&at);
            let (lang, docs, built) = if ja { ("ja/", "docs-ja", "build/ja") } else { ("", "docs", "build") };
            assert_eq!(value(&text, "site_url"), Some(format!("{PUBLISHED}{under}{lang}")), "{name}: site_url");
            assert_eq!(value(&text, "repo_url").as_deref(), Some(repository), "{name}: repo_url");
            assert_eq!(value(&text, "edit_uri"), Some(format!("edit/main/website/{under}{docs}")), "{name}: edit_uri");
            assert_eq!(value(&text, "docs_dir").as_deref(), Some(docs), "{name}: docs_dir");
            assert_eq!(value(&text, "site_dir").as_deref(), Some(built), "{name}: site_dir");
            // The language switch, which links absolutely.
            let switch: Vec<String> = text.lines().filter_map(|l| l.strip_prefix("link = \"")?.strip_suffix('"').map(String::from)).collect();
            assert_eq!(switch, [format!("/ritsu/{under}"), format!("/ritsu/{under}ja/")], "{name}: the language switch");
        }
    }
}

#[test]
fn the_workflow_that_publishes_the_site_runs_on_a_push_to_main_and_by_hand() {
    let text = read(&root().join(".github/workflows/docs.yml"));
    let mut lines = text.lines().skip_while(|l| *l != "on:");
    assert_eq!(lines.next(), Some("on:"), "docs.yml has no `on:`");
    let triggers: Vec<&str> = lines.take_while(|l| l.is_empty() || l.starts_with(' ')).map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).collect();
    let want = ["push:", "branches: [main]", "paths:", "- \"website/**\"", "- \"crates/rulec/AGENTS.md\"", "- \"crates/rulec/docs/**\"", "- \"crates/chobo/docs/**\"", "- \"modules/**\"", "- \".github/workflows/docs.yml\"", "workflow_dispatch:"];
    assert_eq!(triggers, want, "docs.yml runs on a push to main that changes what the site is built from (website/, the documents rulec's and chobo's sync.sh copy in, the modules its list shows, and itself), and by hand");
    assert!(text.contains("./build.sh") && text.contains("path: website/build"), "docs.yml builds with website/build.sh and publishes website/build");
    // the list of modules checks the signatures of the seed module (DESIGN 17.17), with the cosign
    // tools.yml installs
    let tools = read(&root().join(".github/workflows/tools.yml"));
    let pinned: Vec<&str> = text.lines().filter(|l| l.contains("/sigstore/cosign/releases/download/") || l.contains("| sha256sum -c -")).collect();
    assert_eq!(pinned.len(), 2, "docs.yml does not install cosign as tools.yml does: {pinned:?}");
    for line in pinned {
        assert!(tools.lines().any(|t| t == line), "tools.yml installs another cosign than docs.yml's\n{line}");
    }
}

/// The page a language's site had for its playground, in a language (`docs/playground.md` or
/// `docs-ja/playground.md` of website/<site>), and where its site publishes it.
fn page_before(site: &str, ja: bool) -> (PathBuf, String) {
    let file = website().join(site).join(if ja { "docs-ja" } else { "docs" }).join("playground.md");
    let url = if ja { format!("/{site}/ja/playground/") } else { format!("/{site}/playground/") };
    (file, url)
}

/// The value of an attribute of the first element that has `id="…"`, as a page writes it.
fn attribute_of(html: &str, id: &str, attr: &str) -> Option<String> {
    let at = html.find(&format!("id=\"{id}\""))?;
    let start = html[..at].rfind('<')?;
    let tag = &html[start..start + html[start..].find('>')?];
    let from = tag.find(&format!(" {attr}=\""))? + attr.len() + 3;
    Some(tag[from..from + tag[from..].find('"')?].replace("&amp;", "&"))
}

/// What a link of the playground opens it on (`#project=rulec/gap`, `#flow=tests/…`,
/// `#project=shop&file=gates/orders.gate&view=doc`): a project the page lists in its language, by
/// projects.json, and, when the link names them, a file of that project and a view the page has.
fn opens_a_project(fragment: &str, ja: bool) -> bool {
    let v: serde_json::Value = serde_json::from_str(&read(&website().join("docs/playground/projects.json"))).expect("projects.json");
    let lang = if ja { "ja" } else { "en" };
    let listed = |p: &&serde_json::Value| p["lang"].as_str().is_none_or(|l| l == lang);
    let projects: Vec<&serde_json::Value> = v["projects"].as_array().unwrap().iter().filter(listed).collect();
    let parts: Vec<(&str, &str)> = fragment.split('&').map(|kv| kv.split_once('=').unwrap_or((kv, ""))).collect();
    let part = |key: &str| parts.iter().find(|(k, _)| *k == key).map(|(_, v)| *v);
    // the words the playground's script reads of a link (playground.js, `follow`)
    if !parts.iter().all(|(k, _)| ["project", "flow", "file", "view", "target", "edits"].contains(k)) {
        return false;
    }
    let project = match (part("project"), part("flow")) {
        (Some(name), _) => projects.iter().find(|p| p["name"] == name),
        (None, Some(flow)) => projects.iter().find(|p| p["group"] == "dandori" && p["open"] == flow),
        (None, None) => None,
    };
    let Some(project) = project else { return false };
    let files: Vec<&str> = project["files"].as_array().unwrap().iter().filter_map(|f| f[0].as_str()).collect();
    part("file").is_none_or(|f| files.contains(&f)) && part("view").is_none_or(|v| ["check", "gen", "doc", "markdown", "build", "rules"].contains(&v))
}

/// The playgrounds rulec's and dandori's sites had are ritsu's now (DESIGN 13.2). The page each had
/// is kept, in each language, only to send a reader on: its link `#moved` leads to ritsu's playground
/// in the page's language, on a project it opens (what the page that was there opened on); its script
/// follows the link, and takes a link the reader followed that opened something (`=`) along. The page
/// is no more in the site's nav: the nav, and the home page, lead to the same place as the link.
///
/// A link of a page is read from the page's file, as Zensical reads it (and Zensical writes it, in the
/// page it publishes a directory deeper, with one more `../`), and the nav's from the site's root: for
/// all three, from the site's root, which is where the page's file is. Every site is under /ritsu/, and
/// a link is followed from there, so one that climbs out of ritsu's site leads nowhere here.
#[test]
fn the_pages_the_playgrounds_had_send_a_reader_on() {
    let mut seen = 0;
    for site in sites() {
        if !page_before(&site, false).0.is_file() {
            continue;
        }
        for ja in [false, true] {
            let (file, url) = page_before(&site, ja);
            let name = file.strip_prefix(root()).unwrap().display().to_string();
            let text = read(&file);
            let site_root = if ja { format!("/ritsu/{site}/ja/") } else { format!("/ritsu/{site}/") };
            let link = attribute_of(&text, "moved", "href").unwrap_or_else(|| panic!("{name} has no link `#moved`"));
            let (path, fragment) = link.split_once('#').unwrap_or_else(|| panic!("{name}: the link {link} names nothing to open"));
            let lands = normal(&format!("{site_root}{path}"));
            let playground = if ja { "/ritsu/ja/playground/" } else { "/ritsu/playground/" };
            assert_eq!(lands, playground, "{name}: the link {link} leads, from the site's root {site_root} (where the page {url} is read from), elsewhere than to the playground");
            let source = source_of(lands.strip_prefix("/ritsu").unwrap(), &sites()).unwrap_or_else(|| panic!("{name}: {lands} is no page of the site"));
            assert_eq!(source, website().join(if ja { "docs-ja/playground.md" } else { "docs/playground.md" }), "{name}: the link leads to another page");
            assert!(opens_a_project(fragment, ja), "{name}: the playground lists no project #{fragment} opens");
            for part in ["document.getElementById(\"moved\").href", "location.hash.includes(\"=\")", "location.replace("] {
                assert!(text.contains(part), "{name}: the script does not hold `{part}`");
            }
            // the nav of the site, whose links are read from the site's root, and its home page,
            // whose links are read from the page's file
            let config = read(&website().join(&site).join(if ja { "zensical.ja.toml" } else { "zensical.toml" }));
            assert!(!config.contains("\"playground.md\""), "{site}'s nav still names playground.md");
            let nav: Vec<String> = config
                .lines()
                .filter_map(|l| l.trim().strip_prefix('{')?.split_once("\" = \"").map(|(_, t)| t.trim_end_matches(['}', ',', ' ']).trim_end_matches('"').to_string()))
                .filter(|t| t.contains('#'))
                .collect();
            let want = format!("{playground}#{fragment}");
            assert!(nav.iter().any(|t| { let (p, f) = t.split_once('#').unwrap(); format!("{}#{f}", normal(&format!("{site_root}{p}"))) == want }), "{site}'s nav leads to no {want}");
            let index = read(&website().join(&site).join(if ja { "docs-ja/index.md" } else { "docs/index.md" }));
            assert!(link_targets(&index).iter().any(|(_, t)| t.split_once('#').is_some_and(|(p, f)| format!("{}#{f}", normal(&format!("{site_root}{p}"))) == want)), "{site}'s home page leads to no {want}");
            seen += 1;
        }
    }
    // rulec's and dandori's, in two languages each
    assert!(seen >= 4, "{seen} pages that send a reader on: were they changed?");
}

/// Every language's site leads to ritsu's playground in its own language: its nav (read from the
/// site's root, as Zensical reads a nav) and its home page (whose links are read from its file,
/// which is at the site's root) open the playground of the language the site is in, on a project
/// the playground lists in that language. A site's English pages open the English version of an
/// example, and its Japanese pages the Japanese one.
#[test]
fn every_site_leads_to_the_playground_in_its_language() {
    let mut seen = 0;
    for site in sites() {
        for ja in [false, true] {
            let config = website().join(&site).join(if ja { "zensical.ja.toml" } else { "zensical.toml" });
            let name = config.strip_prefix(root()).unwrap().display().to_string();
            let site_root = if ja { format!("/ritsu/{site}/ja/") } else { format!("/ritsu/{site}/") };
            let playground = if ja { "/ritsu/ja/playground/" } else { "/ritsu/playground/" };
            let nav: Vec<String> = read(&config)
                .lines()
                .filter_map(|l| l.trim().strip_prefix('{')?.split_once("\" = \"").map(|(_, t)| t.trim_end_matches(['}', ',', ' ']).trim_end_matches('"').to_string()))
                .filter(|t| t.contains("playground/#"))
                .collect();
            assert!(!nav.is_empty(), "{name}: the nav does not lead to the playground");
            for t in &nav {
                let (path, fragment) = t.split_once('#').unwrap();
                assert_eq!(normal(&format!("{site_root}{path}")), playground, "{name}: the nav's `{t}` leads, from the site's root {site_root}, elsewhere than to the playground in the site's language");
                assert!(opens_a_project(fragment, ja), "{name}: the nav's `{t}` opens no project the playground lists in the site's language");
            }
            let index = website().join(&site).join(if ja { "docs-ja/index.md" } else { "docs/index.md" });
            let index_name = index.strip_prefix(root()).unwrap().display().to_string();
            let leads: Vec<String> = link_targets(&read(&index))
                .into_iter()
                .filter_map(|(_, t)| {
                    let (path, fragment) = t.split_once('#')?;
                    (normal(&format!("{site_root}{path}")) == playground).then(|| fragment.to_string())
                })
                .collect();
            assert!(!leads.is_empty(), "{index_name} does not lead to the playground in the site's language");
            for fragment in leads {
                assert!(opens_a_project(&fragment, ja), "{index_name}: `#{fragment}` opens no project the playground lists in the site's language");
            }
            seen += 1;
        }
    }
    // rulec's, dandori's and chobo's, in two languages each
    assert!(seen >= 6, "{seen} sites' navs lead to the playground: were they changed?");
}

/// The pages of yuen's namespace, in English and in Japanese: what the IRI of a word of yuen's
/// PROV opens (`https://i2y.github.io/ritsu/ns/yuen#Requirement`, which the server sends on to
/// `…/ns/yuen/#Requirement`).
const NAMESPACE_PAGES: [&str; 2] = ["website/docs/ns/yuen.md", "website/docs-ja/ns/yuen.md"];

/// One word's entry on the page of a namespace.
struct Entry {
    /// The id the heading is given (`{ #Requirement }`), which is what the IRI's `#` names.
    id: String,
    /// The id of the `##` heading it is under: `types`, `relations` or `attributes`.
    section: String,
    /// Where the heading is, from 1.
    line: usize,
    /// What is in code in the first paragraph under the heading: `yuen:<word>`, then where it is
    /// written.
    signature: Vec<String>,
    /// The `#anchor` links in the entry.
    links: BTreeSet<String>,
}

/// The id of a heading written `## Text { #id }`.
fn explicit_id(heading: &str) -> Option<String> {
    let h = heading.trim_start_matches('#').trim();
    let open = h.rfind(" { #")?;
    h.ends_with(" }").then(|| h[open + 4..h.len() - 2].trim().to_string())
}

/// The entries of a page of a namespace: every `###` heading with an id, under the `##` heading
/// whose id names its section. What is between a `##` heading and the next `###` belongs to none.
fn entries(p: &Page) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    let mut section = String::new();
    let mut body: Vec<String> = Vec::new();
    let mut open = false;
    let mut fence = false;
    // The entry that is open takes what has been read since its heading.
    let close = |out: &mut Vec<Entry>, body: &mut Vec<String>, open: &mut bool| {
        if let (true, Some(e)) = (*open, out.last_mut()) {
            let text = body.join("\n");
            // The first paragraph, which a long signature takes two lines of.
            let first: Vec<&str> = body.iter().map(String::as_str).skip_while(|l| l.trim().is_empty()).take_while(|l| !l.trim().is_empty()).collect();
            e.signature = first.join(" ").split('`').skip(1).step_by(2).map(String::from).collect();
            e.links = link_targets(&text).into_iter().filter_map(|(_, t)| t.strip_prefix('#').map(String::from)).collect();
        }
        *open = false;
        body.clear();
    };
    for (i, line) in p.text.lines().enumerate() {
        if line.trim_start().starts_with("```") {
            fence = !fence;
        }
        if !fence && line.starts_with("### ") {
            close(&mut out, &mut body, &mut open);
            if let Some(id) = explicit_id(line) {
                out.push(Entry { id, section: section.clone(), line: i + 1, signature: Vec::new(), links: BTreeSet::new() });
                open = true;
            }
        } else if !fence && line.starts_with("## ") {
            close(&mut out, &mut body, &mut open);
            section = explicit_id(line).unwrap_or_default();
        } else {
            body.push(line.to_string());
        }
    }
    close(&mut out, &mut body, &mut open);
    out
}

#[test]
fn the_pages_of_yuens_namespace_say_every_word_yuen_writes_and_no_other() {
    // The namespace is the page's address and a `#`, so that a word's IRI is the address of its entry.
    assert_eq!(YUEN_NS, format!("{PUBLISHED}ns/yuen#"), "yuen writes its words in the namespace that ritsu's site publishes at ns/yuen/");
    let section = |k: Kind| match k {
        Kind::Type => "types",
        Kind::Relation => "relations",
        Kind::Attribute => "attributes",
    };
    let mut wrong = Vec::new();
    for name in NAMESPACE_PAGES {
        let p = page(name);
        if !p.text.contains(&format!("`{YUEN_NS}`")) {
            wrong.push(format!("{name}: does not say the namespace, `{YUEN_NS}`"));
        }
        let entries = entries(&p);
        for t in TERMS {
            let found: Vec<&Entry> = entries.iter().filter(|e| e.id == t.name).collect();
            let [e] = found[..] else {
                wrong.push(format!("{name}: {} entries for `{}`, which yuen writes ({:?}): one `### … {{ #{} }}` is wanted", found.len(), t.name, t.kind, t.name));
                continue;
            };
            let at = format!("{name}:{}", e.line);
            if e.section != section(t.kind) {
                wrong.push(format!("{at}: `{}` is a {:?} and is under `{}`, not `{}`", t.name, t.kind, e.section, section(t.kind)));
            }
            // The first paragraph under the heading: the word, and where it is written.
            if e.signature.first().map(String::as_str) != Some(&format!("yuen:{}", t.name)) {
                wrong.push(format!("{at}: the first paragraph under `{}` does not start with `yuen:{}` in code", t.name, t.name));
            }
            let got: BTreeSet<&str> = e.signature.iter().skip(1).map(String::as_str).collect();
            let want: BTreeSet<&str> = t.on.iter().copied().collect();
            if got != want {
                wrong.push(format!("{at}: `{}` is written on {want:?}, and the first paragraph under it names {got:?}", t.name));
            }
            // A type says which attributes it has, as links to them.
            if t.kind == Kind::Type {
                let want: BTreeSet<&str> = TERMS.iter().filter(|a| a.kind == Kind::Attribute && a.on.contains(&t.name)).map(|a| a.name).collect();
                let got: BTreeSet<&str> = TERMS.iter().filter(|a| a.kind == Kind::Attribute && e.links.contains(a.name)).map(|a| a.name).collect();
                if got != want {
                    wrong.push(format!("{at}: `{}` has the attributes {want:?}, and its entry links to {got:?}", t.name));
                }
            }
        }
        for e in &entries {
            if !TERMS.iter().any(|t| t.name == e.id) {
                wrong.push(format!("{name}:{}: an entry for `{}`, which yuen does not write", e.line, e.id));
            }
        }
        // The three sections are headings whose ids are the ones the entries are filed under.
        for id in ["types", "relations", "attributes"] {
            if !p.anchors.iter().any(|a| a == id) {
                wrong.push(format!("{name}: no `## … {{ #{id} }}` heading"));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    eprintln!("compared: {} words of yuen's namespace with the entries of {} pages", TERMS.len(), NAMESPACE_PAGES.len());
}

#[test]
fn the_example_on_the_pages_of_yuens_namespace_is_what_the_command_prints() {
    let pages: Vec<Page> = NAMESPACE_PAGES.iter().map(|n| page(n)).collect();
    let (ran, wrong) = run_the_consoles(&pages);
    // One export on each of the two pages.
    assert!(ran >= 2, "{ran} commands run: were the fences changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    let (_, wrong) = commands_named(&pages);
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    // Every word the example shows is one yuen writes: a typo in a word would not show otherwise.
    let mut shown = 0;
    for p in &pages {
        for b in p.blocks.iter().filter(|b| b.info == "console") {
            for l in &b.lines {
                let mut rest = l.as_str();
                while let Some(i) = rest.find("yuen:") {
                    let after = &rest[i + 5..];
                    let word: String = after.chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
                    if !word.is_empty() && !rest[..i].ends_with("urn:") {
                        shown += 1;
                        assert!(TERMS.iter().any(|t| t.name == word), "{}:{}: the example shows yuen:{word}, which yuen does not write", p.name, b.at);
                    }
                    rest = after;
                }
            }
        }
    }
    assert!(shown >= 20, "{shown} words in the examples");
    // The words in code on the pages that look like words of the namespace are words of it.
    for p in &pages {
        for (line, code) in inline_code(&p.text) {
            if let Some(word) = code.strip_prefix("yuen:").filter(|w| w.chars().all(|c| c.is_ascii_alphanumeric())) {
                assert!(TERMS.iter().any(|t| t.name == word), "{}:{line}: `{code}` is no word of the namespace", p.name);
            }
        }
    }
}

/// The `"page.md"` targets of the `nav` of a configuration of ritsu's pages, in order.
fn nav_targets(config: &str) -> Vec<String> {
    let text = read(&website().join(config));
    let nav = text.split("nav = [").nth(1).unwrap_or_else(|| panic!("website/{config} has no nav")).split("\n]").next().unwrap();
    nav.lines().filter_map(|l| l.rsplit_once("= \"")).map(|(_, t)| t.split('"').next().unwrap().to_string()).collect()
}

/// The Markdown pages under a directory of ritsu's pages (`docs`, `docs-ja`), as paths from it.
fn pages_under(dir: &str) -> BTreeSet<String> {
    let base = website().join(dir);
    let mut out = BTreeSet::new();
    let mut todo = vec![base.clone()];
    while let Some(d) = todo.pop() {
        for e in fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if e.file_type().unwrap().is_dir() {
                todo.push(p);
            } else if p.extension().is_some_and(|x| x == "md") {
                out.insert(p.strip_prefix(&base).unwrap().display().to_string());
            }
        }
    }
    out
}

/// The pages of ritsu's site that no nav lists: the page of yuen's namespace, which the IRI of a word
/// opens, and yuen's reference links to (website/README.md).
const OFF_THE_NAV: [&str; 1] = ["ns/yuen.md"];

/// A page in one language and not in the other would leave a reader of the other without it; the
/// nav lists every page a reader is meant to find (the same, in the same order, in both), and the
/// home page leads to each of them too.
#[test]
fn the_two_languages_have_the_same_pages_and_the_navs_list_them() {
    let (en, ja) = (pages_under("docs"), pages_under("docs-ja"));
    assert_eq!(en, ja, "website/docs and website/docs-ja hold other pages");
    let (nav_en, nav_ja) = (nav_targets("zensical.toml"), nav_targets("zensical.ja.toml"));
    assert_eq!(nav_en, nav_ja, "the two navs list other pages, or the same in another order");
    let listed: BTreeSet<String> = nav_en.iter().cloned().collect();
    assert_eq!(listed.len(), nav_en.len(), "the nav lists a page twice: {nav_en:?}");
    let want: BTreeSet<String> = en.iter().filter(|p| !OFF_THE_NAV.contains(&p.as_str())).cloned().collect();
    assert_eq!(listed, want, "the nav lists other pages than website/docs holds, leaving out {OFF_THE_NAV:?}");
    assert!(listed.contains("sekisho.md"), "the nav lists no page of sekisho");
    assert!(KANAME_PAGES.iter().all(|p| listed.contains(&format!("kaname/{p}"))), "the nav lists not every page of kaname");
    for (index, nav) in [("website/docs/index.md", &nav_en), ("website/docs-ja/index.md", &nav_ja)] {
        let links: BTreeSet<String> = link_targets(&read(&root().join(index))).into_iter().map(|(_, t)| t.split('#').next().unwrap().to_string()).collect();
        for p in nav.iter().filter(|p| *p != "index.md") {
            assert!(links.contains(p), "{index} does not link to {p}, which its nav lists");
        }
    }
}

/// The pages of sekisho, in English and in Japanese. Their example is sekisho's own
/// (`SEKISHO_EXAMPLE`): a gate that reads a rule of rulec, a dates file and a calendar of koyomi, an
/// OpenAPI document and a workflow of dandori, in an English version and a Japanese one.
const SEKISHO_PAGES: [&str; 2] = ["website/docs/sekisho.md", "website/docs-ja/sekisho.md"];

/// sekisho's example, from the root of the repository.
const SEKISHO_EXAMPLE: &str = "crates/sekisho/examples/refunds";

/// Whether a page of ritsu's site is a Japanese one: what it shows is the Japanese version of the
/// example, and what the tools say in Japanese.
fn japanese(name: &str) -> bool {
    name.starts_with("website/docs-ja/")
}

#[test]
fn the_pages_of_sekisho_show_the_files_and_what_the_commands_print() {
    let pages: Vec<Page> = SEKISHO_PAGES.iter().map(|n| page(n)).collect();
    let (seen, wrong) = code_from_files(&pages);
    // the gate, the rule, the expectation, the files the action reads, the task, the workflow's
    // permit and the second version of the rule of `sekisho diff`, on each of the two pages
    assert!(seen >= 2 * 7, "{seen} blocks of code on the pages: were the fences changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    let (ran, wrong) = run_the_consoles(&pages);
    // check, gen --target cedar, ritsu gen, ritsu check and diff, on each of the two pages
    assert!(ran >= 2 * 5, "{ran} commands run: were the fences changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    let (named, wrong) = commands_named(&pages);
    assert!(named >= 2 * 4, "{named} commands named in the prose: were they changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The words after the fence of a block whose lines are lines `ritsu gen` writes.
const GENERATED: [&str; 5] = ["cedar", "cedarschema", "typescript", "python", "go"];

/// Every file `ritsu gen` writes for sekisho's example into `out` (`--lang ja` for the Japanese
/// pages), with its lines.
fn written_for_the_example(links: &Path, out: &Path, ja: bool) -> Vec<(String, Vec<String>)> {
    let cmd = format!("ritsu gen --out {}{}", out.display(), if ja { " --lang ja" } else { "" });
    let said = sh(links, &root().join(SEKISHO_EXAMPLE), &cmd);
    assert!(out.join("cedar").is_dir(), "`{cmd}` wrote no Cedar:\n{said}");
    let mut files = Vec::new();
    let mut todo = vec![out.to_path_buf()];
    while let Some(d) = todo.pop() {
        for e in fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if e.file_type().unwrap().is_dir() {
                todo.push(p);
            } else if let Ok(text) = fs::read_to_string(&p) {
                files.push((p.display().to_string(), trimmed(&text)));
            }
        }
    }
    files
}

#[test]
fn the_cedar_and_the_code_on_the_pages_of_sekisho_are_what_ritsu_gen_writes() {
    let links = links();
    let t = TempDir::new("sekisho-pages-gen");
    let mut wrong = Vec::new();
    let mut seen = 0;
    for name in SEKISHO_PAGES {
        let p = page(name);
        let ja = japanese(name);
        let files = written_for_the_example(links.path(), &t.path().join(if ja { "ja" } else { "en" }), ja);
        for b in p.blocks.iter().filter(|b| GENERATED.contains(&b.info.as_str())) {
            seen += 1;
            if !files.iter().any(|(_, file)| lines_in_order(&shown_lines(b), file)) {
                wrong.push(format!("{name}:{}: a ```{} block that is the lines of no one file `ritsu gen` writes for {SEKISHO_EXAMPLE}, in order", b.at, b.info));
            }
        }
    }
    // the clerks' permit, the computed value in the schema and the code of the request, on each page
    assert!(seen >= 2 * 3, "{seen} blocks of generated code on the pages: were the fences changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The tables of a page, outside its fences: the line each starts on, from 1, and its lines.
fn tables(text: &str) -> Vec<(usize, Vec<String>)> {
    let mut out: Vec<(usize, Vec<String>)> = Vec::new();
    let (mut fence, mut open) = (false, false);
    for (i, line) in text.lines().enumerate() {
        if line.trim_start().starts_with("```") {
            fence = !fence;
            open = false;
        } else if !fence && line.starts_with('|') {
            if !open {
                out.push((i + 1, Vec::new()));
                open = true;
            }
            out.last_mut().unwrap().1.push(line.trim_end().to_string());
        } else {
            open = false;
        }
    }
    out
}

/// A table on a page of sekisho is one of the page `sekisho doc` draws of the example, row for row,
/// and the count of the combinations the page says the check walks is the one that page says.
#[test]
fn the_tables_on_the_pages_of_sekisho_are_what_sekisho_doc_draws() {
    let links = links();
    let mut wrong = Vec::new();
    for name in SEKISHO_PAGES {
        let ja = japanese(name);
        let text = read(&root().join(name));
        let cmd = if ja { "sekisho doc refunds.ja.gate --lang ja" } else { "sekisho doc refunds.gate" };
        let doc = sh(links.path(), &root().join(SEKISHO_EXAMPLE), cmd);
        let drawn: Vec<String> = doc.lines().map(|l| l.trim_end().to_string()).collect();
        let found = tables(&text);
        assert!(!found.is_empty(), "{name} shows no table of the page `{cmd}` draws");
        for (at, rows) in found {
            if !drawn.windows(rows.len()).any(|w| w == rows.as_slice()) {
                wrong.push(format!("{name}:{at}: a table that is no table of what `{cmd}` draws, row for row"));
            }
        }
        let (before, after) = if ja { ("の組み合わせ ", " 通りをすべて数え") } else { ("walked all ", " combinations") };
        let n = doc.split_once(before).and_then(|(_, rest)| rest.split_once(after)).map(|(n, _)| n.to_string()).unwrap_or_else(|| panic!("`{cmd}` says no count of the combinations it walked:\n{doc}"));
        let says = if ja { format!("組み合わせは {n} 通り") } else { format!("{n} combinations") };
        if !text.contains(&says) {
            wrong.push(format!("{name}: does not say `{says}`, the count `{cmd}` says"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The two pages of sekisho show the same things, the Japanese one with the Japanese version of the
/// example and in its own words: as many blocks of each kind, and as many tables.
#[test]
fn the_two_pages_of_sekisho_show_the_same_things() {
    let shown = |name: &str| {
        let p = page(name);
        let mut kinds: Vec<String> = p.blocks.iter().map(|b| b.info.clone()).collect();
        kinds.sort();
        (kinds, tables(&p.text).len())
    };
    let [en, ja] = SEKISHO_PAGES.map(shown);
    assert_eq!(en, ja, "the pages of sekisho show other kinds of blocks, or other numbers of blocks or tables ({SEKISHO_PAGES:?})");
}

/// The lines of a ```diff block as the file has them, and as the edit leaves them: a line is ` `
/// (in both), `-` (taken out) or `+` (put in), then the line.
fn sides(b: &Block) -> (Vec<String>, Vec<String>) {
    let (mut old, mut new) = (Vec::new(), Vec::new());
    for l in &b.lines {
        let (mark, rest) = l.split_at(l.chars().next().map_or(0, char::len_utf8));
        match mark {
            " " => {
                old.push(rest.to_string());
                new.push(rest.to_string());
            }
            "-" => old.push(rest.to_string()),
            "+" => new.push(rest.to_string()),
            _ => panic!("a line of a ```diff that is not ` `, `-` or `+` before the line: `{l}`"),
        }
    }
    (old, new)
}

/// A page of sekisho shows an edit of the example as a ```diff, and what `sekisho check` says of the
/// gate so edited in the ```text block after it; every ```text block on it is one of those.
#[test]
fn what_the_pages_of_sekisho_say_of_an_edit_is_what_sekisho_check_says() {
    let links = links();
    let example = root().join(SEKISHO_EXAMPLE);
    let gates: Vec<String> = fs::read_dir(&example).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n.ends_with(".gate")).collect();
    let mut wrong = Vec::new();
    let mut edits = 0;
    for name in SEKISHO_PAGES {
        let p = page(name);
        let ja = japanese(name);
        for (i, b) in p.blocks.iter().enumerate() {
            if b.info == "text" && i.checked_sub(1).is_none_or(|j| p.blocks[j].info != "diff") {
                wrong.push(format!("{name}:{}: a ```text block after no ```diff: what edit does it show the check of?", b.at));
            }
            if b.info != "diff" {
                continue;
            }
            edits += 1;
            let (old, new) = sides(b);
            let (old, new) = (old.join("\n") + "\n", new.join("\n") + "\n");
            let hit: Vec<&String> = gates.iter().filter(|g| read(&example.join(g)).matches(&old).count() == 1).collect();
            let [gate] = hit[..] else {
                wrong.push(format!("{name}:{}: the lines the ```diff edits are once in {} gates of the example, not in one", b.at, hit.len()));
                continue;
            };
            let Some(said) = p.blocks.get(i + 1).filter(|n| n.info == "text") else {
                wrong.push(format!("{name}:{}: no ```text block after the ```diff shows what the check says", b.at));
                continue;
            };
            let t = TempDir::new("sekisho-pages-edit");
            ritsu_testkit::tmp::copy_dir(&example, t.path());
            fs::write(t.path().join(gate), read(&example.join(gate)).replacen(&old, &new, 1)).unwrap();
            let cmd = format!("sekisho check {gate} --root .{}", if ja { " --lang ja" } else { "" });
            let got = sh(links.path(), t.path(), &cmd);
            if lines_of(&said.lines.join("\n")) != lines_of(&got) {
                wrong.push(format!("{name}:{}: after the edit, `{cmd}` prints\n{got}--- and the page shows\n{}", said.at, said.lines.join("\n")));
            }
        }
    }
    // the clerks' permit without the line on the rule's answer, on each of the two pages
    assert!(edits >= 2, "{edits} edits shown on the pages: were the fences changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The pages of koyomi on ritsu's site, in English and in Japanese.
const KOYOMI_PAGES: [&str; 2] = ["website/docs/languages/koyomi.md", "website/docs-ja/languages/koyomi.md"];

#[test]
fn the_pages_of_koyomi_show_what_koyomi_diff_prints() {
    let pages: Vec<Page> = KOYOMI_PAGES.iter().map(|n| page(n)).collect();
    let (ran, wrong) = run_the_consoles(&pages);
    // `koyomi diff` of two calendars of its tests, on each of the two pages
    assert!(ran >= 2, "{ran} commands run: were the fences changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    let (named, wrong) = commands_named(&pages);
    // `koyomi check` and `koyomi diff`, on each of the two pages
    assert!(named >= 2 * 2, "{named} commands named in the prose: were they changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    let kinds = |p: &Page| {
        let mut kinds: Vec<String> = p.blocks.iter().map(|b| b.info.clone()).collect();
        kinds.sort();
        kinds
    };
    assert_eq!(kinds(&pages[0]), kinds(&pages[1]), "the pages of koyomi show other kinds of blocks, or other numbers of blocks ({KOYOMI_PAGES:?})");
}

/// sekisho's READMEs lead to its page on the site, each to the page in its own language.
#[test]
fn the_readmes_of_sekisho_link_to_its_pages() {
    for (readme, under, page) in [("crates/sekisho/README.md", "", "docs/sekisho.md"), ("crates/sekisho/README.ja.md", "ja/", "docs-ja/sekisho.md")] {
        let url = format!("{PUBLISHED}{under}sekisho/");
        assert!(read(&root().join(readme)).contains(&url), "{readme} does not link to {url}");
        assert_eq!(source_of(&format!("/{under}sekisho/"), &sites()), Some(website().join(page)), "{url} is no page of the site");
    }
}

/// The pages of kaname, in each language's directory `kaname/`: what kaname is, how a module is
/// written, the four functional cores of its examples with their hosts, what the check guarantees,
/// and the calls of rules, dates and books. They show kaname's examples and the materials of its
/// tests, the English versions on the English pages and the Japanese versions on the Japanese ones;
/// the hosts, which call the code of the English versions, on both.
const KANAME_PAGES: [&str; 5] = ["index.md", "language.md", "examples.md", "checks.md", "together.md"];

/// The guide to modules, in English and in Japanese. Its examples are the modules of ritsu's tests
/// (crates/ritsu/tests/projects/modules, and modules.ja for the Japanese page) and the module of
/// holidays (modules/holidays).
const MODULE_PAGES: [&str; 2] = ["website/docs/modules.md", "website/docs-ja/modules.md"];

#[test]
fn the_guide_to_modules_shows_the_files_and_what_the_commands_print() {
    let pages: Vec<Page> = MODULE_PAGES.iter().map(|n| page(n)).collect();
    let (seen, wrong) = code_from_files(&pages);
    // the rule of pricing that applies tax's, on each page
    assert!(seen >= 2, "{seen} blocks of code on the pages: were the fences changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    let (marked, wrong) = marked_files(&pages);
    // pricing's ritsu.mod, on each page
    assert!(marked >= 2, "{marked} blocks marked with a file: were the marks changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    let (ran, wrong) = run_the_consoles(&pages);
    // `ritsu mod list` and `ritsu check` of the shop, `ritsu mod push --dry-run` of the holidays and
    // `ritsu mod diff` of pricing, on each page
    assert!(ran >= 2 * 4, "{ran} commands run: were the fences changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    let (named, wrong) = commands_named(&pages);
    assert!(named >= 2 * 20, "{named} commands named in the prose: were they changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    let shown = |p: &Page| {
        let mut kinds: Vec<String> = p.blocks.iter().map(|b| b.info.clone()).collect();
        kinds.sort();
        let headings = p.text.lines().filter(|l| l.starts_with("## ")).count();
        (kinds, tables(&p.text).len(), headings)
    };
    let (en, ja) = (shown(&pages[0]), shown(&pages[1]));
    assert_eq!(en.2, 8, "the guide has eight sections");
    assert_eq!(en, ja, "the two pages show other kinds of blocks, or other numbers of blocks, tables or sections");
}

/// The pages of kaname in a language, as paths from the root of the repository.
fn kaname_pages(ja: bool) -> Vec<String> {
    KANAME_PAGES.iter().map(|p| format!("website/{}/kaname/{p}", if ja { "docs-ja" } else { "docs" })).collect()
}

#[test]
fn the_pages_of_kaname_show_the_files_and_what_the_commands_print() {
    let pages: Vec<Page> = kaname_pages(false).iter().chain(kaname_pages(true).iter()).map(|n| page(n)).collect();
    let (seen, wrong) = code_from_files(&pages);
    // on each language's pages: the order and `next`; the lines of an order, its events, `open`, the
    // counts compared, the handle and the tag, the functions given and the tree; what the hosts of
    // the four examples call; `next_id` and the days late left unnarrowed; the late payment
    assert!(seen >= 2 * 15, "{seen} blocks of code on the pages: were the fences changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    let (ran, wrong) = run_the_consoles(&pages);
    // `ritsu check` of the order, a `run` of the counts, `ritsu gen` of the order, three `kaname
    // check`s, and the late payment's `ritsu check` and `run`, the dollars and the net payout, on each
    // language's pages
    assert!(ran >= 2 * 10, "{ran} commands run: were the fences changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    let (named, wrong) = commands_named(&pages);
    assert!(named >= 2 * 6, "{named} commands named in the prose: were they changed?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The code of a host on the pages of kaname is the lines of a host of its examples, and what a
/// ```text block shows is what the hosts above it print; the page of the examples shows every host
/// of every functional core. (ritsu's `tests/kaname.rs` runs the hosts on the package `ritsu gen`
/// writes, and holds what they print to `host/output.txt`.)
#[test]
fn the_hosts_on_the_pages_of_kaname_are_its_examples_and_print_what_they_show() {
    let every: BTreeSet<(String, &str)> = kaname_hosts().into_iter().map(|h| (h.example, h.ext)).collect();
    assert_eq!(every.len(), 4 * 3, "the hosts of kaname's examples: {every:?}");
    for ja in [false, true] {
        let names = kaname_pages(ja);
        let pages: Vec<Page> = names.iter().map(|n| page(n)).collect();
        let (seen, _, wrong) = hosts_from_files(&pages);
        // one on the first page, and twelve on the page of the examples
        assert!(seen >= 13, "{seen} blocks of hosts on the pages: were the fences changed?");
        assert!(wrong.is_empty(), "{}", wrong.join("\n"));
        let examples = names.iter().find(|n| n.ends_with("/examples.md")).unwrap();
        let (_, shown, _) = hosts_from_files(&[page(examples)]);
        assert_eq!(shown, every, "{examples} does not show every host of the four functional cores");
    }
}

/// A page of kaname in English and the same page in Japanese show the same things, the Japanese one
/// with the Japanese versions of the examples and in its own words: as many blocks of each kind, and
/// as many tables.
#[test]
fn the_two_languages_of_the_pages_of_kaname_show_the_same_things() {
    let shown = |name: &str| {
        let p = page(name);
        let mut kinds: Vec<String> = p.blocks.iter().map(|b| b.info.clone()).collect();
        kinds.sort();
        (kinds, tables(&p.text).len())
    };
    for (en, ja) in kaname_pages(false).iter().zip(kaname_pages(true).iter()) {
        assert_eq!(shown(en), shown(ja), "{en} and {ja} show other kinds of blocks, or other numbers of blocks or tables");
    }
}

/// kaname's READMEs and the READMEs at the root lead to the pages of kaname on the site, each to the
/// pages in its own language.
#[test]
fn the_readmes_of_kaname_link_to_its_pages() {
    for (readme, under, page) in [
        ("crates/kaname/README.md", "", "docs/kaname/index.md"),
        ("crates/kaname/README.ja.md", "ja/", "docs-ja/kaname/index.md"),
        ("README.md", "", "docs/kaname/index.md"),
        ("README.ja.md", "ja/", "docs-ja/kaname/index.md"),
    ] {
        let url = format!("{PUBLISHED}{under}kaname/");
        assert!(read(&root().join(readme)).contains(&url), "{readme} does not link to {url}");
        assert_eq!(source_of(&format!("/{under}kaname/"), &sites()), Some(website().join(page)), "{url} is no page of the site");
    }
}

/// `from` copied to `to`, without what a build, Zensical's cache or the virtual environment left.
fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let kind = e.file_type().unwrap();
        if kind.is_dir() {
            if ![".venv", ".cache", "build", "__pycache__"].contains(&name.as_str()) {
                copy_tree(&e.path(), &to.join(&name));
            }
        } else if kind.is_file() {
            fs::copy(e.path(), to.join(&name)).unwrap();
        }
    }
}

/// The fences ritsu's site colours, each with its lexer: `kn` with website/tools/knlexer.py, `gate`
/// with website/tools/gatelexer.py, Cedar's `cedar` and `cedarschema` with
/// website/tools/cedarlexer.py, and `rule`, `flow` and `book` with the lexers of rulec's, dandori's
/// and chobo's sites, which build.sh puts on PYTHONPATH and the two configurations name as Markdown
/// extensions.
const COLOURED: [&str; 7] = ["kn", "rule", "flow", "book", "gate", "cedar", "cedarschema"];

/// The fences of ritsu's lexers that a language's site may show too, and colours with them: a gate
/// and the Cedar it compiles to (chobo's site shows the gate that guards the operations of a book).
/// A site that shows one names the lexer in its configurations, and its build.sh puts website/tools
/// on PYTHONPATH.
const COLOURED_ON_THE_SITES: [&str; 3] = ["gate", "cedar", "cedarschema"];

/// Text as an HTML page escapes it, unescaped.
fn unescaped(text: &str) -> String {
    text.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&amp;", "&")
}

/// The blocks of code of a built page, in order: the language Pygments built each as (`text` for a
/// fence it has no lexer for), and its tokens, each the class of its span and its text.
fn built_blocks(html: &str) -> Vec<(String, Vec<(String, String)>)> {
    let mut out = Vec::new();
    for rest in html.split("<div class=\"language-").skip(1) {
        let Some((lang, after)) = rest.split_once(" highlight\">") else { continue };
        let code = after.split("</code></pre></div>").next().unwrap();
        let tokens = code
            .split("<span class=\"")
            .skip(1)
            .filter_map(|span| span.split_once("\">"))
            .map(|(class, text)| (class.to_string(), unescaped(text.split('<').next().unwrap())))
            .collect();
        out.push((lang.to_string(), tokens));
    }
    out
}

/// The values of the `href` and `src` attributes of an HTML page.
fn targets_in(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    for attr in [" href=\"", " src=\""] {
        let mut rest = html;
        while let Some(i) = rest.find(attr) {
            let after = &rest[i + attr.len()..];
            let Some(j) = after.find('"') else { break };
            out.push(after[..j].replace("&amp;", "&"));
            rest = &after[j..];
        }
    }
    out
}

#[test]
fn build_sh_builds_the_tree_that_is_published() {
    let zensical = website().join(".venv/bin/zensical");
    if !ready(Need::Python, || zensical.is_file(), "Zensical is not in website/.venv; install it as website/README.md says, to build the site") {
        return;
    }
    let t = TempDir::new("website");
    let copy = t.path().join("website");
    copy_tree(&website(), &copy);
    std::os::unix::fs::symlink(website().join(".venv"), copy.join(".venv")).unwrap();
    // rulec's sync.sh copies rulec's documents in from crates/rulec, beside website/, which it reads
    // and does not write.
    std::os::unix::fs::symlink(root().join("crates"), t.path().join("crates")).unwrap();
    // The list of modules (modules.list) shows the seed module as it is on ghcr.io, and checks its
    // signature. This test has no network: the copy of the list shows the module's working tree in
    // its place, beside website/, with the ritsu of these tests (tests/modules_site.rs builds a list
    // from a registry of its own, and checks the signatures).
    let list = read(&website().join("modules.list"));
    let lines: Vec<&str> = list.lines().filter(|l| !l.is_empty() && !l.starts_with('#')).collect();
    assert!(lines.contains(&"module \"ghcr.io/i2y/ritsu-holidays\""), "website/modules.list shows the seed module as it is on ghcr.io:\n{list}");
    assert!(lines.iter().any(|l| l.starts_with("signature \"ghcr.io/i2y/ritsu-holidays\" by matching ")), "website/modules.list checks the seed module's signature:\n{list}");
    let local: Vec<&str> = lines.into_iter().filter(|l| !l.starts_with("module ") && !l.starts_with("signature ")).chain(["local \"../modules/holidays\""]).collect();
    std::fs::write(copy.join("modules.list"), local.join("\n") + "\n").unwrap();
    std::os::unix::fs::symlink(root().join("modules"), t.path().join("modules")).unwrap();
    let o = Command::new("bash")
        .arg(copy.join("build.sh"))
        .env("RITSU", env!("CARGO_BIN_EXE_ritsu"))
        .env(ritsu_base::modules::store::VARIABLE, t.path().join("store"))
        .output()
        .expect("could not run bash");
    assert!(o.status.success(), "build.sh failed:\n{}\n{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    let built = copy.join("build");

    let mut want: Vec<String> = [
        "index.html",
        "ja/index.html",
        "playground/index.html",
        "playground/ritsu.wasm",
        "ja/playground/index.html",
        "ja/playground/projects.json",
        "ja/playground/words.json",
        "ja/playground/libraries.js",
        "ja/playground/worker.js",
        "play/index.html",
        "ja/play/index.html",
        "sekisho/index.html",
        "ja/sekisho/index.html",
        "kaname/index.html",
        "kaname/language/index.html",
        "kaname/examples/index.html",
        "kaname/checks/index.html",
        "kaname/together/index.html",
        "ja/kaname/index.html",
        "ja/kaname/language/index.html",
        "ja/kaname/examples/index.html",
        "ja/kaname/checks/index.html",
        "ja/kaname/together/index.html",
        "ns/yuen/index.html",
        "ja/ns/yuen/index.html",
        "modules/index.html",
        "ja/modules/index.html",
        "modules/list/index.html",
        "ja/modules/list/index.html",
        "modules/list/ghcr.io/i2y/ritsu-holidays/1.0.0/index.html",
        "ja/modules/list/ghcr.io/i2y/ritsu-holidays/1.0.0/module.json",
    ]
    .map(String::from)
    .to_vec();
    let mut before = Vec::new();
    for s in sites() {
        want.extend([format!("{s}/index.html"), format!("{s}/ja/index.html")]);
        if page_before(&s, false).0.is_file() {
            want.extend([format!("{s}/playground/index.html"), format!("{s}/ja/playground/index.html")]);
            before.extend([(s.clone(), false), (s.clone(), true)]);
        }
    }
    // The page a language's playground had is all there is of it, its link leads to the playground
    // of the tree, and so does the nav of the site's home page: the page itself is no more in it.
    for (s, ja) in &before {
        let (_, url) = page_before(s, *ja);
        let dir = built.join(url.trim_start_matches('/'));
        let beside: Vec<String> = fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).filter(|n| n != "index.html").collect();
        assert!(beside.is_empty(), "{url} has {beside:?} beside the page that sends a reader on");
        let html = read(&dir.join("index.html"));
        let link = attribute_of(&html, "moved", "href").unwrap_or_else(|| panic!("{url}: built with no link `#moved`"));
        // followed from where the site publishes the page, under /ritsu/: a link that climbs out of
        // ritsu's site does not come back to it
        let to = normal(&format!("/ritsu{url}{}", link.split('#').next().unwrap()));
        let playground = if *ja { "/ritsu/ja/playground/" } else { "/ritsu/playground/" };
        assert!(to == playground && built.join(to.trim_start_matches("/ritsu/")).join("index.html").is_file(), "{url}: the link {link} leads to {to}, not to the playground of the tree at {playground}");
        let home = if *ja { format!("/{s}/ja/") } else { format!("/{s}/") };
        let nav = read(&built.join(home.trim_start_matches('/')).join("index.html"));
        assert!(targets_in(&nav).iter().any(|t| !t.contains("://") && normal(&format!("/ritsu{home}{}", t.split('#').next().unwrap())) == playground), "{home}: the built page leads nowhere to {playground}");
    }
    // The marketplace of Claude Code that the README adds by its URL (tests/skill.rs holds what it
    // says), published as it is written.
    want.push("marketplace.json".to_string());
    let missing: Vec<&String> = want.iter().filter(|w| !built.join(w).is_file()).collect();
    assert!(missing.is_empty(), "build.sh built no {missing:?}");
    assert!(fs::read(built.join("marketplace.json")).unwrap() == fs::read(website().join("docs/marketplace.json")).unwrap(), "the site publishes another marketplace.json than website/docs/marketplace.json");

    // The page of kaname's examples is built with a set of three tabs for the hosts of each of the four
    // functional cores, TypeScript, Python and Go (pymdownx.tabbed in the configurations).
    for file in ["kaname/examples/index.html", "ja/kaname/examples/index.html"] {
        let html = read(&built.join(file));
        let sets: Vec<&str> = html.split("<div class=\"tabbed-set").skip(1).map(|r| r.split('>').next().unwrap_or("")).collect();
        assert!(sets.len() == 4 && sets.iter().all(|s| s.ends_with(":3\"")), "{file}: built with {} sets of tabs ({sets:?}), not four of three tabs", sets.len());
    }

    // The page of yuen's namespace is built with an element for every word, the id of which is the
    // word as it follows the `#` of its IRI: `…/ns/yuen#Requirement`, which the server sends on to
    // `…/ns/yuen/#Requirement`, leads to `id="Requirement"` and no other (an id is case-sensitive).
    for file in ["ns/yuen/index.html", "ja/ns/yuen/index.html"] {
        let html = read(&built.join(file));
        let ids: BTreeSet<String> = html.split(" id=\"").skip(1).filter_map(|r| r.split('"').next().map(String::from)).collect();
        let missing: Vec<&str> = TERMS.iter().map(|t| t.name).filter(|n| !ids.contains(*n)).collect();
        assert!(missing.is_empty(), "{file}: built with no element whose id is {missing:?}");
        for id in ["types", "relations", "attributes"] {
            assert!(ids.contains(id), "{file}: built with no element whose id is {id}");
        }
    }

    // The code of the languages on ritsu's pages is coloured as the site is built: every fence of a
    // language the site colours is built as a block of that language (a fence Pygments has no lexer
    // for is built as `text`), with a word in a class of its own (more than whitespace, punctuation
    // and plain names) and nothing the lexer could not read (`err`).
    let mut wrong = Vec::new();
    let mut coloured: BTreeMap<(String, &str), usize> = BTreeMap::new();
    for (p, url) in pages() {
        let file = format!("{}index.html", url.trim_start_matches('/'));
        let blocks = built_blocks(&read(&built.join(&file)));
        for lang in COLOURED {
            let fences = p.blocks.iter().filter(|b| b.info == lang).count();
            let of_lang: Vec<&Vec<(String, String)>> = blocks.iter().filter(|(l, _)| l == lang).map(|(_, tokens)| tokens).collect();
            if of_lang.len() != fences {
                wrong.push(format!("{file} (from {}): {fences} ```{lang} fences built as {} blocks of {lang}", p.name, of_lang.len()));
            }
            for tokens in &of_lang {
                if let Some((_, t)) = tokens.iter().find(|(c, _)| c == "err") {
                    wrong.push(format!("{file} (from {}): a block of {lang} has `{t}`, which its lexer could not read", p.name));
                }
                if !tokens.iter().any(|(c, _)| !["w", "p", "n"].contains(&c.as_str())) {
                    wrong.push(format!("{file} (from {}): a block of {lang} with no word coloured", p.name));
                }
            }
            *coloured.entry((file.clone(), lang)).or_default() += of_lang.len();
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    // the rule and the flow on each home page
    for file in ["index.html", "ja/index.html"] {
        for lang in ["rule", "flow"] {
            assert!(coloured.get(&(file.to_string(), lang)).is_some_and(|n| *n >= 1), "{file}: built with no block of {lang}: were the fences changed?");
        }
    }
    // The pages of kaname show each kind of word in its class, in each language.
    let is_date = |t: &str| t.len() == 10 && t.char_indices().all(|(i, c)| if i == 4 || i == 7 { c == '-' } else { c.is_ascii_digit() });
    let kinds: [(&str, &str, fn(&str) -> bool); 7] = [
        ("a keyword", "k", |t| t == "fn"),
        ("a type", "kt", |t| t == "money"),
        ("a number with its unit", "m", |t| t.starts_with(|c: char| c.is_ascii_digit()) && t.ends_with(|c: char| c.is_ascii_alphabetic() || c == '%')),
        ("a date", "m", is_date),
        ("a comment", "c1", |t| t.starts_with('#')),
        ("a string", "s2", |t| t.starts_with('"') && t.ends_with('"')),
        ("an operator", "o", |t| t == ">="),
    ];
    for ja in [false, true] {
        let mut tokens = Vec::new();
        let mut blocks = 0;
        for p in KANAME_PAGES {
            let page = p.trim_end_matches(".md");
            let file = format!("{}kaname/{}index.html", if ja { "ja/" } else { "" }, if page == "index" { String::new() } else { format!("{page}/") });
            for (lang, t) in built_blocks(&read(&built.join(&file))) {
                if lang == "kn" {
                    blocks += 1;
                    tokens.extend(t);
                }
            }
        }
        let lang = if ja { "Japanese" } else { "English" };
        assert!(blocks >= 15, "the {lang} pages of kaname are built with {blocks} blocks of kn: were the fences changed?");
        for (kind, class, is) in kinds {
            assert!(tokens.iter().any(|(c, t)| c == class && is(t)), "the {lang} pages of kaname show {kind} in no span of the class `{class}`");
        }
    }
    // The pages of sekisho show each kind of word of a gate, of Cedar's policies and of a schema in
    // its class, in each language; the alias of a name, which the Japanese version of the example
    // gives what Cedar writes, on the Japanese page.
    type Kind = (&'static str, &'static str, &'static str, fn(&str) -> bool);
    let sekisho_kinds: [Kind; 19] = [
        ("gate", "a keyword", "k", |t| t == "permit"),
        ("gate", "a type", "kt", |t| t == "money"),
        ("gate", "a tax", "kt", |t| t == "incl_tax"),
        ("gate", "a number with its unit", "m", |t| t == "10_000GBP"),
        ("gate", "a date", "m", |t| t == "2026-10-01"),
        ("gate", "a string", "s2", |t| t.starts_with('"') && t.ends_with('"')),
        ("gate", "a name a declaration gives", "nc", |t| !t.is_empty()),
        ("gate", "the name a use gives", "nn", |t| !t.is_empty()),
        ("gate", "an operator word", "ow", |t| t == "is"),
        ("gate", "a comparison", "o", |t| t == "<="),
        ("cedar", "a keyword", "k", |t| t == "permit"),
        ("cedar", "an operator word", "ow", |t| t == "has"),
        ("cedar", "an operator", "o", |t| t == "&&"),
        ("cedar", "an entity type", "nc", |t| t == "Role"),
        ("cedar", "a namespace", "nn", |t| t.starts_with("Shop")),
        ("cedar", "an entity's id", "s2", |t| t == "\"clerk\""),
        ("cedar", "an annotation", "nd", |t| t == "@id"),
        ("cedarschema", "a type", "kt", |t| t == "String"),
        ("cedarschema", "an annotation", "nd", |t| t == "@doc"),
    ];
    for (ja, file) in [(false, "sekisho/index.html"), (true, "ja/sekisho/index.html")] {
        let blocks = built_blocks(&read(&built.join(file)));
        let mut kinds: Vec<Kind> = sekisho_kinds.to_vec();
        if ja {
            kinds.push(("gate", "an alias", "na", |t| t == "refund_order"));
        }
        for (lang, kind, class, is) in kinds {
            let shown = blocks.iter().filter(|(l, _)| l == lang).flat_map(|(_, t)| t).any(|(c, t)| c == class && is(t));
            assert!(shown, "{file}: the blocks of {lang} show {kind} in no span of the class `{class}`");
        }
    }
    // The pages of a language's site are built with their blocks of a gate and of Cedar coloured as
    // ritsu's are: as many blocks of the language as the page has fences, each with a word coloured
    // and nothing the lexer could not read. The pages are read as the site's build.sh left them in the
    // copy (chobo's sync.sh copies some in), and published a directory deeper (`together.md` at
    // `together/`).
    for site in sites() {
        for (docs, under) in [("docs", ""), ("docs-ja", "ja/")] {
            let base = copy.join(&site).join(docs);
            let mut todo = vec![base.clone()];
            while let Some(d) = todo.pop() {
                for e in fs::read_dir(&d).unwrap().flatten() {
                    let p = e.path();
                    if e.file_type().unwrap().is_dir() {
                        todo.push(p);
                        continue;
                    }
                    let Some(rel) = p.strip_prefix(&base).unwrap().to_str().and_then(|r| r.strip_suffix(".md")) else { continue };
                    let fences: Vec<String> = read(&p).lines().filter_map(|l| l.trim_start().strip_prefix("```")).map(|i| i.trim().to_string()).collect();
                    let dir = match rel.strip_suffix("index") {
                        Some(d) if d.is_empty() || d.ends_with('/') => d.to_string(),
                        _ => format!("{rel}/"),
                    };
                    let file = format!("{site}/{under}{dir}index.html");
                    let html = built.join(&file);
                    let blocks = if html.is_file() { built_blocks(&read(&html)) } else { Vec::new() };
                    for lang in COLOURED_ON_THE_SITES {
                        // an opening fence is followed by a closing one, which names no language
                        let n = fences.iter().filter(|i| *i == lang).count();
                        let of_lang: Vec<&Vec<(String, String)>> = blocks.iter().filter(|(l, _)| l == lang).map(|(_, t)| t).collect();
                        if of_lang.len() != n {
                            wrong.push(format!("{file} (from website/{site}/{docs}/{rel}.md): {n} ```{lang} fences built as {} blocks of {lang}", of_lang.len()));
                        }
                        for tokens in &of_lang {
                            if let Some((_, t)) = tokens.iter().find(|(c, _)| c == "err") {
                                wrong.push(format!("{file}: a block of {lang} has `{t}`, which its lexer could not read"));
                            }
                            if !tokens.iter().any(|(c, _)| !["w", "p", "n"].contains(&c.as_str())) {
                                wrong.push(format!("{file}: a block of {lang} with no word coloured"));
                            }
                        }
                        *coloured.entry((file.clone(), lang)).or_default() += of_lang.len();
                    }
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    // the gate that guards the operations of a book, on chobo's pages in each language
    for file in ["chobo/together/index.html", "chobo/ja/together/index.html"] {
        assert!(coloured.get(&(file.to_string(), "gate")).is_some_and(|n| *n >= 1), "{file}: built with no block of gate: were the fences changed?");
    }
    let mut by_lang: BTreeMap<&str, usize> = BTreeMap::new();
    for ((_, lang), n) in &coloured {
        *by_lang.entry(lang).or_default() += n;
    }
    eprintln!("compared: the coloured blocks of the built pages with their fences, by language {by_lang:?} (ritsu's pages, and the gates and Cedar of the languages' sites)");

    // Every link of ritsu's built pages that stays in the site leads to a file of the tree.
    let mut wrong = Vec::new();
    let mut seen = 0;
    for (p, url) in pages() {
        let file = format!("{}index.html", url.trim_start_matches('/'));
        let html = read(&built.join(&file));
        for target in targets_in(&html) {
            if target.contains("://") || target.starts_with("mailto:") || target.starts_with("data:") || target.starts_with("javascript:") {
                continue;
            }
            let path = target.split(['#', '?']).next().unwrap();
            if path.is_empty() {
                continue;
            }
            let at = if let Some(under) = path.strip_prefix("/ritsu/") {
                normal(&format!("/{under}"))
            } else if path.starts_with('/') {
                wrong.push(format!("{file} (from {}): `{target}` leaves the site", p.name));
                continue;
            } else if let Some(at) = follow(&url, path) {
                at
            } else {
                // from /ritsu/<page>/, a `../` too many leaves /ritsu/
                wrong.push(format!("{file} (from {}): `{target}` climbs out of the site", p.name));
                continue;
            };
            seen += 1;
            let f = built.join(at.trim_start_matches('/'));
            if !(f.is_file() || f.join("index.html").is_file()) {
                wrong.push(format!("{file} (from {}): `{target}` is no file of the built site", p.name));
            }
        }
    }
    // and of the pages of the playground alone, which the tree has as they are
    for (source, url) in ALONE {
        let file = format!("{}index.html", url.trim_start_matches('/'));
        let html = read(&built.join(&file));
        assert!(html == read(&root().join(source)), "{file} is not {source} as it is");
        for target in targets_in(&html) {
            let path = target.split(['#', '?']).next().unwrap();
            match follow(url, path) {
                Some(at) if built.join(at.trim_start_matches('/')).is_file() || built.join(at.trim_start_matches('/')).join("index.html").is_file() => seen += 1,
                _ => wrong.push(format!("{file}: `{target}` is no file of the built site")),
            }
        }
    }
    assert!(seen >= 20, "{seen} links on ritsu's built pages: was the tree built?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    eprintln!("compared: {} files of the tree build.sh built, and {seen} links of ritsu's built pages", want.len());
}

/// The files under a directory of the repository whose names end in one of `exts`, past what a build,
/// an install or a virtual environment left there, and past links (the tools a worktree links in).
fn files_under(dir: &Path, exts: &[&str]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        for e in fs::read_dir(&d).unwrap().flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let kind = e.file_type().unwrap();
            if kind.is_dir() {
                if ![".venv", ".cache", "build", "target", "node_modules", "__pycache__", ".lake"].contains(&name.as_str()) {
                    todo.push(e.path());
                }
            } else if kind.is_file() && exts.iter().any(|x| name.ends_with(x)) {
                out.push(e.path());
            }
        }
    }
    out.sort();
    out
}

/// What the lexers of the site make of files: given a line `<alias>\t<path>` for each on standard
/// input, it prints `<n>\t<path>`, `n` the tokens of the class `err`, the ones the lexer could not
/// read. The lexers are imported from the directory its argument names, as the site's build imports
/// them.
const LEX: &str = r#"
import sys
sys.path.insert(0, sys.argv[1])
import gatelexer, cedarlexer
from pygments.lexers import get_lexer_by_name
from pygments.token import Error
lexers = {}
for line in sys.stdin:
    alias, path = line.rstrip("\n").split("\t", 1)
    lexer = lexers.setdefault(alias, get_lexer_by_name(alias))
    with open(path, encoding="utf-8", errors="replace") as f:
        text = f.read()
    print(f"{sum(1 for t, _ in lexer.get_tokens(text) if t in Error)}\t{path}")
"#;

/// The site's lexers of a gate and of Cedar read what the tools read, and mark what they do not.
///
/// - Every `.gate` of the repository is read whole, but the mutants of E001, which sekisho's lexer
///   does not read either.
/// - Every policy and schema among the materials of ritsu-base's tests of Cedar that the official
///   CLI reads, what it wrote for the policies (`cedar format`, in 80 and in 40 columns), the Cedar
///   of sekisho's golden files and of the other languages' tests, and the Cedar `sekisho gen`
///   writes for each version of sekisho's examples, are read whole.
/// - Of the policies and schemas the CLI does not read (`invalid/`, `invalid-schemas/`), the ones
///   it says have an invalid token have something the lexer could not read, and the others, whose
///   tokens the CLI read, nothing. A byte order mark is the one invalid token left out: Pygments
///   drops it from the start of a text before a lexer reads it, so no fence can show one.
#[test]
fn the_lexers_of_gate_and_cedar_read_what_the_tools_read() {
    let python = website().join(".venv/bin/python");
    if !ready(Need::Python, || python.is_file(), "Pygments is not in website/.venv; install Zensical as website/README.md says, to colour the code") {
        return;
    }
    let crates = root().join("crates");
    let fixtures = crates.join("ritsu-base/tests/fixtures/cedar");
    let invalid = |p: &Path| p.starts_with(fixtures.join("invalid")) || p.starts_with(fixtures.join("invalid-schemas"));
    // What sekisho writes for its examples, in English and in Japanese.
    let links = links();
    let t = TempDir::new("site-lexers");
    let mut generated = 0;
    for dir in fs::read_dir(crates.join("sekisho/examples")).unwrap().flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
        for gate in files_under(&dir, &[".gate"]).iter().filter(|g| g.parent() == Some(dir.as_path())) {
            let name = gate.file_name().unwrap().to_string_lossy().into_owned();
            let out = t.path().join(dir.file_name().unwrap()).join(&name);
            let lang = if name.ends_with(".ja.gate") { " --lang ja" } else { "" };
            let said = sh(links.path(), &dir, &format!("sekisho gen {name} --target cedar --out {}{lang}", out.display()));
            assert!(out.join("cedar").is_dir(), "sekisho gen {name} wrote no Cedar:\n{said}");
            generated += 1;
        }
    }
    // (alias, file, what the tools make of it): the tools read it, or they read its tokens and not
    // the whole, or it has a token they cannot read; only the last is to have one the lexer cannot
    let (read_whole, tokens_only, invalid_token) = ("read", "its tokens read", "an invalid token");
    let mut asked: Vec<(&str, PathBuf, &str)> = Vec::new();
    for g in files_under(&crates, &[".gate"]).into_iter().chain(files_under(&website().join("playground"), &[".gate"])) {
        let e001 = g.file_name().is_some_and(|n| n.to_string_lossy().starts_with("E001_")) && g.parent().is_some_and(|d| d.ends_with("tests/mutants"));
        if !e001 {
            asked.push(("gate", g, read_whole));
        }
    }
    for under in [crates.clone(), t.path().to_path_buf()] {
        for f in files_under(&under, &[".cedar", ".cedarschema", ".fmt", ".fmt40"]) {
            let name = f.file_name().unwrap().to_string_lossy().into_owned();
            if name.ends_with(".fmt") || name.ends_with(".fmt40") {
                // what `cedar format` wrote, beside the policies it formatted
                if f.starts_with(&fixtures) && !invalid(&f) {
                    asked.push(("cedar", f, read_whole));
                }
                continue;
            }
            let alias = if name.ends_with(".cedarschema") { "cedarschema" } else { "cedar" };
            if !invalid(&f) {
                asked.push((alias, f, read_whole));
                continue;
            }
            if name.contains("byte-order-mark") {
                continue;
            }
            let err = if alias == "cedar" { f.with_extension("err") } else { PathBuf::from(format!("{}.err", f.display())) };
            let said = read(&err);
            asked.push((alias, f, if said.contains("invalid token") { invalid_token } else { tokens_only }));
        }
    }
    let mut child = Command::new(&python).args(["-I", "-B", "-c", LEX]).arg(website().join("tools")).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().expect("could not run website/.venv/bin/python");
    let input: String = asked.iter().map(|(alias, f, _)| format!("{alias}\t{}\n", f.display())).collect();
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    let o = child.wait_with_output().unwrap();
    assert!(o.status.success(), "the lexers did not read the files:\n{}", String::from_utf8_lossy(&o.stderr));
    let found: BTreeMap<String, usize> = String::from_utf8(o.stdout).unwrap().lines().filter_map(|l| l.split_once('\t')).map(|(n, p)| (p.to_string(), n.parse().unwrap())).collect();
    let mut wrong = Vec::new();
    let mut counted: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    for (alias, f, made) in &asked {
        let shown = f.strip_prefix(root()).or_else(|_| f.strip_prefix(t.path())).unwrap_or(f).display().to_string();
        let Some(n) = found.get(&f.display().to_string()) else {
            wrong.push(format!("{shown}: the lexer of {alias} said nothing of it"));
            continue;
        };
        match (*n > 0, *made == invalid_token) {
            (true, false) => wrong.push(format!("{shown}: the lexer of {alias} could not read {n} of its tokens, which the tools read")),
            (false, true) => wrong.push(format!("{shown}: the official Cedar CLI says it has an invalid token, and the lexer of {alias} reads every token of it")),
            _ => {}
        }
        *counted.entry((alias, made)).or_default() += 1;
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    // 3 examples of sekisho in two versions; the gates of sekisho's examples, tests and mutants, the
    // playground's and the other languages' tests'; the policies, schemas and formatted policies of
    // the materials and of the golden files; the invalid ones
    assert!(generated >= 6, "{generated} versions of sekisho's examples: were they moved?");
    for (key, least) in [(("gate", read_whole), 150), (("cedar", read_whole), 200), (("cedarschema", read_whole), 40), (("cedar", tokens_only), 50), (("cedar", invalid_token), 4)] {
        let n = counted.get(&key).copied().unwrap_or(0);
        assert!(n >= least, "{n} files of {:?}: were the materials moved?", key);
    }
    eprintln!("compared: what the lexers of gate and Cedar read of {} files with what the tools make of them, by language {counted:?}", asked.len());
}
