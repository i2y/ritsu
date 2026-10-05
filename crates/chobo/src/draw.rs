//! The HTML page of `chobo doc --format html`: the charts drawn as SVG by chobo itself, the
//! tables, and a scenario a reader steps through, the balances after each step computed by the
//! reference interpreter and put in the page as JSON. Nothing is fetched: no script, font or
//! image from elsewhere. The colours follow the reader's light or dark setting, and nothing is
//! wider than a narrow screen (tables and code scroll in their own box, the charts shrink).

use crate::diag::Show;
use ritsu_base::docpage::{self, Palette};
use ritsu_base::text::{Lang, Text};
use crate::doc::{self, Input, html_escape};
use crate::model::*;
use serde_json::json;

// ── text ──────────────────────────────────────────────────────────────────

fn wide(c: char) -> bool {
    let u = c as u32;
    matches!(u, 0x1100..=0x115F | 0x2E80..=0xA4CF | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF | 0xFE30..=0xFE4F | 0xFF00..=0xFF60 | 0xFFE0..=0xFFE6 | 0x20000..=0x3FFFD)
}

/// About how wide a line of text is at `size` pixels: a wide character takes the full size, a
/// narrow one a little over half. Generous, so that a box holds its text in any sans-serif font.
pub fn text_w(s: &str, size: f64) -> f64 {
    s.chars().map(|c| if wide(c) { size * 1.02 } else if c == ' ' { size * 0.3 } else { size * 0.62 }).sum()
}

/// A number for an SVG attribute: whole pixels.
fn n(v: f64) -> String {
    format!("{}", v.round() as i64)
}

/// `` `code` `` in a line, as HTML: everything escaped, and the back-quoted parts in `<code>`.
pub fn inline(s: &str) -> String {
    let mut o = String::new();
    for (i, part) in s.split('`').enumerate() {
        if i % 2 == 1 {
            o.push_str(&format!("<code>{}</code>", html_escape(part)));
        } else {
            o.push_str(&html_escape(part));
        }
    }
    o
}

// ── the flow between the accounts ─────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

pub struct Node {
    pub k: usize,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub lines: Vec<String>,
    pub outside: bool,
}

pub struct Edge {
    /// the transfer kind and its move
    pub k: usize,
    pub i: usize,
    pub d: String,
    pub label: String,
    pub lx: f64,
    pub ly: f64,
    pub lw: f64,
    pub pending: bool,
}

