//! The pages of `doc` (DESIGN 11.1, 11.3): the page of each request of `examples/`, of each file it
//! shares and of the longest a ticket of it takes, in English and in Japanese, as Markdown and as
//! HTML, held to the files in `tests/golden/doc/` (`RINGI_BLESS=1 cargo test -p ringi --test doc`
//! writes them again; read the diff). Beside them: what the route's table says, held to what
//! `simulate` says inside and outside each case, on both sides of each boundary and at the ends of
//! the range; the ranges, the days a request can be used on, the longest a ticket takes and who may
//! submit it, held to what `check` and `api` say; an HTML page whose tags all close and whose text
//! is escaped once; the two formats saying the same; and one page however the file's path is
//! written.

mod common;

use ringi::ast::{Route, Spec};
use ringi::check::{Options, Run};
use ringi::simulate::{Ask, On, simulate};
use ritsu_base::json::Json;
use ritsu_base::text::Lang;
use std::path::Path;

/// `ringi doc` with the arguments, as `ritsu ringi doc` runs it: the exit code, out and err.
fn doc(args: &[&str]) -> (u8, String, String) {
    let mut all = vec!["doc".to_string()];
    all.extend(args.iter().map(|s| s.to_string()));
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = ringi::run::run(&all, common::joined(), &mut out, &mut err);
    (code, String::from_utf8(out).unwrap(), String::from_utf8(err).unwrap())
}

/// The requests of `examples/`, each with the flags its pages are drawn with: the contract request
/// reads the organization in its version of 2026-11-16, since a stage of it cannot be answered in
/// the version of 2027-04-01 (E504).
const REQUESTS: [(&str, &[&str]); 5] = [("expense", &[]), ("purchase", &[]), ("contract", &["--at", "2026-11-16"]), ("leave", &[]), ("account", &[])];

/// The files the requests share: an organization, masters and a shared route.
const SHARED: [&str; 3] = ["company", "masters", "manager_approval"];

/// The page of `file` under the root `examples`, which it passes.
fn page(file: &str, flags: &[&str]) -> String {
    let mut args = vec![file, "--root", "examples"];
    args.extend(flags);
    let (code, out, err) = doc(&args);
    assert_eq!(code, 0, "{file} {flags:?}: {err}");
    assert!(err.is_empty(), "{file} {flags:?}: {err}");
    out
}

/// Every page the golden files hold, by the name of its golden file.
fn pages() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (name, flags) in REQUESTS {
        for (file, lang, suffix) in [(format!("examples/{name}.ringi"), "en", ""), (format!("examples/{name}.ja.ringi"), "ja", ".ja")] {
            let with = |more: &[&str]| {
                let mut f: Vec<&str> = flags.to_vec();
                f.extend(["--lang", lang]);
                f.extend(more);
                page(&file, &f)
            };
            out.push((format!("{name}{suffix}.md"), with(&[])));
            out.push((format!("{name}{suffix}.html"), with(&["--format", "html"])));
            out.push((format!("{name}{suffix}.longest.md"), with(&["--longest"])));
            out.push((format!("{name}{suffix}.longest.html"), with(&["--longest", "--format", "html"])));
        }
    }
    for name in SHARED {
        let file = format!("examples/shared/{name}.ringi");
        for (lang, suffix) in [("en", ""), ("ja", ".ja")] {
            out.push((format!("{name}{suffix}.md"), page(&file, &["--lang", lang])));
            out.push((format!("{name}{suffix}.html"), page(&file, &["--lang", lang, "--format", "html"])));
        }
    }
    out
}

#[test]
fn the_pages_are_held_to_their_golden_files() {
    let dir = Path::new("tests/golden/doc");
    let pages = pages();
    for (name, text) in &pages {
        ritsu_testkit::golden::golden(dir.join(name), text);
    }
    // a golden file of a page that is no more is not left behind
    if !ritsu_testkit::golden::bless() {
        let mut there: Vec<String> = std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
        let mut drawn: Vec<String> = pages.iter().map(|(n, _)| n.clone()).collect();
        there.sort();
        drawn.sort();
        assert_eq!(there, drawn);
    }
}

