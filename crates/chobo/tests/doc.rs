//! `chobo doc` draws what the checker and the reference interpreter know. Its Markdown for the
//! examples is the page beside each of them (`doc.md`, `doc.ja.md`), which GitHub shows; for the
//! books of tests/books and a book with warnings, it is word for word the golden files in
//! tests/doc, as is the HTML of every example and test book. The HTML fetches nothing. Every step
//! a page shows is what the reference interpreter left after it. Every Mermaid chart draws, in
//! Mermaid 11 and 12 in headless Chrome; and in Chrome, a page shows the balances its data says,
//! and lights up the transfers each step called.
//!
//! Mermaid is in tools/mermaid (`npm ci --prefix tools/mermaid`); Chrome is found by ritsu-testkit
//! (`RITSU_CHROME` or `CHOBO_CHROME`, else Google Chrome where macOS keeps it, else `google-chrome`
//! or `chromium` on the PATH). A test that cannot find them says so and skips; read the skip lines.
//! `CHOBO_BLESS=1` (or `RITSU_BLESS=1`) rewrites the golden files and the examples' pages.

mod common;
use ritsu_base::text::Lang;
use chobo::scenario;
use common::*;
use serde_json::Value;
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

fn rel(p: &Path) -> String {
    p.strip_prefix(root()).unwrap_or(p).display().to_string()
}

/// One page `doc` is held to: the book, the language, where its Markdown is kept, and where its
/// HTML is, when it is kept too.
struct Page {
    book: PathBuf,
    lang: Lang,
    md: PathBuf,
    html: Option<PathBuf>,
}

/// The example directories, each with one book in English and its Japanese twin.
fn examples() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root().join("examples")).unwrap().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    dirs.sort();
    dirs
}

fn pages() -> Vec<Page> {
    let mut out = Vec::new();
    for dir in examples() {
        let books = books_in(&rel(&dir));
        let ja: Vec<&PathBuf> = books.iter().filter(|b| b.to_string_lossy().ends_with(".ja.book")).collect();
        assert_eq!((books.len(), ja.len()), (2, 1), "{}: an example is one book in English and its Japanese twin", rel(&dir));
        for b in &books {
            let twin = b.to_string_lossy().ends_with(".ja.book");
            let stem = stem(b);
            out.push(Page {
                book: b.clone(),
                lang: if twin { Lang::Ja } else { Lang::En },
                md: dir.join(if twin { "doc.ja.md" } else { "doc.md" }),
                html: Some(root().join(format!("tests/doc/{stem}.html"))),
            });
        }
    }
    for b in books_in("tests/books") {
        let stem = stem(&b);
        out.push(Page { book: b.clone(), lang: Lang::En, md: root().join(format!("tests/doc/{stem}.en.md")), html: None });
        out.push(Page { book: b.clone(), lang: Lang::Ja, md: root().join(format!("tests/doc/{stem}.ja.md")), html: Some(root().join(format!("tests/doc/{stem}.html"))) });
    }
    // a book the check warns about: its page says so
    let warned = root().join("tests/fixtures/順序.book");
    out.push(Page { book: warned.clone(), lang: Lang::En, md: root().join("tests/doc/順序.en.md"), html: None });
    out.push(Page { book: warned, lang: Lang::Ja, md: root().join("tests/doc/順序.ja.md"), html: None });
    // and the same book in English
    let warned = root().join("tests/fixtures/order.book");
    out.push(Page { book: warned.clone(), lang: Lang::En, md: root().join("tests/doc/order.en.md"), html: None });
    out.push(Page { book: warned, lang: Lang::Ja, md: root().join("tests/doc/order.ja.md"), html: None });
    out
}

/// What `doc` writes for a book: the Markdown, or the HTML page.
fn written(book: &Path, lang: Lang, html: bool) -> String {
    let src = std::fs::read_to_string(book).unwrap();
    let c = chobo::check::check_source(&src);
    let b = c.book.as_ref().unwrap_or_else(|| panic!("{} does not load", rel(book)));
    let rep = c.report.as_ref().unwrap_or_else(|| panic!("{} has errors", rel(book)));
    let file = rel(book);
    let input = chobo::doc::Input { book: b, file: &file, src: &src, diags: &c.diags, report: rep, lang };
    if html { chobo::doc::html(&input) } else { chobo::doc::markdown(&input) }
}

#[test]
fn markdown_matches_the_golden_files() {
    std::fs::create_dir_all(root().join("tests/doc")).unwrap();
    for p in pages() {
        golden(&p.md, &written(&p.book, p.lang, false));
    }
}

#[test]
fn html_matches_the_golden_files() {
    std::fs::create_dir_all(root().join("tests/doc")).unwrap();
    for p in pages() {
        if let Some(h) = &p.html {
            golden(h, &written(&p.book, p.lang, true));
        }
    }
}

