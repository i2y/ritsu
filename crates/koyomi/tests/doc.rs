//! `koyomi doc` (PLAN D.1, DESIGN 7): the page for the people who read a `.cal`.
//!
//! - Every example's page, in Markdown, in English and in Japanese, is its golden file in
//!   `tests/golden/doc/`. `KOYOMI_BLESS=1 cargo test --test doc` writes them again; read the
//!   diff.
//! - The HTML loads nothing from anywhere; it marks exactly the input days the check found a
//!   claim failing on; every holiday name on it is a row of the table the calendar read; and
//!   every sentence it says an operation with comes from `paraphrase.rs`.
//! - A file with an error other than a failing claim or example has no page.
//! - In Chrome (found by ritsu-testkit: `RITSU_CHROME` or `KOYOMI_CHROME`, else macOS's Google
//!   Chrome, else `google-chrome` or `chromium` on the PATH), the pages of the two examples that
//!   break a claim on purpose are drawn in the light and the dark palette; `KOYOMI_BLESS=1` keeps
//!   the pictures in `docs/images/` for the READMEs.

use koyomi::check::{Checked, check};
use ritsu_testkit::{Need, TempDir, chrome, need};
use koyomi::doc::{self, Format, Options};
use ritsu_base::text::Lang;
use std::collections::BTreeSet;
use std::process::Command;
use std::time::Duration;

const EXAMPLES: &[&str] = &[
    "examples/calendars/england_and_wales.cal",
    "examples/net30.cal",
    "examples/close_20th_pay_10th.cal",
    "examples/close_eom_pay_two_months_on.cal",
    "examples/close_and_pay_on_given_days.cal",
    "examples/period_of_months.cal",
    "examples/period_of_months_two_readings.cal",
    "examples/calendars/tokyo_business_days.cal",
    "examples/calendars/civil_code_142_days.cal",
    "examples/payment_20th_close_next_10th.cal",
    "examples/eom_close_two_months_later.cal",
    "examples/closing_and_payment_days_as_inputs.cal",
    "examples/civil_code_period_end.cal",
    "examples/civil_code_two_readings.cal",
    "examples/calendars/東京の営業日.cal",
    "examples/calendars/民法142条の休日.cal",
    "examples/payment_20th_close_next_10th.ja.cal",
    "examples/eom_close_two_months_later.ja.cal",
    "examples/closing_and_payment_days_as_inputs.ja.cal",
    "examples/civil_code_period_end.ja.cal",
    "examples/civil_code_two_readings.ja.cal",
];

fn page(path: &str, lang: Lang, f: Format) -> String {
    let o = check(path).unwrap();
    let p = doc::page(&o, lang, &Options::default()).unwrap_or_else(|ds| panic!("{path} has no page: {:?}", ds.iter().map(|d| d.code).collect::<Vec<_>>()));
    doc::render(&p, f)
}

fn stem(path: &str) -> String {
    std::path::Path::new(path).file_stem().unwrap().to_string_lossy().to_string()
}

#[test]
fn every_example_has_its_golden_page() {
    let mut failures = Vec::new();
    for p in EXAMPLES {
        for (lang, tag) in [(Lang::En, "en"), (Lang::Ja, "ja")] {
            let text = page(p, lang, Format::Markdown);
            let golden = format!("tests/golden/doc/{}.{tag}.md", stem(p));
            if let Err(e) = ritsu_testkit::golden::check(std::path::Path::new(&golden), &text) {
                failures.push(format!("{p} ({tag}): {e}"));
            }
        }
    }
    // No golden file is left without its example.
    for e in std::fs::read_dir("tests/golden/doc").unwrap() {
        let n = e.unwrap().file_name().to_string_lossy().to_string();
        let s = n.trim_end_matches(".en.md").trim_end_matches(".ja.md");
        assert!(EXAMPLES.iter().any(|p| stem(p) == s), "tests/golden/doc/{n} has no example");
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The values of `attr="…"` on every tag named `tag`.
fn attrs<'a>(html: &'a str, tag: &str, attr: &str) -> Vec<&'a str> {
    let open = format!("<{tag} ");
    let key = format!("{attr}=\"");
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(i) = rest.find(&open) {
        let t = &rest[i..];
        let end = t.find('>').unwrap();
        let head = &t[..end];
        if let Some(j) = head.find(&key) {
            let v = &head[j + key.len()..];
            out.push(&v[..v.find('"').unwrap()]);
        }
        rest = &t[end..];
    }
    out
}