#[test]
fn every_html_page_closes_its_tags_and_escapes_once() {
    for (name, text) in pages().iter().filter(|(n, _)| n.ends_with(".html")) {
        let probs = common::html_problems(text);
        assert!(probs.is_empty(), "{name}: {probs:?}");
        assert!(text.starts_with("<!doctype html>\n") && text.ends_with("</html>\n"), "{name}");
    }
}

/// The text of a page as a person reads it, in words: for Markdown, with no mark of a heading, a
/// list, a note or a table, nor a backquote; for HTML, the `<main>` with no tag and no table of
/// contents, and its text unescaped. The comment that heads both is left out.
fn words_of(page: &str, html: bool) -> Vec<String> {
    let mut text = String::new();
    if html {
        let main = &page[page.find("<main>").unwrap()..page.find("</main>").unwrap()];
        let main = match (main.find("<nav"), main.find("</nav>")) {
            (Some(a), Some(b)) => format!("{}{}", &main[..a], &main[b + "</nav>".len()..]),
            _ => main.to_string(),
        };
        // a code span is in the words around it; any other tag parts them
        let main = main.replace("<code>", "").replace("</code>", "");
        let mut in_tag = false;
        for c in main.chars() {
            match c {
                '<' => in_tag = true,
                '>' if in_tag => {
                    in_tag = false;
                    text.push(' ');
                }
                c if !in_tag => text.push(c),
                _ => {}
            }
        }
        text = text.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&amp;", "&");
    } else {
        for line in page.lines().filter(|l| !l.starts_with("<!--")) {
            let line = line.trim_start_matches('#').trim_start_matches("> ").trim_start_matches("- ");
            if line.starts_with("|---") {
                continue;
            }
            let line = if line.starts_with('|') {
                // the cells, at each `|` that is not escaped
                let mut cells = Vec::new();
                let mut cell = String::new();
                let mut cs = line.chars().peekable();
                while let Some(c) = cs.next() {
                    match c {
                        '\\' if cs.peek() == Some(&'|') => {
                            cell.push('|');
                            cs.next();
                        }
                        '|' => cells.push(std::mem::take(&mut cell)),
                        c => cell.push(c),
                    }
                }
                cells.join(" ")
            } else {
                line.to_string()
            };
            text.push_str(&line.replace("&lt;", "<"));
            text.push('\n');
        }
        text = text.replace('`', "");
    }
    text.split_whitespace().map(str::to_string).collect()
}

#[test]
fn markdown_and_html_say_the_same() {
    let pages = pages();
    for (name, md) in pages.iter().filter(|(n, _)| n.ends_with(".md")) {
        let html_name = name.replace(".md", ".html");
        let Some((_, html)) = pages.iter().find(|(n, _)| *n == html_name) else { continue };
        let (a, b) = (words_of(md, false), words_of(html, true));
        let at = a.iter().zip(&b).position(|(x, y)| x != y);
        assert!(a == b, "{name}: they part at the word {at:?}: {:?} and {:?}", at.map(|i| &a[i.saturating_sub(3)..(i + 3).min(a.len())]), at.map(|i| &b[i.saturating_sub(3)..(i + 3).min(b.len())]));
    }
}

/// A request whose texts hold what Markdown and HTML mark: each is written once, escaped as its
/// format escapes it, and a `|` does not end a cell.
#[test]
fn what_the_formats_mark_is_escaped_once() {
    let src = "request marks v1 \"Tools & <parts> | spares\"\ndescription \"For a & b, <c> | e\"\n\nuse organization people from \"shared/company.ringi\"\n\nform\n  kind : choice \"Kind | sort\"\n    small \"Small & <cheap>\"\n    large \"Large | wide\"\n  note : text \"Note <a>\"  count <=100  shown when kind is large\n\nroute\n  check  finance\n\nwho\n  finance = team accounting\n\nticket\n  self approval  not allowed\n";
    let path = "examples/marks.ringi";
    let mut files = ritsu_base::fs::Overlay::new(ritsu_base::fs::current());
    files.put(&ritsu_base::paths::absolute(Path::new(path)), src);
    let (md, html) = ritsu_base::fs::with(std::rc::Rc::new(files), || (page(path, &[]), page(path, &["--format", "html"])));
    for line in ["# Tools & &lt;parts> | spares\n", "| Kind \\| sort | one of Small & &lt;cheap> or Large \\| wide |", "| Note &lt;a> | text, one line |"] {
        assert!(md.contains(line), "{line:?} is not in:\n{md}");
    }
    for line in ["<h1>Tools &amp; &lt;parts&gt; | spares</h1>", "<td>Kind | sort</td>", "Small &amp; &lt;cheap&gt;", "For a &amp; b, &lt;c&gt; | e"] {
        assert!(html.contains(line), "{line:?} is not in:\n{html}");
    }
    assert!(common::html_problems(&html).is_empty(), "{:?}", common::html_problems(&html));
}

