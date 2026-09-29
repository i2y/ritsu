//! `doc` draws what the checker knows. Its Markdown for the examples, the flows of tests/flows and a
//! first draft with errors is word for word the golden files in tests/doc, and the site's pages of
//! the examples (website/docs/doc, website/docs-ja/doc) are what it writes now. On every scenario, a
//! run lights up a way that holds together: every statement it passed is on the picture, and every
//! step it lit but the first is reached by an edge it lit. Every Mermaid chart draws, in Mermaid 11
//! and 12 in headless Chrome; and in Chrome, a page lights up what its data says.
//!
//! The rules are read with rulec (`DANDORI_RULEC`, else `rulec` on the PATH); Mermaid is in
//! tools/mermaid (`npm install --prefix tools/mermaid`); Chrome is `DANDORI_CHROME`, else Google
//! Chrome where macOS keeps it, else `google-chrome` or `chromium` on the PATH. A test that cannot
//! find them says so and skips; read the skip lines. `DANDORI_BLESS=1` rewrites the golden files and
//! the site's pages.

use dandori::diag::Lang;
use dandori::doc::{self, Cond, EdgeInfo, Input};
use dandori::interp::Visit;
use dandori::model::TK;
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rel(p: &Path) -> String {
    p.strip_prefix(root()).unwrap_or(p).display().to_string()
}

fn rulec_available() -> bool {
    let bin = std::env::var("DANDORI_RULEC").unwrap_or_else(|_| "rulec".into());
    Command::new(&bin).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

macro_rules! need_rulec {
    () => {
        if !rulec_available() {
            eprintln!("SKIP: rulec is not on the PATH; set DANDORI_RULEC to run this test");
            return;
        }
    };
}

fn chrome() -> Option<String> {
    if let Ok(c) = std::env::var("DANDORI_CHROME") {
        return Some(c);
    }
    let mac = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
    if Path::new(mac).exists() {
        return Some(mac.into());
    }
    ["google-chrome", "chromium", "chromium-browser"].iter().find(|c| Command::new(c).arg("--version").output().map(|o| o.status.success()).unwrap_or(false)).map(|c| c.to_string())
}

/// The DOM of a page after its scripts ran, as headless Chrome dumps it.
fn dump_dom(chrome: &str, url: &str) -> String {
    let out = Command::new(chrome)
        .args(["--headless=new", "--disable-gpu", "--no-first-run", "--no-default-browser-check", "--allow-file-access-from-files", "--virtual-time-budget=20000", "--dump-dom", url])
        .output()
        .expect("could not run chrome");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// The flows `doc` is held to: each example as written for Temporal and the child flow beside them,
/// in English and in Japanese, the flows of tests/flows, and a first draft whose check finds errors.
fn flows() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for lang in ["", ".ja"] {
        for ex in ["fulfillment", "hotel", "inquiry", "order", "review"] {
            out.push(root().join(format!("examples/{ex}/temporal/{ex}{lang}.flow")));
        }
        out.push(root().join(format!("examples/fulfillment/arrange_delivery{lang}.flow")));
    }
    let mut tf: Vec<PathBuf> = std::fs::read_dir(root().join("tests/flows")).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().is_some_and(|x| x == "flow")).collect();
    tf.sort();
    out.extend(tf);
    out.push(root().join("tests/fixtures/hotel_naive.flow"));
    out
}

fn key(f: &Path) -> String {
    rel(f).trim_end_matches(".flow").replace(['/', '.'], "-")
}

/// What `doc` writes for a flow: the Markdown, or the HTML page.
fn written(f: &Path, lang: Lang, html: bool) -> String {
    let (src, d) = dandori::check::drawable(f).unwrap();
    let m = d.model.as_ref().unwrap_or_else(|| panic!("{} does not lower", rel(f)));
    let file = rel(f);
    let i = Input { m, src: &src, file: &file, facts: &d.facts, diags: &d.diags, lang };
    if html {
        doc::html(&i)
    } else {
        doc::markdown(&i)
    }
}

