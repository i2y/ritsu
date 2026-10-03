//! The page as one HTML file: nothing is loaded from anywhere else (no script, no font, no
//! stylesheet), it has a light and a dark palette (`prefers-color-scheme`, or
//! `data-theme="light|dark"` on the root to choose one), and it prints. The frame — the head, the
//! palette's variables and how a page chooses between them — is ritsu-base's
//! ([`ritsu_base::docpage`]); the colours koyomi adds and the rules are koyomi's.

use super::{Block, Inline, Item, Legend, Page, Para, Quote};
use crate::diag::Step;
use ritsu_base::text::{capitalize, ja_spacing};
use crate::doc::months::{Cell, Grid, Named};
use ritsu_base::docpage::{self, Palette, esc};
use ritsu_base::text::{Lang, Text};

/// Words, in which what is between backticks is code, as in the diagnostics.
fn words(s: &str, lang: Lang) -> String {
    let s = if lang == Lang::Ja { ja_spacing(s) } else { s.to_string() };
    let mut o = String::new();
    for (i, part) in s.split('`').enumerate() {
        if i % 2 == 1 {
            o.push_str(&format!("<code>{}</code>", esc(part)));
        } else {
            o.push_str(&esc(part));
        }
    }
    o
}

fn inline(p: &Para, lang: Lang) -> String {
    let mut o = String::new();
    for (i, x) in p.iter().enumerate() {
        match x {
            Inline::T(s) => o.push_str(&words(s, lang)),
            Inline::Say(s) => {
                let s = if i == 0 && lang == Lang::En { capitalize(s) } else { s.clone() };
                o.push_str(&format!("<span class=\"say\">{}</span>", esc(&s)));
            }
            Inline::C(s) => o.push_str(&format!("<code>{}</code>", esc(s))),
            Inline::B(s) => o.push_str(&format!("<strong>{}</strong>", esc(s))),
            Inline::A(s, href) => o.push_str(&format!("<a href=\"{}\">{}</a>", esc(href), esc(s))),
        }
    }
    o
}

fn quote(q: &Quote) -> String {
    let mut o = format!("<blockquote class=\"law\">\n<p class=\"cite\">{}</p>\n", esc(&q.head));
    for l in &q.lines {
        o.push_str(&format!("<p>{}</p>\n", esc(l)));
    }
    o.push_str("</blockquote>\n");
    o
}

fn items(list: &[Item], tag: &str, lang: Lang) -> String {
    let mut o = format!("<{tag}>\n");
    for it in list {
        o.push_str(&format!("<li><p>{}</p>", inline(&it.body, lang)));
        for p in &it.more {
            o.push_str(&format!("<p class=\"more\">{}</p>", inline(p, lang)));
        }
        for q in &it.quotes {
            o.push_str(&quote(q));
        }
        o.push_str("</li>\n");
    }
    o.push_str(&format!("</{tag}>\n"));
    o
}

const WEEK_JA: [&str; 7] = ["月", "火", "水", "木", "金", "土", "日"];
const WEEK_EN: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const MONTH_EN: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

/// What a day cell says when the mouse is on it.
fn title_of(c: &Cell, lang: Lang) -> String {
    let mut parts = vec![crate::diag::day_with_weekday(c.day, lang)];
    if !c.known {
        parts.push(Text::get(&tr!("カレンダーが知らない日", "a day the calendar does not know"), lang).to_string());
    } else if c.closed {
        let r = Text::join(&c.reasons, "、", ", ");
        parts.push(tr!("休み（{}）", "closed ({})", r.ja; r.en).get(lang).to_string());
    } else if c.opened {
        let n: Vec<&str> = c.names.iter().map(Named::text).collect();
        parts.push(tr!("営業日（{}）", "open ({})", n.join("、"); n.join(", ")).get(lang).to_string());
    }
    for f in &c.fails {
        parts.push(capitalize(f.get(lang)));
    }
    if !c.edges.is_empty() {
        let e = Text::join(&c.edges, "。", "; ");
        parts.push(tr!("エッジケース: {}", "Edge case: {}", e.ja; e.en).get(lang).to_string());
    }
    parts.join("\n")
}

