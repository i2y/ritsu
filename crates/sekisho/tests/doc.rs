//! `sekisho doc` (DESIGN 7): the pages of the example's two versions, the English one and its
//! Japanese twin, each in English and in Japanese as Markdown and in its own language as HTML, and
//! of the materials that show what the example does not (the forms of condition and two resource
//! types, a forbid read with `use gate`, an action that guards two operations, an answer that rests
//! on a value no language vouches for, an expectation that picks nothing), are the golden files in
//! `tests/golden/doc/` — but for the pages of the rules and the dates the page holds, which are
//! what rulec's and koyomi's own `doc` draw now, word for word, and stand in the golden files as
//! one line each. The HTML reads nothing from outside, and neither does any page it holds; in
//! Chrome, it draws, and a page it holds opens over it from its button and from `#page=<n>`.
//!
//! Chrome is found by ritsu-testkit (`RITSU_CHROME` or `SEKISHO_CHROME`, else Google Chrome where
//! macOS keeps it, else `google-chrome` or `chromium` on the PATH); a test that cannot find it says
//! so and skips. `SEKISHO_BLESS=1` (or `RITSU_BLESS=1`) writes the golden files again.

mod common;

use ritsu_base::naming::{Name, Tool};
use ritsu_base::text::Lang;
use sekisho::doc::{self, Call, Embedded, FlowCalls, Format, Page};
use sekisho::suite::Suite;
use std::time::Duration;

/// Each page held to a golden file: the `.gate`, the language, the form, and the file's stem.
const PAGES: [(&str, Lang, Format, &str); 14] = [
    ("examples/refunds/refunds.gate", Lang::En, Format::Markdown, "refunds.en.md"),
    ("examples/refunds/refunds.gate", Lang::Ja, Format::Markdown, "refunds.ja.md"),
    ("examples/refunds/refunds.gate", Lang::En, Format::Html, "refunds.html"),
    ("examples/refunds/refunds.ja.gate", Lang::En, Format::Markdown, "refunds.ja.en.md"),
    ("examples/refunds/refunds.ja.gate", Lang::Ja, Format::Markdown, "refunds.ja.ja.md"),
    ("examples/refunds/refunds.ja.gate", Lang::Ja, Format::Html, "refunds.ja.html"),
    ("tests/gen/conditions.gate", Lang::En, Format::Markdown, "conditions.en.md"),
    ("tests/gen/conditions.ja.gate", Lang::Ja, Format::Markdown, "conditions.ja.ja.md"),
    ("tests/gen/papers.gate", Lang::En, Format::Markdown, "papers.en.md"),
    ("tests/gen/guards.gate", Lang::En, Format::Markdown, "guards.en.md"),
    ("tests/mutants/W303_a_value_no_input_reaches.gate", Lang::En, Format::Markdown, "W303_a_value_no_input_reaches.en.md"),
    ("tests/mutants/W303_どの入力も届かない値.gate", Lang::Ja, Format::Markdown, "W303_どの入力も届かない値.ja.md"),
    ("tests/mutants/W304_expect_picks_nothing.gate", Lang::En, Format::Markdown, "W304_expect_picks_nothing.en.md"),
    ("tests/mutants/W304_何も選ばない期待.gate", Lang::Ja, Format::Markdown, "W304_何も選ばない期待.ja.md"),
];

/// The page of a file, with every language joined as `ritsu sekisho` joins them: what its
/// workflows' flows call, too, as dandori answers it.
fn page_of(path: &str, lang: Lang, format: Format) -> Page {
    let o = common::check(path);
    let suite = common::joined();
    let calls = doc::calls_of(&o, &suite);
    doc::page(&o, &suite, lang, format, calls.as_deref()).unwrap_or_else(|| panic!("{path} has no page:\n{}", common::shown(&o, Lang::En)))
}

