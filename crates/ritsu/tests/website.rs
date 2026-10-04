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
//! - .github/workflows/docs.yml runs only by hand;
//! - and, when Zensical is in website/.venv, build.sh builds the tree that is published, in a copy
//!   of website/: the index pages of ritsu's site and of each language's, in both languages, the
//!   playgrounds, and a file of the tree for every link of ritsu's built pages that stays in the
//!   site. Without Zensical the test says SKIP.
//!
//! (tests/common/mod.rs holds what this file shares with tests/readme.rs.)

mod common;

use common::{Page, code_from_files, commands_named, link_targets, page, root, run_the_consoles};
use ritsu_testkit::{Need, TempDir, ready};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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
/// `/ja/`, …).
fn pages() -> Vec<(Page, String)> {
    let mut out = Vec::new();
    for (dir, at) in [("docs", "/"), ("docs-ja", "/ja/")] {
        for e in fs::read_dir(website().join(dir)).unwrap().flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let Some(stem) = name.strip_suffix(".md") else { continue };
            let url = if stem == "index" { at.to_string() } else { format!("{at}{stem}/") };
            out.push((page(&format!("website/{dir}/{name}")), url));
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
fn the_workflow_that_publishes_the_site_runs_only_by_hand() {
    let text = read(&root().join(".github/workflows/docs.yml"));
    let mut lines = text.lines().skip_while(|l| *l != "on:");
    assert_eq!(lines.next(), Some("on:"), "docs.yml has no `on:`");
    let triggers: Vec<&str> = lines.take_while(|l| l.is_empty() || l.starts_with(' ')).map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')).collect();
    assert_eq!(triggers, ["workflow_dispatch:"], "docs.yml runs only by hand until the sites are switched over");
    assert!(text.contains("./build.sh") && text.contains("path: website/build"), "docs.yml builds with website/build.sh and publishes website/build");
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

    let mut want: Vec<String> = ["index.html", "ja/index.html", "playground/index.html", "playground/ritsu.wasm", "ja/playground/index.html", "ja/playground/projects.json"].map(String::from).to_vec();
    for s in sites() {
        want.extend([format!("{s}/index.html"), format!("{s}/ja/index.html")]);
        if website().join(&s).join("docs/playground.md").is_file() {
            want.extend([format!("{s}/playground/index.html"), format!("{s}/ja/playground/index.html")]);
        }
    }
    let missing: Vec<&String> = want.iter().filter(|w| !built.join(w).is_file()).collect();
    assert!(missing.is_empty(), "build.sh built no {missing:?}");

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
