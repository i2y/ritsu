//! The picture of `doc`'s HTML page, drawn as SVG without a layout engine: a `.flow` has no jumps,
//! so its picture is laid out as its blocks nest. A block's statements stand one under another on
//! one axis; a match's arms and a call's handlers stand side by side, the first under the match and
//! the others to its right, and come back to the axis below; a loop is a frame around its body, with
//! the way round on its left and the way out by `break` on its right. The text is monospace, so the
//! width of a label is known here without a browser.

use crate::doc::{Cond, EdgeInfo, Graph, Kind, Panel, Piece};

/// The width of a character of the labels: 12px monospace, and twice that for a wide character.
const CH: f64 = 7.3;
const WIDE: f64 = 12.6;
const LH: f64 = 16.0;
const PAD_X: f64 = 10.0;
const PAD_Y: f64 = 6.0;
/// between two statements of a block
const GAP_Y: f64 = 28.0;
/// between two columns
const GAP_X: f64 = 26.0;

fn wide(c: char) -> bool {
    matches!(c as u32, 0x1100..=0x115F | 0x2E80..=0x303E | 0x3041..=0x33FF | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xA000..=0xA4CF | 0xAC00..=0xD7A3 | 0xF900..=0xFAFF | 0xFE30..=0xFE4F | 0xFF00..=0xFF60 | 0xFFE0..=0xFFE6)
}

pub fn text_width(s: &str) -> f64 {
    s.chars().map(|c| if wide(c) { WIDE } else { CH }).sum()
}

/// An edge's label in lines of about 30 characters, broken after its commas.
fn label_lines(s: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for part in s.split(", ") {
        match out.last_mut() {
            Some(l) if l.chars().count() + 2 + part.chars().count() <= 30 => {
                l.push_str(", ");
                l.push_str(part);
            }
            Some(l) => {
                l.push(',');
                out.push(part.to_string());
            }
            None => out.push(part.to_string()),
        }
    }
    out
}

fn label_width(s: &str) -> f64 {
    label_lines(s).iter().map(|l| text_width(l)).fold(0.0, f64::max)
}

/// The height the lines of a label take below its first.
const LABEL_LH: f64 = 13.0;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Something drawn, in the coordinates of the block that holds it.
#[derive(Clone)]
enum Shape {
    Node { n: usize, x: f64, y: f64, w: f64, h: f64 },
    Frame { id: String, x: f64, y: f64, w: f64, h: f64, title: String, footer: Option<String>, title_x: f64 },
    Edge { id: usize, pts: Vec<(f64, f64)>, dashed: bool, label: Option<(String, f64, f64)>, from: String, to: String, conds: Vec<Cond>, arrow: bool },
}

/// A way out of a block that has yet to be joined to what comes next.
#[derive(Clone)]
struct Loose {
    from: String,
    conds: Vec<Cond>,
    label: Option<(String, f64, f64)>,
    dashed: bool,
    pts: Vec<(f64, f64)>,
}

impl Loose {
    fn last(&self) -> (f64, f64) {
        *self.pts.last().unwrap()
    }
}

/// A block laid out: its size, its axis, what is drawn in it, and its ways out.
struct Lay {
    w: f64,
    h: f64,
    ax: f64,
    shapes: Vec<Shape>,
    loose: Vec<Loose>,
    breaks: Vec<Loose>,
    /// the node the way in goes to
    entry: String,
}

impl Lay {
    fn shift(&mut self, dx: f64, dy: f64) {
        for s in &mut self.shapes {
            match s {
                Shape::Node { x, y, .. } | Shape::Frame { x, y, .. } => {
                    *x += dx;
                    *y += dy;
                }
                Shape::Edge { pts, label, .. } => {
                    for p in pts.iter_mut() {
                        p.0 += dx;
                        p.1 += dy;
                    }
                    if let Some((_, lx, ly)) = label {
                        *lx += dx;
                        *ly += dy;
                    }
                }
            }
            if let Shape::Frame { title_x, .. } = s {
                *title_x += dx;
            }
        }
        for l in self.loose.iter_mut().chain(self.breaks.iter_mut()) {
            for p in l.pts.iter_mut() {
                p.0 += dx;
                p.1 += dy;
            }
            if let Some((_, lx, ly)) = &mut l.label {
                *lx += dx;
                *ly += dy;
            }
        }
    }
}