fn month(g: &Grid, lang: Lang) -> String {
    let title = match lang {
        Lang::Ja => format!("{} 年 {} 月", g.ym.y, g.ym.m),
        Lang::En => format!("{} {}", MONTH_EN[(g.ym.m - 1) as usize], g.ym.y),
    };
    let mut o = format!("<figure class=\"month\" id=\"m{}\">\n<figcaption>{}</figcaption>\n<table class=\"cal\">\n<thead><tr>", g.ym, esc(&title));
    let week = if lang == Lang::Ja { WEEK_JA } else { WEEK_EN };
    for (i, w) in week.iter().enumerate() {
        let class = if i >= 5 { " class=\"we\"" } else { "" };
        o.push_str(&format!("<th{class}>{w}</th>"));
    }
    o.push_str("</tr></thead>\n<tbody>\n");
    for w in &g.weeks {
        o.push_str("<tr>");
        for c in w {
            let Some(c) = c else {
                o.push_str("<td class=\"pad\"></td>");
                continue;
            };
            let mut class = vec!["day"];
            if !c.known {
                class.push("unknown");
            } else if c.closed {
                class.push("closed");
            }
            if c.opened {
                class.push("opened");
            }
            if !c.fails.is_empty() {
                class.push("fail");
            }
            if !c.edges.is_empty() {
                class.push("edge");
            }
            let (_, _, d) = c.day.ymd();
            o.push_str(&format!("<td class=\"{}\" data-day=\"{}\" title=\"{}\">", class.join(" "), c.day, esc(&title_of(c, lang))));
            o.push_str(&format!("<span class=\"n\">{d}</span>"));
            if !c.edges.is_empty() {
                o.push_str("<span class=\"mark\" aria-hidden=\"true\">◆</span>");
            }
            if !c.known {
                o.push_str("<span class=\"mark q\" aria-hidden=\"true\">?</span>");
            }
            for n in &c.names {
                let cls = match n {
                    Named::Holiday(_) => "hol",
                    Named::Rule(_) => "rule",
                };
                o.push_str(&format!("<span class=\"{cls}\">{}</span>", esc(n.text())));
            }
            o.push_str("</td>");
        }
        o.push_str("</tr>\n");
    }
    o.push_str("</tbody>\n</table>\n</figure>\n");
    o
}

/// A computation as a table: the date, its day, and what the line did.
fn trace(steps: &[Step], lang: Lang) -> String {
    let fix = |t: &Text| if lang == Lang::Ja { ja_spacing(&t.ja) } else { t.en.clone() };
    let mut o = String::from("<div class=\"scroll\"><table class=\"trace\">\n<tbody>\n");
    for s in steps {
        match s {
            Step::Value { name, day, label, note, .. } => {
                let d = day.map(|d| crate::diag::day_with_weekday(d, lang)).unwrap_or_default();
                let mut what = fix(label);
                if let Some(n) = note {
                    let n = fix(n);
                    what = if what.is_empty() { n } else { format!("{what}: {n}") };
                }
                o.push_str(&format!("<tr><th>{}</th><td class=\"d\">{}</td><td>{}</td></tr>\n", esc(name), esc(&d), words(&what, Lang::En)));
            }
            Step::Say { text, .. } => {
                o.push_str(&format!("<tr><th></th><td colspan=\"2\" class=\"say-row\">{}</td></tr>\n", words(&fix(text), Lang::En)));
            }
        }
    }
    o.push_str("</tbody>\n</table></div>\n");
    o
}

fn legend(l: &Legend, lang: Lang) -> String {
    let mut o = String::from("<ul class=\"legend\">");
    let item = |sw: &str, mark: &str, t: Text| format!("<li><span class=\"sw {sw}\">{mark}</span>{}</li>", esc(t.get(lang)));
    if l.closed {
        o.push_str(&item("closed", "", tr!("休み", "closed")));
    }
    if l.fails {
        o.push_str(&item("fail", "", tr!("条件が成り立たない入力の日", "an input day a claim fails on")));
    }
    if l.edges {
        o.push_str(&item("edge", "◆", tr!("エッジケースの入力の日", "the input day of an edge case")));
    }
    if l.unknown {
        o.push_str(&item("unknown", "?", tr!("カレンダーが知らない日", "a day the calendar does not know")));
    }
    o.push_str("</ul>\n");
    o
}

