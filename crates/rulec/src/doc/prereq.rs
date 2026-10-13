//! The `prerequisites` blocks on the pages (§15.222): for the reader who checks the rule, each
//! block as a table of what every value needs, a picture of the graph — Mermaid in the Markdown,
//! SVG on the HTML page — laid out by the `ordered by` table where there is one and otherwise by
//! how many steps from `start` a value is reached, and what `check` showed of it; for the
//! customer, what comes before what, in a list.
//!
//! As everywhere on these pages, every line traces back to the file or to the check: the needs are
//! the lines as written, the layers are the table's answers or the waves the check reaches the
//! values in, and the findings are the ones the check gave this rule (it passed, so there is no
//! cycle, nothing it cannot reach, and no order broken; what it warned of is listed).

use super::{html_esc, md_esc, sep};
use crate::ast::RuleFile;
use crate::prereq::{Graph, Order};
use crate::types::Checked;

/// The columns of a block's picture, left to right: each with its heading and its values.
fn layers(f: &RuleFile, c: &Checked, g: &Graph) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    match crate::prereq::order_of(f, c, g) {
        Some(Ok(order)) => {
            // The table's answers in their order: an ordered enum's values, or the numbers.
            let keys: Vec<String> = match &order {
                Order::Enum { order: o, .. } => o.clone(),
                Order::Number { of } => {
                    let mut ns: Vec<crate::num::Rat> = of.values().copied().collect();
                    ns.sort_by(|a, b| a.cmp_to(*b));
                    ns.dedup_by(|a, b| a.cmp_to(*b) == std::cmp::Ordering::Equal);
                    ns.iter().map(|r| r.to_string()).collect()
                }
            };
            for k in keys {
                let vs: Vec<String> = g.values.iter().filter(|v| order.shown(v) == k).cloned().collect();
                if !vs.is_empty() {
                    out.push((k, vs));
                }
            }
            let rest: Vec<String> = g.values.iter().filter(|v| order.shown(v).is_empty()).cloned().collect();
            if !rest.is_empty() {
                out.push((tr!("（答えなし）", "(no answer)"), rest));
            }
        }
        _ => {
            for (i, w) in g.waves().into_iter().enumerate() {
                let head = if i == 0 { crate::kw::START.to_string() } else { tr!("{i} 段目", "step {i}") };
                out.push((head, w));
            }
            let never: Vec<String> = g.values.iter().filter(|v| g.never.contains_key(*v)).cloned().collect();
            if !never.is_empty() {
                out.push((tr!("手に入らない", "never taken"), never));
            }
        }
    }
    out
}

/// What a value needs, as its line writes it: `a, b or c`.
fn needs_text(g: &Graph, v: &str) -> String {
    if g.start.iter().any(|s| s == v) {
        return tr!("何も要らない（`{}`）", "nothing (`{}`)", crate::kw::START);
    }
    if let Some(why) = g.never.get(v) {
        return tr!("手に入らない: {why}", "no case can take it: {why}");
    }
    g.requires(v).iter().map(|gr| gr.join(&format!(" {} ", crate::kw::OR))).collect::<Vec<_>>().join(", ")
}

/// Whether the prerequisite `p` of `v` is one of several in an `or`.
fn alternative(g: &Graph, v: &str, p: &str) -> bool {
    g.requires(v).iter().any(|gr| gr.len() > 1 && gr.iter().any(|x| x == p))
}