struct Drawer<'g> {
    g: &'g Graph,
    edges: usize,
}

impl<'g> Drawer<'g> {
    fn size(&self, n: usize) -> (f64, f64) {
        let node = &self.g.nodes[n];
        let widths: Vec<f64> = node.lines.iter().enumerate().map(|(k, l)| text_width(l) + if k == 0 { number_width(node.line) } else { 0.0 }).collect();
        let tw = widths.iter().cloned().fold(0.0, f64::max);
        let h = node.lines.len() as f64 * LH + 2.0 * PAD_Y;
        let w = match node.kind {
            Kind::Match => tw + 2.0 * PAD_X + 24.0,
            Kind::Rule => tw + 2.0 * PAD_X + 12.0,
            Kind::Receive => tw + 2.0 * PAD_X + 16.0,
            Kind::Wait | Kind::Start | Kind::Succeed | Kind::Fail | Kind::End | Kind::Entry | Kind::Break => tw + 2.0 * PAD_X + 12.0,
            Kind::Call | Kind::Let => tw + 2.0 * PAD_X,
        };
        let h = if node.kind == Kind::Match { h + 6.0 } else { h };
        (w.max(56.0).round(), h.round())
    }

    fn edge(&mut self, l: &Loose, to: &str, tail: Vec<(f64, f64)>, arrow: bool) -> Shape {
        self.edges += 1;
        let mut pts = l.pts.clone();
        for p in tail {
            if pts.last() != Some(&p) {
                pts.push(p);
            }
        }
        Shape::Edge { id: self.edges, pts, dashed: l.dashed, label: l.label.clone(), from: l.from.clone(), to: to.to_string(), conds: l.conds.clone(), arrow }
    }

    fn node_lay(&self, n: usize) -> Lay {
        let (w, h) = self.size(n);
        let id = self.g.nodes[n].id.clone();
        Lay { w, h, ax: w / 2.0, shapes: vec![Shape::Node { n, x: 0.0, y: 0.0, w, h }], loose: vec![Loose { from: id.clone(), conds: vec![], label: None, dashed: false, pts: vec![(w / 2.0, h)] }], breaks: vec![], entry: id }
    }

    fn piece(&mut self, p: &Piece) -> Lay {
        match p {
            Piece::Node(n) => self.node_lay(*n),
            Piece::Stop(n) => {
                let mut l = self.node_lay(*n);
                l.loose.clear();
                l
            }
            Piece::Break(n) => {
                let mut l = self.node_lay(*n);
                l.breaks = std::mem::take(&mut l.loose);
                l
            }
            Piece::Call { node, handlers } => {
                let site = self.g.nodes[*node].site.unwrap_or(0);
                let mut l = self.node_lay(*node);
                l.loose[0].conds.push(Cond::Ok(site));
                self.branches(l, handlers, true)
            }
            Piece::Match { node, arms } => {
                let mut l = self.node_lay(*node);
                l.loose.clear();
                self.branches(l, arms, false)
            }
            Piece::Loop { site, title, footer, body, parallel, exit } => self.frame(*site, title, footer.as_deref(), body, *parallel, exit),
        }
    }

