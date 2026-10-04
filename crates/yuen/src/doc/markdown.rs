//! The page as Markdown (the default form): what a repository's viewer shows, with the same
//! blocks as the HTML page.

use super::{Block, Inline, Page, Para};
use ritsu_base::text::Lang;

/// Code as Markdown writes it: in backticks, more of them when the code holds one.
fn code(s: &str) -> String {
    if s.contains('`') { format!("`` {s} ``") } else { format!("`{s}`") }
}

fn inline(p: &Para, in_table: bool) -> String {
    let mut o = String::new();
    for x in p {
        let part = match x {
            Inline::T(s) => s.clone(),
            Inline::C(s) => code(s),
            Inline::B(s) => format!("**{s}**"),
        };
        o.push_str(&if in_table { part.replace('|', "\\|") } else { part });
    }
    o
}

pub fn render(page: &Page, _lang: Lang) -> String {
    let mut o = format!("# {}\n", page.title);
    for b in &page.blocks {
        o.push('\n');
        match b {
            Block::H2(s, _) => o.push_str(&format!("## {s}\n")),
            Block::H3(s, _) => o.push_str(&format!("### {s}\n")),
            Block::P(p) => o.push_str(&format!("{}\n", inline(p, false))),
            Block::List(items) => {
                for it in items {
                    o.push_str(&format!("- {}\n", inline(it, false)));
                }
            }
            Block::Table(heads, rows) => {
                o.push_str(&format!("| {} |\n", heads.join(" | ")));
                o.push_str(&format!("|{}\n", heads.iter().map(|_| "---|").collect::<String>()));
                for r in rows {
                    let cells: Vec<String> = r.iter().map(|c| inline(c, true)).collect();
                    o.push_str(&format!("| {} |\n", cells.join(" | ")));
                }
            }
            Block::Quote(q) => {
                o.push_str(&format!("> **{}**\n", q.head));
                for l in &q.lines {
                    o.push_str(&format!(">\n> {l}\n"));
                }
            }
            Block::Pre(text) => o.push_str(&format!("```text\n{}```\n", text)),
        }
    }
    o
}