/// Where the organization has no data, the page says so once, at its head, and names nobody: who
/// answers a stage and who may submit are not decided (DESIGN 8.6), and no version is counted.
#[test]
fn a_page_with_no_data_of_the_organization_names_nobody() {
    let org = "examples/shared/company.ringi";
    let src = std::fs::read_to_string(org).unwrap();
    let without: String = src.lines().filter(|l| !l.starts_with("data ")).map(|l| format!("{l}\n")).collect();
    assert_ne!(without, src);
    let mut files = ritsu_base::fs::Overlay::new(ritsu_base::fs::current());
    files.put(&ritsu_base::paths::absolute(Path::new(org)), without.as_str());
    let files = std::rc::Rc::new(files);
    for (lang, says, versions) in [
        ("en", "No data of the organization is read: the organization's file has no `data` line, and no `--org` is given. This page does not say who answers each stage or who may submit the request.", "The organization is read"),
        ("ja", "組織のデータを読んでいません。組織のファイルに `data` の行が無く、`--org` もありません。このページは、各ステップにだれが答えるかと、だれが出せるかを言いません。", "組織は、"),
    ] {
        let md = ritsu_base::fs::with(files.clone(), || page("examples/leave.ringi", &["--lang", lang]));
        assert_eq!(md.matches(says).count(), 1, "{md}");
        assert!(!md.contains(versions), "{md}");
        // no person's name from the data: not in a scenario, nor among the proxies
        for person in ["Sales staff", "Head of Sales Division", "営業担当", "営業本部長", "e1201 (", "e1201（"] {
            assert!(!md.contains(person), "{person} in:\n{md}");
        }
    }
    // the organization's own page says it once, with how to give the data
    for (lang, says) in [
        ("en", "> No data is read: the organization's file has no `data` line, and no `--org` is given. Write a `data` line"),
        ("ja", "> データを読んでいません。組織のファイルに `data` の行が無く、`--org` もありません。組織のファイルに `data` の行を書くか"),
    ] {
        let md = ritsu_base::fs::with(files.clone(), || page(org, &["--lang", lang]));
        assert_eq!(md.matches(says).count(), 1, "{md}");
        assert!(!md.contains("Sales staff") && !md.contains("営業担当"), "{md}");
    }
}

#[test]
fn a_page_reads_alike_however_its_path_is_written() {
    // a request, a shared route, an organization (whose data has a file of proxies, which its page
    // names) and masters
    for f in ["examples/expense.ringi", "examples/shared/manager_approval.ringi", "examples/shared/company.ringi", "examples/shared/masters.ringi"] {
        let abs = ritsu_base::paths::absolute(Path::new(f)).display().to_string();
        let first = page(f, &[]);
        for spelled in [format!("./{f}"), f.replacen("examples/", "examples/shared/../", 1), abs] {
            assert_eq!(page(&spelled, &[]), first, "{spelled}");
        }
        if f.ends_with("company.ringi") {
            assert!(first.contains("(`../org/proxies.csv`)"), "{first}");
        }
    }
    assert!(page("examples/expense.ringi", &[]).contains("File: `expense.ringi`"));
}