pub struct Flow {
    pub w: f64,
    pub h: f64,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

const LINE: f64 = 18.0;
const FONT: f64 = 13.0;
const LABEL: f64 = 12.0;
const GAP_Y: f64 = 30.0;
const LOOP_ROOM: f64 = 56.0;
const ANCHOR: f64 = 22.0;

/// The column of each account, so that things flow from left to right. The moves are followed
/// from where things come from (an account nothing moves into, or one outside the book that the
/// book first takes from); a move back to an account already on the way there (a return, a
/// refund) does not count, and every other move puts its account at least one column to the
/// right of the one it comes from.
fn columns(book: &Book) -> Vec<usize> {
    let count = book.accounts.len();
    let mut out: Vec<Vec<usize>> = vec![vec![]; count];
    let mut has_in = vec![false; count];
    for t in &book.transfers {
        for m in &t.moves {
            let (f, to) = (m.from.kind, m.to.kind);
            if f != to {
                if !out[f].contains(&to) {
                    out[f].push(to);
                }
                has_in[to] = true;
            }
        }
    }
    fn walk(v: usize, out: &[Vec<usize>], seen: &mut [u8], back: &mut Vec<(usize, usize)>) {
        seen[v] = 1;
        for &w in &out[v] {
            match seen[w] {
                0 => walk(w, out, seen, back),
                1 => back.push((v, w)),
                _ => {}
            }
        }
        seen[v] = 2;
    }
    // where things come from: what nothing moves into, then an account outside the book whose
    // first move takes from it, then the rest
    let first_from = |k: usize| {
        for t in &book.transfers {
            for m in &t.moves {
                if m.from.kind == k {
                    return book.accounts[k].outside;
                }
                if m.to.kind == k {
                    return false;
                }
            }
        }
        false
    };
    let mut starts: Vec<usize> = (0..count).filter(|k| !has_in[*k]).collect();
    starts.extend((0..count).filter(|k| has_in[*k] && first_from(*k)));
    let rest: Vec<usize> = (0..count).filter(|k| !starts.contains(k)).collect();
    starts.extend(rest);
    let mut seen = vec![0u8; count];
    let mut back: Vec<(usize, usize)> = Vec::new();
    for v in starts {
        if seen[v] == 0 {
            walk(v, &out, &mut seen, &mut back);
        }
    }
    let mut col = vec![0usize; count];
    for _ in 0..count {
        let mut changed = false;
        for v in 0..count {
            for &w in &out[v] {
                if !back.contains(&(v, w)) && col[w] < col[v] + 1 {
                    col[w] = col[v] + 1;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    col
}

pub fn flow(book: &Book, lang: Lang) -> Flow {
    let col = columns(book);
    let count = book.accounts.len();
    let ncols = col.iter().max().map(|m| m + 1).unwrap_or(1);
    // the box of each account, sized to its lines
    let lines: Vec<Vec<String>> = (0..count).map(|k| doc::node_lines(book, k, lang)).collect();
    let mut w: Vec<f64> = lines.iter().map(|ls| ls.iter().enumerate().map(|(j, l)| text_w(l, if j == 0 { FONT + 1.0 } else { FONT })).fold(0.0, f64::max) + 26.0).collect();
    let mut h: Vec<f64> = lines.iter().map(|ls| ls.len() as f64 * LINE + 14.0).collect();
    for x in &mut w {
        *x = x.max(90.0);
    }

    // where each move leaves and arrives
    struct Plan {
        k: usize,
        i: usize,
        from: usize,
        to: usize,
        fs: Side,
        ts: Side,
    }
    let mut plans: Vec<Plan> = Vec::new();
    for (k, t) in book.transfers.iter().enumerate() {
        for (i, m) in t.moves.iter().enumerate() {
            let (f, to) = (m.from.kind, m.to.kind);
            let (fs, ts) = if f == to {
                (Side::Top, Side::Top)
            } else if col[f] + 1 == col[to] {
                (Side::Right, Side::Left)
            } else if col[to] + 1 == col[f] {
                (Side::Left, Side::Right)
            } else if col[f] != col[to] {
                (Side::Bottom, Side::Bottom)
            } else {
                (Side::Right, Side::Right)
            };
            plans.push(Plan { k, i, from: f, to, fs, ts });
        }
    }
    // a side with many arrows grows its box, so that the arrows keep apart
    for node in 0..count {
        for side in [Side::Left, Side::Right] {
            let ends = plans.iter().filter(|p| (p.from == node && p.fs == side) || (p.to == node && p.ts == side)).count() as f64;
            h[node] = h[node].max(ends * ANCHOR + 12.0);
        }
        for side in [Side::Top, Side::Bottom] {
            let ends = plans.iter().filter(|p| (p.from == node && p.fs == side) || (p.to == node && p.ts == side)).count() as f64;
            w[node] = w[node].max(ends * 34.0 + 20.0);
        }
    }

    // the order in each column: by where the accounts each one moves with stand, a few sweeps
    // each way, so that fewer arrows cross
    let mut order: Vec<Vec<usize>> = vec![vec![]; ncols];
    for k in 0..count {
        order[col[k]].push(k);
    }
    let near: Vec<Vec<usize>> = (0..count).map(|k| plans.iter().filter_map(|p| if p.from == k { Some(p.to) } else if p.to == k { Some(p.from) } else { None }).collect()).collect();
    for _ in 0..4 {
        for (c, side) in (1..ncols).map(|c| (c, c - 1)).chain((0..ncols.saturating_sub(1)).rev().map(|c| (c, c + 1))) {
            let at: Vec<f64> = (0..count).map(|k| order[col[k]].iter().position(|x| *x == k).unwrap() as f64).collect();
            let key = |k: usize| {
                let ns: Vec<f64> = near[k].iter().filter(|n| col[**n] == side).map(|n| at[*n]).collect();
                if ns.is_empty() { at[k] } else { ns.iter().sum::<f64>() / ns.len() as f64 }
            };
            let mut keyed: Vec<(f64, f64, usize)> = order[c].iter().map(|k| (key(*k), at[*k], *k)).collect();
            keyed.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then(a.1.partial_cmp(&b.1).unwrap()));
            order[c] = keyed.into_iter().map(|x| x.2).collect();
        }
    }

    // the columns, side by side, each wide enough for its widest box and the labels after it
    let label_w = |p: &Plan| text_w(&doc::edge_label(book, &book.transfers[p.k], &book.transfers[p.k].moves[p.i], lang), LABEL) + 12.0;
    let mut x_of = vec![0.0f64; ncols];
    let colw: Vec<f64> = (0..ncols).map(|c| order[c].iter().map(|k| w[*k]).fold(0.0, f64::max)).collect();
    let mut x = 20.0;
    for c in 0..ncols {
        x_of[c] = x;
        x += colw[c];
        if c + 1 < ncols {
            let between = plans.iter().filter(|p| (col[p.from] == c && col[p.to] == c + 1) || (col[p.from] == c + 1 && col[p.to] == c)).map(&label_w).fold(0.0, f64::max);
            let bulge = if plans.iter().any(|p| p.fs == Side::Right && p.ts == Side::Right && col[p.from] == c) { 70.0 } else { 0.0 };
            x += (between + 50.0).max(130.0) + bulge;
        }
    }
    let right_room = if plans.iter().any(|p| p.fs == Side::Right && p.ts == Side::Right && col[p.from] + 1 == ncols) { 80.0 } else { 0.0 };
    let width = x + 20.0 + right_room;

    // the boxes of a column, one under another, the column centred on the tallest
    let looped = |k: usize| plans.iter().any(|p| p.from == k && p.to == k);
    let mut y = vec![0.0f64; count];
    let mut heights = vec![0.0f64; ncols];
    for c in 0..ncols {
        let mut at = 0.0;
        for k in &order[c] {
            if looped(*k) {
                at += LOOP_ROOM;
            }
            y[*k] = at;
            at += h[*k] + GAP_Y;
        }
        heights[c] = (at - GAP_Y).max(0.0);
    }
    let tallest = heights.iter().cloned().fold(0.0, f64::max);
    let top = 24.0;
    for k in 0..count {
        y[k] += top + (tallest - heights[col[k]]) / 2.0;
    }
    let nodes: Vec<Node> = (0..count).map(|k| Node { k, x: x_of[col[k]] + (colw[col[k]] - w[k]) / 2.0, y: y[k], w: w[k], h: h[k], lines: lines[k].clone(), outside: book.accounts[k].outside }).collect();
    let bottom = nodes.iter().map(|n| n.y + n.h).fold(0.0, f64::max);

    // the place of each end on its side: spread out, in the order of where the other end is
    let mut anchor: Vec<[(f64, f64); 2]> = vec![[(0.0, 0.0); 2]; plans.len()];
    for node in 0..count {
        for side in [Side::Left, Side::Right, Side::Top, Side::Bottom] {
            let mut ends: Vec<(f64, usize, usize)> = Vec::new();
            for (pi, p) in plans.iter().enumerate() {
                if p.from == node && p.fs == side {
                    let o = &nodes[p.to];
                    ends.push((if matches!(side, Side::Left | Side::Right) { o.y + o.h / 2.0 } else { o.x + o.w / 2.0 }, pi, 0));
                }
                if p.to == node && p.ts == side {
                    let o = &nodes[p.from];
                    ends.push((if matches!(side, Side::Left | Side::Right) { o.y + o.h / 2.0 } else { o.x + o.w / 2.0 }, pi, 1));
                }
            }
            ends.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
            let nd = &nodes[node];
            let m = ends.len() as f64;
            for (j, (_, pi, end)) in ends.iter().enumerate() {
                let f = (j as f64 + 1.0) / (m + 1.0);
                anchor[*pi][*end] = match side {
                    Side::Left => (nd.x, nd.y + nd.h * f),
                    Side::Right => (nd.x + nd.w, nd.y + nd.h * f),
                    Side::Top => (nd.x + nd.w * f, nd.y),
                    Side::Bottom => (nd.x + nd.w * f, nd.y + nd.h),
                };
            }
        }
    }

    // the arrows
    let mut edges: Vec<Edge> = Vec::new();
    let mut under = 0.0;
    for (pi, p) in plans.iter().enumerate() {
        let (x1, y1) = anchor[pi][0];
        let (x2, y2) = anchor[pi][1];
        let t = &book.transfers[p.k];
        let (c1, c2) = match (p.fs, p.ts) {
            (Side::Top, _) => ((x1 - 10.0, y1 - LOOP_ROOM + 6.0), (x2 + 10.0, y2 - LOOP_ROOM + 6.0)),
            (Side::Bottom, _) => {
                under += 1.0;
                let yb = bottom + 30.0 + 18.0 * under;
                ((x1, yb), (x2, yb))
            }
            (Side::Left, Side::Left) => ((x1 - 60.0, y1), (x2 - 60.0, y2)),
            (Side::Right, Side::Right) => ((x1 + 60.0, y1), (x2 + 60.0, y2)),
            _ => {
                let dx = ((x2 - x1).abs() / 2.0).max(30.0);
                let s1 = if p.fs == Side::Right { 1.0 } else { -1.0 };
                let s2 = if p.ts == Side::Right { 1.0 } else { -1.0 };
                ((x1 + s1 * dx, y1), (x2 + s2 * dx, y2))
            }
        };
        let d = format!("M{} {} C{} {}, {} {}, {} {}", n(x1), n(y1), n(c1.0), n(c1.1), n(c2.0), n(c2.1), n(x2), n(y2));
        let lx = (x1 + 3.0 * c1.0 + 3.0 * c2.0 + x2) / 8.0;
        let ly = (y1 + 3.0 * c1.1 + 3.0 * c2.1 + y2) / 8.0;
        let label = doc::edge_label(book, t, &t.moves[p.i], lang);
        let lw = text_w(&label, LABEL) + 10.0;
        edges.push(Edge { k: p.k, i: p.i, d, label, lx, ly, lw, pending: t.is_pending() });
    }
    // labels that would lie on one another move down, one at a time
    for j in 0..edges.len() {
        for _ in 0..60 {
            let (lx, ly, lw) = (edges[j].lx, edges[j].ly, edges[j].lw);
            let hit = edges[..j].iter().any(|e| (e.lx - lx).abs() < (e.lw + lw) / 2.0 && (e.ly - ly).abs() < 17.0);
            if !hit {
                break;
            }
            edges[j].ly += 4.0;
        }
    }
    let lowest = edges.iter().map(|e| e.ly + 12.0).fold(bottom, f64::max);
    let height = lowest + if plans.iter().any(|p| p.fs == Side::Bottom) { 24.0 } else { 20.0 };
    Flow { w: width, h: height, nodes, edges }
}

/// The flow as SVG. `id` keeps two copies on one page apart; each arrow names its transfer
/// (`data-k`), for a scenario to light up.
pub fn flow_svg(book: &Book, lang: Lang, id: &str) -> String {
    let f = flow(book, lang);
    let title = Text { ja: format!("{} の勘定のあいだの流れ", book.name), en: format!("How things move between the accounts of {}", book.name) };
    let mut o = format!(
        "<svg class=\"flow\" viewBox=\"0 0 {} {}\" width=\"{}\" role=\"img\" aria-labelledby=\"{id}-t\"><title id=\"{id}-t\">{}</title>\n",
        n(f.w),
        n(f.h),
        n(f.w),
        html_escape(title.get(lang))
    );
    o.push_str(&format!(
        "<defs><marker id=\"{id}-a\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"7\" markerHeight=\"7\" orient=\"auto-start-reverse\"><path class=\"head\" d=\"M0 0 L10 5 L0 10 z\"/></marker>\
<marker id=\"{id}-ok\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"6\" markerHeight=\"6\" orient=\"auto-start-reverse\"><path class=\"head ok\" d=\"M0 0 L10 5 L0 10 z\"/></marker>\
<marker id=\"{id}-no\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"6\" markerHeight=\"6\" orient=\"auto-start-reverse\"><path class=\"head no\" d=\"M0 0 L10 5 L0 10 z\"/></marker></defs>\n"
    ));
    for e in &f.edges {
        o.push_str(&format!(
            "<g class=\"edge{}\" data-k=\"{}\"><path d=\"{}\" marker-end=\"url(#{id}-a)\" data-a=\"url(#{id}-a)\" data-ok=\"url(#{id}-ok)\" data-no=\"url(#{id}-no)\"/></g>\n",
            if e.pending { " pending" } else { "" },
            e.k,
            e.d
        ));
    }
    for e in &f.edges {
        o.push_str(&format!(
            "<g class=\"label\" data-k=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"16\" rx=\"3\"/><text x=\"{}\" y=\"{}\" text-anchor=\"middle\">{}</text></g>\n",
            e.k,
            n(e.lx - e.lw / 2.0),
            n(e.ly - 9.0),
            n(e.lw),
            n(e.lx),
            n(e.ly + 3.0),
            html_escape(&e.label)
        ));
    }
    for nd in &f.nodes {
        let rx = if nd.outside { 16 } else { 4 };
        o.push_str(&format!(
            "<g class=\"node{}\" data-a=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"{rx}\"/>",
            if nd.outside { " outside" } else { "" },
            nd.k,
            n(nd.x),
            n(nd.y),
            n(nd.w),
            n(nd.h)
        ));
        let first = nd.y + (nd.h - nd.lines.len() as f64 * LINE) / 2.0 + 13.0;
        for (j, l) in nd.lines.iter().enumerate() {
            o.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" text-anchor=\"middle\"{}>{}</text>",
                n(nd.x + nd.w / 2.0),
                n(first + j as f64 * LINE),
                if j == 0 { " class=\"name\"" } else { " class=\"sub\"" },
                html_escape(l)
            ));
        }
        o.push_str("</g>\n");
    }
    o.push_str("</svg>");
    o
}

// ── the life of a hold ────────────────────────────────────────────────────

/// The life of a hold of `t` as SVG: held, then posted, voided, or expired (when it can), and
/// under each end what `post` and `void` are refused with there.
pub fn life_svg(t: &TransferKind, lang: Lang) -> String {
    use crate::interp::HoldState as H;
    let st = |s: H| crate::scenario::state_text(s).get(lang).to_string();
    let expiring = matches!(t.pending, Some(Expiry::After(_)));
    let mut ends: Vec<(H, String, Vec<String>)> = vec![
        (
            H::Posted,
            tr!("確定（post）", "post").get(lang).to_string(),
            vec![tr!("取消は already_posted で拒否される", "void is refused: already_posted").get(lang).to_string(), tr!("違う額の確定は key_conflict で拒否される", "post for other amounts: key_conflict").get(lang).to_string()],
        ),
        (H::Voided, tr!("取消（void）", "void").get(lang).to_string(), vec![tr!("確定は already_voided で拒否される", "post is refused: already_voided").get(lang).to_string()]),
    ];
    if expiring {
        ends.push((H::Expired, doc::expiry_text(t, lang).unwrap_or_default(), vec![tr!("確定も取消も expired で拒否される", "post and void are refused: expired").get(lang).to_string()]));
    }
    let hold_label = format!("{}.hold", t.name);
    let held_w = text_w(&st(H::Held), FONT + 1.0).max(60.0) + 36.0;
    let x_held = 34.0 + text_w(&hold_label, LABEL).max(60.0) + 24.0;
    let arrow_w = ends.iter().map(|e| text_w(&e.1, LABEL)).fold(0.0, f64::max) + 70.0;
    let x_end = x_held + held_w + arrow_w.max(120.0);
    let end_w = ends.iter().map(|e| text_w(&st(e.0), FONT + 1.0)).fold(0.0, f64::max).max(60.0) + 36.0;
    let note_w = ends.iter().flat_map(|e| e.2.iter().map(|l| text_w(l, 11.0))).fold(0.0, f64::max);
    let row = 74.0;
    let box_h = 34.0;
    let top = 14.0;
    let total_h = top + ends.len() as f64 * row + 4.0;
    let cy = top + (ends.len() as f64 * row - (row - box_h)) / 2.0;
    let width = x_end + end_w.max(note_w) + 24.0;
    let mut o = format!(
        "<svg class=\"life\" viewBox=\"0 0 {} {}\" width=\"{}\" role=\"img\" aria-label=\"{}\">\n<defs><marker id=\"life-{}\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"7\" markerHeight=\"7\" orient=\"auto-start-reverse\"><path class=\"head\" d=\"M0 0 L10 5 L0 10 z\"/></marker></defs>\n",
        n(width),
        n(total_h),
        n(width),
        html_escape(tr!("{} の仮押さえのライフサイクル", "The life of a hold of {}", t.name).get(lang)),
        html_escape(&t.name)
    );
    let mk = format!("url(#life-{})", html_escape(&t.name));
    // the start, and the hold
    o.push_str(&format!("<circle class=\"start\" cx=\"16\" cy=\"{}\" r=\"6\"/>\n", n(cy + box_h / 2.0)));
    o.push_str(&format!(
        "<path class=\"arrow\" d=\"M22 {y} L{} {y}\" marker-end=\"{mk}\"/><text class=\"lbl\" x=\"{}\" y=\"{}\" text-anchor=\"middle\">{}</text>\n",
        n(x_held - 2.0),
        n((22.0 + x_held) / 2.0),
        n(cy + box_h / 2.0 - 7.0),
        html_escape(&hold_label),
        y = n(cy + box_h / 2.0)
    ));
    o.push_str(&format!(
        "<g class=\"state\" data-state=\"held\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{box_h}\" rx=\"8\"/><text x=\"{}\" y=\"{}\" text-anchor=\"middle\">{}</text></g>\n",
        n(x_held),
        n(cy),
        n(held_w),
        n(x_held + held_w / 2.0),
        n(cy + 22.0),
        html_escape(&st(H::Held))
    ));
    for (j, (s, label, notes)) in ends.iter().enumerate() {
        let y = top + j as f64 * row;
        let (x1, y1) = (x_held + held_w, cy + box_h * (j as f64 + 1.0) / (ends.len() as f64 + 1.0));
        let (x2, y2) = (x_end, y + box_h / 2.0);
        let dx = (x2 - x1) / 2.0;
        o.push_str(&format!(
            "<path class=\"arrow{}\" d=\"M{} {} C{} {}, {} {}, {} {}\" marker-end=\"{mk}\"/>",
            if *s == H::Expired { " external" } else { "" },
            n(x1),
            n(y1),
            n(x1 + dx),
            n(y1),
            n(x2 - dx),
            n(y2),
            n(x2 - 2.0),
            n(y2)
        ));
        // the label sits on the side of the arrow's last stretch that the arrow does not come from
        let ly = if y1 < y2 - 4.0 { y2 + 16.0 } else { y2 - 7.0 };
        let lw = text_w(label, LABEL) + 8.0;
        o.push_str(&format!(
            "<rect class=\"lblbg\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"15\" rx=\"3\"/><text class=\"lbl\" x=\"{}\" y=\"{}\" text-anchor=\"end\">{}</text>\n",
            n(x2 - 10.0 - lw + 4.0),
            n(ly - 11.0),
            n(lw),
            n(x2 - 10.0),
            n(ly),
            html_escape(label)
        ));
        o.push_str(&format!(
            "<g class=\"state end\" data-state=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{box_h}\" rx=\"8\"/><rect class=\"inner\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" rx=\"6\"/><text x=\"{}\" y=\"{}\" text-anchor=\"middle\">{}</text></g>\n",
            s.name(),
            n(x_end),
            n(y),
            n(end_w),
            n(x_end + 3.0),
            n(y + 3.0),
            n(end_w - 6.0),
            n(box_h - 6.0),
            n(x_end + end_w / 2.0),
            n(y + 22.0),
            html_escape(&st(*s))
        ));
        for (q, line) in notes.iter().enumerate() {
            o.push_str(&format!("<text class=\"note\" x=\"{}\" y=\"{}\">{}</text>\n", n(x_end + 2.0), n(y + box_h + 14.0 + q as f64 * 14.0), html_escape(line)));
        }
    }
    o.push_str("</svg>");
    o
}

// ── the page ──────────────────────────────────────────────────────────────

/// The colours of the page: ritsu-base's palette (whose values were chobo's), and the background
/// of the step a scenario is at, chobo's own.
fn palette() -> Palette {
    Palette::default().set("lit", "#fff8c5", "#3b2e00")
}

/// The rules, after the palette's variables.
const STYLE: &str = r#"* { box-sizing: border-box; }
body { margin: 0; background: var(--bg); color: var(--fg); font: 15px/1.6 system-ui, -apple-system, "Segoe UI", "Hiragino Sans", "Noto Sans JP", sans-serif; }
header, main { max-width: 1040px; margin: 0 auto; padding: 0 16px; }
header { padding-top: 12px; }
h1 { font-size: 1.6em; margin: 0.4em 0 0.2em; }
h2 { font-size: 1.3em; margin: 1.6em 0 0.5em; padding-bottom: 0.2em; border-bottom: 1px solid var(--line); }
h3 { font-size: 1.12em; margin: 1.4em 0 0.4em; }
h4 { font-size: 1em; margin: 1em 0 0.3em; }
p, li { overflow-wrap: break-word; }
nav a { color: var(--accent); }
.muted { color: var(--dim); }
code { font: 0.9em ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; background: var(--code); padding: 0 0.25em; border-radius: 4px; }
pre { font: 13px/1.5 ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; background: var(--code); padding: 8px 12px; border-radius: 6px; overflow-x: auto; }
pre code { background: none; padding: 0; }
.scroll { overflow-x: auto; max-width: 100%; }
table { border-collapse: collapse; margin: 0.4em 0 0.8em; }
th, td { border: 1px solid var(--line); padding: 4px 9px; text-align: left; vertical-align: top; min-width: 4.5em; }
.states td:first-child { white-space: nowrap; }
th { background: var(--soft); }
td { overflow-wrap: break-word; }
td code { white-space: nowrap; }
figure { margin: 0.6em 0; overflow-x: auto; max-width: 100%; }
svg { display: block; max-width: none; height: auto; }
svg text { fill: var(--fg); font-size: 13px; }
svg .name { font-weight: 600; font-size: 14px; }
svg .sub { fill: var(--dim); }
.node rect { fill: var(--soft); stroke: var(--fg); stroke-width: 1.2; }
.node.outside rect { stroke: var(--dim); stroke-dasharray: 5 3; }
.edge path { fill: none; stroke: var(--dim); stroke-width: 1.5; }
.edge.pending path { stroke-dasharray: 6 4; }
.head { fill: var(--dim); }
.head.ok { fill: var(--ok); }
.head.no { fill: var(--bad); }
.label rect { fill: var(--bg); opacity: 0.92; }
.label text { font-size: 12px; }
.edge.lit path { stroke: var(--ok); stroke-width: 3; }
.edge.no path { stroke: var(--bad); stroke-width: 3; }
.label.lit text { fill: var(--ok); font-weight: 600; }
.label.no text { fill: var(--bad); font-weight: 600; }
.life .state rect { fill: var(--soft); stroke: var(--fg); stroke-width: 1.2; }
.life .state rect.inner { fill: none; stroke-width: 0.8; }
.life .arrow { fill: none; stroke: var(--dim); stroke-width: 1.5; }
.life .arrow.external { stroke-dasharray: 5 3; }
.life .start { fill: var(--fg); }
.life .lbl { font-size: 12px; }
.life .lblbg { fill: var(--bg); opacity: 0.9; }
.life .note { font-size: 11px; fill: var(--dim); }
details { margin: 0.5em 0; }
summary { cursor: pointer; color: var(--accent); }
.player { border: 1px solid var(--line); border-radius: 8px; padding: 10px 12px; margin: 0.8em 0; }
.player select { max-width: 100%; font: inherit; padding: 2px 4px; }
.pick { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
.outs { display: flex; flex-wrap: wrap; gap: 6px; margin: 8px 0 0; }
.outs button, .ctl button { font: inherit; padding: 2px 10px; border: 1px solid var(--line); border-radius: 6px; background: var(--soft); color: var(--fg); cursor: pointer; }
.outs button[aria-pressed="true"] { border-color: var(--accent); color: var(--accent); font-weight: 600; }
.cols { display: flex; flex-wrap: wrap; gap: 12px 24px; margin-top: 8px; }
.cols > * { flex: 1 1 320px; min-width: 0; }
#steps { margin: 0; padding-left: 1.8em; }
#steps li { padding: 2px 6px; border-radius: 4px; cursor: pointer; }
#steps li.now { background: var(--lit); }
#steps li.later { color: var(--dim); }
#steps .call { font: 13px ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; overflow-wrap: anywhere; }
#steps .res { display: block; font-size: 0.92em; }
#steps .res.ok { color: var(--ok); }
#steps .res.no { color: var(--bad); }
#steps .why { display: block; font-size: 0.85em; color: var(--dim); }
.ctl { display: flex; gap: 8px; align-items: center; margin-bottom: 6px; }
#bal td.changed { background: var(--lit); font-weight: 600; }
#bal td.num { text-align: right; font-variant-numeric: tabular-nums; }
"#;

const SCRIPT: &str = r#"
(function () {
  var data = JSON.parse(document.getElementById('chobo-data').textContent);
  var w = data.words;
  var sel = document.getElementById('sc');
  var at = { sc: 0, out: 0, step: 0 };
  function el(tag, cls, text) { var e = document.createElement(tag); if (cls) e.className = cls; if (text != null) e.textContent = text; return e; }
  function steps() { return data.scenarios[at.sc].outcomes[at.out]; }
  function after(j) { var sc = data.scenarios[at.sc]; return j === 0 ? { balances: sc.zero, holds: [] } : steps()[j - 1]; }
  function readHash() {
    var h = {}; location.hash.replace(/^#/, '').split('&').forEach(function (kv) { var p = kv.split('='); if (p.length === 2) h[p[0]] = parseInt(p[1], 10); });
    if (h.scenario >= 1 && h.scenario <= data.scenarios.length) at.sc = h.scenario - 1;
    var outs = data.scenarios[at.sc].outcomes.length;
    at.out = h.outcome >= 1 && h.outcome <= outs ? h.outcome - 1 : 0;
    var n = steps().length;
    at.step = h.step >= 0 && h.step <= n ? h.step : n;
  }
  function writeHash() {
    var s = '#scenario=' + (at.sc + 1) + '&step=' + at.step + (data.scenarios[at.sc].outcomes.length > 1 ? '&outcome=' + (at.out + 1) : '');
    // A page opened where the address cannot be rewritten (a sandboxed frame) still steps; it just
    // cannot be linked.
    if (location.hash !== s) { try { history.replaceState(null, '', s); } catch (e) {} }
  }
  function render() {
    var sc = data.scenarios[at.sc];
    sel.value = String(at.sc);
    var outs = document.getElementById('outs');
    outs.textContent = '';
    if (sc.outcomes.length > 1) {
      outs.appendChild(el('span', 'muted', w.ways.replace('{n}', sc.outcomes.length)));
      sc.outcomes.forEach(function (_, k) {
        var b = el('button', null, w.outcome.replace('{n}', k + 1));
        b.setAttribute('aria-pressed', k === at.out ? 'true' : 'false');
        b.onclick = function () { at.out = k; at.step = Math.min(at.step, steps().length); go(); };
        outs.appendChild(b);
      });
    }
    var list = document.getElementById('steps');
    list.textContent = '';
    steps().forEach(function (s, j) {
      var li = el('li', j + 1 === at.step ? 'now' : (j + 1 > at.step ? 'later' : ''));
      li.setAttribute('data-step', j + 1);
      if (s.callers.length) {
        li.appendChild(el('span', 'call', 'together'));
        s.callers.forEach(function (cs, c) {
          cs.forEach(function (x) {
            li.appendChild(el('span', 'call res', w.caller.replace('{n}', c + 1) + ': ' + x.call));
            li.appendChild(el('span', 'res ' + (x.refused ? 'no' : 'ok'), x.result));
            if (x.detail) li.appendChild(el('span', 'why', x.detail));
          });
        });
      } else {
        li.appendChild(el('span', 'call', s.label));
        if (s.result) li.appendChild(el('span', 'res ' + (s.refused ? 'no' : 'ok'), s.result));
        if (s.detail) li.appendChild(el('span', 'why', s.detail));
      }
      li.onclick = function () { at.step = j + 1; go(); };
      list.appendChild(li);
    });
    var n = steps().length;
    document.getElementById('pos').textContent = at.step === 0 ? w.before : w.pos.replace('{i}', at.step).replace('{n}', n);
    document.getElementById('prev').disabled = at.step === 0;
    document.getElementById('next').disabled = at.step === n;
    var now = after(at.step), was = at.step > 0 ? after(at.step - 1) : null;
    var body = document.getElementById('bal-body');
    body.textContent = '';
    sc.accounts.forEach(function (name, a) {
      var tr = el('tr');
      tr.appendChild(el('td', null, name));
      for (var q = 0; q < 3; q++) {
        var changed = was && was.balances[a][q] !== now.balances[a][q];
        tr.appendChild(el('td', 'num' + (changed ? ' changed' : ''), now.balances[a][q]));
      }
      body.appendChild(tr);
    });
    var hb = document.getElementById('holds-body');
    hb.textContent = '';
    if (!now.holds.length) {
      var tr = el('tr'); var td = el('td', 'muted', w.noholds); td.colSpan = 2; tr.appendChild(td); hb.appendChild(tr);
    }
    now.holds.forEach(function (h) { var tr = el('tr'); tr.appendChild(el('td', null, h[0])); tr.appendChild(el('td', null, h[1])); hb.appendChild(tr); });
    var lit = {};
    if (at.step > 0) steps()[at.step - 1].kinds.forEach(function (k) { lit[k[0]] = lit[k[0]] === 'lit' || k[1] ? 'lit' : 'no'; });
    document.querySelectorAll('#mini [data-k]').forEach(function (g) {
      var how = lit[g.getAttribute('data-k')];
      g.classList.remove('lit', 'no');
      if (how) g.classList.add(how);
      var p = g.querySelector('path');
      if (p) p.setAttribute('marker-end', p.getAttribute(how === 'lit' ? 'data-ok' : how === 'no' ? 'data-no' : 'data-a'));
    });
  }
  function go() { writeHash(); render(); }
  sel.onchange = function () { at.sc = parseInt(sel.value, 10); at.out = 0; at.step = 0; go(); };
  document.getElementById('prev').onclick = function () { if (at.step > 0) { at.step--; go(); } };
  document.getElementById('next').onclick = function () { if (at.step < steps().length) { at.step++; go(); } };
  document.getElementById('player').addEventListener('keydown', function (e) {
    if (e.target.tagName === 'SELECT') return;
    if (e.key === 'ArrowRight' && at.step < steps().length) { at.step++; go(); e.preventDefault(); }
    if (e.key === 'ArrowLeft' && at.step > 0) { at.step--; go(); e.preventDefault(); }
  });
  window.addEventListener('hashchange', function () { readHash(); render(); });
  readHash();
  render();
})();
"#;

fn table_html(head: &[String], rows: &[Vec<String>]) -> String {
    let mut o = String::from("<div class=\"scroll\"><table><thead><tr>");
    for h in head {
        o.push_str(&format!("<th>{}</th>", inline(h)));
    }
    o.push_str("</tr></thead><tbody>\n");
    for r in rows {
        o.push_str("<tr>");
        for c in r {
            let cell: Vec<String> = c.lines().map(inline).collect();
            o.push_str(&format!("<td>{}</td>", cell.join("<br>")));
        }
        o.push_str("</tr>\n");
    }
    o.push_str("</tbody></table></div>\n");
    o
}

pub fn page(i: &Input) -> String {
    let (book, lang) = (i.book, i.lang);
    let words = |t: Text| t.get(lang).to_string();
    let mut o = String::new();
    let css = format!("{}{STYLE}", palette().css());
    o.push_str(&docpage::html_head(lang, &format!("chobo {}", env!("CARGO_PKG_VERSION")), &format!("{} v{}", book.name, book.version), &css));
    // the header
    o.push_str(&format!("<header>\n<h1>{} v{}</h1>\n", html_escape(&book.name), book.version));
    if let Some(d) = &book.description {
        o.push_str(&format!("<p>{}</p>\n", inline(d)));
    }
    let intro = doc::intro(i);
    o.push_str(&format!("<p class=\"muted\">{}</p>\n", inline(&intro)));
    o.push_str(&format!(
        "<nav>{}</nav>\n</header>\n<main>\n",
        [("accounts", tr!("勘定", "Accounts")), ("flow", tr!("流れ", "How things move")), ("transfers", tr!("振替", "Transfers")), ("scenarios", tr!("シナリオ", "Scenarios"))]
            .iter()
            .map(|(id, t)| format!("<a href=\"#{id}\">{}</a>", t.get(lang)))
            .collect::<Vec<_>>()
            .join(" · ")
    ));

    // the warnings
    if !i.diags.is_empty() {
        o.push_str(&format!("<section id=\"warnings\">\n<h2>{}</h2>\n", words(tr!("検査の警告", "What the check warns about"))));
        for d in i.diags {
            o.push_str(&format!("<pre>{}</pre>\n", html_escape(&d.shown(i.file, i.src, lang))));
        }
        o.push_str("</section>\n");
    }

    // the accounts
    o.push_str(&format!("<section id=\"accounts\">\n<h2>{}</h2>\n", words(tr!("勘定", "Accounts"))));
    let head: Vec<String> = [tr!("勘定", "Account"), tr!("分け方", "One for each"), tr!("単位", "Unit"), tr!("境界", "Bounds"), tr!("説明", "What it is")].into_iter().map(words).collect();
    let rows: Vec<Vec<String>> = book
        .accounts
        .iter()
        .enumerate()
        .map(|(k, a)| {
            let per = if a.params.is_empty() {
                words(tr!("一つだけ", "one account"))
            } else {
                let ps: Vec<String> = a.params.iter().map(|p| format!("`{p}`")).collect();
                let ps = if lang == Lang::Ja { ps.join(" と ") } else { ps.join(", ") };
                tr!("{ps} ごと", "{ps}").get(lang).to_string()
            };
            vec![format!("`{}`", a.name), per, book.units[a.unit].name.clone(), doc::bounds_lines(book, k, lang).join("\n"), a.description.clone().unwrap_or_default()]
        })
        .collect();
    o.push_str(&table_html(&head, &rows));
    o.push_str("</section>\n");

    // the flow
    o.push_str(&format!("<section id=\"flow\">\n<h2>{}</h2>\n<figure>{}</figure>\n", words(tr!("勘定のあいだの流れ", "How things move")), flow_svg(book, lang, "flow")));
    o.push_str(&format!(
        "<p class=\"muted\">{}</p>\n</section>\n",
        inline(&words(tr!(
            "四角は勘定で、矢印は振替の移動。角の丸い破線の四角は外の勘定で、境界を持たない。破線の矢印は仮押さえの振替で、まず押さえ、確定したときに動く。",
            "A box is an account, and an arrow a move of a transfer. A rounded, dashed box is an account outside the book, which has no bounds. A dashed arrow belongs to a transfer that holds first, and moves when the hold is posted."
        )))
    ));

    // the transfers
    o.push_str(&format!("<section id=\"transfers\">\n<h2>{}</h2>\n", words(tr!("振替", "Transfers"))));
    for (k, t) in book.transfers.iter().enumerate() {
        o.push_str(&format!("<article id=\"t{k}\">\n<h3>{}</h3>\n", html_escape(&t.name)));
        if let Some(d) = &t.description {
            o.push_str(&format!("<p>{}</p>\n", inline(d)));
        }
        o.push_str("<ul>\n");
        for f in doc::transfer_facts(i, k) {
            let mut lines = f.lines();
            o.push_str(&format!("<li>{}", inline(lines.next().unwrap_or(""))));
            let rest: Vec<&str> = lines.collect();
            if !rest.is_empty() {
                o.push_str("<ol>");
                for l in rest {
                    let l = l.split_once(". ").map(|x| x.1).unwrap_or(l);
                    o.push_str(&format!("<li>{}</li>", inline(l)));
                }
                o.push_str("</ol>");
            }
            o.push_str("</li>\n");
        }
        o.push_str("</ul>\n");
        if t.is_pending() {
            o.push_str(&format!("<figure>{}</figure>\n", life_svg(t, lang)));
            let head: Vec<String> = [tr!("仮押さえの状態", "The hold"), tr!("post（確定）", "post"), tr!("void（取消）", "void")].into_iter().map(words).collect();
            let rows: Vec<Vec<String>> = doc::life_rows(t, lang).into_iter().map(|r| r.to_vec()).collect();
            o.push_str(&table_html(&head, &rows).replacen("<table>", "<table class=\"states\">", 1));
        }
        let head: Vec<String> = [tr!("操作", "Operation"), tr!("拒否されうる理由", "May be refused with"), tr!("いつ", "When")].into_iter().map(words).collect();
        let rows: Vec<Vec<String>> = doc::refusal_rows(i, k).into_iter().map(|r| r.to_vec()).collect();
        o.push_str(&table_html(&head, &rows));
        let examples = doc::examples_of(i, k);
        if !examples.is_empty() {
            o.push_str(&format!("<details><summary>{}</summary>\n", words(tr!("拒否される例", "How each refusal comes about"))));
            for (head, lines) in examples {
                o.push_str(&format!("<h4>{}</h4>\n<pre>{}</pre>\n", html_escape(&head), html_escape(&lines.join("\n"))));
            }
            o.push_str("</details>\n");
        }
        o.push_str("</article>\n");
    }
    o.push_str("</section>\n");

    // the scenarios
    let data = doc::page_data(i);
    let count = data["scenarios"].as_array().map(|a| a.len()).unwrap_or(0);
    o.push_str(&format!("<section id=\"scenarios\">\n<h2>{}</h2>\n", words(tr!("シナリオ", "Scenarios"))));
    o.push_str(&format!(
        "<p>{}</p>\n",
        inline(tr!(
            "`chobo scenarios` が帳簿から作ったシナリオ {count} 本。一つ選び、ステップを一つずつ進めると、そのあとの残高と仮押さえが見られる。図では、そのステップで呼んだ振替の矢印に色が付く（拒否されたものは赤）。",
            "{count} scenarios, which `chobo scenarios` makes from the book. Pick one and step through it: after each step come the balances and the holds it left, and on the chart the arrows of the transfers it called light up (red when refused)."
        ).get(lang))
    ));
    o.push_str(&format!("<div class=\"player\" id=\"player\">\n<div class=\"pick\"><label for=\"sc\">{}</label><select id=\"sc\">\n", words(tr!("シナリオ", "Scenario"))));
    if let Some(list) = data["scenarios"].as_array() {
        for (j, s) in list.iter().enumerate() {
            // an option holds plain text: the title without its back-quotes
            o.push_str(&format!("<option value=\"{j}\">{}. {}</option>\n", j + 1, html_escape(&s["title"].as_str().unwrap_or("").replace('`', ""))));
        }
    }
    o.push_str("</select></div>\n<div class=\"outs\" id=\"outs\"></div>\n<div class=\"cols\">\n<ol id=\"steps\"></ol>\n<div>\n");
    o.push_str(&format!(
        "<div class=\"ctl\"><button id=\"prev\" type=\"button\" aria-label=\"{}\">◀</button><span id=\"pos\"></span><button id=\"next\" type=\"button\" aria-label=\"{}\">▶</button></div>\n",
        words(tr!("前へ", "previous step")),
        words(tr!("次へ", "next step"))
    ));
    o.push_str(&format!(
        "<div class=\"scroll\"><table id=\"bal\"><thead><tr><th>{}</th><th>{}</th><th>{}</th><th>{}</th></tr></thead><tbody id=\"bal-body\"></tbody></table></div>\n",
        words(tr!("勘定", "Account")),
        words(tr!("確定", "Posted")),
        words(tr!("出ていく仮押さえ", "Held out")),
        words(tr!("入ってくる仮押さえ", "Held in"))
    ));
    o.push_str(&format!(
        "<div class=\"scroll\"><table id=\"holds\"><thead><tr><th>{}</th><th>{}</th></tr></thead><tbody id=\"holds-body\"></tbody></table></div>\n</div>\n</div>\n",
        words(tr!("仮押さえ", "Hold")),
        words(tr!("状態", "State"))
    ));
    o.push_str(&format!("<figure id=\"mini\">{}</figure>\n</div>\n</section>\n</main>\n", flow_svg(book, lang, "mini")));
    let mut data = data;
    data["words"] = json!({
        "ways": words(tr!("同時の操作の順序によって、結果は {{n}} 通り:", "It can come out {{n}} ways:")),
        "outcome": words(tr!("結果 {{n}}", "outcome {{n}}")),
        "caller": words(tr!("呼び出し元 {{n}}", "caller {{n}}")),
        "before": words(tr!("最初のステップの前。どの残高も 0", "before the first step: every balance is 0")),
        "pos": words(tr!("{{n}} ステップのうち {{i}} つ目のあと", "after step {{i}} of {{n}}")),
        "noholds": words(tr!("仮押さえは無い", "no holds")),
    });
    let json = serde_json::to_string(&data).unwrap().replace("</", "<\\/");
    o.push_str(&format!("<script type=\"application/json\" id=\"chobo-data\">{json}</script>\n<script>{SCRIPT}</script>\n</body>\n</html>\n"));
    o
}
