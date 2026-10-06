//! The page as Markdown (DESIGN 10), with the map as a Mermaid `flowchart LR` that GitHub draws:
//! a context is `<alias>["<name>"]`, an arrow goes from the upstream to the downstream with the
//! pattern as its label, a shared kernel and a partnership are `<-->`, separate ways a dotted
//! line. A link to a context goes to its heading.

use super::{Block, Line, Page, context_heading, slug};

/// Text written as Markdown prose (ritsu's DESIGN 9.2): a `<` that could begin HTML (followed by
/// a letter, `/`, `!` or `?`) is written `&lt;` outside the code spans, so a description that
/// holds `</script>` or a tag stays text in a renderer that passes HTML through; the code spans,
/// and a `<` before anything else (`<= 3`, `a < b`), are left as they are. The same rule as the
/// Markdown of `dandori doc` and `rulec doc`.
fn md_prose(s: &str) -> String {
    let cs: Vec<char> = s.chars().collect();
    let mut o = String::with_capacity(s.len());
    let mut i = 0;
    while i < cs.len() {
        if cs[i] == '`' {
            // a code span runs to the next run of as many backquotes; without one, the backquotes
            // are text
            let n = cs[i..].iter().take_while(|c| **c == '`').count();
            let mut j = i + n;
            let mut end = None;
            while j < cs.len() {
                let m = cs[j..].iter().take_while(|c| **c == '`').count();
                if m == n {
                    end = Some(j + m);
                    break;
                }
                j += m.max(1);
            }
            let stop = end.unwrap_or(i + n);
            o.extend(&cs[i..stop]);
            i = stop;
            continue;
        }
        if cs[i] == '<' && cs.get(i + 1).is_some_and(|c| c.is_ascii_alphabetic() || matches!(c, '/' | '!' | '?')) {
            o.push_str("&lt;");
        } else {
            o.push(cs[i]);
        }
        i += 1;
    }
    o
}

/// `[[alias|text]]` as a link to the context's heading. A carriage return, which a `.ctx` string
/// can hold and Markdown takes for the end of a line, is a space: what a `.ctx` writes does not
/// start a line of the page (ritsu's DESIGN 9.2). The text is prose ([`md_prose`]).
fn inline(p: &Page, s: &str) -> String {
    let s = md_prose(&s.replace('\r', " "));
    let mut out = String::new();
    let mut rest = s.as_str();
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
