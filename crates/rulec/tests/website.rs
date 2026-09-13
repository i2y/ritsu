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
    &["index.md", "install.md", "tour.md", "checks.md", "generate.md", "compare.md", "examples.md"];

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
            // `?v=…` is a cache-busting stamp, not part of the path on disk.
            let path = target.split('#').next().unwrap().split('?').next().unwrap();
            if path.is_empty() || SYNCED.contains(&path) {
                continue; // copied in by sync.sh
            }
            assert!(root().join(&dir).join(path).exists(), "{name}: リンク先が無い: {target}");
        }
    }
}

/// The examples page shows ten corpus rules in full. They are the rules the rest of the
/// suite runs — `check` passes, the examples execute, and the three implementations agree —
/// so the value of the page is entirely in the sources on it being *those* sources. A page
/// that drifts by one cell is worse than no page: it shows a rule nothing has ever run.
#[test]
fn 例のページの規則はコーパスと一字一句同じ() {
    for lang in ["docs", "docs-ja"] {
        let page = read(&format!("website/{lang}/examples.md"));
        let blocks: Vec<&str> = page
            .split("\n```\n")
            .skip(1)
            .step_by(2)
            .collect();
        assert!(blocks.len() >= 10, "{lang}: 例が少なすぎる ({} 本)", blocks.len());
        for b in &blocks {
            // The first line is `rule 名前(alias) v1`; the corpus file is named after it.
            let name = b
                .lines()
                .next()
                .and_then(|l| l.strip_prefix("rule "))
                .and_then(|l| l.split('(').next())
                .unwrap_or_else(|| panic!("{lang}: 規則の行で始まっていない: {b}"));
            let want = read(&format!("tests/corpus/{name}.rule"));
            assert_eq!(
                b.trim_end(),
                want.trim_end(),
                "{lang}: 例のページの {name} がコーパスと違う。`python3 website/tools/make_examples.py` で作り直してください"
            );
        }
    }
}

/// Both languages show the same rules in the same order. The language switcher only swaps a
/// path prefix, so a reader who switches mid-page should land on the same example.
#[test]
fn 例のページは両言語で同じ規則を同じ順に並べる() {
    let names = |lang: &str| -> Vec<String> {
        read(&format!("website/{lang}/examples.md"))
            .split("\n```\n")
            .skip(1)
            .step_by(2)
            .filter_map(|b| b.lines().next()?.strip_prefix("rule ")?.split('(').next().map(String::from))
            .collect()
    };
    assert_eq!(names("docs"), names("docs-ja"), "例の並びが言語で違う");
}

/// The opening diagram draws generated code, a table and a diagnostic witness. All three are
/// things the tool produces, so all three can go stale the moment the generator changes — and
/// a diagram that shows output nobody can reproduce is worse than no diagram. Its script
/// carries a `--verify` mode that holds every one of those to the real thing; this runs it.
#[test]
fn 図が見せている出力は本物と一致する() {
    if !Command::new("python3").arg("--version").output().map(|o| o.status.success()).unwrap_or(false) {
        eprintln!("注意: python3 が無いので飛ばした");
        return;
    }
    for (script, prefix) in DIAGRAMS {
        // Regenerating writes the four SVGs, so their bytes are kept and put back: a test
        // has no business leaving the working tree different from how it found it.
        let images = root().join("website/docs/images");
        let files: Vec<std::path::PathBuf> = std::fs::read_dir(&images)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with(prefix)))
            .collect();
        let before: Vec<(std::path::PathBuf, Vec<u8>)> =
            files.iter().map(|p| (p.clone(), std::fs::read(p).unwrap())).collect();

        let o = Command::new("python3")
            .current_dir(root().join("website"))
            .args([script, "--verify", env!("CARGO_BIN_EXE_rulec")])
            .output()
            .expect("python3 を起動できない");
        let stale: Vec<String> = before
            .iter()
            .filter(|(p, was)| std::fs::read(p).ok().as_ref() != Some(was))
            .map(|(p, _)| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        for (p, was) in &before {
            let _ = std::fs::write(p, was);
        }

        assert!(
            o.status.success(),
            "図が見せている出力が実物とずれています。`python3 {script}` で作り直してください:\n{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        );
        assert!(
            stale.is_empty(),
            "図が古いままです。`python3 {script}` で作り直してください: {stale:?}"
        );
    }
}

/// The two generated diagrams: the script that draws one, and the stem its four files share.
const DIAGRAMS: [(&str, &str); 3] = [
    ("tools/make_overview.py", "overview"),
    ("tools/make_checks.py", "checks"),
    ("tools/make_stack.py", "stack"),
];

/// The opening diagram's URL carries a hash of the diagram's own bytes. Without it a reader
/// who has been to the site before keeps seeing the previous picture — the filename never
/// changes, so nothing tells their browser to fetch again. This is not a preview nicety: it
/// is how a redeploy reaches somebody who already has the page cached.
#[test]
fn 図のurlは中身のハッシュを持っている() {
    let want = diagram_version();
    for lang in ["docs", "docs-ja"] {
        let page = read(&format!("website/{lang}/index.md"));
        let refs: Vec<&str> = DIAGRAMS
            .iter()
            .flat_map(|(_, stem)| {
                let mark = format!("images/{stem}");
                page.match_indices(&mark)
                    .map(|(i, _)| {
                        let rest = &page[i..];
                        &rest[..rest.find(')').unwrap_or(rest.len())]
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        // Two per diagram: one for the dark scheme and one for the light.
        let want_refs = DIAGRAMS.len() * 2;
        assert_eq!(refs.len(), want_refs, "{lang}: 図の参照が {want_refs} つでない: {refs:?}");
        for r in refs {
            assert!(
                r.contains(&format!("?v={want}")),
                "{lang}: 図の URL のハッシュが中身と違う。`?v={want}` にしてください: {r}"
            );
        }
    }
}

/// The eight hex characters stamped into those URLs: a hash over every dark SVG, which
/// changes together with its light twin. One stamp covers both diagrams, so editing one
/// costs the other a single refetch — cheaper than two stamps to keep straight.
fn diagram_version() -> String {
    let mut bytes = Vec::new();
    for (_, stem) in DIAGRAMS {
        for lang in ["-ja", ""] {
            bytes.extend(read(&format!("website/docs/images/{stem}{lang}.svg")).into_bytes());
        }
    }
    // A small, dependency-free digest. It only has to change when the files do.
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")[..8].to_string()
}