#[test]
fn every_example_and_test_book_is_drawn_without_warnings_but_the_one_that_has_them() {
    let pages = pages();
    for p in &pages {
        let c = chobo::check::check_source(&std::fs::read_to_string(&p.book).unwrap());
        let warned = rel(&p.book).starts_with("tests/fixtures/");
        assert_eq!(!c.diags.is_empty(), warned, "{}: {} diagnostic(s)", rel(&p.book), c.diags.len());
    }
    // the page of the book with warnings shows them, as `chobo check` prints them
    let text = written(&root().join("tests/fixtures/順序.book"), Lang::En, false);
    assert!(text.contains("warning[W103]: tests/fixtures/順序.book:15:3:"), "the warnings are not on the page");
    let text = written(&root().join("tests/fixtures/order.book"), Lang::En, false);
    assert!(text.contains("warning[W103]: tests/fixtures/order.book:15:3:"), "the warnings are not on the English page");
}

#[test]
fn the_html_fetches_nothing() {
    for p in pages() {
        let Some(h) = &p.html else { continue };
        let page = written(&p.book, p.lang, true);
        for bad in ["http://", "https://", "<link", "<img", "<iframe", "src=", "@import", "url(http", "data:"] {
            assert!(!page.contains(bad), "{}: the page holds `{bad}`; it must fetch nothing", rel(h));
        }
        assert!(page.contains("@media (prefers-color-scheme: dark)"), "{}: no dark colours", rel(h));
        assert!(page.contains("name=\"viewport\""), "{}: no viewport", rel(h));
    }
}

/// The data a page steps through.
fn data(page: &str) -> Value {
    let json = page.split("<script type=\"application/json\" id=\"chobo-data\">").nth(1).unwrap().split("</script>").next().unwrap();
    serde_json::from_str(&json.replace("<\\/", "</")).unwrap()
}

/// Every step a page shows comes out as the reference interpreter has it: each way a scenario can
/// come out is one `chobo run` finds, and the balances after the last step are the run's.
#[test]
fn every_step_shows_what_the_interpreter_left() {
    let mut steps = 0;
    for p in pages().into_iter().filter(|p| p.html.is_some()) {
        let src = std::fs::read_to_string(&p.book).unwrap();
        let (book, _) = chobo::model::load(&src);
        let book = book.unwrap();
        let d = data(&written(&p.book, p.lang, true));
        let shown = d["scenarios"].as_array().unwrap();
        let made = chobo::scenarios::generate(&book);
        assert_eq!(shown.len(), made.len(), "{}: the page shows {} scenarios of {}", rel(&p.book), shown.len(), made.len());
        for (s, v) in made.iter().zip(shown) {
            let traces = scenario::trace(&book, s).unwrap();
            let want = scenario::run_json(&book, s).unwrap();
            let got: Vec<Value> = traces.iter().map(|t| scenario::result_json(&book, s, &t.run())).collect();
            match want.get("outcomes") {
                Some(Value::Array(outs)) => assert_eq!(&got, outs, "{}: {}", rel(&p.book), s.name),
                _ => assert_eq!(got, vec![want.clone()], "{}: {}", rel(&p.book), s.name),
            }
            let outcomes = v["outcomes"].as_array().unwrap();
            assert_eq!(outcomes.len(), traces.len(), "{}: {}", rel(&p.book), s.name);
            let accounts = scenario::named_accounts(&book, s);
            for (o, t) in outcomes.iter().zip(&traces) {
                let o = o.as_array().unwrap();
                assert_eq!(o.len(), s.steps.len(), "{}: {}", rel(&p.book), s.name);
                for (j, step) in o.iter().enumerate() {
                    let state = &t.states[j];
                    for (a, id) in accounts.iter().enumerate() {
                        let b = state.balance(id);
                        let scale = book.unit_of(id.kind).scale;
                        let want: Vec<String> = [b.posted, b.held_out, b.held_in].iter().map(|v| chobo::model::format_amount(*v, scale)).collect();
                        let got: Vec<String> = step["balances"][a].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect();
                        assert_eq!(got, want, "{}: {}: step {}: {}", rel(&p.book), s.name, j + 1, id.text(&book));
                    }
                    steps += 1;
                }
            }
        }
    }
    assert!(steps > 1000, "only {steps} steps were looked at");
    eprintln!("compared: {steps} steps shown with the balances the reference interpreter left");
}

// ── in a browser ──────────────────────────────────────────────────────────

/// The DOM of a page after its scripts ran, as headless Chrome dumps it (ritsu-testkit stops Chrome
/// as soon as the page is out: it can take a long while to quit by itself).
fn dump_dom(chrome: &Path, url: &str) -> String {
    ritsu_testkit::chrome::dump_dom(chrome, url, 20000, Duration::from_secs(90))
}