/// No page holds the day it is drawn on: what it says of days is what the files say, and nothing
/// that draws it reads the clock (the golden files, drawn on another day, hold it too).
#[test]
fn no_page_reads_the_clock() {
    for e in std::fs::read_dir("src/doc").unwrap() {
        let path = e.unwrap().path();
        let src = std::fs::read_to_string(&path).unwrap();
        for clock in ["SystemTime", "Instant::now", "On::Now"] {
            assert!(!src.contains(clock), "{} reads the clock ({clock})", path.display());
        }
    }
    let cli = std::fs::read_to_string("src/cli.rs").unwrap();
    let doc_cmd = &cli[cli.find("fn doc_cmd(").unwrap()..];
    let doc_cmd = &doc_cmd[..doc_cmd.find("\n}\n").unwrap()];
    assert!(!doc_cmd.contains("now()"), "{doc_cmd}");
}

#[test]
fn what_is_not_a_page_is_refused() {
    // `--longest` is a ticket's: only a request has one
    let (code, out, err) = doc(&["examples/shared/manager_approval.ringi", "--longest"]);
    assert_eq!((code, out.as_str()), (2, ""), "{err}");
    assert!(err.contains("`--longest` shows a ticket of a request"), "{err}");
    // `--on` is the day of `--longest`'s ticket
    let (code, _, err) = doc(&["examples/expense.ringi", "--on", "2026-04-03"]);
    assert_eq!(code, 2, "{err}");
    // a file that does not pass its check says why, as `check` does
    let (code, out, err) = doc(&["examples/contract.ringi"]);
    assert_eq!((code, out.as_str()), (1, ""));
    assert!(err.contains("error[E504]"), "{err}");
    // one file
    let (code, _, err) = doc(&["examples/expense.ringi", "examples/leave.ringi"]);
    assert_eq!(code, 2, "{err}");
}

// ── What the route's table says, held to `simulate` ───────────────────────

/// The stages `simulate` gives a form, each with how it is written: the requester e1201 on
/// 2026-11-16; or what it says when it refuses the form.
fn simulated(file: &str, at: Option<&str>, form: &str) -> Result<Vec<(String, String)>, String> {
    let run = Run::with(common::joined(), Options { at: at.map(|d| ritsu_ports::day_of(d).unwrap()), ..Options::default() });
    let o = run.file(file).unwrap();
    assert!(!o.has_errors(), "{}", common::shown(&o, Lang::En));
    let day = ritsu_ports::day_of("2026-11-16").unwrap();
    let ask = Ask { requester: Some("e1201".to_string()), every: false, form: Some(form.to_string()), on: On::Day(day) };
    let a = simulate(&o, &run, &ask).map_err(|t| t.en)?;
    Ok(a.sims[0].stages.iter().map(|s| (s.stage.clone(), s.shown.clone())).collect())
}

/// The cells of the table of the section `## <heading>` of a Markdown page, row by row.
fn table_of(page: &str, heading: &str) -> Vec<Vec<String>> {
    let at = page.find(&format!("\n## {heading}\n")).unwrap_or_else(|| panic!("no section {heading}"));
    let section = &page[at + 1..];
    let section = &section[..section[3..].find("\n## ").map_or(section.len(), |e| e + 3)];
    section
        .lines()
        .filter(|l| l.starts_with('|') && !l.starts_with("|---"))
        .skip(1)
        .map(|l| l.trim_matches('|').split(" | ").map(|c| c.trim().to_string()).collect())
        .collect()
}

