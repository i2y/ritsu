//! `sakai doc` (DESIGN 10; PLAN D.1): the pages of the example's two maps, the English one and
//! its Japanese twin, each in English and in Japanese as Markdown and in its own language as
//! HTML, are word for word the golden files in tests/golden/doc. The HTML reads nothing from
//! outside, and each box of its map links to the context's part of the page. Every Mermaid chart
//! draws, in Mermaid 11 and 12 in headless Chrome; and in Chrome, the page shows its map, and a
//! box of it goes to its context.
//!
//! Mermaid is in tools/mermaid (`npm ci --prefix tools/mermaid`); Chrome is found by ritsu-testkit
//! (`RITSU_CHROME` or `SAKAI_CHROME`, else Google Chrome where macOS keeps it, else
//! `google-chrome` or `chromium` on the PATH). A test that cannot find them says so and skips.
//! `SAKAI_BLESS=1` (or `RITSU_BLESS=1`) rewrites the golden files.

mod common;
use common::*;
use std::time::Duration;

/// The maps, and the stem of their golden files: the shop, and the services of the web shop that
/// talk by OpenAPI and AsyncAPI documents (DESIGN 15), each with English names and with Japanese.
const MAPS: [(&str, &str); 4] = [
    ("examples/shop/shop.ctx", "shop"),
    ("examples/shop.ja/通販.ctx", "通販"),
    ("examples/webshop/webshop.ctx", "webshop"),
    ("examples/webshop.ja/ネットショップ.ctx", "ネットショップ"),
];

/// How many contexts a map's page draws, and two of them a test presses.
fn boxes_of(stem: &str) -> (usize, [&'static str; 2]) {
    if matches!(stem, "shop" | "通販") { (5, ["billing", "inventory"]) } else { (4, ["payments", "shipping"]) }
}

/// Each golden page: the map, the arguments after it, and the golden file.
fn pages() -> Vec<(&'static str, Vec<&'static str>, String)> {
    let mut out = Vec::new();
    for (map, stem) in MAPS {
        out.push((map, vec!["--lang", "en"], format!("tests/golden/doc/{stem}.en.md")));
        out.push((map, vec!["--lang", "ja"], format!("tests/golden/doc/{stem}.ja.md")));
        let lang = if stem.is_ascii() { "en" } else { "ja" };
        out.push((map, vec!["--format", "html", "--lang", lang], format!("tests/golden/doc/{stem}.html")));
    }
    out
}

fn doc(map: &str, rest: &[&str]) -> String {
    let mut args = vec!["doc", map];
    args.extend_from_slice(rest);
    let (code, out, err) = joined(&args);
    assert_eq!(code, 0, "sakai doc {map} {}: {err}", rest.join(" "));
    out
}

#[test]
fn the_pages_match_the_golden_files() {
    std::fs::create_dir_all("tests/golden/doc").unwrap();
    let mut wrong = Vec::new();
    for (map, rest, file) in pages() {
        if let Some(e) = golden(&file, &doc(map, &rest)) {
            wrong.push(e);
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn the_html_reads_nothing_from_outside_and_its_boxes_link_to_the_contexts() {
    for (map, stem) in MAPS {
        let html = doc(map, &["--format", "html"]);
        assert!(ritsu_base::docpage::outside_urls(&html).is_empty(), "{stem}: the page reads {:?}", ritsu_base::docpage::outside_urls(&html));
        for bad in ["<script", "<link", "<img", "<iframe", "@import", "http://", "https://"] {
            assert!(!html.contains(bad), "{stem}: the page holds `{bad}`");
        }
        assert!(html.contains("@media (prefers-color-scheme: dark)") && html.contains("name=\"viewport\""), "{stem}: no dark colours or no viewport");
        let boxes: Vec<&str> = html.split("<a class=\"ctx\" href=\"#").skip(1).map(|r| r.split('"').next().unwrap()).collect();
        assert_eq!(boxes.len(), boxes_of(stem).0, "{stem}: the map has {} boxes", boxes.len());
        for id in boxes {
            assert!(html.contains(&format!("<h2 id=\"{id}\">")), "{stem}: the box #{id} goes nowhere");
        }
    }
}

#[test]
fn the_markdown_links_go_to_headings() {
    for (map, stem) in MAPS {
        for lang in ["en", "ja"] {
            let md = doc(map, &["--lang", lang]);
            let anchors: Vec<String> = md.lines().filter_map(|l| l.strip_prefix("## ")).map(sakai::doc::slug).collect();
            let mut rest = md.as_str();
            let mut links = 0;
            while let Some(i) = rest.find("](#") {
                let after = &rest[i + 3..];
                let id = &after[..after.find(')').unwrap()];
                assert!(anchors.iter().any(|a| a == id), "{stem} ({lang}): the link #{id} goes to no heading");
                links += 1;
                rest = after;
            }
            assert!(links > 20, "{stem} ({lang}): only {links} links");
        }
    }
}

#[test]
fn the_crate_binary_says_it_cannot_read_the_other_languages() {
    // The example holds rules, calendars and workflows: sakai's own binary refuses, as for check.
    let o = sakai(&["doc", "examples/shop/shop.ctx"]);
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).contains("error[E104]"), "{}", String::from_utf8_lossy(&o.stderr));
    // A map with nothing of another language is drawn by it as by ritsu.
    let o = sakai(&["doc", "tests/maps/basic/basic.ctx"]);
    assert_eq!(o.status.code(), Some(0), "{}", String::from_utf8_lossy(&o.stderr));
    let (_, joined_out, _) = joined(&["doc", "tests/maps/basic/basic.ctx"]);
    assert_eq!(String::from_utf8_lossy(&o.stdout), joined_out);
}

#[test]
fn out_writes_the_page_after_the_map() {
    let dir = TempDir::new("doc-out");
    let out = dir.path().to_string_lossy().to_string();
    let (code, said, err) = joined(&["doc", "examples/shop/shop.ctx", "--format", "html", "--out", &out]);
    assert_eq!(code, 0, "{err}");
    let file = dir.path().join("shop.html");
    assert_eq!(said, format!("{}: Written\n", file.display()));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), doc("examples/shop/shop.ctx", &["--format", "html"]));
}

