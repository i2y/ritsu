//! The page as Markdown (DESIGN 10), with the map as a Mermaid `flowchart LR` that GitHub draws:
//! a context is `<alias>["<name>"]`, an arrow goes from the upstream to the downstream with the
//! pattern as its label, a shared kernel and a partnership are `<-->`, separate ways a dotted
//! line. A link to a context goes to its heading.

use super::{Block, Line, Page, context_heading, slug};

/// `[[alias|text]]` as a link to the context's heading.
fn inline(p: &Page, s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find("[[") {
        out.push_str(&rest[..i]);
        let after = &rest[i + 2..];
        let Some(j) = after.find("]]") else {
            out.push_str(&rest[i..]);
            return out;
        };
        let (alias, text) = after[..j].split_once('|').unwrap_or((&after[..j], &after[..j]));
        match p.nodes.iter().find(|n| n.alias == alias) {
            Some(n) => out.push_str(&format!("[{text}](#{})", slug(&context_heading(p.lang, n)))),
            None => out.push_str(text),
        }
        rest = &after[j + 2..];
    }
    out.push_str(rest);
    out
}

fn cell(p: &Page, s: &str) -> String {
    inline(p, s).replace('|', "\\|").replace('\n', " ")
}

/// A label of the chart, in quotes: Mermaid's `#quot;` for a double quote.
fn label(lines: &[String]) -> String {
    lines.join("<br>").replace('"', "#quot;")
}

/// The map as a Mermaid chart.
pub fn chart(p: &Page) -> String {
    let mut s = String::from("```mermaid\nflowchart LR\n");
    for n in &p.nodes {
        s.push_str(&format!("  {}[\"{}\"]\n", n.alias, n.name.replace('"', "#quot;")));
    }
    for e in &p.edges {
        let (a, b) = (&p.nodes[e.from].alias, &p.nodes[e.to].alias);
        let l = label(&e.label);
        match e.line {
            Line::Arrow => s.push_str(&format!("  {a} -->|\"{l}\"| {b}\n")),
            Line::Both => s.push_str(&format!("  {a} <-->|\"{l}\"| {b}\n")),
            Line::Dotted => s.push_str(&format!("  {a} -.-|\"{l}\"| {b}\n")),
        }
    }
    s.push_str("```\n");
    s
}

pub fn write(p: &Page) -> String {
    let mut out = String::new();
    for b in &p.blocks {
        if !out.is_empty() {
            out.push('\n');
        }
        match b {
            Block::Heading(level, text, _) => out.push_str(&format!("{} {}\n", "#".repeat(*level as usize), inline(p, text))),
            Block::Para(text) => out.push_str(&format!("{}\n", inline(p, text))),
            Block::List(items) => {
                for i in items {
                    out.push_str(&format!("- {}\n", inline(p, i)));
                }
            }
            Block::Table(head, rows) => {
                out.push_str(&format!("| {} |\n", head.iter().map(|h| cell(p, h)).collect::<Vec<_>>().join(" | ")));
                out.push_str(&format!("|{}\n", "---|".repeat(head.len())));
                for r in rows {
                    out.push_str(&format!("| {} |\n", r.iter().map(|c| cell(p, c)).collect::<Vec<_>>().join(" | ")));
                }
            }
            Block::Map => out.push_str(&chart(p)),
        }
    }
    out
}