/// The expense request's table, case by case: at both ends of each case's amounts the route
/// `simulate` gives is the one the case's row writes, and the cell of the page says its stages in
/// order; past the end of the range the form is refused, as the page says.
#[test]
fn the_route_table_of_a_page_is_what_simulate_gives() {
    let o = common::check("examples/expense.ringi");
    let file = o.file.as_ref().unwrap();
    let Some(Route::Table(t)) = &file.route else { panic!("the expense request routes by a table") };
    let rows = table_of(&page("examples/expense.ringi", &[]), "Route");
    assert_eq!(rows.len(), t.rows.len());
    // each case: its row, its category, and the least and the most of its amounts the form accepts
    let cases: [(usize, &str, i64, i64); 8] = [
        (0, "travel", 1, 30_000),
        (1, "travel", 30_001, 100_000),
        (2, "travel", 100_001, 10_000_000),
        (3, "expense", 1, 100_000),
        (4, "expense", 100_001, 1_000_000),
        (5, "expense", 1_000_001, 10_000_000),
        (6, "purchase", 1, 1_000_000),
        (7, "purchase", 1_000_001, 10_000_000),
    ];
    let form = |category: &str, amount: i64| {
        // rows of at most 1,000,000 JPY each, the most a row's price takes
        let mut items = Vec::new();
        let mut left = amount;
        while left > 0 {
            let p = left.min(1_000_000);
            items.push(format!("{{what: \"x\", price: {p}JPY}}"));
            left -= p;
        }
        format!("category = {category}, items = [{}]", items.join(", "))
    };
    for (row, category, least, most) in cases {
        let want: Vec<(String, String)> = t.stages.iter().zip(&t.rows[row].stages).filter(|(_, (sp, _))| !matches!(sp, Spec::Empty)).map(|(st, (sp, _))| (st.text.clone(), sp.shown())).collect();
        for amount in [least, most] {
            let got = simulated("examples/expense.ringi", None, &form(category, amount)).unwrap_or_else(|e| panic!("{category} {amount}: {e}"));
            assert_eq!(got, want, "{category} {amount} JPY, row {}", row + 1);
        }
        // the page's cell names the same stages, in order
        let cell = rows[row].last().unwrap();
        let mut from = 0;
        for (stage, _) in &want {
            let i = cell[from..].find(&format!("({stage})")).unwrap_or_else(|| panic!("row {}: `{stage}` is not in order in {cell:?}", row + 1));
            from += i + 1;
        }
        assert_eq!(cell.matches(" → ").count() + 1, want.len(), "row {}: {cell:?}", row + 1);
    }
    let refused = simulated("examples/expense.ringi", None, &form("travel", 10_000_001)).unwrap_err();
    assert!(refused.contains("outside its range"), "{refused}");
}

/// The requests whose stages come on conditions: on each side of each condition's boundary, and at
/// the ends of the range, the stages `simulate` gives are the ones the page's words say come; the
/// page says each boundary.
#[test]
fn the_conditions_of_a_page_are_what_simulate_gives() {
    let names = |v: &[(String, String)]| v.iter().map(|(s, _)| s.clone()).collect::<Vec<_>>().join(" ");
    let contract = |value: i64, kind: &str| format!("value = {value}JPY, kind = {kind}");
    let cases: Vec<(&str, Option<&str>, String, &str)> = vec![
        ("examples/contract.ringi", Some("2026-11-16"), contract(0, "nda"), "manager summary intake review"),
        ("examples/contract.ringi", Some("2026-11-16"), contract(5_000_000, "service"), "manager summary intake review"),
        ("examples/contract.ringi", Some("2026-11-16"), contract(5_000_001, "service"), "manager summary intake review second"),
        ("examples/contract.ringi", Some("2026-11-16"), contract(10_000_000, "service"), "manager summary intake review second"),
        ("examples/contract.ringi", Some("2026-11-16"), contract(10_000_001, "service"), "manager summary intake review second counsel"),
        ("examples/contract.ringi", Some("2026-11-16"), contract(1_000_000_000, "license"), "manager summary intake review second counsel finance"),
        ("examples/contract.ringi", Some("2026-11-16"), contract(0, "license"), "manager summary intake review finance"),
        ("examples/account.ringi", None, "level = write".to_string(), "manager owner operations"),
        ("examples/account.ringi", None, "level = admin".to_string(), "manager owner security operations"),
        ("examples/leave.ringi", None, "kind = sick, days = 1".to_string(), "manager"),
        ("examples/leave.ringi", None, "kind = sick, days = 5".to_string(), "manager"),
        ("examples/leave.ringi", None, "kind = sick, days = 6".to_string(), "manager head"),
        ("examples/leave.ringi", None, "kind = sick, days = 20".to_string(), "manager head"),
        ("examples/purchase.ringi", None, "kind = goods, lines = [{item: \"x\", quantity: 1, price: 100000JPY}], advance = 30%".to_string(), "first decide inform"),
        ("examples/purchase.ringi", None, "kind = goods, lines = [{item: \"x\", quantity: 1, price: 100000JPY}], advance = 35%".to_string(), "first finance decide inform"),
        ("examples/purchase.ringi", None, "kind = goods, lines = [{item: \"x\", quantity: 1, price: 100000JPY}], advance = 0%, reviewer = person \"e7201\"".to_string(), "first technical decide inform"),
    ];
    for (file, at, form, want) in &cases {
        let got = simulated(file, *at, form).unwrap_or_else(|e| panic!("{file} {form}: {e}"));
        assert_eq!(names(&got), *want, "{file}: {form}");
    }
    // the boundaries, as the pages say them
    let said = [
        ("examples/contract.ringi", &["--at", "2026-11-16"][..], "| second | at least 2 of legal | when “Contract value” is more than 5,000,000 JPY |"),
        ("examples/contract.ringi", &["--at", "2026-11-16"][..], "| counsel | counsel | when “Contract value” is more than 10,000,000 JPY |"),
        ("examples/contract.ringi", &["--at", "2026-11-16"][..], "| finance | circulated to finance | when “Kind of contract” is “License agreement” |"),
        ("examples/account.ringi", &[][..], "| security | security | when “Access level” is “Administrator” |"),
        ("examples/leave.ringi", &[][..], "| head | dept_head | when “Business days” is more than 5 |"),
        ("examples/purchase.ringi", &[][..], "| technical | reviewer | when “Technical reviewer” has a value |"),
        ("examples/purchase.ringi", &[][..], "| finance | finance | when the rule `authority` answers yes in `finance`, or “Paid in advance” is more than 30% |"),
    ];
    for (file, flags, line) in said {
        let p = page(file, flags);
        assert!(p.contains(line), "{file}: {line:?} is not in:\n{p}");
    }
}

