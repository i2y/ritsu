//! The documentation site must not promise what the tool does not do either.
//!
//! `website/docs/` and `website/docs-ja/` hold ten authored pages each; the rest
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
const SYNCED: &[&str] =
    &["agents.md", "reference.md", "formats.md", "generated-code.md", "backends.md", "codes.md"];

/// The pages written for the site itself, in both languages.
const AUTHORED: &[&str] = &[
    "index.md",
    "playground.md",
    "install.md",
    "scenarios.md",
    "tour.md",
    "checks.md",
    "assurance.md",
    "generate.md",
    "compare.md",
    "examples.md",
    "fit.md",
];

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
        // Every source on the page opens with the ```rule fence that colours it.
        let blocks: Vec<&str> = page
            .split("\n```rule\n")
            .skip(1)
            .map(|b| b.split("\n```").next().unwrap())
            .collect();
        assert!(blocks.len() >= 10, "{lang}: 例が少なすぎる ({} 本)", blocks.len());
        for b in &blocks {
            // The first line is `rule 名前(alias) v1`; the corpus file is named after it.
            let name = b
                .lines()
                .next()
                .and_then(|l| l.strip_prefix("rule "))
                // `rule 送料例(fee_demo) v1`, or `rule ec261 v1` when the name is already
                // ASCII and needs no alias.
                .and_then(|l| l.split(['(', ' ']).next())
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

/// The examples page is generated too, prose and all, and nothing else holds that prose to
/// the script that writes it.
///
/// The rule sources are checked above, against the corpus. The sentences around them are
/// not, and they carry claims that go stale: the page said the reference evaluator, Python
/// and Go agree byte for byte, which stopped being the whole list when the fourth target
/// landed. The committed pages were corrected by hand; the generator was not, so running it
/// put the old sentence back over the fix. A generator that silently undoes a correction is
/// worse than no generator, so it is held to its own output the same way the diagrams are.
#[test]
fn 例のページは作り直しても変わらない() {
    if !Command::new("python3").arg("--version").output().map(|o| o.status.success()).unwrap_or(false) {
        eprintln!("注意: python3 が無いので飛ばした");
        return;
    }
    let pages = ["website/docs/examples.md", "website/docs-ja/examples.md"];
    let before: Vec<(std::path::PathBuf, Vec<u8>)> =
        pages.iter().map(|r| root().join(r)).map(|p| (p.clone(), std::fs::read(&p).unwrap())).collect();

    let o = Command::new("python3")
        .current_dir(root().join("website"))
        .arg("tools/make_examples.py")
        .output()
        .expect("python3 を起動できない");

    let stale: Vec<String> = before
        .iter()
        .filter(|(p, was)| std::fs::read(p).ok().as_ref() != Some(was))
        .map(|(p, _)| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    // Put the bytes back before asserting: a test has no business leaving the working tree
    // different from how it found it, least of all when it is about to fail.
    for (p, was) in &before {
        let _ = std::fs::write(p, was);
    }

    assert!(
        o.status.success(),
        "make_examples.py が失敗しました:\n{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(
        stale.is_empty(),
        "例のページと、それを書くスクリプトがずれています（どちらが古いかは差分を見てください）: {stale:?}"
    );
}

/// The generated diagrams: the script that draws one, and the stem its four files share.
const DIAGRAMS: [(&str, &str); 5] = [
    ("tools/make_overview.py", "overview"),
    ("tools/make_checks.py", "checks"),
    ("tools/make_stack.py", "stack"),
    ("tools/make_flow.py", "flow"),
    ("tools/make_assurance.py", "assurance"),
];

/// The four the front page carries. The stamp below is theirs, because they are the ones a
/// returning reader has cached; a diagram further in is fetched the first time either way.
/// Every diagram whose URL carries the hash, in a fixed order. The hash is over their
/// bytes, so **the order and the membership must not change when a diagram merely moves to
/// another page** — otherwise every URL would have to be re-stamped for no reason.
const STAMPED: [&str; 4] = ["overview", "checks", "stack", "flow"];

/// Of those, the ones on the front page.
const FRONT: [&str; 3] = ["overview", "checks", "flow"];

/// Diagrams that live on a page other than the front one, and the page they live on. The
/// stacking picture followed its explanation to the walkthrough; the hash has to follow it.
const ELSEWHERE: [(&str, &str); 1] = [("stack", "tour.md")];

/// The opening diagram's URL carries a hash of the diagram's own bytes. Without it a reader
/// who has been to the site before keeps seeing the previous picture — the filename never
/// changes, so nothing tells their browser to fetch again. This is not a preview nicety: it
/// is how a redeploy reaches somebody who already has the page cached.
#[test]
fn 図のurlは中身のハッシュを持っている() {
    let want = diagram_version();
    // 刻印する図は、トップにあるものとそれ以外で過不足なく分かれている。
    let mut named: Vec<&str> = FRONT.iter().chain(ELSEWHERE.iter().map(|(s, _)| s)).copied().collect();
    named.sort();
    let mut all: Vec<&str> = STAMPED.to_vec();
    all.sort();
    assert_eq!(named, all, "STAMPED と FRONT/ELSEWHERE が食い違っている");
    for lang in ["docs", "docs-ja"] {
        let page = read(&format!("website/{lang}/index.md"));
        let refs: Vec<&str> = FRONT
            .iter()
            .flat_map(|stem| {
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
        let want_refs = FRONT.len() * 2;
        assert_eq!(refs.len(), want_refs, "{lang}: 図の参照が {want_refs} つでない: {refs:?}");
        for r in refs {
            assert!(
                r.contains(&format!("?v={want}")),
                "{lang}: 図の URL のハッシュが中身と違う。`?v={want}` にしてください: {r}"
            );
        }
        for (stem, page_name) in ELSEWHERE {
            let other = read(&format!("website/{lang}/{page_name}"));
            let mark = format!("images/{stem}");
            let refs: Vec<&str> = other
                .match_indices(&mark)
                .map(|(i, _)| {
                    let rest = &other[i..];
                    &rest[..rest.find(')').unwrap_or(rest.len())]
                })
                .collect();
            assert_eq!(refs.len(), 2, "{lang}/{page_name}: {stem} の参照が 2 つでない: {refs:?}");
            for r in refs {
                assert!(
                    r.contains(&format!("?v={want}")),
                    "{lang}/{page_name}: 図の URL のハッシュが中身と違う: {r}"
                );
            }
        }
    }
}

/// The eight hex characters stamped into those URLs: a hash over every dark SVG, which
/// changes together with its light twin. One stamp covers both diagrams, so editing one
/// costs the other a single refetch — cheaper than two stamps to keep straight.
fn diagram_version() -> String {
    let mut bytes = Vec::new();
    for stem in STAMPED {
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

/// The lexer that colours `.rule` on the site keeps the language's vocabulary a second
/// time, in Python. §0 of PLAN says the vocabulary lives in `src/kw.rs` and nowhere else,
/// so the copy is held to the original here: a word added to `kw.rs` and forgotten in the
/// lexer would simply stop being coloured, without anything failing.
#[test]
fn 色づけの語彙はkwと同じ() {
    use rulec::kw;
    let py = read("website/tools/rulelexer.py");
    let list = |name: &str| -> Vec<String> {
        let head = format!("\n{name} = (");
        let at = py.find(&head).unwrap_or_else(|| panic!("rulelexer.py に {name} が無い"));
        let open = at + head.len() - 1;
        let close = open + py[open..].find(')').expect("閉じ括弧が無い");
        py[open + 1..close]
            .split(',')
            .map(|s| s.trim().trim_matches('"').to_string())
            .filter(|s| !s.is_empty())
            .collect()
    };
    let sorted = |v: &[&str]| {
        let mut v: Vec<String> = v.iter().map(|s| s.to_string()).collect();
        v.sort();
        v
    };
    let mut heads = list("HEAD_NAMED");
    heads.extend(list("HEAD_PLAIN"));
    heads.sort();
    let mut want_heads = vec![kw::RULE.to_string()];
    want_heads.extend(kw::LINE_HEAD.iter().map(|s| s.to_string()));
    want_heads.sort();
    assert_eq!(heads, want_heads, "レクサの行頭語が kw::LINE_HEAD と違います");

    for (name, want) in [
        ("MODIFIERS", sorted(&[kw::RANGE, kw::ROUND, kw::CONTRACT_ONLY, kw::DEFAULT, kw::STEP])),
        ("TYPES", sorted(&[kw::MONEY, kw::MASS, kw::LENGTH, kw::AREA, kw::VOLUME, kw::DURATION,
            kw::TEMPERATURE, kw::SOUND, kw::RATE, kw::NUMBER, kw::BOOL, kw::STRING, kw::DATE])),
        ("TAX", sorted(&[kw::INCL_TAX, kw::EXCL_TAX])),
        ("POLICIES", sorted(&[kw::UNIQUE, kw::FIRST])),
        ("ROUNDING", sorted(&[kw::UP, kw::DOWN, kw::HALF_UP, kw::HALF_DOWN, kw::HALF_EVEN])),
        ("CONSTANTS", sorted(&[kw::TRUE, kw::FALSE, kw::NONE, kw::STARTS_WITH])),
        ("ARMS", sorted(&[kw::OVER, kw::WHERE, kw::OF, kw::NEXT, kw::STOP, kw::WITH, kw::TAKE_UNIQUE, kw::TAKE_FIRST, kw::KEEP_MAX, kw::BY, kw::EMPTY, kw::EXHAUSTED, kw::HELD])),
        ("FUNCTIONS", sorted(&[kw::MIN, kw::MAX, kw::ALLOCATE])),
        ("CLAUSE", sorted(&[kw::WHEN, kw::THEN, kw::ALWAYS])),
        ("APPLY", sorted(&[kw::EXCEPT])),
    ] {
        let mut got = list(name);
        got.sort();
        assert_eq!(got, want, "レクサの {name} が kw と違います");
    }
    // The two that are written into the patterns rather than into a list.
    for w in [kw::NOT, kw::STD] {
        assert!(py.contains(w), "レクサが {w} を知りません");
    }

    // The net under all of it: every reserved word has to be in one of those lists. The
    // per-list checks above are written out by hand and rot the same way the lexer does —
    // `half_down` was added to the language and to neither, and simply stopped being
    // coloured. This one cannot be satisfied by forgetting.
    let known: BTreeSet<String> = ["HEAD_NAMED", "HEAD_PLAIN", "MODIFIERS", "TYPES", "TAX",
        "POLICIES", "ROUNDING", "CONSTANTS", "ARMS", "FUNCTIONS", "CLAUSE", "APPLY"]
        .iter()
        .flat_map(|n| list(n))
        .collect();
    for w in kw::RESERVED {
        assert!(
            known.contains(*w) || *w == kw::NOT,
            "レクサの語彙に {w} がありません（色が付かないまま通ってしまいます）"
        );
    }
}

/// A `.rule` fence without its `rule` tag is not coloured, and nothing else goes wrong, so
/// a forgotten tag would never be noticed. Every fenced block that holds rule source — in
/// the pages, in the README, and in what the tool itself writes — carries the tag.
#[test]
fn 規則のコード片には札が付いている() {
    const HEADS: [&str; 13] = [
        "rule ", "description ", "import ", "enum ", "group ", "inputs", "outputs",
        "derive ", "define ", "table ", "policy ", "result ", "examples",
    ];
    let mut bare: Vec<String> = Vec::new();
    let mut files: Vec<String> = vec!["README.md".into()];
    for dir in ["docs", "website/docs", "website/docs-ja"] {
        let mut here: Vec<String> = std::fs::read_dir(root().join(dir))
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "md"))
            .map(|p| format!("{dir}/{}", p.file_name().unwrap().to_string_lossy()))
            .collect();
        here.sort();
        files.append(&mut here);
    }
    for rel in files {
        let src = read(&rel);
        let lines: Vec<&str> = src.lines().collect();
        let mut i = 0;
        while i < lines.len() {
            if !lines[i].starts_with("```") {
                i += 1;
                continue;
            }
            let info = lines[i][3..].trim().to_string();
            let mut j = i + 1;
            while j < lines.len() && !lines[j].starts_with("```") {
                j += 1;
            }
            let body = &lines[i + 1..j];
            // Rule source is what starts with a line-head word, or is a table whose header
            // row carries the `->` that separates the input columns from the output ones.
            let looks = body.iter().any(|b| HEADS.iter().any(|h| b.starts_with(h)))
                || (body.iter().any(|b| b.trim_start().starts_with('|'))
                    && body.iter().any(|b| b.trim_start().starts_with('|') && b.contains("->")));
            if looks && info.is_empty() {
                bare.push(format!("{rel}:{}", i + 1));
            }
            i = j + 1;
        }
    }
    assert!(
        bare.is_empty(),
        "規則のコード片に ```rule の札がありません（付けると色が付きます）: {bare:?}"
    );
}

/// A rule the site shows whole is one that runs.
///
/// A block that opens with `rule` is showing a rule a reader can copy, and the pages say as
/// much — "it passes the checks as written", "copy any of them and it works". So every one
/// of them is put through the checker here, in both languages. The English pages are where
/// an invention would go unnoticed, because an English reader cannot tell a transcription
/// from a mock-up by looking at the words; but a rule that does not check is a broken
/// promise on either page, so neither is exempt. A fragment shown to explain one keyword is
/// not a rule and is not checked, and neither is one a line of `…` marks as abridged — an
/// abridged rule is not something to copy, and the mark is what says so.
#[test]
fn サイトが丸ごと見せる規則は検査を通る() {
    let mut seen = 0usize;
    for lang in ["docs", "docs-ja"] {
        for page in AUTHORED {
            let rel = format!("website/{lang}/{page}");
            let src = read(&rel);
            let mut rest = src.as_str();
            let mut at = 1usize;
            while let Some(k) = rest.find("```rule\n") {
                at += rest[..k].matches('\n').count();
                let open = k + "```rule\n".len();
                let close = rest[open..].find("```").expect("コードブロックが閉じていない") + open;
                let body = &rest[open..close];
                let abridged = body.lines().any(|l| matches!(l.trim(), "…" | "..."));
                if body.starts_with("rule ") && !abridged {
                    seen += 1;
                    // A rule that cites a file source reads it from beside itself, so the
                    // checker is pointed at the corpus directory: on the page the path is
                    // written as the corpus rule writes it.
                    let ds = rulec::check_source(body, "tests/corpus/from-the-site.rule");
                    assert!(
                        !rulec::has_error(&ds),
                        "{rel}:{at}: 丸ごと見せている規則が検査を通りません: {:?}",
                        ds.iter().filter(|d| d.code.starts_with('E')).map(|d| format!("{}: {}", d.code, d.title)).collect::<Vec<_>>()
                    );
                }
                at += rest[k..close].matches('\n').count();
                rest = &rest[close..];
            }
        }
    }
    assert!(seen >= 4, "丸ごとの規則が {seen} 本しか見つかりません。走査が壊れています");
}

/// The assurance page says how many rules, mutants and codes there are. Those numbers are
/// the page's whole point — a map of the evidence with a stale count on it is worse than no
/// map — so they are held to what is actually in the repository, in both languages.
#[test]
fn 確かめ方のページの件数は実物と合っている() {
    let count = |dir: &str| {
        std::fs::read_dir(root().join(dir))
            .unwrap()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "rule"))
            .count()
    };
    let rules = count("tests/corpus");
    let mutants = count("tests/mutants");
    let codes = rulec::codes::ledger().len();
    for (lang, want) in [
        ("docs", vec![format!("**{mutants} deliberately broken rules**"), format!("**{rules} rules**"), format!("{codes} codes")]),
        ("docs-ja", vec![format!("わざと壊した規則 {mutants} 本**"), format!("規則 {rules} 本**"), format!("{codes} 件")]),
    ] {
        let page = read(&format!("website/{lang}/assurance.md"));
        for w in want {
            assert!(page.contains(&w), "website/{lang}/assurance.md に「{w}」がありません");
        }
    }
}

/// The playground opens on the table the front page's first picture is about
/// (`website/tools/overview.rule`), so the two must be the same table. A page showing a
/// different one would be a second source for the same example — the failure `examples.md`
/// is held to, one page over.
#[test]
fn playgroundの表は絵の表と同じ() {
    let js = read("website/docs/playground/playground.js");
    for (key, rule) in
        [("en", "website/tools/overview.rule"), ("ja", "website/tools/overview-ja.rule")]
    {
        let open = format!("  {key}: `");
        let start = js.find(&open).unwrap_or_else(|| panic!("playground.js に {key} の表が無い"))
            + open.len();
        let got = &js[start..start + js[start..].find("`,").expect("表が閉じていない")];
        // The file opens with a comment block about the diagram; the page shows the table.
        let want: String = read(rule)
            .lines()
            .skip_while(|l| l.starts_with('#') || l.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        assert_eq!(got, want, "playground.js の {key} の表が {rule} と違う");
    }
}

/// **Every sample the playground offers is a rule the repository already checks.** The page
/// carries them as text — it is three files and fetches nothing — so the copy can drift from
/// the corpus, and a sample that drifted would be a rule nothing runs: the one place on the
/// site where a reader types into the checker, showing a table no test has seen.
///
/// The button that offers a sample lives in the markdown, so it is checked here too: a key
/// the script does not carry would put a button on the page that empties the box.
#[test]
fn playgroundのサンプルはコーパスの規則と一字一句同じ() {
    const SAMPLES: [(&str, &str); 6] = [
        ("en:multi", "tests/corpus/parcel_rate.rule"),
        ("ja:multi", "tests/corpus/送料.rule"),
        ("en:big", "tests/corpus/Claude利用料.rule"),
        ("ja:big", "tests/corpus/クーポン割引.rule"),
        ("en:walk", "tests/corpus/shipment_surcharge.rule"),
        ("ja:walk", "tests/corpus/買物かごの送料.rule"),
    ];
    let js = read("website/docs/playground/playground.js");
    for (key, rule) in SAMPLES {
        let open = format!("  \"{key}\": `");
        let start = js
            .find(&open)
            .unwrap_or_else(|| panic!("playground.js に {key} が無い。website/tools/samples.py で作り直してください"))
            + open.len();
        let got = &js[start..start + js[start..].find("`,").expect("規則が閉じていない")];
        assert_eq!(got, read(rule), "playground.js の {key} が {rule} と違う。website/tools/samples.py で作り直してください");
    }
    // Both pages offer the same set, and offer nothing the script cannot serve.
    for page in ["website/docs/playground.md", "website/docs-ja/playground.md"] {
        let md = read(page);
        let keys: Vec<&str> = md
            .match_indices("data-preset=\"")
            .map(|(i, m)| {
                let rest = &md[i + m.len()..];
                &rest[..rest.find('"').expect("data-preset が閉じていない")]
            })
            .collect();
        assert_eq!(keys, ["gap", "full", "multi", "big", "walk"], "{page} の並びが違う");
    }
}

/// The count of target languages, written out in words. `tests/docs.rs` holds the *list* to
/// `src/backend.rs` wherever a document enumerates it, but a sentence that says "seven
/// languages" names none of them, so nothing caught the home page saying that after the
/// eighth arrived. The registry knows the number; a page that spells it out has to agree.
const COUNTS: [(&str, usize); 18] = [
    ("六つの言語", 6),
    ("七つの言語", 7),
    ("八つの言語", 8),
    ("九つの言語", 9),
    ("六言語", 6),
    ("七言語", 7),
    ("八言語", 8),
    ("九言語", 9),
    ("six languages", 6),
    ("seven languages", 7),
    ("eight languages", 8),
    ("nine languages", 9),
    ("十言語", 10),
    ("十一言語", 11),
    ("十の言語", 10),
    ("十一の言語", 11),
    ("ten languages", 10),
    ("eleven languages", 11),
];

/// The same word-boundary rule `tests/docs.rs` applies: `JavaScript` does not name Java.
fn named_as_word(para: &str, n: &str) -> bool {
    para.match_indices(n).any(|(i, _)| {
        let before = para[..i].chars().next_back();
        let after = para[i + n.len()..].chars().next();
        !before.is_some_and(|c| c.is_ascii_alphanumeric()) && !after.is_some_and(|c| c.is_ascii_alphanumeric())
    })
}

fn repo_docs() -> Vec<(String, String)> {
    let mut out = vec![("README.md".to_string(), read("README.md")), ("AGENTS.md".to_string(), read("AGENTS.md"))];
    for e in std::fs::read_dir(root().join("docs")).unwrap().flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| x == "md") {
            out.push((format!("docs/{}", p.file_name().unwrap().to_string_lossy()), std::fs::read_to_string(&p).unwrap()));
        }
    }
    out
}

#[test]
fn 言語の数を書いた文は登録簿と合っている() {
    let want = rulec::backend::ALL.len();
    for (name, body) in authored_pages().into_iter().chain(repo_docs()) {
        for (phrase, n) in COUNTS {
            if body.contains(phrase) {
                assert_eq!(n, want, "{name}: 「{phrase}」と書いてありますが、登録簿は {want} です");
            }
        }
    }
}

/// The rule `tests/docs.rs` applies to the repository's documents, for the pages written for
/// the site: enumerating the languages and leaving one out.
///
/// The threshold here is **four**, where the repository's documents use three. These pages are
/// narrative and name languages as examples rather than as the set — "the third one,
/// TypeScript, cost about 700 lines; so did the fifth, Ruby, and the sixth, Swift" names three
/// and is not a list of anything. Four or more in one paragraph is a list, and a list that
/// drops one is the drift this catches: the two it found when it went in named six.
#[test]
fn サイトが並べる対象言語はレジストリと同じ() {
    let names: Vec<&str> = rulec::backend::ALL.iter().map(|b| b.name).collect();
    for (name, body) in authored_pages() {
        for para in body.split("\n\n") {
            if para.trim_start().starts_with("```") {
                continue;
            }
            let named = names.iter().filter(|n| named_as_word(para, n)).count();
            if named < 4 || named == names.len() {
                continue;
            }
            let missing: Vec<&&str> = names.iter().filter(|n| !named_as_word(para, n)).collect();
            panic!(
                "{name}: 対象言語を {named} つ並べて {missing:?} を落としています。\n  段落: {}",
                para.trim().chars().take(160).collect::<String>()
            );
        }
    }
}

/// The numbers about Kani on the pages, held to the harnesses `gen` actually writes and to
/// the run recorded in `experiments/kani/report.txt`.
///
/// Three copies of one count drifted apart before this test existed: the pages said 96, the
/// recorded run said 73, and the corpus had grown to 102. None of them was wrong when it
/// was written — the corpus grew and nothing pointed at them. A count that is not held to
/// the thing it counts is a count that rots, which is why `tests/readme.rs` holds the
/// README's the same way.
#[test]
fn kaniの件数はページと記録と実物で揃っている() {
    // `gen` の書いたハーネスを数える。kani そのものは要らない。
    let out = std::env::temp_dir().join("rulec-kani-count");
    let _ = std::fs::remove_dir_all(&out);
    let o = std::process::Command::new(env!("CARGO_BIN_EXE_rulec"))
        .current_dir(root())
        .args(["gen", "tests/corpus/", "--out", &out.to_string_lossy()])
        .output()
        .expect("rulec を起動できない");
    assert!(o.status.success(), "gen が失敗した");
    let mut real = 0usize;
    for e in std::fs::read_dir(out.join("rust")).unwrap().flatten() {
        if e.path().extension().is_some_and(|x| x == "rs") {
            real += std::fs::read_to_string(e.path()).unwrap().matches("#[kani::proof]").count();
        }
    }
    let _ = std::fs::remove_dir_all(&out);
    assert!(real > 0, "ハーネスが一本も出ていない");

    // 記録した実測と同じ本数か。ずれていたら、どちらかが古い。
    let report = read("experiments/kani/report.txt");
    let recorded: usize = report
        .lines()
        .filter_map(|l| l.split("Complete - ").nth(1))
        .filter_map(|r| r.split(' ').next())
        .filter_map(|n| n.parse::<usize>().ok())
        .sum();
    assert_eq!(
        recorded, real,
        "experiments/kani/report.txt は {recorded} 本ぶんだが、いま gen が書くのは {real} 本。\
         `sh experiments/kani/run.sh > experiments/kani/report.txt` で取り直してください"
    );

    // ページが言う本数も同じか。
    for (page, want) in [
        ("README.md", format!("{real} of them verify")),
        ("website/docs/generate.md", format!("{real} harnesses")),
        ("website/docs-ja/generate.md", format!("{real} 本が")),
        ("docs/generated-code.md", format!("corpus of {} rules", corpus_rules())),
    ] {
        assert!(read(page).contains(&want), "{page} に「{want}」がありません");
    }
}

/// The corpus's own size, for the pages that quote it.
fn corpus_rules() -> usize {
    std::fs::read_dir(root().join("tests/corpus"))
        .unwrap()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "rule"))
        .count()
}

/// The front page and the walkthrough quote the corpus's size too, and they are written by
/// hand in two languages — which is exactly where one copy gets updated and the other does
/// not. It happened: the Japanese pages said 45 while the English ones still said 43.
#[test]
fn トップと道案内が言う規則の本数は実物と合っている() {
    let n = corpus_rules();
    // The ones written rather than transcribed, as tests/readme.rs counts them. The English
    // page said nine for a while after the tenth went in.
    const WRITTEN: usize = 26;
    for (page, want) in [
        ("website/docs/index.md", format!("{n} rules checked, generated and run on every commit")),
        ("website/docs/assurance.md", format!("**{n} rules** — {} transcribed from a published source, {WRITTEN} written", n - WRITTEN)),
        ("website/docs-ja/assurance.md", format!("**規則 {n} 本**（{} 本は公開されている出典からの転記、{WRITTEN} 本は", n - WRITTEN)),
        ("website/docs-ja/index.md", format!("規則 **{n} 本**")),
        ("website/docs/generate.md", format!("On the corpus of {n} rules")),
    ] {
        assert!(read(page).contains(&want), "{page} に「{want}」がありません");
    }
}
