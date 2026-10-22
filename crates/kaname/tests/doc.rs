//! `kaname doc` (DESIGN 13): the pages of the materials that show what a page holds — a function
//! checked when it runs and a decision written in code (`tests/doc/decisions.kn`, in English and in
//! Japanese as Markdown and in English as HTML), a module that does not pass, with the inputs that
//! break it (`tests/modules/places.kn`), a function that may not stop (`tests/modules/rec.kn`), the
//! values of enums that have fields and the `match`es that split them, which the page counts no
//! decision of (`tests/modules/sums.kn`, in English and in Japanese), and an example drawn without the
//! verdicts of the checks across the borders, whose places wait for them (`examples/payout`) — are
//! the golden files in `tests/golden/doc/`, but for the pages of the
//! rules and the dates files the page holds, which are what rulec's and koyomi's own `doc` draw now,
//! word for word, and stand in the golden files as one line each. The pages of the examples as
//! `ritsu kaname doc` draws them, with the verdicts, are ritsu's (its `tests/kaname.rs`). The HTML
//! reads nothing from outside, and neither does any page it holds; in Chrome, it draws, and a page
//! it holds opens over it from its button and from `#page=<n>`.
//!
//! Chrome is found by ritsu-testkit (`RITSU_CHROME` or `KANAME_CHROME`, else Google Chrome where
//! macOS keeps it, else `google-chrome` or `chromium` on the PATH); a test that cannot find it says
//! so and skips. `KANAME_BLESS=1` (or `RITSU_BLESS=1`) writes the golden files again.

mod common;

use kaname::doc::{self, Embedded, Format, Page};
use kaname::suite::Suite;
use ritsu_base::text::Lang;
use std::path::Path;
use std::time::Duration;

/// Each page held to a golden file: the `.kn`, the language, the form, and the file's name.
const PAGES: [(&str, Lang, Format, &str); 10] = [
    ("tests/doc/decisions.kn", Lang::En, Format::Markdown, "decisions.en.md"),
    ("tests/doc/decisions.kn", Lang::Ja, Format::Markdown, "decisions.ja.md"),
    ("tests/doc/decisions.kn", Lang::En, Format::Html, "decisions.html"),
    ("tests/modules/places.kn", Lang::En, Format::Markdown, "places.en.md"),
    ("tests/modules/places.kn", Lang::Ja, Format::Markdown, "places.ja.md"),
    ("tests/modules/rec.kn", Lang::En, Format::Markdown, "rec.en.md"),
    ("examples/payout/payout.kn", Lang::En, Format::Markdown, "payout.waiting.en.md"),
    ("examples/payout/payout.ja.kn", Lang::Ja, Format::Markdown, "payout.waiting.ja.md"),
    ("tests/modules/sums.kn", Lang::En, Format::Markdown, "sums.en.md"),
    ("tests/modules/sums.ja.kn", Lang::Ja, Format::Markdown, "sums.ja.md"),
];

/// The page of a file, with every language joined and no verdicts: as the crate's tests join them.
fn page_of(path: &str, lang: Lang, format: Format) -> Page {
    let o = common::check(path);
    doc::page(&o, &common::joined(), Path::new("."), lang, format).unwrap_or_else(|| panic!("{path} has no page:\n{}", common::shown(&o, Lang::En)))
}