// ── What the pages count, held to `check` and `api` ───────────────────────

/// The facts `check` writes under its `ok` line, in English.
fn facts(file: &str, at: Option<&str>) -> Vec<String> {
    let run = Run::with(common::joined(), Options { at: at.map(|d| ritsu_ports::day_of(d).unwrap()), ..Options::default() });
    let o = run.file(file).unwrap();
    o.facts.iter().map(|f| f.text.en.clone()).collect()
}

/// The days a request can be used on, the longest a ticket takes and the people who may submit it:
/// the numbers of `check`'s facts are the page's, and with no `submitted by` line, everyone of each
/// version of the organization's data; a field's range and count are `api`'s.
#[test]
fn what_a_page_counts_is_what_check_and_api_say() {
    // the people of each version, as the organization's page counts them
    let versions = table_of(&page("examples/shared/company.ringi", &[]), "Versions of the data");
    for (name, flags) in REQUESTS {
        for (file, lang) in [(format!("examples/{name}.ringi"), "en"), (format!("examples/{name}.ja.ringi"), "ja")] {
            let at = flags.get(1).copied();
            let p = page(&file, &[flags, &["--lang", "en"][..]].concat());
            let facts = facts(&file, at);
            let fact = |head: &str| facts.iter().find_map(|f| f.strip_prefix(head)).map(str::to_string);
            // the first day, and what decides it
            let from = fact("used from ").unwrap_or_else(|| panic!("{file}: {facts:?}"));
            assert!(p.contains(&format!("- From {from}.")), "{file}: {from:?} is not in:\n{p}");
            // the longest
            let ends = fact("ends within ").unwrap();
            assert!(p.contains(&format!("A ticket ends within {ends}.")), "{file}: {ends:?} is not in:\n{p}");
            let longest = page(&file, &[flags, &["--lang", "en", "--longest"][..]].concat());
            let total = ends.split_whitespace().next().unwrap();
            assert!(longest.contains(&format!("| In all | {total} |")), "{file}: {total}");
            // the people who may submit it, version by version
            let line = p.lines().find_map(|l| l.strip_prefix("In numbers: ")).unwrap_or_else(|| panic!("{file}:\n{p}"));
            match fact("submitted by ") {
                Some(by) => assert_eq!(Some(line), by.split(" (`").next().map(|b| format!("{b}.")).as_deref(), "{file}"),
                None => {
                    for part in line.trim_end_matches('.').split(", and ") {
                        let n: Vec<&str> = part.split(' ').collect();
                        let day = part.split(" from ").nth(1).unwrap();
                        let people = &versions.iter().find(|v| v[0].starts_with(day)).unwrap()[3];
                        assert_eq!((n[0], n[2]), (people.as_str(), people.as_str()), "{file}: {line}");
                    }
                }
            }
            // the ranges and counts of the fields
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let args: Vec<String> = ["api", file.as_str()].iter().chain(flags).map(|s| s.to_string()).collect();
            let code = ringi::run::run(&args, common::joined(), &mut out, &mut err);
            assert_eq!(code, 0, "{file}: {}", String::from_utf8_lossy(&err));
            let api = ritsu_base::json::parse(&String::from_utf8(out).unwrap()).unwrap();
            // the form as the page in the file's own language says it, whose labels `api` gives
            let own = page(&file, &[flags, &["--lang", lang][..]].concat());
            let form = table_of(&own, if lang == "ja" { "フォーム" } else { "Form" });
            let mut checked = 0;
            walk_fields(api.get("form").and_then(Json::as_arr).unwrap(), None, &form, &file, lang, &mut checked);
            assert!(checked > 0, "{file}");
        }
    }
}