/// What `sekisho doc` prints for a file, as `ritsu sekisho doc` runs it: the exit code and the text.
fn run(args: &[&str], suite: Suite) -> (u8, String, String) {
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = sekisho::run::run(&args, suite, &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

/// The page the language draws for a file the page holds, as its own port draws it.
fn drawn(e: &Embedded, path: &str, lang: Lang, format: Format) -> String {
    use ritsu_ports::{Dates, Rules};
    let dir = std::path::Path::new(path).parent().unwrap();
    // the file as the gate reaches it: the root of the example is the gate's directory, and the
    // materials of tests/ write their paths from the crate's directory
    let file = if path.starts_with("examples/") { dir.join(&e.file) } else { std::path::PathBuf::from(&e.file) };
    let html = format == Format::Html;
    let p = match e.tool {
        "rulec" => rulec::ports::Engine::with_dates(std::sync::Arc::new(koyomi::ports::Engine)).doc(&file, &e.file, html, lang),
        _ => koyomi::ports::Engine.doc(&file, &e.file, html, lang),
    };
    p.unwrap_or_else(|s| panic!("{}: {s:?}", e.file))
}

/// The table of the head that gives each file `gen --target cedar` writes with its digest.
fn cedar_table(p: &mut Page) -> Option<&mut Vec<Vec<String>>> {
    p.blocks.iter_mut().find_map(|b| match b {
        doc::Block::Table { head, rows, .. } if head.get(1).is_some_and(|h| h.starts_with("SHA-256")) => Some(rows),
        _ => None,
    })
}

/// The page as its golden file holds it: sekisho's version as `<version>`, each page of another
/// language as one line that names it, and the digests of the generated Cedar as `<sha256>` —
/// what the generator of Cedar writes is held to the page by its own test
/// (`the_head_gives_the_digests_of_the_cedar_gen_writes`).
fn as_golden(mut p: Page, format: Format) -> String {
    for e in &mut p.embedded {
        e.page = Ok(format!("[the page {} doc draws of {}]", e.tool, e.file));
    }
    if let Some(rows) = cedar_table(&mut p) {
        for r in rows {
            r[1] = "`<sha256>`".into();
        }
    }
    doc::write(&p, format).replace(env!("CARGO_PKG_VERSION"), "<version>")
}

/// The head gives each of the four files `gen --target cedar` writes from the file, in the page's
/// language, with the first sixteen hex digits of its SHA-256, as the generator writes them now.
#[test]
fn the_head_gives_the_digests_of_the_cedar_gen_writes() {
    for (path, lang, format, stem) in PAGES {
        let o = common::check(path);
        let files = sekisho::cedar::files(o.scope.as_ref().unwrap(), o.walked.as_ref().unwrap(), &ritsu_emit::header::file_name(path), lang).unwrap();
        let want: Vec<Vec<String>> = files.paths(&o.walked.as_ref().unwrap().gate.named.alias).iter().map(|(f, text)| vec![format!("`{f}`"), format!("`{}`", ritsu_base::sha256::short(text.as_bytes()))]).collect();
        let mut p = page_of(path, lang, format);
        assert_eq!(cedar_table(&mut p).cloned(), Some(want), "{stem}");
    }
}

#[test]
fn the_pages_match_the_golden_files() {
    let mut wrong = Vec::new();
    for (path, lang, format, stem) in PAGES {
        let p = page_of(path, lang, format);
        if let Err(e) = ritsu_testkit::golden::check(std::path::Path::new(&format!("tests/golden/doc/{stem}")), &as_golden(p, format)) {
            wrong.push(e);
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// The pages of the rules, the dates files and the calendars a page holds are what rulec's and
/// koyomi's `doc` draw for them now, in the page's language and form; each file once.
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
            assert_eq!(*page, drawn(e, path, lang, format), "{path}: the page of {} is not what {} doc draws", e.file, e.tool);
            looked += 1;
        }
    }
    // the example's rule, dates file and calendar, in each of its six pages and in the two pages of
    // W304, which is the example with one more expectation; the rule of W303, in its two
    assert_eq!(looked, 6 * 3 + 2 * 3 + 2, "pages held");
    // the page of the example as the command prints it holds them as they are
    let (code, out, err) = run(&["doc", "examples/refunds/refunds.gate", "--root", "examples/refunds"], common::joined());
    assert_eq!(code, 0, "{err}");
    assert_eq!(out, doc::write(&page_of("examples/refunds/refunds.gate", Lang::En, Format::Markdown), Format::Markdown));
    assert!(out.contains("<summary>The page of <code>rules/refund_limit.rule</code>, as <code>rulec doc</code> draws it</summary>"), "{out}");
}

/// The Cedar a page shows beside each policy is the policy as the `.cedar` file that `gen --target
/// cedar` writes holds it, word for word: one block a policy on an action.
#[test]
fn the_cedar_beside_a_policy_is_the_generated_cedar() {
    fn cedar_of(b: &doc::Block, out: &mut Vec<String>) {
        match b {
            doc::Block::Code { lang: "cedar", text, .. } => out.push(text.clone()),
            doc::Block::Side(xs) => xs.iter().for_each(|x| cedar_of(x, out)),
            doc::Block::Fold { body, .. } => body.iter().for_each(|x| cedar_of(x, out)),
            _ => {}
        }
    }
    for (path, lang, format, stem) in PAGES {
        if format != Format::Markdown {
            continue;
        }
        let o = common::check(path);
        let (scope, checked) = (o.scope.as_ref().unwrap(), o.walked.as_ref().unwrap());
        let files = sekisho::cedar::files(scope, checked, &ritsu_emit::header::file_name(path), lang).unwrap();
        let p = page_of(path, lang, format);
        let mut shown = Vec::new();
        p.blocks.iter().for_each(|b| cedar_of(b, &mut shown));
        let on_actions = checked.gate.policies.iter().filter(|x| !x.actions.is_empty()).count();
        assert_eq!(shown.len(), on_actions, "{stem}: the policies shown");
        for c in &shown {
            assert!(files.policies.contains(&format!("{c}\n")), "{stem}: a policy that is not in the .cedar file:\n{c}");
        }
    }
}

/// The HTML is one page that reads nothing from outside: no stylesheet, script or picture of
/// another server, and no page it holds reads one either. Its colours follow the reader's light or
/// dark, and it fits a narrow screen.
#[test]
fn the_html_reads_nothing_from_outside() {
    for (path, lang, format, stem) in PAGES {
        if format != Format::Html {
            continue;
        }
        let p = page_of(path, lang, format);
        for e in &p.embedded {
            let page = e.page.as_ref().unwrap();
            assert!(ritsu_base::docpage::outside_urls(page).is_empty(), "{stem}: the page of {} reads {:?}", e.file, ritsu_base::docpage::outside_urls(page));
        }
        let html = doc::write(&p, format);
        assert!(ritsu_base::docpage::outside_urls(&html).is_empty(), "{stem}: the page reads {:?}", ritsu_base::docpage::outside_urls(&html));
        for bad in ["<link", "<img", "<script src", "@import"] {
            assert!(!html.contains(bad), "{stem}: the page holds `{bad}`");
        }
        assert!(html.contains("@media (prefers-color-scheme: dark)") && html.contains("name=\"viewport\""), "{stem}: no dark colours or no viewport");
        assert!(html.starts_with("<!doctype html>\n") && html.contains(&format!("<html lang=\"{}\">", lang.code())), "{stem}");
        // the pages it holds are text in its data, which nothing ends early
        let data = html.split("<script type=\"application/json\" id=\"sk-pages\">").nth(1).and_then(|r| r.split("</script>").next()).expect("the data of the pages");
        assert!(!data.contains('<'), "{stem}: a `<` in the data of the pages");
        let v = ritsu_base::json::parse(data).unwrap();
        let held: Vec<&str> = v.as_arr().unwrap().iter().map(|x| x.get("page").and_then(|x| x.as_str()).unwrap()).collect();
        assert_eq!(held, p.embedded.iter().map(|e| e.page.as_deref().unwrap()).collect::<Vec<_>>(), "{stem}");
    }
}

/// What a `.gate` writes stays text where the page puts it (ritsu's DESIGN 9.2): a description that
/// holds `</script>` is text in the HTML and in the Markdown, where a `<` that could begin HTML is
/// written `&lt;` outside the code spans; a carriage return starts no line of the Markdown.
#[test]
fn text_from_the_gate_stays_where_the_page_puts_it() {
    let t = ritsu_testkit::TempDir::new("doc-text");
    let src = std::fs::read_to_string("tests/gen/conditions.gate").unwrap().replacen(
        "description \"Edits documents\"",
        "description \"Edits </script><script>alert(1)</script>\r# Injected; `<b>` and a < 3\"",
        1,
    );
    let gate = t.write("conditions.gate", &src);
    let gate = gate.to_string_lossy().to_string();
    let o = sekisho::check::check_file(&gate, &common::joined(), &sekisho::check::Options { root: Some(t.path().to_path_buf()), ..Default::default() }).unwrap();
    assert!(!o.has_errors(), "{}", common::shown(&o, Lang::En));
    for lang in [Lang::En, Lang::Ja] {
        let md = doc::write(&doc::page(&o, &common::joined(), lang, Format::Markdown, None).unwrap(), Format::Markdown);
        assert!(!md.contains('\r'), "{md:?}");
        assert!(md.contains("Edits &lt;/script>&lt;script>alert(1)&lt;/script> # Injected; `<b>` and a < 3"), "{md}");
        assert!(!md.contains("<script>") && !md.contains("</script>"), "{md}");
        let html = doc::write(&doc::page(&o, &common::joined(), lang, Format::Html, None).unwrap(), Format::Html);
        assert!(!html.contains("<script>alert") && html.contains("Edits &lt;/script&gt;&lt;script&gt;alert(1)&lt;/script&gt;"), "{html}");
    }
}

/// Given what a workflow's flow calls (dandori's answer, which `ritsu sekisho doc` asks for), the
/// part of the page on the workflow lists each call with the action that guards it, how far the
/// workflow is allowed it, and the error the task declares for a denial; an operation no action
/// guards says so.
#[test]
fn the_calls_of_a_flow_stand_under_its_workflow() {
    let o = common::check("examples/refunds/refunds.gate");
    let op = |id: &str| Name::file(Tool::Openapi, "api/orders.json").with("operation", id);
    let calls = [FlowCalls {
        workflow: 0,
        calls: Ok(vec![
            Call { line: 20, task: "refund_order".into(), operation: op("refundOrder"), denied: Some("denied".into()) },
            Call { line: 21, task: "look".into(), operation: op("getOrder"), denied: None },
            Call { line: 22, task: "elsewhere".into(), operation: Name::file(Tool::Openapi, "api/other.json").with("operation", "x"), denied: None },
        ]),
    }];
    for (lang, rows) in [
        (
            Lang::En,
            [
                "| 20 | `refund_order` | `openapi \"api/orders.json\" operation refundOrder` | `refund_order` | sometimes | `denied` |",
                "| 21 | `look` | `openapi \"api/orders.json\" operation getOrder` | `view_order` | never (it takes no Workflow) | none declared |",
                "| 22 | `elsewhere` | `openapi \"api/other.json\" operation x` | (no action guards it) | - | none declared |",
            ],
        ),
        (
            Lang::Ja,
            [
                "| 20 | `refund_order` | `openapi \"api/orders.json\" operation refundOrder` | `refund_order` | 組み合わせによる | `denied` |",
                "| 21 | `look` | `openapi \"api/orders.json\" operation getOrder` | `view_order` | 許さない（Workflow を取らない） | 宣言していない |",
                "| 22 | `elsewhere` | `openapi \"api/other.json\" operation x` | （守る action なし） | - | 宣言していない |",
            ],
        ),
    ] {
        let md = doc::write(&doc::page(&o, &common::joined(), lang, Format::Markdown, Some(&calls)).unwrap(), Format::Markdown);
        for r in rows {
            assert!(md.contains(r), "{lang:?}: no row {r}:\n{}", md.split("## ").find(|s| s.starts_with("Workflows") || s.starts_with("ワークフロー")).unwrap_or(""));
        }
        // without them, the part on the workflow says what it is allowed, and no more
        let without = doc::write(&doc::page(&o, &common::joined(), lang, Format::Markdown, None).unwrap(), Format::Markdown);
        assert!(!without.contains("#### What the flow calls") && !without.contains("#### フローが呼ぶ操作"), "{lang:?}");
    }
    // what dandori says when it cannot read the flow
    let said = ritsu_ports::Said { code: "E001".into(), file: "flows/returns.flow".into(), line: Some(3), message: ritsu_base::text::Text::same("broken") };
    let md = doc::write(&doc::page(&o, &common::joined(), Lang::En, Format::Markdown, Some(&[FlowCalls { workflow: 0, calls: Err(vec![said]) }])).unwrap(), Format::Markdown);
    assert!(md.contains("dandori could not read the flow.") && md.contains("flows/returns.flow: broken"), "{md}");
}

/// With dandori joined, the page asks it what each workflow's flow calls (the port of flows, as
/// ritsu-cross reads it for X16): the example's workflow `returns` calls `refundOrder` at line 22
/// of its flow, which `refund_order` guards and allows the workflow in some combinations, and the
/// task declares `denied` for a 403. Without dandori there is no answer, and no table.
#[test]
fn dandori_says_what_the_flow_calls() {
    let o = common::check("examples/refunds/refunds.gate");
    let calls = doc::calls_of(&o, &common::joined()).expect("dandori is joined");
    assert_eq!(calls.len(), 1);
    let got = calls[0].calls.as_ref().unwrap_or_else(|s| panic!("{s:?}"));
    assert_eq!(
        got,
        &vec![Call { line: 22, task: "refund_order".into(), operation: Name::file(Tool::Openapi, "api/orders.json").with("operation", "refundOrder"), denied: Some("denied".into()) }]
    );
    assert!(doc::calls_of(&o, &Suite::default()).is_none());
    let (code, out, _) = run(&["doc", "examples/refunds/refunds.gate", "--root", "examples/refunds"], common::joined());
    assert_eq!(code, 0);
    assert!(out.contains("| 22 | `refund_order` | `openapi \"api/orders.json\" operation refundOrder` | `refund_order` | sometimes | `denied` |"), "{out}");
}

/// A file that does not pass has no page: what check says, and exit 1. One that reads a rule, a
/// dates file, a calendar or a flow cannot be read by the binary of sekisho's own crate (E209, exit
/// 2); one that reads none of them is drawn by it as by `ritsu sekisho`.
#[test]
fn the_page_is_of_a_file_that_passes() {
    let (code, out, err) = run(&["doc", "tests/mutants/E302_forbid_covers_permit.gate", "--root", "."], common::joined());
    assert_eq!(code, 1, "{out}{err}");
    assert!(out.contains("error[E302]: ") && err.contains("does not pass check, so it has no page"), "{out}{err}");
    let (code, out, _) = run(&["doc", "examples/refunds/refunds.gate"], Suite::default());
    assert_eq!(code, 2);
    assert!(out.contains("error[E209]") && out.contains("ritsu sekisho doc examples/refunds/refunds.gate"), "{out}");
    let alone = run(&["doc", "tests/gen/conditions.gate", "--root", ".", "--format", "html"], Suite::default());
    let joined = run(&["doc", "tests/gen/conditions.gate", "--root", ".", "--format", "html"], common::joined());
    assert_eq!(alone.0, 0, "{}", alone.2);
    assert_eq!(alone, joined);
    // one file a page
    let (code, _, err) = run(&["doc", "examples/refunds/refunds.gate", "examples/refunds/refunds.ja.gate"], common::joined());
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
        ritsu_testkit::skip("Chrome is not found; set RITSU_CHROME (or SEKISHO_CHROME) to its binary to run this test");
    }
    c
}

/// In Chrome, each HTML page draws as wide as the window; each page it holds opens over it, from
/// `#page=<n>` and from its button, in a frame of its own, and is the page rulec or koyomi drew.
#[test]
fn chrome_draws_the_page_and_opens_the_pages_it_holds() {
    let Some(chrome) = chrome() else { return };
    let work = ritsu_testkit::TempDir::new("doc-chrome");
    for (path, lang, format, stem) in PAGES {
        if format != Format::Html {
            continue;
        }
        let p = page_of(path, lang, format);
        let html = doc::write(&p, format);
        let at = work.write(stem, &html);
        let png = work.path().join(format!("{stem}.png"));
        ritsu_testkit::chrome::screenshot(&chrome, &format!("file://{}", at.display()), &png, 1280, 900, Duration::from_secs(60)).unwrap_or_else(|e| panic!("{stem}: {e}"));
        assert_eq!(ritsu_testkit::chrome::png_size(&std::fs::read(&png).unwrap()), Some((1280, 900)), "{stem}: the picture");
        for (k, e) in p.embedded.iter().enumerate() {
            let title = e.page.as_ref().unwrap().split("<title>").nth(1).and_then(|x| x.split("</title>").next()).unwrap_or_else(|| panic!("{stem}: the page of {} has no title", e.file));
            // an attribute's `<` is written as it is or as `&lt;`, by the version of Chrome
            let shown = [format!("<title>{title}</title>"), format!("&lt;title&gt;{title}&lt;/title&gt;")];
            let sheet_of = |dom: &str| -> String { dom.split("id=\"sk-sheet\"").nth(1).and_then(|x| x.split("</iframe>").next()).unwrap_or_else(|| panic!("{stem}: the page has no sheet")).to_string() };
            // from the address
            let dom = ritsu_testkit::chrome::dump_dom(&chrome, &format!("file://{}#page={}", at.display(), k + 1), 5000, Duration::from_secs(60));
            let sheet = sheet_of(&dom);
            assert!(!sheet.split('>').next().unwrap().contains("hidden"), "{stem}: #page={} does not open the page of {}", k + 1, e.file);
            assert!(shown.iter().any(|t| sheet.contains(t.as_str())), "{stem}: #page={} opens another page than the one {} doc draws of {}", k + 1, e.tool, e.file);
            // from its button
            let press = format!(
                "<script>window.addEventListener('load', () => {{ document.querySelector('button[data-page=\"{k}\"]').click(); const o = document.createElement('pre'); o.id = 'pressed'; o.textContent = document.getElementById('sk-sheet').hidden + ' ' + document.querySelector('#sk-sheet .what').textContent; document.body.appendChild(o); }});</script>\n</body>"
            );
            let pressed = work.write(&format!("{stem}-{k}.html"), html.replace("</body>", &press));
            let dom = ritsu_testkit::chrome::dump_dom(&chrome, &format!("file://{}", pressed.display()), 5000, Duration::from_secs(60));
            let said = dom.split("<pre id=\"pressed\">").nth(1).and_then(|r| r.split("</pre>").next()).unwrap_or_else(|| panic!("{stem}: the page did not answer"));
            assert_eq!(said, format!("false {} · {} doc", e.file, e.tool), "{stem}: pressing the button of {}", e.file);
            let sheet = sheet_of(&dom);
            assert!(shown.iter().any(|t| sheet.contains(t.as_str())), "{stem}: the button of {} opens another page", e.file);
        }
        eprintln!("compared: {stem} drawn in Chrome, and the {} pages it holds open over it", p.embedded.len());
    }
    ritsu_testkit::chrome::none_left(work.path()).unwrap();
}