/// What `kaname doc` prints for a file: the exit code and the text.
fn run(args: &[&str], suite: Suite) -> (u8, String, String) {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = kaname::run::run(&args, suite, &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

/// The page the language draws for a file the page holds, as its own port draws it.
fn drawn(e: &Embedded, lang: Lang, format: Format) -> String {
    use ritsu_ports::{Dates, Rules};
    let file = Path::new(&e.file);
    let html = format == Format::Html;
    let p = match e.tool {
        "rulec" => rulec::ports::Engine::with_dates(std::sync::Arc::new(koyomi::ports::Engine)).doc(file, &e.file, html, lang),
        _ => koyomi::ports::Engine.doc(file, &e.file, html, lang),
    };
    p.unwrap_or_else(|s| panic!("{}: {s:?}", e.file))
}

/// The page as its golden file holds it: kaname's version as `<version>`, and each page of another
/// language as one line that names it.
fn as_golden(mut p: Page, format: Format) -> String {
    for e in &mut p.embedded {
        e.page = Ok(format!("[the page {} doc draws of {}]", e.tool, e.file));
    }
    doc::write(&p, format).replace(env!("CARGO_PKG_VERSION"), "<version>")
}

#[test]
fn the_pages_match_the_golden_files() {
    let mut wrong = Vec::new();
    for (path, lang, format, name) in PAGES {
        let p = page_of(path, lang, format);
        if let Err(e) = ritsu_testkit::golden::check(Path::new(&format!("tests/golden/doc/{name}")), &as_golden(p, format)) {
            wrong.push(e);
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The pages of the rules and the dates files a page holds are what rulec's and koyomi's `doc` draw
/// for them now, in the page's language and form; each file once.
#[test]
fn the_pages_held_are_what_rulec_and_koyomi_draw() {
    let mut looked = 0;
    for (path, lang, format, _) in PAGES {
        let p = page_of(path, lang, format);
        let files: Vec<&str> = p.embedded.iter().map(|e| e.file.as_str()).collect();
        let mut once = files.clone();
        once.dedup();
        assert_eq!(files, once, "{path}: a file held twice");
        for e in &p.embedded {
            let page = e.page.as_ref().unwrap_or_else(|why| panic!("{path}: {} could not draw {}: {why}", e.tool, e.file));
            assert_eq!(*page, drawn(e, lang, format), "{path}: the page of {} is not what {} doc draws", e.file, e.tool);
            looked += 1;
        }
    }
    // the rule of the materials, in each of its three pages; the card's rule of the payout, in each
    // of its two
    assert_eq!(looked, 3 + 2, "pages held");
}

/// The HTML is one page that reads nothing from outside: no stylesheet, script or picture of
/// another server, and no page it holds reads one either. Its colours follow the reader's light or
/// dark, and it fits a narrow screen.
#[test]
fn the_html_reads_nothing_from_outside() {
    for (path, lang, format, name) in PAGES {
        if format != Format::Html {
            continue;
        }
        let p = page_of(path, lang, format);
        for e in &p.embedded {
            let page = e.page.as_ref().unwrap();
            assert!(ritsu_base::docpage::outside_urls(page).is_empty(), "{name}: the page of {} reads {:?}", e.file, ritsu_base::docpage::outside_urls(page));
        }
        let html = doc::write(&p, format);
        assert!(ritsu_base::docpage::outside_urls(&html).is_empty(), "{name}: the page reads {:?}", ritsu_base::docpage::outside_urls(&html));
        for bad in ["<link", "<img", "<script src", "@import"] {
            assert!(!html.contains(bad), "{name}: the page holds `{bad}`");
        }
        assert!(html.contains("@media (prefers-color-scheme: dark)") && html.contains("name=\"viewport\""), "{name}: no dark colours or no viewport");
        assert!(html.starts_with("<!doctype html>\n") && html.contains(&format!("<html lang=\"{}\">", lang.code())), "{name}");
        // the pages it holds are text in its data, which nothing ends early
        let data = html.split("<script type=\"application/json\" id=\"kn-pages\">").nth(1).and_then(|r| r.split("</script>").next()).expect("the data of the pages");
        assert!(!data.contains('<'), "{name}: a `<` in the data of the pages");
        let v = ritsu_base::json::parse(data).unwrap();
        let held: Vec<&str> = v.as_arr().unwrap().iter().map(|x| x.get("page").and_then(|x| x.as_str()).unwrap()).collect();
        assert_eq!(held, p.embedded.iter().map(|e| e.page.as_deref().unwrap()).collect::<Vec<_>>(), "{name}");
    }
}

/// What a `.kn` writes stays text where the page puts it (ritsu's DESIGN 9.2): a description that
/// holds `</script>` is text in the HTML and in the Markdown, where a `<` that could begin HTML is
/// written `&lt;` outside the code spans; a carriage return starts no line of the Markdown.
#[test]
fn text_from_the_module_stays_where_the_page_puts_it() {
    let t = ritsu_testkit::TempDir::new("kaname-doc-text");
    let f = t.write("m.kn", "module m v1\ndescription \"Adds </script><script>alert(1)</script>\r# Injected; `<b>` and a < 3\"\n\nfn f(n: number range >=0 <=9) -> number\n  n + 1\n");
    let o = common::check(f.to_str().unwrap());
    assert!(!o.has_errors(), "{}", common::shown(&o, Lang::En));
    for lang in [Lang::En, Lang::Ja] {
        let md = doc::write(&doc::page(&o, &common::joined(), t.path(), lang, Format::Markdown).unwrap(), Format::Markdown);
        assert!(!md.contains('\r'), "{md:?}");
        assert!(md.contains("Adds &lt;/script>&lt;script>alert(1)&lt;/script> # Injected; `<b>` and a < 3"), "{md}");
        assert!(!md.contains("<script>") && !md.contains("</script>"), "{md}");
        let html = doc::write(&doc::page(&o, &common::joined(), t.path(), lang, Format::Html).unwrap(), Format::Html);
        assert!(!html.contains("<script>alert") && html.contains("Adds &lt;/script&gt;&lt;script&gt;alert(1)&lt;/script&gt;"), "{html}");
    }
}

/// A file whose names or types do not pass has no page: what check says, and exit 1. One that reads
/// a rule, a dates file or a book cannot be read by the binary of kaname's own crate (E202, exit 2);
/// one that reads none of them is drawn by it as by `ritsu kaname`.
#[test]
fn the_page_is_of_a_module_whose_names_and_types_pass() {
    let t = ritsu_testkit::TempDir::new("kaname-doc-run");
    let bad = t.write("bad.kn", "module bad v1\n\nfn f(n: number range >=0 <=9) -> number\n  m + 1\n");
    let (code, out, err) = run(&["doc", bad.to_str().unwrap()], common::joined());
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.contains("error[E101]: ") && err.contains("do not pass check, so it has no page"), "{out}{err}");
    // a module the analysis finds wrong with still has its page: it shows where
    let (code, out, _) = run(&["doc", "tests/modules/places.kn", "--root", "."], common::joined());
    assert_eq!(code, 0);
    assert!(out.contains("The module does not pass `kaname check` (4 errors)"), "{out}");
    let (code, out, _) = run(&["doc", "examples/payout/payout.kn"], Suite::default());
    assert_eq!(code, 2);
    assert!(out.contains("error[E202]") && out.contains("ritsu kaname doc examples/payout/payout.kn"), "{out}");
    let alone = run(&["doc", "tests/modules/rec.kn", "--root", ".", "--format", "html"], Suite::default());
    let joined = run(&["doc", "tests/modules/rec.kn", "--root", ".", "--format", "html"], common::joined());
    assert_eq!(alone.0, 0, "{}", alone.2);
    assert_eq!(alone, joined);
    // one file a page
    let (code, _, err) = run(&["doc", "tests/modules/rec.kn", "tests/modules/places.kn"], common::joined());
    assert_eq!(code, 2);
    assert!(err.starts_with("error: "), "{err}");
}

// ── in a browser ──────────────────────────────────────────────────────────

fn chrome() -> Option<std::path::PathBuf> {
    if !ritsu_testkit::need(ritsu_testkit::Need::Chrome) {
        return None;
    }
    let c = ritsu_testkit::chrome::find();
    if c.is_none() {
        ritsu_testkit::skip("Chrome is not found; set RITSU_CHROME (or KANAME_CHROME) to its binary to run this test");
    }
    c
}

/// In Chrome, each HTML page draws as wide as the window; each page it holds opens over it, from
/// `#page=<n>` and from its button, in a frame of its own, and is the page rulec or koyomi drew.
#[test]
fn chrome_draws_the_page_and_opens_the_pages_it_holds() {
    let Some(chrome) = chrome() else { return };
    let work = ritsu_testkit::TempDir::new("kaname-doc-chrome");
    for (path, lang, format, name) in PAGES {
        if format != Format::Html {
            continue;
        }
        let p = page_of(path, lang, format);
        let html = doc::write(&p, format);
        let at = work.write(name, &html);
        let png = work.path().join(format!("{name}.png"));
        ritsu_testkit::chrome::screenshot(&chrome, &format!("file://{}", at.display()), &png, 1280, 900, Duration::from_secs(60)).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(ritsu_testkit::chrome::png_size(&std::fs::read(&png).unwrap()), Some((1280, 900)), "{name}: the picture");
        for (k, e) in p.embedded.iter().enumerate() {
            let title = e.page.as_ref().unwrap().split("<title>").nth(1).and_then(|x| x.split("</title>").next()).unwrap_or_else(|| panic!("{name}: the page of {} has no title", e.file));
            // an attribute's `<` is written as it is or as `&lt;`, by the version of Chrome
            let shown = [format!("<title>{title}</title>"), format!("&lt;title&gt;{title}&lt;/title&gt;")];
            let sheet_of = |dom: &str| -> String { dom.split("id=\"kn-sheet\"").nth(1).and_then(|x| x.split("</iframe>").next()).unwrap_or_else(|| panic!("{name}: the page has no sheet")).to_string() };
            // from the address
            let dom = ritsu_testkit::chrome::dump_dom(&chrome, &format!("file://{}#page={}", at.display(), k + 1), 5000, Duration::from_secs(60));
            let sheet = sheet_of(&dom);
            assert!(!sheet.split('>').next().unwrap().contains("hidden"), "{name}: #page={} does not open the page of {}", k + 1, e.file);
            assert!(shown.iter().any(|t| sheet.contains(t.as_str())), "{name}: #page={} opens another page than the one {} doc draws of {}", k + 1, e.tool, e.file);
            // from its button
            let press = format!(
                "<script>window.addEventListener('load', () => {{ document.querySelector('button[data-page=\"{k}\"]').click(); const o = document.createElement('pre'); o.id = 'pressed'; o.textContent = document.getElementById('kn-sheet').hidden + ' ' + document.querySelector('#kn-sheet .what').textContent; document.body.appendChild(o); }});</script>\n</body>"
            );
            let pressed = work.write(&format!("{name}-{k}.html"), html.replace("</body>", &press));
            let dom = ritsu_testkit::chrome::dump_dom(&chrome, &format!("file://{}", pressed.display()), 5000, Duration::from_secs(60));
            let said = dom.split("<pre id=\"pressed\">").nth(1).and_then(|r| r.split("</pre>").next()).unwrap_or_else(|| panic!("{name}: the page did not answer"));
            assert_eq!(said, format!("false {} · {} doc", e.file, e.tool), "{name}: pressing the button of {}", e.file);
            let sheet = sheet_of(&dom);
            assert!(shown.iter().any(|t| sheet.contains(t.as_str())), "{name}: the button of {} opens another page", e.file);
        }
        eprintln!("compared: {name} drawn in Chrome, and the {} pages it holds open over it", p.embedded.len());
    }
    ritsu_testkit::chrome::none_left(work.path()).unwrap();
}
