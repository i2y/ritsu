//! The page as one HTML file: nothing is read from anywhere else (no script, no font, no image,
//! no stylesheet), it has a light and a dark palette (`prefers-color-scheme`, or
//! `data-theme="light|dark"` on the root to choose one), a table moves sideways inside its own
//! frame on a narrow screen so the page never does, and it prints. The frame — the head, the
//! palette and how a page chooses between its two halves — is ritsu-base's
//! ([`ritsu_base::docpage`]); the rules below are yuen's.

use super::{Block, Inline, Page, Para};
use ritsu_base::docpage::{self, Palette, esc};
use ritsu_base::text::Lang;

const RULES: &str = "\
*, *::before, *::after { box-sizing: border-box; }
html { -webkit-text-size-adjust: 100%; }
body { margin: 0; background: var(--bg); color: var(--fg); font: 15px/1.6 -apple-system, BlinkMacSystemFont, \"Segoe UI\", \"Hiragino Sans\", \"Noto Sans JP\", sans-serif; }
main { max-width: 1040px; margin: 0 auto; padding: 24px 16px 64px; }
h1 { font-size: 1.6rem; line-height: 1.3; margin: 0 0 12px; overflow-wrap: anywhere; }
h2 { font-size: 1.25rem; margin: 40px 0 12px; padding-bottom: 6px; border-bottom: 1px solid var(--line); }
h3 { font-size: 1.05rem; margin: 28px 0 8px; overflow-wrap: anywhere; }
p, li { overflow-wrap: anywhere; }
code { font: 0.88em/1.5 ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; background: var(--code); border-radius: 4px; padding: 1px 4px; overflow-wrap: anywhere; }
pre { font: 0.82rem/1.5 ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; background: var(--panel); border: 1px solid var(--line); border-left: 3px solid var(--bad); border-radius: 6px; padding: 10px 12px; overflow-x: auto; white-space: pre; }
.scroll { overflow-x: auto; margin: 8px 0 16px; border: 1px solid var(--line); border-radius: 6px; }
table { border-collapse: collapse; width: 100%; font-size: 0.9rem; }
th, td { text-align: left; vertical-align: top; padding: 6px 10px; border-bottom: 1px solid var(--line); min-width: 7em; }
th { background: var(--soft); color: var(--dim); font-weight: 600; white-space: nowrap; }
tr:last-child td { border-bottom: 0; }
td strong { color: var(--warn); }
td.nw { white-space: nowrap; }
blockquote.law { margin: 8px 0 16px; padding: 8px 14px; border-left: 3px solid var(--accent); background: var(--panel); border-radius: 0 6px 6px 0; }
blockquote.law p { margin: 4px 0; }
blockquote.law .cite { color: var(--dim); font-size: 0.85rem; font-weight: 600; }
.lead { color: var(--dim); }
@media print { body { background: #fff; color: #000; } .scroll { overflow: visible; } pre { white-space: pre-wrap; } }
";

/// A cell that is a date (`2026-10-04`) or a period (`2026-10-01..`, `2014-04-01..2027-03-31`).
fn day_like(p: &Para) -> bool {
    let [Inline::T(s)] = p.as_slice() else { return false };
    let b = s.as_bytes();
    b.len() >= 10 && b[..10].iter().enumerate().all(|(i, c)| if i == 4 || i == 7 { *c == b'-' } else { c.is_ascii_digit() }) && s[10..].chars().all(|c| c.is_ascii_digit() || c == '-' || c == '.')
}

fn inline(p: &Para) -> String {
    let mut o = String::new();
    for x in p {
        match x {
            Inline::T(s) => o.push_str(&esc(s)),
            Inline::C(s) => o.push_str(&format!("<code>{}</code>", esc(s))),
            Inline::B(s) => o.push_str(&format!("<strong>{}</strong>", esc(s))),
        }
    }
    o
}

pub fn render(page: &Page, lang: Lang) -> String {
    let css = format!("{}{RULES}", Palette::default().css());
    let mut o = docpage::html_head(lang, &format!("yuen {}", crate::api::VERSION), &page.title, &css);
    o.push_str("<main>\n");
    o.push_str(&format!("<h1>{}</h1>\n", esc(&page.title)));
    let mut first = true;
    for b in &page.blocks {
        match b {
            Block::H2(s, id) => o.push_str(&format!("<h2 id=\"{}\">{}</h2>\n", esc(id), esc(s))),
            Block::H3(s, id) => o.push_str(&format!("<h3 id=\"{}\">{}</h3>\n", esc(id), esc(s))),
            Block::P(p) => {
                let class = if first { " class=\"lead\"" } else { "" };
                o.push_str(&format!("<p{class}>{}</p>\n", inline(p)));
            }
            Block::List(items) => {
                o.push_str("<ul>\n");
                for it in items {
                    o.push_str(&format!("<li>{}</li>\n", inline(it)));
                }
                o.push_str("</ul>\n");
            }
            Block::Table(heads, rows) => {
                o.push_str("<div class=\"scroll\"><table>\n<thead><tr>");
                for h in heads {
                    o.push_str(&format!("<th>{}</th>", esc(h)));
                }
                o.push_str("</tr></thead>\n<tbody>\n");
                for r in rows {
                    o.push_str("<tr>");
                    for c in r {
                        // a date or a period stays on one line
                        let class = if day_like(c) { " class=\"nw\"" } else { "" };
                        o.push_str(&format!("<td{class}>{}</td>", inline(c)));
                    }
                    o.push_str("</tr>\n");
                }
                o.push_str("</tbody>\n</table></div>\n");
            }
            Block::Quote(q) => {
                o.push_str(&format!("<blockquote class=\"law\">\n<p class=\"cite\">{}</p>\n", esc(&q.head)));
                for l in &q.lines {
                    o.push_str(&format!("<p>{}</p>\n", esc(l)));
                }
                o.push_str("</blockquote>\n");
            }
            Block::Pre(text) => o.push_str(&format!("<pre>{}</pre>\n", esc(text.trim_end()))),
        }
        first = false;
    }
    o.push_str("</main>\n");
    o.push_str(docpage::HTML_TAIL);
    o
}