/// The texts of every `<span class="<class>">…</span>`.
fn spans(html: &str, class: &str) -> Vec<String> {
    let open = format!("<span class=\"{class}\">");
    html.split(&open).skip(1).map(|s| unescape(&s[..s.find("</span>").unwrap()])).collect()
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&amp;", "&")
}

#[test]
fn the_html_loads_nothing_and_shows_what_the_check_found() {
    let mut looked = 0;
    for p in EXAMPLES {
        let o = check(p).unwrap();
        for lang in [Lang::En, Lang::Ja] {
            let html = page(p, lang, Format::Html);
            // One file: no script, no stylesheet, no font, no picture from anywhere.
            for bad in ["<script", "<link", " src=", "@import", "url(", "<img", "<iframe"] {
                assert!(!html.contains(bad), "{p}: the HTML has {bad}");
            }
            assert!(html.starts_with("<!doctype html>\n") && html.ends_with("</html>\n"), "{p}");
            let cells = attrs(&html, "td", "class");
            let fails = cells.iter().filter(|c| c.split(' ').any(|x| x == "fail")).count();
            let mut want: BTreeSet<i32> = BTreeSet::new();
            let mut names: BTreeSet<String> = BTreeSet::new();
            let mut sentences: BTreeSet<String> = BTreeSet::new();
            match o.checked.as_ref().unwrap() {
                Checked::Dates(m, rep) => {
                    for cr in &rep.claims {
                        for (_, a, b) in &cr.runs {
                            want.extend(a.0..=b.0);
                        }
                    }
                    if let Some(c) = &m.cal {
                        names.extend(c.tables.iter().flat_map(|t| t.rows.iter().map(|r| r.name.clone())));
                        for d in &m.dates {
                            if let Some((at, _)) = d.at {
                                sentences.insert(koyomi::paraphrase::at_sentence(at, c.offset_text().as_deref()).get(lang).to_string());
                            }
                        }
                    }
                    for d in &m.dates {
                        for op in &d.ops {
                            sentences.insert(koyomi::paraphrase::sentence(m, &op.op).get(lang).to_string());
                        }
                    }
                }
                Checked::Calendar(c) => names.extend(c.tables.iter().flat_map(|t| t.rows.iter().map(|r| r.name.clone()))),
            }
            assert_eq!(fails, want.len(), "{p} ({lang:?}): the days marked failing are not the days the check found failing");
            for n in spans(&html, "hol") {
                assert!(names.contains(&n), "{p}: the holiday {n} is not a row of the table");
            }
            let said = spans(&html, "say");
            if matches!(o.checked, Some(Checked::Dates(..))) {
                assert!(!said.is_empty(), "{p}: no operation is said in words");
            }
            for s in said {
                let lower: String = s.chars().take(1).flat_map(char::to_lowercase).chain(s.chars().skip(1)).collect();
                assert!(sentences.contains(&s) || sentences.contains(&lower), "{p}: {s:?} is not a sentence of paraphrase.rs");
            }
            looked += 1;
        }
    }
    assert_eq!(looked, EXAMPLES.len() * 2);
    // The examples that break a claim on purpose are marked where the check says.
    let html = page("examples/eom_close_two_months_later.ja.cal", Lang::Ja, Format::Html);
    assert_eq!(attrs(&html, "td", "class").iter().filter(|c| c.contains("fail")).count(), 648);
    assert!(html.contains("data-day=\"2026-05-03\" title=\"2026-05-03（日）\n休み（日曜、憲法記念日）\n条件「受領から60日以内」が成り立たない\""), "the title of a failing holiday");
}

fn koyomi(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_koyomi")).args(args).env_remove("KOYOMI_LANG").env_remove("RITSU_LANG").output().unwrap()
}