    /// A match's arms, or a call's handlers, beside and below the node laid out in `head`.
    fn branches(&mut self, mut head: Lay, branches: &[crate::doc::Branch], beside: bool) -> Lay {
        let (nw, nh) = (head.w, head.h);
        let id = head.entry.clone();
        let is_match = !beside;
        // the columns, each with its block (None for an empty one), its width and its axis
        let mut cols: Vec<(Option<Lay>, f64, f64)> = Vec::new();
        let mut label_rows: usize = 1;
        for b in branches {
            label_rows = label_rows.max(label_lines(&b.label).len());
            let lw = label_width(&b.label);
            let body = if b.body.is_empty() { None } else { Some(self.block(&b.body)) };
            let (w, ax) = match &body {
                Some(l) => (l.w.max(l.ax + 6.0 + lw + 10.0), l.ax),
                None => (6.0 + 6.0 + lw + 10.0, 6.0),
            };
            cols.push((body, w, ax));
        }
        let col_top = nh + 26.0 + (label_rows - 1) as f64 * LABEL_LH;
        // Under a match goes the first arm that goes on to what follows, so that the way on runs
        // straight down; the other arms stand to the right, in the order they are written. A call's
        // answer goes on under it, and its handlers stand to the right.
        let under: Option<usize> = if is_match { Some(cols.iter().position(|(b, _, _)| b.as_ref().is_none_or(|l| !l.loose.is_empty())).unwrap_or(0)) } else { None };
        let mut ax = nw / 2.0;
        if let Some(u) = under {
            ax = ax.max(cols[u].2);
        }
        let node_x = ax - nw / 2.0;
        let mut right = node_x + nw;
        let mut xs: Vec<f64> = vec![0.0; cols.len()];
        if let Some(u) = under {
            xs[u] = ax - cols[u].2;
            right = right.max(xs[u] + cols[u].1);
        }
        for (k, (_, w, _)) in cols.iter().enumerate() {
            if Some(k) == under {
                continue;
            }
            xs[k] = right + GAP_X;
            right = xs[k] + w;
        }
        head.shift(node_x, 0.0);
        let mut shapes = std::mem::take(&mut head.shapes);
        let mut loose = std::mem::take(&mut head.loose);
        let mut breaks = Vec::new();
        let mut h = nh;
        let side = (node_x + nw, nh / 2.0);
        for (k, (b, (body, _, cax))) in branches.iter().zip(cols.into_iter()).enumerate() {
            let cx = xs[k] + cax;
            let under = Some(k) == under;
            let (start, label_at) = if under { (vec![(ax, nh)], (ax + 6.0, nh + 14.0)) } else { (vec![side, (cx, side.1)], (cx + 6.0, side.1 + 15.0)) };
            let l = Loose { from: id.clone(), conds: vec![b.cond], label: Some((b.label.clone(), label_at.0, label_at.1)), dashed: b.dashed, pts: start };
            match body {
                Some(mut bl) => {
                    bl.shift(xs[k], col_top);
                    let e = self.edge(&l, &bl.entry, vec![(cx, col_top)], !bl.entry.starts_with('L'));
                    shapes.push(e);
                    h = h.max(col_top + bl.h);
                    shapes.extend(bl.shapes);
                    loose.extend(bl.loose);
                    breaks.extend(bl.breaks);
                }
                None => {
                    let mut l = l;
                    if !l.pts.contains(&(cx, col_top)) {
                        l.pts.push((cx, col_top));
                    }
                    h = h.max(col_top);
                    loose.push(l);
                }
            }
        }
        Lay { w: right, h, ax, shapes, loose, breaks, entry: id }
    }