// ── in a browser ──────────────────────────────────────────────────────────

fn chrome() -> Option<std::path::PathBuf> {
    if ritsu_testkit::golden::bless() {
        skip("the golden files are being rewritten; run this test again without SAKAI_BLESS");
        return None;
    }
    let c = ritsu_testkit::chrome::find();
    if c.is_none() {
        skip("Chrome is not found; set RITSU_CHROME (or SAKAI_CHROME) to its binary to run this test");
    }
    c
}

#[test]
fn every_mermaid_chart_draws() {
    if !ritsu_testkit::need(ritsu_testkit::Need::Mermaid) {
        return;
    }
    let Some(chrome) = chrome() else { return };
    let modules = std::fs::canonicalize("tools/mermaid").unwrap().join("node_modules");
    let scripts = match ritsu_testkit::mermaid::scripts(&modules) {
        Ok(s) => s,
        Err(why) => {
            skip(&why);
            return;
        }
    };
    let mut all: Vec<(String, String)> = Vec::new();
    for (_, _, file) in pages().into_iter().filter(|p| p.2.ends_with(".md")) {
        for c in ritsu_testkit::mermaid::charts(&std::fs::read_to_string(&file).unwrap()) {
            all.push((file.clone(), c));
        }
    }
    assert_eq!(all.len(), 8, "a chart on each of the eight Markdown pages");
    let sources: Vec<String> = all.iter().map(|(_, c)| c.clone()).collect();
    for (major, script) in scripts {
        let results = ritsu_testkit::mermaid::draw(&chrome, &script, &sources).unwrap_or_default();
        assert_eq!(results.len(), all.len(), "Mermaid {major} drew {} of {} charts", results.len(), all.len());
        let failed: Vec<String> = results.iter().zip(&all).filter(|(r, _)| **r != "ok").map(|(r, (f, _))| format!("{f}: {r}")).collect();
        assert!(failed.is_empty(), "Mermaid {major} could not draw:\n{}", failed.join("\n"));
        eprintln!("compared: Mermaid {major} drew all {} charts", all.len());
    }
}

#[test]
fn chrome_shows_the_map_and_a_box_goes_to_its_context() {
    if !ritsu_testkit::need(ritsu_testkit::Need::Chrome) {
        return;
    }
    let Some(chrome) = chrome() else { return };
    let work = TempDir::new("doc-chrome");
    for (_, stem) in MAPS {
        let page = std::fs::canonicalize(format!("tests/golden/doc/{stem}.html")).unwrap();
        // the picture: the page draws, as wide as the window
        let png = work.path().join(format!("{stem}.png"));
        ritsu_testkit::chrome::screenshot(&chrome, &format!("file://{}", page.display()), &png, 1280, 900, Duration::from_secs(60)).unwrap_or_else(|e| panic!("{stem}: {e}"));
        let size = ritsu_testkit::chrome::png_size(&std::fs::read(&png).unwrap());
        assert_eq!(size, Some((1280, 900)), "{stem}: the picture");
        // a box: pressed, the page goes to the context's part
        let html = std::fs::read_to_string(&page).unwrap();
        for alias in boxes_of(stem).1 {
            let press = format!(
                "<script>window.addEventListener('load', () => {{ const a = document.querySelector('svg a.ctx[data-alias=\"{alias}\"]'); a.dispatchEvent(new MouseEvent('click', {{bubbles: true, cancelable: true, view: window}})); setTimeout(() => {{ const t = document.getElementById('ctx-{alias}'); const o = document.createElement('pre'); o.id = 'pressed'; o.textContent = location.hash + ' ' + Math.round(t.getBoundingClientRect().top); document.body.appendChild(o); }}, 300); }});</script>\n</body>"
            );
            let at = work.write(&format!("{stem}-{alias}.html"), html.replace("</body>", &press));
            let dom = ritsu_testkit::chrome::dump_dom(&chrome, &format!("file://{}", at.display()), 5000, Duration::from_secs(60));
            let said = dom.split("<pre id=\"pressed\">").nth(1).and_then(|r| r.split("</pre>").next()).unwrap_or_else(|| panic!("{stem}: the page did not answer"));
            let (hash, top) = said.split_once(' ').unwrap();
            assert_eq!(hash, format!("#ctx-{alias}"), "{stem}: pressing the box of {alias}");
            let top: i64 = top.parse().unwrap();
            assert!((-2..=60).contains(&top), "{stem}: the part of {alias} is {top} px from the top after the press");
        }
        eprintln!("compared: {stem}.html drawn in Chrome, and two boxes go to their contexts");
    }
    ritsu_testkit::chrome::none_left(work.path()).unwrap();
}
