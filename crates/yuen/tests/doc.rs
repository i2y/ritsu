//! `yuen doc` (DESIGN 10, PLAN D.1): the page of every example, in Markdown and in HTML, in
//! English and in Japanese, against golden files (`YUEN_BLESS=1` or `RITSU_BLESS=1` writes
//! them); the HTML reads nothing from outside; every line quoted from a law or an OpenSpec spec is
//! a line of its pinned copy or of the spec; the page is not made while the sources or the
//! artifacts have errors (`openspec_greeter_archived` is such an example); and, with Chrome, the
//! pictures the READMEs show.

mod common;

use ritsu_testkit::{Need, TempDir, chrome, need};
use std::path::Path;
use std::time::Duration;

/// The example whose pin stops the check at its third stage, so `doc` makes no page of it.
const NO_PAGE: &str = "openspec_greeter_archived";

fn page(dir: &str, req: &str, lang: &str, html: bool) -> common::Ran {
    let path = format!("{dir}/{req}");
    let mut args = vec!["doc", path.as_str(), "--root", dir, "--lang", lang];
    if html {
        args.extend(["--format", "html"]);
    }
    common::run(&args)
}

#[test]
fn the_page_of_every_example() {
    let mut failures = Vec::new();
    let mut n = 0;
    for (ex, reqs, _) in common::EXAMPLES {
        let dir = format!("examples/{ex}");
        if *ex == NO_PAGE {
            for req in *reqs {
                let r = page(&dir, req, "en", false);
                if r.code != 1 || !r.stderr.contains("[E103]") || !r.stdout.is_empty() {
                    failures.push(format!("{dir}/{req}: a page was made, or not for E103: exit {}\n{}", r.code, r.stderr));
                }
            }
            continue;
        }
        for req in *reqs {
            let stem = if *ex == "openspec_greeter" { format!("openspec_{}", req.trim_end_matches(".req")) } else { req.trim_end_matches(".req").to_string() };
            for lang in ["en", "ja"] {
                for html in [false, true] {
                    let r = page(&dir, req, lang, html);
                    if r.code != 0 || !r.stderr.is_empty() {
                        failures.push(format!("{dir}/{req} --lang {lang} html={html}: exit {}\n{}", r.code, r.stderr));
                        continue;
                    }
                    let ext = if html { "html" } else { "md" };
                    common::golden(&format!("tests/golden/doc/{stem}/{lang}.{ext}"), &r.stdout, &mut failures);
                    n += 1;
                }
            }
        }
    }
    assert_eq!(n, 56, "pages");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The HTML reads no script, stylesheet, font or image from anywhere; it has both palettes and
/// lets a narrow screen move a table inside its frame.
#[test]
fn the_html_reads_nothing_from_outside() {
    for (ex, reqs, _) in common::EXAMPLES {
        let dir = format!("examples/{ex}");
        if *ex == NO_PAGE {
            continue;
        }
        for req in *reqs {
            let r = page(&dir, req, "en", true);
            let html = r.stdout;
            assert_eq!(ritsu_base::docpage::outside_urls(&html), Vec::<String>::new(), "{dir}/{req}");
            assert!(!html.contains("<script"), "{dir}/{req} has a script");
            assert!(!html.contains("<link"), "{dir}/{req} links a file");
            assert!(html.contains("prefers-color-scheme: dark") && html.contains("data-theme=\"dark\""), "{dir}/{req}: both palettes");
            assert!(html.contains(".scroll { overflow-x: auto;"), "{dir}/{req}: tables scroll in their frame");
            assert!(html.contains("<meta name=\"viewport\""), "{dir}/{req}");
        }
    }
}

/// Every line a page quotes from a law is a line of the copy it pins: an e-Gov article as
/// `article_lines` gives it, a table or an eCFR section a line of its text (ritsu-base's
/// `quote_lines`, which `yuen trace` quotes by too).
#[test]
fn every_quoted_line_is_a_line_of_the_pinned_copy() {
    let mut quoted = 0;
    for (ex, reqs, _) in common::EXAMPLES {
        let dir = format!("examples/{ex}");
        let mut lines = std::collections::BTreeSet::new();
        let law = Path::new(&dir).join("sources/law");
        if law.is_dir() {
            for d in std::fs::read_dir(&law).unwrap() {
                for f in std::fs::read_dir(d.unwrap().path()).unwrap() {
                    let f = f.unwrap().path();
                    if f.extension().is_some_and(|e| e == "xml") {
                        let db = if f.to_string_lossy().contains("-CFR-") { ritsu_base::sources::LawDb::Ecfr } else { ritsu_base::sources::LawDb::Egov };
                        let name = f.file_name().unwrap().to_string_lossy().to_string();
                        lines.extend(yuen::copies::quote_lines(db, &name, &std::fs::read_to_string(&f).unwrap()));
                    }
                }
            }
        }
        // the blocks an OpenSpec spec holds, a line each (the page leaves the blank lines out)
        for spec in [Path::new(&dir).join("openspec/specs"), Path::new(&dir).join("ja/openspec/specs")] {
            let Ok(rd) = std::fs::read_dir(&spec) else { continue };
            for cap in rd {
                if let Ok(text) = std::fs::read_to_string(cap.unwrap().path().join("spec.md")) {
                    lines.extend(text.lines().map(str::to_string));
                }
            }
        }
        if *ex == NO_PAGE {
            continue;
        }
        for req in *reqs {
            for lang in ["en", "ja"] {
                let md = page(&dir, req, lang, false).stdout;
                for l in md.lines() {
                    let Some(q) = l.strip_prefix("> ") else { continue };
                    if q.starts_with("**") {
                        continue;
                    }
                    assert!(lines.contains(q), "{dir}/{req}: {q:?} is not a line of a pinned copy");
                    quoted += 1;
                }
            }
        }
    }
    assert!(quoted > 20, "{quoted} quoted lines");
}

/// The page shows a mark (the reread example, with its diff) and exits 0; a copy whose text
/// no longer matches its pin stops the page (stage 3), with the check on standard error.
#[test]
fn a_mark_is_on_the_page_and_a_broken_copy_stops_it() {
    let r = page("examples/civil_code_periods_reread", "civil_code_periods_reread.ja.req", "en", false);
    assert_eq!(r.code, 0);
    assert!(r.stdout.contains("error[E303]") && r.stdout.contains("+ roll following"), "{}", r.stdout);
    assert!(r.stdout.contains("**marked (1)**"), "{}", r.stdout);

    let t = TempDir::new("doc-broken");
    let dir = t.path().join("osha");
    common::copy_dir(Path::new("examples/osha"), &dir);
    common::edit(&dir, "sources/law/29-CFR-1910@2026-01-01/1910.157.xml", "75 feet", "80 feet");
    let d = dir.to_str().unwrap();
    let req = format!("{d}/osha.req");
    let r = common::run(&["doc", &req, "--root", d]);
    assert_eq!(r.code, 1, "{}{}", r.stdout, r.stderr);
    assert!(r.stdout.is_empty(), "no page: {}", r.stdout);
    assert!(r.stderr.contains("error[E1"), "{}", r.stderr);
}

/// `--out <dir>` writes `<name>.md` or `<name>.html` after the first `.req`.
#[test]
fn out_writes_the_page_after_the_first_req() {
    let t = TempDir::new("doc-out");
    let out = t.path().join("site");
    let o = out.to_str().unwrap();
    for (fmt, file) in [("markdown", "osha.md"), ("html", "osha.html")] {
        let r = common::run(&["doc", "examples/osha/osha.req", "--root", "examples/osha", "--format", fmt, "--out", o]);
        assert_eq!(r.code, 0, "{}", r.stderr);
        let written = out.join(file);
        assert!(written.is_file(), "{file}");
        assert_eq!(r.stdout, format!("wrote: {}\n", written.display()));
        let page = page("examples/osha", "osha.req", "en", fmt == "html").stdout;
        assert_eq!(std::fs::read_to_string(&written).unwrap(), page);
    }
}

struct Shot {
    lang: &'static str,
    name: &'static str,
    theme: &'static str,
    size: (u32, u32),
    /// What to scroll to: an anchor of the page, or the top.
    at: &'static str,
}

const SHOTS: &[Shot] = &[
    Shot { lang: "en", name: "doc-top.en.png", theme: "light", size: (1100, 1250), at: "" },
    Shot { lang: "en", name: "doc-why.en.png", theme: "dark", size: (1100, 1250), at: "why-last_day_142" },
    Shot { lang: "ja", name: "doc-top.ja.png", theme: "light", size: (1100, 1250), at: "" },
    Shot { lang: "ja", name: "doc-why.ja.png", theme: "dark", size: (1100, 1250), at: "why-last_day_142" },
];

/// The pictures the READMEs show: the top of the reread example's page (light), and the section
/// of the requirement whose link is marked (dark).
#[test]
fn the_pages_in_chrome() {
    if !need(Need::Chrome) {
        return;
    }
    let Some(chrome) = chrome::find() else {
        ritsu_testkit::skip("Chrome is not found; set RITSU_CHROME (or YUEN_CHROME) to draw the pages");
        return;
    };
    let bless = ritsu_testkit::golden::bless();
    let dir = TempDir::new("doc-chrome");
    for (i, Shot { lang, name, theme, size: (w, h), at }) in SHOTS.iter().enumerate() {
        let html = page("examples/civil_code_periods_reread", "civil_code_periods_reread.ja.req", lang, true).stdout;
        let html = html.replacen(&format!("<html lang=\"{lang}\">"), &format!("<html lang=\"{lang}\" data-theme=\"{theme}\">"), 1);
        // a section is shown by hiding what comes before its heading
        let html = if at.is_empty() {
            html
        } else {
            let i = html.find(&format!("<h3 id=\"{at}\"")).expect("the section's heading");
            let start = html.find("<main>\n").unwrap() + "<main>\n".len();
            format!("{}{}", &html[..start], &html[i..])
        };
        let file = dir.path().join(format!("{i}.html"));
        std::fs::write(&file, html).unwrap();
        let png = dir.path().join(name);
        let drawn = chrome::screenshot(&chrome, &format!("file://{}", file.display()), &png, *w, *h, Duration::from_secs(60));
        let bytes = std::fs::read(&png).unwrap_or_else(|_| panic!("Chrome drew no picture: {drawn:?}"));
        assert_eq!(chrome::png_size(&bytes), Some((*w, *h)), "{name}");
        assert!(bytes.len() > 30_000, "{name} is {} bytes: did the page draw?", bytes.len());
        if bless {
            std::fs::create_dir_all("docs/images").unwrap();
            std::fs::write(format!("docs/images/{name}"), &bytes).unwrap();
        }
        println!("drawn: {name} ({} bytes)", bytes.len());
    }
    for Shot { name, .. } in SHOTS {
        assert!(Path::new(&format!("docs/images/{name}")).is_file(), "docs/images/{name} is missing; YUEN_BLESS=1 cargo test --test doc draws it");
    }
}