    /// A loop: a frame around its body, with the way round on the left and `break` on the right.
    fn frame(&mut self, site: usize, title: &str, footer: Option<&str>, body: &[Piece], parallel: bool, exit: &str) -> Lay {
        let fid = format!("L{site}");
        let mut b = self.block(body);
        let (pl, pr, pt) = (26.0, 26.0, 34.0);
        let pb = if footer.is_some() { 36.0 } else { 20.0 };
        b.shift(pl, pt);
        let ax = pl + b.ax;
        let title_w = text_width(title);
        let footer_w = footer.map(text_width).unwrap_or(0.0);
        let w = (pl + b.w + pr).max(ax + 10.0 + title_w + 14.0).max(ax + 10.0 + footer_w + 14.0);
        let h = pt + b.h + pb;
        let mut shapes = vec![Shape::Frame { id: fid.clone(), x: 0.0, y: 0.0, w, h, title: title.to_string(), footer: footer.map(String::from), title_x: ax + 10.0 }];
        let first = b.entry.clone();
        // the way in comes to the frame, and goes on to the first step when a round begins
        let into = Loose { from: fid.clone(), conds: vec![], label: None, dashed: false, pts: vec![(ax, 0.0)] };
        let e = self.edge(&into, &first, vec![(ax, pt)], !first.starts_with('L'));
        shapes.push(e);
        shapes.append(&mut b.shapes);
        let bottom = pt + b.h + 10.0;
        for l in std::mem::take(&mut b.loose) {
            let (lx, _) = l.last();
            if parallel {
                // a round ends; the loop goes on when every round has
                let e = self.edge(&l, &fid, vec![(lx, bottom), (ax, bottom), (ax, h)], false);
                shapes.push(e);
            } else {
                let mut l = l;
                l.conds.push(Cond::Again(site));
                let e = self.edge(&l, &first, vec![(lx, bottom), (10.0, bottom), (10.0, pt - 10.0), (ax, pt - 10.0), (ax, pt)], true);
                shapes.push(e);
            }
        }
        let mut breaks = Vec::new();
        for mut l in std::mem::take(&mut b.breaks) {
            let (lx, _) = l.last();
            for p in [(lx, bottom - 4.0), (w - 10.0, bottom - 4.0), (w - 10.0, h + 4.0)] {
                l.pts.push(p);
            }
            breaks.push(l);
        }
        // the way out, with room below the frame for its label
        let out = Loose { from: fid.clone(), conds: vec![Cond::Done(site)], label: Some((exit.to_string(), ax + 6.0, h + 13.0)), dashed: false, pts: vec![(ax, h), (ax, h + 18.0)] };
        let mut loose = vec![out];
        loose.extend(breaks);
        Lay { w: w.max(ax + 6.0 + text_width(exit) + 8.0), h: h + 18.0, ax, shapes, loose, breaks: vec![], entry: fid }
    }

    /// The statements of a block, one under another on one axis.
    fn block(&mut self, pieces: &[Piece]) -> Lay {
        let lays: Vec<Lay> = pieces.iter().map(|p| self.piece(p)).collect();
        self.stack(lays)
    }

    fn panel(&mut self, p: &Panel) -> Lay {
        let mut pieces_lay = vec![self.node_lay(p.entry)];
        let body = self.block_or_empty(&p.body);
        if let Some(b) = body {
            pieces_lay.push(b);
        }
        // the end, when a way out is left
        let open = !pieces_lay.last().unwrap().loose.is_empty();
        if open {
            pieces_lay.push(self.node_lay(p.end));
            let last = pieces_lay.len() - 1;
            pieces_lay[last].loose.clear();
        }
        self.stack(pieces_lay)
    }

    fn block_or_empty(&mut self, pieces: &[Piece]) -> Option<Lay> {
        if pieces.is_empty() {
            None
        } else {
            Some(self.block(pieces))
        }
    }

    /// Laid-out statements, one under another on one axis: the ways out of each go on to the next.
    fn stack(&mut self, lays: Vec<Lay>) -> Lay {
        let ax = lays.iter().map(|l| l.ax).fold(0.0, f64::max);
        let w = lays.iter().map(|l| ax - l.ax + l.w).fold(0.0, f64::max);
        let mut shapes = Vec::new();
        let mut loose: Vec<Loose> = Vec::new();
        let mut breaks = Vec::new();
        let mut y = 0.0;
        let mut entry = String::new();
        for (k, mut l) in lays.into_iter().enumerate() {
            if k > 0 {
                y += GAP_Y;
            }
            l.shift(ax - l.ax, y);
            if k == 0 {
                entry = l.entry.clone();
            } else {
                let merge = y - 10.0;
                for lo in std::mem::take(&mut loose) {
                    let (lx, _) = lo.last();
                    let tail = if (lx - ax).abs() < 0.5 { vec![(ax, y)] } else { vec![(lx, merge), (ax, merge), (ax, y)] };
                    let e = self.edge(&lo, &l.entry, tail, !l.entry.starts_with('L'));
                    shapes.push(e);
                }
            }
            y += l.h;
            shapes.append(&mut l.shapes);
            loose = l.loose;
            breaks.append(&mut l.breaks);
        }
        Lay { w, h: y, ax, shapes, loose, breaks, entry }
    }
}

