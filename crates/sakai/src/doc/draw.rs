//! The map of the HTML page as SVG (DESIGN 10; dandori's and chobo's `draw.rs` are the model): a
//! context is a box, a relationship a line with the pattern as its label. Upstreams stand to the
//! left of their downstreams (the longest chain of upstreams decides the column); a context with
//! no upstream or downstream stands in the column of the context it has a relationship with. Two
//! relationships between the same two contexts are drawn side by side. A box is a link to the
//! context's part of the page.

use super::{Line, Page, anchor};
use ritsu_base::docpage::esc;

const BOX_W: f64 = 180.0;
const BOX_H: f64 = 54.0;
const COL: f64 = 440.0;
const ROW: f64 = 170.0;
const PAD: f64 = 40.0;

/// The column and the row of each context.
pub fn layout(p: &Page) -> Vec<(usize, usize)> {
    let n = p.nodes.len();
    let mut col = vec![0usize; n];
    let arrows: Vec<(usize, usize)> = p.edges.iter().filter(|e| e.line == Line::Arrow).map(|e| (e.from, e.to)).collect();
    for _ in 0..n {
        for &(a, b) in &arrows {
            if col[b] < col[a] + 1 {
                col[b] = col[a] + 1;
            }
        }
    }
    for i in 0..n {
        if arrows.iter().any(|&(a, b)| a == i || b == i) {
            continue;
        }
        if let Some(e) = p.edges.iter().find(|e| e.from == i || e.to == i) {
            let other = if e.from == i { e.to } else { e.from };
            if arrows.iter().any(|&(a, b)| a == other || b == other) {
                col[i] = col[other];
            }
        }
    }
    let mut rows = vec![0usize; col.iter().max().map(|m| m + 1).unwrap_or(0)];
    col.iter()
        .map(|&c| {
            let r = rows[c];
            rows[c] += 1;
            (c, r)
        })
        .collect()
}

/// Where the line from the centre of a box toward `(dx, dy)` leaves it.
fn border(cx: f64, cy: f64, dx: f64, dy: f64) -> (f64, f64) {
    let (hw, hh) = (BOX_W / 2.0 + 4.0, BOX_H / 2.0 + 4.0);
    let tx = if dx.abs() > 1e-9 { hw / dx.abs() } else { f64::INFINITY };
    let ty = if dy.abs() > 1e-9 { hh / dy.abs() } else { f64::INFINITY };
    let t = tx.min(ty);
    (cx + dx * t, cy + dy * t)
}

pub fn svg(p: &Page) -> String {
    let at = layout(p);
    let cols = at.iter().map(|a| a.0).max().map(|m| m + 1).unwrap_or(1);
    let rows = at.iter().map(|a| a.1).max().map(|m| m + 1).unwrap_or(1);
    let (w, h) = (PAD * 2.0 + (cols as f64 - 1.0) * COL + BOX_W, PAD * 2.0 + (rows as f64 - 1.0) * ROW + BOX_H);
    let centre = |i: usize| (PAD + at[i].0 as f64 * COL + BOX_W / 2.0, PAD + at[i].1 as f64 * ROW + BOX_H / 2.0);
    let mut s = format!(
        "<svg class=\"map\" viewBox=\"0 0 {w:.0} {h:.0}\" width=\"{w:.0}\" height=\"{h:.0}\" role=\"img\" aria-label=\"{}\">\n",
        esc(&p.title)
    );
    s.push_str("<defs><marker id=\"head\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"8\" markerHeight=\"8\" orient=\"auto-start-reverse\"><path d=\"M0,0 L10,5 L0,10 z\" class=\"head\"/></marker></defs>\n");
    // the lines, under the boxes
    for (k, e) in p.edges.iter().enumerate() {
        let same: Vec<usize> = p.edges.iter().enumerate().filter(|(_, f)| (f.from, f.to) == (e.from, e.to) || (f.from, f.to) == (e.to, e.from)).map(|(j, _)| j).collect();
        let nth = same.iter().position(|&j| j == k).unwrap_or(0) as f64;
        let shift = (nth - (same.len() as f64 - 1.0) / 2.0) * 30.0;
        let ((ax, ay), (bx, by)) = (centre(e.from), centre(e.to));
        let (dx, dy) = (bx - ax, by - ay);
        let len = (dx * dx + dy * dy).sqrt().max(1.0);
        let (ux, uy) = (dx / len, dy / len);
        let (nx, ny) = (-uy * shift, ux * shift);
        let (x1, y1) = border(ax + nx, ay + ny, ux, uy);
        let (x2, y2) = border(bx + nx, by + ny, -ux, -uy);
        let (class, marks) = match e.line {
            Line::Arrow => ("rel", " marker-end=\"url(#head)\""),
            Line::Both => ("rel both", " marker-start=\"url(#head)\" marker-end=\"url(#head)\""),
            Line::Dotted => ("rel dotted", ""),
        };
        s.push_str(&format!(
            "<g class=\"edge\" data-from=\"{}\" data-to=\"{}\"><line class=\"{class}\" x1=\"{x1:.1}\" y1=\"{y1:.1}\" x2=\"{x2:.1}\" y2=\"{y2:.1}\"{marks}/>",
            esc(&p.nodes[e.from].alias),
            esc(&p.nodes[e.to].alias)
        ));
        // two labels on lines side by side stand apart along them
        let at = 0.5 + (nth - (same.len() as f64 - 1.0) / 2.0) * 0.5;
        let (mx, my) = (x1 + (x2 - x1) * at, y1 + (y2 - y1) * at);
        let top = my - (e.label.len() as f64 - 1.0) * 7.0;
        for (i, l) in e.label.iter().enumerate() {
            s.push_str(&format!("<text class=\"label\" x=\"{mx:.1}\" y=\"{:.1}\" text-anchor=\"middle\">{}</text>", top + i as f64 * 14.0 + 4.0, esc(l)));
        }
        s.push_str("</g>\n");
    }
    for (i, n) in p.nodes.iter().enumerate() {
        let (cx, cy) = centre(i);
        let (x, y) = (cx - BOX_W / 2.0, cy - BOX_H / 2.0);
        s.push_str(&format!(
            "<a class=\"ctx\" href=\"#{}\" data-alias=\"{}\"><rect x=\"{x:.1}\" y=\"{y:.1}\" width=\"{BOX_W:.0}\" height=\"{BOX_H:.0}\" rx=\"8\"/><text class=\"name\" x=\"{cx:.1}\" y=\"{:.1}\" text-anchor=\"middle\">{}</text><text class=\"alias\" x=\"{cx:.1}\" y=\"{:.1}\" text-anchor=\"middle\">{}</text></a>\n",
            anchor(&n.alias),
            esc(&n.alias),
            cy - 3.0,
            esc(&n.name),
            cy + 15.0,
            esc(&n.alias)
        ));
    }
    s.push_str("</svg>\n");
    s
}
