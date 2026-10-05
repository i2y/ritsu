//! ritsu's site (website/, DESIGN 13.2): ritsu's own pages in English and Japanese, and below them
//! the site of each language that website/build.sh builds, each with its own configuration in a
//! directory of its own (website/rulec and website/dandori, the sites the two languages had in
//! their own repositories), all built into one tree, website/build, which is published at
//! https://i2y.github.io/ritsu/. This holds:
//!
//! - every relative link of ritsu's pages leads to one of its pages, or to the site of a language
//!   that build.sh builds, and every `#anchor` to a heading of the page it names;
//! - every link into ritsu's repository, on ritsu's pages and on the pages of every language's
//!   site, names a file or a directory that is there, and no page links to the repository of one
//!   language by itself (only to its releases, which still hand out rulec until ritsu's first
//!   release takes them over);
//! - the code on the index pages is the lines of files of the languages, `ritsu check` prints what
//!   they show under it, and a command the prose names is one;
//! - build.sh builds rulec's site and dandori's; both index pages link to the site of every
//!   language build.sh builds, and each such site is there, with its own build.sh and
//!   configurations;
//! - the configurations name the URL each site is published at, under /ritsu/, and ritsu's
//!   repository;
//! - .github/workflows/docs.yml runs on a push to main that changes what the site is built from,
//!   and by hand;
//! - the pages the playgrounds of rulec's and dandori's sites had (`playground.md` in each language)
//!   send a reader on to ritsu's playground in the same language, on a project it opens, and the nav
//!   and the home page of each site link to the same place (tests/playground.rs follows them in
//!   Chrome);
//! - the pages of a language's namespace (website/docs/ns/yuen.md and website/docs-ja/ns/yuen.md,
//!   which the IRIs of yuen's PROV words open: `https://i2y.github.io/ritsu/ns/yuen#Requirement`)
//!   say every word yuen writes and no other, each once, under the heading of its kind, with the
//!   types it is written on, as the id the IRI's `#` names; and the example on them is what the
//!   command prints;
//! - and, when Zensical is in website/.venv, build.sh builds the tree that is published, in a copy
//!   of website/: the index pages of ritsu's site and of each language's, in both languages, the
//!   playground, the pages the languages' playgrounds had, each with nothing beside it and its link
//!   and the nav of its site leading to the playground in the tree, the marketplace of Claude Code as
//!   it is written, and a file of the tree for every link of ritsu's built pages that stays in the
//!   site. Without Zensical the test says SKIP.
//!
//! (tests/common/mod.rs holds what this file shares with tests/readme.rs.)

mod common;

use common::{Page, code_from_files, commands_named, inline_code, link_targets, page, root, run_the_consoles};
use ritsu_testkit::{Need, TempDir, ready};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use yuen::export::prov::{Kind, TERMS, YUEN_NS};

/// Where the site is published.
const PUBLISHED: &str = "https://i2y.github.io/ritsu/";

/// The sites of the languages that ritsu's site holds: the ones rulec and dandori had in their own
/// repositories, moved into website/<language> (DESIGN 13.2).
const HELD: [&str; 2] = ["rulec", "dandori"];

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

/// The Markdown a path of the built site is made from: ritsu's pages at the root and under `/ja/`,
/// a language's site under `/<language>/` and `/<language>/ja/`.
fn source_of(url: &str, sites: &[String]) -> Option<PathBuf> {
    let mut parts: Vec<&str> = url.split('/').filter(|p| !p.is_empty()).collect();
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
    let candidates = if rest.is_empty() { vec![d.join("index.md")] } else { vec![d.join(format!("{rest}.md")), d.join(&rest).join("index.md"), d.join(&rest)] };
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
    for (p, url) in pages() {
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
                // A path of the built site, from the page's place in it.
                source_of(&normal(&format!("{url}{file}")), &sites)
            };
            match (to, frag) {
                (None, _) => wrong.push(format!("{}:{line}: `{target}` leads to no page of the site", p.name)),
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
            // named. Its releases are another thing: until ritsu's first release takes them over,
            // rulec's binaries, packages and action are still rulec's own releases (its install page).
            let mut rest = line;
            while let Some(i) = rest.find("github.com/i2y/") {
                let repo: String = rest[i + 15..].chars().take_while(|c| c.is_alphanumeric() || *c == '-').collect();
                let releases = rest[i + 15 + repo.len()..].starts_with("/releases");
                if repo != "ritsu" && !releases {
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
    // `ritsu check .` on the two files, on each of the two pages.
    assert!(ran >= 2, "{ran} commands run: were the fences changed?");
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
    let want = ["push:", "branches: [main]", "paths:", "- \"website/**\"", "- \"crates/rulec/AGENTS.md\"", "- \"crates/rulec/docs/**\"", "- \".github/workflows/docs.yml\"", "workflow_dispatch:"];
    assert_eq!(triggers, want, "docs.yml runs on a push to main that changes what the site is built from (website/, the documents rulec's sync.sh copies in, and itself), and by hand");
    assert!(text.contains("./build.sh") && text.contains("path: website/build"), "docs.yml builds with website/build.sh and publishes website/build");
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

/// What a link of the playground opens it on (`#project=rulec/gap`, `#flow=tests/…`): a project the
/// page lists in its language, by projects.json.
fn opens_a_project(fragment: &str, ja: bool) -> bool {
    let v: serde_json::Value = serde_json::from_str(&read(&website().join("docs/playground/projects.json"))).expect("projects.json");
    let lang = if ja { "ja" } else { "en" };
    let listed = |p: &&serde_json::Value| p["lang"].as_str().is_none_or(|l| l == lang);
    let projects: Vec<&serde_json::Value> = v["projects"].as_array().unwrap().iter().filter(listed).collect();
    if let Some(name) = fragment.strip_prefix("project=") {
        projects.iter().any(|p| p["name"] == name)
    } else if let Some(flow) = fragment.strip_prefix("flow=") {
        projects.iter().any(|p| p["group"] == "dandori" && p["open"] == flow)
    } else {
        false
    }
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
    let o = Command::new("bash").arg(copy.join("build.sh")).output().expect("could not run bash");
    assert!(o.status.success(), "build.sh failed:\n{}\n{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    let built = copy.join("build");

    let mut want: Vec<String> = ["index.html", "ja/index.html", "playground/index.html", "playground/ritsu.wasm", "ja/playground/index.html", "ja/playground/projects.json", "ns/yuen/index.html", "ja/ns/yuen/index.html"].map(String::from).to_vec();
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
            } else {
                normal(&format!("{url}{path}"))
            };
            seen += 1;
            let f = built.join(at.trim_start_matches('/'));
            if !(f.is_file() || f.join("index.html").is_file()) {
                wrong.push(format!("{file} (from {}): `{target}` is no file of the built site", p.name));
            }
        }
    }
    assert!(seen >= 20, "{seen} links on ritsu's built pages: was the tree built?");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    eprintln!("compared: {} files of the tree build.sh built, and {seen} links of ritsu's built pages", want.len());
}