fn number_width(line: usize) -> f64 {
    if line == 0 {
        0.0
    } else {
        (line.to_string().len() as f64 + 1.0) * CH * 0.9
    }
}

/// One panel drawn: the SVG, and its edges for the runs to light up.
pub struct Drawn {
    pub svg: String,
    pub edges: Vec<EdgeInfo>,
}

pub fn panel(g: &Graph, p: &Panel, tick: &dyn Fn(&str) -> Option<String>) -> Drawn {
    let mut d = Drawer { g, edges: 0 };
    let mut l = d.panel(p);
    let margin = 14.0;
    l.shift(margin, margin);
    // a label may reach past the widest node
    let mut right = l.w + 2.0 * margin;
    for s in &l.shapes {
        if let Shape::Edge { label: Some((t, x, _)), .. } = s {
            right = right.max(x + label_width(t) + margin);
        }
    }
    let (w, h) = (right.ceil(), (l.h + 2.0 * margin + 16.0).ceil());
    let mut frames = String::new();
    let mut edges = String::new();
    let mut nodes = String::new();
    let mut infos = Vec::new();
    for s in &l.shapes {
        match s {
            Shape::Frame { id, x, y, w, h, title, footer, title_x } => {
                frames.push_str(&format!(
                    "<g class=\"frame\" data-id=\"{id}\" tabindex=\"0\"><rect class=\"shape\" x=\"{x:.1}\" y=\"{y:.1}\" width=\"{w:.1}\" height=\"{h:.1}\" rx=\"8\"/><text class=\"ft\" x=\"{:.1}\" y=\"{:.1}\">{}</text>",
                    title_x,
                    y + 20.0,
                    esc(title)
                ));
                if let Some(f) = footer {
                    frames.push_str(&format!("<text class=\"ft\" x=\"{:.1}\" y=\"{:.1}\">{}</text>", title_x, y + h - 12.0, esc(f)));
                }
                frames.push_str("</g>");
            }
            Shape::Edge { id, pts, dashed, label, from, to, conds, arrow } => {
                let eid = format!("{}-{id}", p.key);
                let d: String = pts.iter().enumerate().map(|(k, (x, y))| format!("{}{x:.1},{y:.1}", if k == 0 { "M" } else { " L" })).collect();
                let mut cls = String::from("e");
                if *dashed {
                    cls.push_str(" dash");
                }
                if !*arrow {
                    cls.push_str(" inner");
                }
                edges.push_str(&format!("<g class=\"{cls}\" data-id=\"{eid}\"><path d=\"{d}\"/>"));
                if let Some((t, x, y)) = label {
                    edges.push_str(&format!("<text class=\"el\" x=\"{x:.1}\" y=\"{y:.1}\">"));
                    for (k, line) in label_lines(t).iter().enumerate() {
                        let dy = if k == 0 { 0.0 } else { LABEL_LH };
                        edges.push_str(&format!("<tspan x=\"{x:.1}\" dy=\"{dy}\">{}</tspan>", esc(line)));
                    }
                    edges.push_str("</text>");
                }
                edges.push_str("</g>");
                infos.push(EdgeInfo { id: eid, from: from.clone(), to: to.clone(), conds: conds.clone() });
            }
            Shape::Node { n, x, y, w, h } => {
                nodes.push_str(&node_svg(g, *n, *x, *y, *w, *h, tick));
            }
        }
    }
    let svg = format!(
        "<svg class=\"dd-svg\" xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {w} {h}\" width=\"{w}\" height=\"{h}\"><g class=\"frames\">{frames}</g><g class=\"edges\">{edges}</g><g class=\"nodes\">{nodes}</g></svg>"
    );
    Drawn { svg, edges: infos }
}

