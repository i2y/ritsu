//! The page as one HTML file (DESIGN 10): ritsu's frame of a page (`docpage`: the head and the
//! palette, light and dark), the map as SVG (`draw.rs`), and the same blocks as the Markdown. It
//! reads nothing from outside; a box of the map, and a link to a context, go to the context's
//! part of the page.

use super::{Block, Page, anchor, draw};
use ritsu_base::docpage::{HTML_TAIL, Palette, esc, html_head};

const CSS: &str = "body { margin: 0; background: var(--bg); color: var(--fg); font: 15px/1.55 system-ui, -apple-system, \"Segoe UI\", \"Hiragino Sans\", sans-serif; }
main { max-width: 1120px; margin: 0 auto; padding: 24px 16px 64px; }
h1 { font-size: 26px; margin: 8px 0 12px; }
h2 { font-size: 20px; margin: 40px 0 10px; padding-top: 12px; border-top: 1px solid var(--line); }
h3 { font-size: 16px; margin: 24px 0 8px; }
p, li { max-width: 80ch; }
code { font: 13px ui-monospace, SFMono-Regular, Menlo, monospace; background: var(--code); padding: 1px 4px; border-radius: 4px; }
a { color: var(--accent); }
table { border-collapse: collapse; margin: 8px 0 16px; display: block; overflow-x: auto; }
th, td { border: 1px solid var(--line); padding: 6px 10px; text-align: left; vertical-align: top; }
th { background: var(--soft); }
figure { margin: 16px 0; overflow-x: auto; background: var(--panel); border: 1px solid var(--line); border-radius: 8px; padding: 8px; }
svg.map { display: block; max-width: none; }
svg.map rect { fill: var(--bg); stroke: var(--accent); stroke-width: 2; }
svg.map a.ctx:hover rect, svg.map a.ctx:focus rect { fill: var(--soft); stroke-width: 3; }
svg.map text { fill: var(--fg); font: 11px system-ui, -apple-system, \"Hiragino Sans\", sans-serif; }
svg.map text.name { font-size: 15px; font-weight: 600; }
svg.map text.alias { fill: var(--dim); }
svg.map text.label { paint-order: stroke; stroke: var(--panel); stroke-width: 4px; stroke-linejoin: round; }
svg.map line.rel { stroke: var(--dim); stroke-width: 1.6; }
svg.map line.dotted { stroke-dasharray: 6 5; }
svg.map path.head { fill: var(--dim); }
:target { scroll-margin-top: 8px; }
h2:target { color: var(--accent); }
";

/// Text with `code` in backquotes and `[[alias|text]]` links, as HTML.
fn inline(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    loop {
        let link = rest.find("[[");
        let tick = rest.find('`');
        match (link, tick) {
            (Some(i), t) if t.is_none_or(|t| i < t) => {
                out.push_str(&esc(&rest[..i]));
                let after = &rest[i + 2..];
                let Some(j) = after.find("]]") else {
                    out.push_str(&esc(&rest[i..]));
                    break;
                };
                let (alias, text) = after[..j].split_once('|').unwrap_or((&after[..j], &after[..j]));
                out.push_str(&format!("<a href=\"#{}\">{}</a>", esc(&anchor(alias)), esc(text)));
                rest = &after[j + 2..];
            }
            (_, Some(t)) => {
                out.push_str(&esc(&rest[..t]));
                let after = &rest[t + 1..];
                let Some(j) = after.find('`') else {
                    out.push_str(&esc(&rest[t..]));
                    break;
                };
                out.push_str(&format!("<code>{}</code>", esc(&after[..j])));
                rest = &after[j + 1..];
            }
            _ => {
                out.push_str(&esc(rest));
                break;
            }
        }
    }
    out
}

pub fn write(p: &Page) -> String {
    let palette = Palette::default();
    let css = format!("{}{CSS}", palette.css());
    let mut out = html_head(p.lang, &format!("sakai {}", env!("CARGO_PKG_VERSION")), &p.title, &css);
    out.push_str("<main>\n");
    for b in &p.blocks {
        match b {
            Block::Heading(level, text, alias) => {
                let id = alias.as_ref().map(|a| format!(" id=\"{}\"", esc(&anchor(a)))).unwrap_or_default();
                out.push_str(&format!("<h{level}{id}>{}</h{level}>\n", inline(text)));
            }
            Block::Para(text) => out.push_str(&format!("<p>{}</p>\n", inline(text))),
            Block::List(items) => {
                out.push_str("<ul>\n");
                for i in items {
                    out.push_str(&format!("<li>{}</li>\n", inline(i)));
                }
                out.push_str("</ul>\n");
            }
            Block::Table(head, rows) => {
                out.push_str("<table>\n<thead><tr>");
                for h in head {
                    out.push_str(&format!("<th>{}</th>", inline(h)));
                }
                out.push_str("</tr></thead>\n<tbody>\n");
                for r in rows {
                    out.push_str("<tr>");
                    for c in r {
                        out.push_str(&format!("<td>{}</td>", inline(c)));
                    }
                    out.push_str("</tr>\n");
                }
                out.push_str("</tbody>\n</table>\n");
            }
            Block::Map => {
                out.push_str("<figure>\n");
                out.push_str(&draw::svg(p));
                out.push_str("</figure>\n");
            }
        }
    }
    out.push_str("</main>\n");
    out.push_str(HTML_TAIL);
    out
}
