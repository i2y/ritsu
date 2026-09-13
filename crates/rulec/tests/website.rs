//! The documentation site must not promise what the tool does not do either.
//!
//! `website/docs/` and `website/docs-ja/` hold six authored pages each; the rest
//! are copied in from this repository by `website/sync.sh` and are not committed,
//! so they are checked where they live (`tests/docs.rs`, `tests/codes.rs`,
//! `tests/formats.rs`, `tests/api.rs`). What is checked here is the site's own
//! seams: that both languages carry the same pages, that every page the nav
//! names will exist after a sync, and that the authored pages name real commands
//! and link to pages that are really there.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The pages `website/sync.sh` copies in. They are gitignored, so a page may be
/// linked or listed in the nav without being on disk in a fresh checkout.
const SYNCED: &[&str] = &["agents.md", "reference.md", "formats.md", "generated-code.md", "codes.md"];

/// The pages written for the site itself, in both languages.
const AUTHORED: &[&str] =
    &["index.md", "install.md", "tour.md", "checks.md", "generate.md", "compare.md"];

fn read(rel: &str) -> String {
    std::fs::read_to_string(root().join(rel)).unwrap_or_else(|_| panic!("読めない: {rel}"))
}

/// The `"page.md"` targets of a config's `nav`, in order.
fn nav_targets(config: &str) -> Vec<String> {
    let body = read(config);
    let nav = body
        .split("nav = [")
        .nth(1)
        .unwrap_or_else(|| panic!("{config} に nav が無い"))
        .split("\n\n")
        .next()
        .unwrap();
    let mut out = Vec::new();
    let mut rest = nav;
    while let Some(i) = rest.find(".md\"") {
        let start = rest[..i].rfind('"').unwrap() + 1;
        out.push(rest[start..i + 3].to_string());
        rest = &rest[i + 4..];
    }
    out
}

#[test]
fn 両言語が同じページを持つ() {
    let en = nav_targets("website/zensical.toml");
    let ja = nav_targets("website/zensical.ja.toml");
    // The language switcher rewrites only the prefix of the path, so a page that
    // exists in one language and not the other is a 404 waiting to happen.
    assert_eq!(en, ja, "英語と日本語の nav が指すページが違う");
    assert!(!en.is_empty());
}

#[test]
fn navが指すページは同期後に存在する() {
    for (config, dir) in
        [("website/zensical.toml", "website/docs"), ("website/zensical.ja.toml", "website/docs-ja")]
    {
        for page in nav_targets(config) {
            if SYNCED.contains(&page.as_str()) {
                continue; // copied in by sync.sh
            }
            assert!(
                root().join(dir).join(&page).exists(),
                "{config}: nav の {page} が {dir} に無い"
            );
        }
    }
}

#[test]
fn 書き下ろしのページは両言語に揃っている() {
    for dir in ["website/docs", "website/docs-ja"] {
        for page in AUTHORED {
            assert!(root().join(dir).join(page).exists(), "{dir}/{page} が無い");
        }
    }
    // And nothing authored is missing from the nav.
    let en = nav_targets("website/zensical.toml");
    for page in AUTHORED {
        assert!(en.contains(&page.to_string()), "{page} が nav に無い");
    }
}

/// Every page the authored pages link to, and every page `sync.sh` writes,
/// have to be the same set the nav lists — otherwise a page is built and
/// unreachable, or linked and never built.
#[test]
fn 同期するページとnavが一致する() {
    let sync = read("website/sync.sh");
    let listed: BTreeSet<&str> = SYNCED.iter().copied().collect();
    for page in &listed {
        assert!(sync.contains(&format!("{page}")), "sync.sh が {page} を作らない");
    }
    let nav: BTreeSet<String> = nav_targets("website/zensical.toml").into_iter().collect();
    let want: BTreeSet<String> =
        AUTHORED.iter().chain(SYNCED.iter()).map(|s| s.to_string()).collect();
    assert_eq!(nav, want, "nav と（書き下ろし + 同期）の集合が違う");
}

fn authored_pages() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for dir in ["website/docs", "website/docs-ja"] {
        for page in AUTHORED {
            out.push((format!("{dir}/{page}"), read(&format!("{dir}/{page}"))));
        }
    }
    out
}

#[test]
fn サイトが名指しするコマンドは実在する() {
    let o = Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(root())
        .args(["--help", "--lang", "en"])
        .output()
        .expect("rulec を起動できない");
    let help = String::from_utf8_lossy(&o.stdout).into_owned();
    let have: BTreeSet<String> = help
        .lines()
        .filter_map(|l| l.strip_prefix("  rulec "))
        .filter_map(|l| l.split_whitespace().next())
        .map(|s| s.to_string())
        .collect();
    let extra = ["help", "--help", "--version", "-h", "-V"];
    for (name, body) in authored_pages() {
        for (i, l) in body.lines().enumerate() {
            for lead in ["`rulec ", "$ rulec ", "run: rulec "] {
                let mut rest = l;
                while let Some(k) = rest.find(lead) {
                    rest = &rest[k + lead.len()..];
                    let Some(w) = rest.split(|c: char| c.is_whitespace() || c == '`').next() else {
                        continue;
                    };
                    if w.is_empty() || !w.chars().all(|c| c.is_ascii_lowercase() || c == '-') {
                        continue;
                    }
                    assert!(
                        have.contains(w) || extra.contains(&w),
                        "{name}:{}: `rulec {w}` というコマンドは無い",
                        i + 1
                    );
                }
            }
        }
    }
}

#[test]
fn サイトの相対リンクは実在する() {
    for (name, body) in authored_pages() {
        let dir = name.rsplit_once('/').unwrap().0.to_string();
        let mut rest = body.as_str();
        while let Some(k) = rest.find("](") {
            rest = &rest[k + 2..];
            let Some(end) = rest.find(')') else { break };
            let target = &rest[..end];
            if target.starts_with("http") || target.starts_with('#') {
                continue;
            }
            let path = target.split('#').next().unwrap();
            if path.is_empty() || SYNCED.contains(&path) {
                continue; // copied in by sync.sh
            }
            assert!(root().join(&dir).join(path).exists(), "{name}: リンク先が無い: {target}");
        }
    }
}