#[test]
fn markdown_matches_the_golden_files() {
    need_rulec!();
    let bless = std::env::var("DANDORI_BLESS").is_ok();
    let mut wrong = Vec::new();
    for f in flows() {
        for (lang, tag) in [(Lang::En, "en"), (Lang::Ja, "ja")] {
            let text = written(&f, lang, false);
            let golden = root().join(format!("tests/doc/{}.{tag}.md", key(&f)));
            if bless {
                std::fs::create_dir_all(golden.parent().unwrap()).unwrap();
                std::fs::write(&golden, &text).unwrap();
                continue;
            }
            if std::fs::read_to_string(&golden).unwrap_or_default() != text {
                wrong.push(format!("{} ({tag}) differs from {}; look at the difference, then rewrite it with DANDORI_BLESS=1", rel(&f), rel(&golden)));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn the_site_shows_the_pages_doc_writes_now() {
    need_rulec!();
    let bless = std::env::var("DANDORI_BLESS").is_ok();
    let mut wrong = Vec::new();
    for ex in ["fulfillment", "hotel", "inquiry", "order", "review"] {
        // the Japanese site draws the Japanese version of each example
        for (lang, dir, version) in [(Lang::En, "website/docs/doc", ""), (Lang::Ja, "website/docs-ja/doc", ".ja")] {
            let f = root().join(format!("examples/{ex}/temporal/{ex}{version}.flow"));
            let page = written(&f, lang, true);
            let at = root().join(format!("{dir}/{ex}.html"));
            if bless {
                std::fs::create_dir_all(at.parent().unwrap()).unwrap();
                std::fs::write(&at, &page).unwrap();
                continue;
            }
            if std::fs::read_to_string(&at).unwrap_or_default() != page {
                wrong.push(format!("{} is not what `dandori doc {} --format html` writes now; rewrite it with DANDORI_BLESS=1", rel(&at), rel(&f)));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The ids a picture has: its nodes and its loops' frames.
fn ids(g: &doc::Graph) -> BTreeSet<String> {
    let mut out: BTreeSet<String> = g.nodes.iter().map(|n| n.id.clone()).collect();
    out.extend(g.loops.keys().map(|s| format!("L{s}")));
    out
}

#[test]
fn every_run_lights_up_a_way_that_holds_together() {
    need_rulec!();
    let mut runs = 0;
    for f in flows() {
        let (src, d) = dandori::check::drawable(&f).unwrap();
        let m = d.model.as_ref().unwrap();
        let file = rel(&f);
        let i = Input { m, src: &src, file: &file, facts: &d.facts, diags: &d.diags, lang: Lang::En };
        let g = doc::graph(&i);
        let known = ids(&g);
        assert_eq!(known.len(), g.nodes.len() + g.loops.len(), "{file}: two nodes share an id");
        let tick = |_: &str| None;
        let edges: Vec<EdgeInfo> = g.panels.iter().flat_map(|p| dandori::draw::panel(&g, p, &tick).edges).collect();
        for e in &edges {
            assert!(known.contains(&e.from) && known.contains(&e.to), "{file}: the edge {} goes from {} to {}, not both on the picture", e.id, e.from, e.to);
        }
        let mut pass_sites = BTreeSet::new();
        for s in m.all_stmts() {
            if matches!(s.kind, TK::Pass) {
                pass_sites.insert(s.site);
            }
        }
        if dandori::diag::has_errors(&d.diags) {
            // no scenarios for a workflow that does not pass check; each diagnostic's run lights up instead
            for dg in &d.diags {
                let lit = doc::lit_by_steps(&g, &edges, dg);
                for n in &lit.nodes {
                    assert!(known.contains(n), "{file}: {} lights up {n}, which is not on the picture", dg.code);
                }
            }
            continue;
        }
        for sc in dandori::scenarios::generate(m) {
            let (_, visits) = dandori::interp::run_visits(m, &sc, dandori::render::View::Temporal).unwrap();
            runs += 1;
            let lit = doc::lit_by_visits(&g, &edges, &visits);
            let name = format!("{file} {}", sc["name"].as_str().unwrap_or(""));
            for v in &visits {
                if let Visit::Stmt(site) = v {
                    if pass_sites.contains(site) {
                        continue;
                    }
                    let id = if known.contains(&format!("s{site}")) { format!("s{site}") } else { format!("L{site}") };
                    assert!(known.contains(&id) && lit.nodes.contains(&id), "{name}: the run passed the statement of site {site}, which is not lit");
                }
            }
            for n in &lit.nodes {
                if ["start", "onf", "onc"].contains(&n.as_str()) || n.starts_with('L') {
                    continue;
                }
                let reached = edges.iter().any(|e| e.to == *n && lit.edges.contains(&e.id));
                assert!(reached, "{name}: {n} is lit, but no lit edge goes to it; the run lit {:?}", lit.edges);
            }
            // every arm and handler it took, every way round and out of a loop, is an edge it lit; an
            // arm that goes round again is lit only when the loop did
            let lit_with = |c: Cond| edges.iter().any(|e| e.conds.contains(&c) && lit.edges.contains(&e.id));
            let round_only = |c: Cond| edges.iter().any(|e| e.conds.contains(&c) && e.conds.iter().any(|x| matches!(x, Cond::Again(_))));
            for v in &visits {
                let c = match v {
                    Visit::Arm(s, k) => Cond::Arm(*s, *k),
                    Visit::Handler(s, j) => Cond::Handler(*s, *j),
                    // a parallel loop's rounds run side by side, and it has no way round
                    Visit::Round(s, n) if *n >= 1 && edges.iter().any(|e| e.conds.contains(&Cond::Again(*s))) => Cond::Again(*s),
                    Visit::Done(s) => Cond::Done(*s),
                    _ => continue,
                };
                assert!(lit_with(c) || (matches!(c, Cond::Arm(..) | Cond::Handler(..)) && round_only(c)), "{name}: the run went by {c:?}, and no edge of it is lit");
            }
            // a loop whose first step the run passed more often than it began the loop went round
            // again, and its way round is lit
            for site in g.loops.keys() {
                let frame = format!("L{site}");
                let Some(first) = edges.iter().find(|e| e.from == frame && e.conds.is_empty()).map(|e| e.to.clone()) else { continue };
                let sequential = edges.iter().any(|e| e.conds.contains(&Cond::Again(*site)));
                let (begun, firsts) = (lit.counts.get(&frame).copied().unwrap_or(0), lit.counts.get(&first).copied().unwrap_or(0));
                if sequential && firsts > begun {
                    assert!(lit_with(Cond::Again(*site)), "{name}: the loop of site {site} went round again ({firsts} times at {first}, begun {begun} times), and its way round is not lit");
                }
            }
        }
    }
    assert!(runs > 200, "only {runs} runs were looked at");
    eprintln!("compared: {runs} runs lit up on their pictures");
}

/// The Mermaid charts of a Markdown page.
fn charts(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut open: Option<String> = None;
    for line in text.lines() {
        match (&mut open, line) {
            (None, "```mermaid") => open = Some(String::new()),
            (Some(c), "```") => {
                out.push(std::mem::take(c));
                open = None;
            }
            (Some(c), l) => {
                c.push_str(l);
                c.push('\n');
            }
            (None, _) => {}
        }
    }
    out
}

#[test]
fn every_mermaid_chart_draws() {
    if std::env::var("DANDORI_BLESS").is_ok() {
        eprintln!("SKIP: the golden files are being rewritten; run this test again without DANDORI_BLESS");
        return;
    }
    let Some(chrome) = chrome() else {
        eprintln!("SKIP: Chrome is not found; set DANDORI_CHROME to run this test");
        return;
    };
    let mm = root().join("tools/mermaid/node_modules");
    if !mm.join("mermaid-11/dist/mermaid.min.js").exists() || !mm.join("mermaid-12/dist/mermaid.min.js").exists() {
        eprintln!("SKIP: tools/mermaid/node_modules is missing; run `npm install --prefix tools/mermaid`");
        return;
    }
    let mut all: Vec<(String, String)> = Vec::new();
    let mut goldens: Vec<PathBuf> = std::fs::read_dir(root().join("tests/doc")).unwrap().map(|e| e.unwrap().path()).collect();
    goldens.sort();
    for p in goldens {
        for (k, c) in charts(&std::fs::read_to_string(&p).unwrap()).into_iter().enumerate() {
            all.push((format!("{} #{}", rel(&p), k + 1), c));
        }
    }
    assert!(all.len() > 30, "only {} charts in tests/doc", all.len());
    let scratch = std::env::temp_dir().join(format!("dandori-mermaid-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).unwrap();
    let sources: Vec<&String> = all.iter().map(|(_, c)| c).collect();
    for major in ["11", "12"] {
        let script = mm.join(format!("mermaid-{major}/dist/mermaid.min.js"));
        let page = format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"><script src=\"file://{}\"></script></head><body><pre id=\"out\">not run</pre><script>\nconst charts = {};\nmermaid.initialize({{ startOnLoad: false, securityLevel: 'strict' }});\n(async () => {{ const out = []; for (let k = 0; k < charts.length; k++) {{ try {{ await mermaid.render('c' + k, charts[k]); out.push('ok'); }} catch (e) {{ out.push('error: ' + String(e && e.message || e).split('\\n').join(' ')); }} }} document.getElementById('out').textContent = out.join('\\n'); }})();\n</script></body></html>",
            script.display(),
            serde_json::to_string(&sources).unwrap().replace("</", "<\\/")
        );
        let at = scratch.join(format!("mermaid-{major}.html"));
        std::fs::write(&at, page).unwrap();
        let dom = dump_dom(&chrome, &format!("file://{}", at.display()));
        let body = dom.split("<pre id=\"out\">").nth(1).and_then(|r| r.split("</pre>").next()).unwrap_or("");
        let results: Vec<&str> = body.lines().collect();
        assert_eq!(results.len(), all.len(), "Mermaid {major} drew {} of {} charts: {body}", results.len(), all.len());
        let failed: Vec<String> = results.iter().zip(&all).filter(|(r, _)| **r != "ok").map(|(r, (name, _))| format!("{name}: {r}")).collect();
        assert!(failed.is_empty(), "Mermaid {major} could not draw:\n{}", failed.join("\n"));
        eprintln!("compared: Mermaid {major} drew all {} charts of tests/doc", all.len());
    }
    let _ = std::fs::remove_dir_all(&scratch);
}

/// The ids of the elements a dumped page has lit up.
fn lit_in(dom: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for piece in dom.split('<').skip(1) {
        let tag = piece.split('>').next().unwrap_or("");
        let class = tag.split("class=\"").nth(1).and_then(|r| r.split('"').next()).unwrap_or("");
        if !class.split(' ').any(|c| c == "lit") {
            continue;
        }
        if let Some(id) = tag.split("data-id=\"").nth(1).and_then(|r| r.split('"').next()) {
            out.insert(id.to_string());
        }
    }
    out
}

#[test]
fn a_page_lights_up_what_its_data_says() {
    if std::env::var("DANDORI_BLESS").is_ok() {
        eprintln!("SKIP: the site's pages are being rewritten; run this test again without DANDORI_BLESS");
        return;
    }
    let Some(chrome) = chrome() else {
        eprintln!("SKIP: Chrome is not found; set DANDORI_CHROME to run this test");
        return;
    };
    let page = root().join("website/docs/doc/hotel.html");
    let text = std::fs::read_to_string(&page).expect("the site's page of the hotel booking");
    let data: Value = serde_json::from_str(text.split("<script type=\"application/json\" id=\"dd-data\">").nth(1).unwrap().split("</script>").next().unwrap().replace("<\\/", "</").as_str()).unwrap();
    let runs = data["runs"].as_array().unwrap();
    assert!(runs.len() > 40, "the hotel booking's page has {} scenarios", runs.len());
    let mut looked = 0;
    for k in [0, runs.len() / 3, runs.len() / 2, runs.len() - 1] {
        let dom = dump_dom(&chrome, &format!("file://{}#run={}", page.display(), k + 1));
        let want: BTreeSet<String> = ["nodes", "edges"].iter().flat_map(|f| runs[k][f].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string())).collect();
        let got = lit_in(&dom);
        assert_eq!(got, want, "scenario {} of {}: what Chrome lit up is not what the page's data says", k + 1, rel(&page));
        assert!(dom.contains(&format!("id=\"dd-detail\" aria-live=\"polite\"><h2 class=\"dh\">scenario {}", k + 1)), "scenario {}: the detail pane does not show its steps", k + 1);
        looked += 1;
    }
    // a step picked shows what the checker knows there, and how many scenarios pass it
    let dom = dump_dom(&chrome, &format!("file://{}#node=s5", page.display()));
    assert!(dom.contains("confirm_intent") && dom.contains("scenarios pass here."), "picking a step does not show it in the detail pane");
    // a rule's page opens over the page: the page rulec doc renders, in a frame of its own
    let rules = data["rules"].as_array().unwrap();
    assert!(rules.len() >= 2, "the hotel booking's page has {} rules", rules.len());
    for r in rules {
        let name = r["name"].as_str().unwrap();
        let dom = dump_dom(&chrome, &format!("file://{}#rule={name}", page.display()));
        let sheet = dom.split("id=\"dd-sheet\"").nth(1).and_then(|x| x.split("</iframe>").next()).unwrap_or_else(|| panic!("the page has no sheet for the rules"));
        let title = r["page"].as_str().unwrap().split("<title>").nth(1).and_then(|x| x.split("</title>").next()).unwrap();
        assert!(!sheet.split('>').next().unwrap().contains("hidden"), "#rule={name} does not open the rule's page");
        // an attribute's `<` is written as it is or as `&lt;`, by the version of Chrome
        let shown = [format!("<title>{title}</title>"), format!("&lt;title&gt;{title}&lt;/title&gt;")];
        assert!(shown.iter().any(|t| sheet.contains(t.as_str())), "#rule={name} opens another page than the one rulec doc renders for it");
        looked += 1;
    }
    eprintln!("compared: {looked} scenarios and rules shown in Chrome as the page's data says");
}
