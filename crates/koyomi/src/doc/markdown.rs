//! The page as Markdown, for a pull request: GitHub draws the tables, the lists and the
//! quotes, and the month tables are tables of numbers with a legend.

use super::{Block, Inline, Item, Legend, Page, Para, Quote};
use ritsu_base::text::ja_spacing;
use crate::doc::months::{Grid, named_runs};
use ritsu_base::text::{Lang, Text};

/// Text that Markdown must not read as markup. `_` is left as it is: GitHub never reads one
/// between two letters as emphasis, and names such as `満了日_翌日` have it there.
fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for ch in s.chars() {
        if matches!(ch, '\\' | '*' | '[' | ']' | '<' | '|' | '`') {
            o.push('\\');
        }
        o.push(ch);
    }
    o
}

/// A code span that holds `s` whatever backticks it has.
fn code(s: &str) -> String {
    let ticks = if s.contains('`') { "``" } else { "`" };
    let pad = if s.starts_with('`') || s.ends_with('`') { " " } else { "" };
    format!("{ticks}{pad}{s}{pad}{ticks}")
}

/// Words, in which what is between backticks is code, as in the diagnostics.
fn words(s: &str, lang: Lang) -> String {
    let s = if lang == Lang::Ja { ja_spacing(s) } else { s.to_string() };
    let mut o = String::new();
    for (i, part) in s.split('`').enumerate() {
        if i % 2 == 1 {
            o.push_str(&code(part));
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
                let s = if i == 0 && lang == Lang::En { ritsu_base::text::capitalize(s) } else { s.clone() };
                o.push_str(&esc(&s));
            }
            Inline::C(s) => o.push_str(&code(s)),
            Inline::B(s) => o.push_str(&format!("**{}**", esc(s))),
            Inline::A(s, href) => o.push_str(&format!("[{}]({href})", esc(s))),
        }
    }
    o
}

/// A table cell: no pipe and no line break may end it early.
fn cell(p: &Para, lang: Lang) -> String {
    let s = inline(p, lang).replace('\n', " ");
    // Inside a code span, the pipe still ends a cell in GitHub's tables.
    let mut o = String::new();
    let mut in_code = false;
    let mut prev = ' ';
    for ch in s.chars() {
        if ch == '`' {
            in_code = !in_code;
        }
        if ch == '|' && in_code && prev != '\\' {
            o.push('\\');
        }
        o.push(ch);
        prev = ch;
    }
    o
}

fn quote(q: &Quote, indent: &str, o: &mut String) {
    o.push_str(&format!("{indent}> **{}**\n", esc(&q.head)));
    for l in &q.lines {
        o.push_str(&format!("{indent}>\n{indent}> {}\n", esc(l)));
    }
    o.push('\n');
}

fn items(list: &[Item], ordered: bool, lang: Lang, o: &mut String) {
    for (i, it) in list.iter().enumerate() {
        let mark = if ordered { format!("{}. ", i + 1) } else { "- ".to_string() };
        let indent = " ".repeat(mark.len());
        o.push_str(&format!("{mark}{}\n", inline(&it.body, lang)));
        for p in &it.more {
            o.push_str(&format!("\n{indent}{}\n", inline(p, lang)));
        }
        for q in &it.quotes {
            o.push('\n');
            quote(q, &indent, o);
        }
        if !it.more.is_empty() || !it.quotes.is_empty() {
            o.push('\n');
        }
    }
    o.push('\n');
}

const WEEK_JA: [&str; 7] = ["月", "火", "水", "木", "金", "土", "日"];
const WEEK_EN: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const MONTH_EN: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