/// The blocks for the page that checks the rule (§1.6).
pub(super) fn sections(f: &RuleFile, c: &Checked) -> String {
    let mut o = String::new();
    for (k, g) in Graph::all(f, c).iter().enumerate() {
        let (r, a) = crate::codegen::prereq_fn_names(&g.alias, "python");
        o.push_str(&tr!(
            "\n## 前提のつながり: {}\n\n列挙 {} のそれぞれの値を取る前に、何を持っていなければならないかです。値の行のグループはどれも満たします。`,` で区切ったグループは全部、グループの中の `{or}` はどれか一つです。生成コードは、値に要るもの（Python では `{r}`）と、持っているものでその値を取れるか（`{a}`）の二つの関数で答えます。ほかの言語での名前は `rulec api` が挙げます。\n\n",
            "\n## Prerequisites: {}\n\nWhat a case has to hold before it can take each value of enum {}. A value needs every group on its line: groups separated by `,` are all needed, and the values inside a group joined by `{or}` are any one of them. The generated code answers with two functions: what a value needs (`{r}` in Python), and whether it can be taken with what is held (`{a}`). `rulec api` gives their names in the other languages.\n\n",
            md_esc(&g.name),
            md_esc(&g.over),
            or = crate::kw::OR
        ));
        let order = crate::prereq::order_of(f, c, g).and_then(Result::ok);
        let tname = g.ordered_by.clone().unwrap_or_default();
        match &order {
            Some(ord) => {
                o.push_str(&tr!("| 値 | 要るもの | {tname} |\n|---|---|---|\n", "| Value | Needs | {tname} |\n|---|---|---|\n"));
                for v in &g.values {
                    o.push_str(&format!("| {} | {} | {} |\n", md_esc(v), md_esc(&needs_text(g, v)), md_esc(&ord.shown(v))));
                }
            }
            None => {
                o.push_str(&tr!("| 値 | 要るもの |\n|---|---|\n", "| Value | Needs |\n|---|---|\n"));
                for v in &g.values {
                    o.push_str(&format!("| {} | {} |\n", md_esc(v), md_esc(&needs_text(g, v))));
                }
            }
        }
        o.push('\n');
        o.push_str(&mermaid(f, c, g, k));
        o.push_str(&tr!(
            "\n矢印は、前提から、それが要る値へ向かいます。点線の矢印は `{or}` の中の一つで、どれか一つあれば足ります。両端の丸い枠は `{start}` の値、破線の枠は手に入らない値です。{}\n",
            "\nAn arrow goes from a prerequisite to the value that needs it. A dotted arrow is one of the values of an `{or}`, any one of which is enough. A box with round ends is a value of `{start}`; a dashed one is a value no case can take. {}\n",
            match &order {
                Some(_) => tr!("列は表 {tname} の答えの順です。", "The columns are the answers of table {tname}, in their order."),
                None => tr!("列は `{s}` から何段でたどり着くかです。", "The columns are how many steps from `{s}` a value is reached in.", s = crate::kw::START),
            },
            or = crate::kw::OR,
            start = crate::kw::START
        ));
        // What the check showed: it passed, so these hold; the warnings are listed.
        let never: Vec<&String> = g.values.iter().filter(|v| g.never.contains_key(*v)).collect();
        o.push_str(&tr!("\n`rulec check` が確かめたこと:\n\n", "\nWhat `rulec check` showed:\n\n"));
        o.push_str(&tr!(
            "- 循環はありません。どの値も、要るものより後に取れます（E136）。\n",
            "- There is no cycle: every value can be taken after what it needs (E136).\n"
        ));
        o.push_str(&if never.is_empty() {
            tr!("- どの値にも `start` からたどり着けます（E137）。\n", "- Every value can be reached from `start` (E137).\n")
        } else {
            tr!(
                "- 手に入らない値（{}）のほかは、どれも `start` からたどり着けます（E137）。\n",
                "- Every value but those no case can take ({}) can be reached from `start` (E137).\n",
                never.iter().map(|v| md_esc(v)).collect::<Vec<_>>().join(sep())
            )
        });
        if order.is_some() {
            o.push_str(&tr!(
                "- 表 {tname} の順で、どの値も要るものより前に来ません（E138）。\n",
                "- In the order of table {tname}, no value comes before what it needs (E138).\n"
            ));
        }
        let extra = g.redundant();
        if extra.is_empty() {
            o.push_str(&tr!("- ほかの前提からたどれる余分な前提はありません（W130）。\n", "- No prerequisite is already needed through another one (W130).\n"));
        } else {
            for (v, p, w) in extra {
                o.push_str(&tr!(
                    "- {v} の前提の {p} は、{w} の前提からたどれます（W130）。\n",
                    "- {v} needs {p}, which its prerequisite {w} already needs (W130).\n"
                ));
            }
        }
    }
    o
}

/// One block as Mermaid draws it: a column per layer, an arrow from each prerequisite to the
/// value that needs it, dotted for one of an `or`. The first line names the block, which is how the
/// HTML page knows which picture of its own goes there.
fn mermaid(f: &RuleFile, c: &Checked, g: &Graph, k: usize) -> String {
    let id = |v: &str| format!("p{k}v{}", g.values.iter().position(|x| x == v).unwrap_or(0));
    let label = |s: &str| s.replace('"', "'");
    let mut o = format!("```mermaid\n%% prerequisites {k}\nflowchart LR\n");
    for (i, (head, vs)) in layers(f, c, g).iter().enumerate() {
        o.push_str(&format!("  subgraph p{k}l{i}[\"{}\"]\n", label(head)));
        for v in vs {
            let shape = if g.start.contains(v) { format!("([\"{}\"])", label(v)) } else { format!("[\"{}\"]", label(v)) };
            o.push_str(&format!("    {}{shape}\n", id(v)));
        }
        o.push_str("  end\n");
    }
    for (v, p) in g.edges() {
        let arrow = if alternative(g, &v, &p) { "-.->" } else { "-->" };
        o.push_str(&format!("  {} {arrow} {}\n", id(&p), id(&v)));
    }
    let never: Vec<String> = g.values.iter().filter(|v| g.never.contains_key(*v)).map(|v| id(v)).collect();
    if !never.is_empty() {
        o.push_str(&format!("  classDef never stroke-dasharray: 4 3\n  class {} never\n", never.join(",")));
    }
    o.push_str("```\n");
    o
}