#[test]
fn the_command() {
    // A page on standard output, Markdown unless asked otherwise.
    let o = koyomi(&["doc", "examples/payment_20th_close_next_10th.ja.cal"]);
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(String::from_utf8(o.stdout).unwrap(), page("examples/payment_20th_close_next_10th.ja.cal", Lang::En, Format::Markdown));
    let o = koyomi(&["doc", "examples/payment_20th_close_next_10th.ja.cal", "--format", "html", "--lang", "ja"]);
    assert_eq!(String::from_utf8(o.stdout).unwrap(), page("examples/payment_20th_close_next_10th.ja.cal", Lang::Ja, Format::Html));
    // A claim that fails: the page shows it, and doc has done its job.
    let o = koyomi(&["doc", "examples/eom_close_two_months_later.ja.cal"]);
    assert_eq!(o.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&o.stdout).contains("> [!WARNING]"));
    // Any other error: no page, the diagnostics on standard error.
    for (f, code) in [("tests/mutants/E201_無い日の扱いが無い.cal", "E201"), ("tests/mutants/E203_表の外.cal", "E203"), ("tests/mutants/E101_写しが無い.cal", "E101")] {
        let o = koyomi(&["doc", f]);
        assert_eq!(o.status.code(), Some(1), "{f}");
        assert!(o.stdout.is_empty(), "{f} has no page");
        assert!(String::from_utf8_lossy(&o.stderr).contains(&format!("error[{code}]")), "{f}");
    }
    // A page for a failing example row too.
    assert_eq!(koyomi(&["doc", "tests/mutants/E303_例が違う.cal"]).status.code(), Some(0));
    // The months.
    let o = koyomi(&["doc", "examples/calendars/東京の営業日.cal", "--months", "2026-04..2027-03", "--lang", "ja"]);
    let md = String::from_utf8(o.stdout).unwrap();
    assert!(md.contains("**2026 年 4 月**") && md.contains("**2027 年 3 月**") && !md.contains("**2026 年 3 月**") && !md.contains("**2027 年 4 月**"));
    for bad in ["2026-13..2027-01", "2027-01..2026-12", "2026-1..2026-12", "2026-01", "2026-01..2026-12x"] {
        let o = koyomi(&["doc", "examples/calendars/東京の営業日.cal", "--months", bad]);
        assert_eq!(o.status.code(), Some(2), "--months {bad}");
    }
    assert_eq!(koyomi(&["doc", "examples/net30.cal", "--format", "pdf"]).status.code(), Some(2));
    assert_eq!(koyomi(&["doc"]).status.code(), Some(2));
    assert_eq!(koyomi(&["doc", "examples/net30.cal", "examples/net30.cal"]).status.code(), Some(2));
    // A calendar without a table knows every day, and shows months only when asked.
    let o = koyomi(&["doc", "tests/fixtures/calendars/土日.cal"]);
    assert_eq!(o.status.code(), Some(0));
    let md = String::from_utf8(o.stdout).unwrap();
    assert!(md.contains("--months 2026-01..2026-12") && !md.contains("| Mon |"), "{md}");
    let o = koyomi(&["doc", "tests/fixtures/calendars/土日.cal", "--months", "2026-10..2026-10"]);
    assert!(String::from_utf8(o.stdout).unwrap().contains("**October 2026**"));
}

#[test]
fn what_the_pages_say() {
    // DESIGN 7: the calendar page counts the business days and finds the longest run of
    // closed days in the months it shows.
    let md = page("examples/calendars/東京の営業日.cal", Lang::Ja, Format::Markdown);
    assert!(md.contains("| 2026 | 240 | 125 |"), "{md}");
    assert!(md.contains("2026-01〜2027-12 でいちばん長い連休は 2026-12-29〜2027-01-03 の 6 日です。"));
    // The article a date cites, quoted from its copy, with the version.
    let md = page("examples/civil_code_period_end.ja.cal", Lang::Ja, Format::Markdown);
    assert!(md.contains("> **民法 第143条（e-Gov 法令検索、2026-10-01 時点、版 129AC0000000089_20260624_508AC0000000045）**"));
    assert!(md.contains("> ２　週、月又は年の初めから期間を起算しないときは、その期間は、最後の週、月又は年においてその起算日に応当する日の前日に満了する。ただし、月又は年によって期間を定めた場合において、最後の月に応当する日がないときは、その月の末日に満了する。"));
    // The other ways of handling a missing day, counted on every input (DESIGN 1.7).
    let md = page("examples/civil_code_period_end.ja.cal", Lang::En, Format::Markdown);
    assert!(md.contains("With `else end_of_month` instead, 満了日 would differ on 57 of the 4,380 input combinations."), "{md}");
    // net30 runs over 36 months; only those with an edge case are shown.
    let md = page("examples/net30.cal", Lang::En, Format::Markdown);
    assert!(md.contains("fall in the 36 months of 2026-01..2028-12"), "{md}");
}

/// Chrome: `KOYOMI_CHROME`, else where macOS installs Google Chrome, else a Chrome or a Chromium
/// on the PATH (the order dandori and chobo look in).
/// What the READMEs show: the top of a page, in the light palette, and its month tables, in
/// the dark one. The page is the one `koyomi doc --format html` writes, with `data-theme` on
/// its root to choose the palette and, for the month tables, everything but them hidden.
struct Shot {
    path: &'static str,
    lang: Lang,
    name: &'static str,
    theme: &'static str,
    size: (u32, u32),
    css: &'static str,
}