/// One month: its name, the table, and the names of the days.
fn month(g: &Grid, legend: &Legend, lang: Lang, o: &mut String) {
    let title = match lang {
        Lang::Ja => format!("{} 年 {} 月", g.ym.y, g.ym.m),
        Lang::En => format!("{} {}", MONTH_EN[(g.ym.m - 1) as usize], g.ym.y),
    };
    o.push_str(&format!("**{title}**\n\n"));
    let week = if lang == Lang::Ja { WEEK_JA } else { WEEK_EN };
    o.push_str(&format!("| {} |\n", week.join(" | ")));
    o.push_str("|---:|---:|---:|---:|---:|---:|---:|\n");
    for w in &g.weeks {
        let cells: Vec<String> = w
            .iter()
            .map(|c| match c {
                None => String::new(),
                Some(c) => {
                    let (_, _, d) = c.day.ymd();
                    let mut s = if c.closed && legend.closed { format!("({d})") } else { d.to_string() };
                    if !c.fails.is_empty() {
                        s = format!("**{s}**");
                    }
                    if !c.edges.is_empty() {
                        s.push('◆');
                    }
                    if !c.known {
                        s.push('?');
                    }
                    s
                }
            })
            .collect();
        o.push_str(&format!("| {} |\n", cells.join(" | ")));
    }
    let runs = named_runs(g, lang);
    let (closed, open): (Vec<_>, Vec<_>) = runs.into_iter().partition(|(a, _, _)| {
        g.cells().find(|c| c.day.ymd().2 == *a).is_none_or(|c| !c.opened)
    });
    let say = |list: &[(u32, u32, String)]| -> String {
        list.iter()
            .map(|(a, b, n)| match (a == b, lang) {
                (true, Lang::Ja) => format!("{a} 日 {n}"),
                (false, Lang::Ja) => format!("{a}〜{b} 日 {n}"),
                (true, Lang::En) => format!("{a} {n}"),
                (false, Lang::En) => format!("{a}–{b} {n}"),
            })
            .collect::<Vec<_>>()
            .join(if lang == Lang::Ja { "、" } else { ", " })
    };
    if !closed.is_empty() {
        let head = if lang == Lang::Ja { "休みの名前: " } else { "Named closed days: " };
        o.push_str(&format!("\n{head}{}\n", esc(&say(&closed))));
    }
    if !open.is_empty() {
        let head = if lang == Lang::Ja { "営業する日: " } else { "Open: " };
        o.push_str(&format!("\n{head}{}\n", esc(&say(&open))));
    }
    o.push('\n');
}

pub fn render(p: &Page) -> String {
    let lang = p.lang;
    let mut o = format!("# {}\n\n", esc(&p.title));
    if let Some(d) = &p.description {
        o.push_str(&format!("{}\n\n", esc(d)));
    }
    for b in &p.blocks {
        match b {
            Block::H2(s) => o.push_str(&format!("## {}\n\n", esc(s))),
            Block::H3(s) => o.push_str(&format!("### {}\n\n", esc(s))),
            Block::P(para) => o.push_str(&format!("{}\n\n", inline(para, lang))),
            Block::Alert { ok, lines } => {
                o.push_str(if *ok { "> [!NOTE]\n" } else { "> [!WARNING]\n" });
                for (i, l) in lines.iter().enumerate() {
                    if i > 0 {
                        o.push_str(">\n");
                    }
                    o.push_str(&format!("> {}\n", inline(l, lang)));
                }
                o.push('\n');
            }
            Block::Ol(list) => items(list, true, lang, &mut o),
            Block::Ul(list) => items(list, false, lang, &mut o),
            Block::Table { head, rows } => {
                o.push_str(&format!("| {} |\n", head.iter().map(|h| esc(h)).collect::<Vec<_>>().join(" | ")));
                o.push_str(&format!("|{}\n", "---|".repeat(head.len())));
                for r in rows {
                    o.push_str(&format!("| {} |\n", r.iter().map(|c| cell(c, lang)).collect::<Vec<_>>().join(" | ")));
                }
                o.push('\n');
            }
            Block::Trace(steps) => {
                o.push_str("```text\n");
                for l in crate::diag::render_steps(steps, lang, false) {
                    o.push_str(&l);
                    o.push('\n');
                }
                o.push_str("```\n\n");
            }
            Block::Quote(q) => quote(q, "", &mut o),
            Block::Stamp(rows) => {
                for (k, v) in rows {
                    o.push_str(&format!("- {}: {}\n", esc(k), inline(v, lang)));
                }
                o.push('\n');
            }
            Block::Months { grids, legend } => {
                let mut parts: Vec<Text> = Vec::new();
                if legend.closed {
                    parts.push(tr!("（ ）で囲んだ日は休み", "a day in parentheses is closed"));
                }
                if legend.fails {
                    parts.push(tr!("太字の日は条件が成り立たない入力", "a day in bold is an input a claim fails on"));
                }
                if legend.edges {
                    parts.push(tr!("◆ はエッジケースの入力", "◆ marks the input of an edge case"));
                }
                if legend.unknown {
                    parts.push(tr!("? はカレンダーが知らない日", "? marks a day the calendar does not know"));
                }
                if !parts.is_empty() {
                    let l = Text::join(&parts, "、", "; ");
                    let line = tr!("表の見方: {}。", "In the tables, {}.", l.ja; l.en);
                    o.push_str(&format!("{}\n\n", esc(line.get(lang))));
                }
                for g in grids {
                    month(g, legend, lang, &mut o);
                }
            }
        }
    }
    // Lists end with a blank line of their own; one is enough anywhere.
    while o.contains("\n\n\n") {
        o = o.replace("\n\n\n", "\n\n");
    }
    while o.ends_with("\n\n") {
        o.pop();
    }
    o
}