/// [`what_a_page_counts_is_what_check_and_api_say`]: each field of `api`'s form, and the fields of
/// its rows, with what the page's form says it accepts.
fn walk_fields(fields: &[Json], parent: Option<&str>, form: &[Vec<String>], file: &str, lang: &str, checked: &mut usize) {
    for f in fields {
        let label = f.get("label").and_then(Json::as_str).unwrap().to_string();
        let label = match parent {
            Some(p) => format!("{p} › {label}"),
            None => label,
        };
        let row = form.iter().find(|r| r[0] == label).unwrap_or_else(|| panic!("{file} ({lang}): no row of {label:?} in {form:?}"));
        let accepts = &row[2];
        for key in ["min", "max", "count_min", "count_max"] {
            let Some(v) = f.get(key).filter(|v| !v.is_null()) else { continue };
            // a rate in steps is on the wire as a count of its steps (`rate[step 5%]`: 20 is 100%)
            let step = f.get("type").and_then(Json::as_str).and_then(|t| t.strip_prefix("rate[step ")).and_then(|t| t.strip_suffix("%]")).and_then(|t| t.parse::<i128>().ok());
            let shown = match (v, step) {
                (Json::Int(n), Some(step)) => format!("{}%", n * step),
                // a whole number of five digits or more grouped by commas, as the screens write it
                (Json::Int(n), None) if *n >= 10_000 => ritsu_base::text::count(*n as u64),
                (Json::Int(n), None) => n.to_string(),
                (Json::Str(s), _) => s.clone(),
                (other, _) => other.compact(),
            };
            // no count at the least is said as `up to`
            if key == "count_min" && shown == "0" {
                continue;
            }
            assert!(accepts.contains(&shown), "{file} ({lang}): {label} {key} {shown} is not in {accepts:?}");
            *checked += 1;
        }
        walk_fields(f.get("fields").and_then(Json::as_arr).unwrap_or(&[]), Some(&label), form, file, lang, checked);
    }
}

/// The page of a shared route lists the requests of the project that take it; with no project, it
/// says that each request names the route it takes.
#[test]
fn the_page_of_a_shared_route_lists_the_requests_that_take_it() {
    let p = page("examples/shared/manager_approval.ringi", &[]);
    let rows = table_of(&p, "The requests that take it");
    assert_eq!(rows, vec![vec!["Leave request (`leave_ja`)".to_string(), "`leave.ja.ringi`".to_string()], vec!["Leave request (`leave`)".to_string(), "`leave.ringi`".to_string()]]);
    // a directory with no `.git` and no `ritsu.mod` is no project, unless `--root` names it
    assert_eq!(ringi::doc::project_of(Path::new("examples"), false), None);
    assert!(ringi::doc::project_of(Path::new("examples"), true).is_some());
    let run = Run::new(common::joined());
    let o = run.file("examples/shared/manager_approval.ringi").unwrap();
    let ask = ringi::doc::Ask { longest: false, on: None, project: None };
    let alone = ringi::doc::write::markdown(&ringi::doc::page(&o, &run, "manager_approval.ringi", Lang::En, &ask).unwrap());
    assert!(alone.contains("This file does not say which requests take the route"), "{alone}");
}