pub fn render(p: &Page) -> String {
    let lang = p.lang;
    let css = format!("{}{RULES}", palette().css());
    let mut o = docpage::html_head(lang, &format!("koyomi {}", crate::api::VERSION), &p.title, &css);
    o.push_str(&format!("<main>\n<header>\n<h1>{}</h1>\n", esc(&p.title)));
    if let Some(d) = &p.description {
        o.push_str(&format!("<p class=\"desc\">{}</p>\n", esc(d)));
    }
    let mut in_header = true;
    let mut open_section = false;
    for b in &p.blocks {
        // The header holds the stamp and what follows it, up to the verdict.
        if in_header && !matches!(b, Block::Stamp(_) | Block::P(_)) {
            o.push_str("</header>\n");
            in_header = false;
        }
        match b {
            Block::H2(s) => {
                if open_section {
                    o.push_str("</section>\n");
                }
                o.push_str(&format!("<section>\n<h2>{}</h2>\n", esc(s)));
                open_section = true;
            }
            Block::H3(s) => o.push_str(&format!("<h3>{}</h3>\n", esc(s))),
            Block::P(para) => {
                let class = if in_header { " class=\"made\"" } else { "" };
                o.push_str(&format!("<p{class}>{}</p>\n", inline(para, lang)));
            }
            Block::Alert { ok, lines } => {
                o.push_str(&format!("<div class=\"alert {}\" role=\"status\">\n", if *ok { "ok" } else { "warn" }));
                for l in lines {
                    o.push_str(&format!("<p>{}</p>\n", inline(l, lang)));
                }
                o.push_str("</div>\n");
            }
            Block::Ol(list) => o.push_str(&items(list, "ol", lang)),
            Block::Ul(list) => o.push_str(&items(list, "ul", lang)),
            Block::Table { head, rows } => {
                o.push_str("<div class=\"scroll\"><table class=\"data\">\n<thead><tr>");
                for h in head {
                    o.push_str(&format!("<th>{}</th>", esc(h)));
                }
                o.push_str("</tr></thead>\n<tbody>\n");
                for r in rows {
                    o.push_str("<tr>");
                    for cell in r {
                        o.push_str(&format!("<td>{}</td>", inline(cell, lang)));
                    }
                    o.push_str("</tr>\n");
                }
                o.push_str("</tbody>\n</table></div>\n");
            }
            Block::Trace(steps) => o.push_str(&trace(steps, lang)),
            Block::Quote(q) => o.push_str(&quote(q)),
            Block::Stamp(rows) => {
                o.push_str("<dl class=\"stamp\">\n");
                for (k, v) in rows {
                    o.push_str(&format!("<dt>{}</dt><dd>{}</dd>\n", esc(k), inline(v, lang)));
                }
                o.push_str("</dl>\n");
            }
            Block::Months { grids, legend: l } => {
                o.push_str(&legend(l, lang));
                o.push_str("<div class=\"months\">\n");
                for g in grids {
                    o.push_str(&month(g, lang));
                }
                o.push_str("</div>\n");
            }
        }
    }
    if in_header {
        o.push_str("</header>\n");
    }
    if open_section {
        o.push_str("</section>\n");
    }
    o.push_str("</main>\n</body>\n</html>\n");
    o
}

/// The colours of the page: ritsu-base's palette, and koyomi's own after it — the background of
/// a quote, a closed day and its number, a day an input fails on (alone and closed), the mark of
/// an edge case, and a day the calendar does not know.
fn palette() -> Palette {
    Palette::default()
        .set("quote", "#f4f3ef", "#1c1f24")
        .set("closed", "#ecebe6", "#24272d")
        .set("closed-fg", "#b42318", "#ff8f80")
        .set("fail-bg", "#fff0c2", "#45330f")
        .set("fail-closed", "#f3dca0", "#5a4316")
        .set("fail", "#c4620c", "#f2b14c")
        .set("edge", "#0b63ce", "#7cb6ff")
        .set("unknown", "#98a2b3", "#6b7480")
}