fn node_svg(g: &Graph, n: usize, x: f64, y: f64, w: f64, h: f64, tick: &dyn Fn(&str) -> Option<String>) -> String {
    let node = &g.nodes[n];
    let kind = match node.kind {
        Kind::Start => "start",
        Kind::Call => "call",
        Kind::Rule => "rule",
        Kind::Receive => "receive",
        Kind::Match => "match",
        Kind::Wait => "wait",
        Kind::Let => "let",
        Kind::Succeed => "succeed",
        Kind::Fail => "fail",
        Kind::Break => "break",
        Kind::Entry => "entry",
        Kind::End => "end",
    };
    let extra = match (node.kind, node.id.as_str()) {
        (Kind::End, "fin") => " ok",
        (Kind::End, "onfEnd") => " bad",
        _ => "",
    };
    let mut o = format!("<g class=\"n k-{kind}{extra}\" data-id=\"{}\" tabindex=\"0\">", node.id);
    let shape = match node.kind {
        Kind::Match => {
            let cy = y + h / 2.0;
            format!(
                "<polygon class=\"shape\" points=\"{x:.1},{cy:.1} {:.1},{y:.1} {:.1},{y:.1} {:.1},{cy:.1} {:.1},{:.1} {:.1},{:.1}\"/>",
                x + 13.0,
                x + w - 13.0,
                x + w,
                x + w - 13.0,
                y + h,
                x + 13.0,
                y + h
            )
        }
        Kind::Receive => format!("<polygon class=\"shape\" points=\"{:.1},{y:.1} {:.1},{y:.1} {:.1},{:.1} {x:.1},{:.1}\"/>", x + 9.0, x + w, x + w - 9.0, y + h, y + h),
        Kind::Rule => format!(
            "<rect class=\"shape\" x=\"{x:.1}\" y=\"{y:.1}\" width=\"{w:.1}\" height=\"{h:.1}\" rx=\"3\"/><path class=\"side\" d=\"M{:.1},{y:.1} V{:.1} M{:.1},{y:.1} V{:.1}\"/>",
            x + 6.0,
            y + h,
            x + w - 6.0,
            y + h
        ),
        Kind::Call | Kind::Let => format!("<rect class=\"shape\" x=\"{x:.1}\" y=\"{y:.1}\" width=\"{w:.1}\" height=\"{h:.1}\" rx=\"4\"/>"),
        Kind::Wait => format!("<rect class=\"shape\" x=\"{x:.1}\" y=\"{y:.1}\" width=\"{w:.1}\" height=\"{h:.1}\" rx=\"12\"/>"),
        _ => format!("<rect class=\"shape\" x=\"{x:.1}\" y=\"{y:.1}\" width=\"{w:.1}\" height=\"{h:.1}\" rx=\"{:.1}\"/>", (h / 2.0).min(15.0)),
    };
    o.push_str(&shape);
    let lines = &node.lines;
    let top = y + h / 2.0 - (lines.len() as f64 * LH) / 2.0 + 12.0;
    let left = match node.kind {
        Kind::Match => x + PAD_X + 12.0,
        Kind::Rule => x + PAD_X + 6.0,
        Kind::Receive => x + PAD_X + 8.0,
        Kind::Call | Kind::Let => x + PAD_X,
        _ => x + PAD_X + 6.0,
    };
    for (k, line) in lines.iter().enumerate() {
        let ty = top + k as f64 * LH;
        if k == 0 {
            let num = if node.line > 0 { format!("<tspan class=\"ln\">{}</tspan> ", node.line) } else { String::new() };
            o.push_str(&format!("<text class=\"t\" x=\"{left:.1}\" y=\"{ty:.1}\">{num}{}</text>", esc(line)));
        } else {
            o.push_str(&format!("<text class=\"t sub\" x=\"{left:.1}\" y=\"{ty:.1}\">{}</text>", esc(line)));
        }
    }
    if let Some(t) = tick(&node.id) {
        let (cx, cy) = (x + w, y + 9.0);
        o.push_str(&format!("<g class=\"tick\"><title>{}</title><circle cx=\"{cx:.1}\" cy=\"{cy:.1}\" r=\"6.5\"/><text x=\"{cx:.1}\" y=\"{:.1}\">!</text></g>", esc(&t), cy + 3.5));
    }
    o.push_str("</g>");
    o
}