/// What hides everything of a page but its month tables.
const MONTHS_ONLY: &str = "header,.alert,section:not(:last-of-type),section:last-of-type>h2,section:last-of-type>p,section:last-of-type>ul:not(.legend),section:last-of-type>h3:not(:last-of-type){display:none}";

const SHOTS: &[Shot] = &[
    Shot { path: "examples/close_eom_pay_two_months_on.cal", lang: Lang::En, name: "doc-top.en.png", theme: "light", size: (1100, 1250), css: "" },
    Shot { path: "examples/close_eom_pay_two_months_on.cal", lang: Lang::En, name: "doc-months.en.png", theme: "dark", size: (1100, 1350), css: MONTHS_ONLY },
    Shot { path: "examples/eom_close_two_months_later.ja.cal", lang: Lang::Ja, name: "doc-top.ja.png", theme: "light", size: (1100, 1250), css: "" },
    Shot { path: "examples/eom_close_two_months_later.ja.cal", lang: Lang::Ja, name: "doc-months.ja.png", theme: "dark", size: (1100, 1350), css: MONTHS_ONLY },
];

#[test]
fn the_pages_in_chrome() {
    if !need(Need::Chrome) {
        return;
    }
    let Some(chrome) = chrome::find() else {
        ritsu_testkit::skip("Chrome is not found; set RITSU_CHROME (or KOYOMI_CHROME) to draw the pages");
        return;
    };
    let bless = ritsu_testkit::golden::bless();
    let dir = TempDir::new("doc-chrome");
    let mut sizes = Vec::new();
    std::thread::scope(|s| {
        let hs: Vec<_> = SHOTS
            .iter()
            .enumerate()
            .map(|(i, Shot { path: p, lang, name, theme, size: (w, h), css })| {
                let chrome = chrome.clone();
                let dir = dir.path().to_path_buf();
                s.spawn(move || {
                    let html = page(p, *lang, Format::Html)
                        .replacen(&format!("<html lang=\"{}\">", lang.code()), &format!("<html lang=\"{}\" data-theme=\"{theme}\">", lang.code()), 1)
                        .replacen("</style>", &format!("{css}</style>"), 1);
                    let at = dir.join(format!("{i}.html"));
                    std::fs::write(&at, html).unwrap();
                    let png = dir.join(name);
                    let drawn = chrome::screenshot(&chrome, &format!("file://{}", at.display()), &png, *w, *h, Duration::from_secs(60));
                    let bytes = std::fs::read(&png).unwrap_or_else(|_| panic!("Chrome drew no picture of {p}: {drawn:?}"));
                    (*name, bytes)
                })
            })
            .collect();
        for h in hs {
            sizes.push(h.join().unwrap());
        }
    });
    for ((name, bytes), Shot { size: (w, h), .. }) in sizes.iter().zip(SHOTS) {
        assert_eq!(chrome::png_size(bytes), Some((*w, *h)), "{name} is a {w}x{h} PNG");
        assert!(bytes.len() > 50_000, "{name} is {} bytes: did the page draw?", bytes.len());
        if bless {
            std::fs::create_dir_all("docs/images").unwrap();
            std::fs::write(format!("docs/images/{name}"), bytes).unwrap();
        }
        println!("drawn: {name} ({} bytes)", bytes.len());
    }
    // The pictures the READMEs show are there.
    for Shot { name, .. } in SHOTS {
        assert!(std::path::Path::new(&format!("docs/images/{name}")).is_file(), "docs/images/{name} is missing; KOYOMI_BLESS=1 cargo test --test doc draws it");
    }
}

/// What `the_html_loads_nothing_and_shows_what_the_check_found` checks at its end, in English: the
/// examples that break a claim on purpose are marked where the check says, and a failing holiday
/// says why it is closed.
#[test]
fn the_html_marks_what_the_check_found_in_english() {
    let html = page("examples/eom_close_two_months_later.cal", Lang::En, Format::Html);
    assert_eq!(attrs(&html, "td", "class").iter().filter(|c| c.contains("fail")).count(), 648);
    assert!(html.contains("data-day=\"2026-05-03\" title=\"2026-05-03 Sun\nclosed (Sunday, 憲法記念日)\nThe claim within_60_days_of_receipt fails\""), "the title of a failing holiday");
    let html = page("examples/close_eom_pay_two_months_on.cal", Lang::En, Format::Html);
    assert_eq!(attrs(&html, "td", "class").iter().filter(|c| c.contains("fail")).count(), 1008);
    assert!(html.contains("data-day=\"2026-05-04\" title=\"2026-05-04 Mon\nclosed (Early May bank holiday)\nThe claim within_60_days_of_receipt fails\""), "the title of a failing bank holiday");
}