/// The rules, after the palette's variables.
const RULES: &str = r#"* { box-sizing: border-box; }
body {
  margin: 0; background: var(--bg); color: var(--fg);
  font-family: system-ui, -apple-system, "Segoe UI", "Hiragino Sans", "Hiragino Kaku Gothic ProN", "Noto Sans JP", "Yu Gothic UI", Meiryo, sans-serif;
  font-size: 16px; line-height: 1.65;
}
main { max-width: 68rem; margin: 0 auto; padding: 2.5rem 1.25rem 4rem; }
header { border-bottom: 1px solid var(--line); padding-bottom: 1rem; margin-bottom: 1.5rem; }
h1 { font-size: 1.9rem; line-height: 1.3; margin: 0 0 .5rem; letter-spacing: .01em; }
h2 { font-size: 1.35rem; margin: 2.5rem 0 .75rem; padding-bottom: .3rem; border-bottom: 1px solid var(--line); }
h3 { font-size: 1.08rem; margin: 1.75rem 0 .5rem; }
p { margin: .5rem 0; }
.desc { font-size: 1.05rem; margin: 0 0 1rem; }
.made { color: var(--dim); font-size: .88rem; }
code, pre { font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, "Noto Sans Mono CJK JP", monospace; }
code { background: var(--code); border-radius: 4px; padding: .05em .35em; font-size: .88em; white-space: pre-wrap; }
pre { background: var(--code); border-radius: 6px; padding: .75rem 1rem; overflow-x: auto; font-size: .85rem; line-height: 1.5; }
dl.stamp { display: grid; grid-template-columns: max-content 1fr; gap: .15rem 1rem; margin: 0 0 .75rem; font-size: .88rem; color: var(--dim); }
dl.stamp dt { font-weight: 600; }
dl.stamp dd { margin: 0; overflow-wrap: anywhere; }
.alert { margin: 1.25rem 0 1.5rem; padding: .75rem 1rem; border-left: 4px solid var(--ok); background: var(--panel); border-radius: 0 6px 6px 0; }
.alert.warn { border-left-color: var(--warn); }
.alert p { margin: .25rem 0; }
ol, ul { padding-left: 1.5rem; }
li { margin: .35rem 0; }
li > p { margin: .15rem 0; }
li .more { color: var(--dim); font-size: .93rem; }
.say { font-weight: 600; }
.scroll { overflow-x: auto; }
table.data { border-collapse: collapse; margin: .5rem 0 1rem; font-size: .92rem; }
table.data th, table.data td { border-bottom: 1px solid var(--line); padding: .4rem .7rem; text-align: left; vertical-align: top; }
table.data th { background: var(--soft); font-weight: 600; white-space: nowrap; }
table.data td:not(:last-child), table.data td:not(:last-child) code { white-space: nowrap; }
table.trace { border-collapse: collapse; margin: .25rem 0 1rem; font-size: .88rem; background: var(--code); border-radius: 6px; }
table.trace th, table.trace td { padding: .2rem .7rem; text-align: left; vertical-align: top; }
table.trace th { font-weight: 600; white-space: nowrap; }
table.trace td.d { white-space: nowrap; font-variant-numeric: tabular-nums; }
table.trace td.say-row { color: var(--dim); }
blockquote.law { margin: .5rem 0 1rem; padding: .5rem 1rem; border-left: 3px solid var(--line); background: var(--quote); border-radius: 0 6px 6px 0; }
blockquote.law p { margin: .2rem 0; }
blockquote.law .cite { color: var(--dim); font-size: .85rem; font-weight: 600; }
ul.legend { list-style: none; padding: 0; margin: .75rem 0 1rem; display: flex; flex-wrap: wrap; gap: .5rem 1.25rem; font-size: .88rem; color: var(--dim); }
ul.legend li { margin: 0; display: flex; align-items: center; gap: .4rem; }
.sw { display: inline-flex; align-items: center; justify-content: center; width: 1.3rem; height: 1.1rem; border: 1px solid var(--line); border-radius: 3px; font-size: .7rem; }
.sw.closed { background: var(--closed); }
.sw.fail { background: var(--fail-bg); box-shadow: inset 0 0 0 1.5px var(--fail); }
.sw.edge { color: var(--edge); }
.sw.unknown { color: var(--unknown); border-style: dashed; }
.months { display: grid; grid-template-columns: repeat(auto-fill, minmax(17.5rem, 1fr)); gap: 1.25rem 1.5rem; margin: 1rem 0; }
figure.month { margin: 0; }
figure.month figcaption { font-weight: 700; margin-bottom: .35rem; }
table.cal { width: 100%; border-collapse: collapse; table-layout: fixed; font-size: .82rem; }
table.cal th { font-weight: 600; color: var(--dim); padding: .15rem 0; font-size: .75rem; }
table.cal th.we { color: var(--closed-fg); }
table.cal td { height: 3.2rem; vertical-align: top; padding: 2px 4px; border: 1px solid var(--line); position: relative; }
table.cal td.pad { border: none; }
td.day .n { font-variant-numeric: tabular-nums; }
td.closed { background: var(--closed); }
td.closed .n { color: var(--closed-fg); }
td.opened .n { text-decoration: underline; }
td.fail { background: var(--fail-bg); box-shadow: inset 0 0 0 1.5px var(--fail); }
td.closed.fail { background: var(--fail-closed); }
td.fail .n { font-weight: 800; }
td.unknown { color: var(--unknown); border-style: dashed; }
.mark { position: absolute; top: 2px; right: 4px; color: var(--edge); font-size: .72rem; line-height: 1; }
.mark.q { color: var(--unknown); }
td.edge.unknown .mark.q { top: auto; bottom: 2px; }
.hol, .rule { display: block; font-size: .64rem; line-height: 1.2; margin-top: 1px; color: var(--closed-fg); overflow-wrap: anywhere; }
.rule { color: var(--dim); }
td.opened .rule { color: var(--ok); }
@media print {
  body { font-size: 12px; background: #fff; color: #000; }
  main { max-width: none; padding: 0; }
  .months { grid-template-columns: repeat(3, 1fr); }
  figure.month, blockquote.law, table.data { break-inside: avoid; }
}
"#;