#[test]
fn every_mermaid_chart_draws() {
    if !need(Need::Mermaid) {
        return;
    }
    if bless() {
        skip("the golden files are being rewritten; run this test again without CHOBO_BLESS");
        return;
    }
    let Some(chrome) = ritsu_testkit::chrome::find() else {
        skip("Chrome is not found; set RITSU_CHROME (or CHOBO_CHROME) to its binary to run this test");
        return;
    };
    let scripts = match ritsu_testkit::mermaid::scripts(&root().join("tools/mermaid/node_modules")) {
        Ok(s) => s,
        Err(why) => {
            skip(&why);
            return;
        }
    };
    let mut all: Vec<(String, String)> = Vec::new();
    for p in pages() {
        for (k, c) in ritsu_testkit::mermaid::charts(&std::fs::read_to_string(&p.md).unwrap()).into_iter().enumerate() {
            all.push((format!("{} #{}", rel(&p.md), k + 1), c));
        }
    }
    assert!(all.len() > 30, "only {} charts", all.len());
    let sources: Vec<String> = all.iter().map(|(_, c)| c.clone()).collect();
    for (major, script) in scripts {
        let results = ritsu_testkit::mermaid::draw(&chrome, &script, &sources).unwrap_or_default();
        assert_eq!(results.len(), all.len(), "Mermaid {major} drew {} of {} charts", results.len(), all.len());
        let failed: Vec<String> = results.iter().zip(&all).filter(|(r, _)| **r != "ok").map(|(r, (name, _))| format!("{name}: {r}")).collect();
        assert!(failed.is_empty(), "Mermaid {major} could not draw:\n{}", failed.join("\n"));
        eprintln!("compared: Mermaid {major} drew all {} charts", all.len());
    }
}

/// The cells of the balances a dumped page shows, row by row.
fn balance_cells(dom: &str) -> Vec<Vec<String>> {
    let body = dom.split("<tbody id=\"bal-body\">").nth(1).and_then(|r| r.split("</tbody>").next()).unwrap_or("");
    body.split("<tr>")
        .skip(1)
        .map(|row| row.split("<td").skip(1).map(|c| c.split_once('>').map(|x| x.1).unwrap_or("").split("</td>").next().unwrap_or("").to_string()).collect())
        .collect()
}

/// The transfer kinds a dumped page lights up on its small chart, and those it shows refused.
fn lit(dom: &str) -> (BTreeSet<String>, BTreeSet<String>) {
    let mini = dom.split("id=\"mini\"").nth(1).unwrap_or("");
    let (mut ok, mut no) = (BTreeSet::new(), BTreeSet::new());
    for piece in mini.split("<g class=\"").skip(1) {
        let class = piece.split('"').next().unwrap_or("");
        let k = piece.split("data-k=\"").nth(1).and_then(|r| r.split('"').next()).unwrap_or("").to_string();
        if class.split(' ').any(|c| c == "lit") {
            ok.insert(k.clone());
        }
        if class.split(' ').any(|c| c == "no") {
            no.insert(k);
        }
    }
    (ok, no)
}

#[test]
fn a_page_shows_what_its_data_says() {
    if !need(Need::Chrome) {
        return;
    }
    if bless() {
        skip("the golden files are being rewritten; run this test again without CHOBO_BLESS");
        return;
    }
    let Some(chrome) = ritsu_testkit::chrome::find() else {
        skip("Chrome is not found; set RITSU_CHROME (or CHOBO_CHROME) to its binary to run this test");
        return;
    };
    let mut looked = 0;
    let started = Instant::now();
    for stem in ["marketplace", "refunds.ja"] {
        let page = root().join(format!("tests/doc/{stem}.html"));
        let text = std::fs::read_to_string(&page).expect("the golden page");
        let d = data(&text);
        let scenarios = d["scenarios"].as_array().unwrap();
        // the first, a middle one, and the last: a `together` one, which comes out two ways
        for k in [0, scenarios.len() / 2, scenarios.len() - 1] {
            let s = &scenarios[k];
            let outs = s["outcomes"].as_array().unwrap();
            let o = outs.len() - 1;
            let steps = outs[o].as_array().unwrap();
            for step in [0, steps.len()] {
                let url = format!("file://{}#scenario={}&step={step}&outcome={}", page.display(), k + 1, o + 1);
                let dom = dump_dom(&chrome, &url);
                let want: Vec<Vec<String>> = s["accounts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .enumerate()
                    .map(|(a, name)| {
                        let b = if step == 0 { &s["zero"][a] } else { &steps[step - 1]["balances"][a] };
                        let mut row = vec![name.as_str().unwrap().to_string()];
                        row.extend(b.as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()));
                        row
                    })
                    .collect();
                assert_eq!(balance_cells(&dom), want, "{stem} scenario {} step {step}: the balances Chrome shows are not the page's data", k + 1);
                let (ok, no) = lit(&dom);
                let (mut want_ok, mut want_no) = (BTreeSet::new(), BTreeSet::new());
                if step > 0 {
                    let mut by: std::collections::BTreeMap<String, bool> = std::collections::BTreeMap::new();
                    for kk in steps[step - 1]["kinds"].as_array().unwrap() {
                        let e = by.entry(kk[0].to_string()).or_insert(false);
                        *e = *e || kk[1].as_bool().unwrap();
                    }
                    for (kind, went) in by {
                        if went { want_ok.insert(kind) } else { want_no.insert(kind) };
                    }
                }
                assert_eq!((ok, no), (want_ok, want_no), "{stem} scenario {} step {step}: what the chart lights up", k + 1);
                looked += 1;
            }
        }
    }
    eprintln!("compared: {looked} steps shown in Chrome as the page's data says, in {:.0} s", started.elapsed().as_secs_f64());
}