/// One block as a picture for the HTML page: the layers left to right, the values of each top to
/// bottom, an arrow from each prerequisite to the value that needs it — dotted for one of an `or`,
/// bent round the side between two values of one layer. Styled where it stands with the page's own
/// colours, so that the page's sheet is the same for every rule and the picture follows the
/// reader's theme.
pub(super) fn svg(f: &RuleFile, c: &Checked, g: &Graph, k: usize) -> String {
    let mut cols = layers(f, c, g);
    // Each column after the first in the order of where its values' prerequisites stand on the
    // left, so that fewer arrows cross; the enum's order where that says nothing.
    let mut row_of: std::collections::BTreeMap<String, f64> = std::collections::BTreeMap::new();
    for (_, vs) in cols.iter_mut() {
        let key = |v: &String| -> f64 {
            let ps: Vec<f64> = g.requires(v).iter().flatten().filter_map(|p| row_of.get(p).copied()).collect();
            if ps.is_empty() { f64::MAX } else { ps.iter().sum::<f64>() / ps.len() as f64 }
        };
        let mut keyed: Vec<(f64, usize, String)> = vs.iter().enumerate().map(|(i, v)| (key(v), i, v.clone())).collect();
        keyed.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal).then(a.1.cmp(&b.1)));
        *vs = keyed.into_iter().map(|(_, _, v)| v).collect();
        for (i, v) in vs.iter().enumerate() {
            row_of.insert(v.clone(), i as f64);
        }
    }
    let wide = |s: &str| (crate::diag::width(s) * 7 + 28).max(64) as i64;
    let node_w = g.values.iter().map(|v| wide(v)).chain(cols.iter().map(|(h, _)| wide(h))).max().unwrap_or(64);
    let (node_h, gap_x, row_h, top, left) = (30i64, 72i64, 44i64, 40i64, 16i64);
    let mut pos: std::collections::BTreeMap<String, (i64, i64, usize)> = std::collections::BTreeMap::new();
    let mut body = String::new();
    let mut tallest = 0usize;
    for (ci, (head, vs)) in cols.iter().enumerate() {
        let x = left + ci as i64 * (node_w + gap_x);
        body.push_str(&format!(
            "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\" style=\"fill: var(--rc-dim); font-size: 12px\">{}</text>\n",
            x + node_w / 2,
            top - 14,
            html_esc(head)
        ));
        for (ri, v) in vs.iter().enumerate() {
            pos.insert(v.clone(), (x, top + ri as i64 * row_h, ci));
        }
        tallest = tallest.max(vs.len());
    }
    let mut side = 0i64;
    let marker = format!("rc-pq-arrow-{k}");
    for (v, p) in g.edges() {
        let (Some(&(xp, yp, cp)), Some(&(xv, yv, cv))) = (pos.get(&p), pos.get(&v)) else { continue };
        let dash = if alternative(g, &v, &p) { " stroke-dasharray: 5 4;" } else { "" };
        let path = if cp + 1 == cv {
            let (x1, y1, x2, y2) = (xp + node_w, yp + node_h / 2, xv, yv + node_h / 2);
            let mid = (x1 + x2) / 2;
            format!("M{x1},{y1} C{mid},{y1} {mid},{y2} {x2},{y2}")
        } else if cp < cv {
            // Past a column: along the gap between two rows, where no value stands, so that the
            // arrow is not read as one from a value it passes.
            let (x1, y1, x2, y2) = (xp + node_w, yp + node_h / 2, xv, yv + node_h / 2);
            let rows = ((yp - top) / row_h + (yv - top) / row_h + 1) / 2;
            let lane = top + rows * row_h - (row_h - node_h) / 2;
            let (a, b) = (x1 + gap_x / 2, x2 - gap_x / 2);
            format!("M{x1},{y1} C{a},{y1} {x1},{lane} {a},{lane} L{b},{lane} C{x2},{lane} {b},{y2} {x2},{y2}")
        } else {
            // within one layer, or back: round the right-hand side
            side += 1;
            let bend = xp.max(xv) + node_w + 14 + 8 * side;
            let (x1, y1, x2, y2) = (xp + node_w, yp + node_h / 2, xv + node_w, yv + node_h / 2);
            format!("M{x1},{y1} C{bend},{y1} {bend},{y2} {x2},{y2}")
        };
        body.push_str(&format!(
            "<path d=\"{path}\" marker-end=\"url(#{marker})\" style=\"stroke: var(--rc-wire); stroke-width: 1.3; fill: none;{dash}\"/>\n"
        ));
    }
    for v in &g.values {
        let Some(&(x, y, _)) = pos.get(v) else { continue };
        let never = g.never.contains_key(v);
        let edge = if never { " stroke-dasharray: 4 3;" } else { "" };
        // A value of `start` has round ends, as Mermaid draws it.
        let rx = if g.start.contains(v) { node_h / 2 } else { 8 };
        body.push_str(&format!(
            "<g data-value=\"{}\"><rect x=\"{x}\" y=\"{y}\" width=\"{node_w}\" height=\"{node_h}\" rx=\"{rx}\" style=\"fill: var(--rc-card); stroke: var(--rc-card-line); stroke-width: 1.2;{edge}\"/><text x=\"{}\" y=\"{}\" text-anchor=\"middle\" style=\"fill: var({}); font-size: 13px\">{}</text></g>\n",
            html_esc(v),
            x + node_w / 2,
            y + node_h / 2 + 5,
            if never { "--rc-dim" } else { "--rc-fg" },
            html_esc(v)
        ));
    }
    let width = left + cols.len() as i64 * (node_w + gap_x) - gap_x + 40 + 8 * side + 16;
    let height = top + tallest as i64 * row_h + 8;
    let title = tr!("前提のつながり {}", "The prerequisites {}", g.name);
    format!(
        "<figure class=\"machine\"><svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\" role=\"img\" aria-label=\"{}\"><title>{}</title>\n\
         <defs><marker id=\"{marker}\" viewBox=\"0 0 10 10\" refX=\"10\" refY=\"5\" markerWidth=\"7\" markerHeight=\"7\" orient=\"auto-start-reverse\"><path d=\"M0,0 L10,5 L0,10 z\" style=\"fill: var(--rc-wire)\"/></marker></defs>\n\
         {body}</svg><figcaption>{}</figcaption></figure>",
        html_esc(&title),
        html_esc(&title),
        html_esc(&tr!(
            "矢印は前提から、それが要る値へ。点線は `or` の中の一つ、両端の丸い枠は `start` の値、破線の枠は手に入らない値です。",
            "An arrow goes from a prerequisite to the value that needs it; a dotted one is one of an `or`. A box with round ends is a value of `start`, a dashed one a value no case can take."
        ))
    )
}