#[test]
fn the_command_in_english() {
    let o = koyomi(&["doc", "examples/payment_20th_close_next_10th.cal"]);
    assert_eq!(o.status.code(), Some(0));
    assert_eq!(String::from_utf8(o.stdout).unwrap(), page("examples/payment_20th_close_next_10th.cal", Lang::En, Format::Markdown));
    let o = koyomi(&["doc", "examples/close_20th_pay_10th.cal", "--format", "html", "--lang", "ja"]);
    assert_eq!(String::from_utf8(o.stdout).unwrap(), page("examples/close_20th_pay_10th.cal", Lang::Ja, Format::Html));
    let o = koyomi(&["doc", "examples/close_eom_pay_two_months_on.cal"]);
    assert_eq!(o.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&o.stdout).contains("> [!WARNING]"));
    for (f, code) in [("tests/mutants/E201_no_way_for_a_missing_day.cal", "E201"), ("tests/mutants/E203_past_the_table.cal", "E203"), ("tests/mutants/E101_no_copy.cal", "E101")] {
        let o = koyomi(&["doc", f]);
        assert_eq!(o.status.code(), Some(1), "{f}");
        assert!(o.stdout.is_empty(), "{f} has no page");
        assert!(String::from_utf8_lossy(&o.stderr).contains(&format!("error[{code}]")), "{f}");
    }
    assert_eq!(koyomi(&["doc", "tests/mutants/E303_example_differs.cal"]).status.code(), Some(0));
    let o = koyomi(&["doc", "examples/calendars/england_and_wales.cal", "--months", "2027-04..2028-03"]);
    let md = String::from_utf8(o.stdout).unwrap();
    assert!(md.contains("**April 2027**") && md.contains("**March 2028**") && !md.contains("**March 2027**") && !md.contains("**April 2028**"));
    let o = koyomi(&["doc", "examples/calendars/tokyo_business_days.cal", "--months", "2026-04..2027-03"]);
    let md = String::from_utf8(o.stdout).unwrap();
    assert!(md.contains("**April 2026**") && md.contains("**March 2027**") && !md.contains("**March 2026**") && !md.contains("**April 2027**"));
    for bad in ["2026-13..2027-01", "2027-01..2026-12", "2026-1..2026-12", "2026-01", "2026-01..2026-12x"] {
        let o = koyomi(&["doc", "examples/calendars/england_and_wales.cal", "--months", bad]);
        assert_eq!(o.status.code(), Some(2), "--months {bad}");
    }
    let o = koyomi(&["doc", "tests/fixtures/calendars/weekends.cal"]);
    assert_eq!(o.status.code(), Some(0));
    let md = String::from_utf8(o.stdout).unwrap();
    assert!(md.contains("--months 2026-01..2026-12") && !md.contains("| Mon |"), "{md}");
    let o = koyomi(&["doc", "tests/fixtures/calendars/weekends.cal", "--months", "2026-10..2026-10"]);
    assert!(String::from_utf8(o.stdout).unwrap().contains("**October 2026**"));
}

#[test]
fn what_the_pages_say_in_english() {
    let md = page("examples/calendars/tokyo_business_days.cal", Lang::En, Format::Markdown);
    assert!(md.contains("| 2026 | 240 | 125 |"), "{md}");
    assert!(md.contains("The longest run of closed days in 2026-01..2027-12 is 2026-12-29..2027-01-03, 6 days."), "{md}");
    let md = page("examples/calendars/england_and_wales.cal", Lang::En, Format::Markdown);
    assert!(md.contains("| 2027 | 253 | 112 |"), "{md}");
    assert!(md.contains("The longest run of closed days in 2027-01..2028-12 is 2027-03-26..2027-03-29, 4 days."), "{md}");
    // The article a date cites, quoted from its copy, with the version: in Japanese, as e-Gov
    // serves it, under the English name of its source.
    let md = page("examples/civil_code_period_end.cal", Lang::En, Format::Markdown);
    assert!(md.contains("> **civil_code 第143条 (e-Gov, as of 2026-10-01, revision 129AC0000000089_20260624_508AC0000000045)**"), "{md}");
    assert!(md.contains("> ２　週、月又は年の初めから期間を起算しないときは、その期間は、最後の週、月又は年においてその起算日に応当する日の前日に満了する。"));
    assert!(md.contains("With `else end_of_month` instead, last_day would differ on 57 of the 4,380 input combinations."), "{md}");
    let md = page("examples/period_of_months.cal", Lang::En, Format::Markdown);
    assert!(md.contains("With `else end_of_month` instead, last_day would differ on 57 of the 4,380 input combinations."), "{md}");
}