/// The blocks for the customer: what comes before what, value by value, in a list.
pub(super) fn customer(f: &RuleFile, c: &Checked) -> String {
    let mut o = String::new();
    for g in Graph::all(f, c) {
        o.push_str(&tr!("\n## {} の順序\n\n", "\n## The order of {}\n\n", md_esc(&g.name)));
        if !g.start.is_empty() {
            o.push_str(&tr!(
                "- 最初に取れるもの: {}\n",
                "- Can be taken first: {}\n",
                g.start.iter().map(|v| md_esc(v)).collect::<Vec<_>>().join(sep())
            ));
        }
        for v in &g.values {
            if g.start.contains(v) {
                continue;
            }
            match g.never.get(v) {
                Some(why) => o.push_str(&tr!("- **{}**: 取れません（{}）\n", "- **{}**: cannot be taken ({})\n", md_esc(v), md_esc(why))),
                None => {
                    // Each group, the values of an `or` together: `a と（b か c）`, `a and either b or c`.
                    let groups: Vec<String> = g
                        .requires(v)
                        .iter()
                        .map(|gr| match gr.as_slice() {
                            [one] => one.clone(),
                            [a, b] => tr!("（{a} か {b}）", "either {a} or {b}"),
                            [rest @ .., last] => tr!("（{} か {last}）", "one of {} or {last}", rest.join(sep())),
                            [] => String::new(),
                        })
                        .collect();
                    o.push_str(&tr!(
                        "- **{}**: {} のあと\n",
                        "- **{}**: after {}\n",
                        md_esc(v),
                        md_esc(&groups.join(&tr!(" と ", " and ")))
                    ));
                }
            }
        }
    }
    o
}
